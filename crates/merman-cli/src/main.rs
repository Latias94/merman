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
#[cfg(any(feature = "svg", feature = "ascii"))]
mod operation;
mod output;
#[cfg(any(feature = "svg", feature = "ascii"))]
mod render;
mod resources;
mod runtime;
#[cfg(feature = "rustdoc")]
mod rustdoc;
#[cfg(feature = "markdown")]
mod transaction;

fn main() -> std::process::ExitCode {
    app::run_system()
}
