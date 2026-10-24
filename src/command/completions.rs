//! `completions` command: the shell script for dynamic tab completion.
//!
//! The script is the same one `COMPLETE=<shell> sprcmtrd` prints: on every
//! Tab the shell calls `sprcmtrd`, which completes subcommands, options and
//! values from the clap definitions and recordings with
//! [`crate::complete::recordings`]. Sourcing `COMPLETE=<shell> sprcmtrd` on
//! shell start keeps the script in step with the installed binary; a script
//! written with `--output` has to be written again after an update.

use std::fs::File;
use std::io::{self, BufWriter, Write};
use std::path::PathBuf;

use anyhow::{Context as _, anyhow};
use clap_complete::Shell;
use clap_complete::env::Shells;

use super::Context;
use crate::args::CompletionsArgs;

/// Variable the shell script sets to ask for completions.
pub const VAR: &str = "COMPLETE";

impl CompletionsArgs {
    /// Runs the `completions` command: writes the script for `shell` to
    /// `output`, or to stdout.
    pub fn run(self, _ctx: &Context) -> anyhow::Result<()> {
        let Self { shell, output } = self;
        match output.as_deref() {
            Some(path) => {
                let file = File::create(path).with_context(|| format!("creating {}", path.display()))?;
                let mut out = BufWriter::new(file);
                write(shell, &mut out)?;
                out.flush().with_context(|| format!("writing {}", path.display()))?;
                eprintln!("wrote {shell} completions to {}", path.display());
                Ok(())
            }
            None => write(shell, &mut io::stdout().lock()),
        }
    }
}

/// Writes the completion script for `shell` into `out`.
pub fn write(shell: Shell, out: &mut dyn Write) -> anyhow::Result<()> {
    let name = shell.to_string();
    let shells = Shells::builtins();
    let completer = shells
        .completer(&name)
        .ok_or_else(|| anyhow!("no completion support for {name}"))?;
    completer.write_registration(VAR, "sprcmtrd", "sprcmtrd", &program(), out)?;
    Ok(())
}

/// How the script calls the program: as started (`sprcmtrd` from `PATH`), or
/// the absolute path when started by path (`./target/debug/sprcmtrd`).
fn program() -> String {
    let arg0 = PathBuf::from(std::env::args_os().next().unwrap_or_else(|| "sprcmtrd".into()));
    let path = if arg0.components().count() > 1 {
        std::path::absolute(&arg0).unwrap_or(arg0)
    } else {
        arg0
    };
    path.to_string_lossy().into_owned()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn script(shell: Shell) -> String {
        let mut buf = Vec::new();
        write(shell, &mut buf).unwrap();
        String::from_utf8(buf).unwrap()
    }

    #[test]
    fn scripts_call_back_into_the_program() {
        for shell in [Shell::Bash, Shell::Zsh, Shell::Fish, Shell::PowerShell, Shell::Elvish] {
            let s = script(shell);
            assert!(s.contains(VAR), "{shell} script lacks {VAR}");
            assert!(s.contains("sprcmtrd"), "{shell} script lacks the program");
        }
        assert!(
            script(Shell::Bash)
                .contains("complete -o nospace -o bashdefault -o nosort -F _clap_complete_sprcmtrd sprcmtrd")
        );
        assert!(script(Shell::Zsh).contains("#compdef sprcmtrd"));
    }
}
