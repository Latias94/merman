use super::*;
use serde_json::{Map, Value, json};

pub(super) fn project(host_defaults: AnalysisConfigHostDefaults) -> AnalysisConfigSchemaProjection {
    let client = client_projection();
    let analysis_options = analysis_options_schema(
        &client.profiles,
        &client.severities,
        &client.configurable_rule_ids,
        host_defaults,
    );
    let schema = root_schema(
        analysis_options,
        &client.configurable_rule_ids,
        &client.severities,
    );

    AnalysisConfigSchemaProjection {
        accepted_roots: client.accepted_roots,
        profiles: client.profiles,
        severities: client.severities,
        configurable_rule_ids: client.configurable_rule_ids,
        schema,
    }
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

pub(super) fn field_schema(
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
        schema["x-merman-runtime-constraints"] = json!(
            field
                .runtime_constraints
                .iter()
                .copied()
                .map(AnalysisConfigRuntimeConstraint::as_str)
                .collect::<Vec<_>>()
        );
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
