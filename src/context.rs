//! Settings shared by the commands and the TUI.

use crate::args::{GlobalOpts, ThemeChoice};
use crate::config::AppConfig;
use crate::term::ThemeKind;
use crate::term::themes::Catalog;

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
        self.global
            .theme
            .clone()
            .or_else(|| self.config.theme.clone())
            .unwrap_or(ThemeChoice::Auto)
    }

    /// Name of the theme to use: `auto` resolved to `dark` or `light`.
    pub fn theme_name(&self) -> String {
        match self.theme() {
            ThemeChoice::Auto => ThemeKind::detect().name().to_owned(),
            ThemeChoice::Named(name) => name,
        }
    }

    /// The built-in themes and the user's, from the `themes` directory next
    /// to the configuration file.
    pub fn themes(&self) -> Catalog {
        Catalog::new(self.config_path.as_deref().and_then(AppConfig::themes_dir_of))
    }

    /// Dark or light, for the command output colors. A theme that doesn't
    /// load gives dark: colors are only an extra there, so the command still
    /// runs (the TUI reports the error).
    pub fn theme_kind(&self) -> ThemeKind {
        self.themes().load(&self.theme_name()).map_or_else(
            |e| {
                tracing::warn!("{e:#}");
                ThemeKind::Dark
            },
            |t| t.kind,
        )
    }
}
