use super::*;
use super::{decode::validate_config_field, schema::field_schema};
use crate::{AnalysisRuleProfile, DiagnosticSeverity, configurable_rule_descriptors};
use serde_json::{Value, json};
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
fn client_projection_exposes_each_manifest_setting_and_typed_constraint_once() {
    let projection = AnalysisConfigContract::current().client_projection();

    assert_eq!(projection.accepted_roots, ["direct", "merman", "analysis"]);
    assert_eq!(
        projection
            .constraints
            .settings
            .iter()
            .map(|setting| setting.path.as_str())
            .collect::<Vec<_>>(),
        [
            "fixed_today",
            "fixed_local_offset_minutes",
            "site_config",
            "resources.limits.max_source_bytes",
            "resources.limits.max_document_diagrams",
            "lint.profile",
            "lint.enable_rules",
            "lint.disable_rules",
            "lint.rule_severities",
        ]
    );
    assert_eq!(projection.profiles, ["core", "recommended", "strict"]);
    assert_eq!(projection.severities, ["error", "warning", "info", "hint"]);
    let settings = &projection.constraints.settings;
    assert!(
        settings[..5]
            .iter()
            .all(|setting| setting.change_scope == AnalysisConfigChangeScope::SnapshotAffecting)
    );
    assert!(
        settings[5..]
            .iter()
            .all(|setting| setting.change_scope == AnalysisConfigChangeScope::DiagnosticsOnly)
    );
    assert_eq!(
        settings[0].runtime_constraints,
        [
            AnalysisConfigClientRuntimeConstraint::CanonicalCivilDate,
            AnalysisConfigClientRuntimeConstraint::RepresentableLocalMidnight {
                offset_setting_path: "fixed_local_offset_minutes".to_string(),
            },
        ]
    );
    assert_eq!(
        settings[1].normalization,
        AnalysisConfigClientSettingNormalization::Integer {
            minimum: -1439,
            maximum: 1439,
        }
    );
    assert_eq!(
        settings[3].normalization,
        AnalysisConfigClientSettingNormalization::Integer {
            minimum: 1,
            maximum: u32::MAX as i64,
        }
    );
    assert_eq!(
        settings[6].normalization,
        AnalysisConfigClientSettingNormalization::RuleIdList
    );
    let AnalysisConfigClientSettingNormalization::RuleSeverityOverrides { fields } =
        &settings[8].normalization
    else {
        panic!("rule severities must retain their owned object fields");
    };
    assert_eq!(
        fields,
        &[
            AnalysisConfigClientObjectField {
                name: "rule_id".to_string(),
                required: true,
                normalization: AnalysisConfigClientSettingNormalization::String {
                    pattern: None,
                    values: Some(AnalysisConfigClientValueSet::ConfigurableRuleIds),
                },
            },
            AnalysisConfigClientObjectField {
                name: "severity".to_string(),
                required: true,
                normalization: AnalysisConfigClientSettingNormalization::String {
                    pattern: None,
                    values: Some(AnalysisConfigClientValueSet::Severities),
                },
            },
        ]
    );
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
                crate::AnalysisRuleConfig::default().with_profile(AnalysisRuleProfile::Recommended),
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

#[test]
fn host_defaults_reject_values_below_the_owner_minimum() {
    let error = AnalysisConfigHostDefaults::try_new(Some(0), None)
        .expect_err("zero source bytes must be rejected");

    assert_eq!(
        error.to_string(),
        format!(
            "analysis host default for max_source_bytes must be between 1 and {}, got 0",
            u32::MAX
        )
    );
}

#[cfg(target_pointer_width = "64")]
#[test]
fn host_defaults_reject_values_above_the_published_resource_maximum() {
    let value = u32::MAX as usize + 1;
    let error = AnalysisConfigHostDefaults::try_new(Some(value), None)
        .expect_err("out-of-schema source byte defaults must be rejected");

    assert_eq!(
        error.to_string(),
        format!(
            "analysis host default for max_source_bytes must be between 1 and {}, got {value}",
            u32::MAX
        )
    );
}
