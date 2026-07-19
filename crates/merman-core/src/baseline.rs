//! Active upstream Mermaid baseline metadata.
//!
//! These constants describe the source revision that production parsing, layout, rendering, and
//! parity fixtures target.

pub use crate::generated::mermaid_reference::{
    PINNED_MERMAID_BASELINE_TAG, PINNED_MERMAID_BASELINE_VERSION,
    PINNED_MERMAID_BASELINE_VERSION_SUFFIX,
};

/// Detector registry profile matching Mermaid's feature registration sets.
#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum BaselineRegistryProfile {
    /// Base Mermaid diagrams without large feature registrations.
    Tiny,
    /// Full Mermaid registration set, including large feature diagrams when enabled.
    Full,
}

impl BaselineRegistryProfile {
    /// Returns a stable lowercase profile label for diagnostics and reports.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Tiny => "tiny",
            Self::Full => "full",
        }
    }
}
