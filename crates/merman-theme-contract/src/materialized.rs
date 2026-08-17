use serde::de::Error as _;
use serde::{Deserialize, Deserializer, Serialize};

use crate::canonical_json::CanonicalJsonError;
use crate::finite::ContainsNonFiniteNumber;
use crate::spec::DiagramThemeSpecWireV1;
use crate::version::THEME_CONTRACT_V1;

const MATERIALIZED_THEME_SCHEMA_VERSION_V1: u32 = 1;

/// The closed version 1 success envelope produced by theme materialization.
///
/// This wire carries only the resolved contract versions and the complete theme spec. Diagnostics,
/// provenance, capability evidence, and render admission belong to their respective authorities.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct MaterializedThemeWireV1 {
    schema_version: u32,
    authoring_schema_version: u32,
    expansion_version: u32,
    spec_schema_version: u32,
    spec: DiagramThemeSpecWireV1,
}

impl MaterializedThemeWireV1 {
    /// Creates a materialized success envelope using the supported version 1 contract tuple.
    pub fn try_new(spec: DiagramThemeSpecWireV1) -> Result<Self, CanonicalJsonError> {
        if spec.contains_non_finite_number() {
            return Err(CanonicalJsonError::invalid_input(
                "materialized theme spec contains a non-finite number",
            ));
        }

        Ok(Self {
            schema_version: MATERIALIZED_THEME_SCHEMA_VERSION_V1,
            authoring_schema_version: THEME_CONTRACT_V1.authoring_schema_version(),
            expansion_version: THEME_CONTRACT_V1.expansion_version(),
            spec_schema_version: THEME_CONTRACT_V1.spec_schema_version(),
            spec,
        })
    }

    /// Returns the materialized success-envelope schema version.
    pub const fn schema_version(&self) -> u32 {
        self.schema_version
    }

    /// Returns the authoring schema version used to produce the complete spec.
    pub const fn authoring_schema_version(&self) -> u32 {
        self.authoring_schema_version
    }

    /// Returns the deterministic expansion-table version used to produce the complete spec.
    pub const fn expansion_version(&self) -> u32 {
        self.expansion_version
    }

    /// Returns the complete-spec schema version carried by this envelope.
    pub const fn spec_schema_version(&self) -> u32 {
        self.spec_schema_version
    }

    /// Returns the materialized complete theme spec.
    pub const fn spec(&self) -> &DiagramThemeSpecWireV1 {
        &self.spec
    }

    /// Consumes the envelope and returns the materialized complete theme spec.
    pub fn into_spec(self) -> DiagramThemeSpecWireV1 {
        self.spec
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct MaterializedThemeWireV1Decode {
    schema_version: u32,
    authoring_schema_version: u32,
    expansion_version: u32,
    spec_schema_version: u32,
    spec: DiagramThemeSpecWireV1,
}

impl<'de> Deserialize<'de> for MaterializedThemeWireV1 {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let decoded = MaterializedThemeWireV1Decode::deserialize(deserializer)?;
        if decoded.schema_version != MATERIALIZED_THEME_SCHEMA_VERSION_V1 {
            return Err(D::Error::custom(format_args!(
                "MaterializedThemeWireV1 requires schema version {MATERIALIZED_THEME_SCHEMA_VERSION_V1}"
            )));
        }
        if decoded.authoring_schema_version != THEME_CONTRACT_V1.authoring_schema_version()
            || decoded.expansion_version != THEME_CONTRACT_V1.expansion_version()
            || decoded.spec_schema_version != THEME_CONTRACT_V1.spec_schema_version()
        {
            return Err(D::Error::custom(format_args!(
                "MaterializedThemeWireV1 requires contract version tuple ({}, {}, {})",
                THEME_CONTRACT_V1.authoring_schema_version(),
                THEME_CONTRACT_V1.expansion_version(),
                THEME_CONTRACT_V1.spec_schema_version()
            )));
        }

        Self::try_new(decoded.spec).map_err(D::Error::custom)
    }
}
