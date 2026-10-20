//! Application configuration file (`~/.config/sprcmtrd/config.toml`).
//!
//! ```toml
//! theme = "dark"            # dark | light | auto
//!
//! [keys]                    # action = list of keys, replaces the defaults
//! quit = ["q", "ctrl+c"]
//! zoom_in = ["+", "="]
//! ```

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use anyhow::Context;
use serde::{Deserialize, Serialize};

use crate::args::ThemeChoice;

/// Environment variable that overrides the configuration file location.
pub const CONFIG_ENV: &str = "SPRCMTRD_CONFIG";

/// Settings read from the configuration file.
#[derive(Debug, Clone, Default, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct AppConfig {
    /// Default theme (overridden by `--theme`).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub theme: Option<ThemeChoice>,
    /// Key binding overrides: action name to key list.
    #[serde(default)]
    pub keys: BTreeMap<String, Vec<String>>,
}

impl AppConfig {
    /// Default location of the configuration file.
    pub fn default_path() -> Option<PathBuf> {
        if let Some(p) = std::env::var_os(CONFIG_ENV) {
            return Some(PathBuf::from(p));
        }
        dirs::config_dir().map(|d| d.join("sprcmtrd").join("config.toml"))
    }

    /// Loads the configuration from the default location. A missing file
    /// gives the defaults; an invalid file is an error.
    pub fn load() -> anyhow::Result<Self> {
        match Self::default_path() {
            Some(p) if p.exists() => Self::load_from(&p),
            _ => Ok(Self::default()),
        }
    }

    /// Loads the configuration from `path`.
    pub fn load_from(path: &Path) -> anyhow::Result<Self> {
        let text = std::fs::read_to_string(path)
            .with_context(|| format!("reading config {}", path.display()))?;
        toml::from_str(&text).with_context(|| format!("invalid config {}", path.display()))
    }

    /// Writes the configuration to `path`, creating parent directories.
    /// The file is rewritten completely (comments are not kept).
    pub fn save_to(&self, path: &Path) -> anyhow::Result<()> {
        let body = toml::to_string_pretty(self).context("serializing config")?;
        let text = format!(
            "# sprcmtrd configuration, written by the configuration overlay (Ctrl+, / c).\n\
             # theme: dark | light | auto; [keys]: action = list of keys.\n\n{body}"
        );
        if let Some(dir) = path.parent().filter(|d| !d.as_os_str().is_empty()) {
            std::fs::create_dir_all(dir).with_context(|| format!("creating {}", dir.display()))?;
        }
        std::fs::write(path, text).with_context(|| format!("writing config {}", path.display()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_config() {
        let c: AppConfig = toml::from_str("theme = \"light\"\n[keys]\nquit = [\"x\"]\n").unwrap();
        assert_eq!(c.theme, Some(ThemeChoice::Light));
        assert_eq!(c.keys["quit"], ["x"]);
        assert!(toml::from_str::<AppConfig>("colour = 1").is_err());
    }

    #[test]
    fn save_round_trips() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("sub/config.toml");
        let mut c = AppConfig { theme: Some(ThemeChoice::Dark), ..Default::default() };
        c.keys.insert("open".into(), vec!["ctrl+,".into(), "o".into()]);
        c.save_to(&path).unwrap();
        let back = AppConfig::load_from(&path).unwrap();
        assert_eq!(back.theme, Some(ThemeChoice::Dark));
        assert_eq!(back.keys["open"], ["ctrl+,", "o"]);
    }
}
