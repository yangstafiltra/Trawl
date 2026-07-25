use crate::app::{App, Mode, SidebarTab, TilingMode};

pub fn handle_key(app: &mut App, key: crossterm::event::KeyEvent) {
    if key.code != crossterm::event::KeyCode::Char('q') {
        app.quit_pending = false;
    }
    if key.modifiers == crossterm::event::KeyModifiers::CONTROL {
        match key.code {
            crossterm::event::KeyCode::Char('c') => { app.should_quit = true; return; }
            crossterm::event::KeyCode::Char('s') => { app.cycle_engine(); return; }
            crossterm::event::KeyCode::Char('t') => { app.new_tab("trawl:home".into()); return; }
            _ => {}
        }
    }
    if key.modifiers.contains(crossterm::event::KeyModifiers::ALT) {
        let shift = key.modifiers.contains(crossterm::event::KeyModifiers::SHIFT);
        if handle_tiling(app, key.code, shift) {
            return;
        }
    }
    match key.code {
        crossterm::event::KeyCode::Char(']') => { app.search_next_page(); return; }
        crossterm::event::KeyCode::Char('[') => { app.search_prev_page(); return; }
        _ => {}
    }
    match app.mode {
        Mode::Normal => handle_normal(app, key),
        Mode::Link => handle_link(app, key),
        Mode::View => handle_view(app, key),
        Mode::InsertUrl => handle_insert(app, key, false),
        Mode::InsertSearch => handle_insert(app, key, true),
        Mode::Help => handle_help(app, key),
    }
}

fn handle_normal(app: &mut App, key: crossterm::event::KeyEvent) {
    if app.is_video_page() {
        handle_video_normal(app, key);
        return;
    }
    handle_text_normal(app, key);
}

fn handle_text_normal(app: &mut App, key: crossterm::event::KeyEvent) {
    if app.config.layout == crate::config::LayoutStyle::LazyGit
        && app.focus_panel == crate::app::FocusPanel::Left
    {
        match key.code {
            crossterm::event::KeyCode::Char('j')
            | crossterm::event::KeyCode::Down => {
                app.collecting_link = false;
                app.message.clear();
                app.sidebar_down();
            }
            crossterm::event::KeyCode::Char('k')
            | crossterm::event::KeyCode::Up => {
                app.collecting_link = false;
                app.message.clear();
                app.sidebar_up();
            }
            crossterm::event::KeyCode::Enter => {
                if app.collecting_link {
                    if let Ok(num) = app.input_buf.parse::<usize>() {
                        app.follow_link(num);
                    }
                    app.collecting_link = false;
                    app.message.clear();
                } else {
                    app.open_sidebar_selection();
                }
            }
            crossterm::event::KeyCode::Esc => {
                app.collecting_link = false;
                app.message.clear();
            }
            crossterm::event::KeyCode::Char('/') => {
                app.collecting_link = true;
                app.input_buf.clear();
                app.message = " Go to link #".into();
            }
            crossterm::event::KeyCode::Char(c) if app.collecting_link && c.is_ascii_digit() => {
                app.input_buf.push(c);
                app.message = format!(" Go to link #{}", app.input_buf);
                if app.sidebar_tab == SidebarTab::Links {
                    if let Ok(num) = app.input_buf.parse::<usize>() {
                        if let Some(tab) = app.active_tab() {
                            if let Some(idx) = tab.links.iter().position(|l| l.number == num) {
                                app.sidebar_idx = idx;
                            }
                        }
                    }
                }
            }
            crossterm::event::KeyCode::Tab => {
                app.collecting_link = false;
                app.message.clear();
                app.focus_next_panel();
            }
            crossterm::event::KeyCode::Char('b') => {
                app.collecting_link = false;
                app.message.clear();
                app.sidebar_tab = SidebarTab::Bookmarks;
                app.sidebar_idx = 0;
            }
            crossterm::event::KeyCode::Char('h') => {
                app.collecting_link = false;
                app.message.clear();
                app.sidebar_tab = SidebarTab::History;
                app.sidebar_idx = 0;
            }
            crossterm::event::KeyCode::Char('l') => {
                app.collecting_link = false;
                app.message.clear();
                app.sidebar_tab = SidebarTab::Links;
                app.sidebar_idx = 0;
            }
            crossterm::event::KeyCode::Char('q') => {
                app.collecting_link = false;
                app.message.clear();
                app.handle_quit();
            }
            _ => {
                app.collecting_link = false;
                app.message.clear();
            }
        }
        return;
    }

    match key.code {
        crossterm::event::KeyCode::Char('q') => app.handle_quit(),
        crossterm::event::KeyCode::Char('j') | crossterm::event::KeyCode::Down => {
            if let Some(t) = app.active_tab_mut() {
                let max = t.lines.len().saturating_sub(1);
                if t.scroll < max { t.scroll += 1; }
            }
        }
        crossterm::event::KeyCode::Char('k') | crossterm::event::KeyCode::Up => {
            if let Some(t) = app.active_tab_mut() {
                t.scroll = t.scroll.saturating_sub(1);
            }
        }
        crossterm::event::KeyCode::Char('d') => {
            if let Some(t) = app.active_tab_mut() {
                let max = t.lines.len().saturating_sub(1);
                t.scroll = (t.scroll + 10).min(max);
            }
        }
        crossterm::event::KeyCode::Char('u') => {
            if let Some(t) = app.active_tab_mut() {
                t.scroll = t.scroll.saturating_sub(10);
            }
        }
        crossterm::event::KeyCode::Char('g') => {
            if let Some(t) = app.active_tab_mut() {
                t.scroll = 0;
            }
        }
        crossterm::event::KeyCode::Char('G') => {
            if let Some(t) = app.active_tab_mut() {
                t.scroll = t.lines.len().saturating_sub(1);
            }
        }
        crossterm::event::KeyCode::Char(':') => {
            app.mode = Mode::InsertUrl;
            app.input_buf = app
                .active_tab()
                .map(|t| t.url.to_string())
                .unwrap_or_default();
        }
        crossterm::event::KeyCode::Char('/') => {
            app.mode = Mode::InsertSearch;
            app.input_buf.clear();
        }
        crossterm::event::KeyCode::Char('H') => app.go_back(),
        crossterm::event::KeyCode::Char('L') => app.go_forward(),
        crossterm::event::KeyCode::Left => app.go_back(),
        crossterm::event::KeyCode::Right => app.go_forward(),
        crossterm::event::KeyCode::Char('t') => {
            app.mode = Mode::InsertUrl;
            app.input_buf.clear();
            app.message = "New tab URL: ".into();
        }
        crossterm::event::KeyCode::Char('T') => app.prev_tab(),
        crossterm::event::KeyCode::Char('N') => app.next_tab(),
        crossterm::event::KeyCode::Char('b') => {
            app.sidebar_tab = SidebarTab::Bookmarks;
            app.sidebar_visible = true;
        }
        crossterm::event::KeyCode::Char('B') => app.toggle_bookmark(),
        crossterm::event::KeyCode::Char('h') => {
            app.sidebar_tab = SidebarTab::History;
            app.sidebar_visible = true;
        }
        crossterm::event::KeyCode::Char('l') => {
            if app.is_home() {
                app.message = " No links on home page".into();
            } else {
                app.mode = Mode::Link;
                app.ensure_search_cards();
                app.message = " LINK mode".into();
            }
        }
        crossterm::event::KeyCode::Char('v') => {
            if app.is_home() {
                app.message = " No view on home page".into();
            } else {
                if let Some(t) = app.active_tab() {
                    if !t.lines.is_empty() {
                        app.cursor = app.cursor.min(t.lines.len() - 1);
                    }
                }
                app.cursor_col = 0;
                app.view_sel_start = None;
                app.view_sel_end = None;
                app.mode = Mode::View;
                app.message = " VIEW mode".into();
            }
        }
        crossterm::event::KeyCode::Enter => {
            app.sidebar_tab = SidebarTab::Links;
            app.sidebar_visible = true;
        }
        crossterm::event::KeyCode::Tab => {
            if app.config.layout == crate::config::LayoutStyle::LazyGit {
                app.focus_next_panel();
            } else {
                app.sidebar_visible = !app.sidebar_visible;
            }
        }
        crossterm::event::KeyCode::Char('\\') => app.toggle_layout(),
        crossterm::event::KeyCode::Char('f') => app.toggle_focus_frame(),
        crossterm::event::KeyCode::Char('?') => {
            app.mode = Mode::Help;
            app.help_scroll = 0;
        }
        crossterm::event::KeyCode::Char('R') => {
            let url = app
                .active_tab()
                .map(|t| t.url.to_string())
                .unwrap_or_default();
            if !url.is_empty() {
                app.navigate_to(url);
            }
        }
        crossterm::event::KeyCode::Char('x') => app.close_tab(),
        crossterm::event::KeyCode::Char(c @ '1'..='9') => {
            let num = (c as u8 - b'0') as usize;
            app.follow_link(num);
        }
        crossterm::event::KeyCode::Esc => app.sidebar_visible = false,
        _ => {}
    }
}

fn handle_video_normal(app: &mut App, key: crossterm::event::KeyEvent) {
    match key.code {
        crossterm::event::KeyCode::Char('j') | crossterm::event::KeyCode::Down => app.video_down(),
        crossterm::event::KeyCode::Char('k') | crossterm::event::KeyCode::Up => app.video_up(),
        crossterm::event::KeyCode::Char('h') | crossterm::event::KeyCode::Left => app.video_left(),
        crossterm::event::KeyCode::Char('l') | crossterm::event::KeyCode::Right => app.video_right(),
        crossterm::event::KeyCode::Enter => {
            if let Some(url) = app.video_play() {
                app.navigate_to(url);
            }
        }
        crossterm::event::KeyCode::Tab => app.focus_next_panel(),
        crossterm::event::KeyCode::Esc => {}
        crossterm::event::KeyCode::Char('p') => {
            if let Some(url) = app.video_play() {
                crate::media::play_with_mpv(&url);
                app.message = format!(" Playing: {url}");
            }
        }
        crossterm::event::KeyCode::Char('g') => {
            *app.card_selected_mut() = 0;
            app.update_card_scroll();
        }
        crossterm::event::KeyCode::Char('G') => {
            if let Some(tab) = app.active_tab() {
                if !tab.video_cards.is_empty() {
                    *app.card_selected_mut() = tab.video_cards.len() - 1;
                    app.update_card_scroll();
                }
            }
        }
        crossterm::event::KeyCode::Char('d') => {
            if let Some(t) = app.active_tab_mut() {
                let max = t.lines.len().saturating_sub(1);
                t.scroll = (t.scroll + 10).min(max);
            }
        }
        crossterm::event::KeyCode::Char('u') => {
            if let Some(t) = app.active_tab_mut() {
                t.scroll = t.scroll.saturating_sub(10);
            }
        }
        crossterm::event::KeyCode::Char('q') => app.handle_quit(),
        crossterm::event::KeyCode::Char(':') => {
            app.mode = Mode::InsertUrl;
            app.input_buf = app
                .active_tab()
                .map(|t| t.url.to_string())
                .unwrap_or_default();
        }
        crossterm::event::KeyCode::Char('/') => {
            app.mode = Mode::InsertSearch;
            app.input_buf.clear();
        }
        crossterm::event::KeyCode::Char('t') => {
            app.mode = Mode::InsertUrl;
            app.input_buf.clear();
            app.message = "New tab URL: ".into();
        }
        crossterm::event::KeyCode::Char('T') => app.prev_tab(),
        crossterm::event::KeyCode::Char('N') => app.next_tab(),
        crossterm::event::KeyCode::Char('B') => app.toggle_bookmark(),
        crossterm::event::KeyCode::Char('R') => {
            let url = app
                .active_tab()
                .map(|t| t.url.to_string())
                .unwrap_or_default();
            if !url.is_empty() {
                app.navigate_to(url);
            }
        }
        crossterm::event::KeyCode::Char('x') => app.close_tab(),
        crossterm::event::KeyCode::Char('\\') => app.toggle_layout(),
        crossterm::event::KeyCode::Char('f') => app.toggle_focus_frame(),
        crossterm::event::KeyCode::Char('?') => {
            app.mode = Mode::Help;
            app.help_scroll = 0;
        }
        crossterm::event::KeyCode::Char(c @ '1'..='9') => {
            let num = (c as u8 - b'0') as usize;
            app.follow_link(num);
        }
        _ => {}
    }
}

fn handle_link(app: &mut App, key: crossterm::event::KeyEvent) {
    match key.code {
        crossterm::event::KeyCode::Char('j') | crossterm::event::KeyCode::Down => app.search_down(),
        crossterm::event::KeyCode::Char('k') | crossterm::event::KeyCode::Up => app.search_up(),
        crossterm::event::KeyCode::Char('g') => { app.search_selected = 0; app.search_scroll = 0; }
        crossterm::event::KeyCode::Char('G') => {
            if let Some(tab) = app.active_tab() {
                app.search_selected = tab.search_cards.len().saturating_sub(1);
                app.search_scroll = usize::MAX;
            }
        }
        crossterm::event::KeyCode::Enter => {
            if let Some(url) = app.search_follow() {
                app.new_tab(url);
            }
        }
        crossterm::event::KeyCode::Esc => app.mode = Mode::Normal,
        crossterm::event::KeyCode::Char('v') => { app.cursor_col = 0; app.view_sel_start = None; app.view_sel_end = None; app.mode = Mode::View; }
        crossterm::event::KeyCode::Char(':') => {
            app.mode = Mode::InsertUrl;
            app.input_buf = app.active_tab().map(|t| t.url.to_string()).unwrap_or_default();
        }
        crossterm::event::KeyCode::Char('/') => { app.mode = Mode::InsertSearch; app.input_buf.clear(); }
        crossterm::event::KeyCode::Char('q') => app.handle_quit(),
        crossterm::event::KeyCode::Char('?') => { app.mode = Mode::Help; app.help_scroll = 0; }
        crossterm::event::KeyCode::Char('\\') => app.toggle_layout(),
        crossterm::event::KeyCode::Char('f') => app.toggle_focus_frame(),
        crossterm::event::KeyCode::Char('R') => {
            let url = app.active_tab().map(|t| t.url.to_string()).unwrap_or_default();
            if !url.is_empty() { app.navigate_to(url); }
        }
        crossterm::event::KeyCode::Char('t') => { app.mode = Mode::InsertUrl; app.input_buf.clear(); app.message = "New tab URL: ".into(); }
        crossterm::event::KeyCode::Char('T') => app.prev_tab(),
        crossterm::event::KeyCode::Char('N') => app.next_tab(),
        crossterm::event::KeyCode::Char('x') => app.close_tab(),
        crossterm::event::KeyCode::Char('H') => app.go_back(),
        crossterm::event::KeyCode::Char('L') => app.go_forward(),
        crossterm::event::KeyCode::Left => app.go_back(),
        crossterm::event::KeyCode::Right => app.go_forward(),
        crossterm::event::KeyCode::Char(c @ '1'..='9') => app.follow_link((c as u8 - b'0') as usize),
        _ => {}
    }
}

fn clamp_cursor_col(app: &mut App) {
    let len = app
        .active_tab()
        .and_then(|t| t.lines.get(app.cursor))
        .map(|l| crate::app::line_text(l).chars().count())
        .unwrap_or(0);
    app.cursor_col = app.cursor_col.min(len.saturating_sub(1));
}

fn handle_view(app: &mut App, key: crossterm::event::KeyEvent) {
    match key.code {
        crossterm::event::KeyCode::Char('j') | crossterm::event::KeyCode::Down => {
            let len = app.active_tab().map_or(0, |t| t.lines.len());
            let cur = app.cursor;
            if cur + 1 < len {
                app.cursor += 1;
                let new_cur = app.cursor;
                let scroll = app.active_tab().map_or(0, |t| t.scroll);
                if new_cur >= scroll + 15 {
                    if let Some(t) = app.active_tab_mut() { t.scroll = new_cur.saturating_sub(5); }
                } else if new_cur < scroll {
                    if let Some(t) = app.active_tab_mut() { t.scroll = new_cur; }
                }
                clamp_cursor_col(app);
            }
        }
        crossterm::event::KeyCode::Char('k') | crossterm::event::KeyCode::Up => {
            if app.cursor > 0 {
                app.cursor -= 1;
                let cur = app.cursor;
                if let Some(t) = app.active_tab_mut() {
                    if cur < t.scroll { t.scroll = cur; }
                }
                clamp_cursor_col(app);
            }
        }
        crossterm::event::KeyCode::Char('h') | crossterm::event::KeyCode::Left => {
            if app.cursor_col > 0 {
                app.cursor_col -= 1;
            }
        }
        crossterm::event::KeyCode::Char('l') | crossterm::event::KeyCode::Right => {
            let text = app.active_tab()
                .and_then(|t| t.lines.get(app.cursor))
                .map(|l| crate::app::line_text(l));
            if let Some(ref t) = text {
                let max = t.chars().count().saturating_sub(1);
                if app.cursor_col < max {
                    app.cursor_col += 1;
                }
            }
        }
        crossterm::event::KeyCode::Char('w') => {
            let text = app.active_tab()
                .and_then(|t| t.lines.get(app.cursor))
                .map(|l| crate::app::line_text(l));
            if let Some(ref t) = text {
                let chars: Vec<char> = t.chars().collect();
                if !chars.is_empty() && app.cursor_col < chars.len() {
                    let col = app.cursor_col;
                    // skip current word
                    let after_word = if !chars[col].is_whitespace() {
                        chars[col..].iter().position(|c| c.is_whitespace()).map_or(chars.len(), |i| col + i)
                    } else { col };
                    // find next word start
                    let next = chars[after_word..].iter().position(|c| !c.is_whitespace())
                        .map_or(chars.len(), |i| after_word + i);
                    app.cursor_col = next.min(chars.len().saturating_sub(1));
                }
            }
        }
        crossterm::event::KeyCode::Char('b') => {
            let text = app.active_tab()
                .and_then(|t| t.lines.get(app.cursor))
                .map(|l| crate::app::line_text(l));
            if let Some(ref t) = text {
                let chars: Vec<char> = t.chars().collect();
                if !chars.is_empty() && app.cursor_col > 0 {
                    let col = app.cursor_col.min(chars.len() - 1);
                    // skip current word backwards
                    let before_word = if !chars[col].is_whitespace() {
                        chars[..=col].iter().rposition(|c| c.is_whitespace()).map_or(0, |i| i + 1)
                    } else { col };
                    if before_word > 0 {
                        // find prev word start
                        let prev = chars[..before_word].iter().rposition(|c| !c.is_whitespace())
                            .unwrap_or(before_word);
                        let start = chars[..=prev].iter().rposition(|c| c.is_whitespace())
                            .map_or(0, |i| i + 1);
                        app.cursor_col = start;
                    } else {
                        app.cursor_col = 0;
                    }
                }
            }
        }
        crossterm::event::KeyCode::Char('e') => {
            let text = app.active_tab()
                .and_then(|t| t.lines.get(app.cursor))
                .map(|l| crate::app::line_text(l));
            if let Some(ref t) = text {
                let chars: Vec<char> = t.chars().collect();
                if !chars.is_empty() && app.cursor_col < chars.len() {
                    let col = app.cursor_col;
                    // if on whitespace, skip to next word end
                    let start = if chars[col].is_whitespace() {
                        chars[col..].iter().position(|c| !c.is_whitespace()).map_or(chars.len(), |i| col + i)
                    } else { col };
                    if start < chars.len() {
                        let end = chars[start..].iter().position(|c| c.is_whitespace())
                            .map_or(chars.len(), |i| start + i);
                        app.cursor_col = end.saturating_sub(1);
                    }
                }
            }
        }
        crossterm::event::KeyCode::Char('g') => {
            app.cursor = 0;
            app.cursor_col = 0;
            if let Some(t) = app.active_tab_mut() { t.scroll = 0; }
        }
        crossterm::event::KeyCode::Char('G') => {
            let max = app.active_tab().map_or(0, |t| t.lines.len().saturating_sub(1));
            app.cursor = max;
            app.cursor_col = 0;
            let cur = app.cursor;
            if let Some(t) = app.active_tab_mut() { t.scroll = cur.saturating_sub(10); }
        }
        crossterm::event::KeyCode::Char('d') => {
            let max = app.active_tab().map_or(0, |t| t.lines.len().saturating_sub(1));
            app.cursor = (app.cursor + 10).min(max);
            if let Some(t) = app.active_tab_mut() {
                t.scroll = (t.scroll + 10).min(max);
            }
            clamp_cursor_col(app);
        }
        crossterm::event::KeyCode::Char('u') => {
            app.cursor = app.cursor.saturating_sub(10);
            if let Some(t) = app.active_tab_mut() {
                t.scroll = t.scroll.saturating_sub(10);
            }
            clamp_cursor_col(app);
        }
        crossterm::event::KeyCode::Enter => {
            if app.view_sel_start.is_none() {
                app.view_sel_start = Some((app.cursor, app.cursor_col));
                app.message = " Sel start".into();
            } else if app.view_sel_end.is_none() {
                app.view_sel_end = Some((app.cursor, app.cursor_col));
                app.message = " Sel end".into();
            } else {
                app.view_sel_start = None;
                app.view_sel_end = None;
                app.message = " Sel cleared".into();
            }
        }
        crossterm::event::KeyCode::Char('y') => {
            if let (Some(start), Some(end)) = (app.view_sel_start, app.view_sel_end) {
                if let Some(tab) = app.active_tab() {
                    let text = crate::app::extract_text(&tab.lines, start, end);
                    if !text.is_empty() {
                        crate::app::copy_to_clipboard(&text);
                        let disp: String = text.chars().take(60).collect();
                        app.message = format!(" Yanked: {disp}");
                    }
                }
                app.view_sel_start = None;
                app.view_sel_end = None;
            }
        }
        crossterm::event::KeyCode::Esc => {
            app.view_sel_start = None;
            app.view_sel_end = None;
            app.mode = Mode::Normal;
        }
        crossterm::event::KeyCode::Char(':') => {
            app.mode = Mode::InsertUrl;
            app.input_buf = app.active_tab().map(|t| t.url.to_string()).unwrap_or_default();
        }
        crossterm::event::KeyCode::Char('/') => { app.mode = Mode::InsertSearch; app.input_buf.clear(); }
        crossterm::event::KeyCode::Char('q') => app.handle_quit(),
        crossterm::event::KeyCode::Char('?') => { app.mode = Mode::Help; app.help_scroll = 0; }
        crossterm::event::KeyCode::Char('\\') => app.toggle_layout(),
        crossterm::event::KeyCode::Char('f') => app.toggle_focus_frame(),
        crossterm::event::KeyCode::Char('R') => {
            let url = app.active_tab().map(|t| t.url.to_string()).unwrap_or_default();
            if !url.is_empty() { app.navigate_to(url); }
        }
        crossterm::event::KeyCode::Char('t') => { app.mode = Mode::InsertUrl; app.input_buf.clear(); app.message = "New tab URL: ".into(); }
        crossterm::event::KeyCode::Char('T') => app.prev_tab(),
        crossterm::event::KeyCode::Char('N') => app.next_tab(),
        crossterm::event::KeyCode::Char('x') => app.close_tab(),
        crossterm::event::KeyCode::Char('H') => app.go_back(),
        crossterm::event::KeyCode::Char('L') => app.go_forward(),
        crossterm::event::KeyCode::Char(c @ '1'..='9') => app.follow_link((c as u8 - b'0') as usize),
        _ => {}
    }
}

fn handle_tiling(app: &mut App, code: crossterm::event::KeyCode, shift: bool) -> bool {
    // Detect digit from unshifted digit or shifted symbol
    let (digit, auto_shift) = match code {
        crossterm::event::KeyCode::Char(c) if c.is_ascii_digit() => (Some(c as u8 - b'0'), shift),
        crossterm::event::KeyCode::Char('!') => (Some(1), true),
        crossterm::event::KeyCode::Char('@') => (Some(2), true),
        crossterm::event::KeyCode::Char('#') => (Some(3), true),
        crossterm::event::KeyCode::Char('$') => (Some(4), true),
        crossterm::event::KeyCode::Char('%') => (Some(5), true),
        crossterm::event::KeyCode::Char('^') => (Some(6), true),
        crossterm::event::KeyCode::Char('&') => (Some(7), true),
        crossterm::event::KeyCode::Char('*') => (Some(8), true),
        crossterm::event::KeyCode::Char('(') => (Some(9), true),
        _ => (None, false),
    };
    if let Some(n) = digit {
        if n >= 1 && n <= 9 {
            if auto_shift {
                app.move_tab_to_slot(n as usize);
            } else {
                app.switch_slot(n as usize);
            }
        }
        return true;
    }
    match code {
        crossterm::event::KeyCode::Char(c @ ('h' | 'j' | 'k' | 'l'))
        | crossterm::event::KeyCode::Char(c @ ('a' | 's' | 'w' | 'd')) => {
            if shift {
                app.swap_tab_dir(c);
            } else {
                app.focus_tile(c);
            }
            true
        }
        crossterm::event::KeyCode::Char('f') => {
            app.toggle_fullscreen();
            true
        }
        crossterm::event::KeyCode::Char('v') => {
            app.set_tiling(TilingMode::Vertical);
            true
        }
        crossterm::event::KeyCode::Char('b') => {
            app.set_tiling(TilingMode::Horizontal);
            true
        }
        crossterm::event::KeyCode::Char('q') => {
            app.handle_quit();
            true
        }
        crossterm::event::KeyCode::Enter => {
            app.mode = Mode::InsertUrl;
            app.input_buf.clear();
            app.message = "New tab URL: ".into();
            true
        }
        _ => false,
    }
}

fn handle_insert(app: &mut App, key: crossterm::event::KeyEvent, is_search: bool) {
    let is_home = app.active_tab().map_or(false, |t| t.url.as_str() == "trawl:home");
    match key.code {
        crossterm::event::KeyCode::Enter => {
            let input = app.input_buf.trim().to_string();
            if !input.is_empty() {
                if is_search {
                    app.search(&input);
                } else {
                    app.navigate_to(input);
                    app.mode = Mode::Normal;
                }
            } else if !is_home {
                app.mode = Mode::Normal;
            }
            app.message.clear();
        }
        crossterm::event::KeyCode::Esc => {
            if is_home {
                app.input_buf.clear();
            } else {
                app.mode = Mode::Normal;
                app.message.clear();
            }
        }
        crossterm::event::KeyCode::Char(c) if c != '\x7f' => app.input_buf.push(c),
        crossterm::event::KeyCode::Backspace | crossterm::event::KeyCode::Char('\x7f') => {
            app.input_buf.pop();
        }
        _ => {}
    }
}

fn handle_help(app: &mut App, key: crossterm::event::KeyEvent) {
    match key.code {
        crossterm::event::KeyCode::Char('q') | crossterm::event::KeyCode::Esc => {
            app.mode = Mode::Normal;
        }
        crossterm::event::KeyCode::Char('j') | crossterm::event::KeyCode::Down => {
            app.help_scroll = app.help_scroll.saturating_add(1);
        }
        crossterm::event::KeyCode::Char('k') | crossterm::event::KeyCode::Up => {
            app.help_scroll = app.help_scroll.saturating_sub(1);
        }
        _ => {}
    }
}
