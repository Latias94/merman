#![cfg(merman_internal_theme_acceptance)]

//! Non-published theme runtime acceptance harness.
//!
//! The crate exposes the C6 progress report, the exact native C6a eligibility seal, and the
//! independent route-cutover/KTD23 authorization summaries, plus a representative PNG/JPEG/PDF
//! native export smoke that does not participate in those gates. KTD23 compares current renderer
//! probes with an acceptance-owned historical witness; it does not derive authorization from the
//! current classifier. Internal observation, per-cell, and per-route receipts remain private so
//! callers cannot report their own success.
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
mod preset_qualification;

#[cfg(feature = "png")]
pub use preset_qualification::{
    PresetAdmissionError, PresetAdmissionObservation, PresetAdmissionReport,
    PresetQualificationReceipt, PresetQualificationSpec, inspect_preset_admission,
    run_preset_qualification,
};

mod route_retirement_manifest;

#[cfg(feature = "png")]
pub use cutover::RouteCutoverAuthorizationReport;

#[cfg(feature = "png")]
pub use eligibility::C6aEligibilityReceipt;

#[cfg(feature = "png")]
pub use observation::{C6ExecutionReport, C6RuntimeError, RouteCutoverRuntimeError};

#[cfg(feature = "png")]
pub use runner::{run_c6a_eligibility, run_enforced_c6_runtime, run_route_cutover_authorization};

pub use route_retirement_manifest::{
    LegacyProjectionRetirementAuthorization, LegacyProjectionVerificationError,
    authorize_legacy_projection_retirements,
};

#[cfg(all(feature = "png", feature = "jpeg", feature = "pdf"))]
pub use runner::{
    NativeExportSmokeError, NativeExportSmokeSummary, run_representative_native_export_smoke,
};
