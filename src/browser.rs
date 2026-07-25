use ratatui::prelude::*;
use ratatui_image::protocol::StatefulProtocol;
use scraper::{Html, Selector};
use url::Url;
use std::sync::{OnceLock, Mutex};

fn shared_agent() -> &'static Mutex<ureq::Agent> {
    static AGENT: OnceLock<Mutex<ureq::Agent>> = OnceLock::new();
    AGENT.get_or_init(|| Mutex::new(ureq::Agent::new_with_defaults()))
}

#[derive(Clone)]
pub struct Link {
    pub number: usize,
    pub url: String,
    pub text: String,
}

pub struct ImageInfo {
    pub line_idx: usize,
    pub src: String,
    pub alt: String,
}

pub struct SearchCard {
    pub number: usize,
    pub title: String,
    pub snippet: String,
    pub url: String,
}

fn is_nav_link(link: &Link) -> bool {
    if link.text.trim().chars().count() <= 4 { return true; }
    if let Ok(url) = Url::parse(&link.url) {
        if let Some(host) = url.host_str() {
            if host.contains("bing.com") || host.contains("google.") { return true; }
        }
    }
    false
}

pub fn build_search_cards(links: &[Link], lines: &[Line<'static>]) -> Vec<SearchCard> {
    links.iter().filter(|l| !is_nav_link(l)).map(|link| {
        let snippet = extract_snippet(link, lines);
        SearchCard { number: link.number, title: link.text.clone(), snippet, url: link.url.clone() }
    }).collect()
}

fn extract_snippet(link: &Link, lines: &[Line<'static>]) -> String {
    let link_lower = link.text.to_lowercase();
    for (i, line) in lines.iter().enumerate() {
        let text: String = line.spans.iter().map(|s| s.content.as_ref()).collect();
        if text.to_lowercase().contains(&link_lower) {
            let mut snippet = String::new();
            let mut lines_taken = 0u8;
            for j in (i + 1)..lines.len().min(i + 5) {
                let next: String = lines[j].spans.iter().map(|s| s.content.as_ref()).collect();
                let next = next.trim();
                if next.is_empty() || next.starts_with("http") || next.len() < 5 { continue; }
                if !snippet.is_empty() { snippet.push(' '); }
                snippet.push_str(next);
                lines_taken += 1;
                if lines_taken >= 2 { break; }
            }
            if snippet.chars().count() > 80 { return snippet.chars().take(77).collect::<String>() + "..."; }
            return snippet;
        }
    }
    String::new()
}

pub struct VideoCard {
    pub number: usize,
    pub title: String,
    pub url: String,
    pub author: String,
    pub views: String,
    pub duration: String,
    pub thumb_url: String,
    pub cover: Option<StatefulProtocol>,
}

fn is_video_link(link: &Link) -> bool {
    let t = link.text.trim();
    // filter short text (navigation labels like "游戏中心", "会员购")
    if t.chars().count() <= 4 {
        return false;
    }
    // filter shallow section links like /game, /member
    if let Ok(url) = Url::parse(&link.url) {
        let path = url.path().trim_end_matches('/');
        let segments: Vec<&str> = path.split('/').filter(|s| !s.is_empty()).collect();
        if segments.len() <= 1 && url.query().is_none() {
            return false;
        }
    }
    true
}

pub fn extract_video_cards(links: &[Link]) -> Vec<VideoCard> {
    links
        .iter()
        .filter(|link| is_video_link(link))
        .map(|link| {
            let text = link.text.trim().to_string();
            let title = if text.len() > 60 {
                text.chars().take(57).collect::<String>() + "..."
            } else {
                text
            };
            VideoCard {
                number: link.number,
                title,
                url: link.url.clone(),
                author: String::new(),
                views: String::new(),
                duration: String::new(),
                thumb_url: String::new(),
                cover: None,
            }
        })
        .collect()
}

pub fn extract_categories(links: &[Link]) -> Vec<String> {
    let mut cats = vec!["🔍 搜索".to_string(), "📋 全部".to_string()];
    let max_total = 10;
    for link in links {
        let t = link.text.trim();
        if t.len() <= 6 && !t.is_empty() && !cats.iter().any(|c| c == t) {
            cats.push(t.to_string());
            if cats.len() >= max_total {
                break;
            }
        }
    }
    cats
}

pub struct RenderedPage {
    pub title: String,
    pub lines: Vec<Line<'static>>,
    pub links: Vec<Link>,
    pub raw_html: String,
    pub images: Vec<ImageInfo>,
    pub next_page_url: Option<String>,
    pub prev_page_url: Option<String>,
}

pub fn resolve_url(base: &Url, href: &str) -> String {
    if href.starts_with("javascript:") || href.starts_with("mailto:") || href.starts_with("#") {
        return base.to_string();
    }
    Url::parse(href)
        .or_else(|_| base.join(href))
        .map(|u| u.to_string())
        .unwrap_or_else(|_| href.to_string())
}

pub fn fetch_page(url_str: &str) -> Result<RenderedPage, String> {
    let url = Url::parse(url_str).map_err(|e| format!("Invalid URL: {e}"))?;
    let scheme = url.scheme();
    if scheme != "http" && scheme != "https" {
        return Err(format!("Unsupported scheme: {scheme}"));
    }

    let agent = shared_agent();
    let mut response = agent
        .lock()
        .map_err(|e| format!("Lock error: {e}"))?
        .get(url_str)
        .call()
        .map_err(|e| format!("HTTP error: {e}"))?;

    let status = response.status();
    if status != 200 {
        return Err(format!("HTTP {status}"));
    }

    let ct = response
        .headers()
        .get("content-type")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("text/html")
        .to_string();

    let body = response
        .body_mut()
        .read_to_string()
        .map_err(|e| format!("Read error: {e}"))?;

    if ct.contains("text/html") {
        let mut page = render_html(&body, &url, body.clone())?;
        let (next, prev) = extract_pagination_urls(&body, &url);
        page.next_page_url = next;
        page.prev_page_url = prev;
        Ok(page)
    } else if ct.starts_with("text/") {
        render_plain(&body, &url)
    } else {
        render_binary(&ct, &url)
    }
}

fn render_plain(body: &str, url: &Url) -> Result<RenderedPage, String> {
    let lines: Vec<Line<'static>> = body.lines().map(|l| Line::from(l.to_string())).collect();
    Ok(RenderedPage {
        title: url.to_string(),
        lines,
        links: vec![],
        raw_html: body.to_string(),
        images: vec![],
        next_page_url: None,
        prev_page_url: None,
    })
}

fn render_binary(ct: &str, url: &Url) -> Result<RenderedPage, String> {
    let lines = vec![
        Line::from(Span::styled(
            format!(" Binary: {ct}"),
            Style::default().fg(Color::Yellow),
        )),
        Line::from(Span::styled(
            url.to_string(),
            Style::default().fg(Color::Cyan),
        )),
        Line::from(""),
        Line::from(Span::styled(
            " Press 'd' to download",
            Style::default().fg(Color::DarkGray),
        )),
    ];
    Ok(RenderedPage {
        title: format!("Binary: {url}"),
        lines,
        links: vec![],
        raw_html: String::new(),
        images: vec![],
        next_page_url: None,
        prev_page_url: None,
    })
}

fn render_html(html: &str, base: &Url, raw_html: String) -> Result<RenderedPage, String> {
    let doc = Html::parse_document(html);
    let mut lines: Vec<Line<'static>> = Vec::new();

    let title = Selector::parse("title")
        .ok()
        .and_then(|sel| {
            doc.select(&sel)
                .next()
                .map(|e| e.text().collect::<String>().trim().to_string())
        })
        .unwrap_or_else(|| "Untitled".into());

    let mut link_n = 1;
    let mut links: Vec<Link> = Vec::new();

    if let Ok(sel) = Selector::parse("a[href]") {
        for el in doc.select(&sel) {
            let href = el.attr("href").unwrap_or("");
            let text: String = el.text().collect::<Vec<_>>().concat().trim().to_string();
            if !href.is_empty() && !text.is_empty() {
                let resolved = resolve_url(base, href);
                links.push(Link {
                    number: link_n,
                    url: resolved,
                    text: text.clone(),
                });
                link_n += 1;
            }
        }
    }

    for tag in &["h1", "h2", "h3"] {
        if let Ok(sel) = Selector::parse(tag) {
            for el in doc.select(&sel) {
                let t: String = el.text().collect::<Vec<_>>().concat().trim().to_string();
                if !t.is_empty() {
                    let style = Style::default()
                        .fg(Color::White)
                        .add_modifier(Modifier::BOLD);
                    lines.push(Line::from(Span::styled(t, style)));
                    lines.push(Line::from(""));
                }
            }
        }
    }

    if let Ok(sel) = Selector::parse("p") {
        for el in doc.select(&sel) {
            let t: String = el.text().collect::<Vec<_>>().concat().trim().to_string();
            if !t.is_empty() {
                lines.push(Line::from(t));
                lines.push(Line::from(""));
            }
        }
    }

    if let Ok(sel) = Selector::parse("blockquote") {
        for el in doc.select(&sel) {
            let t: String = el.text().collect::<Vec<_>>().concat().trim().to_string();
            if !t.is_empty() {
                lines.push(Line::from(Span::styled(
                    format!(" \u{2502} {t}"),
                    Style::default().fg(Color::DarkGray),
                )));
                lines.push(Line::from(""));
            }
        }
    }

    if let Ok(sel) = Selector::parse("ul") {
        for ul in doc.select(&sel) {
            if let Ok(li_sel) = Selector::parse("li") {
                for li in ul.select(&li_sel) {
                    let t: String = li.text().collect::<Vec<_>>().concat().trim().to_string();
                    if !t.is_empty() {
                        lines.push(Line::from(format!(" \u{2022} {t}")));
                    }
                }
            }
            lines.push(Line::from(""));
        }
    }

    if let Ok(sel) = Selector::parse("pre") {
        for el in doc.select(&sel) {
            let t: String = el.text().collect::<Vec<_>>().concat().to_string();
            if !t.is_empty() {
                for line in t.lines() {
                    lines.push(Line::from(Span::styled(
                        line.to_string(),
                        Style::default().fg(Color::Green),
                    )));
                }
                lines.push(Line::from(""));
            }
        }
    }

    let mut images: Vec<ImageInfo> = vec![];
    if let Ok(sel) = Selector::parse("img") {
        for el in doc.select(&sel) {
            let alt = el.attr("alt").unwrap_or("img").to_string();
            let src = el.attr("src").unwrap_or("").to_string();
            let resolved = if src.starts_with("http") {
                src.clone()
            } else {
                base.join(&src).map(|u| u.to_string()).unwrap_or(src.clone())
            };
            images.push(ImageInfo {
                line_idx: lines.len(),
                src: resolved,
                alt,
            });
            lines.push(Line::from(Span::styled(
                " \u{1F5BC} [image]",
                Style::default().fg(Color::Magenta),
            )));
        }
    }

    if lines.is_empty() {
        lines.push(Line::from(Span::styled(
            " (empty)",
            Style::default().fg(Color::DarkGray),
        )));
    }

    Ok(RenderedPage {
        title,
        lines,
        links,
        raw_html,
        images,
        next_page_url: None,
        prev_page_url: None,
    })
}

pub fn extract_embedded_videos(html: &str, base: &Url) -> Vec<VideoCard> {
    let mut cards: Vec<VideoCard> = Vec::new();

    // Look for __INITIAL_STATE__ in script blocks (bilibili, etc.)
    let doc = Html::parse_document(html);
    if let Ok(sel) = Selector::parse("script") {
        for script in doc.select(&sel) {
            let text: String = script.text().collect();
            if text.contains("__INITIAL_STATE__") {
                if let Some(eq_pos) = text.find('=') {
                    let json_str = text[eq_pos + 1..].trim().trim_end_matches(';');
                    if let Ok(val) = serde_json::from_str::<serde_json::Value>(json_str) {
                        scan_for_videos(&val, base, &mut cards);
                        if !cards.is_empty() {
                            return cards;
                        }
                    }
                }
            }
        }
    }

    // Fallback: scan all script tags for JSON arrays
    if let Ok(sel) = Selector::parse("script") {
        for script in doc.select(&sel) {
            let text: String = script.text().collect();
            let trimmed = text.trim();
            if trimmed.len() < 200 || trimmed.len() > 100000 {
                continue;
            }
            // try to extract JSON object or array from the JS content
            if let Some(start) = trimmed.find('{') {
                if let Some(end) = trimmed.rfind('}') {
                    let json_slice = &trimmed[start..=end];
                    if let Ok(val) = serde_json::from_str::<serde_json::Value>(json_slice) {
                        let before = cards.len();
                        scan_for_videos(&val, base, &mut cards);
                        if cards.len() > before {
                            return cards;
                        }
                    }
                }
            }
        }
    }

    cards
}

fn scan_for_videos(val: &serde_json::Value, base: &Url, cards: &mut Vec<VideoCard>) {
    // known keys that typically hold video arrays
    let video_keys = [
        "videoData", "recommendData", "items", "list", "data",
        "videos", "results", "cards", "item", "feed",
    ];
    // if it's an array, check each item
    if let serde_json::Value::Array(arr) = val {
        for item in arr {
            if let Some(v) = try_extract_video(item, base) {
                cards.push(v);
                if cards.len() >= 200 {
                    return;
                }
            }
        }
        return;
    }
    // if it's an object, look for known video keys
    if let serde_json::Value::Object(map) = val {
        for key in &video_keys {
            if let Some(v) = map.get(*key) {
                scan_for_videos(v, base, cards);
                if !cards.is_empty() {
                    return;
                }
            }
        }
        // if no known key found, recurse into all values
        if cards.is_empty() {
            for v in map.values() {
                scan_for_videos(v, base, cards);
                if !cards.is_empty() {
                    return;
                }
            }
        }
    }
}



pub fn fetch_videos_from_api(url: &str) -> (Vec<VideoCard>, u32) {
    let lower = url.to_lowercase();
    if lower.contains("bilibili.com") {
        let d = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default();
        let page = ((d.as_secs() / 10) % 10 + 1) as u32;
        return (fetch_bilibili_region(0, page, 50), page);
    }
    (vec![], 1)
}

pub fn fetch_videos_api_page(rid: u32, page: u32) -> Vec<VideoCard> {
    fetch_bilibili_region(rid, page, 50)
}

fn parse_video_list(val: &serde_json::Value, list_key: &str) -> Vec<VideoCard> {
    let mut cards = Vec::new();
    let list = val.pointer(list_key).and_then(|l| l.as_array());
    let list = match list {
        Some(l) => l,
        None => return cards,
    };
    for item in list {
        let title = item.get("title").and_then(|t| t.as_str()).unwrap_or("");
        if title.is_empty() {
            continue;
        }
        let pic = item.get("pic").and_then(|p| p.as_str()).unwrap_or("");
        let bvid = item.get("bvid").and_then(|b| b.as_str()).unwrap_or("");
        if bvid.is_empty() {
            continue;
        }
        let video_url = format!("https://www.bilibili.com/video/{bvid}");
        let thumb_url = if !pic.is_empty() {
            if pic.starts_with("http") {
                pic.to_string()
            } else {
                format!("https:{pic}")
            }
        } else {
            String::new()
        };
        let author = item
            .get("owner")
            .and_then(|o| o.get("name"))
            .and_then(|n| n.as_str())
            .unwrap_or("");
        let views = item
            .get("stat")
            .and_then(|s| s.get("view"))
            .and_then(|v| v.as_i64())
            .map(|v| if v >= 10000 { format!("{:.1}万", v as f64 / 10000.0) } else { v.to_string() })
            .unwrap_or_default();
        let duration = item
            .get("duration")
            .and_then(|d| d.as_i64())
            .map(|secs| format!("{:02}:{:02}", secs / 60, secs % 60))
            .unwrap_or_default();
        cards.push(VideoCard {
            number: cards.len() + 1,
            title: title.to_string(),
            url: video_url,
            author: author.to_string(),
            views,
            duration,
            thumb_url,
            cover: None,
        });
    }
    cards
}

pub fn fetch_bilibili_region(rid: u32, page: u32, page_size: u32) -> Vec<VideoCard> {
    let api_url = if rid == 0 {
        format!("https://api.bilibili.com/x/web-interface/popular?pn={}&ps={}", page.max(1), page_size.max(1))
    } else {
        format!("https://api.bilibili.com/x/web-interface/ranking/v2?rid={rid}&type=all")
    };
    let agent = ureq::Agent::new_with_defaults();
    let mut resp = match agent.get(&api_url).call() {
        Ok(r) => r,
        Err(_) => return vec![],
    };
    let body: String = match resp.body_mut().read_to_string() {
        Ok(b) => b,
        Err(_) => return vec![],
    };
    let val: serde_json::Value = match serde_json::from_str(&body) {
        Ok(v) => v,
        Err(_) => return vec![],
    };
    if val.get("code").and_then(|c| c.as_i64()) != Some(0) {
        // fallback to popular for rids that don't have a ranking category
        let fb_url = format!("https://api.bilibili.com/x/web-interface/popular?pn=1&ps={}", page_size.max(1));
        if let Ok(mut resp) = agent.get(&fb_url).call() {
            if let Ok(body) = resp.body_mut().read_to_string() {
                if let Ok(val) = serde_json::from_str::<serde_json::Value>(&body) {
                    return parse_video_list(&val, "/data/list");
                }
            }
        }
        return vec![];
    }
    parse_video_list(&val, "/data/list")
}

fn try_extract_video(item: &serde_json::Value, base: &Url) -> Option<VideoCard> {
    let obj = item.as_object()?;
    let title = obj.get("title")?.as_str()?;
    if title.is_empty() {
        return None;
    }
    let uri = obj.get("uri")
        .or_else(|| obj.get("link"))
        .or_else(|| obj.get("url"))?
        .as_str()?;
    // convert bilibili:// scheme to https://
    let resolved = if uri.starts_with("bilibili://") {
        uri.replacen("bilibili://", "https://www.bilibili.com/", 1)
    } else {
        resolve_url(base, uri)
    };
    let pic = obj.get("pic").or_else(|| obj.get("cover"))
        .and_then(|p| p.as_str())
        .unwrap_or("");
    let thumb_url = if !pic.is_empty() {
        resolve_url(base, pic)
    } else {
        String::new()
    };
    let author = obj.get("author")
        .or_else(|| obj.get("up"))
        .and_then(|a| a.as_str())
        .or_else(|| {
            obj.get("owner").and_then(|o| {
                o.get("name").and_then(|n| n.as_str())
            })
        })
        .or_else(|| {
            obj.get("author").and_then(|a| {
                a.get("name").and_then(|n| n.as_str())
            })
        })
        .unwrap_or("");
    Some(VideoCard {
        number: 0,
        title: title.to_string(),
        url: resolved,
        author: author.to_string(),
        views: String::new(),
        duration: String::new(),
        thumb_url,
        cover: None,
    })
}

pub fn render_content_lines<'a>(
    lines: &'a [Line<'static>],
    scroll: usize,
    height: usize,
) -> Vec<&'a Line<'static>> {
    if lines.is_empty() || height == 0 {
        return vec![];
    }
    let max = lines.len();
    let start = scroll.min(max.saturating_sub(height));
    let end = (start + height).min(max);
    lines[start..end].iter().collect()
}

pub fn extract_pagination_urls(html: &str, base: &Url) -> (Option<String>, Option<String>) {
    let doc = Html::parse_fragment(html);
    let sel_next = Selector::parse("a.sb_pagN").ok();
    let sel_prev = Selector::parse(r#"a[aria-label="Previous page"]"#).ok();
    let next = sel_next.and_then(|s| {
        doc.select(&s).next().and_then(|el| el.value().attr("href"))
    });
    let prev = sel_prev.and_then(|s| {
        doc.select(&s).next().and_then(|el| el.value().attr("href"))
    });
    let resolve = |href: &str| -> String {
        if href.starts_with("http") { href.to_string() }
        else { base.join(href).map(|u| u.to_string()).unwrap_or_else(|_| href.to_string()) }
    };
    (next.map(resolve), prev.map(resolve))
}
