#![cfg(merman_internal_theme_acceptance)]

//! Non-published theme runtime acceptance harness.
//!
//! Preset qualification checks exact catalog recipes under named host-dependent profiles.

macro_rules! c6_ensure {
    ($stage:expr, $condition:expr, $($arg:tt)+) => {
        if !$condition {
            return Err(C6ProofError::new($stage, format!($($arg)+)));
        }
    };
}

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
    preset_qualification_config, run_preset_qualification,
};
