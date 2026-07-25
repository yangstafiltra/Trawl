use std::env;
use std::process::Command;

#[derive(Clone, Debug, Default)]
pub struct TerminalCapabilities {
    pub kitty: bool,
    pub iterm2: bool,
    pub sixel: bool,
    pub color: bool,
    pub true_color: bool,
}

impl TerminalCapabilities {
    pub fn detect() -> Self {
        let term = env::var("TERM").unwrap_or_default();
        let term_program = env::var("TERM_PROGRAM").unwrap_or_default();
        let kitty_window = env::var("KITTY_WINDOW_ID").is_ok();
        let kitty = kitty_window || term == "xterm-kitty";
        let iterm2 = term_program == "iTerm.app" || env::var("ITERM_SESSION_ID").is_ok();
        let sixel = term.contains("sixel");
        let true_color = env::var("COLORTERM")
            .map(|v| v == "truecolor" || v == "24bit")
            .unwrap_or(false);
        let color = !term.is_empty()
            && (term.contains("color") || term.contains("kitty") || kitty || iterm2 || true_color);
        Self {
            kitty,
            iterm2,
            sixel,
            color,
            true_color,
        }
    }

    pub fn supports_images(&self) -> bool {
        self.kitty || self.iterm2 || self.sixel
    }
    pub fn supports_video(&self) -> bool {
        self.supports_images()
    }

    pub fn best_protocol(&self) -> &str {
        if self.kitty {
            "kitty"
        } else if self.sixel {
            "sixel"
        } else if self.iterm2 {
            "iterm2"
        } else {
            "none"
        }
    }
}

pub fn check_external_player() -> Option<String> {
    for player in &["mpv", "ffplay", "vlc"] {
        if Command::new(player).arg("--version").output().is_ok() {
            return Some(player.to_string());
        }
    }
    None
}

pub fn get_image_size(data: &[u8]) -> Option<(u32, u32)> {
    image::ImageReader::new(std::io::Cursor::new(data))
        .with_guessed_format()
        .ok()
        .and_then(|r| r.into_dimensions().ok())
}

pub fn encode_kitty_image(data: &[u8]) -> Option<String> {
    use base64::Engine;
    let encoded = base64::engine::general_purpose::STANDARD.encode(data);
    let (w, h) = get_image_size(data)?;
    Some(format!("\x1b_Ga=T,f=100,w={w},h={h},m=0;{encoded}\x1b\\"))
}

pub fn encode_iterm2_image(data: &[u8]) -> Option<String> {
    use base64::Engine;
    let encoded = base64::engine::general_purpose::STANDARD.encode(data);
    Some(format!("\x1b]1337;File=inline=1:{encoded}\x07"))
}

pub fn play_video(url: &str) -> Option<String> {
    let player = check_external_player()?;
    Some(format!("{player} '{url}'"))
}

pub fn play_with_mpv(url: &str) {
    use crossterm::terminal::{disable_raw_mode, enable_raw_mode};

    let _ = disable_raw_mode();

    if let Ok(mut child) = Command::new("mpv")
        .arg(url)
        .arg("--keep-open=yes")
        .arg("--really-quiet")
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .spawn()
    {
        let _ = child.wait();
    }

    let _ = enable_raw_mode();
}

pub fn is_image_mime(mime: &str) -> bool {
    mime.starts_with("image/")
}
pub fn is_video_mime(mime: &str) -> bool {
    mime.starts_with("video/")
}
pub fn is_audio_mime(mime: &str) -> bool {
    mime.starts_with("audio/")
}
