use crate::{
    AnalysisOptions, AnalysisRuleProfile, DiagnosticSeverity,
    MAX_DOCUMENT_DIAGRAMS_RESOURCE_LIMIT_ID, configurable_rule_descriptors,
    options_json::{
        AnalysisOptionsJson, AnalysisOptionsJsonError, LintOptionsJson,
        LintRuleSeverityOverrideJson, ResourceOptionsJson,
    },
};
use serde::{Deserialize, Serialize, de::DeserializeOwned};
use serde_json::{Map, Value, json};

pub const FIXED_TODAY_SCHEMA_PATTERN: &str = concat!(
    r"^(?:\d{4}|\+(?:[1-9]\d{4,8}|1\d{9}|20\d{8}|21[0-3]\d{7}|214[0-6]\d{6}|",
    r"2147[0-3]\d{5}|21474[0-7]\d{4}|214748[0-2]\d{3}|2147483[0-5]\d{2}|",
    r"21474836[0-3]\d|214748364[0-7])|-(?:000[1-9]|00[1-9]\d|0[1-9]\d{2}|",
    r"[1-9]\d{3}|[1-9]\d{4,8}|1\d{9}|20\d{8}|21[0-3]\d{7}|214[0-6]\d{6}|",
    r"2147[0-3]\d{5}|21474[0-7]\d{4}|214748[0-2]\d{3}|2147483[0-5]\d{2}|",
    r"21474836[0-3]\d|214748364[0-8]))-\d{2}-\d{2}$",
);

const WRAPPER_KEYS: [&str; 2] = ["merman", "analysis"];

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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
struct AnalysisConfigFieldDescriptor {
    path: &'static str,
    root_key: &'static str,
    change_scope: AnalysisConfigChangeScope,
}

const ANALYSIS_CONFIG_FIELDS: [AnalysisConfigFieldDescriptor; 9] = [
    AnalysisConfigFieldDescriptor {
        path: "fixed_today",
        root_key: "fixed_today",
        change_scope: AnalysisConfigChangeScope::SnapshotAffecting,
    },
    AnalysisConfigFieldDescriptor {
        path: "fixed_local_offset_minutes",
        root_key: "fixed_local_offset_minutes",
        change_scope: AnalysisConfigChangeScope::SnapshotAffecting,
    },
    AnalysisConfigFieldDescriptor {
        path: "site_config",
        root_key: "site_config",
        change_scope: AnalysisConfigChangeScope::SnapshotAffecting,
    },
    AnalysisConfigFieldDescriptor {
        path: "resources.limits.max_source_bytes",
        root_key: "resources",
        change_scope: AnalysisConfigChangeScope::SnapshotAffecting,
    },
    AnalysisConfigFieldDescriptor {
        path: "resources.limits.max_document_diagrams",
        root_key: "resources",
        change_scope: AnalysisConfigChangeScope::SnapshotAffecting,
    },
    AnalysisConfigFieldDescriptor {
        path: "lint.profile",
        root_key: "lint",
        change_scope: AnalysisConfigChangeScope::DiagnosticsOnly,
    },
    AnalysisConfigFieldDescriptor {
        path: "lint.enable_rules",
        root_key: "lint",
        change_scope: AnalysisConfigChangeScope::DiagnosticsOnly,
    },
    AnalysisConfigFieldDescriptor {
        path: "lint.disable_rules",
        root_key: "lint",
        change_scope: AnalysisConfigChangeScope::DiagnosticsOnly,
    },
    AnalysisConfigFieldDescriptor {
        path: "lint.rule_severities",
        root_key: "lint",
        change_scope: AnalysisConfigChangeScope::DiagnosticsOnly,
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
        decode_analysis_options_object(options)
    }

    pub fn classify_change(
        self,
        current: &AnalysisOptions,
        next: &AnalysisOptions,
    ) -> AnalysisConfigChange {
        if current == next {
            AnalysisConfigChange::Unchanged
        } else if current.snapshot_policy() == next.snapshot_policy() {
            AnalysisConfigChange::DiagnosticsOnly
        } else {
            AnalysisConfigChange::SnapshotAffecting
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
        let resource_limits = resource_limit_properties(host_defaults);
        let analysis_options = analysis_options_schema(&profiles, resource_limits);
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
}

const RESOURCE_LIMIT_MAXIMUM: usize = u32::MAX as usize;

#[derive(Debug, Clone, Copy)]
struct ResourceLimitSchemaDescriptor {
    stable_id: &'static str,
    minimum_value: usize,
    maximum_value: usize,
    description: &'static str,
}

fn resource_limit_descriptor(limit_id: &str) -> Option<ResourceLimitSchemaDescriptor> {
    let source = merman_core::resources::InputResourceLimitId::MaxSourceBytes.descriptor();
    if source.stable_id == limit_id {
        return Some(ResourceLimitSchemaDescriptor {
            stable_id: source.stable_id,
            minimum_value: source.minimum_value,
            maximum_value: RESOURCE_LIMIT_MAXIMUM,
            description: source.description,
        });
    }
    crate::ANALYSIS_RESOURCE_LIMIT_DESCRIPTORS
        .iter()
        .find(|descriptor| descriptor.stable_id == limit_id)
        .map(|descriptor| ResourceLimitSchemaDescriptor {
            stable_id: descriptor.stable_id,
            minimum_value: descriptor.minimum_value,
            maximum_value: RESOURCE_LIMIT_MAXIMUM,
            description: descriptor.description,
        })
}

fn resource_limit_properties(host_defaults: AnalysisConfigHostDefaults) -> Value {
    let source = merman_core::resources::InputResourceLimitId::MaxSourceBytes.descriptor();
    let mut properties = Map::new();
    for descriptor in std::iter::once(ResourceLimitSchemaDescriptor {
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
    ) {
        let mut schema = json!({
            "type": "integer",
            "minimum": descriptor.minimum_value,
            "maximum": descriptor.maximum_value,
            "description": descriptor.description,
            "x-merman-change-scope": AnalysisConfigChangeScope::SnapshotAffecting.as_str(),
        });
        if let Some(default) = host_defaults.value_for(descriptor.stable_id) {
            assert!(
                default >= descriptor.minimum_value,
                "analysis host default for {} must satisfy its owner minimum",
                descriptor.stable_id
            );
            schema["default"] = json!(default);
        }
        properties.insert(descriptor.stable_id.to_string(), schema);
    }
    Value::Object(properties)
}

fn analysis_options_schema(profiles: &[String], resource_limits: Value) -> Value {
    json!({
        "type": "object",
        "additionalProperties": true,
        "not": { "required": ["parse"] },
        "x-merman-unknown-fields": AnalysisConfigCompatibility::ForwardCompatible.as_str(),
        "properties": {
            "fixed_today": {
                "type": ["string", "null"],
                "pattern": FIXED_TODAY_SCHEMA_PATTERN,
                "description": "Canonical fixed local civil date. Years 0000 through 9999 use YYYY-MM-DD; later years use +YEAR-MM-DD and negative years use -YEAR-MM-DD. Calendar validity and the representable local-midnight instant are validated when the configuration is applied.",
                "x-merman-change-scope": field_scope("fixed_today"),
                "x-merman-runtime-constraints": [
                    "canonical_civil_date",
                    "representable_local_midnight"
                ]
            },
            "fixed_local_offset_minutes": {
                "type": ["integer", "null"],
                "minimum": -1439,
                "maximum": 1439,
                "description": "Fixed local UTC offset in minutes.",
                "x-merman-change-scope": field_scope("fixed_local_offset_minutes")
            },
            "site_config": {
                "type": ["object", "null"],
                "additionalProperties": true,
                "description": "Mermaid site configuration forwarded to the shared parser/config layer.",
                "x-merman-change-scope": field_scope("site_config")
            },
            "resources": {
                "type": ["object", "null"],
                "additionalProperties": false,
                "x-merman-unknown-fields": AnalysisConfigCompatibility::Strict.as_str(),
                "properties": {
                    "limits": {
                        "type": "object",
                        "additionalProperties": false,
                        "x-merman-unknown-fields": AnalysisConfigCompatibility::Strict.as_str(),
                        "properties": resource_limits
                    }
                },
                "x-merman-change-scope": field_scope("resources.limits.max_source_bytes")
            },
            "lint": {
                "type": ["object", "null"],
                "additionalProperties": true,
                "x-merman-unknown-fields": AnalysisConfigCompatibility::ForwardCompatible.as_str(),
                "properties": {
                    "profile": {
                        "type": ["string", "null"],
                        "enum": [null, profiles[0], profiles[1], profiles[2]],
                        "default": AnalysisRuleProfile::Core.as_str(),
                        "description": "Base lint profile. Recommended and strict may enable additional governed authoring rules.",
                        "x-merman-change-scope": field_scope("lint.profile")
                    },
                    "enable_rules": {
                        "type": "array",
                        "items": { "$ref": "#/$defs/ruleId" },
                        "description": "Configurable rule ids to enable explicitly.",
                        "x-merman-change-scope": field_scope("lint.enable_rules")
                    },
                    "disable_rules": {
                        "type": "array",
                        "items": { "$ref": "#/$defs/ruleId" },
                        "description": "Configurable rule ids to disable explicitly.",
                        "x-merman-change-scope": field_scope("lint.disable_rules")
                    },
                    "rule_severities": {
                        "type": "array",
                        "items": {
                            "type": "object",
                            "required": ["rule_id", "severity"],
                            "additionalProperties": true,
                            "x-merman-unknown-fields": AnalysisConfigCompatibility::ForwardCompatible.as_str(),
                            "properties": {
                                "rule_id": { "$ref": "#/$defs/ruleId" },
                                "severity": { "$ref": "#/$defs/severity" }
                            }
                        },
                        "description": "Per-rule diagnostic severity overrides.",
                        "x-merman-change-scope": field_scope("lint.rule_severities")
                    }
                },
                "x-merman-change-scope": field_scope("lint.profile")
            }
        }
    })
}

fn root_schema(
    analysis_options: Value,
    configurable_rule_ids: &[String],
    severities: &[String],
) -> Value {
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
        "oneOf": [
            direct_root_schema(),
            wrapped_root_schema(AnalysisConfigRoot::Merman),
            wrapped_root_schema(AnalysisConfigRoot::Analysis)
        ]
    })
}

fn direct_root_schema() -> Value {
    json!({
        "allOf": [
            { "$ref": "#/$defs/analysisOptions" },
            {
                "not": {
                    "anyOf": [
                        { "required": ["merman"] },
                        { "required": ["analysis"] }
                    ]
                }
            }
        ]
    })
}

fn wrapped_root_schema(root: AnalysisConfigRoot) -> Value {
    let wrapper = root
        .wrapper_key()
        .expect("wrapped root must expose its wrapper key");
    let other_wrapper = match root {
        AnalysisConfigRoot::Merman => "analysis",
        AnalysisConfigRoot::Analysis => "merman",
        AnalysisConfigRoot::Direct => unreachable!("direct root is not wrapped"),
    };
    let mut forbidden = vec![
        json!({ "required": [other_wrapper] }),
        json!({ "required": ["parse"] }),
    ];
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

fn field_scope(path: &str) -> &'static str {
    ANALYSIS_CONFIG_FIELDS
        .iter()
        .find(|field| field.path == path)
        .map(|field| field.change_scope.as_str())
        .expect("analysis config schema path must have one typed field descriptor")
}

fn unique_root_keys() -> Vec<&'static str> {
    let mut keys = Vec::new();
    for field in ANALYSIS_CONFIG_FIELDS {
        if !keys.contains(&field.root_key) {
            keys.push(field.root_key);
        }
    }
    keys
}

fn reject_removed_parse(value: &Value) -> Result<(), AnalysisOptionsJsonError> {
    let Value::Object(map) = value else {
        return Ok(());
    };
    let removed = map.contains_key("parse")
        || WRAPPER_KEYS.iter().any(|key| {
            map.get(*key)
                .and_then(Value::as_object)
                .is_some_and(|options| options.contains_key("parse"))
        });
    if removed {
        return Err(AnalysisOptionsJsonError::new(
            "analysis option `parse` was removed; analysis always retains family parse failures",
        ));
    }
    Ok(())
}

fn select_analysis_options_root(value: &Value) -> Result<&Value, AnalysisOptionsJsonError> {
    let Value::Object(map) = value else {
        return Err(AnalysisOptionsJsonError::new(
            "analysis options JSON must be an object",
        ));
    };
    let merman = map.get("merman");
    let analysis = map.get("analysis");
    if merman.is_some() && analysis.is_some() {
        return Err(AnalysisOptionsJsonError::new(
            "options JSON must not contain both `merman` and `analysis` wrappers",
        ));
    }

    let wrapper = merman
        .map(|value| ("merman", value))
        .or_else(|| analysis.map(|value| ("analysis", value)));
    if let Some((key, wrapped)) = wrapper {
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
    ANALYSIS_CONFIG_FIELDS
        .iter()
        .any(|field| map.contains_key(field.root_key))
}

fn decode_analysis_options_object(
    value: &Value,
) -> Result<AnalysisOptionsJson, AnalysisOptionsJsonError> {
    let map = value
        .as_object()
        .ok_or_else(|| AnalysisOptionsJsonError::new("analysis options JSON must be an object"))?;
    Ok(AnalysisOptionsJson {
        fixed_today: decode_optional_field(map, "fixed_today")?,
        fixed_local_offset_minutes: decode_optional_integer_field(
            map,
            "fixed_local_offset_minutes",
            -1439,
            1439,
        )?
        .map(|value| value as i32),
        site_config: map
            .get("site_config")
            .filter(|value| !value.is_null())
            .cloned(),
        resources: map
            .get("resources")
            .filter(|value| !value.is_null())
            .map(decode_resource_options)
            .transpose()?,
        lint: decode_lint(map.get("lint"))?,
    })
}

pub(crate) fn decode_resource_options(
    value: &Value,
) -> Result<ResourceOptionsJson, AnalysisOptionsJsonError> {
    let map = value.as_object().ok_or_else(|| {
        AnalysisOptionsJsonError::new("invalid analysis options JSON: resources must be an object")
    })?;
    if let Some(unknown) = map.keys().find(|key| key.as_str() != "limits") {
        return Err(AnalysisOptionsJsonError::new(format!(
            "invalid analysis options JSON: unknown field `{unknown}` in resources"
        )));
    }
    let Some(limits) = map.get("limits") else {
        return Ok(ResourceOptionsJson::default());
    };
    let limits = limits.as_object().ok_or_else(|| {
        AnalysisOptionsJsonError::new(
            "invalid analysis options JSON: resources.limits must be an object",
        )
    })?;
    let mut decoded = std::collections::BTreeMap::new();
    for (limit_id, value) in limits {
        let Some(descriptor) = resource_limit_descriptor(limit_id) else {
            return Err(AnalysisOptionsJsonError::new(format!(
                "unknown analysis resource limit id: {limit_id}"
            )));
        };
        let integer = decode_json_integer(
            value,
            &format!("resources.limits.{limit_id}"),
            descriptor.minimum_value as i64,
            descriptor.maximum_value as i64,
        )?;
        decoded.insert(limit_id.clone(), integer as usize);
    }
    Ok(ResourceOptionsJson { limits: decoded })
}

fn decode_lint(value: Option<&Value>) -> Result<Option<LintOptionsJson>, AnalysisOptionsJsonError> {
    let Some(value) = value.filter(|value| !value.is_null()) else {
        return Ok(None);
    };
    let map = value.as_object().ok_or_else(|| {
        AnalysisOptionsJsonError::new("invalid analysis options JSON: lint must be an object")
    })?;
    Ok(Some(LintOptionsJson {
        profile: decode_optional_field(map, "profile")?,
        enable_rules: decode_default_field(map, "enable_rules")?,
        disable_rules: decode_default_field(map, "disable_rules")?,
        rule_severities: decode_rule_severities(map.get("rule_severities"))?,
    }))
}

fn decode_rule_severities(
    value: Option<&Value>,
) -> Result<Vec<LintRuleSeverityOverrideJson>, AnalysisOptionsJsonError> {
    let Some(value) = value else {
        return Ok(Vec::new());
    };
    let values = value.as_array().ok_or_else(|| {
        AnalysisOptionsJsonError::new(
            "invalid analysis options JSON: lint.rule_severities must be an array",
        )
    })?;
    values
        .iter()
        .enumerate()
        .map(|(index, value)| {
            let map = value.as_object().ok_or_else(|| {
                AnalysisOptionsJsonError::new(format!(
                    "invalid analysis options JSON: lint.rule_severities[{index}] must be an object"
                ))
            })?;
            let rule_id = decode_required_string(map, "rule_id", index)?;
            let severity = decode_required_string(map, "severity", index)?;
            Ok(LintRuleSeverityOverrideJson { rule_id, severity })
        })
        .collect()
}

fn decode_required_string(
    map: &Map<String, Value>,
    field: &str,
    index: usize,
) -> Result<String, AnalysisOptionsJsonError> {
    map.get(field)
        .and_then(Value::as_str)
        .map(str::to_string)
        .ok_or_else(|| {
            AnalysisOptionsJsonError::new(format!(
                "invalid analysis options JSON: lint.rule_severities[{index}].{field} must be a string"
            ))
        })
}

fn decode_optional_field<T>(
    map: &Map<String, Value>,
    field: &str,
) -> Result<Option<T>, AnalysisOptionsJsonError>
where
    T: DeserializeOwned,
{
    let Some(value) = map.get(field).filter(|value| !value.is_null()) else {
        return Ok(None);
    };
    serde_json::from_value(value.clone())
        .map(Some)
        .map_err(|error| {
            AnalysisOptionsJsonError::new(format!("invalid analysis options JSON: {error}"))
        })
}

fn decode_optional_integer_field(
    map: &Map<String, Value>,
    field: &str,
    minimum: i64,
    maximum: i64,
) -> Result<Option<i64>, AnalysisOptionsJsonError> {
    let Some(value) = map.get(field).filter(|value| !value.is_null()) else {
        return Ok(None);
    };
    decode_json_integer(value, field, minimum, maximum).map(Some)
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

fn decode_default_field<T>(
    map: &Map<String, Value>,
    field: &str,
) -> Result<T, AnalysisOptionsJsonError>
where
    T: DeserializeOwned + Default,
{
    let Some(value) = map.get(field) else {
        return Ok(T::default());
    };
    serde_json::from_value(value.clone()).map_err(|error| {
        AnalysisOptionsJsonError::new(format!("invalid analysis options JSON: {error}"))
    })
}
