//! Non-published theme runtime acceptance harness.
//!
//! The crate exposes separate coarse summaries for the C6 catalog-cell gate and the exact
//! route-cutover authorization gate, plus a representative PNG/JPEG/PDF native export smoke that
//! does not participate in either gate. Route authorization only permits bridge ownership cutover;
//! it does not add C6 cells or contribute to the C6 execution report or digest. Internal
//! observation, per-route, and sealed authorization receipts remain private so callers cannot
//! report their own success.
//!
//! ```compile_fail
//! use merman_theme_acceptance::C6CellReceipt;
//! ```
//!
//! ```compile_fail
//! use merman_theme_acceptance::RouteCutoverAuthorizationReceipt;
//! ```

macro_rules! c6_ensure {
    ($stage:expr, $condition:expr, $($arg:tt)+) => {
        if !$condition {
            return Err(C6ProofError::new($stage, format!($($arg)+)));
        }
    };
}

#[cfg(all(feature = "png", feature = "jpeg", feature = "pdf"))]
mod cutover;

#[cfg(all(feature = "png", feature = "jpeg", feature = "pdf"))]
mod cutover_manifest;

#[cfg(all(feature = "png", feature = "jpeg", feature = "pdf"))]
mod observation;

#[cfg(all(feature = "png", feature = "jpeg", feature = "pdf"))]
mod runner;

#[cfg(all(feature = "png", feature = "jpeg", feature = "pdf"))]
pub use cutover::RouteCutoverAuthorizationReport;

#[cfg(all(feature = "png", feature = "jpeg", feature = "pdf"))]
pub use observation::{C6ExecutionReport, C6RuntimeError, RouteCutoverRuntimeError};

#[cfg(all(feature = "png", feature = "jpeg", feature = "pdf"))]
pub use runner::{
    NativeExportSmokeError, NativeExportSmokeSummary, run_enforced_c6_runtime,
    run_representative_native_export_smoke, run_route_cutover_authorization,
};
