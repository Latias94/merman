use super::*;
use crate::{
    DiagnosticSeverity, configurable_rule_descriptors,
    options_json::{LintOptionsJson, LintRuleSeverityOverrideJson, ResourceOptionsJson},
};
use serde_json::{Map, Value};

pub(super) fn decode_json(value: &Value) -> Result<AnalysisOptionsJson, AnalysisOptionsJsonError> {
    reject_removed_parse(value)?;
    let options = select_analysis_options_root(value)?;
    validate_config_object(AnalysisConfigObjectId::Options, options)?;
    decode_analysis_options_object(options)
}

fn field_descriptor(
    parent: AnalysisConfigObjectId,
    key: &str,
) -> Option<AnalysisConfigFieldDescriptor> {
    fields_for_object(parent).find(|field| field.key == key)
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

pub(super) fn validate_config_field(
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
        AnalysisConfigEnumSource::Profiles => lint_profile_requirement(),
        AnalysisConfigEnumSource::RuleIds => {
            "must reference a configurable analysis rule id".to_string()
        }
        AnalysisConfigEnumSource::Severities => diagnostic_severity_requirement(),
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
        decode_resource_limit(limit_id, value, &format!("resources.limits.{limit_id}"))?;
    }
    Ok(())
}

fn decode_resource_limit(
    limit_id: &str,
    value: &Value,
    path: &str,
) -> Result<usize, AnalysisOptionsJsonError> {
    let descriptor = resource_limit_descriptor_or_error(limit_id)?;
    decode_json_integer(
        value,
        path,
        descriptor.minimum_value as i64,
        descriptor.maximum_value as i64,
    )
    .map(|integer| integer as usize)
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
                    let integer = decode_resource_limit(
                        limit_id,
                        value,
                        &format!("{}.{}", field.path, limit_id),
                    )?;
                    resources.limits.insert(limit_id.clone(), integer);
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
