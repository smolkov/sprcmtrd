//! Rendering of the TUI layout and overlays.

mod channels;
mod config;
mod filepicker;
mod footer;
mod header;
mod help;
mod info;
mod plot;
mod time_axis;

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
        let msg = Rect {
            y: area.y + y.saturating_sub(1),
            height: 2.min(area.height),
            ..area
        };
        f.render_widget(
            Paragraph::new(text)
                .alignment(Alignment::Center)
                .style(app.theme.warning()),
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
            let [side, main] = Layout::horizontal([Constraint::Length(width), Constraint::Min(0)]).areas(body);
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
        Some(Overlay::Config(state)) => config::render(f, area, app, state),
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
    use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
    use ratatui::Terminal;
    use ratatui::backend::TestBackend;
    use ratatui::buffer::Buffer;

    use super::*;
    use crate::args::{GlobalOpts, ThemeChoice};
    use crate::comtrade::Comtrade;
    use crate::context::Context;

    fn app() -> App {
        app_with_config(None)
    }

    /// Test app; `config_path` is where the configuration overlay saves.
    fn app_with_config(config_path: Option<std::path::PathBuf>) -> App {
        let ctx = Context {
            global: GlobalOpts {
                theme: Some(ThemeChoice::Dark),
                secondary: false,
                no_color: true,
                verbose: 0,
            },
            config: Default::default(),
            config_path,
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
        assert!(render(&app, 100, 40).contains("Toggle dark / light theme"));
        assert!(render(&app, 60, 20).contains("Terminal too small"));
    }

    /// Second to last row: the time axis inside the plot border.
    fn axis_row(screen: &str) -> &str {
        let rows: Vec<&str> = screen.lines().collect();
        rows[rows.len() - 3]
    }

    #[test]
    fn time_axis_always_shows_cursor_time() {
        let mut app = app();
        // 2 kHz: one decimal shows every sample time exactly.
        assert!(axis_row(&render(&app, 110, 30)).contains("▲ 0.0 ms"));

        app.on_key(KeyEvent::new(KeyCode::Right, KeyModifiers::NONE));
        let rec = app.recording.as_ref().unwrap();
        assert!(rec.data.time().contains(&rec.cursor), "cursor snaps to a sample");
        assert!(axis_row(&render(&app, 110, 30)).contains("▲ 2.0 ms"));

        // At the right edge the label flips to the left of the marker.
        let last = *app.recording.as_ref().unwrap().data.time().last().unwrap();
        app.recording.as_mut().unwrap().cursor = last;
        assert!(
            axis_row(&render(&app, 110, 30))
                .trim_end_matches(['│', ' '])
                .ends_with("139.5 ms ▲")
        );

        // Zoomed in, the label stays visible on the narrowest supported terminal.
        for _ in 0..6 {
            app.on_key(KeyEvent::new(KeyCode::Char('+'), KeyModifiers::NONE));
        }
        assert!(axis_row(&render(&app, 80, 24)).contains("139.5 ms ▲"));
    }

    fn press(app: &mut App, code: KeyCode, mods: KeyModifiers) {
        app.on_key(KeyEvent::new(code, mods));
    }

    #[test]
    fn config_overlay_creates_file_and_saves_changes() {
        use crate::config::AppConfig;
        use crate::term::ThemeKind;
        use crate::tui::app::Overlay;
        use crate::tui::keys::{Action, BINDINGS};

        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("sprcmtrd/config.toml");
        let mut app = app_with_config(Some(path.clone()));

        // Ctrl+, opens it (terminals with the kitty keyboard protocol);
        // the missing file is created with the current settings.
        press(&mut app, KeyCode::Char(','), KeyModifiers::CONTROL);
        assert!(matches!(app.overlay, Some(Overlay::Config(_))));
        let saved = AppConfig::load_from(&path).unwrap();
        assert_eq!(saved.theme, Some(ThemeChoice::Dark));
        assert_eq!(saved.keys["quit"], ["q", "ctrl+c"]);
        let screen = render(&app, 100, 30);
        assert!(screen.contains("Configuration"));
        assert!(screen.contains("◀ dark ▶"));
        assert!(screen.contains("zoom_in"));

        // Theme row: Enter switches and saves.
        press(&mut app, KeyCode::Enter, KeyModifiers::NONE);
        assert_eq!(app.theme_kind, ThemeKind::Light);
        assert_eq!(AppConfig::load_from(&path).unwrap().theme, Some(ThemeChoice::Light));

        // Rebind zoom_in to z.
        let row = 1 + BINDINGS.iter().position(|b| b.name == "zoom_in").unwrap();
        for _ in 0..row {
            press(&mut app, KeyCode::Down, KeyModifiers::NONE);
        }
        press(&mut app, KeyCode::Enter, KeyModifiers::NONE);
        assert!(render(&app, 100, 30).contains("press a key"));
        press(&mut app, KeyCode::Char('z'), KeyModifiers::NONE);
        let z = KeyEvent::new(KeyCode::Char('z'), KeyModifiers::NONE);
        assert_eq!(app.keymap.action(&z), Some(Action::ZoomIn));
        assert_eq!(AppConfig::load_from(&path).unwrap().keys["zoom_in"], ["z"]);

        // A key used by another action is refused.
        press(&mut app, KeyCode::Enter, KeyModifiers::NONE);
        press(&mut app, KeyCode::Char('q'), KeyModifiers::NONE);
        assert!(render(&app, 100, 30).contains("already used by 'quit'"));
        assert_eq!(app.keymap.action(&z), Some(Action::ZoomIn));

        // Backspace restores the default.
        press(&mut app, KeyCode::Backspace, KeyModifiers::NONE);
        assert_eq!(AppConfig::load_from(&path).unwrap().keys["zoom_in"], ["+", "="]);

        // Esc closes; the saved file loads on the next start.
        press(&mut app, KeyCode::Esc, KeyModifiers::NONE);
        assert!(app.overlay.is_none());
        let saved = AppConfig::load_from(&path).unwrap();
        crate::tui::keys::KeyMap::new(&saved.keys).unwrap();
    }

    #[test]
    fn config_overlay_fallback_key_without_config_dir() {
        let mut app = app();
        press(&mut app, KeyCode::Char('c'), KeyModifiers::NONE);
        press(&mut app, KeyCode::Enter, KeyModifiers::NONE);
        assert!(render(&app, 100, 30).contains("changes are not saved"));
    }

    #[test]
    fn analog_plots_alternate_background_shades() {
        use ratatui::style::Color;

        use crate::tui::theme::Theme;

        let mut app = app();
        // Colors on (the test app runs with --no-color).
        app.theme = Theme::dark();
        let mut t = Terminal::new(TestBackend::new(110, 30)).unwrap();
        t.draw(|f| draw(f, &app)).unwrap();
        let buf = t.backend().buffer();

        // Background in the middle of the plot, on the row of each channel label.
        let bg_of = |label: &str| {
            let y = (0..buf.area.height)
                .find(|&y| {
                    let row: String = (0..buf.area.width).map(|x| buf[(x, y)].symbol()).collect();
                    row.contains(label)
                })
                .unwrap_or_else(|| panic!("{label} not drawn"));
            (buf[(80, y)].bg, buf[(25, y)].bg)
        };
        let [light, dark] = Theme::dark().plot_shades;
        assert_ne!(light, dark);
        // Plot and gutter share the shade; 1st light, 2nd dark, 3rd light again.
        assert_eq!(bg_of("IA [A]"), (light, light));
        assert_eq!(bg_of("IB [A]"), (dark, dark));
        assert_eq!(bg_of("IC [A]"), (light, light));
        // Without colors there is no shading.
        assert_eq!(Theme::plain().plot_background(0), Color::Reset);
        assert_eq!(Theme::plain().plot_background(1), Color::Reset);
    }

    #[test]
    fn plot_area_scrolls_and_keeps_the_time_axis() {
        let mut app = app();
        // Without the sidebar, channel names on screen are plot labels.
        app.sidebar = false;
        let screen = render(&app, 80, 24);
        if std::env::var_os("SHOW_SCREEN").is_some() {
            println!("{screen}");
        }
        // 4 analog plots and 3 binary strips (one never changes and is
        // hidden) don't fit in 19 rows at 80×24.
        assert!(screen.contains("channels 1–3 of 7"), "{screen}");
        assert!(screen.contains("IA [A]") && !screen.contains("TRIP"));
        assert!(axis_row(&screen).contains("0.0 ms"));

        // One page down; the time axis stays on the bottom row.
        press(&mut app, KeyCode::PageDown, KeyModifiers::NONE);
        let screen = render(&app, 80, 24);
        assert!(screen.contains("channels 2–7 of 7"), "{screen}");
        assert!(screen.contains("TRIP") && !screen.contains("IA [A]"));
        assert!(axis_row(&screen).contains("0.0 ms"));
        press(&mut app, KeyCode::PageUp, KeyModifiers::NONE);
        assert!(render(&app, 80, 24).contains("channels 1–3 of 7"));

        // Selecting a channel in the sidebar scrolls its plot into view.
        // VA, the 4th analog channel, is below the first page.
        app.sidebar = true;
        for _ in 0..3 {
            press(&mut app, KeyCode::Down, KeyModifiers::NONE);
        }
        let screen = render(&app, 80, 24);
        assert!(
            screen.contains("channels 2–7 of 7") && screen.contains("VA [kV]"),
            "{screen}"
        );

        // With room for everything there is no scrolling.
        assert!(!render(&app, 110, 40).contains("channels"));
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
