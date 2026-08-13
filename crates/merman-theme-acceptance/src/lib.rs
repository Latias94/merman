//! Non-published C6 runtime acceptance harness.
//!
//! The crate exposes only a summary computed from real renderer and exporter artifacts. Internal
//! observation receipts and evaluators remain private so callers cannot report their own success.
//!
//! ```compile_fail
//! use merman_theme_acceptance::C6CellReceipt;
//! ```

#[cfg(all(feature = "png", feature = "jpeg", feature = "pdf"))]
mod observation;

#[cfg(all(feature = "png", feature = "jpeg", feature = "pdf"))]
mod runner;

#[cfg(all(feature = "png", feature = "jpeg", feature = "pdf"))]
pub use observation::{C6ExecutionReport, C6RuntimeError};

#[cfg(all(feature = "png", feature = "jpeg", feature = "pdf"))]
pub use runner::run_enforced_c6_runtime;
