#[cfg(all(feature = "ascii", not(feature = "svg")))]
mod ascii_render;
mod capabilities;
mod cli;
mod commands;
mod config;
mod error;
mod io;
#[cfg(feature = "markdown")]
mod markdown;
#[cfg(feature = "svg")]
mod render;

use clap::Parser;
use std::process::ExitCode;

fn main() -> ExitCode {
    let cli = cli::Cli::parse();
    match commands::run(cli) {
        Ok(exit_code) => u8::try_from(exit_code)
            .map(ExitCode::from)
            .unwrap_or(ExitCode::FAILURE),
        Err(err) => {
            if err.is_broken_stdout_pipe() {
                return ExitCode::SUCCESS;
            }
            let exit_code = err.exit_code();
            eprintln!("{err}");
            exit_code
        }
    }
}
