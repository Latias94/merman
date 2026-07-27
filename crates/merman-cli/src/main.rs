mod app;
#[cfg(all(feature = "ascii", not(feature = "svg")))]
mod ascii_render;
mod capabilities;
mod cli;
mod commands;
mod config;
mod error;
mod invocation;
mod io;
#[cfg(feature = "markdown")]
mod markdown;
#[cfg(feature = "svg")]
mod render;

fn main() -> std::process::ExitCode {
    app::CliApp::system().execute()
}
