mod app;
#[cfg(all(feature = "ascii", not(feature = "svg")))]
mod ascii_render;
mod capabilities;
mod cli;
mod commands;
mod config;
mod error;
mod input;
mod invocation;
mod io;
#[cfg(feature = "markdown")]
mod markdown;
#[cfg(feature = "network-icons")]
mod network;
#[cfg(feature = "svg")]
mod render;
mod resources;

fn main() -> std::process::ExitCode {
    app::CliApp::system().execute()
}
