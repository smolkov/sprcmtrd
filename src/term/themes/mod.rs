//! Color themes as TOML files: the built-in ones compiled in from
//! `src/term/themes/*.toml`, the user's in `<config dir>/themes/<name>.toml`.
//! Both the TUI (colors) and the commands (`themes`, the dark or light
//! output colors) use them, so they live here and not in `tui`.
//!
//! ```toml
//! kind = "dark"              # dark | light
//! extends = "nord"           # optional: missing roles come from this theme
//!
//! [colors]                   # "#rrggbb", ANSI name, 0-255 or "reset"
//! accent = "#ebcb8b"
//!
//! [style]
//! border = "rounded"         # plain | rounded | thick | double
//! muted = ["italic"]         # bold | italic | dim | underlined | reversed
//! ```
//!
//! Missing roles come from `extends`, or else from the built-in `dark` or
//! `light` theme of the same `kind`.

use std::fmt::Write as _;
use std::path::{Path, PathBuf};
use std::str::FromStr;

use anyhow::{Context as _, bail};
use ratatui::style::{Color, Modifier};
use serde::Deserialize;

use super::ThemeKind;

/// The built-in themes in the order `T` goes through them.
const BUILTIN: &[(&str, &str)] = &[
    ("dark", include_str!("dark.toml")),
    ("light", include_str!("light.toml")),
    ("atom-one-dark", include_str!("atom-one-dark.toml")),
    ("nord", include_str!("nord.toml")),
    ("nord-night", include_str!("nord-night.toml")),
    ("solarized-dark", include_str!("solarized-dark.toml")),
    ("tokyonight", include_str!("tokyonight.toml")),
];

/// Longest `extends` chain; longer ones are taken for a cycle.
const MAX_DEPTH: usize = 8;

/// Border lines of the blocks.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Border {
    /// `┌─┐`
    Plain,
    /// `╭─╮`
    Rounded,
    /// `┏━┓`
    Thick,
    /// `╔═╗`
    Double,
}

impl Border {
    fn name(self) -> &'static str {
        match self {
            Self::Plain => "plain",
            Self::Rounded => "rounded",
            Self::Thick => "thick",
            Self::Double => "double",
        }
    }
}

/// Text modifiers a theme may set for a role.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
enum StyleName {
    Bold,
    Italic,
    Dim,
    Underlined,
    Reversed,
}

/// The modifiers and their names, for reading and writing theme files.
const MODIFIERS: [(Modifier, &str); 5] = [
    (Modifier::BOLD, "bold"),
    (Modifier::ITALIC, "italic"),
    (Modifier::DIM, "dim"),
    (Modifier::UNDERLINED, "underlined"),
    (Modifier::REVERSED, "reversed"),
];

impl StyleName {
    fn modifier(self) -> Modifier {
        match self {
            Self::Bold => Modifier::BOLD,
            Self::Italic => Modifier::ITALIC,
            Self::Dim => Modifier::DIM,
            Self::Underlined => Modifier::UNDERLINED,
            Self::Reversed => Modifier::REVERSED,
        }
    }
}

/// Colors of all roles.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Colors {
    /// Background.
    pub background: Color,
    /// Normal text.
    pub text: Color,
    /// Less important text.
    pub muted: Color,
    /// Borders of unfocused blocks.
    pub border: Color,
    /// Focused borders, titles, highlights.
    pub accent: Color,
    /// Background of the selected item.
    pub selection: Color,
    /// Alternating backgrounds of the analog plots: lighter, darker.
    pub plot_shades: [Color; 2],
    /// Warnings and abnormal binary states.
    pub warning: Color,
    /// Errors.
    pub error: Color,
    /// Trigger marker.
    pub trigger: Color,
    /// Plot cursor.
    pub cursor: Color,
    /// Phases A, B, C and N.
    pub phase: [Color; 4],
    /// Channels without a known phase.
    pub palette: [Color; 8],
}

impl Colors {
    /// Every color, for mapping them all at once.
    fn all_mut(&mut self) -> impl Iterator<Item = &mut Color> {
        [
            &mut self.background,
            &mut self.text,
            &mut self.muted,
            &mut self.border,
            &mut self.accent,
            &mut self.selection,
            &mut self.warning,
            &mut self.error,
            &mut self.trigger,
            &mut self.cursor,
        ]
        .into_iter()
        .chain(&mut self.plot_shades)
        .chain(&mut self.phase)
        .chain(&mut self.palette)
    }
}

/// Text modifiers of the roles.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Styles {
    /// Title of the focused block.
    pub title: Modifier,
    /// Highlights.
    pub accent: Modifier,
    /// Selected item.
    pub selection: Modifier,
    /// Warnings.
    pub warning: Modifier,
    /// Errors.
    pub error: Modifier,
    /// Less important text.
    pub muted: Modifier,
}

/// A complete theme, read from its file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ThemeSpec {
    /// Dark or light background.
    pub kind: ThemeKind,
    /// Colors of the roles.
    pub colors: Colors,
    /// Modifiers of the roles.
    pub styles: Styles,
    /// Border lines.
    pub border: Border,
}

impl ThemeSpec {
    /// The theme with every RGB color replaced by the nearest of the 256
    /// colors, for terminals without truecolor.
    pub fn to_256_colors(mut self) -> Self {
        for c in self.colors.all_mut() {
            if let Color::Rgb(r, g, b) = *c {
                *c = Color::Indexed(nearest_256(r, g, b));
            }
        }
        self
    }

    /// The complete theme file (all roles), a template for a custom theme.
    pub fn to_toml(&self) -> String {
        let c = &self.colors;
        let s = &self.styles;
        let list = |colors: &[Color]| {
            colors
                .iter()
                .map(|&c| format!("\"{}\"", color_name(c)))
                .collect::<Vec<_>>()
        };
        let mut out = format!("kind = \"{}\"\n\n[colors]\n", self.kind.name());
        for (name, color) in [
            ("background", c.background),
            ("text", c.text),
            ("muted", c.muted),
            ("border", c.border),
            ("accent", c.accent),
            ("selection", c.selection),
        ] {
            let _ = writeln!(out, "{name} = \"{}\"", color_name(color));
        }
        let _ = writeln!(out, "plot_shades = [{}]", list(&c.plot_shades).join(", "));
        for (name, color) in [
            ("warning", c.warning),
            ("error", c.error),
            ("trigger", c.trigger),
            ("cursor", c.cursor),
        ] {
            let _ = writeln!(out, "{name} = \"{}\"", color_name(color));
        }
        let _ = writeln!(out, "phase = [{}]", list(&c.phase).join(", "));
        let _ = writeln!(out, "palette = [{}]", list(&c.palette).join(", "));
        let _ = write!(out, "\n[style]\nborder = \"{}\"\n", self.border.name());
        for (name, m) in [
            ("title", s.title),
            ("accent", s.accent),
            ("selection", s.selection),
            ("warning", s.warning),
            ("error", s.error),
            ("muted", s.muted),
        ] {
            let names: Vec<String> = MODIFIERS
                .iter()
                .filter(|(flag, _)| m.contains(*flag))
                .map(|(_, n)| format!("\"{n}\""))
                .collect();
            let _ = writeln!(out, "{name} = [{}]", names.join(", "));
        }
        out
    }
}

/// `true` when the terminal says it shows 24-bit colors (`COLORTERM`).
pub fn truecolor() -> bool {
    std::env::var("COLORTERM").is_ok_and(|v| matches!(v.to_ascii_lowercase().as_str(), "truecolor" | "24bit"))
}

/// Nearest color of the 6×6×6 cube and the gray ramp (16-255); the first 16
/// are left out because the terminal's scheme defines them.
fn nearest_256(r: u8, g: u8, b: u8) -> u8 {
    const LEVELS: [u8; 6] = [0, 95, 135, 175, 215, 255];
    let dist = |(r2, g2, b2): (u8, u8, u8)| {
        let d = |a: u8, b: u8| (i32::from(a) - i32::from(b)).pow(2);
        d(r, r2) + d(g, g2) + d(b, b2)
    };
    let level = |v: u8| {
        (0..6)
            .min_by_key(|&i| (i32::from(LEVELS[i]) - i32::from(v)).abs())
            .expect("six levels")
    };
    let (ri, gi, bi) = (level(r), level(g), level(b));
    let cube = 16 + 36 * ri + 6 * gi + bi;
    let cube_dist = dist((LEVELS[ri], LEVELS[gi], LEVELS[bi]));
    let (gray, gray_dist) = (0..24)
        .map(|i| {
            let v = 8 + 10 * i as u8;
            (232 + i, dist((v, v, v)))
        })
        .min_by_key(|&(_, d)| d)
        .expect("24 grays");
    if gray_dist < cube_dist { gray as u8 } else { cube as u8 }
}

/// A color as theme files write it: `#rrggbb`, an index or a kebab-case name.
fn color_name(c: Color) -> String {
    match c {
        Color::Rgb(r, g, b) => format!("#{r:02x}{g:02x}{b:02x}"),
        Color::Indexed(i) => i.to_string(),
        named => {
            let mut out = String::new();
            for (i, ch) in named.to_string().chars().enumerate() {
                if ch.is_ascii_uppercase() && i > 0 {
                    out.push('-');
                }
                out.push(ch.to_ascii_lowercase());
            }
            out
        }
    }
}

/// A theme file as written: every role optional.
#[derive(Debug, Default, Deserialize)]
#[serde(deny_unknown_fields)]
struct ThemeFile {
    kind: Option<ThemeKind>,
    extends: Option<String>,
    #[serde(default)]
    colors: ColorsFile,
    #[serde(default)]
    style: StyleFile,
}

#[derive(Debug, Default, Deserialize)]
#[serde(deny_unknown_fields)]
struct ColorsFile {
    background: Option<String>,
    text: Option<String>,
    muted: Option<String>,
    border: Option<String>,
    accent: Option<String>,
    selection: Option<String>,
    plot_shades: Option<[String; 2]>,
    warning: Option<String>,
    error: Option<String>,
    trigger: Option<String>,
    cursor: Option<String>,
    phase: Option<[String; 4]>,
    palette: Option<[String; 8]>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(deny_unknown_fields)]
struct StyleFile {
    border: Option<Border>,
    title: Option<Vec<StyleName>>,
    accent: Option<Vec<StyleName>>,
    selection: Option<Vec<StyleName>>,
    warning: Option<Vec<StyleName>>,
    error: Option<Vec<StyleName>>,
    muted: Option<Vec<StyleName>>,
}

/// A theme with every role optional and the colors parsed; filled from its
/// parent until complete.
#[derive(Debug, Default)]
struct Partial {
    kind: Option<ThemeKind>,
    colors: [Option<Color>; 10],
    plot_shades: Option<[Color; 2]>,
    phase: Option<[Color; 4]>,
    palette: Option<[Color; 8]>,
    styles: [Option<Modifier>; 6],
    border: Option<Border>,
}

/// Names of the single colors in `Partial::colors`.
const COLOR_ROLES: [&str; 10] = [
    "background",
    "text",
    "muted",
    "border",
    "accent",
    "selection",
    "warning",
    "error",
    "trigger",
    "cursor",
];

fn parse_color(role: &str, s: &str) -> anyhow::Result<Color> {
    Color::from_str(s.trim()).map_err(|_| {
        anyhow::anyhow!("invalid color \"{s}\" for {role} (use \"#rrggbb\", an ANSI name, 0-255 or \"reset\")")
    })
}

fn parse_colors<const N: usize>(role: &str, list: Option<[String; N]>) -> anyhow::Result<Option<[Color; N]>> {
    let Some(list) = list else {
        return Ok(None);
    };
    let mut out = [Color::Reset; N];
    for (o, s) in out.iter_mut().zip(&list) {
        *o = parse_color(role, s)?;
    }
    Ok(Some(out))
}

impl ThemeFile {
    fn partial(self) -> anyhow::Result<Partial> {
        let c = self.colors;
        let singles = [
            c.background,
            c.text,
            c.muted,
            c.border,
            c.accent,
            c.selection,
            c.warning,
            c.error,
            c.trigger,
            c.cursor,
        ];
        let mut colors = [None; 10];
        for ((out, value), role) in colors.iter_mut().zip(singles).zip(COLOR_ROLES) {
            *out = value.map(|s| parse_color(role, &s)).transpose()?;
        }
        let s = self.style;
        let styles = [s.title, s.accent, s.selection, s.warning, s.error, s.muted]
            .map(|v| v.map(|names| names.iter().fold(Modifier::empty(), |m, n| m | n.modifier())));
        Ok(Partial {
            kind: self.kind,
            colors,
            plot_shades: parse_colors("plot_shades", c.plot_shades)?,
            phase: parse_colors("phase", c.phase)?,
            palette: parse_colors("palette", c.palette)?,
            styles,
            border: s.border,
        })
    }
}

impl Partial {
    /// Takes every role this theme doesn't set from `parent`.
    fn fill_from(&mut self, parent: Partial) {
        self.kind = self.kind.or(parent.kind);
        for (c, p) in self.colors.iter_mut().zip(parent.colors) {
            *c = c.or(p);
        }
        self.plot_shades = self.plot_shades.or(parent.plot_shades);
        self.phase = self.phase.or(parent.phase);
        self.palette = self.palette.or(parent.palette);
        for (s, p) in self.styles.iter_mut().zip(parent.styles) {
            *s = s.or(p);
        }
        self.border = self.border.or(parent.border);
    }

    fn complete(self) -> anyhow::Result<ThemeSpec> {
        let missing = |role: &str| anyhow::anyhow!("no color for {role}");
        let mut c = [Color::Reset; 10];
        for ((out, value), role) in c.iter_mut().zip(self.colors).zip(COLOR_ROLES) {
            *out = value.ok_or_else(|| missing(role))?;
        }
        let [title, accent, selection, warning, error, muted] = self.styles.map(Option::unwrap_or_default);
        Ok(ThemeSpec {
            kind: self.kind.context("no kind (dark or light)")?,
            colors: Colors {
                background: c[0],
                text: c[1],
                muted: c[2],
                border: c[3],
                accent: c[4],
                selection: c[5],
                plot_shades: self.plot_shades.ok_or_else(|| missing("plot_shades"))?,
                warning: c[6],
                error: c[7],
                trigger: c[8],
                cursor: c[9],
                phase: self.phase.ok_or_else(|| missing("phase"))?,
                palette: self.palette.ok_or_else(|| missing("palette"))?,
            },
            styles: Styles {
                title,
                accent,
                selection,
                warning,
                error,
                muted,
            },
            border: self.border.unwrap_or(Border::Plain),
        })
    }
}

/// Where a theme comes from.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Source {
    /// Compiled into the program.
    BuiltIn,
    /// A file in the user's themes directory.
    File(PathBuf),
}

/// A theme name and where it comes from.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Entry {
    /// Name (the file stem).
    pub name: String,
    /// Built-in or the user's file (which replaces a built-in of that name).
    pub source: Source,
}

/// The available themes: built-in ones and the files in the user's themes
/// directory. Files are only read when their theme is loaded.
#[derive(Debug, Clone, Default)]
pub struct Catalog {
    dir: Option<PathBuf>,
}

impl Catalog {
    /// Themes from the built-in ones and the `.toml` files in `dir`.
    pub fn new(dir: Option<PathBuf>) -> Self {
        Self { dir }
    }

    /// The user's themes directory.
    pub fn dir(&self) -> Option<&Path> {
        self.dir.as_deref()
    }

    /// One of the built-in themes; they are complete and checked by tests.
    pub fn builtin(name: &str) -> ThemeSpec {
        Self::default()
            .load(name)
            .unwrap_or_else(|e| panic!("built-in theme {name}: {e:#}"))
    }

    /// The built-in themes in their order, then the user's other themes by
    /// name. A user file with a built-in name keeps the built-in's place.
    pub fn entries(&self) -> Vec<Entry> {
        let files = self.user_files();
        let mut entries: Vec<Entry> = BUILTIN
            .iter()
            .map(|(name, _)| Entry {
                name: (*name).to_owned(),
                source: files
                    .iter()
                    .find(|(n, _)| n == name)
                    .map_or(Source::BuiltIn, |(_, p)| Source::File(p.clone())),
            })
            .collect();
        for (name, path) in files {
            if !BUILTIN.iter().any(|(n, _)| *n == name) {
                entries.push(Entry {
                    name,
                    source: Source::File(path),
                });
            }
        }
        entries
    }

    /// Names of all themes, in the order of `entries`.
    pub fn names(&self) -> Vec<String> {
        self.entries().into_iter().map(|e| e.name).collect()
    }

    /// The `<name>.toml` files in the themes directory, sorted by name.
    fn user_files(&self) -> Vec<(String, PathBuf)> {
        let Some(dir) = &self.dir else {
            return Vec::new();
        };
        let Ok(read) = std::fs::read_dir(dir) else {
            return Vec::new();
        };
        let mut files: Vec<(String, PathBuf)> = read
            .flatten()
            .map(|e| e.path())
            .filter(|p| p.is_file() && p.extension().is_some_and(|x| x == "toml"))
            .filter_map(|p| Some((p.file_stem()?.to_str()?.to_owned(), p)))
            .collect();
        files.sort();
        files
    }

    /// Loads theme `name`: the user's file when there is one, else the
    /// built-in. An unknown name is an error that lists the names.
    pub fn load(&self, name: &str) -> anyhow::Result<ThemeSpec> {
        self.resolve(name, false, 0)?.complete()
    }

    /// The theme after (or before, `!forward`) `current` in `names()`,
    /// wrapping around, and the ones from there on.
    pub fn cycle(&self, current: &str, forward: bool) -> Vec<String> {
        let mut names = self.names();
        let at = names.iter().position(|n| n == current).map_or(0, |i| i + 1);
        // After the rotation `current` (if known) is last.
        let n = names.len();
        names.rotate_left(at.min(n));
        names.retain(|n| n != current);
        if !forward {
            names.reverse();
        }
        names
    }

    /// The theme's text and a name for its errors: the user's file, or the
    /// built-in when `builtin_only` or there is no file.
    fn text(&self, name: &str, builtin_only: bool) -> anyhow::Result<(String, String)> {
        if !builtin_only && let Some((_, path)) = self.user_files().into_iter().find(|(n, _)| n == name) {
            let text = std::fs::read_to_string(&path).with_context(|| format!("reading theme {}", path.display()))?;
            return Ok((text, path.display().to_string()));
        }
        match BUILTIN.iter().find(|(n, _)| *n == name) {
            Some((_, text)) => Ok(((*text).to_owned(), format!("built-in theme {name}"))),
            None => bail!("unknown theme '{name}' (available: {})", self.names().join(", ")),
        }
    }

    /// Reads theme `name` and fills it from its parent: `extends`, else the
    /// built-in theme of its kind. `extends` naming the theme itself means
    /// the built-in one, so a user file can change a built-in theme.
    fn resolve(&self, name: &str, builtin_only: bool, depth: usize) -> anyhow::Result<Partial> {
        if depth > MAX_DEPTH {
            bail!("theme '{name}': extends goes in a circle");
        }
        let (text, origin) = self.text(name, builtin_only)?;
        let file: ThemeFile = toml::from_str(&text).with_context(|| format!("invalid theme {origin}"))?;
        let extends = file.extends.clone();
        let mut partial = file.partial().with_context(|| format!("invalid theme {origin}"))?;
        let parent = match extends {
            Some(parent) => Some(
                self.resolve(&parent, builtin_only || parent == name, depth + 1)
                    .with_context(|| format!("theme {origin} extends '{parent}'"))?,
            ),
            None => {
                let kind = partial
                    .kind
                    .with_context(|| format!("invalid theme {origin}: kind (dark or light) is missing"))?;
                let base = kind.name();
                // The built-in dark and light themes are complete.
                (!(builtin_only && name == base))
                    .then(|| self.resolve(base, true, depth + 1))
                    .transpose()?
            }
        };
        if let Some(parent) = parent {
            partial.fill_from(parent);
        }
        Ok(partial)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rgb(s: &str) -> Color {
        Color::from_str(s).unwrap()
    }

    fn write(dir: &Path, name: &str, text: &str) {
        std::fs::write(dir.join(format!("{name}.toml")), text).unwrap();
    }

    #[test]
    fn builtin_themes_are_complete() {
        for (name, text) in BUILTIN {
            let spec = Catalog::builtin(name);
            // Complete on their own, without falling back to dark or light.
            let file: ThemeFile = toml::from_str(text).unwrap();
            assert!(file.partial().unwrap().complete().is_ok(), "{name}");
            // The template parses again into the same theme.
            let again: ThemeFile = toml::from_str(&spec.to_toml()).unwrap();
            assert_eq!(again.partial().unwrap().complete().unwrap(), spec, "{name}");
        }
    }

    #[test]
    fn dark_and_light_keep_their_ansi_colors() {
        let dark = Catalog::builtin("dark");
        assert_eq!(dark.kind, ThemeKind::Dark);
        assert_eq!(dark.colors.background, Color::Black);
        assert_eq!(dark.colors.border, Color::Gray);
        assert_eq!(dark.colors.plot_shades, [Color::Indexed(234), Color::Black]);
        assert_eq!(dark.colors.error, Color::LightRed);
        assert_eq!(dark.border, Border::Plain);
        assert_eq!(dark.styles.accent, Modifier::BOLD);
        assert_eq!(dark.styles.muted, Modifier::empty());
        let light = Catalog::builtin("light");
        assert_eq!(light.kind, ThemeKind::Light);
        assert_eq!(light.colors.selection, Color::Gray);
        assert_eq!(light.colors.muted, Color::DarkGray);
        assert_eq!(light.colors.plot_shades, [Color::White, Color::Indexed(254)]);
    }

    #[test]
    fn new_themes_have_their_colors() {
        let t = Catalog::builtin("tokyonight");
        assert_eq!(t.colors.background, rgb("#1a1b26"));
        assert_eq!(t.colors.accent, rgb("#7aa2f7"));
        assert_eq!(t.colors.palette[6], rgb("#1abc9c"));
        assert_eq!(t.border, Border::Rounded);
        assert_eq!(t.styles.muted, Modifier::ITALIC);
        assert_eq!(Catalog::builtin("nord").colors.background, rgb("#2e3440"));
        assert_eq!(Catalog::builtin("nord-night").colors.background, rgb("#1b1e25"));
        assert_eq!(Catalog::builtin("solarized-dark").colors.text, rgb("#839496"));
        assert_eq!(Catalog::builtin("atom-one-dark").colors.error, rgb("#e06c75"));
    }

    #[test]
    fn user_themes_extend_and_replace() {
        let dir = tempfile::tempdir().unwrap();
        write(
            dir.path(),
            "mine",
            "extends = \"nord\"\n[colors]\naccent = \"#ebcb8b\"\n[style]\nborder = \"thick\"\n",
        );
        write(dir.path(), "small", "kind = \"light\"\n[colors]\naccent = \"red\"\n");
        write(dir.path(), "nord", "extends = \"nord\"\n[colors]\ntext = \"reset\"\n");
        std::fs::write(dir.path().join("notes.txt"), "not a theme").unwrap();
        let cat = Catalog::new(Some(dir.path().to_owned()));

        let names = cat.names();
        assert_eq!(&names[..2], ["dark", "light"]);
        assert_eq!(&names[names.len() - 2..], ["mine", "small"]);
        let nord = cat.entries().into_iter().find(|e| e.name == "nord").unwrap();
        assert_eq!(nord.source, Source::File(dir.path().join("nord.toml")));

        let mine = cat.load("mine").unwrap();
        assert_eq!(mine.colors.accent, rgb("#ebcb8b"));
        assert_eq!(mine.border, Border::Thick);
        // The rest from the user's nord, which changes the built-in nord.
        assert_eq!(mine.colors.text, Color::Reset);
        assert_eq!(mine.colors.background, rgb("#2e3440"));
        assert_eq!(mine.kind, ThemeKind::Dark);

        let small = cat.load("small").unwrap();
        assert_eq!(small.colors.accent, Color::Red);
        assert_eq!(small.colors.background, Color::White);
    }

    #[test]
    fn broken_themes_are_errors_with_the_file() {
        let dir = tempfile::tempdir().unwrap();
        write(
            dir.path(),
            "unknown-key",
            "kind = \"dark\"\n[colors]\naccnt = \"red\"\n",
        );
        write(
            dir.path(),
            "bad-color",
            "kind = \"dark\"\n[colors]\naccent = \"#12345\"\n",
        );
        write(dir.path(), "no-kind", "[colors]\naccent = \"red\"\n");
        write(dir.path(), "a", "extends = \"b\"\n");
        write(dir.path(), "b", "extends = \"a\"\n");
        write(dir.path(), "missing", "extends = \"nope\"\n");
        let cat = Catalog::new(Some(dir.path().to_owned()));
        let err = |name: &str| format!("{:#}", cat.load(name).unwrap_err());

        let e = err("unknown-key");
        assert!(e.contains("unknown-key.toml") && e.contains("accnt"), "{e}");
        let e = err("bad-color");
        assert!(e.contains("\"#12345\" for accent"), "{e}");
        assert!(err("no-kind").contains("kind (dark or light) is missing"));
        assert!(err("a").contains("circle"));
        let e = err("missing");
        assert!(e.contains("unknown theme 'nope'") && e.contains("tokyonight"), "{e}");
        let e = format!("{:#}", Catalog::default().load("nordd").unwrap_err());
        assert!(e.contains("available: dark, light, atom-one-dark"), "{e}");
    }

    #[test]
    fn cycles_through_the_names() {
        let cat = Catalog::default();
        assert_eq!(cat.cycle("dark", true)[0], "light");
        assert_eq!(cat.cycle("tokyonight", true)[0], "dark");
        assert_eq!(cat.cycle("dark", false)[0], "tokyonight");
        assert_eq!(cat.cycle("light", false)[0], "dark");
        let all = cat.cycle("nord", true);
        assert_eq!(all.len(), BUILTIN.len() - 1);
        assert!(!all.contains(&"nord".to_owned()));
        // An unknown current name starts at the first theme.
        assert_eq!(cat.cycle("gone", true)[0], "dark");
    }

    #[test]
    fn maps_rgb_to_256_colors() {
        assert_eq!(nearest_256(0, 0, 0), 16);
        assert_eq!(nearest_256(255, 255, 255), 231);
        assert_eq!(nearest_256(0x1a, 0x1b, 0x26), 234);
        assert_eq!(nearest_256(0x7a, 0xa2, 0xf7), 111);
        let t = Catalog::builtin("tokyonight").to_256_colors();
        assert!(matches!(t.colors.background, Color::Indexed(_)));
        // ANSI colors stay as they are.
        assert_eq!(Catalog::builtin("dark").to_256_colors(), Catalog::builtin("dark"));
    }

    #[test]
    fn writes_color_names_in_kebab_case() {
        assert_eq!(color_name(Color::LightRed), "light-red");
        assert_eq!(color_name(Color::DarkGray), "dark-gray");
        assert_eq!(color_name(Color::Indexed(234)), "234");
        assert_eq!(color_name(Color::Rgb(26, 27, 38)), "#1a1b26");
        assert_eq!(Color::from_str("dark-gray").unwrap(), Color::DarkGray);
    }
}
