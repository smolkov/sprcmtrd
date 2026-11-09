//! `themes` command: the color themes, built-in and the user's, or one of
//! them as a complete theme file to start a custom theme from.

use super::Context;
use crate::args::ThemesArgs;
use crate::format::table;
use crate::term::out;
use crate::term::themes::Source;

impl ThemesArgs {
    /// Runs the `themes` command.
    pub fn run(self, ctx: &Context) -> anyhow::Result<()> {
        let catalog = ctx.themes();
        if let Some(name) = self.print {
            out!("{}", catalog.load(&name)?.to_toml())?;
            return Ok(());
        }
        let mut rows = Vec::new();
        let mut errors = Vec::new();
        for entry in catalog.entries() {
            let kind = match catalog.load(&entry.name) {
                Ok(t) => t.kind.name().to_owned(),
                Err(e) => {
                    errors.push(format!("{e:#}"));
                    "invalid".to_owned()
                }
            };
            let source = match entry.source {
                Source::BuiltIn => "built-in".to_owned(),
                Source::File(path) => path.display().to_string(),
            };
            rows.push(vec![entry.name, kind, source]);
        }
        out!("{}", table(&["NAME", "KIND", "SOURCE"], &rows))?;
        for e in errors {
            eprintln!("error: {e}");
        }
        Ok(())
    }
}
