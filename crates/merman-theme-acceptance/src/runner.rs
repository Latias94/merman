//! Shared artifact observers for host-dependent preset qualification.

use crate::observation::C6TargetArtifact;

#[derive(Debug, thiserror::Error)]
#[error("{stage}: {detail}")]
pub(crate) struct C6ProofError {
    stage: &'static str,
    detail: String,
}

impl C6ProofError {
    pub(crate) fn new(stage: &'static str, detail: impl Into<String>) -> Self {
        Self {
            stage,
            detail: detail.into(),
        }
    }
}

pub(crate) type C6ProofResult<T> = Result<T, C6ProofError>;

#[path = "support/artifact_observation.rs"]
pub(crate) mod artifact_observation;
#[path = "support/c6_raster_proof.rs"]
mod c6_raster_proof;
#[path = "support/c6_sequence_proof.rs"]
pub(crate) mod c6_sequence_proof;

pub(crate) use c6_raster_proof::{
    RasterImage as C6RasterImage, decode_bounded_png_artifact, parse_c6_hex_rgb,
};
