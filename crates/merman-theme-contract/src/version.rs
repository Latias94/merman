use thiserror::Error;

/// A supported authoring, expansion, and complete-spec schema version tuple.
///
/// Values can only be obtained from [`authoring_version_registry`] or
/// [`resolve_authoring_version`]. This prevents callers from constructing a tuple that the
/// contract does not recognize.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ThemeContractVersion {
    authoring_schema_version: u32,
    expansion_version: u32,
    spec_schema_version: u32,
}

impl ThemeContractVersion {
    const fn new(
        authoring_schema_version: u32,
        expansion_version: u32,
        spec_schema_version: u32,
    ) -> Self {
        Self {
            authoring_schema_version,
            expansion_version,
            spec_schema_version,
        }
    }

    /// Returns the persisted authoring envelope schema version.
    pub const fn authoring_schema_version(self) -> u32 {
        self.authoring_schema_version
    }

    /// Returns the deterministic token expansion-table version.
    pub const fn expansion_version(self) -> u32 {
        self.expansion_version
    }

    /// Returns the complete theme-spec wire schema produced by this tuple.
    pub const fn spec_schema_version(self) -> u32 {
        self.spec_schema_version
    }
}

const AUTHORING_VERSION_REGISTRY: &[ThemeContractVersion] = &[ThemeContractVersion::new(1, 1, 1)];

/// Returns the immutable registry of complete authoring-version tuples.
///
/// Callers must not infer support by independently comparing version numbers. Use
/// [`resolve_authoring_version`] for persisted input.
pub const fn authoring_version_registry() -> &'static [ThemeContractVersion] {
    AUTHORING_VERSION_REGISTRY
}

/// Resolves an authoring and expansion version pair to its complete contract tuple.
///
/// Resolution fails closed when the pair is not present in [`authoring_version_registry`].
pub fn resolve_authoring_version(
    authoring_schema_version: u32,
    expansion_version: u32,
) -> Result<ThemeContractVersion, ThemeContractVersionError> {
    AUTHORING_VERSION_REGISTRY
        .iter()
        .copied()
        .find(|version| {
            version.authoring_schema_version == authoring_schema_version
                && version.expansion_version == expansion_version
        })
        .ok_or(ThemeContractVersionError {
            authoring_schema_version,
            expansion_version,
        })
}

/// An unsupported authoring and expansion version pair.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
#[error(
    "unsupported theme authoring version tuple ({authoring_schema_version}, {expansion_version})"
)]
pub struct ThemeContractVersionError {
    authoring_schema_version: u32,
    expansion_version: u32,
}

impl ThemeContractVersionError {
    /// Returns the rejected authoring envelope schema version.
    pub const fn authoring_schema_version(self) -> u32 {
        self.authoring_schema_version
    }

    /// Returns the rejected expansion-table version.
    pub const fn expansion_version(self) -> u32 {
        self.expansion_version
    }
}
