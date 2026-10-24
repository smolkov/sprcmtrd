//! Command line arguments.

use std::path::PathBuf;

use chrono::{NaiveDate, NaiveDateTime, NaiveTime};
use clap::{Args, Parser, Subcommand, ValueEnum, ValueHint};
use clap_complete::engine::ArgValueCompleter;

use crate::complete::recordings;

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
    #[arg(value_hint = ValueHint::AnyPath, add = ArgValueCompleter::new(recordings))]
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
    Info(InfoArgs),

    /// List recordings in a directory.
    List(ListArgs),

    /// Print a summary and plots of a recording to stdout.
    View(ViewArgs),

    /// Check recordings for problems (for scripts and CI).
    ///
    /// Errors: the recording can't be read correctly. Warnings: missing or
    /// clipped values, times out of order, missing units, unusable ratios, ...
    /// Exit code 4 when a recording has an error (or a warning with --strict).
    ///
    /// Examples:
    ///   sprcmtrd check fault.cfg
    ///   sprcmtrd check /data/recorder --strict
    #[command(verbatim_doc_comment)]
    Check(CheckArgs),

    /// Timeline of binary channel changes (start, trip, breaker open, ...).
    ///
    /// Examples:
    ///   sprcmtrd events fault.cfg
    ///   sprcmtrd events fault.cfg --channels TRIP,CB_OPEN --from 0
    #[command(verbatim_doc_comment)]
    Events(EventsArgs),

    /// Phasors of the fundamental frequency and symmetrical components.
    ///
    /// A DFT over whole cycles ending at --at, as a protection relay sees the
    /// signals at that moment. Magnitudes are RMS, angles in degrees relative
    /// to the reference channel. Channels with phases A, B, C of one circuit
    /// and unit also get zero, positive and negative sequence.
    ///
    /// Examples:
    ///   sprcmtrd phasors fault.cfg --at -20
    ///   sprcmtrd phasors fault.cfg --at 40 --channels IA,IB,IC --reference VA
    #[command(visible_alias = "phasor", verbatim_doc_comment)]
    Phasors(PhasorsArgs),

    /// Statistics per channel: min, max, mean, RMS and peak of analog
    /// channels, state changes of binary channels.
    ///
    /// Examples:
    ///   sprcmtrd stats fault.cfg
    ///   sprcmtrd stats fault.cfg --channels IA,IB,IC --from 0 --to 100
    #[command(visible_alias = "stat", verbatim_doc_comment)]
    Stats(StatsArgs),

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
    Export(ExportArgs),

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
    Cut(CutArgs),

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
    Convert(ConvertArgs),

    /// Print the tab-completion script for your shell.
    ///
    /// On Tab the shell asks sprcmtrd, which completes subcommands, options and
    /// recordings (.cfg / .cff) in the typed directory and its subdirectories.
    /// Recommended: load the script on shell start, so it always matches the
    /// installed version:
    ///   bash  ~/.bashrc:                      source <(COMPLETE=bash sprcmtrd)
    ///   zsh   ~/.zshrc (after compinit):      source <(COMPLETE=zsh sprcmtrd)
    ///   fish  ~/.config/fish/config.fish:     COMPLETE=fish sprcmtrd | source
    /// Or write it to a file (again after each update):
    ///   sprcmtrd completions bash --output ~/.local/share/bash-completion/completions/sprcmtrd
    #[command(verbatim_doc_comment)]
    Completions(CompletionsArgs),
}

/// Arguments of `info`.
#[derive(Debug, Args)]
pub struct InfoArgs {
    /// Any file of the recording (.cfg, .dat, .cff, ...).
    #[arg(value_hint = ValueHint::FilePath, add = ArgValueCompleter::new(recordings))]
    pub file: PathBuf,
    /// Print JSON instead of text.
    #[arg(long)]
    pub json: bool,
    /// Do not print .hdr / .inf content.
    #[arg(long)]
    pub no_extra: bool,
}

/// Arguments of `list`.
#[derive(Debug, Args)]
pub struct ListArgs {
    /// Directory to search.
    #[arg(default_value = ".", value_hint = ValueHint::DirPath)]
    pub dir: PathBuf,
    /// Only search DIR itself, not its subdirectories.
    #[arg(long)]
    pub no_recursive: bool,
    /// Only show recordings whose file name matches this regular expression.
    #[arg(long)]
    pub filter: Option<String>,
    /// Only show recordings that load without errors (configuration and data).
    #[arg(long)]
    pub valid: bool,
    /// Only show recordings whose station name matches this regular
    /// expression (case-insensitive).
    #[arg(long)]
    pub station: Option<String>,
    /// Only recordings that start at or after this time
    /// (`2024-03-15`, `2024-03-15 08:30`, `2024-03-15T08:30:00`).
    #[arg(long, value_parser = parse_since)]
    pub since: Option<NaiveDateTime>,
    /// Only recordings that start at or before this time; a date alone
    /// includes the whole day.
    #[arg(long, value_parser = parse_until)]
    pub until: Option<NaiveDateTime>,
    /// Only recordings at least this long (`500ms`, `2s`, `1min`; a bare
    /// number is ms).
    #[arg(long, value_parser = parse_duration)]
    pub min_duration: Option<f64>,
    /// Only recordings at most this long.
    #[arg(long, value_parser = parse_duration)]
    pub max_duration: Option<f64>,
    /// Sort order. Default: in a terminal, rows are shown as they are
    /// found; otherwise sorted by start time.
    #[arg(long, value_enum)]
    pub sort: Option<SortKey>,
}

/// Arguments of `view`.
#[derive(Debug, Args)]
pub struct ViewArgs {
    /// Any file of the recording (.cfg, .dat, .cff, ...).
    #[arg(value_hint = ValueHint::FilePath, add = ArgValueCompleter::new(recordings))]
    pub file: PathBuf,
    /// Channels to show: comma separated 1-based indices or ids
    /// (analog: `3` or `IA`, binary: `d2` or `TRIP`). Default: all.
    #[arg(long, value_delimiter = ',')]
    pub channels: Vec<String>,
    /// Start of the time window in ms relative to the trigger.
    #[arg(long, allow_hyphen_values = true)]
    pub from: Option<f64>,
    /// End of the time window in ms relative to the trigger.
    #[arg(long, allow_hyphen_values = true)]
    pub to: Option<f64>,
    /// Plot width in columns (default: terminal width).
    #[arg(long)]
    pub width: Option<u16>,
    /// Height of each analog plot in rows.
    #[arg(long, default_value_t = 10)]
    pub height: u16,
}

/// Arguments of `check`.
#[derive(Debug, Args)]
pub struct CheckArgs {
    /// A recording file, or a directory to check all recordings in.
    #[arg(value_hint = ValueHint::AnyPath, add = ArgValueCompleter::new(recordings))]
    pub path: PathBuf,
    /// Only check DIR itself, not its subdirectories.
    #[arg(long)]
    pub no_recursive: bool,
    /// Fail (exit code 4) on warnings too.
    #[arg(long)]
    pub strict: bool,
    /// Print JSON instead of text.
    #[arg(long)]
    pub json: bool,
}

/// Arguments of `events`.
#[derive(Debug, Args)]
pub struct EventsArgs {
    /// Any file of the recording (.cfg, .dat, .cff, ...).
    #[arg(value_hint = ValueHint::FilePath, add = ArgValueCompleter::new(recordings))]
    pub file: PathBuf,
    /// Binary channels: comma separated `dN` indices or ids. Default: all.
    #[arg(long, value_delimiter = ',')]
    pub channels: Vec<String>,
    /// Start of the time window in ms relative to the trigger.
    #[arg(long, allow_hyphen_values = true)]
    pub from: Option<f64>,
    /// End of the time window in ms relative to the trigger.
    #[arg(long, allow_hyphen_values = true)]
    pub to: Option<f64>,
    /// Print JSON instead of text.
    #[arg(long)]
    pub json: bool,
}

/// Arguments of `phasors`.
#[derive(Debug, Args)]
pub struct PhasorsArgs {
    /// Any file of the recording (.cfg, .dat, .cff, ...).
    #[arg(value_hint = ValueHint::FilePath, add = ArgValueCompleter::new(recordings))]
    pub file: PathBuf,
    /// End of the window in ms relative to the trigger.
    #[arg(long, allow_hyphen_values = true, default_value_t = 0.0)]
    pub at: f64,
    /// Window length in cycles of the fundamental frequency.
    #[arg(long, default_value_t = 1.0)]
    pub cycles: f64,
    /// Analog channels: comma separated 1-based indices or ids. Default: all.
    #[arg(long, value_delimiter = ',')]
    pub channels: Vec<String>,
    /// Channel the angles refer to (index or id). Default: the first channel.
    #[arg(long)]
    pub reference: Option<String>,
    /// Fundamental frequency in Hz. Default: the line frequency of the recording.
    #[arg(long)]
    pub frequency: Option<f64>,
    /// Print JSON instead of text.
    #[arg(long)]
    pub json: bool,
}

/// Arguments of `stats`.
#[derive(Debug, Args)]
pub struct StatsArgs {
    /// Any file of the recording (.cfg, .dat, .cff, ...).
    #[arg(value_hint = ValueHint::FilePath, add = ArgValueCompleter::new(recordings))]
    pub file: PathBuf,
    /// Channels: comma separated 1-based indices or ids
    /// (analog: `3` or `IA`, binary: `d2` or `TRIP`). Default: all.
    #[arg(long, value_delimiter = ',')]
    pub channels: Vec<String>,
    /// Start of the time window in ms relative to the trigger.
    #[arg(long, allow_hyphen_values = true)]
    pub from: Option<f64>,
    /// End of the time window in ms relative to the trigger.
    #[arg(long, allow_hyphen_values = true)]
    pub to: Option<f64>,
    /// Print JSON instead of text.
    #[arg(long)]
    pub json: bool,
}

/// Arguments of `export`.
#[derive(Debug, Args)]
pub struct ExportArgs {
    /// Any file of the recording (.cfg, .dat, .cff, ...).
    #[arg(value_hint = ValueHint::FilePath, add = ArgValueCompleter::new(recordings))]
    pub file: PathBuf,
    /// Output format (default: json for a .json output file, otherwise csv).
    #[arg(long, value_enum)]
    pub format: Option<ExportFormat>,
    /// Channels to export: comma separated 1-based indices or ids
    /// (analog: `3` or `IA`, binary: `d2` or `TRIP`). Default: all.
    #[arg(long, value_delimiter = ',')]
    pub channels: Vec<String>,
    /// Start of the time window in ms relative to the trigger.
    #[arg(long, allow_hyphen_values = true)]
    pub from: Option<f64>,
    /// End of the time window in ms relative to the trigger.
    #[arg(long, allow_hyphen_values = true)]
    pub to: Option<f64>,
    /// Export only every Nth sample (no filtering).
    #[arg(long, default_value_t = 1, value_parser = clap::value_parser!(u32).range(1..))]
    pub every: u32,
    /// Time as date and time of day instead of ms relative to the trigger.
    #[arg(long)]
    pub absolute: bool,
    /// Write to this file instead of stdout.
    #[arg(short, long, value_hint = ValueHint::FilePath)]
    pub output: Option<PathBuf>,
}

/// Arguments of `cut`.
#[derive(Debug, Args)]
pub struct CutArgs {
    /// Any file of the recording (.cfg, .dat, .cff, ...).
    #[arg(value_hint = ValueHint::FilePath, add = ArgValueCompleter::new(recordings))]
    pub file: PathBuf,
    /// Output file: NAME.cfg (with NAME.dat) or NAME.cff.
    #[arg(short, long, value_hint = ValueHint::FilePath)]
    pub output: PathBuf,
    /// Start of the time window in ms relative to the trigger.
    #[arg(long, allow_hyphen_values = true)]
    pub from: Option<f64>,
    /// End of the time window in ms relative to the trigger.
    #[arg(long, allow_hyphen_values = true)]
    pub to: Option<f64>,
    /// Channels to keep: comma separated 1-based indices or ids
    /// (analog: `3` or `IA`, binary: `d2` or `TRIP`). Default: all.
    #[arg(long, value_delimiter = ',')]
    pub channels: Vec<String>,
    /// Overwrite existing output files.
    #[arg(long)]
    pub force: bool,
}

/// Arguments of `convert`.
#[derive(Debug, Args)]
pub struct ConvertArgs {
    /// Any file of the recording (.cfg, .dat, .cff, ...).
    #[arg(value_hint = ValueHint::FilePath, add = ArgValueCompleter::new(recordings))]
    pub file: PathBuf,
    /// Output file: NAME.cfg (with NAME.dat) or NAME.cff.
    #[arg(short, long, value_hint = ValueHint::FilePath)]
    pub output: PathBuf,
    /// Data format of the output (default: the source format).
    #[arg(long, value_enum)]
    pub format: Option<FormatArg>,
    /// Standard revision of the output (default: the source revision, or
    /// 2013 when the format or .cff output needs it).
    #[arg(long, value_enum)]
    pub rev: Option<RevisionArg>,
    /// Overwrite existing output files.
    #[arg(long)]
    pub force: bool,
}

/// Arguments of `completions`.
#[derive(Debug, Args)]
pub struct CompletionsArgs {
    /// Shell to generate the script for.
    pub shell: clap_complete::Shell,
    /// Write the script to this file instead of stdout.
    #[arg(short, long, value_hint = ValueHint::FilePath)]
    pub output: Option<PathBuf>,
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

/// Parses a date or date and time for `--since`; a date alone is its start.
fn parse_since(s: &str) -> Result<NaiveDateTime, String> {
    parse_datetime(s, NaiveTime::MIN)
}

/// Parses a date or date and time for `--until`; a date alone is its end, so
/// the whole day is included.
fn parse_until(s: &str) -> Result<NaiveDateTime, String> {
    let end_of_day = NaiveTime::from_hms_nano_opt(23, 59, 59, 999_999_999).expect("valid time");
    parse_datetime(s, end_of_day)
}

/// `2024-03-15`, `2024-03-15 08:30`, `2024-03-15 08:30:00(.fff)`, also with
/// `T` between date and time. A date alone gets `day_time`.
fn parse_datetime(s: &str, day_time: NaiveTime) -> Result<NaiveDateTime, String> {
    let s = s.trim();
    if let Ok(date) = NaiveDate::parse_from_str(s, "%Y-%m-%d") {
        return Ok(date.and_time(day_time));
    }
    let s = s.replacen('T', " ", 1);
    ["%Y-%m-%d %H:%M:%S%.f", "%Y-%m-%d %H:%M"]
        .iter()
        .find_map(|f| NaiveDateTime::parse_from_str(&s, f).ok())
        .ok_or_else(|| format!("expected a date like 2024-03-15 or 2024-03-15 08:30, got '{s}'"))
}

/// Parses a duration in seconds: `500ms`, `2s`, `1.5min`, `1h`; a bare number is ms.
fn parse_duration(s: &str) -> Result<f64, String> {
    let s = s.trim();
    let split = s.find(|c: char| c.is_ascii_alphabetic()).unwrap_or(s.len());
    let (number, unit) = s.split_at(split);
    let value: f64 = number
        .trim()
        .parse()
        .map_err(|_| format!("expected a duration like 500ms or 2s, got '{s}'"))?;
    let factor = match unit.trim() {
        "" | "ms" => 1e-3,
        "s" => 1.0,
        "min" => 60.0,
        "h" => 3600.0,
        u => return Err(format!("unknown unit '{u}' (ms, s, min, h)")),
    };
    if value < 0.0 {
        return Err("a duration can't be negative".into());
    }
    Ok(value * factor)
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
    fn parses_dates_and_durations() {
        let t = |s| NaiveDateTime::parse_from_str(s, "%Y-%m-%d %H:%M:%S%.f").unwrap();
        assert_eq!(parse_since("2024-03-15"), Ok(t("2024-03-15 00:00:00")));
        assert_eq!(parse_until("2024-03-15"), Ok(t("2024-03-15 23:59:59.999999999")));
        assert_eq!(parse_since("2024-03-15 08:30"), Ok(t("2024-03-15 08:30:00")));
        assert_eq!(parse_until("2024-03-15T08:30:01.5"), Ok(t("2024-03-15 08:30:01.5")));
        assert!(parse_since("15.03.2024").is_err());

        assert_eq!(parse_duration("500ms"), Ok(0.5));
        assert_eq!(parse_duration("2s"), Ok(2.0));
        assert_eq!(parse_duration("1.5 min"), Ok(90.0));
        assert_eq!(parse_duration("250"), Ok(0.25));
        assert!(parse_duration("2 days").is_err());
        assert!(parse_duration("-1s").is_err());
    }

    #[test]
    fn parses_path_or_subcommand() {
        let c = Cli::parse_from(["sprcmtrd", "x.cfg", "--theme", "light"]);
        assert!(c.command.is_none());
        assert_eq!(c.global.theme, Some(ThemeChoice::Light));
        let c = Cli::parse_from(["sprcmtrd", "view", "x.cfg", "--from", "-20", "--channels", "IA,d1"]);
        let Some(Command::View(ViewArgs { from, channels, .. })) = c.command else {
            panic!()
        };
        assert_eq!(from, Some(-20.0));
        assert_eq!(channels, ["IA", "d1"]);
        let c = Cli::parse_from(["sprcmtrd", "completions", "zsh", "-o", "_sprcmtrd"]);
        let Some(Command::Completions(CompletionsArgs { shell, output })) = c.command else {
            panic!()
        };
        assert_eq!(shell, clap_complete::Shell::Zsh);
        assert_eq!(output, Some(PathBuf::from("_sprcmtrd")));
    }
}
