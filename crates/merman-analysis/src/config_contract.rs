use crate::{
    AnalysisOptions, AnalysisRuleProfile, DiagnosticSeverity,
    MAX_DOCUMENT_DIAGRAMS_RESOURCE_LIMIT_ID, configurable_rule_descriptors,
    options_json::{
        AnalysisOptionsJson, AnalysisOptionsJsonError, LintOptionsJson,
        LintRuleSeverityOverrideJson, ResourceOptionsJson,
    },
};
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value, json};

pub const FIXED_TODAY_SCHEMA_PATTERN: &str = concat!(
    r"^(?:\d{4}|\+(?:[1-9]\d{4,8}|1\d{9}|20\d{8}|21[0-3]\d{7}|214[0-6]\d{6}|",
    r"2147[0-3]\d{5}|21474[0-7]\d{4}|214748[0-2]\d{3}|2147483[0-5]\d{2}|",
    r"21474836[0-3]\d|214748364[0-7])|-(?:000[1-9]|00[1-9]\d|0[1-9]\d{2}|",
    r"[1-9]\d{3}|[1-9]\d{4,8}|1\d{9}|20\d{8}|21[0-3]\d{7}|214[0-6]\d{6}|",
    r"2147[0-3]\d{5}|21474[0-7]\d{4}|214748[0-2]\d{3}|2147483[0-5]\d{2}|",
    r"21474836[0-3]\d|214748364[0-8]))-\d{2}-\d{2}$",
);

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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
enum AnalysisConfigChangeScope {
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
    runtime_constraints: &'static [&'static str],
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
        runtime_constraints: &["canonical_civil_date", "representable_local_midnight"],
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
    pub max_source_bytes: Option<usize>,
    pub max_document_diagrams: Option<usize>,
}

impl AnalysisConfigHostDefaults {
    fn value_for(self, limit_id: &str) -> Option<usize> {
        match limit_id {
            "max_source_bytes" => self.max_source_bytes,
            MAX_DOCUMENT_DIAGRAMS_RESOURCE_LIMIT_ID => self.max_document_diagrams,
            _ => None,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct AnalysisConfigSchemaProjection {
    pub accepted_roots: Vec<String>,
    pub profiles: Vec<String>,
    pub severities: Vec<String>,
    pub configurable_rule_ids: Vec<String>,
    pub schema: Value,
}

#[derive(Debug, Clone, Copy, Default)]
pub struct AnalysisConfigContract;

impl AnalysisConfigContract {
    pub const fn current() -> Self {
        Self
    }

    pub fn decode(self, value: &Value) -> Result<AnalysisOptions, AnalysisOptionsJsonError> {
        self.decode_json(value)?.to_analysis_options()
    }

    pub fn decode_json(
        self,
        value: &Value,
    ) -> Result<AnalysisOptionsJson, AnalysisOptionsJsonError> {
        reject_removed_parse(value)?;
        let options = select_analysis_options_root(value)?;
        validate_config_object(AnalysisConfigObjectId::Options, options)?;
        decode_analysis_options_object(options)
    }

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

    pub fn json_schema(
        self,
        host_defaults: AnalysisConfigHostDefaults,
    ) -> AnalysisConfigSchemaProjection {
        let profiles = AnalysisRuleProfile::ALL
            .into_iter()
            .map(AnalysisRuleProfile::as_str)
            .map(str::to_string)
            .collect::<Vec<_>>();
        let severities = DiagnosticSeverity::ALL
            .into_iter()
            .map(DiagnosticSeverity::as_str)
            .map(str::to_string)
            .collect::<Vec<_>>();
        let configurable_rule_ids = configurable_rule_descriptors()
            .map(|descriptor| descriptor.id.to_string())
            .collect::<Vec<_>>();
        let analysis_options = analysis_options_schema(
            &profiles,
            &severities,
            &configurable_rule_ids,
            host_defaults,
        );
        let schema = root_schema(analysis_options, &configurable_rule_ids, &severities);

        AnalysisConfigSchemaProjection {
            accepted_roots: AnalysisConfigRoot::ALL
                .into_iter()
                .map(AnalysisConfigRoot::as_str)
                .map(str::to_string)
                .collect(),
            profiles,
            severities,
            configurable_rule_ids,
            schema,
        }
    }

    pub(crate) fn resource_limit_minimum(self, limit_id: &str) -> Option<usize> {
        resource_limit_descriptor(limit_id).map(|descriptor| descriptor.minimum_value)
    }

    pub(crate) fn resource_limit_maximum(self, limit_id: &str) -> Option<usize> {
        resource_limit_descriptor(limit_id).map(|descriptor| descriptor.maximum_value)
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

fn resource_limit_properties(host_defaults: AnalysisConfigHostDefaults) -> Value {
    let change_scope = field_by_id(AnalysisConfigFieldId::Resources(
        ResourceOptionsFieldId::Limits,
    ))
    .change_scope()
    .as_str();
    let mut properties = Map::new();
    for descriptor in resource_limit_descriptors() {
        let mut schema = json!({
            "type": "integer",
            "minimum": descriptor.minimum_value,
            "maximum": descriptor.maximum_value,
            "description": descriptor.description,
            "x-merman-change-scope": change_scope,
        });
        if let Some(default) = host_defaults.value_for(descriptor.stable_id) {
            assert!(
                default >= descriptor.minimum_value,
                "analysis host default for {} must satisfy its owner minimum",
                descriptor.stable_id
            );
            assert!(
                default <= descriptor.maximum_value,
                "analysis host default for {} must satisfy its owner maximum",
                descriptor.stable_id
            );
            schema["default"] = json!(default);
        }
        properties.insert(descriptor.stable_id.to_string(), schema);
    }
    Value::Object(properties)
}

fn analysis_options_schema(
    profiles: &[String],
    severities: &[String],
    configurable_rule_ids: &[String],
    host_defaults: AnalysisConfigHostDefaults,
) -> Value {
    object_schema(
        AnalysisConfigObjectId::Options,
        profiles,
        severities,
        configurable_rule_ids,
        host_defaults,
    )
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

fn object_schema(
    id: AnalysisConfigObjectId,
    profiles: &[String],
    severities: &[String],
    configurable_rule_ids: &[String],
    host_defaults: AnalysisConfigHostDefaults,
) -> Value {
    let descriptor = object_descriptor(id);
    let mut properties = Map::new();
    let mut required = Vec::new();
    for field in fields_for_object(id) {
        properties.insert(
            field.key.to_string(),
            field_schema(
                field,
                profiles,
                severities,
                configurable_rule_ids,
                host_defaults,
            ),
        );
        if field.required {
            required.push(field.key);
        }
    }

    let mut schema = json!({
        "type": "object",
        "additionalProperties": descriptor.compatibility
            == AnalysisConfigCompatibility::ForwardCompatible,
        "x-merman-unknown-fields": descriptor.compatibility.as_str(),
        "properties": properties,
    });
    if !required.is_empty() {
        schema["required"] = json!(required);
    }
    if !descriptor.removed_keys.is_empty() {
        schema["not"] = json!({
            "anyOf": descriptor
                .removed_keys
                .iter()
                .map(|key| json!({ "required": [key] }))
                .collect::<Vec<_>>()
        });
    }
    schema
}

fn field_schema(
    field: AnalysisConfigFieldDescriptor,
    profiles: &[String],
    severities: &[String],
    configurable_rule_ids: &[String],
    host_defaults: AnalysisConfigHostDefaults,
) -> Value {
    let mut schema = match field.value_kind {
        AnalysisConfigValueKind::String {
            enum_source,
            pattern,
        } => {
            let mut schema = match enum_source {
                Some(AnalysisConfigEnumSource::RuleIds) => {
                    json!({ "$ref": "#/$defs/ruleId" })
                }
                Some(AnalysisConfigEnumSource::Severities) => {
                    json!({ "$ref": "#/$defs/severity" })
                }
                Some(AnalysisConfigEnumSource::Profiles) => json!({
                    "type": "string",
                    "enum": enum_values(
                        AnalysisConfigEnumSource::Profiles,
                        profiles,
                        severities,
                        configurable_rule_ids,
                    ),
                }),
                None => json!({ "type": "string" }),
            };
            if let Some(pattern) = pattern {
                schema["pattern"] = json!(pattern);
            }
            schema
        }
        AnalysisConfigValueKind::Integer { minimum, maximum } => json!({
            "type": "integer",
            "minimum": minimum,
            "maximum": maximum,
        }),
        AnalysisConfigValueKind::JsonObject => json!({
            "type": "object",
            "additionalProperties": true,
        }),
        AnalysisConfigValueKind::Object(id) => object_schema(
            id,
            profiles,
            severities,
            configurable_rule_ids,
            host_defaults,
        ),
        AnalysisConfigValueKind::Array(item) => json!({
            "type": "array",
            "items": array_item_schema(
                item,
                profiles,
                severities,
                configurable_rule_ids,
                host_defaults,
            ),
        }),
        AnalysisConfigValueKind::ResourceLimits => json!({
            "type": "object",
            "additionalProperties": false,
            "x-merman-unknown-fields": AnalysisConfigCompatibility::Strict.as_str(),
            "properties": resource_limit_properties(host_defaults),
        }),
    };

    if field.nullable {
        make_schema_nullable(&mut schema);
    }
    match field.default {
        AnalysisConfigDefault::None => {}
        AnalysisConfigDefault::RuleProfile(profile) => {
            schema["default"] = json!(profile.as_str());
        }
        AnalysisConfigDefault::EmptyArray => {
            schema["default"] = json!([]);
        }
    }
    schema["description"] = json!(field.description);
    schema["x-merman-change-scope"] = json!(field.change_scope().as_str());
    if !field.runtime_constraints.is_empty() {
        schema["x-merman-runtime-constraints"] = json!(field.runtime_constraints);
    }
    schema
}

fn array_item_schema(
    item: AnalysisConfigArrayItem,
    profiles: &[String],
    severities: &[String],
    configurable_rule_ids: &[String],
    host_defaults: AnalysisConfigHostDefaults,
) -> Value {
    match item {
        AnalysisConfigArrayItem::RuleId => json!({ "$ref": "#/$defs/ruleId" }),
        AnalysisConfigArrayItem::RuleSeverityOverride => object_schema(
            AnalysisConfigObjectId::RuleSeverityOverride,
            profiles,
            severities,
            configurable_rule_ids,
            host_defaults,
        ),
    }
}

fn enum_values<'a>(
    source: AnalysisConfigEnumSource,
    profiles: &'a [String],
    severities: &'a [String],
    configurable_rule_ids: &'a [String],
) -> &'a [String] {
    match source {
        AnalysisConfigEnumSource::Profiles => profiles,
        AnalysisConfigEnumSource::RuleIds => configurable_rule_ids,
        AnalysisConfigEnumSource::Severities => severities,
    }
}

fn make_schema_nullable(schema: &mut Value) {
    let type_value = schema
        .get_mut("type")
        .expect("nullable analysis config fields must expose a JSON Schema type");
    let base = type_value
        .as_str()
        .expect("analysis config field type must be a single string before null projection")
        .to_string();
    *type_value = json!([base, "null"]);
    if let Some(values) = schema.get_mut("enum").and_then(Value::as_array_mut) {
        values.insert(0, Value::Null);
    }
}

fn root_schema(
    analysis_options: Value,
    configurable_rule_ids: &[String],
    severities: &[String],
) -> Value {
    let mut roots = vec![direct_root_schema()];
    roots.extend(wrapped_config_roots().map(|(root, _)| wrapped_root_schema(root)));
    json!({
        "$schema": "https://json-schema.org/draft/2020-12/schema",
        "title": "Merman analysis options",
        "description": "Options accepted directly or under exactly one merman or analysis wrapper.",
        "$defs": {
            "ruleId": {
                "type": "string",
                "enum": configurable_rule_ids,
                "description": "A configurable Merman analysis rule id."
            },
            "severity": {
                "type": "string",
                "enum": severities,
                "description": "Diagnostic severity for an explicit rule override."
            },
            "analysisOptions": analysis_options
        },
        "oneOf": roots
    })
}

fn direct_root_schema() -> Value {
    let wrappers = wrapped_config_roots()
        .map(|(_, key)| json!({ "required": [key] }))
        .collect::<Vec<_>>();
    json!({
        "allOf": [
            { "$ref": "#/$defs/analysisOptions" },
            {
                "not": {
                    "anyOf": wrappers
                }
            }
        ]
    })
}

fn wrapped_root_schema(root: AnalysisConfigRoot) -> Value {
    let wrapper = root
        .wrapper_key()
        .expect("wrapped root must expose its wrapper key");
    let mut forbidden = wrapped_config_roots()
        .filter(|(other, _)| *other != root)
        .map(|(_, key)| json!({ "required": [key] }))
        .collect::<Vec<_>>();
    for removed in object_descriptor(AnalysisConfigObjectId::Options).removed_keys {
        forbidden.push(json!({ "required": [removed] }));
    }
    for root_key in unique_root_keys() {
        forbidden.push(json!({ "required": [root_key] }));
    }
    json!({
        "type": "object",
        "required": [wrapper],
        "additionalProperties": true,
        "properties": {
            (wrapper): { "$ref": "#/$defs/analysisOptions" }
        },
        "not": { "anyOf": forbidden }
    })
}

fn unique_root_keys() -> Vec<&'static str> {
    let mut keys = Vec::new();
    for field in fields_for_object(AnalysisConfigObjectId::Options) {
        if !keys.contains(&field.key) {
            keys.push(field.key);
        }
    }
    keys
}

fn field_descriptor(
    parent: AnalysisConfigObjectId,
    key: &str,
) -> Option<AnalysisConfigFieldDescriptor> {
    fields_for_object(parent).find(|field| field.key == key)
}

fn fields_for_object(
    parent: AnalysisConfigObjectId,
) -> impl Iterator<Item = AnalysisConfigFieldDescriptor> {
    ANALYSIS_CONFIG_FIELDS
        .iter()
        .copied()
        .filter(move |field| field.id.parent() == parent)
}

fn validate_config_object(
    id: AnalysisConfigObjectId,
    value: &Value,
) -> Result<(), AnalysisOptionsJsonError> {
    let descriptor = object_descriptor(id);
    let map = value.as_object().ok_or_else(|| {
        AnalysisOptionsJsonError::new(format!(
            "invalid analysis options JSON: {} must be an object",
            descriptor.path
        ))
    })?;

    for removed in descriptor.removed_keys {
        if map.contains_key(*removed) {
            return Err(AnalysisOptionsJsonError::new(format!(
                "analysis option `{removed}` was removed; analysis always retains family parse failures"
            )));
        }
    }

    for field in fields_for_object(id).filter(|field| field.required) {
        if !map.contains_key(field.key) {
            return Err(AnalysisOptionsJsonError::new(format!(
                "invalid analysis options JSON: {}.{} is required",
                descriptor.path, field.key
            )));
        }
    }

    for (key, value) in map {
        let Some(field) = field_descriptor(id, key) else {
            if descriptor.compatibility == AnalysisConfigCompatibility::Strict {
                return Err(AnalysisOptionsJsonError::new(format!(
                    "invalid analysis options JSON: unknown field `{key}` in {}",
                    descriptor.path
                )));
            }
            continue;
        };
        validate_config_field(field, value)?;
    }
    Ok(())
}

fn validate_config_field(
    field: AnalysisConfigFieldDescriptor,
    value: &Value,
) -> Result<(), AnalysisOptionsJsonError> {
    if value.is_null() {
        return if field.nullable {
            Ok(())
        } else {
            Err(AnalysisOptionsJsonError::new(format!(
                "invalid analysis options JSON: {} must not be null",
                field.path
            )))
        };
    }

    match field.value_kind {
        AnalysisConfigValueKind::String {
            enum_source,
            pattern: _,
        } => {
            let string = value.as_str().ok_or_else(|| {
                AnalysisOptionsJsonError::new(format!(
                    "invalid analysis options JSON: {} must be a string",
                    field.path
                ))
            })?;
            if let Some(source) = enum_source {
                validate_enum_value(source, string, field.path)?;
            }
            Ok(())
        }
        AnalysisConfigValueKind::Integer { minimum, maximum } => {
            decode_json_integer(value, field.path, minimum, maximum).map(|_| ())
        }
        AnalysisConfigValueKind::JsonObject => value.as_object().map(|_| ()).ok_or_else(|| {
            AnalysisOptionsJsonError::new(format!(
                "invalid analysis options JSON: {} must be an object",
                field.path
            ))
        }),
        AnalysisConfigValueKind::Object(id) => validate_config_object(id, value),
        AnalysisConfigValueKind::Array(item) => {
            let values = value.as_array().ok_or_else(|| {
                AnalysisOptionsJsonError::new(format!(
                    "invalid analysis options JSON: {} must be an array",
                    field.path
                ))
            })?;
            for (index, value) in values.iter().enumerate() {
                match item {
                    AnalysisConfigArrayItem::RuleId => {
                        let rule_id = value.as_str().ok_or_else(|| {
                            AnalysisOptionsJsonError::new(format!(
                                "invalid analysis options JSON: {}[{index}] must be a string",
                                field.path
                            ))
                        })?;
                        validate_enum_value(
                            AnalysisConfigEnumSource::RuleIds,
                            rule_id,
                            field.path,
                        )?;
                    }
                    AnalysisConfigArrayItem::RuleSeverityOverride => {
                        validate_config_object(
                            AnalysisConfigObjectId::RuleSeverityOverride,
                            value,
                        )?;
                    }
                }
            }
            Ok(())
        }
        AnalysisConfigValueKind::ResourceLimits => validate_resource_limits(value),
    }
}

fn validate_enum_value(
    source: AnalysisConfigEnumSource,
    value: &str,
    path: &str,
) -> Result<(), AnalysisOptionsJsonError> {
    let valid = match source {
        AnalysisConfigEnumSource::Profiles => AnalysisRuleProfile::from_config_str(value).is_some(),
        AnalysisConfigEnumSource::RuleIds => {
            configurable_rule_descriptors().any(|descriptor| descriptor.id == value)
        }
        AnalysisConfigEnumSource::Severities => {
            DiagnosticSeverity::from_config_str(value).is_some()
        }
    };
    if valid {
        return Ok(());
    }
    let requirement = match source {
        AnalysisConfigEnumSource::Profiles => "must be core, recommended, or strict",
        AnalysisConfigEnumSource::RuleIds => "must reference a configurable analysis rule id",
        AnalysisConfigEnumSource::Severities => "must be error, warning, info, or hint",
    };
    Err(AnalysisOptionsJsonError::new(format!(
        "{path} entry `{value}` {requirement}"
    )))
}

fn validate_resource_limits(value: &Value) -> Result<(), AnalysisOptionsJsonError> {
    let limits = value.as_object().ok_or_else(|| {
        AnalysisOptionsJsonError::new(
            "invalid analysis options JSON: resources.limits must be an object",
        )
    })?;
    for (limit_id, value) in limits {
        let Some(descriptor) = resource_limit_descriptor(limit_id) else {
            return Err(AnalysisOptionsJsonError::new(format!(
                "unknown analysis resource limit id: {limit_id}"
            )));
        };
        decode_json_integer(
            value,
            &format!("resources.limits.{limit_id}"),
            descriptor.minimum_value as i64,
            descriptor.maximum_value as i64,
        )?;
    }
    Ok(())
}

fn reject_removed_parse(value: &Value) -> Result<(), AnalysisOptionsJsonError> {
    let Value::Object(map) = value else {
        return Ok(());
    };
    let descriptor = object_descriptor(AnalysisConfigObjectId::Options);
    for removed in descriptor.removed_keys {
        let present = map.contains_key(*removed)
            || wrapped_config_roots().any(|(_, key)| {
                map.get(key)
                    .and_then(Value::as_object)
                    .is_some_and(|options| options.contains_key(*removed))
            });
        if present {
            return Err(AnalysisOptionsJsonError::new(format!(
                "analysis option `{removed}` was removed; analysis always retains family parse failures"
            )));
        }
    }
    Ok(())
}

fn select_analysis_options_root(value: &Value) -> Result<&Value, AnalysisOptionsJsonError> {
    let Value::Object(map) = value else {
        return Err(AnalysisOptionsJsonError::new(
            "analysis options JSON must be an object",
        ));
    };
    let mut selected_wrapper = None;
    for (_, key) in wrapped_config_roots() {
        let Some(wrapped) = map.get(key) else {
            continue;
        };
        if selected_wrapper.is_some() {
            return Err(AnalysisOptionsJsonError::new(
                "options JSON must not contain both `merman` and `analysis` wrappers",
            ));
        }
        selected_wrapper = Some((key, wrapped));
    }

    if let Some((key, wrapped)) = selected_wrapper {
        if root_option_key_present(map) {
            return Err(AnalysisOptionsJsonError::new(
                "options JSON must not mix top-level analysis options with `analysis` or `merman` wrappers",
            ));
        }
        if !wrapped.is_object() {
            return Err(AnalysisOptionsJsonError::new(format!(
                "options JSON wrapper `{key}` must contain an object"
            )));
        }
        return Ok(wrapped);
    }
    Ok(value)
}

fn root_option_key_present(map: &Map<String, Value>) -> bool {
    fields_for_object(AnalysisConfigObjectId::Options).any(|field| map.contains_key(field.key))
}

fn decode_analysis_options_object(
    value: &Value,
) -> Result<AnalysisOptionsJson, AnalysisOptionsJsonError> {
    let map = value
        .as_object()
        .ok_or_else(|| AnalysisOptionsJsonError::new("analysis options JSON must be an object"))?;
    let mut decoded = AnalysisOptionsJson::default();
    for field in fields_for_object(AnalysisConfigObjectId::Options) {
        let Some(value) = map.get(field.key).filter(|value| !value.is_null()) else {
            continue;
        };
        let AnalysisConfigFieldId::Options(field_id) = field.id else {
            unreachable!("options object contained a non-options field descriptor")
        };
        match field_id {
            AnalysisOptionsFieldId::FixedToday => {
                decoded.fixed_today = Some(decoded_string(value));
            }
            AnalysisOptionsFieldId::FixedLocalOffsetMinutes => {
                decoded.fixed_local_offset_minutes = Some(decoded_integer(field, value)? as i32);
            }
            AnalysisOptionsFieldId::SiteConfig => decoded.site_config = Some(value.clone()),
            AnalysisOptionsFieldId::Resources => {
                decoded.resources = Some(decode_resource_options(value)?);
            }
            AnalysisOptionsFieldId::Lint => decoded.lint = Some(decode_lint(value)?),
        }
    }
    Ok(decoded)
}

pub(crate) fn decode_resource_options(
    value: &Value,
) -> Result<ResourceOptionsJson, AnalysisOptionsJsonError> {
    validate_config_object(AnalysisConfigObjectId::Resources, value)?;
    let map = value
        .as_object()
        .expect("validated resources configuration must be an object");
    let mut resources = ResourceOptionsJson::default();
    for field in fields_for_object(AnalysisConfigObjectId::Resources) {
        let Some(value) = map.get(field.key) else {
            continue;
        };
        let AnalysisConfigFieldId::Resources(field_id) = field.id else {
            unreachable!("resources object contained a non-resource field descriptor")
        };
        match field_id {
            ResourceOptionsFieldId::Limits => {
                let limits = value
                    .as_object()
                    .expect("validated resource limits must be an object");
                for (limit_id, value) in limits {
                    let descriptor = resource_limit_descriptor(limit_id)
                        .expect("validated resource limit ids must have owner descriptors");
                    let integer = decode_json_integer(
                        value,
                        &format!("{}.{}", field.path, limit_id),
                        descriptor.minimum_value as i64,
                        descriptor.maximum_value as i64,
                    )?;
                    resources.limits.insert(limit_id.clone(), integer as usize);
                }
            }
        }
    }
    Ok(resources)
}

fn decode_lint(value: &Value) -> Result<LintOptionsJson, AnalysisOptionsJsonError> {
    let map = value
        .as_object()
        .expect("validated lint configuration must be an object");
    let mut lint = LintOptionsJson::default();
    for field in fields_for_object(AnalysisConfigObjectId::Lint) {
        let Some(value) = map.get(field.key).filter(|value| !value.is_null()) else {
            continue;
        };
        let AnalysisConfigFieldId::Lint(field_id) = field.id else {
            unreachable!("lint object contained a non-lint field descriptor")
        };
        match field_id {
            LintOptionsFieldId::Profile => lint.profile = Some(decoded_string(value)),
            LintOptionsFieldId::EnableRules => lint.enable_rules = decoded_string_array(value),
            LintOptionsFieldId::DisableRules => lint.disable_rules = decoded_string_array(value),
            LintOptionsFieldId::RuleSeverities => {
                lint.rule_severities = decode_rule_severities(value)?
            }
        }
    }
    Ok(lint)
}

fn decode_rule_severities(
    value: &Value,
) -> Result<Vec<LintRuleSeverityOverrideJson>, AnalysisOptionsJsonError> {
    let values = value
        .as_array()
        .expect("validated lint rule severities must be an array");
    Ok(values
        .iter()
        .map(|value| {
            let map = value
                .as_object()
                .expect("validated rule severity override must be an object");
            let mut override_json = LintRuleSeverityOverrideJson::default();
            for field in fields_for_object(AnalysisConfigObjectId::RuleSeverityOverride) {
                let value = map
                    .get(field.key)
                    .expect("validated rule severity field must be present");
                let AnalysisConfigFieldId::RuleSeverityOverride(field_id) = field.id else {
                    unreachable!("rule severity object contained a non-override field descriptor")
                };
                match field_id {
                    RuleSeverityOverrideFieldId::RuleId => {
                        override_json.rule_id = decoded_string(value)
                    }
                    RuleSeverityOverrideFieldId::Severity => {
                        override_json.severity = decoded_string(value)
                    }
                }
            }
            override_json
        })
        .collect())
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

fn decoded_string(value: &Value) -> String {
    value
        .as_str()
        .expect("validated analysis config string must be a string")
        .to_string()
}

fn decoded_integer(
    field: AnalysisConfigFieldDescriptor,
    value: &Value,
) -> Result<i64, AnalysisOptionsJsonError> {
    let AnalysisConfigValueKind::Integer { minimum, maximum } = field.value_kind else {
        unreachable!("integer decoder must consume an integer field descriptor")
    };
    decode_json_integer(value, field.path, minimum, maximum)
}

fn decoded_string_array(value: &Value) -> Vec<String> {
    value
        .as_array()
        .expect("validated analysis config array must be an array")
        .iter()
        .map(decoded_string)
        .collect()
}

fn decode_json_integer(
    value: &Value,
    field: &str,
    minimum: i64,
    maximum: i64,
) -> Result<i64, AnalysisOptionsJsonError> {
    let integer = value
        .as_i64()
        .or_else(|| value.as_u64().and_then(|value| i64::try_from(value).ok()))
        .or_else(|| {
            let value = value.as_f64()?;
            (value.is_finite()
                && value.fract() == 0.0
                && value >= minimum as f64
                && value <= maximum as f64)
                .then_some(value as i64)
        })
        .ok_or_else(|| {
            AnalysisOptionsJsonError::new(format!(
                "invalid analysis options JSON: {field} must be an integer between {minimum} and {maximum}"
            ))
        })?;
    if !(minimum..=maximum).contains(&integer) {
        return Err(AnalysisOptionsJsonError::new(format!(
            "invalid analysis options JSON: {field} must be an integer between {minimum} and {maximum}"
        )));
    }
    Ok(integer)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeSet;

    #[test]
    fn typed_descriptor_tree_projects_every_runtime_field_once() {
        let projection =
            AnalysisConfigContract::current().json_schema(AnalysisConfigHostDefaults::default());
        let analysis = &projection.schema["$defs"]["analysisOptions"];

        for object in ANALYSIS_CONFIG_OBJECTS {
            let schema = match object.id {
                AnalysisConfigObjectId::Options => analysis,
                AnalysisConfigObjectId::Resources => &analysis["properties"]["resources"],
                AnalysisConfigObjectId::Lint => &analysis["properties"]["lint"],
                AnalysisConfigObjectId::RuleSeverityOverride => {
                    &analysis["properties"]["lint"]["properties"]["rule_severities"]["items"]
                }
            };
            let projected = schema["properties"]
                .as_object()
                .expect("typed config objects must project properties")
                .keys()
                .map(String::as_str)
                .collect::<BTreeSet<_>>();
            let declared = ANALYSIS_CONFIG_FIELDS
                .iter()
                .filter(|field| field.id.parent() == object.id)
                .map(|field| field.key)
                .collect::<BTreeSet<_>>();
            assert_eq!(projected, declared, "field drift for {:?}", object.id);
            assert_eq!(
                schema["additionalProperties"],
                json!(object.compatibility == AnalysisConfigCompatibility::ForwardCompatible),
                "compatibility drift for {:?}",
                object.id
            );

            let required = schema
                .get("required")
                .and_then(Value::as_array)
                .into_iter()
                .flatten()
                .filter_map(Value::as_str)
                .collect::<BTreeSet<_>>();
            let declared_required = ANALYSIS_CONFIG_FIELDS
                .iter()
                .filter(|field| field.id.parent() == object.id && field.required)
                .map(|field| field.key)
                .collect::<BTreeSet<_>>();
            assert_eq!(required, declared_required);
        }

        let paths = ANALYSIS_CONFIG_FIELDS
            .iter()
            .map(|field| field.path)
            .collect::<BTreeSet<_>>();
        assert_eq!(paths.len(), ANALYSIS_CONFIG_FIELDS.len());
        let ids = ANALYSIS_CONFIG_FIELDS
            .iter()
            .map(|field| field.id)
            .collect::<BTreeSet<_>>();
        assert_eq!(ids.len(), ANALYSIS_CONFIG_FIELDS.len());
    }

    #[test]
    fn descriptor_bounds_nullability_and_scope_drive_both_projections() {
        let profiles = AnalysisRuleProfile::ALL
            .into_iter()
            .map(AnalysisRuleProfile::as_str)
            .map(str::to_string)
            .collect::<Vec<_>>();
        let severities = DiagnosticSeverity::ALL
            .into_iter()
            .map(DiagnosticSeverity::as_str)
            .map(str::to_string)
            .collect::<Vec<_>>();
        let rule_ids = configurable_rule_descriptors()
            .map(|descriptor| descriptor.id.to_string())
            .collect::<Vec<_>>();

        for field in ANALYSIS_CONFIG_FIELDS {
            let schema = field_schema(
                field,
                &profiles,
                &severities,
                &rule_ids,
                AnalysisConfigHostDefaults::default(),
            );
            assert_eq!(
                schema["x-merman-change-scope"],
                json!(field.change_scope().as_str())
            );
            assert_eq!(schema["description"], json!(field.description));

            let schema_accepts_null = schema["type"]
                .as_array()
                .is_some_and(|types| types.iter().any(|value| value.as_str() == Some("null")));
            assert_eq!(schema_accepts_null, field.nullable);
            assert_eq!(
                validate_config_field(field, &Value::Null).is_ok(),
                field.nullable
            );

            if let AnalysisConfigValueKind::Integer { minimum, maximum } = field.value_kind {
                assert_eq!(schema["minimum"], json!(minimum));
                assert_eq!(schema["maximum"], json!(maximum));
                assert!(validate_config_field(field, &json!(minimum)).is_ok());
                assert!(validate_config_field(field, &json!(maximum)).is_ok());
                assert!(validate_config_field(field, &json!(minimum - 1)).is_err());
                assert!(validate_config_field(field, &json!(maximum + 1)).is_err());
            }
        }
    }

    #[test]
    fn policy_descriptors_drive_runtime_classification_and_field_schema_scope() {
        let current = AnalysisOptions::default();
        let policy_ids = ANALYSIS_CONFIG_POLICIES
            .iter()
            .map(|policy| policy.id)
            .collect::<BTreeSet<_>>();
        assert_eq!(policy_ids.len(), ANALYSIS_CONFIG_POLICIES.len());

        for policy in ANALYSIS_CONFIG_POLICIES {
            let next = match policy.id {
                AnalysisConfigPolicyId::FixedToday => {
                    AnalysisOptions::default().with_fixed_today(Some("2026-08-11".parse().unwrap()))
                }
                AnalysisConfigPolicyId::FixedLocalOffsetMinutes => AnalysisOptions::default()
                    .try_with_fixed_local_offset_minutes(60)
                    .unwrap(),
                AnalysisConfigPolicyId::SiteConfig => AnalysisOptions::default().with_site_config(
                    merman_core::MermaidConfig::from_value(json!({ "theme": "dark" })),
                ),
                AnalysisConfigPolicyId::Resources => {
                    AnalysisOptions::default().with_max_source_bytes(Some(1))
                }
                AnalysisConfigPolicyId::Lint => AnalysisOptions::default().with_rule_config(
                    crate::AnalysisRuleConfig::default()
                        .with_profile(AnalysisRuleProfile::Recommended),
                ),
            };
            assert!(policy.changed(&current, &next));
            assert_eq!(
                AnalysisConfigContract::current().classify_change(&current, &next),
                policy.change_scope.change(),
                "classification drifted for {:?}",
                policy.id
            );
        }

        for field in ANALYSIS_CONFIG_FIELDS {
            assert!(policy_ids.contains(&field.policy));
            assert_eq!(
                field.change_scope(),
                policy_descriptor(field.policy).change_scope
            );
        }
    }

    #[test]
    fn descriptor_defaults_match_runtime_defaults() {
        let contract = AnalysisConfigContract::current();
        let projection = contract.json_schema(AnalysisConfigHostDefaults::default());
        let decoded_json = contract.decode_json(&json!({ "lint": {} })).unwrap();
        let decoded = contract.decode(&json!({ "lint": {} })).unwrap();
        let lint = decoded_json.lint.expect("decoded lint options");

        assert_eq!(
            projection.schema["$defs"]["analysisOptions"]["properties"]["lint"]["properties"]["profile"]
                ["default"],
            json!(default_lint_profile().as_str())
        );
        assert_eq!(
            decoded.diagnostic_policy().rule_config.profile(),
            default_lint_profile()
        );
        for (field_id, values) in [
            (
                AnalysisConfigFieldId::Lint(LintOptionsFieldId::EnableRules),
                lint.enable_rules,
            ),
            (
                AnalysisConfigFieldId::Lint(LintOptionsFieldId::DisableRules),
                lint.disable_rules,
            ),
        ] {
            assert_eq!(
                field_by_id(field_id).default,
                AnalysisConfigDefault::EmptyArray
            );
            assert!(values.is_empty());
        }
        assert_eq!(
            field_by_id(AnalysisConfigFieldId::Lint(
                LintOptionsFieldId::RuleSeverities
            ))
            .default,
            AnalysisConfigDefault::EmptyArray
        );
        assert!(lint.rule_severities.is_empty());
    }

    #[cfg(target_pointer_width = "64")]
    #[test]
    #[should_panic(expected = "must satisfy its owner maximum")]
    fn host_defaults_cannot_exceed_the_published_resource_maximum() {
        let _ = AnalysisConfigContract::current().json_schema(AnalysisConfigHostDefaults {
            max_source_bytes: Some(u32::MAX as usize + 1),
            max_document_diagrams: None,
        });
    }
}
