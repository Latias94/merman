pub const PINNED_MERMAID_BASELINE_TAG: &str = "mermaid@11.15.0";
pub const PINNED_MERMAID_BASELINE_VERSION: &str = "11.15.0";
pub const PINNED_MERMAID_BASELINE_VERSION_SUFFIX: &str = "11_15_0";

// Generated override/file names still carry the old suffix in many places.
// Keep that fact explicit instead of pretending it is the active baseline.
pub const LEGACY_GENERATED_BASELINE_SUFFIX: &str = "11_12_2";

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum BaselineRegistryProfile {
    Tiny,
    Full,
}

impl BaselineRegistryProfile {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Tiny => "tiny",
            Self::Full => "full",
        }
    }
}
