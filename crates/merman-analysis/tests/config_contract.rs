use merman_analysis::{
    AnalysisConfigChange, AnalysisConfigChangeScope, AnalysisConfigCompatibility,
    AnalysisConfigContract, AnalysisConfigHostDefaults, AnalysisOptions, AnalysisRuleConfig,
    AnalysisRuleProfile, DiagnosticSeverity, configurable_rule_descriptors,
};
use serde_json::{Value, json};

const CONFIGURABLE_RULE_ID: &str = "merman.parse.no_diagram";

struct ConfigCase {
    name: &'static str,
    value: Value,
    accepted: bool,
}

#[test]
fn runtime_decoder_and_draft_2020_12_schema_share_the_acceptance_corpus() {
    let contract = AnalysisConfigContract::current();
    let projection = contract.json_schema(AnalysisConfigHostDefaults::default());
    let validator = jsonschema::options()
        .with_draft(jsonschema::Draft::Draft202012)
        .build(&projection.schema)
        .expect("analysis must publish a valid Draft 2020-12 schema");

    for case in config_cases() {
        let runtime_accepted = contract.decode(&case.value).is_ok();
        let schema_accepted = validator.is_valid(&case.value);
        assert_eq!(
            runtime_accepted, case.accepted,
            "runtime acceptance drifted for {}: {}",
            case.name, case.value
        );
        assert_eq!(
            schema_accepted, case.accepted,
            "schema acceptance drifted for {}: {}",
            case.name, case.value
        );
    }
}

#[test]
fn runtime_only_date_constraints_are_named_in_the_published_schema() {
    let contract = AnalysisConfigContract::current();
    let projection = contract.json_schema(AnalysisConfigHostDefaults::default());
    let validator = jsonschema::options()
        .with_draft(jsonschema::Draft::Draft202012)
        .build(&projection.schema)
        .unwrap();
    let fixed_today = &projection.schema["$defs"]["analysisOptions"]["properties"]["fixed_today"];

    assert_eq!(
        fixed_today["x-merman-runtime-constraints"],
        json!(["canonical_civil_date", "representable_local_midnight"])
    );

    for value in [
        json!({ "fixed_today": "2026-02-29" }),
        json!({
            "fixed_today": "-2147483648-01-01",
            "fixed_local_offset_minutes": 1439
        }),
    ] {
        assert!(
            validator.is_valid(&value),
            "the standard schema intentionally validates only the named lexical constraint"
        );
        assert!(
            contract.decode(&value).is_err(),
            "the runtime must enforce the named civil-date constraint"
        );
    }
}

#[test]
fn projected_metadata_comes_from_the_analysis_authorities_once() {
    let contract = AnalysisConfigContract::current();
    let projection = contract.json_schema(AnalysisConfigHostDefaults {
        max_source_bytes: Some(4 * 1024 * 1024),
        max_document_diagrams: Some(256),
    });

    assert_eq!(projection.accepted_roots, ["direct", "merman", "analysis"]);
    assert_eq!(projection.profiles, ["core", "recommended", "strict"]);
    assert_eq!(projection.severities, ["error", "warning", "info", "hint"]);

    let expected_rule_ids = configurable_rule_descriptors()
        .map(|descriptor| descriptor.id.to_string())
        .collect::<Vec<_>>();
    assert_eq!(projection.configurable_rule_ids, expected_rule_ids);
    let unique_rule_ids = projection
        .configurable_rule_ids
        .iter()
        .collect::<std::collections::BTreeSet<_>>();
    assert_eq!(
        unique_rule_ids.len(),
        projection.configurable_rule_ids.len()
    );
    assert_eq!(
        projection.schema["$defs"]["ruleId"]["enum"],
        json!(projection.configurable_rule_ids)
    );
    assert_eq!(
        projection.schema["$defs"]["severity"]["enum"],
        json!(projection.severities)
    );

    let options = &projection.schema["$defs"]["analysisOptions"];
    assert_eq!(
        options["properties"]["resources"]["properties"]["limits"]["properties"]["max_source_bytes"]
            ["default"],
        json!(4 * 1024 * 1024)
    );
    assert_eq!(
        options["properties"]["resources"]["properties"]["limits"]["properties"]["max_document_diagrams"]
            ["default"],
        json!(256)
    );
    assert_eq!(
        options["properties"]["lint"]["properties"]["profile"]["enum"],
        json!([null, "core", "recommended", "strict"])
    );
    assert!(
        options["properties"]["lint"]["properties"]["enable_rules"]
            .get("uniqueItems")
            .is_none()
    );
}

#[test]
fn typed_field_and_container_contracts_expose_change_and_compatibility_policy() {
    let contract = AnalysisConfigContract::current();

    for field in contract.fields() {
        let expected = if field.path.starts_with("lint.") {
            AnalysisConfigChangeScope::DiagnosticsOnly
        } else {
            AnalysisConfigChangeScope::SnapshotAffecting
        };
        assert_eq!(field.change_scope, expected, "{}", field.path);
    }

    let containers = contract
        .containers()
        .iter()
        .map(|container| (container.path, container.compatibility))
        .collect::<std::collections::BTreeMap<_, _>>();
    assert_eq!(
        containers[""],
        AnalysisConfigCompatibility::ForwardCompatible
    );
    assert_eq!(
        containers["lint"],
        AnalysisConfigCompatibility::ForwardCompatible
    );
    assert_eq!(containers["resources"], AnalysisConfigCompatibility::Strict);
    assert_eq!(
        containers["resources.limits"],
        AnalysisConfigCompatibility::Strict
    );
}

#[test]
fn analysis_owns_configuration_change_classification() {
    let contract = AnalysisConfigContract::current();
    let current = AnalysisOptions::default();
    let diagnostic_only = AnalysisOptions::default().with_rule_config(
        AnalysisRuleConfig::default().with_profile(AnalysisRuleProfile::Recommended),
    );
    let snapshot_affecting = [
        AnalysisOptions::default().with_max_source_bytes(Some(1)),
        AnalysisOptions::default().with_max_document_diagrams(Some(0)),
        AnalysisOptions::default().with_fixed_today(Some("2026-08-11".parse().unwrap())),
    ];

    assert_eq!(
        contract.classify_change(&current, &current),
        AnalysisConfigChange::Unchanged
    );
    assert_eq!(
        contract.classify_change(&current, &diagnostic_only),
        AnalysisConfigChange::DiagnosticsOnly
    );
    for next in &snapshot_affecting {
        assert_eq!(
            contract.classify_change(&current, next),
            AnalysisConfigChange::SnapshotAffecting
        );
    }
    assert_eq!(AnalysisRuleProfile::ALL.len(), 3);
    assert_eq!(DiagnosticSeverity::ALL.len(), 4);
}

fn config_cases() -> Vec<ConfigCase> {
    vec![
        accepted("empty direct root", json!({})),
        accepted("future direct field", json!({ "future_root": true })),
        accepted(
            "direct nullable fields",
            json!({
                "fixed_today": null,
                "fixed_local_offset_minutes": null,
                "site_config": null,
                "resources": null,
                "lint": null
            }),
        ),
        accepted(
            "direct fields",
            json!({
                "fixed_today": "2024-02-29",
                "fixed_local_offset_minutes": -1439,
                "site_config": { "theme": "dark" },
                "resources": {
                    "limits": {
                        "max_source_bytes": 1,
                        "max_document_diagrams": 0
                    }
                },
                "lint": {
                    "profile": "strict",
                    "enable_rules": [CONFIGURABLE_RULE_ID, CONFIGURABLE_RULE_ID],
                    "disable_rules": [],
                    "rule_severities": [{
                        "rule_id": CONFIGURABLE_RULE_ID,
                        "severity": "hint"
                    }],
                    "future_lint": true
                }
            }),
        ),
        accepted("empty merman wrapper", json!({ "merman": {} })),
        accepted(
            "future-only analysis wrapper",
            json!({ "analysis": { "future_option": true }, "future_root": true }),
        ),
        accepted(
            "forward-compatible severity override",
            json!({
                "analysis": {
                    "lint": {
                        "rule_severities": [{
                            "rule_id": CONFIGURABLE_RULE_ID,
                            "severity": "warning",
                            "future_override": true
                        }]
                    }
                }
            }),
        ),
        rejected("non-object root", json!("core")),
        rejected("non-object wrapper", json!({ "analysis": "core" })),
        rejected("both wrappers", json!({ "analysis": {}, "merman": {} })),
        rejected(
            "direct and wrapped fields",
            json!({ "lint": {}, "analysis": {} }),
        ),
        rejected(
            "direct field and non-object wrapper",
            json!({ "lint": {}, "analysis": null }),
        ),
        rejected("removed direct parse", json!({ "parse": {} })),
        rejected(
            "removed wrapped parse",
            json!({ "merman": { "parse": {} } }),
        ),
        rejected(
            "strict resource container",
            json!({ "resources": { "limits": {}, "future": true } }),
        ),
        rejected(
            "strict resource limits",
            json!({ "resources": { "limits": { "future_limit": 1 } } }),
        ),
        rejected(
            "source minimum",
            json!({ "resources": { "limits": { "max_source_bytes": 0 } } }),
        ),
        rejected(
            "profile case",
            json!({ "lint": { "profile": "Recommended" } }),
        ),
        rejected(
            "profile whitespace",
            json!({ "lint": { "profile": " recommended " } }),
        ),
        rejected(
            "severity alias",
            json!({
                "lint": {
                    "rule_severities": [{
                        "rule_id": CONFIGURABLE_RULE_ID,
                        "severity": "warn"
                    }]
                }
            }),
        ),
        rejected(
            "severity case",
            json!({
                "lint": {
                    "rule_severities": [{
                        "rule_id": CONFIGURABLE_RULE_ID,
                        "severity": "WARNING"
                    }]
                }
            }),
        ),
        rejected(
            "unknown rule",
            json!({ "lint": { "enable_rules": ["merman.unknown.rule"] } }),
        ),
        rejected(
            "internal rule",
            json!({ "lint": { "disable_rules": ["merman.internal.rule_registry_gap"] } }),
        ),
        rejected(
            "resource rule",
            json!({
                "lint": {
                    "rule_severities": [{
                        "rule_id": "merman.resource.source_bytes_exceeded",
                        "severity": "warning"
                    }]
                }
            }),
        ),
        rejected(
            "invalid date syntax",
            json!({ "fixed_today": "+9999-01-01" }),
        ),
        rejected(
            "offset out of range",
            json!({ "fixed_local_offset_minutes": 1440 }),
        ),
        rejected("site config type", json!({ "site_config": "dark" })),
        rejected("lint type", json!({ "lint": [] })),
        rejected(
            "missing severity",
            json!({
                "lint": {
                    "rule_severities": [{ "rule_id": CONFIGURABLE_RULE_ID }]
                }
            }),
        ),
    ]
}

fn accepted(name: &'static str, value: Value) -> ConfigCase {
    ConfigCase {
        name,
        value,
        accepted: true,
    }
}

fn rejected(name: &'static str, value: Value) -> ConfigCase {
    ConfigCase {
        name,
        value,
        accepted: false,
    }
}
