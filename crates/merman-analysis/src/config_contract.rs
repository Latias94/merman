use crate::{
    AnalysisOptions, AnalysisRuleProfile, DiagnosticSeverity,
    MAX_DOCUMENT_DIAGRAMS_RESOURCE_LIMIT_ID,
    options_json::{AnalysisOptionsJson, AnalysisOptionsJsonError},
};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::BTreeMap;

mod decode;
mod schema;
#[cfg(test)]
mod tests;

pub(crate) use decode::decode_resource_options;

pub const FIXED_TODAY_SCHEMA_PATTERN: &str = concat!(
    r"^(?:\d{4}|\+(?:[1-9]\d{4,8}|1\d{9}|20\d{8}|21[0-3]\d{7}|214[0-6]\d{6}|",
    r"2147[0-3]\d{5}|21474[0-7]\d{4}|214748[0-2]\d{3}|2147483[0-5]\d{2}|",
    r"21474836[0-3]\d|214748364[0-7])|-(?:000[1-9]|00[1-9]\d|0[1-9]\d{2}|",
    r"[1-9]\d{3}|[1-9]\d{4,8}|1\d{9}|20\d{8}|21[0-3]\d{7}|214[0-6]\d{6}|",
    r"2147[0-3]\d{5}|21474[0-7]\d{4}|214748[0-2]\d{3}|2147483[0-5]\d{2}|",
    r"21474836[0-3]\d|214748364[0-8]))-\d{2}-\d{2}$",
);

/// Version of the host-neutral client constraints projected from the analysis contract.
pub const ANALYSIS_CONFIG_CLIENT_CONSTRAINTS_VERSION: u32 = 1;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
enum AnalysisConfigRoot {
    Direct,
    Merman,
    Analysis,
}

impl AnalysisConfigRoot {
    pub const ALL: [Self; 3] = [Self::Direct, Self::Merman, Self::Analysis];

    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Direct => "direct",
            Self::Merman => "merman",
            Self::Analysis => "analysis",
        }
    }

    const fn wrapper_key(self) -> Option<&'static str> {
        match self {
            Self::Direct => None,
            Self::Merman => Some("merman"),
            Self::Analysis => Some("analysis"),
        }
    }
}

fn wrapped_config_roots() -> impl Iterator<Item = (AnalysisConfigRoot, &'static str)> {
    AnalysisConfigRoot::ALL
        .into_iter()
        .filter_map(|root| root.wrapper_key().map(|key| (root, key)))
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AnalysisConfigChange {
    Unchanged,
    DiagnosticsOnly,
    SnapshotAffecting,
}

impl AnalysisConfigChange {
    pub const fn affects_diagnostics(self) -> bool {
        !matches!(self, Self::Unchanged)
    }

    pub const fn affects_snapshots(self) -> bool {
        matches!(self, Self::SnapshotAffecting)
    }
}

/// Invalidation scope owned by one analysis configuration policy.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AnalysisConfigChangeScope {
    DiagnosticsOnly,
    SnapshotAffecting,
}

impl AnalysisConfigChangeScope {
    const fn as_str(self) -> &'static str {
        match self {
            Self::DiagnosticsOnly => "diagnostics_only",
            Self::SnapshotAffecting => "snapshot_affecting",
        }
    }

    const fn change(self) -> AnalysisConfigChange {
        match self {
            Self::DiagnosticsOnly => AnalysisConfigChange::DiagnosticsOnly,
            Self::SnapshotAffecting => AnalysisConfigChange::SnapshotAffecting,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
enum AnalysisConfigCompatibility {
    ForwardCompatible,
    Strict,
}

impl AnalysisConfigCompatibility {
    const fn as_str(self) -> &'static str {
        match self {
            Self::ForwardCompatible => "forward_compatible",
            Self::Strict => "strict",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum AnalysisConfigObjectId {
    Options,
    Resources,
    Lint,
    RuleSeverityOverride,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum AnalysisConfigFieldId {
    Options(AnalysisOptionsFieldId),
    Resources(ResourceOptionsFieldId),
    Lint(LintOptionsFieldId),
    RuleSeverityOverride(RuleSeverityOverrideFieldId),
}

impl AnalysisConfigFieldId {
    const fn parent(self) -> AnalysisConfigObjectId {
        match self {
            Self::Options(_) => AnalysisConfigObjectId::Options,
            Self::Resources(_) => AnalysisConfigObjectId::Resources,
            Self::Lint(_) => AnalysisConfigObjectId::Lint,
            Self::RuleSeverityOverride(_) => AnalysisConfigObjectId::RuleSeverityOverride,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum AnalysisOptionsFieldId {
    FixedToday,
    FixedLocalOffsetMinutes,
    SiteConfig,
    Resources,
    Lint,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum ResourceOptionsFieldId {
    Limits,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum LintOptionsFieldId {
    Profile,
    EnableRules,
    DisableRules,
    RuleSeverities,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum RuleSeverityOverrideFieldId {
    RuleId,
    Severity,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum AnalysisConfigPolicyId {
    FixedToday,
    FixedLocalOffsetMinutes,
    SiteConfig,
    Resources,
    Lint,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct AnalysisConfigPolicyDescriptor {
    id: AnalysisConfigPolicyId,
    change_scope: AnalysisConfigChangeScope,
}

impl AnalysisConfigPolicyDescriptor {
    fn changed(self, current: &AnalysisOptions, next: &AnalysisOptions) -> bool {
        match self.id {
            AnalysisConfigPolicyId::FixedToday => {
                current.runtime_policy().fixed_today() != next.runtime_policy().fixed_today()
            }
            AnalysisConfigPolicyId::FixedLocalOffsetMinutes => {
                current.runtime_policy().fixed_local_offset_minutes()
                    != next.runtime_policy().fixed_local_offset_minutes()
            }
            AnalysisConfigPolicyId::SiteConfig => current.site_config() != next.site_config(),
            AnalysisConfigPolicyId::Resources => {
                current.snapshot_policy().resources != next.snapshot_policy().resources
            }
            AnalysisConfigPolicyId::Lint => current.diagnostic_policy() != next.diagnostic_policy(),
        }
    }
}

const ANALYSIS_CONFIG_POLICIES: [AnalysisConfigPolicyDescriptor; 5] = [
    AnalysisConfigPolicyDescriptor {
        id: AnalysisConfigPolicyId::FixedToday,
        change_scope: AnalysisConfigChangeScope::SnapshotAffecting,
    },
    AnalysisConfigPolicyDescriptor {
        id: AnalysisConfigPolicyId::FixedLocalOffsetMinutes,
        change_scope: AnalysisConfigChangeScope::SnapshotAffecting,
    },
    AnalysisConfigPolicyDescriptor {
        id: AnalysisConfigPolicyId::SiteConfig,
        change_scope: AnalysisConfigChangeScope::SnapshotAffecting,
    },
    AnalysisConfigPolicyDescriptor {
        id: AnalysisConfigPolicyId::Resources,
        change_scope: AnalysisConfigChangeScope::SnapshotAffecting,
    },
    AnalysisConfigPolicyDescriptor {
        id: AnalysisConfigPolicyId::Lint,
        change_scope: AnalysisConfigChangeScope::DiagnosticsOnly,
    },
];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum AnalysisConfigEnumSource {
    Profiles,
    RuleIds,
    Severities,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum AnalysisConfigArrayItem {
    RuleId,
    RuleSeverityOverride,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum AnalysisConfigRuntimeConstraint {
    CanonicalCivilDate,
    RepresentableLocalMidnight { offset_setting_path: &'static str },
}

impl AnalysisConfigRuntimeConstraint {
    const fn as_str(self) -> &'static str {
        match self {
            Self::CanonicalCivilDate => "canonical_civil_date",
            Self::RepresentableLocalMidnight { .. } => "representable_local_midnight",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum AnalysisConfigValueKind {
    String {
        enum_source: Option<AnalysisConfigEnumSource>,
        pattern: Option<&'static str>,
    },
    Integer {
        minimum: i64,
        maximum: i64,
    },
    JsonObject,
    Object(AnalysisConfigObjectId),
    Array(AnalysisConfigArrayItem),
    ResourceLimits,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum AnalysisConfigDefault {
    None,
    RuleProfile(AnalysisRuleProfile),
    EmptyArray,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct AnalysisConfigObjectDescriptor {
    id: AnalysisConfigObjectId,
    path: &'static str,
    compatibility: AnalysisConfigCompatibility,
    removed_keys: &'static [&'static str],
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct AnalysisConfigFieldDescriptor {
    id: AnalysisConfigFieldId,
    key: &'static str,
    path: &'static str,
    policy: AnalysisConfigPolicyId,
    value_kind: AnalysisConfigValueKind,
    nullable: bool,
    required: bool,
    default: AnalysisConfigDefault,
    description: &'static str,
    runtime_constraints: &'static [AnalysisConfigRuntimeConstraint],
}

impl AnalysisConfigFieldDescriptor {
    fn change_scope(self) -> AnalysisConfigChangeScope {
        policy_descriptor(self.policy).change_scope
    }
}

const ANALYSIS_CONFIG_OBJECTS: [AnalysisConfigObjectDescriptor; 4] = [
    AnalysisConfigObjectDescriptor {
        id: AnalysisConfigObjectId::Options,
        path: "analysis options",
        compatibility: AnalysisConfigCompatibility::ForwardCompatible,
        removed_keys: &["parse"],
    },
    AnalysisConfigObjectDescriptor {
        id: AnalysisConfigObjectId::Resources,
        path: "resources",
        compatibility: AnalysisConfigCompatibility::Strict,
        removed_keys: &[],
    },
    AnalysisConfigObjectDescriptor {
        id: AnalysisConfigObjectId::Lint,
        path: "lint",
        compatibility: AnalysisConfigCompatibility::ForwardCompatible,
        removed_keys: &[],
    },
    AnalysisConfigObjectDescriptor {
        id: AnalysisConfigObjectId::RuleSeverityOverride,
        path: "lint.rule_severities entry",
        compatibility: AnalysisConfigCompatibility::ForwardCompatible,
        removed_keys: &[],
    },
];

const ANALYSIS_CONFIG_FIELDS: [AnalysisConfigFieldDescriptor; 12] = [
    AnalysisConfigFieldDescriptor {
        id: AnalysisConfigFieldId::Options(AnalysisOptionsFieldId::FixedToday),
        key: "fixed_today",
        path: "fixed_today",
        policy: AnalysisConfigPolicyId::FixedToday,
        value_kind: AnalysisConfigValueKind::String {
            enum_source: None,
            pattern: Some(FIXED_TODAY_SCHEMA_PATTERN),
        },
        nullable: true,
        required: false,
        default: AnalysisConfigDefault::None,
        description: "Canonical fixed local civil date. Years 0000 through 9999 use YYYY-MM-DD; later years use +YEAR-MM-DD and negative years use -YEAR-MM-DD. Calendar validity and the representable local-midnight instant are validated when the configuration is applied.",
        runtime_constraints: &[
            AnalysisConfigRuntimeConstraint::CanonicalCivilDate,
            AnalysisConfigRuntimeConstraint::RepresentableLocalMidnight {
                offset_setting_path: "fixed_local_offset_minutes",
            },
        ],
    },
    AnalysisConfigFieldDescriptor {
        id: AnalysisConfigFieldId::Options(AnalysisOptionsFieldId::FixedLocalOffsetMinutes),
        key: "fixed_local_offset_minutes",
        path: "fixed_local_offset_minutes",
        policy: AnalysisConfigPolicyId::FixedLocalOffsetMinutes,
        value_kind: AnalysisConfigValueKind::Integer {
            minimum: -1439,
            maximum: 1439,
        },
        nullable: true,
        required: false,
        default: AnalysisConfigDefault::None,
        description: "Fixed local UTC offset in minutes.",
        runtime_constraints: &[],
    },
    AnalysisConfigFieldDescriptor {
        id: AnalysisConfigFieldId::Options(AnalysisOptionsFieldId::SiteConfig),
        key: "site_config",
        path: "site_config",
        policy: AnalysisConfigPolicyId::SiteConfig,
        value_kind: AnalysisConfigValueKind::JsonObject,
        nullable: true,
        required: false,
        default: AnalysisConfigDefault::None,
        description: "Mermaid site configuration forwarded to the shared parser/config layer.",
        runtime_constraints: &[],
    },
    AnalysisConfigFieldDescriptor {
        id: AnalysisConfigFieldId::Options(AnalysisOptionsFieldId::Resources),
        key: "resources",
        path: "resources",
        policy: AnalysisConfigPolicyId::Resources,
        value_kind: AnalysisConfigValueKind::Object(AnalysisConfigObjectId::Resources),
        nullable: true,
        required: false,
        default: AnalysisConfigDefault::None,
        description: "Strict, versioned resource limits for bounded analysis.",
        runtime_constraints: &[],
    },
    AnalysisConfigFieldDescriptor {
        id: AnalysisConfigFieldId::Resources(ResourceOptionsFieldId::Limits),
        key: "limits",
        path: "resources.limits",
        policy: AnalysisConfigPolicyId::Resources,
        value_kind: AnalysisConfigValueKind::ResourceLimits,
        nullable: false,
        required: false,
        default: AnalysisConfigDefault::None,
        description: "Owner-defined resource limit values.",
        runtime_constraints: &[],
    },
    AnalysisConfigFieldDescriptor {
        id: AnalysisConfigFieldId::Options(AnalysisOptionsFieldId::Lint),
        key: "lint",
        path: "lint",
        policy: AnalysisConfigPolicyId::Lint,
        value_kind: AnalysisConfigValueKind::Object(AnalysisConfigObjectId::Lint),
        nullable: true,
        required: false,
        default: AnalysisConfigDefault::None,
        description: "Diagnostic rule profile and per-rule overrides.",
        runtime_constraints: &[],
    },
    AnalysisConfigFieldDescriptor {
        id: AnalysisConfigFieldId::Lint(LintOptionsFieldId::Profile),
        key: "profile",
        path: "lint.profile",
        policy: AnalysisConfigPolicyId::Lint,
        value_kind: AnalysisConfigValueKind::String {
            enum_source: Some(AnalysisConfigEnumSource::Profiles),
            pattern: None,
        },
        nullable: true,
        required: false,
        default: AnalysisConfigDefault::RuleProfile(AnalysisRuleProfile::Core),
        description: "Base lint profile. Recommended and strict may enable additional governed authoring rules.",
        runtime_constraints: &[],
    },
    AnalysisConfigFieldDescriptor {
        id: AnalysisConfigFieldId::Lint(LintOptionsFieldId::EnableRules),
        key: "enable_rules",
        path: "lint.enable_rules",
        policy: AnalysisConfigPolicyId::Lint,
        value_kind: AnalysisConfigValueKind::Array(AnalysisConfigArrayItem::RuleId),
        nullable: false,
        required: false,
        default: AnalysisConfigDefault::EmptyArray,
        description: "Configurable rule ids to enable explicitly.",
        runtime_constraints: &[],
    },
    AnalysisConfigFieldDescriptor {
        id: AnalysisConfigFieldId::Lint(LintOptionsFieldId::DisableRules),
        key: "disable_rules",
        path: "lint.disable_rules",
        policy: AnalysisConfigPolicyId::Lint,
        value_kind: AnalysisConfigValueKind::Array(AnalysisConfigArrayItem::RuleId),
        nullable: false,
        required: false,
        default: AnalysisConfigDefault::EmptyArray,
        description: "Configurable rule ids to disable explicitly.",
        runtime_constraints: &[],
    },
    AnalysisConfigFieldDescriptor {
        id: AnalysisConfigFieldId::Lint(LintOptionsFieldId::RuleSeverities),
        key: "rule_severities",
        path: "lint.rule_severities",
        policy: AnalysisConfigPolicyId::Lint,
        value_kind: AnalysisConfigValueKind::Array(AnalysisConfigArrayItem::RuleSeverityOverride),
        nullable: false,
        required: false,
        default: AnalysisConfigDefault::EmptyArray,
        description: "Per-rule diagnostic severity overrides.",
        runtime_constraints: &[],
    },
    AnalysisConfigFieldDescriptor {
        id: AnalysisConfigFieldId::RuleSeverityOverride(RuleSeverityOverrideFieldId::RuleId),
        key: "rule_id",
        path: "lint.rule_severities[].rule_id",
        policy: AnalysisConfigPolicyId::Lint,
        value_kind: AnalysisConfigValueKind::String {
            enum_source: Some(AnalysisConfigEnumSource::RuleIds),
            pattern: None,
        },
        nullable: false,
        required: true,
        default: AnalysisConfigDefault::None,
        description: "Configurable analysis rule id.",
        runtime_constraints: &[],
    },
    AnalysisConfigFieldDescriptor {
        id: AnalysisConfigFieldId::RuleSeverityOverride(RuleSeverityOverrideFieldId::Severity),
        key: "severity",
        path: "lint.rule_severities[].severity",
        policy: AnalysisConfigPolicyId::Lint,
        value_kind: AnalysisConfigValueKind::String {
            enum_source: Some(AnalysisConfigEnumSource::Severities),
            pattern: None,
        },
        nullable: false,
        required: true,
        default: AnalysisConfigDefault::None,
        description: "Diagnostic severity for this rule override.",
        runtime_constraints: &[],
    },
];

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct AnalysisConfigHostDefaults {
    max_source_bytes: Option<usize>,
    max_document_diagrams: Option<usize>,
}

impl AnalysisConfigHostDefaults {
    /// Creates validated host defaults for the generated analysis schema.
    pub fn try_new(
        max_source_bytes: Option<usize>,
        max_document_diagrams: Option<usize>,
    ) -> Result<Self, AnalysisConfigHostDefaultsError> {
        let defaults = Self {
            max_source_bytes,
            max_document_diagrams,
        };
        for descriptor in resource_limit_descriptors() {
            let Some(value) = defaults.value_for(descriptor.stable_id) else {
                continue;
            };
            if value < descriptor.minimum_value || value > descriptor.maximum_value {
                return Err(AnalysisConfigHostDefaultsError {
                    limit_id: descriptor.stable_id,
                    value,
                    minimum: descriptor.minimum_value,
                    maximum: descriptor.maximum_value,
                });
            }
        }
        Ok(defaults)
    }

    fn value_for(self, limit_id: &str) -> Option<usize> {
        match limit_id {
            id if id == merman_core::resources::InputResourceLimitId::MaxSourceBytes.as_str() => {
                self.max_source_bytes
            }
            MAX_DOCUMENT_DIAGRAMS_RESOURCE_LIMIT_ID => self.max_document_diagrams,
            _ => None,
        }
    }
}

pub(crate) fn lint_profile_requirement() -> String {
    enum_requirement(AnalysisRuleProfile::ALL.map(AnalysisRuleProfile::as_str))
}

pub(crate) fn diagnostic_severity_requirement() -> String {
    enum_requirement(DiagnosticSeverity::ALL.map(DiagnosticSeverity::as_str))
}

fn enum_requirement<const N: usize>(values: [&'static str; N]) -> String {
    let allowed = match values.as_slice() {
        [] => String::new(),
        [only] => (*only).to_string(),
        [left, right] => format!("{left} or {right}"),
        many => format!(
            "{}, or {}",
            many[..many.len() - 1].join(", "),
            many[many.len() - 1]
        ),
    };
    format!("must be {allowed}")
}

/// Error returned when a host schema default violates the owning resource contract.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error(
    "analysis host default for {limit_id} must be between {minimum} and {maximum}, got {value}"
)]
pub struct AnalysisConfigHostDefaultsError {
    limit_id: &'static str,
    value: usize,
    minimum: usize,
    maximum: usize,
}

/// Stable constraints consumed by editor clients without interpreting JSON Schema internals.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AnalysisConfigClientConstraints {
    /// Version of this client-constraint DTO.
    pub version: u32,
    /// Leaf settings and their owner-projected normalization policy.
    pub settings: Vec<AnalysisConfigClientSetting>,
}

/// Named value catalog referenced by a client setting normalizer.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AnalysisConfigClientValueSet {
    Profiles,
    Severities,
    ConfigurableRuleIds,
}

/// Runtime validation projected in a host-neutral form for editor normalization.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum AnalysisConfigClientRuntimeConstraint {
    CanonicalCivilDate,
    RepresentableLocalMidnight {
        /// Setting whose normalized offset participates in the instant-range check.
        offset_setting_path: String,
    },
}

/// One field of an object-valued client setting item.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AnalysisConfigClientObjectField {
    /// Wire field name owned by the analysis configuration descriptor.
    pub name: String,
    /// Whether an object item must contain this field.
    pub required: bool,
    /// Value normalization projected from the field's owning descriptor.
    pub normalization: AnalysisConfigClientSettingNormalization,
}

/// Typed normalization policy projected from one analysis field descriptor.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum AnalysisConfigClientSettingNormalization {
    String {
        /// Optional lexical pattern owned by the field descriptor.
        #[serde(skip_serializing_if = "Option::is_none")]
        pattern: Option<String>,
        /// Optional owner-defined catalog of accepted string values.
        #[serde(skip_serializing_if = "Option::is_none")]
        values: Option<AnalysisConfigClientValueSet>,
    },
    Integer {
        /// Inclusive minimum accepted value.
        minimum: i64,
        /// Inclusive maximum accepted value.
        maximum: i64,
    },
    Object,
    RuleIdList,
    RuleSeverityOverrides {
        /// Item fields projected mechanically from the owned object descriptor.
        fields: Vec<AnalysisConfigClientObjectField>,
    },
}

/// One leaf setting owned by the analysis configuration contract.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AnalysisConfigClientSetting {
    /// Dot-separated path below the `merman.analysis` VS Code namespace.
    pub path: String,
    /// Runtime invalidation scope inherited from the setting's owning policy.
    pub change_scope: AnalysisConfigChangeScope,
    /// Server-owned validation steps projected for bootstrap-safe normalization.
    pub runtime_constraints: Vec<AnalysisConfigClientRuntimeConstraint>,
    /// Value shape and shallow normalization constraints.
    pub normalization: AnalysisConfigClientSettingNormalization,
}

/// Host-neutral client projection of the analysis configuration contract.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AnalysisConfigClientProjection {
    /// Accepted direct and wrapped configuration roots.
    pub accepted_roots: Vec<String>,
    /// Current lint profile identifiers.
    pub profiles: Vec<String>,
    /// Current diagnostic severity identifiers.
    pub severities: Vec<String>,
    /// Current rule identifiers accepted in lint configuration.
    pub configurable_rule_ids: Vec<String>,
    /// Typed constraints consumed by clients without evaluating JSON Schema.
    pub constraints: AnalysisConfigClientConstraints,
}

#[derive(Debug, Clone, PartialEq)]
pub struct AnalysisConfigSchemaProjection {
    pub accepted_roots: Vec<String>,
    pub profiles: Vec<String>,
    pub severities: Vec<String>,
    pub configurable_rule_ids: Vec<String>,
    /// Complete Draft 2020-12 schema for inspection and standards-based validation.
    pub schema: Value,
}

#[derive(Debug, Clone, Copy, Default)]
pub struct AnalysisConfigContract;

impl AnalysisConfigContract {
    /// Returns the current analysis configuration contract.
    pub const fn current() -> Self {
        Self
    }

    /// Decodes a supported direct or wrapped JSON value into analysis options.
    pub fn decode(self, value: &Value) -> Result<AnalysisOptions, AnalysisOptionsJsonError> {
        self.decode_json(value)?.to_analysis_options()
    }

    /// Decodes a supported direct or wrapped JSON value into its transport representation.
    pub fn decode_json(
        self,
        value: &Value,
    ) -> Result<AnalysisOptionsJson, AnalysisOptionsJsonError> {
        decode::decode_json(value)
    }

    /// Classifies the invalidation scope of an accepted configuration change.
    pub fn classify_change(
        self,
        current: &AnalysisOptions,
        next: &AnalysisOptions,
    ) -> AnalysisConfigChange {
        if current == next {
            return AnalysisConfigChange::Unchanged;
        }

        let mut change = AnalysisConfigChange::Unchanged;
        for policy in ANALYSIS_CONFIG_POLICIES {
            if !policy.changed(current, next) {
                continue;
            }
            if policy.change_scope == AnalysisConfigChangeScope::SnapshotAffecting {
                return AnalysisConfigChange::SnapshotAffecting;
            }
            change = policy.change_scope.change();
        }
        if change != AnalysisConfigChange::Unchanged {
            return change;
        }

        // Source descriptors and future host-only generation inputs are not JSON fields, but any
        // change to them must still invalidate the canonical snapshot.
        if current.snapshot_policy() != next.snapshot_policy() {
            AnalysisConfigChange::SnapshotAffecting
        } else {
            debug_assert_ne!(current.diagnostic_policy(), next.diagnostic_policy());
            AnalysisConfigChange::DiagnosticsOnly
        }
    }

    /// Projects the complete Draft 2020-12 schema with validated host defaults.
    pub fn json_schema(
        self,
        host_defaults: AnalysisConfigHostDefaults,
    ) -> AnalysisConfigSchemaProjection {
        schema::project(host_defaults)
    }

    /// Projects the host-neutral contract consumed by editor clients and manifest generators.
    pub fn client_projection(self) -> AnalysisConfigClientProjection {
        client_projection()
    }

    /// Returns whether this contract owns the named analysis resource limit.
    pub fn accepts_resource_limit(self, limit_id: &str) -> bool {
        resource_limit_descriptor(limit_id).is_some()
    }
}

const RESOURCE_LIMIT_MAXIMUM: usize = u32::MAX as usize;

#[derive(Debug, Clone, Copy)]
struct ResourceLimitSchemaDescriptor {
    stable_id: &'static str,
    minimum_value: usize,
    maximum_value: usize,
    description: &'static str,
}

fn resource_limit_descriptors() -> impl Iterator<Item = ResourceLimitSchemaDescriptor> {
    let source = merman_core::resources::InputResourceLimitId::MaxSourceBytes.descriptor();
    std::iter::once(ResourceLimitSchemaDescriptor {
        stable_id: source.stable_id,
        minimum_value: source.minimum_value,
        maximum_value: RESOURCE_LIMIT_MAXIMUM,
        description: source.description,
    })
    .chain(
        crate::ANALYSIS_RESOURCE_LIMIT_DESCRIPTORS
            .iter()
            .map(|descriptor| ResourceLimitSchemaDescriptor {
                stable_id: descriptor.stable_id,
                minimum_value: descriptor.minimum_value,
                maximum_value: RESOURCE_LIMIT_MAXIMUM,
                description: descriptor.description,
            }),
    )
}

fn resource_limit_descriptor(limit_id: &str) -> Option<ResourceLimitSchemaDescriptor> {
    resource_limit_descriptors().find(|descriptor| descriptor.stable_id == limit_id)
}

fn resource_limit_descriptor_or_error(
    limit_id: &str,
) -> Result<ResourceLimitSchemaDescriptor, AnalysisOptionsJsonError> {
    resource_limit_descriptor(limit_id).ok_or_else(|| {
        AnalysisOptionsJsonError::new(format!("unknown analysis resource limit id: {limit_id}"))
    })
}

pub(crate) fn validate_resource_limit_values(
    limits: &BTreeMap<String, usize>,
) -> Result<(), AnalysisOptionsJsonError> {
    for (limit_id, value) in limits {
        let descriptor = resource_limit_descriptor_or_error(limit_id)?;
        if *value < descriptor.minimum_value {
            return Err(AnalysisOptionsJsonError::new(format!(
                "resources.limits.{limit_id} must be at least {}",
                descriptor.minimum_value
            )));
        }
        if *value > descriptor.maximum_value {
            return Err(AnalysisOptionsJsonError::new(format!(
                "resources.limits.{limit_id} must be at most {}",
                descriptor.maximum_value
            )));
        }
    }
    Ok(())
}

fn client_projection() -> AnalysisConfigClientProjection {
    AnalysisConfigClientProjection {
        accepted_roots: AnalysisConfigRoot::ALL
            .into_iter()
            .map(AnalysisConfigRoot::as_str)
            .map(str::to_string)
            .collect(),
        profiles: AnalysisRuleProfile::ALL
            .into_iter()
            .map(AnalysisRuleProfile::as_str)
            .map(str::to_string)
            .collect(),
        severities: DiagnosticSeverity::ALL
            .into_iter()
            .map(DiagnosticSeverity::as_str)
            .map(str::to_string)
            .collect(),
        configurable_rule_ids: crate::configurable_rule_descriptors()
            .map(|descriptor| descriptor.id.to_string())
            .collect(),
        constraints: AnalysisConfigClientConstraints {
            version: ANALYSIS_CONFIG_CLIENT_CONSTRAINTS_VERSION,
            settings: client_settings(),
        },
    }
}

fn client_settings() -> Vec<AnalysisConfigClientSetting> {
    let mut settings = Vec::new();
    collect_client_settings(AnalysisConfigObjectId::Options, &mut settings);
    settings
}

fn collect_client_settings(
    parent: AnalysisConfigObjectId,
    settings: &mut Vec<AnalysisConfigClientSetting>,
) {
    for field in fields_for_object(parent) {
        match field.value_kind {
            AnalysisConfigValueKind::Object(child) => {
                collect_client_settings(child, settings);
            }
            AnalysisConfigValueKind::ResourceLimits => {
                settings.extend(resource_limit_descriptors().map(|descriptor| {
                    client_setting(
                        field,
                        format!("{}.{}", field.path, descriptor.stable_id),
                        AnalysisConfigClientSettingNormalization::Integer {
                            minimum: descriptor.minimum_value as i64,
                            maximum: descriptor.maximum_value as i64,
                        },
                    )
                }));
            }
            value_kind => settings.push(client_setting(
                field,
                field.path.to_string(),
                client_normalization(value_kind),
            )),
        }
    }
}

fn client_normalization(
    value_kind: AnalysisConfigValueKind,
) -> AnalysisConfigClientSettingNormalization {
    match value_kind {
        AnalysisConfigValueKind::String {
            enum_source,
            pattern,
        } => AnalysisConfigClientSettingNormalization::String {
            pattern: pattern.map(str::to_string),
            values: enum_source.map(client_value_set),
        },
        AnalysisConfigValueKind::Integer { minimum, maximum } => {
            AnalysisConfigClientSettingNormalization::Integer { minimum, maximum }
        }
        AnalysisConfigValueKind::JsonObject | AnalysisConfigValueKind::Object(_) => {
            AnalysisConfigClientSettingNormalization::Object
        }
        AnalysisConfigValueKind::Array(AnalysisConfigArrayItem::RuleId) => {
            AnalysisConfigClientSettingNormalization::RuleIdList
        }
        AnalysisConfigValueKind::Array(AnalysisConfigArrayItem::RuleSeverityOverride) => {
            AnalysisConfigClientSettingNormalization::RuleSeverityOverrides {
                fields: client_object_fields(AnalysisConfigObjectId::RuleSeverityOverride),
            }
        }
        AnalysisConfigValueKind::ResourceLimits => {
            unreachable!("resource limits must be expanded into leaf client settings")
        }
    }
}

fn client_object_fields(parent: AnalysisConfigObjectId) -> Vec<AnalysisConfigClientObjectField> {
    fields_for_object(parent)
        .map(|field| AnalysisConfigClientObjectField {
            name: field.key.to_string(),
            required: field.required,
            normalization: client_normalization(field.value_kind),
        })
        .collect()
}

fn client_setting(
    field: AnalysisConfigFieldDescriptor,
    path: String,
    normalization: AnalysisConfigClientSettingNormalization,
) -> AnalysisConfigClientSetting {
    AnalysisConfigClientSetting {
        path,
        change_scope: field.change_scope(),
        runtime_constraints: field
            .runtime_constraints
            .iter()
            .copied()
            .map(client_runtime_constraint)
            .collect(),
        normalization,
    }
}

fn client_runtime_constraint(
    constraint: AnalysisConfigRuntimeConstraint,
) -> AnalysisConfigClientRuntimeConstraint {
    match constraint {
        AnalysisConfigRuntimeConstraint::CanonicalCivilDate => {
            AnalysisConfigClientRuntimeConstraint::CanonicalCivilDate
        }
        AnalysisConfigRuntimeConstraint::RepresentableLocalMidnight {
            offset_setting_path,
        } => AnalysisConfigClientRuntimeConstraint::RepresentableLocalMidnight {
            offset_setting_path: offset_setting_path.to_string(),
        },
    }
}

const fn client_value_set(source: AnalysisConfigEnumSource) -> AnalysisConfigClientValueSet {
    match source {
        AnalysisConfigEnumSource::Profiles => AnalysisConfigClientValueSet::Profiles,
        AnalysisConfigEnumSource::RuleIds => AnalysisConfigClientValueSet::ConfigurableRuleIds,
        AnalysisConfigEnumSource::Severities => AnalysisConfigClientValueSet::Severities,
    }
}

fn object_descriptor(id: AnalysisConfigObjectId) -> AnalysisConfigObjectDescriptor {
    ANALYSIS_CONFIG_OBJECTS
        .iter()
        .copied()
        .find(|descriptor| descriptor.id == id)
        .expect("analysis config object must have one typed descriptor")
}

fn policy_descriptor(id: AnalysisConfigPolicyId) -> AnalysisConfigPolicyDescriptor {
    ANALYSIS_CONFIG_POLICIES
        .iter()
        .copied()
        .find(|descriptor| descriptor.id == id)
        .expect("analysis config policy must have one typed descriptor")
}

fn fields_for_object(
    parent: AnalysisConfigObjectId,
) -> impl Iterator<Item = AnalysisConfigFieldDescriptor> {
    ANALYSIS_CONFIG_FIELDS
        .iter()
        .copied()
        .filter(move |field| field.id.parent() == parent)
}

fn field_by_id(id: AnalysisConfigFieldId) -> AnalysisConfigFieldDescriptor {
    ANALYSIS_CONFIG_FIELDS
        .iter()
        .copied()
        .find(|field| field.id == id)
        .expect("analysis config field id must have one typed descriptor")
}

pub(crate) fn default_lint_profile() -> AnalysisRuleProfile {
    match field_by_id(AnalysisConfigFieldId::Lint(LintOptionsFieldId::Profile)).default {
        AnalysisConfigDefault::RuleProfile(profile) => profile,
        AnalysisConfigDefault::None | AnalysisConfigDefault::EmptyArray => {
            unreachable!("lint profile descriptor must declare a rule-profile default")
        }
    }
}
