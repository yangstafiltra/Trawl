use crate::browser::{self, RenderedPage, SearchCard, VideoCard};
use crate::config::{Bookmark, Config};
use crate::media::TerminalCapabilities;
use ratatui::prelude::*;
use ratatui_image::{picker::Picker, protocol::StatefulProtocol, Resize, ResizeEncodeRender};
use std::io;
use std::sync::{mpsc, Arc};
use url::Url;

pub struct Tab {
    pub url: Url,
    pub title: String,
    pub lines: Vec<Line<'static>>,
    pub links: Vec<browser::Link>,
    pub images: Vec<browser::ImageInfo>,
    pub image_protocols: Vec<(usize, StatefulProtocol, u16, u16)>,
    pub video_cards: Vec<browser::VideoCard>,
    pub search_cards: Vec<SearchCard>,
    pub tab_mode: Mode,
    pub scroll: usize,
    pub history: Vec<String>,
    pub forward: Vec<String>,
    pub loading: bool,
    pub video_page: u32,
    pub video_loading_more: bool,
    pub slot: usize,
    pub search_query: String,
    pub search_page: usize,
    pub next_page_url: Option<String>,
    pub prev_page_url: Option<String>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TilingMode {
    Auto,
    Vertical,
    Horizontal,
    Master,
    Single,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Mode {
    Normal,
    InsertUrl,
    InsertSearch,
    Help,
    Link,
    View,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum SidebarTab {
    Links,
    History,
    Bookmarks,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum FocusPanel {
    Left,
    Center,
    Right,
}

pub struct App {
    pub mode: Mode,
    pub tiling_mode: TilingMode,
    pub tabs: Vec<Tab>,
    pub active_tab: usize,
    pub active_slot: usize,
    pub input_buf: String,
    pub sidebar_tab: SidebarTab,
    pub sidebar_idx: usize,
    pub sidebar_visible: bool,
    pub bookmarks: Vec<Bookmark>,
    pub history: Vec<String>,
    pub term_cap: TerminalCapabilities,
    pub config: Config,
    pub focus_frame_visible: bool,
    pub focus_panel: FocusPanel,
    pub help_scroll: usize,
    pub message: String,
    pub should_quit: bool,
    pub quit_pending: bool,
    pub card_h: u16,
    pub fetch_tx: mpsc::Sender<(String, Result<RenderedPage, String>)>,
    fetch_rx: mpsc::Receiver<(String, Result<RenderedPage, String>)>,
    api_fetch_tx: mpsc::Sender<Vec<VideoCard>>,
    api_fetch_rx: mpsc::Receiver<Vec<VideoCard>>,
    cover_tx: mpsc::Sender<(usize, StatefulProtocol)>,
    cover_rx: mpsc::Receiver<(usize, StatefulProtocol)>,
    img_tx: mpsc::Sender<(String, usize, StatefulProtocol, u16, u16)>,
    img_rx: mpsc::Receiver<(String, usize, StatefulProtocol, u16, u16)>,
    picker: Arc<Picker>,
    tick_count: u64,
    pub collecting_link: bool,
    pub search_selected: usize,
    pub search_scroll: usize,
    pub cursor: usize,
    pub cursor_col: usize,
    pub view_sel_start: Option<(usize, usize)>,
    pub view_sel_end: Option<(usize, usize)>,
}

impl App {
    pub fn new() -> io::Result<Self> {
        let (fetch_tx, fetch_rx) = mpsc::channel();
        let (api_fetch_tx, api_fetch_rx) = mpsc::channel();
        let (cover_tx, cover_rx) = mpsc::channel();
        let (img_tx, img_rx) = mpsc::channel();
        let picker = Arc::new(Picker::from_query_stdio().unwrap_or_else(|_| Picker::halfblocks()));
        let term_cap = TerminalCapabilities::detect();
        let config = Config::load();
        let bookmarks = config.bookmarks.clone();

        let mut app = Self {
            mode: Mode::Normal,
            tiling_mode: TilingMode::Auto,
            tabs: Vec::new(),
            active_tab: usize::MAX,
            active_slot: 1,
            input_buf: String::new(),
            sidebar_tab: SidebarTab::Links,
            sidebar_idx: 0,
            sidebar_visible: true,
            bookmarks,
            history: Vec::new(),
            term_cap,
            config,
            focus_frame_visible: true,
            focus_panel: FocusPanel::Center,
            help_scroll: 0,
            message: String::new(),
            should_quit: false,
            quit_pending: false,
            card_h: 7,
            fetch_tx: fetch_tx.clone(),
            fetch_rx,
            api_fetch_tx,
            api_fetch_rx,
            cover_tx,
            cover_rx,
            img_tx,
            img_rx,
            picker,
            tick_count: 0,
            collecting_link: false,
            search_selected: 0,
            search_scroll: 0,
            cursor: 0,
            cursor_col: 0,
            view_sel_start: None,
            view_sel_end: None,
        };
        app.new_tab("trawl:home".into());
        Ok(app)
    }

    pub fn tick(&mut self) {
        self.tick_count += 1;

        // check for loaded pages
        while let Ok((url_str, result)) = self.fetch_rx.try_recv() {
            let idx = self.active_tab;
            if idx >= self.tabs.len() {
                continue;
            }
            if self.tabs[idx].url.to_string() != url_str {
                continue;
            }

            match result {
                Ok(page) => {
                    let mut link_count = 0;
                    let mut card_count = 0;
                    let is_video;
                    {
                        let tab = &mut self.tabs[idx];
                        tab.title = page.title;
                        tab.lines = page.lines;
                        tab.links = page.links;
                        tab.images = page.images;
                        tab.image_protocols.clear();
                        is_video = should_use_lazygit_layout(&tab.url.to_string());
                        if is_video {
                            let url = tab.url.clone();
                            link_count = tab.links.len();
                            let (cards, start_page) = browser::fetch_videos_from_api(&url.to_string());
                            tab.video_cards = cards;
                            tab.video_page = start_page;
                            if tab.video_cards.is_empty() {
                                let embedded = browser::extract_embedded_videos(&page.raw_html, &url);
                                if !embedded.is_empty() {
                                    tab.video_cards = embedded;
                                } else {
                                    tab.video_cards = browser::extract_video_cards(&tab.links);
                                }
                            }
                            card_count = tab.video_cards.len();
                            for (i, card) in tab.video_cards.iter_mut().enumerate() {
                                card.number = i + 1;
                            }
                        } else {
                            tab.video_cards.clear();
                            if !tab.links.is_empty() {
                                let is_search = crate::config::SearchEngine::is_search_host(
                                    tab.url.host_str().unwrap_or(""),
                                );
                                if is_search && self.mode != Mode::Link {
                                    self.mode = Mode::Link;
                                    tab.tab_mode = Mode::Link;
                                }
                                if self.mode == Mode::Link {
                                    tab.search_cards =
                                        browser::build_search_cards(&tab.links, &tab.lines);
                                }
                            }
                        }
                        tab.loading = false;
                        tab.scroll = 0;
                    }
                    if is_video {
                        self.start_cover_downloads();
                        self.message = format!("Loaded: {} links, {} videos", link_count, card_count);
                        if self.config.layout != crate::config::LayoutStyle::LazyGit {
                            self.config.layout = crate::config::LayoutStyle::LazyGit;
                        }
                    } else {
                        self.start_image_downloads();
                        let link_cnt = self.tabs[idx].links.len();
                        let img_cnt = self.tabs[idx].images.len();
                        self.message = format!("Loaded: {} links, {} images", link_cnt, img_cnt);
                        if link_cnt > 0 {
                            self.message = format!("Search: {} results", link_cnt);
                        }
                    }
                }
                Err(e) => {
                    let err_msg = e.clone();
                    if let Some(tab) = self.tabs.get_mut(idx) {
                        tab.loading = false;
                        tab.lines = vec![
                            Line::from(Span::styled(
                                " Error:",
                                Style::default().fg(Color::Red).add_modifier(Modifier::BOLD),
                            )),
                            Line::from(Span::styled(err_msg, Style::default().fg(Color::LightRed))),
                        ];
                    }
                    self.message = e;
                }
            }
        }

        // check for cover download results
        while let Ok((card_idx, protocol)) = self.cover_rx.try_recv() {
            if let Some(tab) = self.active_tab_mut() {
                if let Some(card) = tab.video_cards.get_mut(card_idx) {
                    card.cover = Some(protocol);
                }
            }
        }

        // check for image download results
        while let Ok((img_url, line_idx, protocol, cell_w, cell_h)) = self.img_rx.try_recv() {
            if let Some(tab) = self.active_tab_mut() {
                let still_relevant = tab.images.iter().any(|img| img.src == img_url);
                if still_relevant {
                    tab.image_protocols.push((line_idx, protocol, cell_w, cell_h));
                }
            }
        }

        // check for API pagination results
        while let Ok(more_cards) = self.api_fetch_rx.try_recv() {
            let idx = self.active_tab;
            if idx >= self.tabs.len() {
                continue;
            }
            let tab = &mut self.tabs[idx];
            tab.video_loading_more = false;
            let before = tab.video_cards.len();
            for mut card in more_cards {
                card.number = before + tab.video_cards.len() + 1;
                tab.video_cards.push(card);
            }
            let added = tab.video_cards.len() - before;
            tab.video_page += 1;
            self.message = format!("Loaded {} more videos (page {})", added, tab.video_page);
            self.start_cover_downloads();
        }

        // auto-load more when near bottom of video grid
        let should_load = self.tabs.get(self.active_tab).map_or(false, |tab| {
            !tab.video_cards.is_empty()
                && !tab.video_loading_more
                && tab.video_page < 10
                && {
                    let selected = self.card_selected();
                    let total = tab.video_cards.len();
                    total >= 10 && selected + 3 >= total
                }
        });
        if should_load {
            let next_page = self.tabs[self.active_tab].video_page + 1;
            let tx = self.api_fetch_tx.clone();
            std::thread::spawn(move || {
                let cards = browser::fetch_videos_api_page(0, next_page);
                if !cards.is_empty() {
                    let _ = tx.send(cards);
                }
            });
            if let Some(t) = self.active_tab_mut() {
                t.video_loading_more = true;
                self.message = format!("Loading page {}...", next_page);
            }
        }

        if let Some(tab) = self.tabs.get(self.active_tab)
            && tab.loading
            && self.tick_count.is_multiple_of(8)
        {
            self.message = format!(" Fetching {} ...", tab.url);
        }
    }

    pub fn new_tab(&mut self, url_str: String) {
        let slot = self.next_free_slot();
        let is_home = url_str == "trawl:home";
        if is_home {
            self.tabs.push(Tab {
                url: Url::parse("trawl:home").unwrap(),
                title: "Home".into(),
                lines: vec![],
                links: vec![],
                images: vec![],
                image_protocols: vec![],
                video_cards: vec![],
                search_cards: vec![],
                tab_mode: Mode::InsertSearch,
                video_page: 1,
                video_loading_more: false,
                scroll: 0,
                history: vec![],
                forward: vec![],
                loading: false,
                slot,
                search_query: String::new(),
                search_page: 1,
                next_page_url: None,
                prev_page_url: None,
            });
            self.switch_tab(self.tabs.len() - 1);
            self.active_slot = slot;
            self.input_buf.clear();
            return;
        }

        let url = Url::parse(&url_str)
            .or_else(|_| Url::parse(&format!("https://{url_str}")))
            .unwrap_or_else(|_| Url::parse("https://example.com").unwrap());

        if should_use_lazygit_layout(&url.to_string()) {
            self.config.layout = crate::config::LayoutStyle::LazyGit;
        }

        self.tabs.push(Tab {
            url: url.clone(),
            title: "Loading...".into(),
            lines: vec![
                Line::from(Span::styled(
                    " Loading...",
                    Style::default().fg(Color::Yellow),
                )),
                Line::from(Span::styled(
                    format!(" {url}"),
                    Style::default().fg(Color::Cyan),
                )),
            ],
            links: vec![],
            images: vec![],
            image_protocols: vec![],
            video_cards: vec![],
            search_cards: vec![],
            tab_mode: Mode::Normal,
            video_page: 1,
            video_loading_more: false,
            scroll: 0,
            history: vec![],
            forward: vec![],
            loading: true,
            slot,
            search_query: String::new(),
            search_page: 1,
            next_page_url: None,
            prev_page_url: None,
        });
        self.switch_tab(self.tabs.len() - 1);
        self.active_slot = slot;
        self.fetch_page(url.to_string());
    }

    fn fetch_page(&self, url_str: String) {
        let tx = self.fetch_tx.clone();
        std::thread::spawn(move || {
            let result = browser::fetch_page(&url_str);
            let _ = tx.send((url_str, result));
        });
    }

    fn start_cover_downloads(&self) {
        let idx = self.active_tab;
        if idx >= self.tabs.len() {
            return;
        }
        let tab = &self.tabs[idx];
        let cols = self.card_cols();
        let scroll_row = self.card_scroll_row();
        let vis_rows = 4usize;
        let card_start = scroll_row * cols;
        let card_end = (card_start + vis_rows * cols).min(tab.video_cards.len());
        let cover_tx = self.cover_tx.clone();
        let picker = self.picker.clone();
        let card_h = self.card_h;
        for (i, card) in tab.video_cards.iter().enumerate() {
            if i < card_start || i >= card_end {
                continue;
            }
            if card.thumb_url.is_empty() || card.cover.is_some() {
                continue;
            }
            let url = card.thumb_url.clone();
            let cover_tx = cover_tx.clone();
            let picker = picker.clone();
            std::thread::spawn(move || {
                let agent = ureq::Agent::new_with_defaults();
                let mut resp = match agent.get(&url).call() {
                    Ok(r) => r,
                    Err(_) => return,
                };
                let data: Vec<u8> = match resp.body_mut().read_to_vec() {
                    Ok(d) => d,
                    Err(_) => return,
                };
                let img = match image::load_from_memory(&data) {
                    Ok(img) => img.thumbnail(200, 125),
                    Err(_) => return,
                };
                let mut protocol = picker.new_resize_protocol(img);
                let encode_h = (card_h as u16).saturating_sub(3).max(2);
                protocol.resize_encode(&Resize::Fit(None), Size::new(20, encode_h));
                let _ = cover_tx.send((i, protocol));
            });
        }
    }

    fn start_image_downloads(&self) {
        let idx = self.active_tab;
        if idx >= self.tabs.len() {
            return;
        }
        let tab = &self.tabs[idx];
        if tab.images.is_empty() {
            return;
        }
        let img_tx = self.img_tx.clone();
        let picker = self.picker.clone();
        for img in &tab.images {
            if img.src.is_empty() {
                continue;
            }
            let url = img.src.clone();
            let line_idx = img.line_idx;
            let img_tx = img_tx.clone();
            let picker = picker.clone();
            std::thread::spawn(move || {
                let agent = ureq::Agent::new_with_defaults();
                let mut resp = match agent.get(&url).call() {
                    Ok(r) => r,
                    Err(_) => return,
                };
                let data: Vec<u8> = match resp.body_mut().read_to_vec() {
                    Ok(d) => d,
                    Err(_) => return,
                };
                let img = match image::load_from_memory(&data) {
                    Ok(img) => img,
                    Err(_) => return,
                };
                let mut protocol = picker.new_resize_protocol(img);
                let cell_size = protocol.size_for(Resize::Fit(None), Size::new(80, 20));
                protocol.resize_encode(&Resize::Fit(None), Size::new(80, 20));
                let _ = img_tx.send((url, line_idx, protocol, cell_size.width, cell_size.height));
            });
        }
    }

    pub fn navigate_to(&mut self, url_str: String) {
        let idx = self.active_tab;
        if idx >= self.tabs.len() {
            return;
        }

        if url_str == "trawl:home" {
            let prev = self.tabs[idx].url.to_string();
            if prev != "trawl:home" {
                self.tabs[idx].history.push(prev);
            }
            self.tabs[idx].url = Url::parse("trawl:home").unwrap();
            self.tabs[idx].title = "Home".into();
            self.tabs[idx].lines.clear();
            self.tabs[idx].links.clear();
            self.tabs[idx].images.clear();
            self.tabs[idx].image_protocols.clear();
            self.tabs[idx].video_cards.clear();
            self.tabs[idx].search_cards.clear();
            self.tabs[idx].scroll = 0;
            self.tabs[idx].loading = false;
            self.mode = Mode::InsertSearch;
            self.tabs[idx].tab_mode = Mode::InsertSearch;
            self.input_buf.clear();
            return;
        }

        let prev = self.tabs[idx].url.to_string();

        if let Ok(url) = Url::parse(&url_str).or_else(|_| Url::parse(&format!("https://{url_str}")))
        {
            if should_use_lazygit_layout(&url.to_string()) {
                self.config.layout = crate::config::LayoutStyle::LazyGit;
            }
            self.tabs[idx].history.push(prev);
            self.tabs[idx].forward.clear();
            self.tabs[idx].url = url.clone();
            self.tabs[idx].title = "Loading...".into();
            self.tabs[idx].scroll = 0;
            self.tabs[idx].lines = vec![
                Line::from(Span::styled(
                    " Loading...",
                    Style::default().fg(Color::Yellow),
                )),
                Line::from(Span::styled(
                    format!(" {url}"),
                    Style::default().fg(Color::Cyan),
                )),
            ];
            self.tabs[idx].links.clear();
            self.tabs[idx].images.clear();
            self.tabs[idx].image_protocols.clear();
            self.tabs[idx].loading = true;
            self.tabs[idx].video_cards.clear();
            self.tabs[idx].search_cards.clear();
            self.tabs[idx].video_page = 1;
            self.fetch_page(url.to_string());
        }
        self.search_selected = 0;
        self.search_scroll = 0;
    }

    pub fn search(&mut self, query: &str) {
        let url = self.config.search_engine.search_url(query);
        self.config.layout = crate::config::LayoutStyle::BrowserChrome;
        self.navigate_to(url);
        self.mode = Mode::Link;
        if let Some(tab) = self.tabs.get_mut(self.active_tab) {
            tab.tab_mode = Mode::Link;
            tab.search_query = query.to_string();
            tab.search_page = 1;
        }
        self.search_selected = 0;
        self.search_scroll = 0;
    }

    pub fn search_next_page(&mut self) {
        if !self.config.search_engine.supports_pagination() {
            if let Some(note) = self.config.search_engine.pagination_limit_note() {
                self.message = note.to_string();
            }
            return;
        }
        let idx = self.active_tab;
        if idx >= self.tabs.len() { return; }
        let page = self.tabs[idx].search_page + 1;
        let query = self.tabs[idx].search_query.clone();
        if query.is_empty() { return; }
        let url = self.config.search_engine.search_url_with_page(&query, page);
        self.config.layout = crate::config::LayoutStyle::BrowserChrome;
        self.navigate_to(url);
        self.mode = Mode::Link;
        if let Some(tab) = self.tabs.get_mut(self.active_tab) {
            tab.tab_mode = Mode::Link;
            tab.search_page = page;
        }
        self.search_selected = 0;
        self.search_scroll = 0;
        self.message = format!(" Search page {page}");
    }

    pub fn search_prev_page(&mut self) {
        if !self.config.search_engine.supports_pagination() {
            if let Some(note) = self.config.search_engine.pagination_limit_note() {
                self.message = note.to_string();
            }
            return;
        }
        let idx = self.active_tab;
        if idx >= self.tabs.len() { return; }
        let page = self.tabs[idx].search_page.saturating_sub(1).max(1);
        let query = self.tabs[idx].search_query.clone();
        if query.is_empty() { return; }
        if page == self.tabs[idx].search_page { return; }
        let url = self.config.search_engine.search_url_with_page(&query, page);
        self.config.layout = crate::config::LayoutStyle::BrowserChrome;
        self.navigate_to(url);
        self.mode = Mode::Link;
        if let Some(tab) = self.tabs.get_mut(self.active_tab) {
            tab.tab_mode = Mode::Link;
            tab.search_page = page;
        }
        self.search_selected = 0;
        self.search_scroll = 0;
        self.message = format!(" Search page {page}");
    }

    pub fn ensure_search_cards(&mut self) {
        if let Some(tab) = self.active_tab_mut() {
            if !tab.links.is_empty() && tab.search_cards.is_empty() {
                tab.search_cards = browser::build_search_cards(&tab.links, &tab.lines);
            }
        }
    }

    pub fn cycle_engine(&mut self) {
        self.config.search_engine = self.config.search_engine.next();
        let _ = Config::save(&self.config);
        self.message = format!(" Engine: {}", self.config.search_engine);
    }

    pub fn toggle_layout(&mut self) {
        self.config.layout = self.config.layout.toggle();
        let _ = Config::save(&self.config);
        self.message = format!(" Layout: {}", self.config.layout);
    }

    pub fn go_back(&mut self) {
        let idx = self.active_tab;
        if idx >= self.tabs.len() {
            return;
        }
        let prev = self.tabs[idx].history.pop();
        if let Some(prev_url) = prev {
            let cur = self.tabs[idx].url.to_string();
            self.tabs[idx].forward.push(cur);
            self.navigate_to(prev_url);
        }
    }

    pub fn go_forward(&mut self) {
        let idx = self.active_tab;
        if idx >= self.tabs.len() {
            return;
        }
        let next = self.tabs[idx].forward.pop();
        if let Some(next_url) = next {
            let cur = self.tabs[idx].url.to_string();
            self.tabs[idx].history.push(cur);
            self.navigate_to(next_url);
        }
    }

    pub fn switch_tab(&mut self, idx: usize) {
        if idx >= self.tabs.len() || idx == self.active_tab {
            return;
        }
        if let Some(tab) = self.tabs.get_mut(self.active_tab) {
            tab.tab_mode = self.mode;
        }
        self.active_tab = idx;
        self.mode = self.tabs[idx].tab_mode;
    }

    pub fn is_home(&self) -> bool {
        self.active_tab()
            .is_some_and(|t| t.url.as_str() == "trawl:home")
    }

    pub fn handle_quit(&mut self) {
        if self.is_home() {
            self.message = "Cannot quit from home page".into();
            return;
        }
        if self.quit_pending {
            self.should_quit = true;
            self.message.clear();
        } else {
            self.quit_pending = true;
            self.message = " Press q again to quit".into();
        }
    }

    pub fn set_tiling(&mut self, mode: TilingMode) {
        self.tiling_mode = mode;
        self.message = format!(" {:?}", mode);
    }

    pub fn toggle_fullscreen(&mut self) {
        self.tiling_mode = match self.tiling_mode {
            TilingMode::Single => TilingMode::Auto,
            _ => TilingMode::Single,
        };
        self.message = format!(" {:?}", self.tiling_mode);
    }

    pub fn move_tab_to(&mut self, target: usize) {
        let current = self.active_tab;
        if current == target || target >= self.tabs.len() {
            return;
        }
        let tab = self.tabs.remove(current);
        let new_idx = if target > current { target - 1 } else { target };
        self.tabs.insert(new_idx, tab);
        self.active_tab = new_idx;
    }

    pub fn focus_tile(&mut self, dir: char) {
        let slot = self.active_slot;
        let indices: Vec<usize> = (0..self.tabs.len()).filter(|&i| self.tabs[i].slot == slot).collect();
        if indices.len() <= 1 { return; }
        let pos = indices.iter().position(|&i| i == self.active_tab).unwrap_or(0);
        let cols = match self.tiling_mode {
            TilingMode::Single => 1,
            TilingMode::Horizontal => 1,
            TilingMode::Vertical if indices.len() <= 2 => indices.len(),
            TilingMode::Vertical => 2.min(indices.len()),
            TilingMode::Auto => 2.min(indices.len()),
            TilingMode::Master => 2.min(indices.len()),
        };
        let new_pos = match dir {
            'h' | 'a' => pos.saturating_sub(1),
            'l' | 'd' => (pos + 1).min(indices.len() - 1),
            'j' | 's' => (pos + cols).min(indices.len() - 1),
            'k' | 'w' => pos.saturating_sub(cols),
            _ => pos,
        };
        if new_pos != pos {
            self.switch_tab(indices[new_pos]);
        }
    }

    pub fn swap_tab_dir(&mut self, dir: char) {
        let idx = self.active_tab;
        let n = self.tabs.len();
        let target = match dir {
            'h' | 'a' | 'k' | 'w' => idx.saturating_sub(1),
            'l' | 'd' | 'j' | 's' => (idx + 1).min(n - 1),
            _ => idx,
        };
        if target != idx {
            self.tabs.swap(idx, target);
            self.active_tab = target;
        }
    }

    pub fn close_tab(&mut self) {
        if self.tabs.len() <= 1 {
            return;
        }
        let slot = self.tabs[self.active_tab].slot;
        let old_idx = self.active_tab;
        self.tabs.remove(old_idx);
        if self.tabs.is_empty() {
            return;
        }
        let remaining = self.tabs.iter().filter(|t| t.slot == slot).count();
        if remaining == 0 {
            // slot emptied, move to first tab in next occupied slot
            if let Some(idx) = self.tabs.iter().position(|_| true) {
                self.switch_tab(idx);
                self.active_slot = self.tabs[idx].slot;
            }
        } else {
            if self.active_tab >= self.tabs.len() {
                self.switch_tab(self.tabs.len() - 1);
            }
            if remaining <= 1 && self.tiling_mode != TilingMode::Single {
                self.tiling_mode = TilingMode::Single;
            }
        }
    }

    pub fn next_tab(&mut self) {
        if self.active_tab + 1 < self.tabs.len() {
            self.switch_tab(self.active_tab + 1);
        }
    }

    pub fn prev_tab(&mut self) {
        if self.active_tab > 0 {
            self.switch_tab(self.active_tab - 1);
        }
    }

    pub fn toggle_bookmark(&mut self) {
        if let Some(tab) = self.tabs.get(self.active_tab) {
            let url = tab.url.to_string();
            let title = tab.title.clone();
            if let Some(pos) = self.bookmarks.iter().position(|b| b.url == url) {
                self.bookmarks.remove(pos);
                self.message = "Bookmark removed".into();
            } else {
                self.bookmarks.push(Bookmark { url, title });
                self.message = "Bookmark added".into();
            }
            self.config.bookmarks = self.bookmarks.clone();
            let _ = Config::save(&self.config);
        }
    }

    pub fn follow_link(&mut self, num: usize) {
        let link = self
            .tabs
            .get(self.active_tab)
            .and_then(|tab| tab.links.iter().find(|l| l.number == num))
            .map(|l| l.url.clone());
        if let Some(url) = link {
            self.new_tab(url);
        }
    }

    pub fn active_tab(&self) -> Option<&Tab> {
        self.tabs.get(self.active_tab)
    }

    pub fn active_tab_mut(&mut self) -> Option<&mut Tab> {
        self.tabs.get_mut(self.active_tab)
    }

    pub fn next_free_slot(&self) -> usize {
        for s in 1..=9 {
            if !self.tabs.iter().any(|t| t.slot == s) {
                return s;
            }
        }
        self.tabs.len() + 1
    }

    pub fn switch_slot(&mut self, slot: usize) {
        if slot < 1 || slot > 9 || slot == self.active_slot {
            return;
        }
        if let Some(idx) = self.tabs.iter().position(|t| t.slot == slot) {
            self.active_slot = slot;
            self.switch_tab(idx);
        }
    }

    pub fn move_tab_to_slot(&mut self, slot: usize) {
        if slot < 1 || slot > 9 {
            return;
        }
        let idx = self.active_tab;
        if idx >= self.tabs.len() {
            return;
        }
        let old_slot = self.tabs[idx].slot;
        if old_slot == slot {
            return;
        }
        self.tabs[idx].slot = slot;
        let count = self.tabs.iter().filter(|t| t.slot == slot).count();
        if count > 1 {
            self.tiling_mode = TilingMode::Auto;
            self.message = format!(" Tiling slot {slot} ({count} tabs)");
        } else {
            self.tiling_mode = TilingMode::Single;
            self.message = format!(" Slot {slot}");
        }
        self.active_slot = slot;
    }

    pub fn is_video_page(&self) -> bool {
        self.active_tab().is_some_and(|t| !t.video_cards.is_empty())
    }

    pub fn has_search_cards(&self) -> bool {
        self.active_tab().is_some_and(|t| !t.search_cards.is_empty())
    }

    pub fn search_up(&mut self) {
        if self.search_selected > 0 {
            self.search_selected -= 1;
            self.update_search_scroll();
        }
    }

    pub fn search_down(&mut self) {
        if let Some(tab) = self.active_tab() {
            if self.search_selected + 1 < tab.search_cards.len() {
                self.search_selected += 1;
                self.update_search_scroll();
            }
        }
    }

    pub fn update_search_scroll(&mut self) {}

    pub fn search_follow(&self) -> Option<String> {
        self.active_tab().and_then(|t| {
            t.search_cards.get(self.search_selected).map(|c| c.url.clone())
        })
    }

    pub fn card_cols(&self) -> usize {
        3
    }

    pub fn card_selected(&self) -> usize {
        self.sidebar_idx
    }

    pub fn card_selected_mut(&mut self) -> &mut usize {
        &mut self.sidebar_idx
    }

    pub fn card_scroll_row(&self) -> usize {
        self.help_scroll
    }

    pub fn card_scroll_row_mut(&mut self) -> &mut usize {
        &mut self.help_scroll
    }

    pub fn video_up(&mut self) {
        if let Some(tab) = self.active_tab() {
            if tab.video_cards.is_empty() {
                return;
            }
            let cols = self.card_cols();
            let new = self.card_selected().saturating_sub(cols);
            *self.card_selected_mut() = new;
            self.update_card_scroll();
        }
    }

    pub fn video_down(&mut self) {
        if let Some(tab) = self.active_tab() {
            if tab.video_cards.is_empty() {
                return;
            }
            let cols = self.card_cols();
            let new = self.card_selected() + cols;
            if new < tab.video_cards.len() {
                *self.card_selected_mut() = new;
            }
            self.update_card_scroll();
        }
    }

    pub fn video_left(&mut self) {
        if let Some(tab) = self.active_tab() {
            if tab.video_cards.is_empty() {
                return;
            }
            if self.card_selected() > 0 {
                *self.card_selected_mut() -= 1;
                self.update_card_scroll();
            }
        }
    }

    pub fn video_right(&mut self) {
        if let Some(tab) = self.active_tab() {
            if tab.video_cards.is_empty() {
                return;
            }
            let new = self.card_selected() + 1;
            if new < tab.video_cards.len() {
                *self.card_selected_mut() = new;
                self.update_card_scroll();
            }
        }
    }

    pub fn video_play(&self) -> Option<String> {
        self.active_tab().and_then(|tab| {
            tab.video_cards
                .get(self.card_selected())
                .map(|card| card.url.clone())
        })
    }

    pub fn update_card_scroll(&mut self) {
        if let Some(_tab) = self.active_tab() {
            let cols = self.card_cols();
            let selected_row = self.card_selected() / cols;
            let scroll_row = self.card_scroll_row();
            let vis_rows = 4usize;
            if selected_row < scroll_row {
                *self.card_scroll_row_mut() = selected_row;
            } else if selected_row >= scroll_row + vis_rows {
                *self.card_scroll_row_mut() = selected_row - vis_rows + 1;
            }
        }
        self.start_cover_downloads();
    }

    pub fn toggle_focus_frame(&mut self) {
        self.focus_frame_visible = !self.focus_frame_visible;
        self.message = format!(
            " Focus frame: {}",
            if self.focus_frame_visible {
                "ON"
            } else {
                "OFF"
            }
        );
    }

    pub fn focus_next_panel(&mut self) {
        self.focus_panel = match self.focus_panel {
            FocusPanel::Left => FocusPanel::Center,
            FocusPanel::Center => FocusPanel::Left,
            FocusPanel::Right => FocusPanel::Left,
        };
    }

    pub fn sidebar_down(&mut self) {
        let max = match self.sidebar_tab {
            SidebarTab::Links => self.active_tab().map(|t| t.links.len()).unwrap_or(0),
            SidebarTab::History => self.history.len().min(50),
            SidebarTab::Bookmarks => self.bookmarks.len(),
        };
        if max > 0 {
            self.sidebar_idx = (self.sidebar_idx + 1).min(max - 1);
        }
    }

    pub fn sidebar_up(&mut self) {
        self.sidebar_idx = self.sidebar_idx.saturating_sub(1);
    }

    pub fn open_sidebar_selection(&mut self) {
        let url = match self.sidebar_tab {
            SidebarTab::Links => self
                .active_tab()
                .and_then(|t| t.links.get(self.sidebar_idx))
                .map(|l| l.url.clone()),
            SidebarTab::History => {
                self.history.iter().rev().nth(self.sidebar_idx).cloned()
            }
            SidebarTab::Bookmarks => self
                .bookmarks
                .get(self.sidebar_idx)
                .map(|bm| bm.url.clone()),
        };
        if let Some(url) = url {
            self.focus_panel = FocusPanel::Center;
            self.new_tab(url);
        }
    }
}

pub fn is_video_site(url: &str) -> bool {
    let lower = url.to_lowercase();
    lower.contains("bilibili.com")
        || lower.contains("youtube.com")
        || lower.contains("youtu.be")
        || lower.contains("twitch.tv")
        || lower.contains("vimeo.com")
        || lower.contains("nicovideo.jp")
        || lower.contains("dailymotion.com")
}

pub fn should_use_lazygit_layout(url: &str) -> bool {
    is_video_site(url)
}

pub fn word_range(text: &str, col: usize) -> (usize, usize) {
    let chars: Vec<char> = text.chars().collect();
    if chars.is_empty() || col >= chars.len() {
        return (0, 0);
    }
    if chars[col].is_whitespace() {
        return (col, col);
    }
    let start = chars[..=col]
        .iter()
        .rposition(|c| c.is_whitespace())
        .map_or(0, |i| i + 1);
    let end = chars[col..]
        .iter()
        .position(|c| c.is_whitespace())
        .map_or(chars.len(), |i| col + i);
    (start, end)
}

pub fn line_text(line: &ratatui::text::Line<'static>) -> String {
    line.spans.iter().map(|s| s.content.as_ref()).collect()
}

pub fn copy_to_clipboard(text: &str) {
    use std::process::Command;
    let _ = Command::new("wl-copy").arg(text).output();
}

pub fn extract_text(lines: &[ratatui::text::Line<'static>], start: (usize, usize), end: (usize, usize)) -> String {
    let (sl, sc) = start;
    let (el, ec) = end;
    if sl >= lines.len() || el >= lines.len() {
        return String::new();
    }
    let (s_line, s_col, e_line, e_col) = if (sl, sc) <= (el, ec) {
        (sl, sc, el, ec)
    } else {
        (el, ec, sl, sc)
    };
    let mut result = String::new();
    for li in s_line..=e_line {
        let t = line_text(&lines[li]);
        let chars: Vec<char> = t.chars().collect();
        if s_line == e_line {
            let end_col = e_col.min(chars.len());
            if s_col < end_col {
                result += &chars[s_col..end_col].iter().collect::<String>();
            }
        } else if li == s_line {
            if s_col < chars.len() {
                result += &chars[s_col..].iter().collect::<String>();
            }
            result += "\n";
        } else if li == e_line {
            let end_col = e_col.min(chars.len());
            result += &chars[..end_col].iter().collect::<String>();
        } else {
            result += &chars.iter().collect::<String>();
            result += "\n";
        }
    }
    result
}
