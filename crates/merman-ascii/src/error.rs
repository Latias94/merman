use thiserror::Error;

use crate::options::TerminalWidthProfile;
use crate::resource::AsciiResourceLimitExceeded;

pub type Result<T> = std::result::Result<T, AsciiError>;

#[non_exhaustive]
#[derive(Debug, Clone, Error, PartialEq, Eq)]
pub enum AsciiError {
    #[error(transparent)]
    Cancelled(#[from] merman_model::OperationCancelled),
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
    /// The bounded routing heuristic ended without finding a valid scene.
    ///
    /// This is not an unsupported feature, proof that no valid scene exists, or a
    /// resource-policy failure.
    #[error(
        "ASCII bounded routing search did not find a valid scene for edge[{edge_index}] in `{diagram_type}`"
    )]
    RoutingSearchExhausted {
        diagram_type: &'static str,
        /// Zero-based edge index in the authored model, before canonical reordering.
        edge_index: usize,
    },
    /// Independent marker demand exceeds a conservative upper bound for the current
    /// fixed-terminal presentation, after explicitly allocated shared markers are counted once.
    ///
    /// This certificate applies only to that presentation and its bounded marker candidates;
    /// it does not establish that the graph is impossible under other presentations or layouts.
    #[error(
        "ASCII fixed-terminal capacity exceeded in the current presentation: {required} independent markers > upper bound {available} for endpoint `{endpoint}` in `{diagram_type}`"
    )]
    PortCapacityExceeded {
        diagram_type: &'static str,
        endpoint: String,
        required: usize,
        available: usize,
    },
    #[error("ASCII rendering could not allocate bounded storage during `{phase}`")]
    AllocationFailed { phase: &'static str },
    #[error(transparent)]
    ResourceLimitExceeded(#[from] AsciiResourceLimitExceeded),
    #[error(transparent)]
    OperationResourceTerminal(merman_model::OperationLedgerError),
    #[error(
        "ASCII output exceeds requested width: actual {actual_width} cells > maximum {max_width} ({profile:?})"
    )]
    WidthOverflow {
        max_width: usize,
        actual_width: usize,
        profile: TerminalWidthProfile,
    },
    /// Internal signal used when a family-owned planned extent exceeds a Fallback viewport
    /// before its primary text has been encoded.
    #[error(
        "ASCII primary projection exceeds fallback viewport: actual {actual_width} cells > maximum {max_width} ({profile:?})"
    )]
    PrimaryViewportOverflow {
        max_width: usize,
        actual_width: usize,
        height: usize,
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
