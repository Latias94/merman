use merman_render::diagram_theme::{
    DiagramThemeCompiler, ThemeDefinitionCompileError, ThemeDefinitionV1, ThemeMaterializer,
    ThemeResourceLimitId, ThemeResourcePolicy, compile_theme_definition,
    compile_theme_definition_json,
};
use merman_theme_contract::{
    SpecifiedWireV1, ThemeCanvasPaintWireV1, ThemeColorTokenV1, ThemeMaterializationErrorV1,
    ThemeRuleSetWireV1, ThemeStylePatchWireV1, ThemeTokensV1,
};

fn assert_same_budget_diagnostic(
    typed: &ThemeMaterializationErrorV1,
    json: &ThemeMaterializationErrorV1,
) {
    assert_eq!(json.diagnostic().code(), typed.diagnostic().code());
    assert_eq!(json.diagnostic().path(), typed.diagnostic().path());
    assert_eq!(json.diagnostic().limit_id(), typed.diagnostic().limit_id());
    assert_eq!(json.diagnostic().actual(), typed.diagnostic().actual());
    assert_eq!(json.diagnostic().max(), typed.diagnostic().max());
}

#[test]
fn typed_and_json_operations_share_bounded_materialization_without_compiling() {
    let definition = ThemeDefinitionV1::new(
        ThemeTokensV1::default().with_color(ThemeColorTokenV1::Canvas, "#010203"),
    )
    .with_styles(vec![ThemeRuleSetWireV1::Rule {
        target: "future-target".to_owned(),
        family: None,
        variant: None,
        ordinal: None,
        style: ThemeStylePatchWireV1 {
            fill: SpecifiedWireV1::Value(ThemeCanvasPaintWireV1::Color("#abcdef".to_owned())),
            ..ThemeStylePatchWireV1::default()
        },
    }]);
    let compiler = DiagramThemeCompiler::new();
    let materializer = ThemeMaterializer::new();

    let typed = materializer
        .materialize_theme(&definition)
        .expect("the typed operation should stop after materialization");
    let json = definition
        .canonical_json_bytes()
        .expect("the definition should have canonical JSON");
    let decoded = materializer
        .materialize_theme_json(&json)
        .expect("the JSON operation should use the same materialization path");

    assert_eq!(typed, decoded);
    assert!(matches!(
        compile_theme_definition(&compiler, &definition),
        Err(ThemeDefinitionCompileError::Compilation(_))
    ));
    assert!(matches!(
        compile_theme_definition_json(&compiler, &json),
        Err(ThemeDefinitionCompileError::Compilation(_))
    ));
}

#[test]
fn compile_facade_delegates_the_contract_materialization_error() {
    let definition = ThemeDefinitionV1::new(ThemeTokensV1::default().with_series(Vec::new()));
    let compiler = DiagramThemeCompiler::new();

    let materializer = ThemeMaterializer::new();
    let operation_error = materializer
        .materialize_theme(&definition)
        .expect_err("the public operation must reject an empty series");
    let json = br#"{"authoring_schema_version":1,"expansion_version":1,"tokens":{"series":[]}}"#;
    let json_error = materializer
        .materialize_theme_json(json)
        .expect_err("the JSON operation must share the same expansion error");
    let compile_error = compile_theme_definition(&compiler, &definition)
        .expect_err("the compile convenience must delegate materialization");
    let json_compile_error = compile_theme_definition_json(&compiler, json)
        .expect_err("the JSON compile convenience must delegate materialization");

    assert_eq!(json_error, operation_error);
    assert!(matches!(
        compile_error,
        ThemeDefinitionCompileError::Materialization(error) if error == operation_error
    ));
    assert!(matches!(
        json_compile_error,
        ThemeDefinitionCompileError::Materialization(error) if error == operation_error
    ));
}

#[test]
fn materialization_errors_use_the_closed_contract_diagnostics() {
    let materializer = ThemeMaterializer::new();

    let invalid_color = ThemeDefinitionV1::new(
        ThemeTokensV1::default().with_color(ThemeColorTokenV1::Canvas, "not-a-color"),
    );
    let error = materializer
        .materialize_theme(&invalid_color)
        .expect_err("an invalid token must fail without a partial result");
    assert_eq!(
        error.diagnostic().code(),
        "theme-authoring.invalid-token-value"
    );
    assert_eq!(error.diagnostic().path(), "/tokens/canvas");
    assert_eq!(error.diagnostic().expected_domain_id(), Some("css-color"));

    let duplicate_palette = ThemeDefinitionV1::new(ThemeTokensV1::default()).with_styles(vec![
        ThemeRuleSetWireV1::OrdinalPalette {
            target: "node".to_owned(),
            colors: vec!["#111111".to_owned()],
        },
        ThemeRuleSetWireV1::OrdinalPalette {
            target: "node".to_owned(),
            colors: vec!["#222222".to_owned()],
        },
    ]);
    let error = materializer
        .materialize_theme(&duplicate_palette)
        .expect_err("duplicate authored palettes must fail closed");
    let diagnostic = error.diagnostic();
    assert_eq!(
        diagnostic.code(),
        "theme-authoring.duplicate-palette-target"
    );
    assert_eq!(diagnostic.path(), "/styles/1/target");
    assert_eq!(diagnostic.target_id(), Some("node"));
    assert_eq!(diagnostic.first_authored_index(), Some(0));
    assert_eq!(diagnostic.duplicate_authored_index(), Some(1));

    let unsupported_effect = ThemeDefinitionV1::new(ThemeTokensV1::default()).with_styles(vec![
        ThemeRuleSetWireV1::Rule {
            target: "node".to_owned(),
            family: None,
            variant: None,
            ordinal: None,
            style: ThemeStylePatchWireV1 {
                effect: SpecifiedWireV1::Value("soft-shadow".to_owned()),
                ..ThemeStylePatchWireV1::default()
            },
        },
    ]);
    let error = materializer
        .materialize_theme(&unsupported_effect)
        .expect_err("effect references are outside the version one authoring envelope");
    let diagnostic = error.diagnostic();
    assert_eq!(
        diagnostic.code(),
        "theme-authoring.effect-reference-not-supported"
    );
    assert_eq!(diagnostic.path(), "/styles/0/style/effect");
    assert_eq!(diagnostic.authored_index(), Some(0));
}

#[test]
fn materialization_budgets_keep_their_stable_contract_limits() {
    let authored_rule = ThemeRuleSetWireV1::Rule {
        target: "node".to_owned(),
        family: None,
        variant: None,
        ordinal: None,
        style: ThemeStylePatchWireV1::default(),
    };
    let rule_error = ThemeMaterializer::new()
        .materialize_theme(
            &ThemeDefinitionV1::new(ThemeTokensV1::default()).with_styles(vec![authored_rule; 490]),
        )
        .expect_err("the first authored rule beyond the derived budget must fail");
    let diagnostic = rule_error.diagnostic();
    assert_eq!(diagnostic.code(), "theme-authoring.rule-budget-exceeded");
    assert_eq!(diagnostic.limit_id(), Some("max_authored_rules"));
    assert_eq!(diagnostic.actual(), Some(490));
    assert_eq!(diagnostic.max(), Some(489));

    let palettes = (0..63)
        .map(|index| ThemeRuleSetWireV1::OrdinalPalette {
            target: format!("custom-palette-{index}"),
            colors: vec!["#123456".to_owned()],
        })
        .collect();
    let palette_error = ThemeMaterializer::new()
        .materialize_theme(&ThemeDefinitionV1::new(ThemeTokensV1::default()).with_styles(palettes))
        .expect_err("the first materialized palette beyond the complete-spec budget must fail");
    let diagnostic = palette_error.diagnostic();
    assert_eq!(diagnostic.code(), "theme-authoring.resource-limit-exceeded");
    assert_eq!(diagnostic.path(), "/styles");
    assert_eq!(diagnostic.limit_id(), Some("max_theme_ordinal_palettes"));
    assert_eq!(diagnostic.actual(), Some(65));
    assert_eq!(diagnostic.max(), Some(64));
}

#[test]
fn json_operation_reports_versions_and_stable_rejection_reasons() {
    let materializer = ThemeMaterializer::new();
    let unsupported = br#"{
        "authoring_schema_version": 2,
        "expansion_version": 7,
        "tokens": {}
    }"#;
    let error = materializer
        .materialize_theme_json(unsupported)
        .expect_err("unknown authoring tuples must fail before typed decoding");
    assert_eq!(
        error.diagnostic().code(),
        "theme-authoring.unsupported-version-tuple"
    );
    assert_eq!(error.diagnostic().actual_version_tuple(), Some((2, 7)));

    let cases: [(&[u8], &str); 3] = [
        (br#"{"authoring_schema_version": 1,"#, "malformed-json"),
        (
            br#"{"authoring_schema_version":1,"authoring_schema_version":1,"expansion_version":1,"tokens":{}}"#,
            "duplicate-object-key",
        ),
        (
            br#"{"authoring_schema_version":1,"expansion_version":1,"tokens":{},"unknown":true}"#,
            "contract-shape",
        ),
    ];
    for (json, expected_reason) in cases {
        let error = materializer
            .materialize_theme_json(json)
            .expect_err("invalid JSON must use one stable coarse rejection reason");
        assert_eq!(
            error.diagnostic().code(),
            "theme-authoring.invalid-definition-json"
        );
        assert_eq!(error.diagnostic().reason_id(), Some(expected_reason));
    }
}

#[test]
fn unknown_version_tuple_precedes_version_one_semantic_errors() {
    let rule = r#"{"kind":"rule","target":"node","style":{}}"#;
    let styles = std::iter::repeat_n(rule, 490).collect::<Vec<_>>().join(",");
    let json = format!(
        r#"{{"styles":[{styles}],"tokens":{{}},"authoring_schema_version":2,"expansion_version":7}}"#
    );

    let error = ThemeMaterializer::new()
        .materialize_theme_json(json.as_bytes())
        .expect_err(
            "an unknown tuple must win even when a V1 rule budget fails first in source order",
        );

    assert_eq!(
        error.diagnostic().code(),
        "theme-authoring.unsupported-version-tuple"
    );
    assert_eq!(error.diagnostic().actual_version_tuple(), Some((2, 7)));
}

#[test]
fn typed_and_canonical_json_failures_share_diagnostic_identity() {
    let rule = ThemeRuleSetWireV1::Rule {
        target: "node".to_owned(),
        family: None,
        variant: None,
        ordinal: None,
        style: ThemeStylePatchWireV1::default(),
    };
    let oversized_rules =
        ThemeDefinitionV1::new(ThemeTokensV1::default()).with_styles(vec![rule; 500]);
    let rules_json = oversized_rules
        .canonical_json_bytes()
        .expect("an over-budget typed definition still has canonical JSON");
    let materializer = ThemeMaterializer::new();
    let typed_error = materializer
        .materialize_theme(&oversized_rules)
        .expect_err("typed rules must fail the authored-rule budget");
    let json_error = materializer
        .materialize_theme_json(&rules_json)
        .expect_err("canonical JSON rules must fail the same budget");
    assert_same_budget_diagnostic(&typed_error, &json_error);
    assert_eq!(
        typed_error.diagnostic().code(),
        "theme-authoring.rule-budget-exceeded"
    );
    assert_eq!(typed_error.diagnostic().path(), "/styles");
    assert_eq!(
        typed_error.diagnostic().limit_id(),
        Some("max_authored_rules")
    );
    assert_eq!(typed_error.diagnostic().actual(), Some(490));
    assert_eq!(typed_error.diagnostic().max(), Some(489));

    let oversized_series = ThemeDefinitionV1::new(
        ThemeTokensV1::default().with_series(vec!["#123456".to_owned(); 300]),
    );
    let series_json = oversized_series
        .canonical_json_bytes()
        .expect("an over-budget palette still has canonical JSON");
    let typed_error = materializer
        .materialize_theme(&oversized_series)
        .expect_err("typed palette colors must fail the palette budget");
    let json_error = materializer
        .materialize_theme_json(&series_json)
        .expect_err("canonical JSON palette colors must fail the same budget");
    assert_same_budget_diagnostic(&typed_error, &json_error);
    assert_eq!(
        typed_error.diagnostic().code(),
        "theme-authoring.resource-limit-exceeded"
    );
    assert_eq!(typed_error.diagnostic().path(), "/tokens/series");
    assert_eq!(
        typed_error.diagnostic().limit_id(),
        Some("max_theme_palette_colors")
    );
    assert_eq!(typed_error.diagnostic().actual(), Some(257));
    assert_eq!(typed_error.diagnostic().max(), Some(256));

    let definition = ThemeDefinitionV1::new(ThemeTokensV1::default());
    let json = definition
        .canonical_json_bytes()
        .expect("the default definition must have canonical JSON");
    let materializer = ThemeMaterializer::new().with_resource_policy(
        ThemeResourcePolicy::default()
            .with_limit(ThemeResourceLimitId::MaxThemeEncodedBytes, 1)
            .expect("one byte is a valid caller-owned input ceiling"),
    );
    let typed_error = materializer
        .materialize_theme(&definition)
        .expect_err("typed input must fail the encoded-byte budget");
    let json_error = materializer
        .materialize_theme_json(&json)
        .expect_err("canonical JSON must fail the same encoded-byte budget");
    assert_same_budget_diagnostic(&typed_error, &json_error);
    assert_eq!(
        typed_error.diagnostic().code(),
        "theme-authoring.resource-limit-exceeded"
    );
    assert_eq!(typed_error.diagnostic().path(), "");
    assert_eq!(
        typed_error.diagnostic().limit_id(),
        Some("max_theme_encoded_bytes")
    );
    assert_eq!(typed_error.diagnostic().actual(), Some(2));
    assert_eq!(typed_error.diagnostic().max(), Some(1));
}

#[test]
fn rule_budget_precedes_earlier_palette_errors_for_typed_and_json() {
    let duplicate_palettes =
        ["#111111", "#222222"].map(|color| ThemeRuleSetWireV1::OrdinalPalette {
            target: "node".to_owned(),
            colors: vec![color.to_owned()],
        });
    let rule = ThemeRuleSetWireV1::Rule {
        target: "node".to_owned(),
        family: None,
        variant: None,
        ordinal: None,
        style: ThemeStylePatchWireV1::default(),
    };
    let styles = duplicate_palettes
        .into_iter()
        .chain(std::iter::repeat_n(rule, 490))
        .collect();
    let definition = ThemeDefinitionV1::new(ThemeTokensV1::default()).with_styles(styles);
    let json = definition
        .canonical_json_bytes()
        .expect("the over-budget definition should still have canonical JSON");
    let materializer = ThemeMaterializer::new();

    let typed_error = materializer
        .materialize_theme(&definition)
        .expect_err("the typed operation must apply the global rule budget first");
    let json_error = materializer
        .materialize_theme_json(&json)
        .expect_err("the JSON operation must use the same semantic error priority");

    assert_eq!(json_error, typed_error);
    assert_eq!(
        typed_error.diagnostic().code(),
        "theme-authoring.rule-budget-exceeded"
    );
}

#[test]
fn json_collection_limits_fail_as_resources_before_typed_decode() {
    let colors = std::iter::repeat_n(r##""#123456""##, 256)
        .collect::<Vec<_>>()
        .join(",");
    let oversized = format!(
        r#"{{"authoring_schema_version":1,"expansion_version":1,"tokens":{{"series":[{colors},{{"typed":"decode must not reach this value"}}]}}}}"#
    );

    let error = ThemeMaterializer::new()
        .materialize_theme_json(oversized.as_bytes())
        .expect_err("the first item beyond the palette bound must fail during preflight");
    let diagnostic = error.diagnostic();
    assert_eq!(diagnostic.code(), "theme-authoring.resource-limit-exceeded");
    assert_eq!(diagnostic.path(), "/tokens/series");
    assert_eq!(diagnostic.limit_id(), Some("max_theme_palette_colors"));
    assert_eq!(diagnostic.actual(), Some(257));
    assert_eq!(diagnostic.max(), Some(256));
}

#[test]
fn json_preflight_preserves_duplicate_palette_precedence_over_the_total_budget() {
    let palette = r##"{"kind":"ordinal-palette","target":"duplicate","colors":["#123456"]}"##;
    let styles = std::iter::repeat_n(palette, 63)
        .collect::<Vec<_>>()
        .join(",");
    let json = format!(
        r#"{{"authoring_schema_version":1,"expansion_version":1,"tokens":{{}},"styles":[{styles}]}}"#
    );

    let error = ThemeMaterializer::new()
        .materialize_theme_json(json.as_bytes())
        .expect_err("the second repeated target must fail before the palette-count ceiling");
    let diagnostic = error.diagnostic();
    assert_eq!(
        diagnostic.code(),
        "theme-authoring.duplicate-palette-target"
    );
    assert_eq!(diagnostic.target_id(), Some("duplicate"));
    assert_eq!(diagnostic.first_authored_index(), Some(0));
    assert_eq!(diagnostic.duplicate_authored_index(), Some(1));
}
