//! TUI state, actions and key handling.

use std::cell::Cell;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant};

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use tokio::sync::mpsc::UnboundedSender;

use super::event::Message;
use super::keys::{Action, BINDINGS, Binding, KeyMap, KeySpec};
use super::picker::{Picker, PickerEntry, scan};
use super::recording::Recording;
use super::theme::Theme;
use crate::args::ThemeChoice;
use crate::comtrade::{Comtrade, Side};
use crate::config::AppConfig;
use crate::context::Context;
use crate::format;
use crate::term::ThemeKind;

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
    /// Configuration (theme and key bindings).
    Config(ConfigState),
}

/// Configuration overlay state.
///
/// Rows: 0 is the theme, row `i + 1` is `BINDINGS[i]`.
#[derive(Debug, Clone, Default)]
pub struct ConfigState {
    /// Selected row.
    pub selected: usize,
    /// Waiting for the new key of the selected binding.
    pub capturing: bool,
    /// Result of the last change (saved, conflict, error).
    pub message: Option<(String, StatusKind)>,
}

impl ConfigState {
    /// Number of rows.
    pub const ROWS: usize = BINDINGS.len() + 1;

    /// Binding of the selected row, `None` on the theme row.
    pub fn binding(&self) -> Option<&'static Binding> {
        self.selected.checked_sub(1).and_then(|i| BINDINGS.get(i))
    }
}

/// The whole TUI state.
pub struct App {
    /// Key bindings.
    pub keymap: KeyMap,
    /// Current theme colors.
    pub theme: Theme,
    /// Dark or light.
    pub theme_kind: ThemeKind,
    /// Where configuration changes are saved.
    pub config_path: Option<PathBuf>,
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
    /// First channel row drawn in the plot area (index into the shown
    /// channels). The plot widget clamps it to the area when drawing.
    pub plot_scroll: Cell<usize>,
    /// Number of channel rows drawn at the last draw: the page size.
    pub plot_page: Cell<usize>,
    /// The plot area scrolls the selected channel into view; cleared by
    /// scrolling with the keys, so the view stays where the user put it.
    pub plot_follow: Cell<bool>,
    /// Show absolute instead of trigger-relative time.
    pub absolute_time: bool,
    /// Open overlay.
    pub overlay: Option<Overlay>,
    /// Animation frame counter.
    pub frame: usize,
    /// Set when the app should exit.
    pub quit: bool,
    tx: UnboundedSender<Message>,
    /// Picker searches subdirectories (kept when changing directories).
    picker_recursive: bool,
    /// Picker lists only valid recordings (kept when changing directories).
    picker_valid_only: bool,
    /// Generation of the current directory scan; older scans stop early.
    scan_generation: Arc<AtomicU64>,
}

/// How long footer messages stay.
const STATUS_TIME: Duration = Duration::from_secs(5);

impl App {
    /// Creates the app from command line and configuration settings.
    pub fn new(ctx: &Context, tx: UnboundedSender<Message>) -> anyhow::Result<Self> {
        let keymap = KeyMap::new(&ctx.config.keys)?;
        let choice = ctx.theme();
        let theme_kind = ThemeKind::resolve(choice);
        Ok(Self {
            keymap,
            theme: Theme::new(theme_kind, ctx.global.colors_disabled()),
            theme_kind,
            config_path: ctx.config_path.clone(),
            recording: None,
            loading: None,
            status: None,
            focus: Focus::Sidebar,
            sidebar: true,
            selected: 0,
            side: if ctx.global.secondary {
                Side::Secondary
            } else {
                Side::Primary
            },
            plot_scroll: Cell::new(0),
            plot_page: Cell::new(0),
            plot_follow: Cell::new(true),
            absolute_time: false,
            overlay: None,
            frame: 0,
            quit: false,
            tx,
            picker_recursive: true,
            picker_valid_only: false,
            scan_generation: Arc::new(AtomicU64::new(0)),
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
        let generation = self.scan_generation.fetch_add(1, Ordering::Relaxed) + 1;
        let query = match &self.overlay {
            Some(Overlay::Picker(p)) if p.dir == dir => p.query.clone(),
            _ => String::new(),
        };
        let (recursive, valid_only) = (self.picker_recursive, self.picker_valid_only);
        self.overlay = Some(Overlay::Picker(Picker::new(
            dir.clone(),
            query,
            recursive,
            valid_only,
            generation,
        )));
        let tx = self.tx.clone();
        let current = Arc::clone(&self.scan_generation);
        tokio::task::spawn_blocking(move || scan(&dir, recursive, valid_only, generation, &current, &tx));
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
        let size = rec.size.map(format::bytes).unwrap_or_default();
        self.set_status(format!("loaded {} {size}", rec.name), StatusKind::Info);
        self.recording = Some(rec);
        self.selected = 0;
        self.plot_scroll.set(0);
        self.plot_follow.set(true);
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
                    p.push(entry);
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
        if matches!(self.overlay, Some(Overlay::Config(_))) {
            self.config_key(key);
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
            Action::Config => self.open_config(),
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
            Action::Config => self.open_config(),
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
            Action::Up if focus == Focus::Sidebar => {
                self.selected = self.selected.saturating_sub(1);
                self.plot_follow.set(true);
            }
            Action::Down if focus == Focus::Sidebar => {
                self.selected = (self.selected + 1).min(n.saturating_sub(1));
                self.plot_follow.set(true);
            }
            Action::ScrollUp | Action::ScrollDown => {
                // A page is what the last draw showed; the widget clamps the end.
                let page = self.plot_page.get().max(1);
                let scroll = self.plot_scroll.get();
                self.plot_scroll.set(if action == Action::ScrollUp {
                    scroll.saturating_sub(page)
                } else {
                    scroll + page
                });
                self.plot_follow.set(false);
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
                rec.visible_analog
                    .iter_mut()
                    .chain(&mut rec.visible_binary)
                    .for_each(|v| *v = !any);
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
        self.set_status(
            format!("no recording loaded, press {key} to open one"),
            StatusKind::Info,
        );
    }

    /// Opens the configuration overlay. A missing configuration file is
    /// created with the current (default) settings.
    pub fn open_config(&mut self) {
        let mut state = ConfigState::default();
        if let Some(path) = self.config_path.as_ref().filter(|p| !p.exists()) {
            state.message = Some(match self.save_config() {
                Ok(()) => (format!("created {}", path.display()), StatusKind::Info),
                Err(e) => (e, StatusKind::Error),
            });
        }
        self.overlay = Some(Overlay::Config(state));
    }

    /// Writes theme and key bindings to the configuration file.
    fn save_config(&self) -> Result<(), String> {
        let path = self
            .config_path
            .as_ref()
            .ok_or("no configuration directory, changes are not saved")?;
        let config = AppConfig {
            theme: Some(match self.theme_kind {
                ThemeKind::Dark => ThemeChoice::Dark,
                ThemeKind::Light => ThemeChoice::Light,
            }),
            keys: self.keymap.to_config(),
        };
        config.save_to(path).map_err(|e| format!("{e:#}"))
    }

    fn config_key(&mut self, key: KeyEvent) {
        let Some(Overlay::Config(state)) = &mut self.overlay else {
            return;
        };
        let mut state = std::mem::take(state);
        let mut changed = false;

        if state.capturing {
            state.capturing = false;
            let binding = state.binding().expect("capturing only on binding rows");
            state.message = if key.code == KeyCode::Esc {
                None
            } else if let Some(spec) = KeySpec::from_event(&key) {
                match self.keymap.action(&key).filter(|&a| a != binding.action) {
                    Some(other) => {
                        let name = BINDINGS.iter().find(|b| b.action == other).map_or("?", |b| b.name);
                        Some((format!("{spec} is already used by '{name}'"), StatusKind::Error))
                    }
                    None => {
                        self.keymap.set(binding.action, vec![spec]);
                        changed = true;
                        None
                    }
                }
            } else {
                Some(("this key cannot be used".into(), StatusKind::Error))
            };
        } else {
            match key.code {
                KeyCode::Esc => {
                    self.overlay = None;
                    return;
                }
                KeyCode::Up | KeyCode::Char('k') => state.selected = state.selected.saturating_sub(1),
                KeyCode::Down | KeyCode::Char('j') => {
                    state.selected = (state.selected + 1).min(ConfigState::ROWS - 1);
                }
                KeyCode::Home => state.selected = 0,
                KeyCode::End => state.selected = ConfigState::ROWS - 1,
                KeyCode::Enter | KeyCode::Char(' ') | KeyCode::Left | KeyCode::Right if state.selected == 0 => {
                    self.theme_kind = self.theme_kind.toggled();
                    self.theme = Theme::new(self.theme_kind, self.theme.plain);
                    changed = true;
                }
                KeyCode::Enter => {
                    state.capturing = true;
                    state.message = None;
                }
                KeyCode::Backspace | KeyCode::Delete => {
                    if let Some(b) = state.binding() {
                        self.keymap.reset(b.action);
                        changed = true;
                    }
                }
                _ => {
                    if self.keymap.action(&key) == Some(Action::Config) {
                        self.overlay = None;
                        return;
                    }
                }
            }
        }

        if changed {
            state.message = Some(match self.save_config() {
                Ok(()) if self.theme.plain && state.selected == 0 => (
                    "saved; colors are disabled (--no-color / NO_COLOR)".into(),
                    StatusKind::Info,
                ),
                Ok(()) => (
                    format!("saved to {}", self.config_path.as_ref().expect("saved").display()),
                    StatusKind::Info,
                ),
                Err(e) => (e, StatusKind::Error),
            });
        }
        self.overlay = Some(Overlay::Config(state));
    }

    fn picker_key(&mut self, key: KeyEvent) {
        let Some(Overlay::Picker(p)) = &mut self.overlay else {
            return;
        };
        let typing = !key.modifiers.intersects(KeyModifiers::CONTROL | KeyModifiers::ALT);
        match key.code {
            KeyCode::Esc => {
                self.overlay = None;
                // Stop a running scan.
                self.scan_generation.fetch_add(1, Ordering::Relaxed);
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
                    self.scan_generation.fetch_add(1, Ordering::Relaxed);
                    self.open(&path);
                }
                None => {}
            },
            KeyCode::Char('r') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                self.picker_recursive = !p.recursive;
                let dir = p.dir.clone();
                self.open_picker(dir);
            }
            KeyCode::Char('b') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                self.picker_valid_only = !p.valid_only;
                let dir = p.dir.clone();
                self.open_picker(dir);
            }
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
