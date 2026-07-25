pub mod app;
pub mod browser;
pub mod config;
pub mod input;
pub mod media;
pub mod ui;

use crossterm::tty::IsTty;
use crossterm::{
    execute,
    terminal::{EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode},
};
use ratatui::Terminal;
use std::io;
use std::time::Duration;

fn main() -> io::Result<()> {
    if !io::stdout().is_tty() {
        eprintln!("trawl requires a TTY terminal");
        std::process::exit(1);
    }

    enable_raw_mode()?;
    let mut stderr = io::stderr();
    execute!(stderr, EnterAlternateScreen)?;

    let mut terminal = Terminal::new(ratatui::backend::CrosstermBackend::new(stderr))?;
    let mut app = app::App::new()?;

    while !app.should_quit {
        if let Err(e) = terminal.draw(|f| ui::render(&mut app, f)) {
            eprintln!("Render error: {e}");
            break;
        }
        app.tick();

        if crossterm::event::poll(Duration::from_millis(30))?
            && let crossterm::event::Event::Key(key) = crossterm::event::read()?
        {
            input::handle_key(&mut app, key);
        }
    }

    execute!(io::stdout(), LeaveAlternateScreen)?;
    disable_raw_mode()?;
    Ok(())
}
