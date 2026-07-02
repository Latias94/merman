mod cli;
mod commands;
mod config;
mod error;
mod io;
mod markdown;
mod render;

use clap::Parser;
use std::process::ExitCode;

fn main() -> ExitCode {
    let cli = cli::Cli::parse();
    if let Err(err) = commands::run(cli) {
        if err.is_broken_stdout_pipe() {
            return ExitCode::SUCCESS;
        }
        let exit_code = err.exit_code();
        eprintln!("{err}");
        return exit_code;
    }
    ExitCode::SUCCESS
}
