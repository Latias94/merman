use merman_theme_contract::{ThemeMaterializationDiagnosticV1, ThemeMaterializationErrorV1};

fn encode(diagnostic: ThemeMaterializationDiagnosticV1) -> String {
    serde_json::to_string(&ThemeMaterializationErrorV1::from_diagnostic(diagnostic))
        .expect("a valid materialization error should serialize")
}

fn empty_series_value() -> serde_json::Value {
    serde_json::to_value(ThemeMaterializationErrorV1::from_diagnostic(
        ThemeMaterializationDiagnosticV1::empty_series("series must not be empty"),
    ))
    .expect("the empty-series diagnostic should serialize")
}

#[test]
fn empty_series_has_an_exact_version_one_golden_wire() {
    let diagnostic = ThemeMaterializationDiagnosticV1::empty_series("series must not be empty");
    let error = ThemeMaterializationErrorV1::from_diagnostic(diagnostic);

    assert_eq!(error.schema_version(), 1);
    assert_eq!(error.diagnostics().len(), 1);
    assert_eq!(error.diagnostic().code(), "theme-authoring.empty-series");
    assert_eq!(error.diagnostic().severity(), "error");
    assert_eq!(error.diagnostic().path(), "/tokens/series");
    assert_eq!(error.diagnostic().message(), "series must not be empty");
    assert_eq!(error.diagnostic().limit_id(), None);
    assert_eq!(error.diagnostic().actual(), None);

    let encoded = serde_json::to_string(&error).expect("the error should serialize");
    assert_eq!(
        encoded,
        r#"{"schema_version":1,"diagnostics":[{"code":"theme-authoring.empty-series","severity":"error","path":"/tokens/series","details":{},"message":"series must not be empty"}]}"#
    );

    let decoded: ThemeMaterializationErrorV1 =
        serde_json::from_str(&encoded).expect("the golden error should decode");
    assert_eq!(decoded, error);
}

#[test]
fn duplicate_palette_has_an_exact_version_one_golden_wire() {
    let diagnostic = ThemeMaterializationDiagnosticV1::duplicate_palette_target(
        "/styles/3/target",
        "chart-series",
        1,
        3,
        "palette target is duplicated",
    );

    assert_eq!(diagnostic.target_id(), Some("chart-series"));
    assert_eq!(diagnostic.first_authored_index(), Some(1));
    assert_eq!(diagnostic.duplicate_authored_index(), Some(3));
    assert_eq!(
        encode(diagnostic),
        r#"{"schema_version":1,"diagnostics":[{"code":"theme-authoring.duplicate-palette-target","severity":"error","path":"/styles/3/target","details":{"target_id":"chart-series","first_authored_index":1,"duplicate_authored_index":3},"message":"palette target is duplicated"}]}"#
    );
}

#[test]
fn resource_limit_has_an_exact_version_one_golden_wire() {
    let diagnostic = ThemeMaterializationDiagnosticV1::resource_limit_exceeded(
        "",
        "max_theme_encoded_bytes",
        65_537,
        65_536,
        "encoded theme definition is too large",
    );

    assert_eq!(diagnostic.limit_id(), Some("max_theme_encoded_bytes"));
    assert_eq!(diagnostic.actual(), Some(65_537));
    assert_eq!(diagnostic.max(), Some(65_536));
    assert_eq!(
        encode(diagnostic),
        r#"{"schema_version":1,"diagnostics":[{"code":"theme-authoring.resource-limit-exceeded","severity":"error","path":"","details":{"limit_id":"max_theme_encoded_bytes","actual":65537,"max":65536},"message":"encoded theme definition is too large"}]}"#
    );
}

#[test]
fn unsupported_version_tuple_has_an_exact_version_one_golden_wire() {
    let diagnostic = ThemeMaterializationDiagnosticV1::unsupported_version_tuple(
        2,
        7,
        "unsupported authoring version tuple",
    );

    assert_eq!(diagnostic.actual_version_tuple(), Some((2, 7)));
    assert_eq!(
        encode(diagnostic),
        r#"{"schema_version":1,"diagnostics":[{"code":"theme-authoring.unsupported-version-tuple","severity":"error","path":"","details":{"actual":{"authoring_schema_version":2,"expansion_version":7},"supported":[{"authoring_schema_version":1,"expansion_version":1}]},"message":"unsupported authoring version tuple"}]}"#
    );
}

#[test]
fn the_remaining_closed_codes_have_typed_details_and_round_trip() {
    let diagnostics = [
        ThemeMaterializationDiagnosticV1::invalid_definition_json(
            "schema-mismatch",
            "definition does not match ThemeDefinitionV1",
        ),
        ThemeMaterializationDiagnosticV1::invalid_token_value(
            "/tokens/canvas",
            "css-color",
            "canvas is not a valid color",
        ),
        ThemeMaterializationDiagnosticV1::rule_budget_exceeded(
            "max_authored_rules",
            490,
            489,
            "too many authored rules",
        ),
        ThemeMaterializationDiagnosticV1::effect_reference_not_supported(
            "/styles/2/style/effect",
            2,
            "effect references are not supported",
        ),
    ];
    let expected_codes = [
        "theme-authoring.invalid-definition-json",
        "theme-authoring.invalid-token-value",
        "theme-authoring.rule-budget-exceeded",
        "theme-authoring.effect-reference-not-supported",
    ];

    for (diagnostic, expected_code) in diagnostics.into_iter().zip(expected_codes) {
        assert_eq!(diagnostic.code(), expected_code);
        let encoded = encode(diagnostic);
        let decoded: ThemeMaterializationErrorV1 =
            serde_json::from_str(&encoded).expect("every closed diagnostic should round trip");
        assert_eq!(decoded.diagnostic().code(), expected_code);
    }
}

#[test]
fn display_message_is_serialized_but_not_part_of_diagnostic_identity() {
    let first = ThemeMaterializationErrorV1::from_diagnostic(
        ThemeMaterializationDiagnosticV1::empty_series("first wording"),
    );
    let second = ThemeMaterializationErrorV1::from_diagnostic(
        ThemeMaterializationDiagnosticV1::empty_series("second wording"),
    );

    assert_eq!(first, second);
    assert_ne!(
        serde_json::to_string(&first).unwrap(),
        serde_json::to_string(&second).unwrap()
    );
}

#[test]
fn error_envelope_rejects_unknown_null_missing_and_wrong_cardinality() {
    let valid = empty_series_value();

    let mut unknown = valid.clone();
    unknown
        .as_object_mut()
        .unwrap()
        .insert("unexpected".to_owned(), true.into());
    assert!(serde_json::from_value::<ThemeMaterializationErrorV1>(unknown).is_err());

    for field in ["schema_version", "diagnostics"] {
        let mut missing = valid.clone();
        missing.as_object_mut().unwrap().remove(field);
        assert!(
            serde_json::from_value::<ThemeMaterializationErrorV1>(missing).is_err(),
            "missing {field} must be rejected"
        );

        let mut null = valid.clone();
        null.as_object_mut()
            .unwrap()
            .insert(field.to_owned(), serde_json::Value::Null);
        assert!(
            serde_json::from_value::<ThemeMaterializationErrorV1>(null).is_err(),
            "null {field} must be rejected"
        );
    }

    let mut wrong_version = valid.clone();
    wrong_version["schema_version"] = 2.into();
    assert!(serde_json::from_value::<ThemeMaterializationErrorV1>(wrong_version).is_err());

    let mut empty = valid.clone();
    empty["diagnostics"] = serde_json::json!([]);
    assert!(serde_json::from_value::<ThemeMaterializationErrorV1>(empty).is_err());

    let mut multiple = valid.clone();
    let diagnostic = multiple["diagnostics"][0].clone();
    multiple["diagnostics"] = serde_json::json!([diagnostic.clone(), diagnostic]);
    assert!(serde_json::from_value::<ThemeMaterializationErrorV1>(multiple).is_err());
}

#[test]
fn diagnostic_rejects_unknown_null_and_code_details_mismatches() {
    let valid = empty_series_value();

    for field in ["code", "severity", "path", "details", "message"] {
        let mut missing = valid.clone();
        missing["diagnostics"][0]
            .as_object_mut()
            .unwrap()
            .remove(field);
        assert!(
            serde_json::from_value::<ThemeMaterializationErrorV1>(missing).is_err(),
            "missing diagnostic {field} must be rejected"
        );

        let mut null = valid.clone();
        null["diagnostics"][0][field] = serde_json::Value::Null;
        assert!(
            serde_json::from_value::<ThemeMaterializationErrorV1>(null).is_err(),
            "null diagnostic {field} must be rejected"
        );
    }

    let mut unknown_code = valid.clone();
    unknown_code["diagnostics"][0]["code"] = "theme-authoring.future-code".into();
    assert!(serde_json::from_value::<ThemeMaterializationErrorV1>(unknown_code).is_err());

    let mut unknown_field = valid.clone();
    unknown_field["diagnostics"][0]["unexpected"] = true.into();
    assert!(serde_json::from_value::<ThemeMaterializationErrorV1>(unknown_field).is_err());

    let mut unknown_detail = valid.clone();
    unknown_detail["diagnostics"][0]["details"]["unexpected"] = true.into();
    assert!(serde_json::from_value::<ThemeMaterializationErrorV1>(unknown_detail).is_err());

    let mut wrong_severity = valid.clone();
    wrong_severity["diagnostics"][0]["severity"] = "warning".into();
    assert!(serde_json::from_value::<ThemeMaterializationErrorV1>(wrong_severity).is_err());

    let mut wrong_details = valid.clone();
    wrong_details["diagnostics"][0]["code"] = "theme-authoring.resource-limit-exceeded".into();
    assert!(serde_json::from_value::<ThemeMaterializationErrorV1>(wrong_details).is_err());

    let mut null_diagnostic = valid.clone();
    null_diagnostic["diagnostics"][0] = serde_json::Value::Null;
    assert!(serde_json::from_value::<ThemeMaterializationErrorV1>(null_diagnostic).is_err());

    let mut wrong_supported_tuple: serde_json::Value = serde_json::from_str(&encode(
        ThemeMaterializationDiagnosticV1::unsupported_version_tuple(2, 7, "unsupported tuple"),
    ))
    .unwrap();
    wrong_supported_tuple["diagnostics"][0]["details"]["supported"][0]["authoring_schema_version"] =
        2.into();
    assert!(serde_json::from_value::<ThemeMaterializationErrorV1>(wrong_supported_tuple).is_err());
}

#[test]
fn paths_must_be_bounded_rfc_6901_json_pointers() {
    let mut valid = empty_series_value();
    valid["diagnostics"][0]["path"] = "/tokens/a~0b/~1".into();
    serde_json::from_value::<ThemeMaterializationErrorV1>(valid)
        .expect("escaped reference tokens are valid JSON Pointer syntax");

    for path in [
        "tokens/series",
        "#/tokens/series",
        "/tokens/~",
        "/tokens/~2",
    ] {
        let mut invalid = empty_series_value();
        invalid["diagnostics"][0]["path"] = path.into();
        assert!(
            serde_json::from_value::<ThemeMaterializationErrorV1>(invalid).is_err(),
            "invalid JSON Pointer {path:?} must be rejected"
        );
    }

    let mut at_limit = empty_series_value();
    at_limit["diagnostics"][0]["path"] = format!("/{}", "p".repeat(511)).into();
    serde_json::from_value::<ThemeMaterializationErrorV1>(at_limit)
        .expect("a 512-byte JSON Pointer should be accepted");

    let mut over_limit = empty_series_value();
    over_limit["diagnostics"][0]["path"] = format!("/{}", "p".repeat(512)).into();
    assert!(serde_json::from_value::<ThemeMaterializationErrorV1>(over_limit).is_err());
}

#[test]
fn message_identifiers_and_target_ids_enforce_byte_limits() {
    let mut at_message_limit = empty_series_value();
    at_message_limit["diagnostics"][0]["message"] = "m".repeat(512).into();
    serde_json::from_value::<ThemeMaterializationErrorV1>(at_message_limit)
        .expect("a 512-byte message should be accepted");

    let mut over_message_limit = empty_series_value();
    over_message_limit["diagnostics"][0]["message"] = "m".repeat(513).into();
    assert!(serde_json::from_value::<ThemeMaterializationErrorV1>(over_message_limit).is_err());

    let diagnostic = ThemeMaterializationDiagnosticV1::invalid_definition_json(
        "r".repeat(128),
        "invalid definition",
    );
    let at_identifier_limit =
        serde_json::to_value(ThemeMaterializationErrorV1::from_diagnostic(diagnostic)).unwrap();
    serde_json::from_value::<ThemeMaterializationErrorV1>(at_identifier_limit)
        .expect("a 128-byte stable identifier should be accepted");

    let mut over_identifier_limit = empty_series_value();
    over_identifier_limit["diagnostics"][0]["code"] =
        "theme-authoring.invalid-definition-json".into();
    over_identifier_limit["diagnostics"][0]["details"] =
        serde_json::json!({"reason_id": "r".repeat(129)});
    assert!(serde_json::from_value::<ThemeMaterializationErrorV1>(over_identifier_limit).is_err());

    let target_at_limit = ThemeMaterializationDiagnosticV1::duplicate_palette_target(
        "/styles/1/target",
        "t".repeat(64 * 1024),
        0,
        1,
        "duplicate palette target",
    );
    serde_json::to_value(ThemeMaterializationErrorV1::from_diagnostic(
        target_at_limit,
    ))
    .expect("a 64-KiB target identifier should be accepted");
}

#[test]
fn renderer_constructors_enforce_the_same_bounds() {
    assert!(
        std::panic::catch_unwind(|| {
            ThemeMaterializationDiagnosticV1::invalid_token_value(
                "tokens/canvas",
                "css-color",
                "invalid color",
            )
        })
        .is_err()
    );
    assert!(
        std::panic::catch_unwind(|| {
            ThemeMaterializationDiagnosticV1::empty_series("m".repeat(513))
        })
        .is_err()
    );
    assert!(
        std::panic::catch_unwind(|| {
            ThemeMaterializationDiagnosticV1::invalid_definition_json(
                "r".repeat(129),
                "invalid definition",
            )
        })
        .is_err()
    );
    assert!(
        std::panic::catch_unwind(|| {
            ThemeMaterializationDiagnosticV1::duplicate_palette_target(
                "/styles/1/target",
                "t".repeat(64 * 1024 + 1),
                0,
                1,
                "duplicate palette target",
            )
        })
        .is_err()
    );
}
