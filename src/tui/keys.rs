//! Key bindings: the actions, the default bindings table and the user's
//! key map from the configuration file.

use std::collections::BTreeMap;
use std::fmt;

use anyhow::{Context as _, bail};
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

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
    ScrollUp,
    ScrollDown,
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
    Config,
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
    Binding {
        action: Action::Quit,
        name: "quit",
        keys: &["q", "ctrl+c"],
        help: "Quit (closes an open overlay first)",
        hint: Some("quit"),
    },
    Binding {
        action: Action::Open,
        name: "open",
        keys: &["o", "ctrl+p"],
        help: "Open file picker",
        hint: Some("open"),
    },
    Binding {
        action: Action::Info,
        name: "info",
        keys: &["i"],
        help: "Toggle recording info",
        hint: Some("info"),
    },
    Binding {
        action: Action::Help,
        name: "help",
        keys: &["?"],
        help: "Toggle this help",
        hint: Some("help"),
    },
    Binding {
        action: Action::Focus,
        name: "focus",
        keys: &["tab"],
        help: "Switch focus sidebar / plot",
        hint: None,
    },
    Binding {
        action: Action::Up,
        name: "up",
        keys: &["up", "k"],
        help: "Previous channel / scroll up",
        hint: None,
    },
    Binding {
        action: Action::Down,
        name: "down",
        keys: &["down", "j"],
        help: "Next channel / scroll down",
        hint: None,
    },
    Binding {
        action: Action::ScrollUp,
        name: "scroll_up",
        keys: &["pageup"],
        help: "Scroll plots up one page",
        hint: None,
    },
    Binding {
        action: Action::ScrollDown,
        name: "scroll_down",
        keys: &["pagedown"],
        help: "Scroll plots down one page",
        hint: None,
    },
    Binding {
        action: Action::ToggleChannel,
        name: "toggle_channel",
        keys: &["space"],
        help: "Show / hide selected channel",
        hint: Some("toggle"),
    },
    Binding {
        action: Action::ToggleAll,
        name: "toggle_all",
        keys: &["a"],
        help: "Show all / hide all channels",
        hint: None,
    },
    Binding {
        action: Action::ZoomIn,
        name: "zoom_in",
        keys: &["+", "="],
        help: "Zoom in (time)",
        hint: Some("zoom"),
    },
    Binding {
        action: Action::ZoomOut,
        name: "zoom_out",
        keys: &["-"],
        help: "Zoom out (time)",
        hint: None,
    },
    Binding {
        action: Action::CursorLeft,
        name: "cursor_left",
        keys: &["left", "h"],
        help: "Move cursor left",
        hint: Some("cursor"),
    },
    Binding {
        action: Action::CursorRight,
        name: "cursor_right",
        keys: &["right", "l"],
        help: "Move cursor right",
        hint: None,
    },
    Binding {
        action: Action::PanLeft,
        name: "pan_left",
        keys: &["shift+left", "H"],
        help: "Pan view left",
        hint: Some("pan"),
    },
    Binding {
        action: Action::PanRight,
        name: "pan_right",
        keys: &["shift+right", "L"],
        help: "Pan view right",
        hint: None,
    },
    Binding {
        action: Action::ResetZoom,
        name: "reset_zoom",
        keys: &["0"],
        help: "Reset zoom (whole recording)",
        hint: None,
    },
    Binding {
        action: Action::Trigger,
        name: "trigger",
        keys: &["t"],
        help: "Jump cursor to trigger",
        hint: None,
    },
    Binding {
        action: Action::Side,
        name: "side",
        keys: &["p"],
        help: "Toggle primary / secondary values",
        hint: None,
    },
    Binding {
        action: Action::TimeMode,
        name: "time_mode",
        keys: &["m"],
        help: "Toggle relative / absolute time",
        hint: None,
    },
    Binding {
        action: Action::Sidebar,
        name: "sidebar",
        keys: &["s"],
        help: "Show / hide channel sidebar",
        hint: None,
    },
    Binding {
        action: Action::Theme,
        name: "theme",
        keys: &["T"],
        help: "Toggle dark / light theme",
        hint: None,
    },
    Binding {
        action: Action::Config,
        name: "config",
        keys: &["ctrl+,", "c"],
        help: "Configuration (theme, key bindings)",
        hint: Some("config"),
    },
    Binding {
        action: Action::Close,
        name: "close",
        keys: &["esc"],
        help: "Close overlay",
        hint: None,
    },
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

impl KeySpec {
    /// The key of a key event, `None` for keys that cannot be configured
    /// (function keys, media keys, ...).
    pub fn from_event(key: &KeyEvent) -> Option<Self> {
        let mut mods = key.modifiers & (KeyModifiers::CONTROL | KeyModifiers::ALT | KeyModifiers::SHIFT);
        match key.code {
            // Shift is part of the character (`T`, `?`).
            KeyCode::Char(_) => mods -= KeyModifiers::SHIFT,
            KeyCode::Tab
            | KeyCode::BackTab
            | KeyCode::Enter
            | KeyCode::Esc
            | KeyCode::Backspace
            | KeyCode::Up
            | KeyCode::Down
            | KeyCode::Left
            | KeyCode::Right
            | KeyCode::Home
            | KeyCode::End
            | KeyCode::PageUp
            | KeyCode::PageDown => {}
            _ => return None,
        }
        Some(Self { code: key.code, mods })
    }

    /// The key as written in the configuration file (inverse of [`KeySpec::parse`]).
    pub fn config_name(&self) -> String {
        let mut s = String::new();
        for (m, name) in [
            (KeyModifiers::CONTROL, "ctrl+"),
            (KeyModifiers::ALT, "alt+"),
            (KeyModifiers::SHIFT, "shift+"),
        ] {
            if self.mods.contains(m) {
                s.push_str(name);
            }
        }
        match self.code {
            KeyCode::Char(' ') => s.push_str("space"),
            KeyCode::Char(c) => s.push(c),
            KeyCode::Tab => s.push_str("tab"),
            KeyCode::BackTab => s.push_str("backtab"),
            KeyCode::Enter => s.push_str("enter"),
            KeyCode::Esc => s.push_str("esc"),
            KeyCode::Backspace => s.push_str("backspace"),
            KeyCode::Up => s.push_str("up"),
            KeyCode::Down => s.push_str("down"),
            KeyCode::Left => s.push_str("left"),
            KeyCode::Right => s.push_str("right"),
            KeyCode::Home => s.push_str("home"),
            KeyCode::End => s.push_str("end"),
            KeyCode::PageUp => s.push_str("pageup"),
            KeyCode::PageDown => s.push_str("pagedown"),
            _ => s.push('?'),
        }
        s
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
            let specs = b
                .keys
                .iter()
                .map(|k| KeySpec::parse(k))
                .collect::<anyhow::Result<_>>()?;
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
        self.keys(action)
            .first()
            .map_or_else(|| "-".into(), ToString::to_string)
    }

    /// Replaces the keys of `action`.
    pub fn set(&mut self, action: Action, keys: Vec<KeySpec>) {
        self.keys.insert(action, keys);
    }

    /// Restores the default keys of `action`.
    pub fn reset(&mut self, action: Action) {
        if let Some(b) = BINDINGS.iter().find(|b| b.action == action) {
            let specs = b.keys.iter().filter_map(|k| KeySpec::parse(k).ok()).collect();
            self.keys.insert(action, specs);
        }
    }

    /// All bindings in configuration file form (action name to key names).
    pub fn to_config(&self) -> BTreeMap<String, Vec<String>> {
        BINDINGS
            .iter()
            .map(|b| {
                let keys = self.keys(b.action).iter().map(KeySpec::config_name).collect();
                (b.name.to_owned(), keys)
            })
            .collect()
    }
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
    fn key_specs_round_trip_through_config_names() {
        for s in [
            "q",
            "T",
            "?",
            "ctrl+,",
            "ctrl+p",
            "shift+left",
            "space",
            "esc",
            "alt+x",
            "pagedown",
        ] {
            assert_eq!(KeySpec::parse(s).unwrap().config_name(), s);
        }
        let shifted = KeyEvent::new(KeyCode::Char('T'), KeyModifiers::SHIFT);
        assert_eq!(KeySpec::from_event(&shifted).unwrap().config_name(), "T");
        assert!(KeySpec::from_event(&KeyEvent::new(KeyCode::F(5), KeyModifiers::NONE)).is_none());
    }

    #[test]
    fn keymap_defaults_and_overrides() {
        let km = KeyMap::new(&BTreeMap::new()).unwrap();
        assert_eq!(
            km.action(&key(KeyCode::Left, KeyModifiers::SHIFT)),
            Some(Action::PanLeft)
        );
        assert_eq!(
            km.action(&key(KeyCode::Left, KeyModifiers::NONE)),
            Some(Action::CursorLeft)
        );
        assert_eq!(
            km.action(&key(KeyCode::Char('?'), KeyModifiers::SHIFT)),
            Some(Action::Help)
        );
        assert_eq!(
            km.action(&key(KeyCode::PageDown, KeyModifiers::NONE)),
            Some(Action::ScrollDown)
        );

        let mut o = BTreeMap::new();
        o.insert("quit".to_owned(), vec!["x".to_owned()]);
        let km = KeyMap::new(&o).unwrap();
        assert_eq!(
            km.action(&key(KeyCode::Char('x'), KeyModifiers::NONE)),
            Some(Action::Quit)
        );
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
