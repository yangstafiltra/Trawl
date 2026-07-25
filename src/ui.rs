use crate::app::{App, FocusPanel, Mode, SidebarTab, TilingMode};
use crate::browser;
use crate::config::LayoutStyle;
use ratatui::prelude::*;
use ratatui_image::StatefulImage;
use ratatui::widgets::{
    Block, BorderType, Borders, List, ListItem, Paragraph,
};

fn highlight_char(line: &Line<'static>, col: usize) -> Line<'static> {
    let text: String = line.spans.iter().map(|s| s.content.as_ref()).collect();
    let chars: Vec<char> = text.chars().collect();
    if chars.is_empty() || col >= chars.len() {
        return line.clone();
    }
    let before: String = chars[..col].iter().collect();
    let at: String = chars[col].to_string();
    let after: String = chars[col + 1..].iter().collect();
    Line::from(vec![
        Span::raw(before),
        Span::styled(at, Style::default().bg(Color::Rgb(50, 50, 70))),
        Span::raw(after),
    ])
}

fn focus_style(focused: bool) -> Style {
    if focused {
        Style::default()
            .fg(Color::White)
            .add_modifier(Modifier::BOLD)
    } else {
        Style::default().fg(Color::Rgb(60, 60, 60))
    }
}

fn render_bordered_panel<'a>(
    frame: &mut Frame,
    area: Rect,
    focused: bool,
    title: Option<&'a str>,
) -> Rect {
    if area.width < 3 || area.height < 3 {
        return area;
    }
    let mut block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(focus_style(focused));
    if let Some(t) = title {
        block = block.title(t);
    }
    let inner = block.inner(area);
    frame.render_widget(block, area);
    inner
}

pub fn render(app: &mut App, frame: &mut Frame) {
    match app.config.layout {
        LayoutStyle::BrowserChrome => render_chrome(app, frame),
        LayoutStyle::LazyGit => render_lazygit(app, frame),
    }
}

// ─── Layout 3: Browser-Chrome ──────────────────────────────────────

fn render_chrome(app: &mut App, frame: &mut Frame) {
    let area = frame.area();
    let is_insert = matches!(app.mode, Mode::InsertUrl)
        || (matches!(app.mode, Mode::InsertSearch) && !is_home_page(app));
    let chunks = Layout::vertical(vec![
        Constraint::Length(1),
        Constraint::Length(1),
        Constraint::Min(1),
        Constraint::Length(1),
    ])
    .split(area);

    render_chrome_address(app, frame, chunks[0], is_insert);
    render_tab_bar(app, frame, chunks[1]);
    if matches!(app.mode, Mode::Help) {
        render_help_screen(app, frame, chunks[2]);
    } else {
        render_tiled_content(app, frame, chunks[2]);
    }
    render_chrome_status(app, frame, chunks[3]);
}

fn render_chrome_address(app: &App, frame: &mut Frame, area: Rect, editable: bool) {
    let (url, cursor) = if editable && matches!(app.mode, Mode::InsertUrl) {
        (
            app.input_buf.clone(),
            if app.input_buf.is_empty() {
                "\u{2588}"
            } else {
                ""
            },
        )
    } else if editable && matches!(app.mode, Mode::InsertSearch) {
        (
            format!("{} {}", app.config.search_engine, app.input_buf),
            if app.input_buf.is_empty() {
                "\u{2588}"
            } else {
                ""
            },
        )
    } else {
        let u = app
            .active_tab()
            .map(|t| t.url.to_string())
            .unwrap_or_default();
        (u, "")
    };

    let prefix = match app.mode {
        Mode::InsertUrl => " \u{1F310} ",
        Mode::InsertSearch => " \u{1F50D} ",
        _ => " \u{1F310} ",
    };

    let line = Line::from(vec![
        Span::styled(prefix, Style::default().fg(Color::LightBlue)),
        Span::raw(url),
        Span::styled(cursor, Style::default().fg(Color::White)),
    ]);
    let bg = if editable {
        Color::Rgb(28, 28, 40)
    } else {
        Color::Rgb(16, 16, 24)
    };
    frame.render_widget(Paragraph::new(line).style(Style::default().bg(bg)), area);
}

fn render_content_panel(app: &App, frame: &mut Frame, area: Rect, focused: bool) -> Rect {
    if app.focus_frame_visible {
        render_bordered_panel(frame, area, focused, None)
    } else {
        Rect {
            x: area.x + 1,
            y: area.y,
            width: area.width.saturating_sub(2),
            height: area.height,
        }
    }
}

fn render_content_full(app: &mut App, frame: &mut Frame, area: Rect) {
    let inner = render_content_panel(app, frame, area, true);
    render_content_inner(app, frame, inner);
}

fn render_tiled_content(app: &mut App, frame: &mut Frame, area: Rect) {
    let slot = app.active_slot;
    let slot_tabs: Vec<usize> = (0..app.tabs.len()).filter(|&i| app.tabs[i].slot == slot).collect();
    let n = slot_tabs.len();
    if n <= 1 {
        return render_content_full(app, frame, area);
    }
    let tile_rects = calc_tile_rects(area, app.tiling_mode, n);
    for (pos, &tab_idx) in slot_tabs.iter().enumerate() {
        let Some(&rect) = tile_rects.get(pos) else { break };
        let is_focused = tab_idx == app.active_tab;
        let border_style = if is_focused {
            Style::default().fg(Color::LightBlue).add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(Color::Rgb(40, 40, 50))
        };
        let block = Block::default()
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .border_style(border_style);
        let inner = block.inner(rect);
        frame.render_widget(block, rect);
        if inner.width < 3 || inner.height < 1 {
            continue;
        }
        let pad_inner = Rect {
            x: inner.x + 1,
            y: inner.y,
            width: inner.width.saturating_sub(2),
            height: inner.height,
        };
        if is_focused {
            let inner2 = render_content_panel(app, frame, pad_inner, true);
            render_content_inner(app, frame, inner2);
        } else {
            render_tile_text(app, frame, pad_inner, tab_idx);
        }
    }
}

fn calc_tile_rects(area: Rect, mode: TilingMode, n: usize) -> Vec<Rect> {
    match (mode, n) {
        (TilingMode::Single, _) | (_, 1) => vec![area],
        (TilingMode::Auto, 2) | (TilingMode::Vertical, 2) => {
            let mw = area.width / 2;
            vec![
                Rect { x: area.x, y: area.y, width: mw, height: area.height },
                Rect { x: area.x + mw, y: area.y, width: area.width - mw, height: area.height },
            ]
        }
        (TilingMode::Auto, 3) | (TilingMode::Master, _) => {
            let mw = area.width * 50 / 100;
            let rest_w = area.width - mw - 1;
            let sh = area.height / (n - 1) as u16;
            let mut rects = vec![Rect { x: area.x, y: area.y, width: mw, height: area.height }];
            for i in 0..n - 1 {
                rects.push(Rect {
                    x: area.x + mw + 1,
                    y: area.y + i as u16 * sh,
                    width: rest_w,
                    height: if i == n - 2 { area.height - i as u16 * sh } else { sh },
                });
            }
            rects
        }
        (TilingMode::Auto, n) | (TilingMode::Vertical, n) if n >= 2 && n <= 6 => {
            let cols = if n <= 2 { n } else { 2 };
            let rows = (n + cols - 1) / cols;
            let cw = area.width / cols as u16;
            let rh = area.height / rows as u16;
            (0..n)
                .map(|i| {
                    let col = i % cols;
                    let row = i / cols;
                    Rect {
                        x: area.x + col as u16 * cw,
                        y: area.y + row as u16 * rh,
                        width: if col == cols - 1 { area.width - col as u16 * cw } else { cw },
                        height: if row == rows - 1 { area.height - row as u16 * rh } else { rh },
                    }
                })
                .collect()
        }
        (TilingMode::Auto, _) | (TilingMode::Vertical, _) => {
            let cw = area.width / 2;
            let sh = area.height / ((n + 1) / 2) as u16;
            (0..n)
                .map(|i| {
                    let col = i % 2;
                    let row = i / 2;
                    Rect {
                        x: area.x + col as u16 * cw,
                        y: area.y + row as u16 * sh,
                        width: if col == 1 { area.width - cw } else { cw },
                        height: if row == (n - 1) / 2 { area.height - row as u16 * sh } else { sh },
                    }
                })
                .collect()
        }
        (TilingMode::Horizontal, _) => {
            let rh = area.height / n as u16;
            (0..n)
                .map(|i| Rect {
                    x: area.x,
                    y: area.y + i as u16 * rh,
                    width: area.width,
                    height: if i == n - 1 { area.height - i as u16 * rh } else { rh },
                })
                .collect()
        }
    }
}

fn render_tile_text(app: &App, frame: &mut Frame, area: Rect, tab_idx: usize) {
    let Some(tab) = app.tabs.get(tab_idx) else { return };
    let h = area.height as usize;
    let text_lines = browser::render_content_lines(&tab.lines, tab.scroll, h);
    for (i, line) in text_lines.iter().enumerate() {
        let y = area.y + i as u16;
        if y >= area.y + area.height {
            break;
        }
        frame.render_widget(
            Paragraph::new((*line).clone()),
            Rect { x: area.x, y, width: area.width, height: 1 },
        );
    }
}

fn render_content_inner(app: &mut App, frame: &mut Frame, inner: Rect) {
    if inner.width < 2 || inner.height == 0 {
        return;
    }
    if is_home_page(app) {
        return render_home_page(app, frame, inner);
    }
    if app.is_video_page() {
        return render_video_content(app, frame, inner);
    }
    if matches!(app.mode, Mode::Link) && app.has_search_cards() {
        return render_search_cards(app, frame, inner);
    }

    let scroll: usize;
    let is_view = matches!(app.mode, Mode::View);
    let cursor = app.cursor;
    let text_lines: Vec<Line<'static>>;
    {
        let Some(tab) = app.active_tab() else { return };
        let h = inner.height as usize;
        scroll = tab.scroll;
        let ref_lines = browser::render_content_lines(&tab.lines, scroll, h);
        text_lines = ref_lines.iter().map(|l| (*l).clone()).collect();
    }

    let cursor_col = app.cursor_col;
    let spacing = 2u16;
    let mut vis_i = 0usize;
    let mut y = inner.y;

    if let Some(tab) = app.active_tab_mut() {
        let mut sorted: Vec<usize> = (0..tab.image_protocols.len()).collect();
        sorted.sort_by_key(|&i| tab.image_protocols[i].0);
        let mut img_idx = 0usize;

        while y < inner.y + inner.height && vis_i < text_lines.len() {
            let content_line = scroll + vis_i;

            if img_idx < sorted.len() {
                let (line_idx, protocol, cell_w, cell_h) = &mut tab.image_protocols[sorted[img_idx]];
                if *line_idx == content_line {
                    let cw = *cell_w;
                    let ch = *cell_h;
                    let img_h = ch.min(inner.y + inner.height - y);
                    let img_w = cw.min(inner.width);
                    let area = Rect { x: inner.x, y, width: img_w, height: img_h };
                    frame.render_stateful_widget(StatefulImage::default(), area, protocol);
                    y += img_h + spacing;
                    vis_i += 1;
                    img_idx += 1;
                    continue;
                }
            }

            let line = text_lines[vis_i].clone();
            let rendered = if is_view && content_line == cursor {
                highlight_char(&line, cursor_col)
            } else {
                line
            };
            frame.render_widget(
                Paragraph::new(rendered),
                Rect { x: inner.x, y, width: inner.width, height: 1 },
            );
            y += 1;
            vis_i += 1;
        }
    } else {
        for (i, line) in text_lines.iter().enumerate() {
            let line_y = inner.y + i as u16;
            if line_y >= inner.y + inner.height { break; }
            let line = (*line).clone();
            let rendered = if is_view && (scroll + i) == cursor {
                highlight_char(&line, cursor_col)
            } else {
                line
            };
            frame.render_widget(
                Paragraph::new(rendered),
                Rect { x: inner.x, y: line_y, width: inner.width, height: 1 },
            );
        }
    }
}

fn render_search_cards(app: &mut App, frame: &mut Frame, area: Rect) {
    let (heights, y_offsets, sel, new_scroll) = {
        let Some(tab) = app.active_tab() else { return };
        if tab.search_cards.is_empty() { return; }
        let spacing = 1u16;
        let inner_w = area.width.saturating_sub(4);
        let heights: Vec<u16> = tab.search_cards.iter().map(|c| {
            let mut n = 2u16;
            if !c.snippet.is_empty() {
                let w = inner_w.max(1) as usize;
                n += ((c.snippet.chars().count() + w - 1) / w).min(2) as u16;
            }
            n + 3
        }).collect();
        let total = heights.len();
        let mut cy = spacing;
        let mut y_offsets = vec![0u16; total];
        for (i, &h) in heights.iter().enumerate() {
            y_offsets[i] = cy;
            cy += h + spacing;
        }
        let content_h = cy;
        let sel = app.search_selected.min(total - 1);
        let view_h = area.height;
        let max_scroll = if content_h > view_h { content_h - view_h } else { 0 };
        let sel_top = y_offsets[sel];
        let sel_bot = sel_top + heights[sel];
        let scroll_pos = (app.search_scroll as u16).min(max_scroll);
        let new_scroll = if sel_bot > scroll_pos + view_h {
            sel_bot.saturating_sub(view_h)
        } else if sel_top < scroll_pos {
            sel_top
        } else {
            scroll_pos
        };
        (heights, y_offsets, sel, new_scroll)
    };

    app.search_scroll = new_scroll as usize;

    let view_h = area.height;
    let area_top = 0i32;
    let area_bot = view_h as i32;
    let Some(tab) = app.active_tab() else { return };

    for (i, card) in tab.search_cards.iter().enumerate() {
        let y_rel = y_offsets[i] as i32 - new_scroll as i32;
        if y_rel + heights[i] as i32 <= area_top || y_rel >= area_bot { continue; }

        let y = (area.y as i32 + y_rel).max(area.y as i32) as u16;
        let ch = heights[i];
        let focused = i == sel;
        let border_style = if focused {
            Style::default().fg(Color::LightBlue).add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(Color::Rgb(50, 50, 60))
        };
        let block = Block::default().borders(Borders::ALL).border_type(BorderType::Rounded).border_style(border_style);
        let card_area = Rect { x: area.x + 1, y, width: area.width.saturating_sub(2), height: ch.min((area_bot - y_rel) as u16) };
        let inner = block.inner(card_area);
        frame.render_widget(block, card_area);

        let title_style = if focused { Style::default().fg(Color::White) } else { Style::default().fg(Color::Gray) };
        let mut lines = vec![Line::from(Span::styled(format!("{}  {}", card.number, &card.title), title_style))];
        if !card.snippet.is_empty() {
            lines.push(Line::from(Span::styled(&card.snippet, Style::default().fg(Color::Rgb(120, 120, 120)))));
        }
        let url_style = if focused { Style::default().fg(Color::Rgb(160, 160, 160)) } else { Style::default().fg(Color::DarkGray) };
        lines.push(Line::from(Span::styled(&card.url, url_style)));
        frame.render_widget(Paragraph::new(lines), inner);
    }
}

// ─── Video page: header + card grid ─────────────────────────────

fn render_video_content(app: &mut App, frame: &mut Frame, area: Rect) {
    let chunks = Layout::vertical([Constraint::Length(3), Constraint::Min(1)]).split(area);
    render_video_header(app, frame, chunks[0]);
    render_video_grid(app, frame, chunks[1]);
}

fn render_video_header(_app: &App, frame: &mut Frame, area: Rect) {
    let line = Line::from(Span::styled("Videos", Style::default().fg(Color::LightBlue).add_modifier(Modifier::BOLD)));
    frame.render_widget(Paragraph::new(line), area);
}

fn render_video_grid(app: &mut App, frame: &mut Frame, area: Rect) {
    if area.width < 6 || area.height < 6 {
        return;
    }
    let total_cards = app.active_tab().map_or(0, |t| t.video_cards.len());
    if total_cards == 0 {
        return;
    }

    let cols = 3usize;
    let vis_rows = 4usize;
    let card_h = (area.height / vis_rows as u16).max(7);
    app.card_h = card_h;
    let card_w = area.width / cols as u16;
    let scroll_row = app.card_scroll_row();

    for row_off in 0..vis_rows {
        let actual_row = scroll_row + row_off;
        let start_idx = actual_row * cols;
        if start_idx >= total_cards {
            break;
        }

        let row_y = area.y + (row_off as u16) * card_h;
        let row_h = card_h.min(area.height.saturating_sub(row_y - area.y));

        for col in 0..cols {
            let idx = start_idx + col;
            if idx >= total_cards {
                break;
            }
            let card_area = Rect {
                x: area.x + (col as u16) * card_w,
                y: row_y,
                width: card_w,
                height: row_h,
            };
            let is_selected = idx == app.card_selected();
            if let Some(tab) = app.active_tab_mut() {
                if let Some(card) = tab.video_cards.get_mut(idx) {
                    render_video_card(frame, card_area, card, is_selected);
                }
            }
        }
    }
}

fn render_video_card(frame: &mut Frame, area: Rect, card: &mut crate::browser::VideoCard, is_selected: bool) {
    let border_style = if is_selected {
        Style::default()
            .fg(Color::LightBlue)
            .add_modifier(Modifier::BOLD)
    } else {
        Style::default().fg(Color::Rgb(50, 50, 60))
    };

    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(border_style);

    let inner = block.inner(area);
    frame.render_widget(block, area);

    let chunks = Layout::vertical([Constraint::Min(4), Constraint::Length(2)]).split(inner);

    // Cover image or placeholder
    if let Some(ref mut cover) = card.cover {
        let image = StatefulImage::default();
        frame.render_stateful_widget(image, chunks[0], cover);
    } else {
        let thumb_text = if is_selected { " ▶ " } else { " 📺 " };
        frame.render_widget(
            Paragraph::new(thumb_text)
                .style(Style::default().fg(if is_selected {
                    Color::LightBlue
                } else {
                    Color::DarkGray
                }))
                .alignment(Alignment::Center),
            chunks[0],
        );
    }

    // Info: title, author, views
    let info_style = if is_selected {
        Style::default().fg(Color::White)
    } else {
        Style::default().fg(Color::Gray)
    };
    let title: String = if card.title.chars().count() > inner.width as usize - 2 {
        card.title
            .chars()
            .take((inner.width as usize).saturating_sub(4))
            .collect::<String>()
            + ".."
    } else {
        card.title.clone()
    };
    let meta = if card.author.is_empty() && card.views.is_empty() {
        format!("#{}", card.number)
    } else {
        format!("{} · {}", card.author, card.views)
    };
    let info = Text::from(vec![
        Line::from(Span::styled(title, info_style)),
        Line::from(Span::styled(meta, Style::default().fg(Color::DarkGray))),
        Line::from(Span::styled(
            &card.duration,
            Style::default().fg(Color::Green),
        )),
    ]);
    frame.render_widget(Paragraph::new(info), chunks[1]);
}

fn render_chrome_status(app: &App, frame: &mut Frame, area: Rect) {
    let mode_str = match app.mode {
        Mode::Normal => " NORM ",
        Mode::Link => " LINK ",
        Mode::View => " VIEW ",
        Mode::InsertUrl | Mode::InsertSearch => " EDIT ",
        Mode::Help => " HELP ",
    };
    let color = match app.mode {
        Mode::Normal => Color::LightGreen,
        Mode::Link => Color::LightBlue,
        Mode::View => Color::LightYellow,
        Mode::InsertUrl | Mode::InsertSearch => Color::LightYellow,
        Mode::Help => Color::LightCyan,
    };
    let n = app.active_tab().map(|t| t.links.len()).unwrap_or(0);
    let pct = app
        .active_tab()
        .map(|t| {
            let total = t.lines.len().max(1);
            ((t.scroll as f64 / total as f64 * 100.0) as usize).min(100)
        })
        .unwrap_or(0);

    let left = vec![
        Span::styled(
            mode_str,
            Style::default()
                .fg(Color::Black)
                .bg(color)
                .add_modifier(Modifier::BOLD),
        ),
        Span::raw(" "),
        Span::styled(
            format!("\u{1F4CE}{n}"),
            Style::default().fg(Color::DarkGray),
        ),
    ];
    let tile_mode = match app.tiling_mode {
        TilingMode::Auto => "A",
        TilingMode::Vertical => "V",
        TilingMode::Horizontal => "H",
        TilingMode::Master => "M",
        TilingMode::Single => "S",
    };
    let right = format!(" S{}/{} {}  {}%  \\=layout ?help ", app.active_slot, tile_mode, app.config.search_engine, pct);
    let lw: u16 = left.iter().map(|s| s.content.len() as u16).sum();
    let rw = right.len() as u16;
    let pad = area.width.saturating_sub(lw + rw + 2);

    let mut all = left;
    all.push(Span::raw(" ".repeat(pad as usize)));
    all.push(Span::styled(right, Style::default().fg(Color::DarkGray)));
    frame.render_widget(
        Paragraph::new(Line::from(all)).style(Style::default().bg(Color::Rgb(20, 20, 20))),
        area,
    );
}

// ─── Layout 4: lazygit-Panels ──────────────────────────────────────

fn render_lazygit(app: &mut App, frame: &mut Frame) {
    let area = frame.area();
    let is_insert = matches!(app.mode, Mode::InsertUrl)
        || (matches!(app.mode, Mode::InsertSearch) && !is_home_page(app));

    let rows = if is_insert {
        vec![
            Constraint::Length(1),
            Constraint::Min(1),
            Constraint::Length(1),
            Constraint::Length(1),
        ]
    } else {
        vec![
            Constraint::Length(1),
            Constraint::Min(1),
            Constraint::Length(1),
        ]
    };
    let chunks = Layout::vertical(rows).split(area);

    render_tab_bar(app, frame, chunks[0]);
    if matches!(app.mode, Mode::Help) {
        render_help_screen(app, frame, chunks[1]);
    } else {
        render_lazygit_main(app, frame, chunks[1]);
    }
    if is_insert {
        render_insert_bar(app, frame, chunks[2]);
        render_lazygit_status(app, frame, chunks[3]);
    } else {
        render_lazygit_status(app, frame, chunks[2]);
    }
}

fn render_lazygit_main(app: &mut App, frame: &mut Frame, area: Rect) {
    let slot = app.active_slot;
    let in_slot = app.tabs.iter().filter(|t| t.slot == slot).count();
    if in_slot > 1 {
        return render_tiled_content(app, frame, area);
    }
    if app.is_video_page() || is_home_page(app) {
        let inner = render_content_panel(app, frame, area, true);
        if is_home_page(app) {
            render_home_page(app, frame, inner);
        } else {
            render_content_inner(app, frame, inner);
        }
    } else {
        let cols = Layout::horizontal([
            Constraint::Length(28),
            Constraint::Min(1),
        ])
        .split(area);
        let inner = if app.focus_frame_visible {
            render_bordered_panel(frame, cols[0], app.focus_panel == FocusPanel::Left, Some(" Links "))
        } else {
            Rect {
                x: cols[0].x + 1,
                y: cols[0].y,
                width: cols[0].width.saturating_sub(2),
                height: cols[0].height,
            }
        };
        render_lazygit_left_sidebar(app, frame, inner);
        let inner = render_content_panel(app, frame, cols[1], app.focus_panel == FocusPanel::Center);
        render_content_inner(app, frame, inner);
    }
}

fn render_lazygit_left_sidebar(app: &App, frame: &mut Frame, area: Rect) {
    let chunks = Layout::vertical([Constraint::Length(1), Constraint::Min(1)]).split(area);
    let labels = [" Links ", " Hist ", " Bookm "];
    let sel = match app.sidebar_tab {
        SidebarTab::Links => 0,
        SidebarTab::History => 1,
        SidebarTab::Bookmarks => 2,
    };
    let tabs: Vec<Span> = labels
        .iter()
        .enumerate()
        .map(|(i, l)| {
            if i == sel {
                Span::styled(
                    *l,
                    Style::default()
                        .fg(Color::Black)
                        .bg(Color::LightBlue)
                        .add_modifier(Modifier::BOLD),
                )
            } else {
                Span::styled(*l, Style::default().fg(Color::DarkGray))
            }
        })
        .collect();
    frame.render_widget(Paragraph::new(Line::from(tabs)), chunks[0]);

    let focused = app.config.layout == crate::config::LayoutStyle::LazyGit
        && app.focus_panel == crate::app::FocusPanel::Left;
    let items: Vec<ListItem> = match app.sidebar_tab {
        SidebarTab::Links => app
            .active_tab()
            .map(|tab| {
                tab.links
                    .iter()
                    .enumerate()
                    .map(|(i, link)| {
                        let is_sel = focused && i == app.sidebar_idx;
                        let chars: Vec<char> = link.text.chars().collect();
                        let t = if chars.len() > 22 {
                            format!("{}..", chars[..20].iter().collect::<String>())
                        } else {
                            link.text.clone()
                        };
                        let style = if is_sel {
                            Style::default().bg(Color::Rgb(50, 50, 65))
                        } else {
                            Style::default()
                        };
                        ListItem::new(Line::from(vec![
                            Span::styled(
                                format!("{:>2} ", link.number),
                                Style::default()
                                    .fg(Color::LightYellow)
                                    .add_modifier(Modifier::BOLD),
                            ),
                            Span::styled(t, Style::default().fg(Color::LightCyan)),
                        ]))
                        .style(style)
                    })
                    .collect()
            })
            .unwrap_or_default(),
        SidebarTab::History => app
            .history
            .iter()
            .rev()
            .take(50)
            .enumerate()
            .map(|(i, u)| {
                let is_sel = focused && i == app.sidebar_idx;
                let chars: Vec<char> = u.chars().collect();
                let s = if chars.len() > 24 {
                    format!("{}..", chars[..22].iter().collect::<String>())
                } else {
                    u.clone()
                };
                let style = if is_sel {
                    Style::default().bg(Color::Rgb(50, 50, 65))
                } else {
                    Style::default()
                };
                ListItem::new(Line::from(Span::raw(s))).style(style)
            })
            .collect(),
        SidebarTab::Bookmarks => app
            .bookmarks
            .iter()
            .enumerate()
            .map(|(i, bm)| {
                let is_sel = focused && i == app.sidebar_idx;
                let style = if is_sel {
                    Style::default().bg(Color::Rgb(50, 50, 65))
                } else {
                    Style::default()
                };
                ListItem::new(Line::from(vec![
                    Span::styled(" \u{1F516} ", Style::default().fg(Color::Yellow)),
                    Span::raw(&bm.title),
                ]))
                .style(style)
            })
            .collect(),
    };
    let list_state = if focused {
        Some(app.sidebar_idx)
    } else {
        None
    };
    let mut state = ratatui::widgets::ListState::default().with_selected(list_state);
    frame.render_stateful_widget(List::new(items), chunks[1], &mut state);
}

fn render_lazygit_status(app: &App, frame: &mut Frame, area: Rect) {
    let mode_str = match app.mode {
        Mode::Normal => " NORM ",
        Mode::Link => " LINK ",
        Mode::View => " VIEW ",
        Mode::InsertUrl | Mode::InsertSearch => " EDIT ",
        Mode::Help => " HELP ",
    };
    let color = match app.mode {
        Mode::Normal => Color::LightGreen,
        Mode::Link => Color::LightBlue,
        Mode::View => Color::LightYellow,
        Mode::InsertUrl | Mode::InsertSearch => Color::LightYellow,
        Mode::Help => Color::LightCyan,
    };
    let tab = app.active_tab();
    let url = tab.map(|t| t.url.to_string()).unwrap_or_default();
    let n = tab.map(|t| t.links.len()).unwrap_or(0);
    let pct = tab
        .map(|t| {
            let total = t.lines.len().max(1);
            ((t.scroll as f64 / total as f64 * 100.0) as usize).min(100)
        })
        .unwrap_or(0);

    let chars: Vec<char> = url.chars().collect();
    let url_disp = if chars.len() > 40 {
        format!("{}..", chars[..38].iter().collect::<String>())
    } else {
        url
    };

    let left = vec![
        Span::styled(
            mode_str,
            Style::default()
                .fg(Color::Black)
                .bg(color)
                .add_modifier(Modifier::BOLD),
        ),
        Span::raw(" "),
        Span::styled(url_disp, Style::default().fg(Color::Cyan)),
    ];
    let mut right_parts: Vec<String> = vec![];
    if !app.message.is_empty() {
        right_parts.push(app.message.clone());
    }
    let tile_mode = match app.tiling_mode {
        TilingMode::Auto => "A",
        TilingMode::Vertical => "V",
        TilingMode::Horizontal => "H",
        TilingMode::Master => "M",
        TilingMode::Single => "S",
    };
    right_parts.push(format!(" S{}/{} \u{1F4CE}{n}  {pct}%  \\=layout ?help", app.active_slot, tile_mode));
    let right = right_parts.join(" ");
    let lw: u16 = left.iter().map(|s| s.content.len() as u16).sum();
    let rw = right.len() as u16;
    let pad = area.width.saturating_sub(lw + rw + 2);
    let mut all = left;
    all.push(Span::raw(" ".repeat(pad as usize)));
    all.push(Span::styled(right, Style::default().fg(Color::DarkGray)));
    frame.render_widget(
        Paragraph::new(Line::from(all)).style(Style::default().bg(Color::Rgb(20, 20, 20))),
        area,
    );
}

// ─── Shared: Tab bar ───────────────────────────────────────────────

fn render_tab_bar(app: &App, frame: &mut Frame, area: Rect) {
    let mut spans: Vec<Span> = vec![];
    let mut slot_order: Vec<usize> = app.tabs.iter().map(|t| t.slot).collect();
    slot_order.sort();
    slot_order.dedup();
    for &slot in &slot_order {
        let is_active_slot = slot == app.active_slot;
        let dot = if is_active_slot { "\u{25CF}" } else { "\u{25CB}" };
        let header = format!("[{}]{} ", slot, dot);
        let header_style = if is_active_slot {
            Style::default().fg(Color::White)
        } else {
            Style::default().fg(Color::Rgb(100, 100, 110))
        };
        spans.push(Span::styled(header, header_style));
        let mut first = true;
        for (i, tab) in app.tabs.iter().enumerate() {
            if tab.slot != slot { continue; }
            let is_focused = i == app.active_tab;
            let chars: Vec<char> = tab.title.chars().collect();
            let label = if chars.len() > 8 {
                format!("{}.", chars[..7].iter().collect::<String>())
            } else {
                tab.title.clone()
            };
            let loading = if tab.loading { "\u{2026}" } else { "" };
            if !first {
                spans.push(Span::styled(" | ", Style::default().fg(Color::Rgb(60, 60, 70))));
            }
            first = false;
            let style = if is_focused {
                Style::default().fg(Color::White).bg(Color::Rgb(30, 30, 40))
            } else if is_active_slot {
                Style::default().fg(Color::Rgb(140, 140, 160))
            } else {
                Style::default().fg(Color::Rgb(60, 60, 70))
            };
            spans.push(Span::styled(format!("{}{}", label, loading), style));
        }
        spans.push(Span::raw("  "));
    }
    frame.render_widget(Paragraph::new(Line::from(spans)), area);
}

// ─── Shared: Insert command bar ────────────────────────────────────

fn render_insert_bar(app: &App, frame: &mut Frame, area: Rect) {
    let (prefix, buf) = match app.mode {
        Mode::InsertUrl => (" URL: ", &app.input_buf),
        Mode::InsertSearch => (" / ", &app.input_buf),
        _ => return,
    };
    let cursor = if buf.is_empty() { "\u{2588}" } else { "" };
    let line = Line::from(vec![
        Span::styled(prefix, Style::default().fg(Color::LightGreen)),
        Span::raw(buf.clone()),
        Span::styled(cursor, Style::default().fg(Color::White)),
    ]);
    frame.render_widget(
        Paragraph::new(line).style(Style::default().bg(Color::Rgb(30, 30, 30))),
        area,
    );
}

// ─── Home page ─────────────────────────────────────────────────────

const TRAWL_LOGO: &[&str] = &[
    "████████╗ ██████╗ █████╗ ██╗    ██╗██╗     ",
    "╚══██╔══╝██╔══██╗██╔══██╗██║    ██║██║     ",
    "   ██║   ██████╔╝███████║██║ █╗ ██║██║     ",
    "   ██║   ██╔══██╗██╔══██║██║███╗██║██║     ",
    "   ██║   ██║  ██║██║  ██║╚███╔███╔╝███████╗",
    "   ╚═╝   ╚═╝  ╚═╝╚═╝  ╚═╝ ╚══╝╚══╝ ╚══════╝",
];

fn is_home_page(app: &App) -> bool {
    app.active_tab()
        .is_some_and(|t| t.url.as_str() == "trawl:home")
}

fn render_home_page(app: &App, frame: &mut Frame, area: Rect) {
    let mut y = area.y.saturating_add(area.height / 4).max(area.y);

    let logo_cyan = Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD);
    for line in TRAWL_LOGO {
        let cw = line.chars().count() as u16;
        let x = area.x + area.width.saturating_sub(cw) / 2;
        frame.render_widget(
            Paragraph::new(Line::from(Span::styled(*line, logo_cyan))),
            Rect { x, y, width: cw, height: 1 },
        );
        y += 1;
    }

    y += 1;
    let tagline = "\"trawl the web\"";
    let tw = tagline.chars().count() as u16;
    let tx = area.x + area.width.saturating_sub(tw) / 2;
    frame.render_widget(
        Paragraph::new(Line::from(Span::styled(
            tagline,
            Style::default().fg(Color::Rgb(100, 120, 140)),
        ))),
        Rect { x: tx, y, width: tw, height: 1 },
    );

    y += 3;
    let engine = app.config.search_engine.to_string();
    let placeholder = if app.input_buf.is_empty() && !matches!(app.mode, Mode::InsertSearch) {
        "  search or type a URL "
    } else {
        ""
    };
    let query = if matches!(app.mode, Mode::InsertSearch) || !app.input_buf.is_empty() {
        format!(" {} ", app.input_buf)
    } else {
        String::new()
    };
    let cursor = if matches!(app.mode, Mode::InsertSearch) && app.input_buf.is_empty() {
        " \u{2588} "
    } else if matches!(app.mode, Mode::InsertSearch) {
        " "
    } else {
        ""
    };
    let box_inner = format!(" {engine}{query}{cursor}{placeholder}");
    let min_box = 50u16;
    let box_w = (box_inner.chars().count() as u16 + 4).max(min_box);
    let box_x = area.x + area.width.saturating_sub(box_w) / 2;

    let border = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(Color::Rgb(80, 80, 120)));
    let inner = border.inner(Rect {
        x: box_x, y,
        width: box_w, height: 3,
    });
    frame.render_widget(border, Rect {
        x: box_x, y,
        width: box_w, height: 3,
    });

    let engine_style = Style::default()
        .fg(Color::LightGreen)
        .add_modifier(Modifier::BOLD);
    let engine_span = Span::styled(format!("{} ", engine), engine_style);
    let query_span = if !app.input_buf.is_empty() || matches!(app.mode, Mode::InsertSearch) {
        vec![
            engine_span,
            Span::raw(&app.input_buf),
            Span::styled(
                if matches!(app.mode, Mode::InsertSearch) { "\u{2588}" } else { "" },
                Style::default().fg(Color::White),
            ),
        ]
    } else {
        vec![
            Span::styled(
                format!("{}", engine),
                engine_style,
            ),
            Span::styled(
                "  search or type a URL ",
                Style::default().fg(Color::Rgb(60, 60, 80)),
            ),
        ]
    };

    frame.render_widget(
        Paragraph::new(Line::from(query_span)),
        Rect {
            x: inner.x + 1,
            y: inner.y,
            width: inner.width.saturating_sub(2),
            height: 1,
        },
    );

    y += 4;
    let hint = "[s] engine  [Enter] search  [:url] URL  [?] help";
    let hw = hint.chars().count() as u16;
    let hx = area.x + area.width.saturating_sub(hw) / 2;
    frame.render_widget(
        Paragraph::new(Line::from(Span::styled(
            hint,
            Style::default().fg(Color::Rgb(60, 60, 70)),
        ))),
        Rect { x: hx, y, width: hw, height: 1 },
    );
}

// ─── Shared: Help screen ───────────────────────────────────────────

fn render_help_screen(app: &App, frame: &mut Frame, area: Rect) {
    let inner = Rect {
        x: area.x + 2,
        y: area.y + 1,
        width: area.width.saturating_sub(4),
        height: area.height.saturating_sub(2),
    };
    if inner.is_empty() {
        return;
    }
    let layout_name = match app.config.layout {
        LayoutStyle::BrowserChrome => "Browser-Chrome",
        LayoutStyle::LazyGit => "lazygit-Panels",
    };

    let lines = vec![
        Line::from(Span::styled(
            " NAVIGATION",
            Style::default()
                .fg(Color::LightBlue)
                .add_modifier(Modifier::BOLD),
        )),
        Line::from("  j/k         Scroll up/down"),
        Line::from("  d/u         Scroll half page"),
        Line::from("  g/G         Go to top/bottom"),
        Line::from(""),
        Line::from(Span::styled(
            " VIEW MODE (v)",
            Style::default()
                .fg(Color::LightBlue)
                .add_modifier(Modifier::BOLD),
        )),
        Line::from("  j/k         Line up/down"),
        Line::from("  h/l         Char left/right"),
        Line::from("  w/b/e       Word next/prev/end"),
        Line::from("  Enter       Set start/end mark"),
        Line::from("  y           Yank marked range"),
        Line::from(""),
        Line::from(Span::styled(
            " SEARCH CARDS",
            Style::default()
                .fg(Color::LightBlue)
                .add_modifier(Modifier::BOLD),
        )),
        Line::from("  j/k         Select prev/next card"),
        Line::from("  g/G         First/last card"),
        Line::from("  Enter       Open selected link"),
        Line::from(""),
        Line::from(Span::styled(
            " PAGES",
            Style::default()
                .fg(Color::LightBlue)
                .add_modifier(Modifier::BOLD),
        )),
        Line::from("  :           Open URL"),
        Line::from("  t           Open URL in new tab"),
        Line::from("  R           Reload"),
        Line::from("  T/N         Prev/Next tab"),
        Line::from("  x           Close tab"),
        Line::from("  1-9         Follow link #"),
        Line::from(""),
        Line::from(Span::styled(
            " HISTORY & BOOKMARKS",
            Style::default()
                .fg(Color::LightBlue)
                .add_modifier(Modifier::BOLD),
        )),
        Line::from("  H/L         Back/Forward"),
        Line::from("  b           Show bookmarks sidebar"),
        Line::from("  B           Toggle bookmark"),
        Line::from("  h           Show history"),
        Line::from("  Enter       Show links"),
        Line::from("  Tab         Toggle sidebar"),
        Line::from("  l / Esc     Hide sidebar"),
        Line::from(""),
        Line::from(Span::styled(
            " SEARCH & LAYOUT",
            Style::default()
                .fg(Color::LightBlue)
                .add_modifier(Modifier::BOLD),
        )),
        Line::from("  /           Search"),
        Line::from("  s           Switch engine"),
        Line::from("  \\           Toggle layout"),
        Line::from("  f           Toggle focus frame"),
        Line::from("  Tab         Cycle panels (lazygit mode)"),
        Line::from("  ?           This help"),
        Line::from(""),
        Line::from(Span::styled(
            " LAYOUTS",
            Style::default().fg(Color::LightYellow),
        )),
        Line::from(Span::raw(format!("  Current: {layout_name}"))),
        Line::from("  [3] Browser-Chrome — address bar on top"),
        Line::from("  [4] lazygit-Panels — three-column info panel"),
        Line::from(""),
        Line::from(Span::styled(
            " DISPLAY",
            Style::default().fg(Color::LightYellow),
        )),
        Line::from(format!(
            "  Focus Frame: {}  Layout: {layout_name}",
            if app.focus_frame_visible { "ON" } else { "OFF" }
        )),
        Line::from(""),
        Line::from(Span::styled(
            " MEDIA",
            Style::default().fg(Color::LightYellow),
        )),
        Line::from(format!(
            "  Protocol: {} | TrueColor: {}",
            app.term_cap.best_protocol(),
            app.term_cap.true_color
        )),
        Line::from(format!(
            "  Images: {}  Video: {}",
            if app.term_cap.supports_images() {
                "yes"
            } else {
                "no"
            },
            if app.term_cap.supports_video() {
                "yes"
            } else {
                "no"
            }
        )),
        Line::from(""),
        Line::from(Span::styled(
            " q/Esc to close",
            Style::default().fg(Color::DarkGray),
        )),
    ];
    let height = inner.height as usize;
    let start = app.help_scroll.min(lines.len().saturating_sub(height));
    let end = (start + height).min(lines.len());
    frame.render_widget(Paragraph::new(lines[start..end].to_vec()), inner);
}
