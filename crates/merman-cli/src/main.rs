mod cli;
mod commands;
mod config;
mod error;
mod io;
mod render;

use clap::Parser;

fn main() {
    let cli = cli::Cli::parse();
    if let Err(err) = commands::run(cli) {
        eprintln!("{err}");
        std::process::exit(1);
    }
}
