//! Command line arguments.

use std::path::PathBuf;

use clap::{Args, Parser, Subcommand, ValueEnum};

/// Terminal viewer for COMTRADE (IEEE C37.111) power system recordings.
///
/// Without a subcommand the interactive viewer is started.
#[derive(Debug, Parser)]
#[command(name = "sprcmtrd", version, about, args_conflicts_with_subcommands = true)]
pub struct Cli {
    /// Subcommand to run; starts the TUI when omitted.
    #[command(subcommand)]
    pub command: Option<Command>,

    /// File or directory to open in the TUI.
    pub path: Option<PathBuf>,

    /// Global options.
    #[command(flatten)]
    pub global: GlobalOpts,
}

/// Options shared by all commands.
#[derive(Debug, Clone, Args)]
pub struct GlobalOpts {
    /// TUI theme.
    #[arg(long, global = true, value_enum)]
    pub theme: Option<ThemeChoice>,

    /// Show analog values on the secondary instead of the primary side.
    #[arg(long, global = true)]
    pub secondary: bool,

    /// Disable colors (also enabled by the NO_COLOR environment variable).
    #[arg(long, global = true)]
    pub no_color: bool,

    /// More log output (-v info, -vv debug, -vvv trace).
    #[arg(short, long, global = true, action = clap::ArgAction::Count)]
    pub verbose: u8,
}

impl GlobalOpts {
    /// Returns `true` when colors are disabled by flag or `NO_COLOR`.
    pub fn colors_disabled(&self) -> bool {
        self.no_color || std::env::var_os("NO_COLOR").is_some_and(|v| !v.is_empty())
    }
}

/// Theme selection.
#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "lowercase")]
pub enum ThemeChoice {
    /// Dark background.
    Dark,
    /// Light background.
    Light,
    /// Detect from the terminal.
    Auto,
}

/// Subcommands.
#[derive(Debug, Subcommand)]
pub enum Command {
    /// Print a summary of a recording.
    Info {
        /// Any file of the recording (.cfg, .dat, .cff, ...).
        file: PathBuf,
        /// Print JSON instead of text.
        #[arg(long)]
        json: bool,
        /// Do not print .hdr / .inf content.
        #[arg(long)]
        no_extra: bool,
    },

    /// List recordings in a directory.
    List {
        /// Directory to search.
        #[arg(default_value = ".")]
        dir: PathBuf,
        /// Only search DIR itself, not its subdirectories.
        #[arg(long)]
        no_recursive: bool,
        /// Only show recordings whose file name matches this regular expression.
        #[arg(long)]
        filter: Option<String>,
        /// Sort order.
        #[arg(long, value_enum, default_value_t = SortKey::Time)]
        sort: SortKey,
    },

    /// Print a summary and plots of a recording to stdout.
    View {
        /// Any file of the recording (.cfg, .dat, .cff, ...).
        file: PathBuf,
        /// Channels to show: comma separated 1-based indices or ids
        /// (analog: `3` or `IA`, binary: `d2` or `TRIP`). Default: all.
        #[arg(long, value_delimiter = ',')]
        channels: Vec<String>,
        /// Start of the time window in ms relative to the trigger.
        #[arg(long, allow_hyphen_values = true)]
        from: Option<f64>,
        /// End of the time window in ms relative to the trigger.
        #[arg(long, allow_hyphen_values = true)]
        to: Option<f64>,
        /// Plot width in columns (default: terminal width).
        #[arg(long)]
        width: Option<u16>,
        /// Height of each analog plot in rows.
        #[arg(long, default_value_t = 10)]
        height: u16,
    },
}

/// Sort order for `list`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub enum SortKey {
    /// File name.
    Name,
    /// Start time of the recording.
    Time,
    /// Size of the data file.
    Size,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cli_is_valid() {
        use clap::CommandFactory;
        Cli::command().debug_assert();
    }

    #[test]
    fn parses_path_or_subcommand() {
        let c = Cli::parse_from(["sprcmtrd", "x.cfg", "--theme", "light"]);
        assert!(c.command.is_none());
        assert_eq!(c.global.theme, Some(ThemeChoice::Light));
        let c = Cli::parse_from(["sprcmtrd", "view", "x.cfg", "--from", "-20", "--channels", "IA,d1"]);
        let Some(Command::View { from, channels, .. }) = c.command else {
            panic!()
        };
        assert_eq!(from, Some(-20.0));
        assert_eq!(channels, ["IA", "d1"]);
    }
}
