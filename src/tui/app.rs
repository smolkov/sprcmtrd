//! TUI state, actions and key handling.

use std::collections::BTreeMap;
use std::fmt;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, Instant};

use anyhow::{Context as _, bail};
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use regex::RegexBuilder;
use tokio::sync::mpsc::UnboundedSender;

use super::event::Message;
use super::theme::{Theme, ThemeKind};
use crate::args::ThemeChoice;
use crate::command::{self, Context};
use crate::comtrade::{Comtrade, RecordingPath, Side, discover};

/// Something the user can do with a key.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Action {
    Quit,
    Open,
    Info,
    Help,
    Focus,
    Up,
    Down,
    ToggleChannel,
    ToggleAll,
    ZoomIn,
    ZoomOut,
    CursorLeft,
    CursorRight,
    PanLeft,
    PanRight,
    ResetZoom,
    Trigger,
    Side,
    TimeMode,
    Sidebar,
    Theme,
    Close,
}

/// Default key binding of an action.
pub struct Binding {
    /// The action.
    pub action: Action,
    /// Name used in the configuration file.
    pub name: &'static str,
    /// Default keys.
    pub keys: &'static [&'static str],
    /// Description for the help overlay.
    pub help: &'static str,
    /// Short label for the footer, if shown there.
    pub hint: Option<&'static str>,
}

/// All key bindings. Footer hints and the help overlay are generated from
/// this table; the configuration file can replace the keys per action.
pub const BINDINGS: &[Binding] = &[
    Binding { action: Action::Quit, name: "quit", keys: &["q", "ctrl+c"], help: "Quit (closes an open overlay first)", hint: Some("quit") },
    Binding { action: Action::Open, name: "open", keys: &["o", "ctrl+p"], help: "Open file picker", hint: Some("open") },
    Binding { action: Action::Info, name: "info", keys: &["i"], help: "Toggle recording info", hint: Some("info") },
    Binding { action: Action::Help, name: "help", keys: &["?"], help: "Toggle this help", hint: Some("help") },
    Binding { action: Action::Focus, name: "focus", keys: &["tab"], help: "Switch focus sidebar / plot", hint: None },
    Binding { action: Action::Up, name: "up", keys: &["up", "k"], help: "Previous channel / scroll up", hint: None },
    Binding { action: Action::Down, name: "down", keys: &["down", "j"], help: "Next channel / scroll down", hint: None },
    Binding { action: Action::ToggleChannel, name: "toggle_channel", keys: &["space"], help: "Show / hide selected channel", hint: Some("toggle") },
    Binding { action: Action::ToggleAll, name: "toggle_all", keys: &["a"], help: "Show all / hide all channels", hint: None },
    Binding { action: Action::ZoomIn, name: "zoom_in", keys: &["+", "="], help: "Zoom in (time)", hint: Some("zoom") },
    Binding { action: Action::ZoomOut, name: "zoom_out", keys: &["-"], help: "Zoom out (time)", hint: None },
    Binding { action: Action::CursorLeft, name: "cursor_left", keys: &["left", "h"], help: "Move cursor left", hint: Some("cursor") },
    Binding { action: Action::CursorRight, name: "cursor_right", keys: &["right", "l"], help: "Move cursor right", hint: None },
    Binding { action: Action::PanLeft, name: "pan_left", keys: &["shift+left", "H"], help: "Pan view left", hint: Some("pan") },
    Binding { action: Action::PanRight, name: "pan_right", keys: &["shift+right", "L"], help: "Pan view right", hint: None },
    Binding { action: Action::ResetZoom, name: "reset_zoom", keys: &["0"], help: "Reset zoom (whole recording)", hint: None },
    Binding { action: Action::Trigger, name: "trigger", keys: &["t"], help: "Jump cursor to trigger", hint: None },
    Binding { action: Action::Side, name: "side", keys: &["p"], help: "Toggle primary / secondary values", hint: None },
    Binding { action: Action::TimeMode, name: "time_mode", keys: &["m"], help: "Toggle relative / absolute time", hint: None },
    Binding { action: Action::Sidebar, name: "sidebar", keys: &["s"], help: "Show / hide channel sidebar", hint: None },
    Binding { action: Action::Theme, name: "theme", keys: &["T"], help: "Toggle dark / light theme", hint: None },
    Binding { action: Action::Close, name: "close", keys: &["esc"], help: "Close overlay", hint: None },
];

/// A key with modifiers, as written in the configuration (`ctrl+p`, `shift+left`, `T`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct KeySpec {
    code: KeyCode,
    mods: KeyModifiers,
}

impl KeySpec {
    /// Parses a key description.
    pub fn parse(s: &str) -> anyhow::Result<Self> {
        let mut mods = KeyModifiers::NONE;
        let mut rest = s;
        loop {
            let lower = rest.to_ascii_lowercase();
            let (m, len) = if lower.starts_with("ctrl+") {
                (KeyModifiers::CONTROL, 5)
            } else if lower.starts_with("alt+") {
                (KeyModifiers::ALT, 4)
            } else if lower.starts_with("shift+") {
                (KeyModifiers::SHIFT, 6)
            } else {
                break;
            };
            if rest.len() == len {
                break; // a lone "+" after a modifier, e.g. "ctrl++" is not supported
            }
            mods |= m;
            rest = &rest[len..];
        }
        let code = match rest.to_ascii_lowercase().as_str() {
            "space" => KeyCode::Char(' '),
            "tab" => KeyCode::Tab,
            "backtab" => KeyCode::BackTab,
            "enter" => KeyCode::Enter,
            "esc" | "escape" => KeyCode::Esc,
            "backspace" => KeyCode::Backspace,
            "up" => KeyCode::Up,
            "down" => KeyCode::Down,
            "left" => KeyCode::Left,
            "right" => KeyCode::Right,
            "home" => KeyCode::Home,
            "end" => KeyCode::End,
            "pageup" => KeyCode::PageUp,
            "pagedown" => KeyCode::PageDown,
            _ => {
                let mut chars = rest.chars();
                match (chars.next(), chars.next()) {
                    (Some(c), None) => KeyCode::Char(c),
                    _ => bail!("unknown key '{s}'"),
                }
            }
        };
        Ok(Self { code, mods })
    }

    /// Returns `true` when `key` is this key. For characters, Shift is part
    /// of the character itself (`T`, `?`), so it is not compared.
    pub fn matches(&self, key: &KeyEvent) -> bool {
        let relevant = KeyModifiers::CONTROL | KeyModifiers::ALT | KeyModifiers::SHIFT;
        match self.code {
            KeyCode::Char(c) => {
                let ignore_shift = relevant - KeyModifiers::SHIFT;
                key.code == KeyCode::Char(c) && key.modifiers & ignore_shift == self.mods & ignore_shift
            }
            code => key.code == code && key.modifiers & relevant == self.mods,
        }
    }
}

impl fmt::Display for KeySpec {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.mods.contains(KeyModifiers::CONTROL) {
            f.write_str("Ctrl+")?;
        }
        if self.mods.contains(KeyModifiers::ALT) {
            f.write_str("Alt+")?;
        }
        if self.mods.contains(KeyModifiers::SHIFT) {
            f.write_str("Shift+")?;
        }
        match self.code {
            KeyCode::Char(' ') => f.write_str("Space"),
            KeyCode::Char(c) => write!(f, "{c}"),
            KeyCode::Tab => f.write_str("Tab"),
            KeyCode::BackTab => f.write_str("BackTab"),
            KeyCode::Enter => f.write_str("Enter"),
            KeyCode::Esc => f.write_str("Esc"),
            KeyCode::Backspace => f.write_str("Backspace"),
            KeyCode::Up => f.write_str("↑"),
            KeyCode::Down => f.write_str("↓"),
            KeyCode::Left => f.write_str("←"),
            KeyCode::Right => f.write_str("→"),
            KeyCode::Home => f.write_str("Home"),
            KeyCode::End => f.write_str("End"),
            KeyCode::PageUp => f.write_str("PgUp"),
            KeyCode::PageDown => f.write_str("PgDn"),
            _ => f.write_str("?"),
        }
    }
}

/// Active key bindings.
#[derive(Debug, Clone)]
pub struct KeyMap {
    keys: BTreeMap<Action, Vec<KeySpec>>,
}

impl KeyMap {
    /// Default bindings with the overrides from the configuration file.
    pub fn new(overrides: &BTreeMap<String, Vec<String>>) -> anyhow::Result<Self> {
        let mut keys = BTreeMap::new();
        for b in BINDINGS {
            let specs = b.keys.iter().map(|k| KeySpec::parse(k)).collect::<anyhow::Result<_>>()?;
            keys.insert(b.action, specs);
        }
        for (name, list) in overrides {
            let b = BINDINGS
                .iter()
                .find(|b| b.name == name)
                .with_context(|| format!("unknown action '{name}' in [keys]"))?;
            let specs = list
                .iter()
                .map(|k| KeySpec::parse(k))
                .collect::<anyhow::Result<_>>()
                .with_context(|| format!("invalid key for '{name}'"))?;
            keys.insert(b.action, specs);
        }
        Ok(Self { keys })
    }

    /// Action bound to `key`.
    pub fn action(&self, key: &KeyEvent) -> Option<Action> {
        // Non-character keys compare modifiers exactly, so "left" and
        // "shift+left" never both match.
        self.keys
            .iter()
            .find(|(_, specs)| specs.iter().any(|s| s.matches(key)))
            .map(|(a, _)| *a)
    }

    /// Keys bound to `action`.
    pub fn keys(&self, action: Action) -> &[KeySpec] {
        self.keys.get(&action).map_or(&[], Vec::as_slice)
    }

    /// First key bound to `action` as text.
    pub fn label(&self, action: Action) -> String {
        self.keys(action).first().map_or_else(|| "-".into(), ToString::to_string)
    }
}

/// Which pane has the keyboard focus.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Focus {
    /// Channel list.
    Sidebar,
    /// Plot area.
    Plot,
}

/// Kind of a footer status message.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StatusKind {
    /// Information.
    Info,
    /// Error.
    Error,
}

/// A temporary footer message.
#[derive(Debug, Clone)]
pub struct Status {
    /// Message text.
    pub text: String,
    /// Kind of message.
    pub kind: StatusKind,
    /// When the message disappears.
    pub until: Instant,
}

/// A loaded recording and its view state.
#[derive(Debug)]
pub struct Recording {
    /// Recording data.
    pub data: Arc<Comtrade>,
    /// File name shown in the header.
    pub name: String,
    /// Size of the data file in bytes.
    pub size: Option<u64>,
    /// Visibility of each analog channel.
    pub visible_analog: Vec<bool>,
    /// Visibility of each binary channel.
    pub visible_binary: Vec<bool>,
    /// Visible time window (seconds relative to trigger).
    pub view: (f64, f64),
    /// Cursor time.
    pub cursor: f64,
}

/// Default number of analog channels shown after loading.
const DEFAULT_ANALOG: usize = 6;
/// Default number of binary channels shown after loading.
const DEFAULT_BINARY: usize = 8;

impl Recording {
    fn new(data: Comtrade) -> Self {
        let name = data.path.as_ref().map(RecordingPath::name).unwrap_or_default();
        let size = data.path.as_ref().and_then(RecordingPath::data_size);
        let visible_analog = (0..data.analog_channels().len()).map(|i| i < DEFAULT_ANALOG).collect();
        // Binary channels that never change are rarely interesting.
        let mut shown = 0;
        let visible_binary = data
            .binary_channels()
            .map(|b| {
                let show = shown < DEFAULT_BINARY && b.changes();
                shown += usize::from(show);
                show
            })
            .collect();
        let view = data.time_span();
        let cursor = 0.0f64.clamp(view.0, view.1.max(view.0));
        Self {
            data: Arc::new(data),
            name,
            size,
            visible_analog,
            visible_binary,
            view,
            cursor,
        }
    }

    fn n_channels(&self) -> usize {
        self.visible_analog.len() + self.visible_binary.len()
    }

    fn visible_mut(&mut self, i: usize) -> Option<&mut bool> {
        let na = self.visible_analog.len();
        if i < na {
            self.visible_analog.get_mut(i)
        } else {
            self.visible_binary.get_mut(i - na)
        }
    }

    /// Shortest allowed view span.
    fn min_span(&self) -> f64 {
        let t = self.data.time();
        let (a, b) = self.data.time_span();
        if t.len() > 1 {
            ((b - a) / (t.len() - 1) as f64 * 8.0).max(1e-9)
        } else {
            1e-3
        }
    }

    /// Clamps the view to the recording and to the minimum span.
    fn clamp_view(&mut self) {
        let (first, last) = self.data.time_span();
        let full = last - first;
        let mut span = (self.view.1 - self.view.0).max(self.min_span().min(full));
        if span >= full {
            self.view = (first, last);
            return;
        }
        span = span.max(f64::EPSILON);
        let mut start = self.view.0.clamp(first, last - span);
        if start + span > last {
            start = last - span;
        }
        self.view = (start, start + span);
    }

    fn zoom(&mut self, factor: f64) {
        let (a, b) = self.view;
        let span = b - a;
        let new_span = span * factor;
        let anchor = if (a..=b).contains(&self.cursor) {
            self.cursor
        } else {
            (a + b) / 2.0
        };
        let start = anchor - (anchor - a) * factor;
        self.view = (start, start + new_span);
        self.clamp_view();
    }

    fn pan(&mut self, fraction: f64) {
        let shift = (self.view.1 - self.view.0) * fraction;
        self.view = (self.view.0 + shift, self.view.1 + shift);
        self.clamp_view();
        if !(self.view.0..=self.view.1).contains(&self.cursor) {
            self.cursor = self.cursor.clamp(self.view.0, self.view.1);
        }
    }

    fn move_cursor(&mut self, direction: f64) {
        let step = (self.view.1 - self.view.0) / 100.0;
        let t = self.data.time();
        let mut new = self.cursor + direction * step;
        // Always move at least one sample.
        if let (Some(i), Some(j)) = (self.data.index_at(self.cursor), self.data.index_at(new))
            && i == j
        {
            let k = if direction > 0.0 { (i + 1).min(t.len() - 1) } else { i.saturating_sub(1) };
            new = t[k];
        }
        self.set_cursor(new);
    }

    fn set_cursor(&mut self, t: f64) {
        let (first, last) = self.data.time_span();
        self.cursor = t.clamp(first, last);
        let span = self.view.1 - self.view.0;
        if self.cursor < self.view.0 {
            self.view = (self.cursor, self.cursor + span);
        } else if self.cursor > self.view.1 {
            self.view = (self.cursor - span, self.cursor);
        }
        self.clamp_view();
    }
}

/// An entry in the file picker.
#[derive(Debug, Clone)]
pub enum PickerEntry {
    /// A subdirectory.
    Dir {
        /// Directory name.
        name: String,
        /// Full path.
        path: PathBuf,
    },
    /// A recording.
    Recording {
        /// File name.
        name: String,
        /// Path of the main file.
        path: PathBuf,
        /// Station name, start time and channel counts, or the parse error.
        details: Result<(String, String, usize, usize), String>,
    },
}

impl PickerEntry {
    /// Name used for filtering and display.
    pub fn name(&self) -> &str {
        match self {
            Self::Dir { name, .. } | Self::Recording { name, .. } => name,
        }
    }
}

/// File picker state.
#[derive(Debug, Clone)]
pub struct Picker {
    /// Directory being shown.
    pub dir: PathBuf,
    /// Filter text.
    pub query: String,
    /// All entries found so far.
    pub entries: Vec<PickerEntry>,
    /// Indices into `entries` matching the filter.
    pub filtered: Vec<usize>,
    /// Selected position in `filtered`.
    pub selected: usize,
    /// Scan still running.
    pub scanning: bool,
    /// Filter or scan error.
    pub error: Option<String>,
    generation: u64,
}

impl Picker {
    fn refilter(&mut self) {
        let matcher: Box<dyn Fn(&str) -> bool> = if let Some(re) = self.query.strip_prefix('/') {
            match RegexBuilder::new(re).case_insensitive(true).build() {
                Ok(re) => {
                    self.error = None;
                    Box::new(move |s| re.is_match(s))
                }
                Err(e) => {
                    // Keep the previous list while the regex is invalid.
                    self.error = Some(format!("invalid regex: {}", e.to_string().lines().last().unwrap_or("")));
                    return;
                }
            }
        } else {
            self.error = None;
            let q = self.query.to_lowercase();
            Box::new(move |s| s.to_lowercase().contains(&q))
        };
        self.filtered = (0..self.entries.len())
            .filter(|&i| matcher(self.entries[i].name()))
            .collect();
        self.selected = self.selected.min(self.filtered.len().saturating_sub(1));
    }

    /// The selected entry.
    pub fn current(&self) -> Option<&PickerEntry> {
        self.filtered.get(self.selected).map(|&i| &self.entries[i])
    }
}

/// Overlay drawn on top of the main layout.
#[derive(Debug, Clone)]
pub enum Overlay {
    /// Key help.
    Help {
        /// Scroll offset.
        scroll: u16,
    },
    /// Recording information.
    Info {
        /// Scroll offset.
        scroll: u16,
    },
    /// File picker.
    Picker(Picker),
}

/// The whole TUI state.
pub struct App {
    /// Key bindings.
    pub keymap: KeyMap,
    /// Current theme colors.
    pub theme: Theme,
    theme_kind: ThemeKind,
    /// Loaded recording.
    pub recording: Option<Recording>,
    /// File currently loading in the background.
    pub loading: Option<PathBuf>,
    /// Footer message.
    pub status: Option<Status>,
    /// Focused pane.
    pub focus: Focus,
    /// Sidebar shown.
    pub sidebar: bool,
    /// Selected channel (analog first, then binary).
    pub selected: usize,
    /// Side analog values are shown on.
    pub side: Side,
    /// Show absolute instead of trigger-relative time.
    pub absolute_time: bool,
    /// Open overlay.
    pub overlay: Option<Overlay>,
    /// Animation frame counter.
    pub frame: usize,
    /// Set when the app should exit.
    pub quit: bool,
    tx: UnboundedSender<Message>,
    generation: u64,
}

/// How long footer messages stay.
const STATUS_TIME: Duration = Duration::from_secs(5);

impl App {
    /// Creates the app from command line and configuration settings.
    pub fn new(ctx: &Context, tx: UnboundedSender<Message>) -> anyhow::Result<Self> {
        let keymap = KeyMap::new(&ctx.config.keys)?;
        let choice = ctx.global.theme.or(ctx.config.theme).unwrap_or(ThemeChoice::Auto);
        let theme_kind = ThemeKind::resolve(choice);
        Ok(Self {
            keymap,
            theme: Theme::new(theme_kind, ctx.global.colors_disabled()),
            theme_kind,
            recording: None,
            loading: None,
            status: None,
            focus: Focus::Sidebar,
            sidebar: true,
            selected: 0,
            side: if ctx.global.secondary { Side::Secondary } else { Side::Primary },
            absolute_time: false,
            overlay: None,
            frame: 0,
            quit: false,
            tx,
            generation: 0,
        })
    }

    /// Opens `path`: a file is loaded, a directory opens the file picker.
    pub fn open(&mut self, path: &Path) {
        if path.is_dir() {
            self.open_picker(path.to_path_buf());
            return;
        }
        self.loading = Some(path.to_path_buf());
        let tx = self.tx.clone();
        let path = path.to_path_buf();
        tokio::task::spawn_blocking(move || {
            let result = Comtrade::open(&path).map(Box::new).map_err(|e| e.to_string());
            let _ = tx.send(Message::Loaded { path, result });
        });
    }

    /// Opens the file picker in `dir` and starts scanning it.
    pub fn open_picker(&mut self, dir: PathBuf) {
        let dir = std::path::absolute(&dir).unwrap_or(dir);
        self.generation += 1;
        let generation = self.generation;
        let query = match &self.overlay {
            Some(Overlay::Picker(p)) if p.dir == dir => p.query.clone(),
            _ => String::new(),
        };
        self.overlay = Some(Overlay::Picker(Picker {
            dir: dir.clone(),
            query,
            entries: Vec::new(),
            filtered: Vec::new(),
            selected: 0,
            scanning: true,
            error: None,
            generation,
        }));
        let tx = self.tx.clone();
        tokio::task::spawn_blocking(move || scan(dir, generation, tx));
    }

    fn picker_dir(&self) -> PathBuf {
        self.recording
            .as_ref()
            .and_then(|r| r.data.path.as_ref())
            .and_then(|p| p.main_file().parent().map(Path::to_path_buf))
            .filter(|p| !p.as_os_str().is_empty())
            .unwrap_or_else(|| PathBuf::from("."))
    }

    /// Shows `data` as the current recording.
    pub fn set_recording(&mut self, data: Comtrade) {
        let rec = Recording::new(data);
        let size = rec.size.map(command::bytes).unwrap_or_default();
        self.set_status(format!("loaded {} {size}", rec.name), StatusKind::Info);
        self.recording = Some(rec);
        self.selected = 0;
    }

    /// Shows a footer message.
    pub fn set_status(&mut self, text: impl Into<String>, kind: StatusKind) {
        self.status = Some(Status {
            text: text.into(),
            kind,
            until: Instant::now() + STATUS_TIME,
        });
    }

    /// Handles a background task message.
    pub fn on_message(&mut self, msg: Message) {
        match msg {
            Message::Loaded { path, result } => {
                if self.loading.as_ref() != Some(&path) {
                    return; // superseded by a newer request
                }
                self.loading = None;
                match result {
                    Ok(data) => self.set_recording(*data),
                    Err(e) => {
                        tracing::warn!("loading {} failed: {e}", path.display());
                        self.set_status(e, StatusKind::Error);
                    }
                }
            }
            Message::ScanEntry { generation, entry } => {
                if let Some(Overlay::Picker(p)) = &mut self.overlay
                    && p.generation == generation
                {
                    p.entries.push(entry);
                    p.refilter();
                }
            }
            Message::ScanDone { generation, error } => {
                if let Some(Overlay::Picker(p)) = &mut self.overlay
                    && p.generation == generation
                {
                    p.scanning = false;
                    if error.is_some() {
                        p.error = error;
                    }
                }
            }
        }
    }

    /// Called on every tick. Returns `true` when a redraw is needed.
    pub fn on_tick(&mut self) -> bool {
        let mut redraw = false;
        if self.status.as_ref().is_some_and(|s| Instant::now() >= s.until) {
            self.status = None;
            redraw = true;
        }
        let scanning = matches!(&self.overlay, Some(Overlay::Picker(p)) if p.scanning);
        if self.loading.is_some() || scanning {
            self.frame = self.frame.wrapping_add(1);
            redraw = true;
        }
        redraw
    }

    /// Handles a key press.
    pub fn on_key(&mut self, key: KeyEvent) {
        let ctrl_c = key.code == KeyCode::Char('c') && key.modifiers.contains(KeyModifiers::CONTROL);
        if ctrl_c {
            self.quit = true;
            return;
        }
        if matches!(self.overlay, Some(Overlay::Picker(_))) {
            self.picker_key(key);
            return;
        }
        let Some(action) = self.keymap.action(&key) else {
            return;
        };
        if self.overlay.is_some() {
            self.overlay_action(action);
        } else {
            self.action(action);
        }
    }

    fn overlay_action(&mut self, action: Action) {
        let scroll = match &mut self.overlay {
            Some(Overlay::Help { scroll } | Overlay::Info { scroll }) => scroll,
            _ => return,
        };
        match action {
            Action::Up => *scroll = scroll.saturating_sub(1),
            Action::Down => *scroll = scroll.saturating_add(1),
            Action::Quit | Action::Close => self.overlay = None,
            Action::Help if matches!(self.overlay, Some(Overlay::Help { .. })) => self.overlay = None,
            Action::Info if matches!(self.overlay, Some(Overlay::Info { .. })) => self.overlay = None,
            Action::Help => self.overlay = Some(Overlay::Help { scroll: 0 }),
            Action::Info => self.overlay = Some(Overlay::Info { scroll: 0 }),
            Action::Open => self.open_picker(self.picker_dir()),
            _ => {}
        }
    }

    fn action(&mut self, action: Action) {
        match action {
            Action::Quit => self.quit = true,
            Action::Open => self.open_picker(self.picker_dir()),
            Action::Help => self.overlay = Some(Overlay::Help { scroll: 0 }),
            Action::Info => {
                if self.recording.is_some() {
                    self.overlay = Some(Overlay::Info { scroll: 0 });
                } else {
                    self.no_recording();
                }
            }
            Action::Close => {}
            Action::Focus => {
                self.focus = match self.focus {
                    Focus::Sidebar => Focus::Plot,
                    Focus::Plot => Focus::Sidebar,
                };
                if self.focus == Focus::Sidebar {
                    self.sidebar = true;
                }
            }
            Action::Sidebar => {
                self.sidebar = !self.sidebar;
                if !self.sidebar {
                    self.focus = Focus::Plot;
                }
            }
            Action::Theme => {
                if self.theme.plain {
                    self.set_status("colors are disabled", StatusKind::Info);
                } else {
                    self.theme_kind = self.theme_kind.toggled();
                    self.theme = Theme::new(self.theme_kind, false);
                }
            }
            Action::Side => {
                self.side = match self.side {
                    Side::Primary => Side::Secondary,
                    Side::Secondary => Side::Primary,
                };
                let name = format!("{:?}", self.side).to_lowercase();
                self.set_status(format!("showing {name} values"), StatusKind::Info);
            }
            Action::TimeMode => self.absolute_time = !self.absolute_time,
            _ => self.recording_action(action),
        }
    }

    fn recording_action(&mut self, action: Action) {
        let focus = self.focus;
        let Some(rec) = &mut self.recording else {
            self.no_recording();
            return;
        };
        let n = rec.n_channels();
        match action {
            Action::Up if focus == Focus::Sidebar => self.selected = self.selected.saturating_sub(1),
            Action::Down if focus == Focus::Sidebar => {
                self.selected = (self.selected + 1).min(n.saturating_sub(1));
            }
            // In the plot, up/down zoom like +/-.
            Action::Up => rec.zoom(0.5),
            Action::Down => rec.zoom(2.0),
            Action::ToggleChannel => {
                if let Some(v) = rec.visible_mut(self.selected) {
                    *v = !*v;
                }
            }
            Action::ToggleAll => {
                let any = rec.visible_analog.iter().chain(&rec.visible_binary).any(|&v| v);
                rec.visible_analog.iter_mut().chain(&mut rec.visible_binary).for_each(|v| *v = !any);
            }
            Action::ZoomIn => rec.zoom(0.5),
            Action::ZoomOut => rec.zoom(2.0),
            Action::CursorLeft => rec.move_cursor(-1.0),
            Action::CursorRight => rec.move_cursor(1.0),
            Action::PanLeft => rec.pan(-0.25),
            Action::PanRight => rec.pan(0.25),
            Action::ResetZoom => {
                rec.view = rec.data.time_span();
            }
            Action::Trigger => rec.set_cursor(0.0),
            _ => {}
        }
    }

    fn no_recording(&mut self) {
        let key = self.keymap.label(Action::Open);
        self.set_status(format!("no recording loaded, press {key} to open one"), StatusKind::Info);
    }

    fn picker_key(&mut self, key: KeyEvent) {
        let Some(Overlay::Picker(p)) = &mut self.overlay else {
            return;
        };
        let typing = !key.modifiers.intersects(KeyModifiers::CONTROL | KeyModifiers::ALT);
        match key.code {
            KeyCode::Esc => {
                self.overlay = None;
            }
            KeyCode::Up => p.selected = p.selected.saturating_sub(1),
            KeyCode::Down => p.selected = (p.selected + 1).min(p.filtered.len().saturating_sub(1)),
            KeyCode::PageUp => p.selected = p.selected.saturating_sub(10),
            KeyCode::PageDown => p.selected = (p.selected + 10).min(p.filtered.len().saturating_sub(1)),
            KeyCode::Enter => match p.current().cloned() {
                Some(PickerEntry::Dir { path, .. }) => {
                    p.query.clear();
                    self.open_picker(path);
                }
                Some(PickerEntry::Recording { path, .. }) => {
                    self.overlay = None;
                    self.open(&path);
                }
                None => {}
            },
            KeyCode::Backspace if p.query.is_empty() => {
                if let Some(parent) = p.dir.parent().map(Path::to_path_buf) {
                    self.open_picker(parent);
                }
            }
            KeyCode::Backspace => {
                p.query.pop();
                p.refilter();
            }
            KeyCode::Char(c) if typing => {
                p.query.push(c);
                p.selected = 0;
                p.refilter();
            }
            _ => {
                // Allow closing the picker with its own key (e.g. Ctrl+p).
                if self.keymap.action(&key) == Some(Action::Open) {
                    self.overlay = None;
                }
            }
        }
    }
}

/// Scans `dir` for subdirectories and recordings, sending entries as found.
fn scan(dir: PathBuf, generation: u64, tx: UnboundedSender<Message>) {
    let send = |entry| tx.send(Message::ScanEntry { generation, entry }).is_ok();
    let mut dirs: Vec<PathBuf> = std::fs::read_dir(&dir)
        .map(|rd| {
            rd.flatten()
                .map(|e| e.path())
                .filter(|p| p.is_dir())
                .filter(|p| !p.file_name().is_some_and(|n| n.to_string_lossy().starts_with('.')))
                .collect()
        })
        .unwrap_or_default();
    dirs.sort();
    for path in dirs {
        let name = format!("{}/", path.file_name().unwrap_or_default().to_string_lossy());
        if !send(PickerEntry::Dir { name, path }) {
            return;
        }
    }
    let mut alive = true;
    let result = discover::for_each_recording(&dir, false, &mut |rec| {
        if !alive {
            return;
        }
        let details = Comtrade::config_of(&rec)
            .map(|c| {
                (
                    c.station_name,
                    c.start_time.format("%Y-%m-%d %H:%M:%S").to_string(),
                    c.analog.len(),
                    c.binary.len(),
                )
            })
            .map_err(|e| e.to_string());
        alive = send(PickerEntry::Recording {
            name: rec.name(),
            path: rec.main_file().to_path_buf(),
            details,
        });
    });
    let _ = tx.send(Message::ScanDone {
        generation,
        error: result.err().map(|e| e.to_string()),
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    fn key(code: KeyCode, mods: KeyModifiers) -> KeyEvent {
        KeyEvent::new(code, mods)
    }

    #[test]
    fn key_specs_parse_and_match() {
        let k = KeySpec::parse("ctrl+p").unwrap();
        assert!(k.matches(&key(KeyCode::Char('p'), KeyModifiers::CONTROL)));
        assert!(!k.matches(&key(KeyCode::Char('p'), KeyModifiers::NONE)));
        let t = KeySpec::parse("T").unwrap();
        assert!(t.matches(&key(KeyCode::Char('T'), KeyModifiers::SHIFT)));
        assert!(!t.matches(&key(KeyCode::Char('t'), KeyModifiers::NONE)));
        let sl = KeySpec::parse("shift+left").unwrap();
        assert!(sl.matches(&key(KeyCode::Left, KeyModifiers::SHIFT)));
        assert!(!sl.matches(&key(KeyCode::Left, KeyModifiers::NONE)));
        assert_eq!(sl.to_string(), "Shift+←");
        assert!(KeySpec::parse("nope").is_err());
    }

    #[test]
    fn keymap_defaults_and_overrides() {
        let km = KeyMap::new(&BTreeMap::new()).unwrap();
        assert_eq!(km.action(&key(KeyCode::Left, KeyModifiers::SHIFT)), Some(Action::PanLeft));
        assert_eq!(km.action(&key(KeyCode::Left, KeyModifiers::NONE)), Some(Action::CursorLeft));
        assert_eq!(km.action(&key(KeyCode::Char('?'), KeyModifiers::SHIFT)), Some(Action::Help));

        let mut o = BTreeMap::new();
        o.insert("quit".to_owned(), vec!["x".to_owned()]);
        let km = KeyMap::new(&o).unwrap();
        assert_eq!(km.action(&key(KeyCode::Char('x'), KeyModifiers::NONE)), Some(Action::Quit));
        assert_eq!(km.action(&key(KeyCode::Char('q'), KeyModifiers::NONE)), None);

        o.insert("fly".to_owned(), vec!["f".to_owned()]);
        assert!(KeyMap::new(&o).is_err());
    }

    #[test]
    fn binding_names_are_unique() {
        let mut names: Vec<_> = BINDINGS.iter().map(|b| b.name).collect();
        names.sort();
        names.dedup();
        assert_eq!(names.len(), BINDINGS.len());
    }
}
