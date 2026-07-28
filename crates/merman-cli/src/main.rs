mod app;
#[cfg(feature = "markdown")]
mod batch;
mod capabilities;
mod cli;
mod commands;
mod config;
mod diagnostics;
mod error;
#[cfg(feature = "analysis")]
mod fix;
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
#[cfg(feature = "markdown")]
mod transaction;

fn main() -> std::process::ExitCode {
    app::CliApp::system().execute()
}
