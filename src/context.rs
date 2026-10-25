//! Settings shared by the commands and the TUI.

use crate::args::{GlobalOpts, ThemeChoice};
use crate::config::AppConfig;

/// Settings shared by the commands and the TUI: global command line options
/// and the configuration file.
#[derive(Debug, Clone)]
pub struct Context {
    /// Global command line options.
    pub global: GlobalOpts,
    /// Configuration file settings.
    pub config: AppConfig,
    /// Where the configuration overlay saves settings (`None`: no config directory).
    pub config_path: Option<std::path::PathBuf>,
}

impl Context {
    /// Context for `global` options and `config`, saving to the default
    /// configuration path.
    pub fn new(global: GlobalOpts, config: AppConfig) -> Self {
        Self {
            global,
            config,
            config_path: AppConfig::default_path(),
        }
    }

    /// The theme: `--theme`, else the configuration file, else `auto`.
    pub fn theme(&self) -> ThemeChoice {
        self.global.theme.or(self.config.theme).unwrap_or(ThemeChoice::Auto)
    }
}
