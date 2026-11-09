//! Command line arguments.

use std::path::PathBuf;

use chrono::{NaiveDate, NaiveDateTime, NaiveTime};
use clap::{Args, Parser, Subcommand, ValueEnum, ValueHint};
use clap_complete::engine::{ArgValueCandidates, ArgValueCompleter};

use crate::complete::{recordings, theme_names};

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
    /// Color theme: dark, light, auto or a theme name (see `sprcmtrd themes`).
    #[arg(long, global = true, value_name = "NAME", add = ArgValueCandidates::new(theme_names))]
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
    /// The side values are shown on: secondary with `--secondary`.
    pub fn side(&self) -> crate::comtrade::Side {
        if self.secondary {
            crate::comtrade::Side::Secondary
        } else {
            crate::comtrade::Side::Primary
        }
    }

    /// Returns `true` when colors are disabled by flag or `NO_COLOR`.
    pub fn colors_disabled(&self) -> bool {
        self.no_color || std::env::var_os("NO_COLOR").is_some_and(|v| !v.is_empty())
    }
}

/// Time window of `view`, `stats`, `export`, `cut` and `events`.
#[derive(Debug, Clone, Default, Args)]
pub struct TimeWindow {
    /// Start of the time window in ms relative to the trigger.
    #[arg(long, allow_hyphen_values = true)]
    pub from: Option<f64>,
    /// End of the time window in ms relative to the trigger.
    #[arg(long, allow_hyphen_values = true)]
    pub to: Option<f64>,
}

/// Channels and time window of `view`, `stats`, `export` and `cut`.
#[derive(Debug, Clone, Default, Args)]
pub struct Selection {
    /// Channels: comma separated 1-based indices or ids
    /// (analog: `3` or `IA`, binary: `d2` or `TRIP`). Default: all.
    #[arg(long, value_delimiter = ',')]
    pub channels: Vec<String>,
    /// Time window.
    #[command(flatten)]
    pub window: TimeWindow,
}

/// Theme selection: `auto` or the name of a theme. The names are checked
/// when the theme is loaded, because user themes are files.
#[derive(Debug, Clone, PartialEq, Eq, serde::Deserialize, serde::Serialize)]
#[serde(try_from = "String", into = "String")]
pub enum ThemeChoice {
    /// Dark or light from the terminal (`COLORFGBG`).
    Auto,
    /// A built-in or user theme.
    Named(String),
}

impl ThemeChoice {
    /// The theme `name`.
    pub fn named(name: &str) -> Self {
        Self::Named(name.to_owned())
    }
}

impl std::str::FromStr for ThemeChoice {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.trim() {
            "" => Err("the theme name is empty".into()),
            "auto" => Ok(Self::Auto),
            name => Ok(Self::named(name)),
        }
    }
}

impl TryFrom<String> for ThemeChoice {
    type Error = String;

    fn try_from(s: String) -> Result<Self, Self::Error> {
        s.parse()
    }
}

impl From<ThemeChoice> for String {
    fn from(c: ThemeChoice) -> Self {
        match c {
            ThemeChoice::Auto => "auto".into(),
            ThemeChoice::Named(name) => name,
        }
    }
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

    /// Fault summary: inception, faulted phases, duration, peak currents and
    /// operate times (start, trip, breaker open, current interrupted).
    ///
    /// Inception is the first sample where a current departs from the cycle
    /// before (superimposed current). The faulted phases come from the change
    /// of the phasors against the pre-fault cycle, ground from the residual
    /// current. Start, trip and breaker channels are found by name (START,
    /// PICKUP, TRIP, CB, BREAKER) or given with --start, --trip, --breaker.
    /// Only the first fault in --from / --to is analysed.
    ///
    /// Examples:
    ///   sprcmtrd fault fault.cfg
    ///   sprcmtrd fault fault.cfg --current IL1,IL2,IL3 --trip D12 --breaker D8
    #[command(verbatim_doc_comment)]
    Fault(FaultArgs),

    /// Decaying DC offset of the fault current: DC part, time constant, X/R.
    ///
    /// Finds the fault as the fault command does and fits each phase current
    /// from inception over --cycles cycles (cut at the interruption) as
    /// a·sin(ωt) + b·cos(ωt) + D·e^(−t/τ). Shows D (in A and % of the AC
    /// peak), τ and X/R = ω·τ, relevant for CT dimensioning and breaker duty.
    /// τ only for a DC part of at least 5 % of the AC peak.
    ///
    /// Examples:
    ///   sprcmtrd dc-offset fault.cfg
    ///   sprcmtrd dc-offset fault.cfg --cycles 5 --json
    #[command(verbatim_doc_comment)]
    DcOffset(DcOffsetArgs),

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

    /// Apparent impedance of the six fault loops, as a distance relay sees it.
    ///
    /// Phase to ground V / (I + k0·3I0), phase to phase (V1 − V2) / (I1 − I2),
    /// from the phasors of a voltage and a current A/B/C group (found by unit,
    /// or --voltage / --current). With --trend, R and X per loop over time.
    ///
    /// Examples:
    ///   sprcmtrd impedance fault.cfg --at 40 --k0 0.8@-5
    ///   sprcmtrd impedance fault.cfg --trend --loops BN,BC --format csv
    #[command(verbatim_doc_comment)]
    Impedance(ImpedanceArgs),

    /// Single-ended fault location: distance to the fault from this line end.
    ///
    /// Finds the fault as the fault command does, measures the faulted loop
    /// (ground loops compensated with k0 from --z1 / --z0) over one cycle in
    /// the middle of the fault, and computes the distance with the reactance
    /// method, Im(V/I) / Im(z1), and with Takagi, which uses the change of the
    /// current against the pre-fault cycle to remove the fault resistance.
    /// Line data per km on the primary side.
    ///
    /// Examples:
    ///   sprcmtrd locate fault.cfg --z1 0.03+j0.33 --z0 0.1+j1.1 --length 42
    ///   sprcmtrd locate fault.cfg --z1 0.33@85 --length 42 --loop BC --json
    #[command(verbatim_doc_comment)]
    Locate(LocateArgs),

    /// Two recordings of the same event side by side: phasors per channel.
    ///
    /// Pairs the analog channels with the same id (or --pair), aligns the
    /// time axes (by absolute time, the triggers or the fault inception) and
    /// compares the phasors at --at in A's time: RMS, ratio B/A and the angle
    /// difference, B's angles turned into A's time frame. With
    /// --differential, the differential current of two line ends. --trend
    /// shows the values cycle by cycle, --plot the waveforms of each pair.
    ///
    /// Examples:
    ///   sprcmtrd compare end_a.cfg end_b.cfg --at 40 --differential
    ///   sprcmtrd compare relay.cfg recorder.cfg --align fault --pair IA=IL1,IB=IL2,IC=IL3
    ///   sprcmtrd compare end_a.cfg end_b.cfg --trend --differential --from -20 --to 100
    ///   sprcmtrd compare end_a.cfg end_b.cfg --plot --channels IA --from -40 --to 80
    #[command(verbatim_doc_comment)]
    Compare(CompareArgs),

    /// Active, reactive and apparent power and power factor of a three-phase
    /// circuit, per phase and in total.
    ///
    /// S = V·I* per phase from the phasors of a voltage and a current A/B/C
    /// group (found by unit, or --voltage / --current; phase-to-neutral
    /// voltages). Positive P flows the way the currents are measured, positive
    /// Q is inductive. With --trend, the power over time.
    ///
    /// Examples:
    ///   sprcmtrd power fault.cfg --at -40
    ///   sprcmtrd power fault.cfg --trend --from -60 --to 200 --format csv
    #[command(verbatim_doc_comment)]
    Power(PowerArgs),

    /// Harmonic spectrum and THD per channel at one moment.
    ///
    /// A DFT over whole cycles ending at --at, like phasors. Harmonics and DC
    /// in % of the fundamental; orders up to --max-order, at most below half
    /// the sample rate.
    ///
    /// Examples:
    ///   sprcmtrd harmonics fault.cfg --at 40 --channels IA,IB,IC
    ///   sprcmtrd harmonics fault.cfg --cycles 10 --max-order 25 --angles
    #[command(verbatim_doc_comment)]
    Harmonics(HarmonicsArgs),

    /// Values over time, cycle by cycle: fundamental magnitude per channel,
    /// symmetrical components per three-phase group and the system frequency.
    ///
    /// Each row is a window of --cycles cycles ending at that time, like the
    /// phasors command at one moment. Group columns: + positive, - negative,
    /// 0 zero sequence, -/+ unbalance in %. The frequency comes from the
    /// voltages (positive sequence) if they have a signal, else the currents.
    ///
    /// Examples:
    ///   sprcmtrd trend fault.cfg --from -40 --to 120 --channels IA,IB,IC
    ///   sprcmtrd trend fault.cfg --step 5ms --format csv > trend.csv
    #[command(verbatim_doc_comment)]
    Trend(TrendArgs),

    /// System frequency over time and its rate of change (ROCOF).
    ///
    /// Measured on one channel (default: the first voltage) over --cycles
    /// cycles ending at each row: the periods between rising zero crossings,
    /// or the turn of the phasor (with frequency tracking). The ROCOF compares
    /// each measurement with the one of the cycles before it. For frequency
    /// events use more cycles, e.g. --cycles 5 --step 100ms.
    ///
    /// Examples:
    ///   sprcmtrd frequency fault.cfg
    ///   sprcmtrd frequency event.cfg --channel VA --cycles 5 --step 100ms --format csv
    #[command(visible_alias = "freq", verbatim_doc_comment)]
    Frequency(FrequencyArgs),

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

    /// List the color themes, or print one as a template for your own.
    ///
    /// Your themes are `<name>.toml` files in the `themes` directory next to
    /// the configuration file (`~/.config/sprcmtrd/themes/`).
    Themes(ThemesArgs),
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
    /// Channels and time window.
    #[command(flatten)]
    pub selection: Selection,
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

/// Arguments of `fault`.
#[derive(Debug, Args)]
pub struct FaultArgs {
    /// Any file of the recording (.cfg, .dat, .cff, ...).
    #[arg(value_hint = ValueHint::FilePath, add = ArgValueCompleter::new(recordings))]
    pub file: PathBuf,
    /// Current channels of phases A, B, C (indices or ids). Default: found by unit.
    #[arg(long, value_delimiter = ',', num_args = 1)]
    pub current: Vec<String>,
    /// Voltage channels of phases A, B, C (indices or ids). Default: found by unit, if any.
    #[arg(long, value_delimiter = ',', num_args = 1)]
    pub voltage: Vec<String>,
    /// Binary channel of the protection start (`dN` or id). Default: found by name.
    #[arg(long)]
    pub start: Option<String>,
    /// Binary channel of the trip command (`dN` or id). Default: found by name.
    #[arg(long)]
    pub trip: Option<String>,
    /// Binary channel of the breaker position (`dN` or id); open is a change to 1,
    /// or to 0 when the id contains CLOSE. Default: found by name.
    #[arg(long)]
    pub breaker: Option<String>,
    /// Start of the search in ms relative to the trigger.
    #[arg(long, allow_hyphen_values = true)]
    pub from: Option<f64>,
    /// End of the search in ms relative to the trigger.
    #[arg(long, allow_hyphen_values = true)]
    pub to: Option<f64>,
    /// Fundamental frequency in Hz. Default: the line frequency of the recording.
    #[arg(long)]
    pub frequency: Option<f64>,
    /// Print JSON instead of text.
    #[arg(long)]
    pub json: bool,
}

/// Arguments of `dc-offset`.
#[derive(Debug, Args)]
pub struct DcOffsetArgs {
    /// Any file of the recording (.cfg, .dat, .cff, ...).
    #[arg(value_hint = ValueHint::FilePath, add = ArgValueCompleter::new(recordings))]
    pub file: PathBuf,
    /// Fit window from inception in cycles of the fundamental frequency (at least 1).
    #[arg(long, default_value_t = 3.0, value_parser = parse_fit_cycles)]
    pub cycles: f64,
    /// Current channels of phases A, B, C (indices or ids). Default: found by unit.
    #[arg(long, value_delimiter = ',', num_args = 1)]
    pub current: Vec<String>,
    /// Start of the fault search in ms relative to the trigger.
    #[arg(long, allow_hyphen_values = true)]
    pub from: Option<f64>,
    /// End of the fault search in ms relative to the trigger.
    #[arg(long, allow_hyphen_values = true)]
    pub to: Option<f64>,
    /// Fundamental frequency in Hz. Default: the line frequency of the recording.
    #[arg(long)]
    pub frequency: Option<f64>,
    /// Print JSON instead of text.
    #[arg(long)]
    pub json: bool,
}

/// Parses the `dc-offset` fit window: a number of cycles of at least 1.
/// A number of cycles above 0 (`--cycles`).
fn parse_cycles(s: &str) -> Result<f64, String> {
    let v: f64 = s.trim().parse().map_err(|_| format!("'{s}' is not a number"))?;
    if v.is_finite() && v > 0.0 {
        Ok(v)
    } else {
        Err(format!("the number of cycles must be above 0, got {s}"))
    }
}

fn parse_fit_cycles(s: &str) -> Result<f64, String> {
    let v: f64 = s.trim().parse().map_err(|_| format!("'{s}' is not a number"))?;
    if v.is_finite() && v >= 1.0 {
        Ok(v)
    } else {
        Err(format!("the fit needs at least 1 cycle, got {s}"))
    }
}

/// Arguments of `compare`.
#[derive(Debug, Args)]
pub struct CompareArgs {
    /// Recording A (any file of it: .cfg, .dat, .cff, ...); times refer to A.
    #[arg(value_hint = ValueHint::FilePath, add = ArgValueCompleter::new(recordings))]
    pub a: PathBuf,
    /// Recording B, compared with A.
    #[arg(value_hint = ValueHint::FilePath, add = ArgValueCompleter::new(recordings))]
    pub b: PathBuf,
    /// End of the window in ms relative to A's trigger (without --trend / --plot).
    #[arg(long, allow_hyphen_values = true, default_value_t = 0.0)]
    pub at: f64,
    /// Window length in cycles of the fundamental frequency.
    #[arg(long, default_value_t = 1.0, value_parser = parse_cycles)]
    pub cycles: f64,
    /// Ratio, angle difference and differential current over time instead of
    /// at one moment.
    #[arg(long, conflicts_with = "plot")]
    pub trend: bool,
    /// Waveforms of each pair, A above B on the same scale, in A's time.
    #[arg(long)]
    pub plot: bool,
    /// With --plot: a third plot per pair, B − A on its own scale (B
    /// interpolated linearly onto A's samples).
    #[arg(long, requires = "plot")]
    pub difference: bool,
    /// Start of the time range in ms relative to A's trigger (with --trend / --plot).
    #[arg(long, allow_hyphen_values = true)]
    pub from: Option<f64>,
    /// End of the time range in ms relative to A's trigger (with --trend / --plot).
    #[arg(long, allow_hyphen_values = true)]
    pub to: Option<f64>,
    /// Distance between rows with --trend: cycles (`1`) or a time (`10ms`).
    #[arg(long, default_value = "1", value_parser = parse_step)]
    pub step: TrendStep,
    /// Plot width in columns with --plot (default: terminal width).
    #[arg(long)]
    pub width: Option<u16>,
    /// Height of each plot in rows with --plot.
    #[arg(long, default_value_t = 8)]
    pub height: u16,
    /// How the time axes are aligned.
    #[arg(long, value_enum, default_value_t = AlignArg::Time)]
    pub align: AlignArg,
    /// Moves B later by this many ms, after the alignment.
    #[arg(long, allow_hyphen_values = true, default_value_t = 0.0)]
    pub shift: f64,
    /// Only these channels of A (indices or ids). Default: all pairs.
    #[arg(long, value_delimiter = ',')]
    pub channels: Vec<String>,
    /// Channel pairs with different ids: `A_ID=B_ID`, comma separated. Default: equal ids.
    #[arg(long, value_delimiter = ',', value_parser = parse_pair)]
    pub pair: Vec<(String, String)>,
    /// Channel of A the angles refer to (index or id). Default: the first pair.
    #[arg(long)]
    pub reference: Option<String>,
    /// Differential current of two line ends from the current groups of A and B.
    #[arg(long)]
    pub differential: bool,
    /// Current channels of phases A, B, C in A (indices or ids). Default: found by unit.
    #[arg(long, value_delimiter = ',', num_args = 1)]
    pub current_a: Vec<String>,
    /// Current channels of phases A, B, C in B (indices or ids). Default: found by unit.
    #[arg(long, value_delimiter = ',', num_args = 1)]
    pub current_b: Vec<String>,
    /// B's currents are measured out of the line: turned for --differential
    /// and in --plot.
    #[arg(long)]
    pub invert_b: bool,
    /// Fundamental frequency in Hz. Default: the line frequency of A.
    #[arg(long)]
    pub frequency: Option<f64>,
    /// Output format (csv only with --trend, text only with --plot).
    #[arg(long, value_enum, default_value_t = TableFormat::Text)]
    pub format: TableFormat,
}

/// Time alignment of `compare`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub enum AlignArg {
    /// By the absolute trigger times (synchronized clocks).
    Time,
    /// Both triggers at 0.
    Trigger,
    /// The fault inception of A on that of B (as the fault command finds it).
    Fault,
}

/// Parses an `A_ID=B_ID` channel pair.
fn parse_pair(s: &str) -> Result<(String, String), String> {
    match s.split_once('=') {
        Some((a, b)) if !a.trim().is_empty() && !b.trim().is_empty() => Ok((a.trim().to_owned(), b.trim().to_owned())),
        _ => Err(format!("expected A_ID=B_ID, got '{s}'")),
    }
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
    /// Time window.
    #[command(flatten)]
    pub window: TimeWindow,
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
    #[arg(long, default_value_t = 1.0, value_parser = parse_cycles)]
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

/// Arguments of `locate`.
#[derive(Debug, Args)]
pub struct LocateArgs {
    /// Any file of the recording (.cfg, .dat, .cff, ...).
    #[arg(value_hint = ValueHint::FilePath, add = ArgValueCompleter::new(recordings))]
    pub file: PathBuf,
    /// Positive sequence impedance of the line in Ω/km primary: `R+jX` or `MAG@ANGLE`.
    #[arg(long, value_parser = parse_line_impedance, allow_hyphen_values = true)]
    pub z1: LineImpedance,
    /// Zero sequence impedance of the line in Ω/km primary (needed for ground faults).
    #[arg(long, value_parser = parse_line_impedance, allow_hyphen_values = true)]
    pub z0: Option<LineImpedance>,
    /// Length of the line in km.
    #[arg(long, value_parser = parse_length)]
    pub length: f64,
    /// Loop to measure (`AN`, `BC`, ...). Default: from the faulted phases.
    #[arg(long = "loop")]
    pub fault_loop: Option<String>,
    /// End of the measurement window in ms relative to the trigger. Default: the middle of the fault.
    #[arg(long, allow_hyphen_values = true)]
    pub at: Option<f64>,
    /// Measurement window length in cycles of the fundamental frequency.
    #[arg(long, default_value_t = 1.0, value_parser = parse_cycles)]
    pub cycles: f64,
    /// Start of the fault search in ms relative to the trigger.
    #[arg(long, allow_hyphen_values = true)]
    pub from: Option<f64>,
    /// End of the fault search in ms relative to the trigger.
    #[arg(long, allow_hyphen_values = true)]
    pub to: Option<f64>,
    /// Voltage channels of phases A, B, C to neutral (indices or ids). Default: found by unit.
    #[arg(long, value_delimiter = ',', num_args = 1)]
    pub voltage: Vec<String>,
    /// Current channels of phases A, B, C (indices or ids). Default: found by unit.
    #[arg(long, value_delimiter = ',', num_args = 1)]
    pub current: Vec<String>,
    /// Fundamental frequency in Hz. Default: the line frequency of the recording.
    #[arg(long)]
    pub frequency: Option<f64>,
    /// Print JSON instead of text.
    #[arg(long)]
    pub json: bool,
}

/// A line impedance per km: resistance and reactance in Ω/km.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct LineImpedance {
    /// Resistance.
    pub r: f64,
    /// Reactance.
    pub x: f64,
}

/// Splits `MAG@ANGLE` (degrees; `MAG` alone is angle 0) into its numbers.
fn parse_polar(s: &str) -> Option<(f64, f64)> {
    let (m, a) = s.split_once('@').unwrap_or((s, "0"));
    Some((m.trim().parse().ok()?, a.trim().parse().ok()?))
}

/// Parses `R+jX` / `R-jX` (also `R+Xj`, spaces allowed) into R and X.
fn parse_rectangular(s: &str) -> Option<(f64, f64)> {
    let s: String = s.chars().filter(|c| !c.is_whitespace()).collect();
    // The sign before the imaginary part, not the one of an exponent or of R.
    let split = s
        .char_indices()
        .skip(1)
        .find(|&(i, c)| (c == '+' || c == '-') && !s[..i].ends_with(['e', 'E']))?
        .0;
    let (r, x) = s.split_at(split);
    let (sign, x) = x.split_at(1);
    let x = x.strip_prefix('j').or_else(|| x.strip_suffix('j'))?;
    let x: f64 = x.parse().ok()?;
    Some((r.parse().ok()?, if sign == "-" { -x } else { x }))
}

/// Parses `--z1` / `--z0`: `R+jX` or `MAG@ANGLE` in Ω/km, with a reactance above 0.
fn parse_line_impedance(s: &str) -> Result<LineImpedance, String> {
    let (r, x) = if s.contains('j') {
        parse_rectangular(s)
    } else {
        parse_polar(s).map(|(m, a)| (m * a.to_radians().cos(), m * a.to_radians().sin()))
    }
    .ok_or_else(|| format!("expected R+jX or MAG@ANGLE, got '{s}'"))?;
    if !(x > 0.0 && r.is_finite() && x.is_finite()) {
        return Err(format!("the reactance of a line must be above 0, got '{s}'"));
    }
    Ok(LineImpedance { r, x })
}

/// Parses `--length`: km above 0.
fn parse_length(s: &str) -> Result<f64, String> {
    match s.trim().parse::<f64>() {
        Ok(v) if v > 0.0 && v.is_finite() => Ok(v),
        _ => Err(format!("expected a length in km above 0, got '{s}'")),
    }
}

/// Arguments of `impedance`.
#[derive(Debug, Args)]
pub struct ImpedanceArgs {
    /// Any file of the recording (.cfg, .dat, .cff, ...).
    #[arg(value_hint = ValueHint::FilePath, add = ArgValueCompleter::new(recordings))]
    pub file: PathBuf,
    /// End of the window in ms relative to the trigger (without --trend).
    #[arg(long, allow_hyphen_values = true, default_value_t = 0.0)]
    pub at: f64,
    /// Window length in cycles of the fundamental frequency.
    #[arg(long, default_value_t = 1.0, value_parser = parse_cycles)]
    pub cycles: f64,
    /// Voltage channels of phases A, B, C (indices or ids). Default: found by unit.
    #[arg(long, value_delimiter = ',', num_args = 1)]
    pub voltage: Vec<String>,
    /// Current channels of phases A, B, C (indices or ids). Default: found by unit.
    #[arg(long, value_delimiter = ',', num_args = 1)]
    pub current: Vec<String>,
    /// Residual compensation k0 = (Z0 − Z1) / (3·Z1) as magnitude, optionally
    /// with angle in degrees (`0.8` or `0.8@-5`). Default 0: no compensation.
    #[arg(long, default_value = "0", value_parser = parse_k0, allow_hyphen_values = true)]
    pub k0: K0,
    /// Loops to show (`AN,BN,CN,AB,BC,CA`). Default: all six.
    #[arg(long, value_delimiter = ',')]
    pub loops: Vec<String>,
    /// R and X of each loop over time instead of at one moment.
    #[arg(long)]
    pub trend: bool,
    /// Start of the time range in ms relative to the trigger (with --trend).
    #[arg(long, allow_hyphen_values = true)]
    pub from: Option<f64>,
    /// End of the time range in ms relative to the trigger (with --trend).
    #[arg(long, allow_hyphen_values = true)]
    pub to: Option<f64>,
    /// Distance between rows with --trend: cycles (`1`) or a time (`10ms`).
    #[arg(long, default_value = "1", value_parser = parse_step)]
    pub step: TrendStep,
    /// Fundamental frequency in Hz. Default: the line frequency of the recording.
    #[arg(long)]
    pub frequency: Option<f64>,
    /// Output format.
    #[arg(long, value_enum, default_value_t = TableFormat::Text)]
    pub format: TableFormat,
}

/// Arguments of `power`.
#[derive(Debug, Args)]
pub struct PowerArgs {
    /// Any file of the recording (.cfg, .dat, .cff, ...).
    #[arg(value_hint = ValueHint::FilePath, add = ArgValueCompleter::new(recordings))]
    pub file: PathBuf,
    /// End of the window in ms relative to the trigger (without --trend).
    #[arg(long, allow_hyphen_values = true, default_value_t = 0.0)]
    pub at: f64,
    /// Window length in cycles of the fundamental frequency.
    #[arg(long, default_value_t = 1.0, value_parser = parse_cycles)]
    pub cycles: f64,
    /// Voltage channels of phases A, B, C to neutral (indices or ids). Default: found by unit.
    #[arg(long, value_delimiter = ',', num_args = 1)]
    pub voltage: Vec<String>,
    /// Current channels of phases A, B, C (indices or ids). Default: found by unit.
    #[arg(long, value_delimiter = ',', num_args = 1)]
    pub current: Vec<String>,
    /// Power over time instead of at one moment.
    #[arg(long)]
    pub trend: bool,
    /// With --trend in text: P and Q of each phase too, not only the totals.
    #[arg(long)]
    pub phases: bool,
    /// Start of the time range in ms relative to the trigger (with --trend).
    #[arg(long, allow_hyphen_values = true)]
    pub from: Option<f64>,
    /// End of the time range in ms relative to the trigger (with --trend).
    #[arg(long, allow_hyphen_values = true)]
    pub to: Option<f64>,
    /// Distance between rows with --trend: cycles (`1`) or a time (`10ms`).
    #[arg(long, default_value = "1", value_parser = parse_step)]
    pub step: TrendStep,
    /// Fundamental frequency in Hz. Default: the line frequency of the recording.
    #[arg(long)]
    pub frequency: Option<f64>,
    /// Output format.
    #[arg(long, value_enum, default_value_t = TableFormat::Text)]
    pub format: TableFormat,
}

/// Residual compensation factor of `impedance`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct K0 {
    /// Magnitude.
    pub magnitude: f64,
    /// Angle in degrees.
    pub angle: f64,
}

/// Parses `--k0`: `MAG` or `MAG@ANGLE` (degrees).
fn parse_k0(s: &str) -> Result<K0, String> {
    let (magnitude, angle) = parse_polar(s).ok_or_else(|| format!("expected MAG or MAG@ANGLE, got '{s}'"))?;
    let k0 = K0 { magnitude, angle };
    if k0.magnitude < 0.0 {
        return Err("the magnitude of k0 can't be negative".into());
    }
    Ok(k0)
}

/// Arguments of `harmonics`.
#[derive(Debug, Args)]
pub struct HarmonicsArgs {
    /// Any file of the recording (.cfg, .dat, .cff, ...).
    #[arg(value_hint = ValueHint::FilePath, add = ArgValueCompleter::new(recordings))]
    pub file: PathBuf,
    /// End of the window in ms relative to the trigger.
    #[arg(long, allow_hyphen_values = true, default_value_t = 0.0)]
    pub at: f64,
    /// Window length in cycles of the fundamental frequency.
    #[arg(long, default_value_t = 1.0, value_parser = parse_cycles)]
    pub cycles: f64,
    /// Analog channels: comma separated 1-based indices or ids. Default: all.
    #[arg(long, value_delimiter = ',')]
    pub channels: Vec<String>,
    /// Highest harmonic order (limited to below half the sample rate).
    #[arg(long, default_value_t = 50, value_parser = clap::value_parser!(u32).range(2..))]
    pub max_order: u32,
    /// Fundamental frequency in Hz. Default: the line frequency of the recording.
    #[arg(long)]
    pub frequency: Option<f64>,
    /// Also show the angle of each harmonic (relative to the fundamental).
    #[arg(long)]
    pub angles: bool,
    /// Print JSON instead of text.
    #[arg(long)]
    pub json: bool,
}

/// Arguments of `trend`.
#[derive(Debug, Args)]
pub struct TrendArgs {
    /// Any file of the recording (.cfg, .dat, .cff, ...).
    #[arg(value_hint = ValueHint::FilePath, add = ArgValueCompleter::new(recordings))]
    pub file: PathBuf,
    /// Analog channels: comma separated 1-based indices or ids. Default: all.
    #[arg(long, value_delimiter = ',')]
    pub channels: Vec<String>,
    /// Start of the time range in ms relative to the trigger.
    #[arg(long, allow_hyphen_values = true)]
    pub from: Option<f64>,
    /// End of the time range in ms relative to the trigger.
    #[arg(long, allow_hyphen_values = true)]
    pub to: Option<f64>,
    /// Distance between rows: cycles (`1`, `0.5`) or a time (`10ms`, `1s`).
    #[arg(long, default_value = "1", value_parser = parse_step)]
    pub step: TrendStep,
    /// Window length in cycles of the fundamental frequency.
    #[arg(long, default_value_t = 1.0, value_parser = parse_cycles)]
    pub cycles: f64,
    /// Channel the angles refer to (one of the shown channels). Default: the
    /// first channel.
    #[arg(long)]
    pub reference: Option<String>,
    /// Channel to measure the frequency from (index or id, any analog
    /// channel). Default: the positive sequence of the strongest voltage
    /// group with a signal, else of the strongest current group, else the
    /// strongest single voltage or current.
    #[arg(long)]
    pub frequency_from: Option<String>,
    /// Fundamental frequency in Hz. Default: the line frequency of the recording.
    #[arg(long)]
    pub frequency: Option<f64>,
    /// Also show the angle of each channel.
    #[arg(long)]
    pub angles: bool,
    /// Also show the true RMS of each channel (with harmonics and DC).
    #[arg(long)]
    pub rms: bool,
    /// Output format.
    #[arg(long, value_enum, default_value_t = TableFormat::Text)]
    pub format: TableFormat,
}

/// Arguments of `frequency`.
#[derive(Debug, Args)]
pub struct FrequencyArgs {
    /// Any file of the recording (.cfg, .dat, .cff, ...).
    #[arg(value_hint = ValueHint::FilePath, add = ArgValueCompleter::new(recordings))]
    pub file: PathBuf,
    /// Analog channel to measure (index or id). Default: the first voltage
    /// channel, else the first analog channel.
    #[arg(long)]
    pub channel: Option<String>,
    /// How the frequency is measured.
    #[arg(long, value_enum, default_value_t = FrequencyMethod::Crossings)]
    pub method: FrequencyMethod,
    /// Start of the time range in ms relative to the trigger.
    #[arg(long, allow_hyphen_values = true)]
    pub from: Option<f64>,
    /// End of the time range in ms relative to the trigger.
    #[arg(long, allow_hyphen_values = true)]
    pub to: Option<f64>,
    /// Distance between rows: cycles (`1`, `0.5`) or a time (`10ms`, `1s`).
    #[arg(long, default_value = "1", value_parser = parse_step)]
    pub step: TrendStep,
    /// Measurement length in cycles of the nominal frequency (whole cycles
    /// with --method crossings).
    #[arg(long, default_value_t = 1.0, value_parser = parse_cycles)]
    pub cycles: f64,
    /// Nominal frequency in Hz. Default: the line frequency of the recording.
    #[arg(long)]
    pub frequency: Option<f64>,
    /// Output format.
    #[arg(long, value_enum, default_value_t = TableFormat::Text)]
    pub format: TableFormat,
}

/// Measuring method of `frequency`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub enum FrequencyMethod {
    /// Periods between rising zero crossings.
    Crossings,
    /// Turn of the fundamental phasor between windows.
    Phasor,
}

/// Distance between the rows of `trend`, `frequency` and `impedance --trend`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum TrendStep {
    /// In cycles of the fundamental frequency.
    Cycles(f64),
    /// In seconds.
    Seconds(f64),
}

/// Output format of `trend`, `frequency` and `impedance`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub enum TableFormat {
    /// Aligned table.
    Text,
    /// Comma separated values with a header row.
    Csv,
    /// One JSON object.
    Json,
}

/// Parses `--step`: a bare number is cycles, with a unit a time (`10ms`).
fn parse_step(s: &str) -> Result<TrendStep, String> {
    let step = match s.trim().parse::<f64>() {
        Ok(cycles) => TrendStep::Cycles(cycles),
        Err(_) => TrendStep::Seconds(parse_duration(s)?),
    };
    match step {
        TrendStep::Cycles(v) | TrendStep::Seconds(v) if v > 0.0 => Ok(step),
        _ => Err("the step must be above 0".into()),
    }
}

/// Arguments of `stats`.
#[derive(Debug, Args)]
pub struct StatsArgs {
    /// Any file of the recording (.cfg, .dat, .cff, ...).
    #[arg(value_hint = ValueHint::FilePath, add = ArgValueCompleter::new(recordings))]
    pub file: PathBuf,
    /// Channels and time window.
    #[command(flatten)]
    pub selection: Selection,
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
    /// Channels and time window.
    #[command(flatten)]
    pub selection: Selection,
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
    /// Channels and time window.
    #[command(flatten)]
    pub selection: Selection,
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

/// Arguments of `themes`.
#[derive(Debug, Args)]
pub struct ThemesArgs {
    /// Print theme NAME as a complete theme file.
    #[arg(long, value_name = "NAME", add = ArgValueCandidates::new(theme_names))]
    pub print: Option<String>,
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

        assert_eq!(
            parse_k0("0.8"),
            Ok(K0 {
                magnitude: 0.8,
                angle: 0.0
            })
        );
        assert_eq!(
            parse_k0("0.8@-5"),
            Ok(K0 {
                magnitude: 0.8,
                angle: -5.0
            })
        );
        assert!(parse_k0("x").is_err() && parse_k0("-1").is_err());

        let z = |r, x| Ok(LineImpedance { r, x });
        assert_eq!(parse_line_impedance("0.03+j0.33"), z(0.03, 0.33));
        assert_eq!(parse_line_impedance(" 0.03 + 0.33j"), z(0.03, 0.33));
        assert_eq!(parse_line_impedance("-0.01+j0.3"), z(-0.01, 0.3));
        assert_eq!(parse_line_impedance("3e-2+j3.3e-1"), z(0.03, 0.33));
        let polar = parse_line_impedance("2@90").unwrap();
        assert!(polar.r.abs() < 1e-12 && (polar.x - 2.0).abs() < 1e-12);
        assert!(parse_line_impedance("0.03-j0.33").is_err());
        assert!(parse_line_impedance("0.03+k0.33").is_err());
        assert!(parse_line_impedance("1@0").is_err());
        assert_eq!(parse_length("42.5"), Ok(42.5));
        assert!(parse_length("0").is_err() && parse_length("-3").is_err());

        assert_eq!(parse_step("1"), Ok(TrendStep::Cycles(1.0)));
        assert_eq!(parse_step("0.5"), Ok(TrendStep::Cycles(0.5)));
        assert_eq!(parse_step("10ms"), Ok(TrendStep::Seconds(0.01)));
        assert!(parse_step("0").is_err());
        assert!(parse_step("10 apples").is_err());
        assert!(parse_duration("-1s").is_err());
    }

    #[test]
    fn parses_path_or_subcommand() {
        let c = Cli::parse_from(["sprcmtrd", "x.cfg", "--theme", "light"]);
        assert!(c.command.is_none());
        assert_eq!(c.global.theme, Some(ThemeChoice::named("light")));
        let c = Cli::parse_from(["sprcmtrd", "view", "x.cfg", "--from", "-20", "--channels", "IA,d1"]);
        let Some(Command::View(ViewArgs { selection, .. })) = c.command else {
            panic!()
        };
        assert_eq!(selection.window.from, Some(-20.0));
        assert_eq!(selection.window.to, None);
        assert_eq!(selection.channels, ["IA", "d1"]);
        let c = Cli::parse_from(["sprcmtrd", "completions", "zsh", "-o", "_sprcmtrd"]);
        let Some(Command::Completions(CompletionsArgs { shell, output })) = c.command else {
            panic!()
        };
        assert_eq!(shell, clap_complete::Shell::Zsh);
        assert_eq!(output, Some(PathBuf::from("_sprcmtrd")));
    }
}
