//! Recording information overlay (same content as the `info` command).

use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::text::Line;

use super::{centered, overlay_block, scrolled_text};
use crate::command::info::{analog_rows, binary_rows, key_values, summary};
use crate::command::table;
use crate::tui::app::App;

pub fn render(f: &mut Frame, area: Rect, app: &App, scroll: u16) {
    let inner = overlay_block(f, centered(area, 85, 85), app, "Recording info");
    let Some(rec) = &app.recording else {
        return;
    };
    let data = &rec.data;
    let c = &data.config;
    let file = data
        .path
        .as_ref()
        .map(|p| p.main_file().display().to_string())
        .unwrap_or_default();

    let mut lines: Vec<Line<'static>> = Vec::new();
    let mut text = |title: Option<&str>, body: String| {
        if let Some(t) = title {
            lines.push(Line::raw(""));
            lines.push(Line::styled(t.to_owned(), app.theme.accent()));
        }
        lines.extend(body.lines().map(|l| Line::raw(l.to_owned())));
    };
    text(None, key_values(&summary(&file, c)));
    if !c.analog.is_empty() {
        let (h, rows) = analog_rows(c);
        text(Some("Analog channels"), table(&h, &rows));
    }
    if !c.binary.is_empty() {
        let (h, rows) = binary_rows(c);
        text(Some("Binary channels"), table(&h, &rows));
    }
    if let Some(h) = data.header.as_ref().filter(|t| !t.trim().is_empty()) {
        text(Some("Header (.hdr)"), h.clone());
    }
    if let Some(i) = data.info.as_ref().filter(|t| !t.trim().is_empty()) {
        text(Some("Information (.inf)"), i.clone());
    }
    scrolled_text(f, inner, app, lines, scroll);
}
