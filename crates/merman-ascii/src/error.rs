use thiserror::Error;

use crate::options::TerminalWidthProfile;
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
    #[error(transparent)]
    OperationResourceTerminal(merman_core::OperationLedgerError),
    #[error(
        "ASCII output exceeds requested width: actual {actual_width} cells > maximum {max_width} ({profile:?})"
    )]
    WidthOverflow {
        max_width: usize,
        actual_width: usize,
        profile: TerminalWidthProfile,
    },
    #[error(
        "ASCII structured fallback is unavailable for `{diagram_type}` within {max_width} cells (actual {actual_width})"
    )]
    FallbackUnavailable {
        diagram_type: String,
        max_width: usize,
        actual_width: usize,
    },
}

impl AsciiError {
    pub(crate) const fn allocation_failed(phase: &'static str) -> Self {
        Self::AllocationFailed { phase }
    }
}
