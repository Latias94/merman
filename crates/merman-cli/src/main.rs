mod app;
mod capabilities;
mod cli;
mod commands;
mod config;
mod diagnostics;
mod error;
mod input;
mod invocation;
mod io;
#[cfg(feature = "markdown")]
mod markdown;
#[cfg(feature = "network-icons")]
mod network;
mod output;
#[cfg(any(feature = "svg", feature = "ascii"))]
mod render;
mod resources;

fn main() -> std::process::ExitCode {
    app::CliApp::system().execute()
}
