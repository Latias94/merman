//! Shared version-one theme-authoring vectors for first-party contract tests.

/// One theme-authoring vector shared by first-party contract tests.
#[derive(Debug, Clone, Copy)]
pub struct ThemeAuthoringGoldenVectorV1 {
    name: &'static str,
    readable_design_json: &'static [u8],
    canonical_design_json: &'static str,
    canonical_spec_json: &'static str,
    references_json: &'static str,
}

impl ThemeAuthoringGoldenVectorV1 {
    const fn new(
        name: &'static str,
        readable_design_json: &'static [u8],
        canonical_design_json: &'static str,
        canonical_spec_json: &'static str,
        references_json: &'static str,
    ) -> Self {
        Self {
            name,
            readable_design_json,
            canonical_design_json,
            canonical_spec_json,
            references_json,
        }
    }

    /// Returns the vector name.
    pub const fn name(self) -> &'static str {
        self.name
    }

    /// Returns the human-readable persisted design.
    pub const fn readable_design_json(self) -> &'static [u8] {
        self.readable_design_json
    }

    /// Returns the expected canonical design bytes.
    pub fn canonical_design_json(self) -> &'static [u8] {
        self.canonical_design_json.trim_ascii_end().as_bytes()
    }

    /// Returns the expected canonical materialized complete-spec bytes.
    pub fn canonical_spec_json(self) -> &'static [u8] {
        self.canonical_spec_json.trim_ascii_end().as_bytes()
    }

    /// Returns the independently authored reference-location oracle.
    pub const fn references_json(self) -> &'static str {
        self.references_json
    }
}

/// Light and dark authoring vectors for first-party contract verification.
pub const THEME_AUTHORING_GOLDEN_VECTORS_V1: [ThemeAuthoringGoldenVectorV1; 2] = [
    ThemeAuthoringGoldenVectorV1::new(
        "light",
        include_bytes!("../fixtures/authoring-v1/light.design.json"),
        include_str!("../fixtures/authoring-v1/light.design.canonical.json"),
        include_str!("../fixtures/authoring-v1/light.spec.canonical.json"),
        include_str!("../fixtures/authoring-v1/light.references.json"),
    ),
    ThemeAuthoringGoldenVectorV1::new(
        "dark",
        include_bytes!("../fixtures/authoring-v1/dark.design.json"),
        include_str!("../fixtures/authoring-v1/dark.design.canonical.json"),
        include_str!("../fixtures/authoring-v1/dark.spec.canonical.json"),
        include_str!("../fixtures/authoring-v1/dark.references.json"),
    ),
];
