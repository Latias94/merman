use std::fmt;

use serde::de::Error as _;
use serde::ser::Error as _;
use serde::{Deserialize, Deserializer, Serialize, Serializer};

use crate::version::THEME_CONTRACT_V1;

const THEME_MATERIALIZATION_ERROR_SCHEMA_VERSION_V1: u32 = 1;
const MAX_DIAGNOSTIC_PATH_BYTES_V1: usize = 512;
const MAX_DIAGNOSTIC_MESSAGE_BYTES_V1: usize = 512;
const MAX_DIAGNOSTIC_IDENTIFIER_BYTES_V1: usize = 128;
const MAX_DIAGNOSTIC_TARGET_ID_BYTES_V1: usize = 64 * 1024;
const EMPTY_SERIES_PATH_V1: &str = "/tokens/series";
const RULE_BUDGET_PATH_V1: &str = "/styles";

const SUPPORTED_VERSION_TUPLE_V1: ThemeAuthoringVersionTupleDetailsV1 =
    ThemeAuthoringVersionTupleDetailsV1 {
        authoring_schema_version: THEME_CONTRACT_V1.authoring_schema_version(),
        expansion_version: THEME_CONTRACT_V1.expansion_version(),
    };

/// One closed, fatal version 1 theme-materialization diagnostic.
///
/// The display message is serialized for people but is deliberately excluded from equality. A
/// diagnostic's stable identity is its code, severity, path, and typed details.
#[derive(Debug, Clone)]
pub struct ThemeMaterializationDiagnosticV1 {
    wire: ThemeMaterializationDiagnosticWireV1,
}

impl ThemeMaterializationDiagnosticV1 {
    /// Maximum encoded JSON Pointer length accepted by this diagnostic contract.
    #[doc(hidden)]
    pub const MAX_PATH_BYTES: usize = MAX_DIAGNOSTIC_PATH_BYTES_V1;

    /// Creates an unsupported authoring-version tuple diagnostic for renderer adapters.
    #[doc(hidden)]
    pub fn unsupported_version_tuple(
        actual_authoring_schema_version: u32,
        actual_expansion_version: u32,
        message: impl Into<String>,
    ) -> Self {
        Self::from_wire(
            ThemeMaterializationDiagnosticWireV1::UnsupportedVersionTuple {
                severity: ThemeMaterializationSeverityV1::Error,
                path: String::new(),
                details: UnsupportedVersionTupleDetailsV1 {
                    actual: ThemeAuthoringVersionTupleDetailsV1 {
                        authoring_schema_version: actual_authoring_schema_version,
                        expansion_version: actual_expansion_version,
                    },
                    supported: [SUPPORTED_VERSION_TUPLE_V1],
                },
                message: message.into(),
            },
        )
    }

    /// Creates an invalid definition-JSON diagnostic for renderer adapters.
    #[doc(hidden)]
    pub fn invalid_definition_json(
        reason_id: impl Into<String>,
        message: impl Into<String>,
    ) -> Self {
        Self::from_wire(
            ThemeMaterializationDiagnosticWireV1::InvalidDefinitionJson {
                severity: ThemeMaterializationSeverityV1::Error,
                path: String::new(),
                details: InvalidDefinitionJsonDetailsV1 {
                    reason_id: reason_id.into(),
                },
                message: message.into(),
            },
        )
    }

    /// Creates an invalid token-value diagnostic for renderer adapters.
    #[doc(hidden)]
    pub fn invalid_token_value(
        path: impl Into<String>,
        expected_domain_id: impl Into<String>,
        message: impl Into<String>,
    ) -> Self {
        Self::from_wire(ThemeMaterializationDiagnosticWireV1::InvalidTokenValue {
            severity: ThemeMaterializationSeverityV1::Error,
            path: path.into(),
            details: InvalidTokenValueDetailsV1 {
                expected_domain_id: expected_domain_id.into(),
            },
            message: message.into(),
        })
    }

    /// Creates an empty authored-series diagnostic for renderer adapters.
    #[doc(hidden)]
    pub fn empty_series(message: impl Into<String>) -> Self {
        Self::from_wire(ThemeMaterializationDiagnosticWireV1::EmptySeries {
            severity: ThemeMaterializationSeverityV1::Error,
            path: EMPTY_SERIES_PATH_V1.to_owned(),
            details: EmptySeriesDetailsV1 {},
            message: message.into(),
        })
    }

    /// Creates an authored-rule budget diagnostic for renderer adapters.
    #[doc(hidden)]
    pub fn rule_budget_exceeded(
        limit_id: impl Into<String>,
        actual: usize,
        max: usize,
        message: impl Into<String>,
    ) -> Self {
        Self::from_wire(ThemeMaterializationDiagnosticWireV1::RuleBudgetExceeded {
            severity: ThemeMaterializationSeverityV1::Error,
            path: RULE_BUDGET_PATH_V1.to_owned(),
            details: RuleBudgetExceededDetailsV1 {
                limit_id: limit_id.into(),
                actual: wire_count(actual),
                max: wire_count(max),
            },
            message: message.into(),
        })
    }

    /// Creates a duplicate authored-palette target diagnostic for renderer adapters.
    #[doc(hidden)]
    pub fn duplicate_palette_target(
        path: impl Into<String>,
        target_id: impl Into<String>,
        first_authored_index: usize,
        duplicate_authored_index: usize,
        message: impl Into<String>,
    ) -> Self {
        Self::from_wire(
            ThemeMaterializationDiagnosticWireV1::DuplicatePaletteTarget {
                severity: ThemeMaterializationSeverityV1::Error,
                path: path.into(),
                details: DuplicatePaletteTargetDetailsV1 {
                    target_id: target_id.into(),
                    first_authored_index: wire_count(first_authored_index),
                    duplicate_authored_index: wire_count(duplicate_authored_index),
                },
                message: message.into(),
            },
        )
    }

    /// Creates an unsupported effect-reference diagnostic for renderer adapters.
    #[doc(hidden)]
    pub fn effect_reference_not_supported(
        path: impl Into<String>,
        authored_index: usize,
        message: impl Into<String>,
    ) -> Self {
        Self::from_wire(
            ThemeMaterializationDiagnosticWireV1::EffectReferenceNotSupported {
                severity: ThemeMaterializationSeverityV1::Error,
                path: path.into(),
                details: EffectReferenceNotSupportedDetailsV1 {
                    authored_index: wire_count(authored_index),
                },
                message: message.into(),
            },
        )
    }

    /// Creates a materialization resource-limit diagnostic for renderer adapters.
    #[doc(hidden)]
    pub fn resource_limit_exceeded(
        path: impl Into<String>,
        limit_id: impl Into<String>,
        actual: usize,
        max: usize,
        message: impl Into<String>,
    ) -> Self {
        Self::from_wire(
            ThemeMaterializationDiagnosticWireV1::ResourceLimitExceeded {
                severity: ThemeMaterializationSeverityV1::Error,
                path: path.into(),
                details: ResourceLimitExceededDetailsV1 {
                    limit_id: limit_id.into(),
                    actual: wire_count(actual),
                    max: wire_count(max),
                },
                message: message.into(),
            },
        )
    }

    /// Returns the stable diagnostic code.
    pub const fn code(&self) -> &'static str {
        match &self.wire {
            ThemeMaterializationDiagnosticWireV1::UnsupportedVersionTuple { .. } => {
                "theme-authoring.unsupported-version-tuple"
            }
            ThemeMaterializationDiagnosticWireV1::InvalidDefinitionJson { .. } => {
                "theme-authoring.invalid-definition-json"
            }
            ThemeMaterializationDiagnosticWireV1::InvalidTokenValue { .. } => {
                "theme-authoring.invalid-token-value"
            }
            ThemeMaterializationDiagnosticWireV1::EmptySeries { .. } => {
                "theme-authoring.empty-series"
            }
            ThemeMaterializationDiagnosticWireV1::RuleBudgetExceeded { .. } => {
                "theme-authoring.rule-budget-exceeded"
            }
            ThemeMaterializationDiagnosticWireV1::DuplicatePaletteTarget { .. } => {
                "theme-authoring.duplicate-palette-target"
            }
            ThemeMaterializationDiagnosticWireV1::EffectReferenceNotSupported { .. } => {
                "theme-authoring.effect-reference-not-supported"
            }
            ThemeMaterializationDiagnosticWireV1::ResourceLimitExceeded { .. } => {
                "theme-authoring.resource-limit-exceeded"
            }
        }
    }

    /// Returns the only severity admitted by the version 1 fatal wire.
    pub const fn severity(&self) -> &'static str {
        "error"
    }

    /// Returns the bounded RFC 6901 JSON Pointer to the failed authoring value.
    /// Resource errors in oversized unknown paths identify the nearest bounded ancestor.
    pub fn path(&self) -> &str {
        self.wire.path()
    }

    /// Returns the bounded, non-identity display message.
    pub fn message(&self) -> &str {
        self.wire.message()
    }

    /// Returns the rejected version tuple when this is an unsupported-version diagnostic.
    pub const fn actual_version_tuple(&self) -> Option<(u32, u32)> {
        match &self.wire {
            ThemeMaterializationDiagnosticWireV1::UnsupportedVersionTuple { details, .. } => {
                Some((
                    details.actual.authoring_schema_version,
                    details.actual.expansion_version,
                ))
            }
            _ => None,
        }
    }

    /// Returns the stable coarse JSON rejection reason when present.
    pub fn reason_id(&self) -> Option<&str> {
        match &self.wire {
            ThemeMaterializationDiagnosticWireV1::InvalidDefinitionJson { details, .. } => {
                Some(&details.reason_id)
            }
            _ => None,
        }
    }

    /// Returns the expected value-domain identifier when present.
    pub fn expected_domain_id(&self) -> Option<&str> {
        match &self.wire {
            ThemeMaterializationDiagnosticWireV1::InvalidTokenValue { details, .. } => {
                Some(&details.expected_domain_id)
            }
            _ => None,
        }
    }

    /// Returns the stable resource or rule limit identifier when present.
    pub fn limit_id(&self) -> Option<&str> {
        match &self.wire {
            ThemeMaterializationDiagnosticWireV1::RuleBudgetExceeded { details, .. } => {
                Some(&details.limit_id)
            }
            ThemeMaterializationDiagnosticWireV1::ResourceLimitExceeded { details, .. } => {
                Some(&details.limit_id)
            }
            _ => None,
        }
    }

    /// Returns the observed bounded count when present.
    pub const fn actual(&self) -> Option<u64> {
        match &self.wire {
            ThemeMaterializationDiagnosticWireV1::RuleBudgetExceeded { details, .. } => {
                Some(details.actual)
            }
            ThemeMaterializationDiagnosticWireV1::ResourceLimitExceeded { details, .. } => {
                Some(details.actual)
            }
            _ => None,
        }
    }

    /// Returns the admitted maximum count when present.
    pub const fn max(&self) -> Option<u64> {
        match &self.wire {
            ThemeMaterializationDiagnosticWireV1::RuleBudgetExceeded { details, .. } => {
                Some(details.max)
            }
            ThemeMaterializationDiagnosticWireV1::ResourceLimitExceeded { details, .. } => {
                Some(details.max)
            }
            _ => None,
        }
    }

    /// Returns the duplicated palette target identifier when present.
    pub fn target_id(&self) -> Option<&str> {
        match &self.wire {
            ThemeMaterializationDiagnosticWireV1::DuplicatePaletteTarget { details, .. } => {
                Some(&details.target_id)
            }
            _ => None,
        }
    }

    /// Returns the first authored palette index when present.
    pub const fn first_authored_index(&self) -> Option<u64> {
        match &self.wire {
            ThemeMaterializationDiagnosticWireV1::DuplicatePaletteTarget { details, .. } => {
                Some(details.first_authored_index)
            }
            _ => None,
        }
    }

    /// Returns the duplicate authored palette index when present.
    pub const fn duplicate_authored_index(&self) -> Option<u64> {
        match &self.wire {
            ThemeMaterializationDiagnosticWireV1::DuplicatePaletteTarget { details, .. } => {
                Some(details.duplicate_authored_index)
            }
            _ => None,
        }
    }

    /// Returns the authored rule index when present.
    pub const fn authored_index(&self) -> Option<u64> {
        match &self.wire {
            ThemeMaterializationDiagnosticWireV1::EffectReferenceNotSupported {
                details, ..
            } => Some(details.authored_index),
            _ => None,
        }
    }

    fn from_wire(wire: ThemeMaterializationDiagnosticWireV1) -> Self {
        let diagnostic = Self { wire };
        if let Err(error) = diagnostic.validate() {
            panic!("invalid renderer materialization diagnostic: {error}");
        }
        diagnostic
    }

    fn validate(&self) -> Result<(), &'static str> {
        validate_json_pointer(self.path())?;
        validate_message(self.message())?;
        self.wire.validate_details()
    }
}

impl PartialEq for ThemeMaterializationDiagnosticV1 {
    fn eq(&self, other: &Self) -> bool {
        self.code() == other.code()
            && self.path() == other.path()
            && self.actual_version_tuple() == other.actual_version_tuple()
            && self.reason_id() == other.reason_id()
            && self.expected_domain_id() == other.expected_domain_id()
            && self.limit_id() == other.limit_id()
            && self.actual() == other.actual()
            && self.max() == other.max()
            && self.target_id() == other.target_id()
            && self.first_authored_index() == other.first_authored_index()
            && self.duplicate_authored_index() == other.duplicate_authored_index()
            && self.authored_index() == other.authored_index()
    }
}

impl Eq for ThemeMaterializationDiagnosticV1 {}

impl Serialize for ThemeMaterializationDiagnosticV1 {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        self.validate().map_err(S::Error::custom)?;
        self.wire.serialize(serializer)
    }
}

impl<'de> Deserialize<'de> for ThemeMaterializationDiagnosticV1 {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let diagnostic = Self {
            wire: ThemeMaterializationDiagnosticWireV1::deserialize(deserializer)?,
        };
        diagnostic.validate().map_err(D::Error::custom)?;
        Ok(diagnostic)
    }
}

/// The closed version 1 fatal theme-materialization error envelope.
///
/// Version 1 contains exactly one diagnostic and never carries a partial materialized spec.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ThemeMaterializationErrorV1 {
    schema_version: u32,
    diagnostics: [ThemeMaterializationDiagnosticV1; 1],
}

impl ThemeMaterializationErrorV1 {
    /// Creates a fatal error envelope from exactly one renderer diagnostic.
    #[doc(hidden)]
    pub const fn from_diagnostic(diagnostic: ThemeMaterializationDiagnosticV1) -> Self {
        Self {
            schema_version: THEME_MATERIALIZATION_ERROR_SCHEMA_VERSION_V1,
            diagnostics: [diagnostic],
        }
    }

    /// Returns the materialization-error envelope schema version.
    pub const fn schema_version(&self) -> u32 {
        self.schema_version
    }

    /// Returns the version 1 fixed-width diagnostic array.
    pub const fn diagnostics(&self) -> &[ThemeMaterializationDiagnosticV1; 1] {
        &self.diagnostics
    }

    /// Returns the only fatal diagnostic in this envelope.
    pub const fn diagnostic(&self) -> &ThemeMaterializationDiagnosticV1 {
        &self.diagnostics[0]
    }

    /// Consumes the envelope and returns its only fatal diagnostic.
    pub fn into_diagnostic(self) -> ThemeMaterializationDiagnosticV1 {
        let [diagnostic] = self.diagnostics;
        diagnostic
    }
}

impl fmt::Display for ThemeMaterializationErrorV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.diagnostic().message())
    }
}

impl std::error::Error for ThemeMaterializationErrorV1 {}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ThemeMaterializationErrorV1Decode {
    schema_version: u32,
    diagnostics: [ThemeMaterializationDiagnosticV1; 1],
}

impl<'de> Deserialize<'de> for ThemeMaterializationErrorV1 {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let decoded = ThemeMaterializationErrorV1Decode::deserialize(deserializer)?;
        if decoded.schema_version != THEME_MATERIALIZATION_ERROR_SCHEMA_VERSION_V1 {
            return Err(D::Error::custom(format_args!(
                "ThemeMaterializationErrorV1 requires schema version {THEME_MATERIALIZATION_ERROR_SCHEMA_VERSION_V1}"
            )));
        }
        Ok(Self {
            schema_version: decoded.schema_version,
            diagnostics: decoded.diagnostics,
        })
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "code", deny_unknown_fields)]
enum ThemeMaterializationDiagnosticWireV1 {
    #[serde(rename = "theme-authoring.unsupported-version-tuple")]
    UnsupportedVersionTuple {
        severity: ThemeMaterializationSeverityV1,
        path: String,
        details: UnsupportedVersionTupleDetailsV1,
        message: String,
    },
    #[serde(rename = "theme-authoring.invalid-definition-json")]
    InvalidDefinitionJson {
        severity: ThemeMaterializationSeverityV1,
        path: String,
        details: InvalidDefinitionJsonDetailsV1,
        message: String,
    },
    #[serde(rename = "theme-authoring.invalid-token-value")]
    InvalidTokenValue {
        severity: ThemeMaterializationSeverityV1,
        path: String,
        details: InvalidTokenValueDetailsV1,
        message: String,
    },
    #[serde(rename = "theme-authoring.empty-series")]
    EmptySeries {
        severity: ThemeMaterializationSeverityV1,
        path: String,
        details: EmptySeriesDetailsV1,
        message: String,
    },
    #[serde(rename = "theme-authoring.rule-budget-exceeded")]
    RuleBudgetExceeded {
        severity: ThemeMaterializationSeverityV1,
        path: String,
        details: RuleBudgetExceededDetailsV1,
        message: String,
    },
    #[serde(rename = "theme-authoring.duplicate-palette-target")]
    DuplicatePaletteTarget {
        severity: ThemeMaterializationSeverityV1,
        path: String,
        details: DuplicatePaletteTargetDetailsV1,
        message: String,
    },
    #[serde(rename = "theme-authoring.effect-reference-not-supported")]
    EffectReferenceNotSupported {
        severity: ThemeMaterializationSeverityV1,
        path: String,
        details: EffectReferenceNotSupportedDetailsV1,
        message: String,
    },
    #[serde(rename = "theme-authoring.resource-limit-exceeded")]
    ResourceLimitExceeded {
        severity: ThemeMaterializationSeverityV1,
        path: String,
        details: ResourceLimitExceededDetailsV1,
        message: String,
    },
}

impl ThemeMaterializationDiagnosticWireV1 {
    fn path(&self) -> &str {
        match self {
            Self::UnsupportedVersionTuple { path, .. }
            | Self::InvalidDefinitionJson { path, .. }
            | Self::InvalidTokenValue { path, .. }
            | Self::EmptySeries { path, .. }
            | Self::RuleBudgetExceeded { path, .. }
            | Self::DuplicatePaletteTarget { path, .. }
            | Self::EffectReferenceNotSupported { path, .. }
            | Self::ResourceLimitExceeded { path, .. } => path,
        }
    }

    fn message(&self) -> &str {
        match self {
            Self::UnsupportedVersionTuple { message, .. }
            | Self::InvalidDefinitionJson { message, .. }
            | Self::InvalidTokenValue { message, .. }
            | Self::EmptySeries { message, .. }
            | Self::RuleBudgetExceeded { message, .. }
            | Self::DuplicatePaletteTarget { message, .. }
            | Self::EffectReferenceNotSupported { message, .. }
            | Self::ResourceLimitExceeded { message, .. } => message,
        }
    }

    fn validate_details(&self) -> Result<(), &'static str> {
        match self {
            Self::UnsupportedVersionTuple { details, .. } => {
                if details.supported != [SUPPORTED_VERSION_TUPLE_V1] {
                    return Err("unsupported-version details must carry the version 1 registry");
                }
                Ok(())
            }
            Self::InvalidDefinitionJson { details, .. } => validate_identifier(&details.reason_id),
            Self::InvalidTokenValue { details, .. } => {
                validate_identifier(&details.expected_domain_id)
            }
            Self::EmptySeries { .. } => Ok(()),
            Self::RuleBudgetExceeded { details, .. } => validate_identifier(&details.limit_id),
            Self::DuplicatePaletteTarget { details, .. } => validate_target_id(&details.target_id),
            Self::EffectReferenceNotSupported { .. } => Ok(()),
            Self::ResourceLimitExceeded { details, .. } => validate_identifier(&details.limit_id),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
enum ThemeMaterializationSeverityV1 {
    Error,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct ThemeAuthoringVersionTupleDetailsV1 {
    authoring_schema_version: u32,
    expansion_version: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct UnsupportedVersionTupleDetailsV1 {
    actual: ThemeAuthoringVersionTupleDetailsV1,
    supported: [ThemeAuthoringVersionTupleDetailsV1; 1],
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct InvalidDefinitionJsonDetailsV1 {
    reason_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct InvalidTokenValueDetailsV1 {
    expected_domain_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct EmptySeriesDetailsV1 {}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct RuleBudgetExceededDetailsV1 {
    limit_id: String,
    actual: u64,
    max: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct DuplicatePaletteTargetDetailsV1 {
    target_id: String,
    first_authored_index: u64,
    duplicate_authored_index: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct EffectReferenceNotSupportedDetailsV1 {
    authored_index: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct ResourceLimitExceededDetailsV1 {
    limit_id: String,
    actual: u64,
    max: u64,
}

fn validate_json_pointer(path: &str) -> Result<(), &'static str> {
    if path.len() > MAX_DIAGNOSTIC_PATH_BYTES_V1 {
        return Err("diagnostic path exceeds 512 UTF-8 bytes");
    }
    if path.is_empty() {
        return Ok(());
    }
    if !path.starts_with('/') {
        return Err("diagnostic path is not an RFC 6901 JSON Pointer");
    }

    let bytes = path.as_bytes();
    let mut index = 0;
    while index < bytes.len() {
        if bytes[index] == b'~' {
            index += 1;
            if index == bytes.len() || !matches!(bytes[index], b'0' | b'1') {
                return Err("diagnostic path contains an invalid RFC 6901 escape");
            }
        }
        index += 1;
    }
    Ok(())
}

fn validate_message(message: &str) -> Result<(), &'static str> {
    if message.len() > MAX_DIAGNOSTIC_MESSAGE_BYTES_V1 {
        Err("diagnostic message exceeds 512 UTF-8 bytes")
    } else {
        Ok(())
    }
}

fn validate_identifier(value: &str) -> Result<(), &'static str> {
    if value.len() > MAX_DIAGNOSTIC_IDENTIFIER_BYTES_V1 {
        Err("diagnostic identifier exceeds 128 UTF-8 bytes")
    } else {
        Ok(())
    }
}

fn validate_target_id(value: &str) -> Result<(), &'static str> {
    if value.len() > MAX_DIAGNOSTIC_TARGET_ID_BYTES_V1 {
        Err("diagnostic target identifier exceeds 64 KiB")
    } else {
        Ok(())
    }
}

fn wire_count(value: usize) -> u64 {
    u64::try_from(value).expect("theme materialization counts must fit the version 1 u64 wire")
}
