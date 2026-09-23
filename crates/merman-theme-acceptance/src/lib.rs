#![cfg(merman_internal_theme_acceptance)]

//! Non-published theme runtime acceptance harness.
//!
//! Preset qualification checks exact catalog recipes under named host-dependent profiles.
//! The independent KTD23 inventory still compares renderer probes with historical witnesses.

macro_rules! c6_ensure {
    ($stage:expr, $condition:expr, $($arg:tt)+) => {
        if !$condition {
            return Err(C6ProofError::new($stage, format!($($arg)+)));
        }
    };
}

#[cfg(all(test, feature = "png"))]
mod cutover_manifest;

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

pub use route_retirement_manifest::{
    LegacyProjectionRetirementAuthorization, LegacyProjectionVerificationError,
    authorize_legacy_projection_retirements,
};
