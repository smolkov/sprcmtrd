//! Sidebar with the channel list.

use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::Style;
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, List, ListItem, ListState};

use crate::tui::app::{App, Focus};
use crate::tui::recording::Recording;

/// Preferred sidebar width for `rec`.
pub fn width(rec: &Recording) -> u16 {
    let longest = rec
        .data
        .analog_channels()
        .map(|a| a.info.id.chars().count() + a.info.unit.chars().count() + 1)
        .chain(rec.data.binary_channels().map(|b| b.info.id.chars().count()))
        .max()
        .unwrap_or(0);
    (longest as u16 + 6).clamp(18, 32)
}

pub fn render(f: &mut Frame, area: Rect, app: &App) {
    let Some(rec) = &app.recording else {
        return;
    };
    let theme = &app.theme;
    let focused = app.focus == Focus::Sidebar;
    let block = Block::bordered()
        .title(" Channels ")
        .title_style(if focused { theme.accent() } else { theme.base() })
        .border_style(theme.border(focused));
    let inner_width = area.width.saturating_sub(2) as usize;

    let mark = |visible: bool| if visible { "● " } else { "○ " };
    let mut items: Vec<ListItem> = Vec::new();
    for (i, ch) in rec.data.analog_channels().enumerate() {
        let visible = rec.visible_analog[i];
        let color = Style::new().fg(theme.channel(&ch.info.phase, i));
        let style = if visible { color } else { theme.muted() };
        let id_width = inner_width.saturating_sub(3 + ch.info.unit.chars().count());
        items.push(ListItem::new(Line::from(vec![
            Span::styled(mark(visible), style),
            Span::styled(format!("{:<id_width$.id_width$}", ch.info.id), style),
            Span::styled(format!(" {}", ch.info.unit), theme.muted()),
        ])));
    }
    let separator = !rec.visible_binary.is_empty() && !rec.visible_analog.is_empty();
    if separator {
        items.push(ListItem::new(Line::styled(
            format!("{:─^inner_width$}", " binary "),
            theme.muted(),
        )));
    }
    for (i, ch) in rec.data.binary_channels().enumerate() {
        let visible = rec.visible_binary[i];
        let style = if visible {
            Style::new().fg(theme.channel("", i + 4))
        } else {
            theme.muted()
        };
        items.push(ListItem::new(Line::from(vec![
            Span::styled(mark(visible), style),
            Span::styled(ch.info.id.clone(), style),
        ])));
    }

    // Map the channel selection to the list row (skipping the separator).
    let n_analog = rec.visible_analog.len();
    let row = if separator && app.selected >= n_analog {
        app.selected + 1
    } else {
        app.selected
    };
    let mut state = ListState::default().with_selected(Some(row));
    let list = List::new(items)
        .block(block)
        .highlight_style(if focused { theme.selection() } else { Style::new() });
    f.render_stateful_widget(list, area, &mut state);
}
