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
    app.overlay_scroll_max.set(max);
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
    fn jump_keys_move_the_channel_selection() {
        let mut app = app();
        let last = app.recording.as_ref().unwrap().n_channels() - 1;
        let n_analog = app.recording.as_ref().unwrap().visible_analog.len();
        assert!(last > n_analog, "the fixture has binary channels");

        // Fewer than 10 channels: a jump stops at the end, past the separator.
        press(&mut app, KeyCode::Down, KeyModifiers::CONTROL);
        assert_eq!(app.selected, last);
        press(&mut app, KeyCode::Up, KeyModifiers::CONTROL);
        assert_eq!(app.selected, 0);
        press(&mut app, KeyCode::Char('J'), KeyModifiers::SHIFT);
        assert_eq!(app.selected, last);
        press(&mut app, KeyCode::Char('K'), KeyModifiers::SHIFT);
        assert_eq!(app.selected, 0);
        press(&mut app, KeyCode::End, KeyModifiers::NONE);
        assert_eq!(app.selected, last);
        press(&mut app, KeyCode::Home, KeyModifiers::NONE);
        assert_eq!(app.selected, 0);
    }

    #[test]
    fn jump_keys_scroll_the_selected_plot_into_view() {
        let mut app = app();
        assert!(render(&app, 80, 24).contains("channels 1–3 of 7"));
        press(&mut app, KeyCode::PageDown, KeyModifiers::NONE);
        press(&mut app, KeyCode::PageUp, KeyModifiers::NONE);
        assert!(!app.plot_follow.get());

        // The last channel is hidden by default, so there is no plot to follow.
        press(&mut app, KeyCode::End, KeyModifiers::NONE);
        assert!(app.plot_follow.get());
        assert!(render(&app, 80, 24).contains("channels 1–3 of 7"));

        // Shown, End scrolls to it and Home back.
        press(&mut app, KeyCode::Char(' '), KeyModifiers::NONE);
        press(&mut app, KeyCode::Home, KeyModifiers::NONE);
        assert!(render(&app, 80, 24).contains("channels 1–3 of 8"));
        press(&mut app, KeyCode::End, KeyModifiers::NONE);
        assert!(render(&app, 80, 24).contains("–8 of 8"));
        press(&mut app, KeyCode::Home, KeyModifiers::NONE);
        assert!(render(&app, 80, 24).contains("channels 1–3 of 8"));
    }

    #[test]
    fn jump_keys_do_nothing_with_the_plot_focused() {
        let mut app = app();
        press(&mut app, KeyCode::Tab, KeyModifiers::NONE);
        press(&mut app, KeyCode::Down, KeyModifiers::NONE);
        press(&mut app, KeyCode::Down, KeyModifiers::NONE);
        let view = app.recording.as_ref().unwrap().view;
        for (code, mods) in [
            (KeyCode::Down, KeyModifiers::CONTROL),
            (KeyCode::Up, KeyModifiers::CONTROL),
            (KeyCode::Char('J'), KeyModifiers::SHIFT),
            (KeyCode::End, KeyModifiers::NONE),
            (KeyCode::Home, KeyModifiers::NONE),
        ] {
            press(&mut app, code, mods);
            assert_eq!(app.selected, 0);
            assert_eq!(app.recording.as_ref().unwrap().view, view);
        }
    }

    /// Scroll offset of the open text overlay.
    fn overlay_scroll(app: &App) -> u16 {
        use crate::tui::app::Overlay;

        match app.overlay {
            Some(Overlay::Help { scroll } | Overlay::Info { scroll }) => scroll,
            _ => panic!("no text overlay open"),
        }
    }

    #[test]
    fn overlay_scroll_keys_stay_in_range() {
        // Wide, so no help line wraps: `scrolled_text` counts lines before wrapping.
        let (w, h) = (140, 24);
        let mut app = app();
        press(&mut app, KeyCode::Char('?'), KeyModifiers::SHIFT);
        render(&app, w, h);
        let max = app.overlay_scroll_max.get();
        assert!(max > 11, "the help must not fit: {max}");

        press(&mut app, KeyCode::Down, KeyModifiers::CONTROL);
        assert_eq!(overlay_scroll(&app), 10);
        press(&mut app, KeyCode::End, KeyModifiers::NONE);
        assert_eq!(overlay_scroll(&app), max);
        assert!(render(&app, w, h).contains("config.toml"));

        // At the end, further keys stay there and one ↑ moves at once.
        press(&mut app, KeyCode::Down, KeyModifiers::NONE);
        press(&mut app, KeyCode::Down, KeyModifiers::CONTROL);
        assert_eq!(overlay_scroll(&app), max);
        press(&mut app, KeyCode::Up, KeyModifiers::NONE);
        assert_eq!(overlay_scroll(&app), max - 1);

        press(&mut app, KeyCode::Home, KeyModifiers::NONE);
        assert_eq!(overlay_scroll(&app), 0);
        assert!(render(&app, w, h).contains("Quit"));
        press(&mut app, KeyCode::Up, KeyModifiers::CONTROL);
        assert_eq!(overlay_scroll(&app), 0);
    }

    /// App with an open file picker listing `n` directories.
    fn app_with_picker(n: usize) -> App {
        use crate::tui::app::Overlay;
        use crate::tui::picker::{Picker, PickerEntry};

        let mut app = app();
        let mut picker = Picker::new(".".into(), String::new(), true, false, 0);
        for i in 0..n {
            picker.push(PickerEntry::Dir {
                name: format!("dir{i}"),
                path: format!("dir{i}").into(),
            });
        }
        app.overlay = Some(Overlay::Picker(picker));
        app
    }

    fn picker_selected(app: &App) -> usize {
        use crate::tui::app::Overlay;

        match &app.overlay {
            Some(Overlay::Picker(p)) => p.selected,
            _ => panic!("no picker open"),
        }
    }

    #[test]
    fn picker_jump_keys_move_the_selection() {
        let mut app = app_with_picker(25);
        for (code, mods, expected) in [
            (KeyCode::Down, KeyModifiers::CONTROL, 10),
            (KeyCode::PageDown, KeyModifiers::NONE, 20),
            (KeyCode::Down, KeyModifiers::CONTROL, 24),
            (KeyCode::Up, KeyModifiers::CONTROL, 14),
            (KeyCode::PageUp, KeyModifiers::NONE, 4),
            (KeyCode::Up, KeyModifiers::CONTROL, 0),
            (KeyCode::End, KeyModifiers::NONE, 24),
            (KeyCode::Up, KeyModifiers::NONE, 23),
            (KeyCode::Home, KeyModifiers::NONE, 0),
            (KeyCode::Down, KeyModifiers::NONE, 1),
        ] {
            press(&mut app, code, mods);
            assert_eq!(picker_selected(&app), expected, "{code:?} {mods:?}");
        }
    }

    #[test]
    fn picker_jump_keys_handle_an_empty_list_and_typing() {
        use crate::tui::app::Overlay;

        let mut app = app_with_picker(0);
        for (code, mods) in [
            (KeyCode::Down, KeyModifiers::CONTROL),
            (KeyCode::PageDown, KeyModifiers::NONE),
            (KeyCode::End, KeyModifiers::NONE),
            (KeyCode::Home, KeyModifiers::NONE),
        ] {
            press(&mut app, code, mods);
            assert_eq!(picker_selected(&app), 0);
        }

        // J types into the filter instead of jumping.
        press(&mut app, KeyCode::Char('J'), KeyModifiers::SHIFT);
        let Some(Overlay::Picker(p)) = &app.overlay else {
            panic!("no picker open");
        };
        assert_eq!(p.query, "J");
    }

    /// App with one channel: 100 A peak at 50 Hz with `dc` A of DC, 2 kHz,
    /// trigger after 100 ms, line frequency `line` Hz.
    fn app_with_sine(dc: f64, line: f64) -> App {
        let mut cfg = String::from("S,D,1999\n2,1A,1D\n1,IA,A,LINE,A,0.001,0,0,-1e9,1e9,1,1,P\n1,TRIP,,LINE,0\n");
        cfg += &format!("{line}\n1\n2000,800\n01/01/2020,00:00:00.000000\n01/01/2020,00:00:00.100000\nASCII\n1\n");
        let mut dat = String::new();
        for k in 0..800 {
            let t = k as f64 / 2000.0;
            let v = dc + 100.0 * (2.0 * std::f64::consts::PI * 50.0 * t).sin();
            dat += &format!(
                "{},{},{},{}\n",
                k + 1,
                (t * 1e6).round(),
                (v * 1000.0).round(),
                u8::from(k >= 400)
            );
        }
        let mut app = app();
        app.set_recording(Comtrade::from_readers(cfg.as_bytes(), dat.as_bytes()).unwrap());
        app
    }

    #[test]
    fn rms_view_keys_change_the_plot() {
        let mut app = app_with_sine(10.0, 50.0);
        let waveform = render(&app, 110, 30);
        assert!(waveform.contains(" Plot "), "{waveform}");

        press(&mut app, KeyCode::Char('r'), KeyModifiers::NONE);
        let with_dc = render(&app, 110, 30);
        assert!(with_dc.contains(" RMS "), "{with_dc}");
        // sqrt(100² / 2 + 10²).
        assert!(with_dc.contains("▸ 71.41"), "{with_dc}");
        assert!(with_dc.contains("RMS with DC, 1 cycle (fitted to IA)"), "{with_dc}");
        assert!(with_dc.contains("↓ 0"), "{with_dc}");

        press(&mut app, KeyCode::Char('d'), KeyModifiers::NONE);
        let without_dc = render(&app, 110, 30);
        assert!(without_dc.contains(" RMS-DC "), "{without_dc}");
        assert!(without_dc.contains("▸ 70.71"), "{without_dc}");
        assert!(without_dc.contains("RMS without DC"), "{without_dc}");

        // The time axis and the binary strip stay the same.
        let strip = |screen: &str| {
            let plots = screen.lines().filter_map(|r| r.split("││").nth(1));
            plots.into_iter().find(|r| r.starts_with("TRIP")).unwrap().to_owned()
        };
        assert_eq!(strip(&waveform), strip(&with_dc));
        assert_eq!(axis_row(&waveform), axis_row(&with_dc));

        // Off again: exactly the waveform, apart from the footer message.
        press(&mut app, KeyCode::Char('r'), KeyModifiers::NONE);
        app.status = None;
        let again = render(&app, 110, 30);
        let body = |s: &str| s.lines().rev().skip(1).map(str::to_owned).collect::<Vec<_>>();
        assert_eq!(body(&again), body(&waveform));
    }

    #[test]
    fn rms_view_without_a_reference_uses_the_nominal_window() {
        // No voltage or current to measure: a unit neither of them.
        let mut cfg = String::from("S,D,1999\n1,1A,0D\n1,P1,,,bar,0.001,0,0,-1e9,1e9,1,1,P\n");
        cfg += "50\n1\n1000,100\n01/01/2020,00:00:00.000000\n01/01/2020,00:00:00.000000\nASCII\n1\n";
        let dat: String = (0..100).map(|k| format!("{},{},5000\n", k + 1, k * 1000)).collect();
        let mut app = app();
        app.set_recording(Comtrade::from_readers(cfg.as_bytes(), dat.as_bytes()).unwrap());
        press(&mut app, KeyCode::Char('r'), KeyModifiers::NONE);
        assert!(render(&app, 110, 30).contains("RMS with DC, 1 cycle (20.00 ms)"));
    }

    #[test]
    fn rms_view_title_reads_at_the_smallest_size() {
        let mut app = app();
        press(&mut app, KeyCode::Char('r'), KeyModifiers::NONE);
        press(&mut app, KeyCode::Char('d'), KeyModifiers::NONE);
        let screen = render(&app, 80, 24);
        assert!(screen.contains("RMS-DC · channels 1–3 of 7"), "{screen}");
        assert!(screen.contains("window 199.5 ms"), "{screen}");
    }

    #[test]
    fn rms_view_needs_a_line_frequency() {
        use crate::tui::app::PlotView;

        let mut app = app_with_sine(0.0, 0.0);
        press(&mut app, KeyCode::Char('r'), KeyModifiers::NONE);
        assert_eq!(app.plot_view, PlotView::Waveform);
        assert!(render(&app, 110, 30).contains("no line frequency"));

        // Opening such a recording in the RMS view goes back to the waveform.
        let mut app = app_with_sine(0.0, 50.0);
        press(&mut app, KeyCode::Char('r'), KeyModifiers::NONE);
        assert_eq!(app.plot_view, PlotView::Rms);
        let other = app_with_sine(0.0, 0.0).recording.take().unwrap();
        let data = std::sync::Arc::into_inner(other.data).unwrap();
        app.set_recording(data);
        assert_eq!(app.plot_view, PlotView::Waveform);
        assert!(render(&app, 110, 30).contains("no line frequency"));
    }

    #[test]
    fn rms_view_stays_when_opening_another_recording() {
        use crate::tui::app::PlotView;

        let mut app = app_with_sine(0.0, 50.0);
        press(&mut app, KeyCode::Char('r'), KeyModifiers::NONE);
        let path = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/data/fault_1999.cfg");
        app.set_recording(Comtrade::open(path).unwrap());
        assert_eq!(app.plot_view, PlotView::Rms);
    }

    #[test]
    fn rms_key_without_a_recording_says_so() {
        let mut app = app();
        app.recording = None;
        press(&mut app, KeyCode::Char('r'), KeyModifiers::NONE);
        assert!(app.status.as_ref().unwrap().text.contains("no recording loaded"));
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
