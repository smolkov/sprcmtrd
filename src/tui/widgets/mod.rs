//! Rendering of the TUI layout and overlays.

mod channels;
mod filepicker;
mod footer;
mod header;
mod help;
mod info;
mod plot;

use ratatui::Frame;
use ratatui::layout::{Alignment, Constraint, Layout, Rect};
use ratatui::widgets::{Block, Clear, Paragraph, Wrap};

use super::app::{App, Overlay};

/// Smallest supported terminal size.
pub const MIN_SIZE: (u16, u16) = (80, 24);
/// Width from which the big header banner is shown.
const BIG_HEADER_WIDTH: u16 = 100;

/// Draws the whole screen.
pub fn draw(f: &mut Frame, app: &App) {
    let area = f.area();
    f.render_widget(Block::new().style(app.theme.base()), area);

    if area.width < MIN_SIZE.0 || area.height < MIN_SIZE.1 {
        let text = format!(
            "Terminal too small: {}×{}\nneeds at least {}×{}",
            area.width, area.height, MIN_SIZE.0, MIN_SIZE.1
        );
        let y = area.height / 2;
        let msg = Rect { y: area.y + y.saturating_sub(1), height: 2.min(area.height), ..area };
        f.render_widget(
            Paragraph::new(text).alignment(Alignment::Center).style(app.theme.warning()),
            msg,
        );
        return;
    }

    let header_height = if area.width >= BIG_HEADER_WIDTH { 3 } else { 1 };
    let [header_area, body, footer_area] = Layout::vertical([
        Constraint::Length(header_height),
        Constraint::Min(0),
        Constraint::Length(1),
    ])
    .areas(area);

    header::render(f, header_area, app);
    match (&app.recording, app.sidebar) {
        (Some(rec), true) => {
            let width = channels::width(rec).min(body.width / 3);
            let [side, main] =
                Layout::horizontal([Constraint::Length(width), Constraint::Min(0)]).areas(body);
            channels::render(f, side, app);
            plot::render(f, main, app);
        }
        _ => plot::render(f, body, app),
    }
    footer::render(f, footer_area, app);

    match &app.overlay {
        Some(Overlay::Help { scroll }) => help::render(f, area, app, *scroll),
        Some(Overlay::Info { scroll }) => info::render(f, area, app, *scroll),
        Some(Overlay::Picker(p)) => filepicker::render(f, area, app, p),
        None => {}
    }
}

/// A rectangle of `width_pct` × `height_pct` percent centered in `area`.
fn centered(area: Rect, width_pct: u16, height_pct: u16) -> Rect {
    let width = area.width * width_pct / 100;
    let height = area.height * height_pct / 100;
    Rect {
        x: area.x + (area.width - width) / 2,
        y: area.y + (area.height - height) / 2,
        width,
        height,
    }
}

/// Draws an overlay frame (cleared background, bordered block) and returns its inner area.
fn overlay_block(f: &mut Frame, area: Rect, app: &App, title: &str) -> Rect {
    f.render_widget(Clear, area);
    let block = Block::bordered()
        .title(format!(" {title} "))
        .title_style(app.theme.accent())
        .border_style(app.theme.border(true))
        .style(app.theme.base());
    let inner = block.inner(area);
    f.render_widget(block, area);
    inner
}

/// Renders scrollable text lines; the scroll offset is clamped to the content.
fn scrolled_text(f: &mut Frame, area: Rect, app: &App, lines: Vec<ratatui::text::Line<'static>>, scroll: u16) {
    let max = (lines.len() as u16).saturating_sub(area.height);
    f.render_widget(
        Paragraph::new(lines)
            .style(app.theme.base())
            .wrap(Wrap { trim: false })
            .scroll((scroll.min(max), 0)),
        area,
    );
}

#[cfg(test)]
mod tests {
    use ratatui::Terminal;
    use ratatui::backend::TestBackend;
    use ratatui::buffer::Buffer;
    use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

    use super::*;
    use crate::args::{GlobalOpts, ThemeChoice};
    use crate::command::Context;
    use crate::comtrade::Comtrade;

    fn app() -> App {
        let ctx = Context {
            global: GlobalOpts { theme: Some(ThemeChoice::Dark), secondary: false, no_color: true, verbose: 0 },
            config: Default::default(),
        };
        let (tx, _rx) = tokio::sync::mpsc::unbounded_channel();
        let mut app = App::new(&ctx, tx).unwrap();
        let path = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/data/fault_1999.cfg");
        app.set_recording(Comtrade::open(path).unwrap());
        app
    }

    fn render(app: &App, w: u16, h: u16) -> String {
        let mut t = Terminal::new(TestBackend::new(w, h)).unwrap();
        t.draw(|f| draw(f, app)).unwrap();
        text(t.backend().buffer())
    }

    fn text(buf: &Buffer) -> String {
        let mut s = String::new();
        for y in 0..buf.area.height {
            for x in 0..buf.area.width {
                s.push_str(buf[(x, y)].symbol());
            }
            s.push('\n');
        }
        s
    }

    #[test]
    fn renders_main_layout() {
        let screen = render(&app(), 110, 30);
        if std::env::var_os("SHOW_SCREEN").is_some() {
            println!("{screen}");
        }
        assert!(screen.contains("fault_1999.cfg"));
        assert!(screen.contains("Channels"));
        assert!(screen.contains("IA [A]"));
        assert!(screen.contains("TRIP"));
        assert!(screen.contains("quit"));
    }

    #[test]
    fn renders_overlays_and_small_terminal() {
        let mut app = app();
        app.on_key(KeyEvent::new(KeyCode::Char('i'), KeyModifiers::NONE));
        let screen = render(&app, 100, 30);
        assert!(screen.contains("Recording info"));
        assert!(screen.contains("SUBSTATION A"));
        app.on_key(KeyEvent::new(KeyCode::Char('?'), KeyModifiers::SHIFT));
        assert!(render(&app, 100, 30).contains("Toggle dark / light theme"));
        assert!(render(&app, 60, 20).contains("Terminal too small"));
    }

    #[test]
    fn zoom_and_cursor_keys_change_view() {
        let mut app = app();
        let full = app.recording.as_ref().unwrap().view;
        app.on_key(KeyEvent::new(KeyCode::Char('+'), KeyModifiers::NONE));
        let zoomed = app.recording.as_ref().unwrap().view;
        assert!((zoomed.1 - zoomed.0 - (full.1 - full.0) / 2.0).abs() < 1e-9);
        app.on_key(KeyEvent::new(KeyCode::Right, KeyModifiers::NONE));
        assert!(app.recording.as_ref().unwrap().cursor > 0.0);
        app.on_key(KeyEvent::new(KeyCode::Char('0'), KeyModifiers::NONE));
        assert_eq!(app.recording.as_ref().unwrap().view, full);
        app.on_key(KeyEvent::new(KeyCode::Char('q'), KeyModifiers::NONE));
        assert!(app.quit);
    }
}
