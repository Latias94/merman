use merman::diagram_theme::{
    DiagramFamilyId, DiagramThemeCompiler, ThemeColorTokenV1, ThemeDefinitionAdmissionError,
    ThemeDefinitionBuilderV1, ThemeDefinitionCompileError, ThemeDefinitionV1,
    ThemeMaterializationError, ThemeMaterializer, ThemeRuleBuilderV1, ThemeRuleFacetV1,
    ThemeRuleSetWireV1, ThemeStylePatchWireV1, ThemeTarget, ThemeTokensV1, ThemeVariant,
    compile_theme_definition, compile_theme_definition_json,
};
use merman::svg::{ThemeResourceLimitId, ThemeResourcePolicy};

#[test]
fn typed_family_rule_matches_the_equivalent_shareable_json() {
    let typed = ThemeDefinitionBuilderV1::new(ThemeTokensV1::default())
        .with_rule(
            ThemeRuleBuilderV1::new(ThemeTarget::Node)
                .for_family(DiagramFamilyId::FLOWCHART)
                .with_variant(ThemeVariant::Primary)
                .with_ordinal_cycle(4, 1)
                .with_fill_color("#60a5fa")
                .with_stroke_color("#1e293b")
                .with_stroke_width(2.0)
                .with_radius(8.0)
                .clear(ThemeRuleFacetV1::FillOpacity),
        )
        .with_ordinal_palette(ThemeTarget::Node, ["#60a5fa", "#34d399"])
        .build();
    let decoded: ThemeDefinitionV1 = serde_json::from_str(
        r##"{
            "authoring_schema_version": 1,
            "expansion_version": 1,
            "tokens": {},
            "styles": [
                {
                    "kind": "rule",
                    "target": "node",
                    "family": "flowchart",
                    "variant": "primary",
                    "ordinal": {"cycle": {"period": 4, "offset": 1}},
                    "style": {
                        "fill": "#60a5fa",
                        "fill_opacity": null,
                        "stroke": {"paint": "#1e293b", "width": 2},
                        "radius": 8
                    }
                },
                {
                    "kind": "ordinal-palette",
                    "target": "node",
                    "colors": ["#60a5fa", "#34d399"]
                }
            ]
        }"##,
    )
    .expect("the equivalent shared JSON should decode");

    assert_eq!(
        typed
            .canonical_json_bytes()
            .expect("the typed definition should canonicalize"),
        decoded
            .canonical_json_bytes()
            .expect("the decoded definition should canonicalize"),
    );
}

#[test]
fn versioned_authoring_materializes_and_compiles_through_the_rust_facade() {
    let definition = ThemeDefinitionV1::new(
        ThemeTokensV1::default()
            .with_color(ThemeColorTokenV1::Canvas, "#0f172a")
            .with_color(ThemeColorTokenV1::Surface, "#111827")
            .with_color(ThemeColorTokenV1::Text, "#e5e7eb"),
    );
    let compiler = DiagramThemeCompiler::new();
    let materialized = ThemeMaterializer::new()
        .materialize_theme(&definition)
        .expect("the explicit materialization step should succeed");
    let expected = compiler
        .compile_spec_wire(materialized.into_spec())
        .expect("the explicit compilation step should succeed");
    let theme = compile_theme_definition(&compiler, &definition)
        .expect("the materialized complete spec should compile");

    assert_eq!(theme.recipe_fingerprint(), expected.recipe_fingerprint());
}

#[test]
fn rust_facade_preserves_materialization_and_compilation_error_sources() {
    let materialization_error = compile_theme_definition(
        &DiagramThemeCompiler::new(),
        &ThemeDefinitionV1::new(ThemeTokensV1::default().with_series(Vec::new())),
    )
    .expect_err("an empty authored series should fail during materialization");
    assert!(matches!(
        materialization_error,
        ThemeDefinitionCompileError::Materialization(ThemeMaterializationError::EmptySeries)
    ));

    let compilation_error = compile_theme_definition(
        &DiagramThemeCompiler::new(),
        &ThemeDefinitionV1::new(ThemeTokensV1::default()).with_styles(vec![
            ThemeRuleSetWireV1::Rule {
                target: "future-node".to_owned(),
                family: None,
                variant: None,
                ordinal: None,
                style: ThemeStylePatchWireV1::default(),
            },
        ]),
    )
    .expect_err("an unknown semantic target should fail during compilation");
    assert!(matches!(
        compilation_error,
        ThemeDefinitionCompileError::Compilation(_)
    ));
}

#[test]
fn json_authoring_rejects_the_first_rule_beyond_the_v1_budget_before_typed_decode() {
    let rule = r##"{"kind":"rule","target":"node","style":{"fill":"#123456"}}"##;
    let admitted_styles = std::iter::repeat_n(rule, 489).collect::<Vec<_>>().join(",");

    let oversized = format!(
        r##"{{"authoring_schema_version":1,"expansion_version":1,"tokens":{{}},"styles":[{admitted_styles},{{"kind":"rule","target":{{"typed":"decode must not reach this value"}},"style":{{"fill":"#123456"}}}}]}}"##
    );
    let error = compile_theme_definition_json(&DiagramThemeCompiler::new(), oversized.as_bytes())
        .expect_err("the first rule beyond the V1 budget must fail during streaming admission");
    assert!(matches!(
        error,
        ThemeDefinitionCompileError::Materialization(
            ThemeMaterializationError::RuleBudgetExceeded {
                actual: 490,
                max: 489
            }
        )
    ));
}

#[test]
fn json_authoring_collection_limits_accept_the_exact_boundary_and_reject_the_next_item() {
    let colors = std::iter::repeat_n("\"#123456\"", 256)
        .collect::<Vec<_>>()
        .join(",");
    let exact_series = format!(
        r#"{{"authoring_schema_version":1,"expansion_version":1,"tokens":{{"series":[{colors}]}}}}"#
    );
    compile_theme_definition_json(&DiagramThemeCompiler::new(), exact_series.as_bytes())
        .expect("the exact series-color budget should compile");
    let oversized_series = format!(
        r#"{{"authoring_schema_version":1,"expansion_version":1,"tokens":{{"series":[{colors},{{"typed":"decode must not reach this value"}}]}}}}"#
    );
    assert!(matches!(
        compile_theme_definition_json(&DiagramThemeCompiler::new(), oversized_series.as_bytes()),
        Err(ThemeDefinitionCompileError::Materialization(
            ThemeMaterializationError::InvalidTokenValue {
                path: "/tokens/series"
            }
        ))
    ));

    let families = (0..32)
        .map(|index| format!("\"Authoring Font {index}\""))
        .collect::<Vec<_>>()
        .join(",");
    let exact_fonts = format!(
        r#"{{"authoring_schema_version":1,"expansion_version":1,"tokens":{{"typography":{{"font_stack":[{families}]}}}}}}"#
    );
    compile_theme_definition_json(&DiagramThemeCompiler::new(), exact_fonts.as_bytes())
        .expect("the exact font-stack budget should compile");
    let oversized_fonts = format!(
        r#"{{"authoring_schema_version":1,"expansion_version":1,"tokens":{{"typography":{{"font_stack":[{families},{{"typed":"decode must not reach this value"}}]}}}}}}"#
    );
    assert!(matches!(
        compile_theme_definition_json(&DiagramThemeCompiler::new(), oversized_fonts.as_bytes()),
        Err(ThemeDefinitionCompileError::Materialization(
            ThemeMaterializationError::InvalidTokenValue {
                path: "/tokens/typography/font_stack"
            }
        ))
    ));

    let exact_palette = format!(
        r#"{{"authoring_schema_version":1,"expansion_version":1,"tokens":{{}},"styles":[{{"kind":"ordinal-palette","target":"node","colors":[{colors}]}}]}}"#
    );
    compile_theme_definition_json(&DiagramThemeCompiler::new(), exact_palette.as_bytes())
        .expect("the exact ordinal-palette color budget should compile");
    let oversized_palette = format!(
        r#"{{"authoring_schema_version":1,"expansion_version":1,"tokens":{{}},"styles":[{{"kind":"ordinal-palette","target":"node","colors":[{colors},{{"typed":"decode must not reach this value"}}]}}]}}"#
    );
    assert!(matches!(
        compile_theme_definition_json(&DiagramThemeCompiler::new(), oversized_palette.as_bytes()),
        Err(ThemeDefinitionCompileError::Admission(
            ThemeDefinitionAdmissionError::InvalidJson { .. }
        ))
    ));
}

#[test]
fn typed_compact_paint_obeys_the_decoded_string_ceiling() {
    let definition = ThemeDefinitionBuilderV1::new(ThemeTokensV1::default())
        .with_rule(
            ThemeRuleBuilderV1::new(ThemeTarget::Node).with_fill_color("x".repeat(64 * 1024 + 1)),
        )
        .build();
    let error = compile_theme_definition(&DiagramThemeCompiler::new(), &definition)
        .expect_err("compact paint strings must use the shared decoded-string ceiling");

    assert!(matches!(
        error,
        ThemeDefinitionCompileError::Admission(ThemeDefinitionAdmissionError::CollectionLimit {
            path: "/styles/rule/style/paint/color",
            actual: 65_537,
            max: 65_536,
        })
    ));
}

#[test]
fn typed_authoring_uses_the_caller_owned_encoded_input_policy_before_expansion() {
    let policy = ThemeResourcePolicy::default()
        .with_limit(ThemeResourceLimitId::MaxThemeEncodedBytes, 1)
        .expect("one byte is a valid caller-owned encoded-theme ceiling");
    let compiler = DiagramThemeCompiler::new().with_resource_policy(policy);
    let error =
        compile_theme_definition(&compiler, &ThemeDefinitionV1::new(ThemeTokensV1::default()))
            .expect_err(
                "the typed facade must apply the compiler policy before materialization expands",
            );

    assert!(matches!(
        error,
        ThemeDefinitionCompileError::Admission(ThemeDefinitionAdmissionError::ResourceLimit(
            ref error
        )) if error.limit == "max_theme_encoded_bytes" && error.actual == 2 && error.max == 1
    ));
}
