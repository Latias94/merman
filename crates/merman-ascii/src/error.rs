use thiserror::Error;

use crate::resource::AsciiResourceLimitExceeded;

pub type Result<T> = std::result::Result<T, AsciiError>;

#[non_exhaustive]
#[derive(Debug, Clone, Error, PartialEq, Eq)]
pub enum AsciiError {
    #[error(transparent)]
    Cancelled(#[from] merman_core::OperationCancelled),
    #[error("invalid ASCII render option `{field}`: {message}")]
    InvalidOption {
        field: &'static str,
        message: &'static str,
    },
    #[error("ASCII rendering does not support diagram type `{diagram_type}`")]
    UnsupportedDiagram { diagram_type: String },
    #[error("ASCII rendering does not support `{feature}` for `{diagram_type}` yet")]
    UnsupportedFeature {
        diagram_type: &'static str,
        feature: &'static str,
    },
    #[error("ASCII rendering could not allocate bounded storage during `{phase}`")]
    AllocationFailed { phase: &'static str },
    #[error(transparent)]
    ResourceLimitExceeded(#[from] AsciiResourceLimitExceeded),
}

impl AsciiError {
    pub(crate) const fn allocation_failed(phase: &'static str) -> Self {
        Self::AllocationFailed { phase }
    }
}
