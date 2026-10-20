//! Interactive terminal user interface.

pub mod app;
pub mod event;
pub mod theme;
pub mod widgets;

use std::path::PathBuf;

use ratatui::DefaultTerminal;
use tokio::sync::mpsc;

use self::app::App;
use self::event::{Event, Events};
use crate::command::Context;

/// Runs the TUI. `path` may be a recording file, a directory to browse, or
/// `None` to browse the current directory.
///
/// The terminal is restored on exit, on errors and on panics.
pub async fn run(path: Option<PathBuf>, ctx: &Context) -> anyhow::Result<()> {
    let (tx, rx) = mpsc::unbounded_channel();
    // Configuration errors are reported before the screen is taken over.
    let mut app = App::new(ctx, tx)?;
    match path {
        Some(p) => app.open(&p),
        None => app.open_picker(PathBuf::from(".")),
    }

    // `try_init` enables raw mode and the alternate screen and installs a
    // panic hook that restores the terminal.
    let mut terminal = ratatui::try_init()?;
    let result = event_loop(&mut terminal, &mut app, Events::new(rx)).await;
    ratatui::restore();
    result
}

async fn event_loop(terminal: &mut DefaultTerminal, app: &mut App, mut events: Events) -> anyhow::Result<()> {
    terminal.draw(|f| widgets::draw(f, app))?;
    while !app.quit {
        let Some(event) = events.next().await? else {
            break;
        };
        let redraw = match event {
            Event::Key(key) => {
                app.on_key(key);
                true
            }
            Event::Resize => true,
            Event::Tick => app.on_tick(),
            Event::Message(msg) => {
                app.on_message(msg);
                true
            }
        };
        if redraw && !app.quit {
            terminal.draw(|f| widgets::draw(f, app))?;
        }
    }
    Ok(())
}
