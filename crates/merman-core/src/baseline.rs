//! Active upstream Mermaid baseline metadata.
//!
//! These constants describe the source revision that production parsing, layout, rendering, and
//! parity fixtures target.

/// Upstream Mermaid tag pinned by this repository.
pub const PINNED_MERMAID_BASELINE_TAG: &str = "mermaid@11.16.0";

/// Upstream Mermaid semver pinned by this repository.
pub const PINNED_MERMAID_BASELINE_VERSION: &str = "11.16.0";

/// Filesystem/module-name-safe form of [`PINNED_MERMAID_BASELINE_VERSION`].
pub const PINNED_MERMAID_BASELINE_VERSION_SUFFIX: &str = "11_16_0";

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
