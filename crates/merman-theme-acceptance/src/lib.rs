//! Non-published theme runtime acceptance harness.
//!
//! The crate exposes the C6 progress report, the exact native C6a eligibility seal, and the
//! independent route-cutover authorization summary, plus a representative PNG/JPEG/PDF native
//! export smoke that does not participate in those gates. Route authorization only permits bridge
//! ownership cutover; it does not add C6 cells or contribute to the C6 execution or eligibility
//! digests. Internal observation, per-cell, and per-route receipts remain private so callers cannot
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

#[cfg(feature = "png")]
mod cutover;

#[cfg(feature = "png")]
mod cutover_manifest;

#[cfg(feature = "png")]
mod eligibility;

#[cfg(feature = "png")]
mod observation;

#[cfg(feature = "png")]
mod runner;

#[cfg(feature = "png")]
mod route_retirement_manifest;

#[cfg(feature = "png")]
pub use cutover::RouteCutoverAuthorizationReport;

#[cfg(feature = "png")]
pub use eligibility::C6aEligibilityReceipt;

#[cfg(feature = "png")]
pub use observation::{C6ExecutionReport, C6RuntimeError, RouteCutoverRuntimeError};

#[cfg(feature = "png")]
pub use runner::{run_c6a_eligibility, run_enforced_c6_runtime, run_route_cutover_authorization};

#[cfg(all(feature = "png", feature = "jpeg", feature = "pdf"))]
pub use runner::{
    NativeExportSmokeError, NativeExportSmokeSummary, run_representative_native_export_smoke,
};
