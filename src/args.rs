//! Command line arguments.

use std::path::PathBuf;

use clap::{Args, Parser, Subcommand, ValueEnum, ValueHint};

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
    #[arg(value_hint = ValueHint::AnyPath)]
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
        #[arg(value_hint = ValueHint::FilePath)]
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
        #[arg(default_value = ".", value_hint = ValueHint::DirPath)]
        dir: PathBuf,
        /// Only search DIR itself, not its subdirectories.
        #[arg(long)]
        no_recursive: bool,
        /// Only show recordings whose file name matches this regular expression.
        #[arg(long)]
        filter: Option<String>,
        /// Only show recordings that load without errors (configuration and data).
        #[arg(long)]
        valid: bool,
        /// Sort order. Default: in a terminal, rows are shown as they are
        /// found; otherwise sorted by start time.
        #[arg(long, value_enum)]
        sort: Option<SortKey>,
    },

    /// Print a summary and plots of a recording to stdout.
    View {
        /// Any file of the recording (.cfg, .dat, .cff, ...).
        #[arg(value_hint = ValueHint::FilePath)]
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

    /// Statistics per channel: min, max, mean, RMS and peak of analog
    /// channels, state changes of binary channels.
    ///
    /// Examples:
    ///   sprcmtrd stats fault.cfg
    ///   sprcmtrd stats fault.cfg --channels IA,IB,IC --from 0 --to 100
    #[command(visible_alias = "stat", verbatim_doc_comment)]
    Stats {
        /// Any file of the recording (.cfg, .dat, .cff, ...).
        #[arg(value_hint = ValueHint::FilePath)]
        file: PathBuf,
        /// Channels: comma separated 1-based indices or ids
        /// (analog: `3` or `IA`, binary: `d2` or `TRIP`). Default: all.
        #[arg(long, value_delimiter = ',')]
        channels: Vec<String>,
        /// Start of the time window in ms relative to the trigger.
        #[arg(long, allow_hyphen_values = true)]
        from: Option<f64>,
        /// End of the time window in ms relative to the trigger.
        #[arg(long, allow_hyphen_values = true)]
        to: Option<f64>,
        /// Print JSON instead of text.
        #[arg(long)]
        json: bool,
    },

    /// Export samples as CSV or JSON (for Excel, Python, gnuplot, ...).
    ///
    /// Analog values are scaled (primary side unless --secondary), binary
    /// channels are 0 or 1, missing values are empty (CSV) or null (JSON).
    ///
    /// Examples:
    ///   sprcmtrd export fault.cfg -o fault.csv
    ///   sprcmtrd export fault.cfg --channels IA,IB,TRIP --from -20 --to 100
    ///   sprcmtrd export fault.cfg --format json --every 4 -o fault.json
    #[command(verbatim_doc_comment)]
    Export {
        /// Any file of the recording (.cfg, .dat, .cff, ...).
        #[arg(value_hint = ValueHint::FilePath)]
        file: PathBuf,
        /// Output format (default: json for a .json output file, otherwise csv).
        #[arg(long, value_enum)]
        format: Option<ExportFormat>,
        /// Channels to export: comma separated 1-based indices or ids
        /// (analog: `3` or `IA`, binary: `d2` or `TRIP`). Default: all.
        #[arg(long, value_delimiter = ',')]
        channels: Vec<String>,
        /// Start of the time window in ms relative to the trigger.
        #[arg(long, allow_hyphen_values = true)]
        from: Option<f64>,
        /// End of the time window in ms relative to the trigger.
        #[arg(long, allow_hyphen_values = true)]
        to: Option<f64>,
        /// Export only every Nth sample (no filtering).
        #[arg(long, default_value_t = 1, value_parser = clap::value_parser!(u32).range(1..))]
        every: u32,
        /// Time as date and time of day instead of ms relative to the trigger.
        #[arg(long)]
        absolute: bool,
        /// Write to this file instead of stdout.
        #[arg(short, long, value_hint = ValueHint::FilePath)]
        output: Option<PathBuf>,
    },

    /// Cut a time window and/or some channels into a new, smaller recording.
    ///
    /// The output keeps the revision and data format of the source (`.cff`
    /// output needs revision 2013, see `convert`). Times relative to the
    /// trigger stay the same.
    ///
    /// Examples:
    ///   sprcmtrd cut fault.cfg --from -20 --to 100 -o fault_short.cfg
    ///   sprcmtrd cut fault.cfg --channels IA,IB,IC,TRIP -o fault_currents.cfg
    #[command(verbatim_doc_comment)]
    Cut {
        /// Any file of the recording (.cfg, .dat, .cff, ...).
        #[arg(value_hint = ValueHint::FilePath)]
        file: PathBuf,
        /// Output file: NAME.cfg (with NAME.dat) or NAME.cff.
        #[arg(short, long, value_hint = ValueHint::FilePath)]
        output: PathBuf,
        /// Start of the time window in ms relative to the trigger.
        #[arg(long, allow_hyphen_values = true)]
        from: Option<f64>,
        /// End of the time window in ms relative to the trigger.
        #[arg(long, allow_hyphen_values = true)]
        to: Option<f64>,
        /// Channels to keep: comma separated 1-based indices or ids
        /// (analog: `3` or `IA`, binary: `d2` or `TRIP`). Default: all.
        #[arg(long, value_delimiter = ',')]
        channels: Vec<String>,
        /// Overwrite existing output files.
        #[arg(long)]
        force: bool,
    },

    /// Convert a recording to another revision, data format or file layout.
    ///
    /// The output extension selects the layout: `.cfg` writes .cfg + .dat
    /// (and .hdr / .inf when the source has them), `.cff` one combined file.
    ///
    /// Examples:
    ///   sprcmtrd convert fault.cfg --output fault_ascii.cfg --format ascii
    ///   sprcmtrd convert fault.cfg --output fault.cff
    ///   sprcmtrd convert new.cff --output old.cfg --rev 1999 --format binary
    #[command(verbatim_doc_comment)]
    Convert {
        /// Any file of the recording (.cfg, .dat, .cff, ...).
        #[arg(value_hint = ValueHint::FilePath)]
        file: PathBuf,
        /// Output file: NAME.cfg (with NAME.dat) or NAME.cff.
        #[arg(short, long, value_hint = ValueHint::FilePath)]
        output: PathBuf,
        /// Data format of the output (default: the source format).
        #[arg(long, value_enum)]
        format: Option<FormatArg>,
        /// Standard revision of the output (default: the source revision, or
        /// 2013 when the format or .cff output needs it).
        #[arg(long, value_enum)]
        rev: Option<RevisionArg>,
        /// Overwrite existing output files.
        #[arg(long)]
        force: bool,
    },

    /// Print a tab-completion script for your shell.
    ///
    /// Examples:
    ///   sprcmtrd completions bash --output ~/.local/share/bash-completion/completions/sprcmtrd
    ///   sprcmtrd completions zsh --output ~/.zfunc/_sprcmtrd
    ///   sprcmtrd completions fish --output ~/.config/fish/completions/sprcmtrd.fish
    ///   sprcmtrd completions powershell >> $PROFILE
    #[command(verbatim_doc_comment)]
    Completions {
        /// Shell to generate the script for.
        shell: clap_complete::Shell,
        /// Write the script to this file instead of stdout.
        #[arg(short, long, value_hint = ValueHint::FilePath)]
        output: Option<PathBuf>,
    },
}

/// Output format for `export`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub enum ExportFormat {
    /// Comma separated values with a header row.
    Csv,
    /// One JSON object with an array per column.
    Json,
}

/// Data file format for `convert`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub enum FormatArg {
    /// Comma separated text.
    Ascii,
    /// 16-bit integers.
    Binary,
    /// 32-bit integers (revision 2013).
    Binary32,
    /// 32-bit floats (revision 2013).
    Float32,
}

/// Standard revision for `convert`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub enum RevisionArg {
    /// IEEE C37.111-1991.
    #[value(name = "1991")]
    Rev1991,
    /// IEEE C37.111-1999.
    #[value(name = "1999")]
    Rev1999,
    /// IEEE C37.111-2013 / IEC 60255-24:2013.
    #[value(name = "2013")]
    Rev2013,
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
        let c = Cli::parse_from(["sprcmtrd", "completions", "zsh", "-o", "_sprcmtrd"]);
        let Some(Command::Completions { shell, output }) = c.command else {
            panic!()
        };
        assert_eq!(shell, clap_complete::Shell::Zsh);
        assert_eq!(output, Some(PathBuf::from("_sprcmtrd")));
    }
}
