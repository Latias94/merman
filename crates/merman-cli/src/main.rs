mod cli;
mod commands;
mod config;
mod error;
mod io;
mod markdown;
mod render;

use clap::Parser;

fn main() {
    let cli = cli::Cli::parse();
    let exit_code = match commands::run(cli) {
        Ok(exit_code) => exit_code,
        Err(err) => {
            eprintln!("{err}");
            1
        }
    };
    if exit_code != 0 {
        std::process::exit(exit_code);
    }
}
