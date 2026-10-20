use std::process::ExitCode;

use clap::Parser;
use sprcmtrd::args::Cli;
use sprcmtrd::config::AppConfig;
use sprcmtrd::logging::{self, Target};

/// Exit code for a missing or unreadable file.
const EXIT_FILE: u8 = 3;
/// Exit code for invalid or unsupported COMTRADE content.
const EXIT_CONTENT: u8 = 4;

#[tokio::main]
async fn main() -> ExitCode {
    let cli = Cli::parse();
    let target = if cli.command.is_some() {
        Target::Stderr
    } else {
        Target::File
    };
    logging::init(cli.global.verbose, target);

    let result = match AppConfig::load() {
        Ok(config) => sprcmtrd::command::run(cli, config).await,
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

fn exit_code(e: &anyhow::Error) -> u8 {
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
