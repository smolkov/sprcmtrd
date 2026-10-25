use std::process::ExitCode;

use clap::{CommandFactory, Parser};
use clap_complete::CompleteEnv;
use sprcmtrd::args::Cli;
use sprcmtrd::config::AppConfig;
use sprcmtrd::context::Context;
use sprcmtrd::logging::{self, Target};

/// Exit code for a missing or unreadable file.
const EXIT_FILE: u8 = 3;
/// Exit code for invalid or unsupported COMTRADE content.
const EXIT_CONTENT: u8 = 4;

#[tokio::main]
async fn main() -> ExitCode {
    // `COMPLETE=<shell> sprcmtrd ...` is a request from the shell's
    // completion script: answer it and exit before anything else runs.
    CompleteEnv::with_factory(Cli::command).complete();
    let cli = Cli::parse();
    let target = if cli.command.is_some() {
        Target::Stderr
    } else {
        Target::File
    };
    logging::init(cli.global.verbose, target);

    let result = match AppConfig::load() {
        Ok(config) => run(cli, config).await,
        Err(e) => Err(e),
    };
    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("error: {e:#}");
            ExitCode::from(exit_code(&e))
        }
    }
}

/// Runs the subcommand, or the TUI when there is none. The two sides only
/// share the context, so neither depends on the other.
async fn run(cli: Cli, config: AppConfig) -> anyhow::Result<()> {
    let ctx = Context::new(cli.global, config);
    match cli.command {
        Some(cmd) => cmd.run(&ctx),
        None => sprcmtrd::tui::run(cli.path, &ctx).await,
    }
}

fn exit_code(e: &anyhow::Error) -> u8 {
    if e.downcast_ref::<sprcmtrd::command::check::Failed>().is_some() {
        return EXIT_CONTENT;
    }
    match e.chain().find_map(|c| c.downcast_ref::<sprcmtrd::Error>()) {
        Some(err) if err.is_file_error() => EXIT_FILE,
        Some(_) => EXIT_CONTENT,
        None if e.chain().any(|c| {
            c.downcast_ref::<std::io::Error>()
                .is_some_and(|io| io.kind() == std::io::ErrorKind::NotFound)
        }) =>
        {
            EXIT_FILE
        }
        None => 1,
    }
}
