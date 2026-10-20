//! `completions` command: tab-completion scripts for bash, zsh, fish, PowerShell and elvish.
//!
//! The scripts are generated from the clap definitions in [`crate::args`], so
//! subcommands, options and values (shells, themes, sort keys) stay in sync
//! with the binary. File and directory arguments carry value hints, so the
//! shell completes paths and steps into subdirectories as you type.

use std::fs::File;
use std::io::{self, BufWriter, Write};
use std::path::Path;

use anyhow::Context;
use clap::CommandFactory;
use clap_complete::Shell;

use crate::args::Cli;

/// Runs the `completions` command: writes the script for `shell` to
/// `output`, or to stdout.
pub fn run(shell: Shell, output: Option<&Path>) -> anyhow::Result<()> {
    match output {
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

/// Generates the completion script for `shell` into `out`.
pub fn write(shell: Shell, out: &mut dyn Write) -> anyhow::Result<()> {
    let mut cmd = Cli::command();
    let name = cmd.get_name().to_owned();
    clap_complete::generate(shell, &mut cmd, name, out);
    Ok(())
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
    fn scripts_list_subcommands_and_values() {
        for shell in [Shell::Bash, Shell::Zsh, Shell::Fish, Shell::PowerShell, Shell::Elvish] {
            let s = script(shell);
            // Fish writes long options as `-l no-recursive`, so match without dashes.
            for word in ["info", "list", "view", "completions", "no-recursive", "theme"] {
                assert!(s.contains(word), "{shell} script lacks {word}");
            }
        }
        let zsh = script(Shell::Zsh);
        assert!(zsh.starts_with("#compdef sprcmtrd"));
        // Path arguments complete files / directories.
        assert!(zsh.contains("_files"));
        assert!(zsh.contains("_files -/"));
        // Theme values with their descriptions.
        assert!(script(Shell::Fish).contains("-l theme -d 'TUI theme' -r -f -a \"dark\\t'Dark background'"));
    }
}
