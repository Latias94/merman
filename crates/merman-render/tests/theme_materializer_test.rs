use merman_render::diagram_theme::{
    DiagramThemeCompiler, ThemeMaterializationError, ThemeMaterializer,
};
use merman_theme_contract::{
    SpecifiedWireV1, ThemeAuthoringTypographyV1, ThemeCanvasPaintWireV1, ThemeColorTokenV1,
    ThemeDefinitionV1, ThemeLineHeightWireV1, ThemeRuleSetWireV1, ThemeStylePatchWireV1,
    ThemeTokensV1,
};

#[test]
fn tokens_only_definition_materializes_the_complete_version_one_spec() {
    let definition = ThemeDefinitionV1::new(ThemeTokensV1::default());

    let first = ThemeMaterializer::new()
        .materialize_theme(&definition)
        .expect("version one defaults should materialize");
    let replayed: ThemeDefinitionV1 = serde_json::from_slice(
        &definition
            .canonical_json_bytes()
            .expect("the admitted definition should canonicalize"),
    )
    .expect("canonical authoring JSON should replay through the contract wire");
    let second = ThemeMaterializer::new()
        .materialize_theme(&replayed)
        .expect("the same definition should replay");

    assert_eq!(first.authoring_schema_version(), 1);
    assert_eq!(first.expansion_version(), 1);
    assert_eq!(first.spec_schema_version(), 1);
    assert_eq!(
        first.theme_materialization_digest(),
        second.theme_materialization_digest()
    );

    let spec = first.spec();
    assert_eq!(
        spec.canvas.as_ref().and_then(|canvas| canvas.base.as_ref()),
        Some(&ThemeCanvasPaintWireV1::Color("#ffffff".to_owned()))
    );
    let typography = spec
        .typography
        .as_ref()
        .and_then(|typography| typography.default.as_ref())
        .expect("authoring typography should be explicit in the complete spec");
    assert_eq!(
        typography.font_stack.as_deref(),
        Some(
            ["Inter", "ui-sans-serif", "system-ui", "sans-serif"]
                .map(str::to_owned)
                .as_slice()
        )
    );
    assert_eq!(typography.font_size_px, Some(16.0));
    assert_eq!(typography.font_weight, Some(400));

    let styles = spec
        .styles
        .as_ref()
        .expect("the generated rule set should be explicit");
    assert_eq!(styles.len(), 42);
    assert!(matches!(
        &styles[0],
        ThemeRuleSetWireV1::Rule { target, .. } if target == "text"
    ));
    assert!(matches!(
        &styles[39],
        ThemeRuleSetWireV1::Rule { target, variant, .. }
            if target == "table" && variant.as_deref() == Some("even")
    ));
    assert!(matches!(
        &styles[40],
        ThemeRuleSetWireV1::OrdinalPalette { target, colors }
            if target == "node"
                && colors == &["#2563eb", "#16a34a", "#d97706", "#9333ea"]
    ));
    assert!(matches!(
        &styles[41],
        ThemeRuleSetWireV1::OrdinalPalette { target, colors }
            if target == "pie-slice"
                && colors == &["#2563eb", "#16a34a", "#d97706", "#9333ea"]
    ));

    DiagramThemeCompiler::new()
        .compile_spec_wire(first.into_spec())
        .expect("the complete materialized wire should compile without another lowering path");
}

#[test]
fn authored_rules_append_and_authored_palettes_replace_or_extend_generated_slots() {
    let authored_rule = ThemeRuleSetWireV1::Rule {
        target: "node".to_owned(),
        family: Some("flowchart-v2".to_owned()),
        variant: Some("primary".to_owned()),
        ordinal: None,
        style: ThemeStylePatchWireV1 {
            fill: SpecifiedWireV1::Value(ThemeCanvasPaintWireV1::Color("#abcdef".to_owned())),
            ..ThemeStylePatchWireV1::default()
        },
    };
    let definition = ThemeDefinitionV1::new(
        ThemeTokensV1::default()
            .with_color(ThemeColorTokenV1::Canvas, "#010203")
            .with_color(ThemeColorTokenV1::Text, "#101010")
            .with_series(vec!["#111111".to_owned(), "#222222".to_owned()])
            .with_typography(
                ThemeAuthoringTypographyV1::default()
                    .with_font_stack(vec!["Merman Sans".to_owned(), "sans-serif".to_owned()])
                    .with_font_size_px(18.0)
                    .with_font_weight(500)
                    .with_line_height(ThemeLineHeightWireV1::Multiplier(1.4)),
            ),
    )
    .with_styles(vec![
        authored_rule.clone(),
        ThemeRuleSetWireV1::OrdinalPalette {
            target: "node".to_owned(),
            colors: vec!["#333333".to_owned(), "#444444".to_owned()],
        },
        ThemeRuleSetWireV1::OrdinalPalette {
            target: "chart-series".to_owned(),
            colors: vec!["#555555".to_owned()],
        },
    ]);

    let materialized = ThemeMaterializer::new()
        .materialize_theme(&definition)
        .expect("authored rules and unique palettes should compose");
    let styles = materialized.spec().styles.as_ref().expect("styles");

    assert_eq!(styles.len(), 44);
    assert_eq!(
        materialized
            .spec()
            .canvas
            .as_ref()
            .and_then(|canvas| canvas.base.as_ref()),
        Some(&ThemeCanvasPaintWireV1::Color("#010203".to_owned()))
    );
    let typography = materialized
        .spec()
        .typography
        .as_ref()
        .and_then(|typography| typography.default.as_ref())
        .expect("custom typography");
    assert_eq!(
        typography.font_stack.as_deref(),
        Some(["Merman Sans".to_owned(), "sans-serif".to_owned()].as_slice())
    );
    assert_eq!(typography.font_size_px, Some(18.0));
    assert_eq!(typography.font_weight, Some(500));
    assert_eq!(
        typography.line_height,
        Some(ThemeLineHeightWireV1::Multiplier(1.4))
    );
    assert!(matches!(
        &styles[0],
        ThemeRuleSetWireV1::Rule { style, .. }
            if matches!(
                &style.fill,
                SpecifiedWireV1::Value(ThemeCanvasPaintWireV1::Color(color))
                    if color == "#101010"
            )
    ));
    assert_eq!(&styles[40], &authored_rule);
    assert!(matches!(
        &styles[41],
        ThemeRuleSetWireV1::OrdinalPalette { target, colors }
            if target == "node" && colors == &["#333333", "#444444"]
    ));
    assert!(matches!(
        &styles[42],
        ThemeRuleSetWireV1::OrdinalPalette { target, colors }
            if target == "pie-slice" && colors == &["#111111", "#222222"]
    ));
    assert!(matches!(
        &styles[43],
        ThemeRuleSetWireV1::OrdinalPalette { target, colors }
            if target == "chart-series" && colors == &["#555555"]
    ));

    let defaults = ThemeMaterializer::new()
        .materialize_theme(&ThemeDefinitionV1::new(ThemeTokensV1::default()))
        .expect("defaults");
    assert_ne!(
        defaults.theme_materialization_digest(),
        materialized.theme_materialization_digest(),
        "changing tokens and authored styles must change replay identity"
    );
}

#[test]
fn duplicate_authored_palette_targets_fail_without_a_partial_spec() {
    let definition = ThemeDefinitionV1::new(ThemeTokensV1::default()).with_styles(vec![
        ThemeRuleSetWireV1::OrdinalPalette {
            target: "node".to_owned(),
            colors: vec!["#111111".to_owned()],
        },
        ThemeRuleSetWireV1::OrdinalPalette {
            target: "node".to_owned(),
            colors: vec!["#222222".to_owned()],
        },
    ]);

    let error = ThemeMaterializer::new()
        .materialize_theme(&definition)
        .expect_err("duplicate authored palette targets must fail closed");
    assert_eq!(error.code(), "theme-authoring.duplicate-palette-target");
    assert!(matches!(
        error,
        ThemeMaterializationError::DuplicatePaletteTarget {
            ref target,
            first_authored_index: 0,
            duplicate_authored_index: 1,
        } if target == "node"
    ));
}

#[test]
fn authored_rule_budget_accepts_472_and_rejects_473_before_expansion() {
    let authored_rule = ThemeRuleSetWireV1::Rule {
        target: "node".to_owned(),
        family: None,
        variant: None,
        ordinal: None,
        style: ThemeStylePatchWireV1::default(),
    };
    let boundary =
        ThemeDefinitionV1::new(ThemeTokensV1::default())
            .with_styles(vec![authored_rule.clone(); 472]);
    let materialized = ThemeMaterializer::new()
        .materialize_theme(&boundary)
        .expect("40 generated plus 472 authored rules should fit the compiler ceiling");
    assert_eq!(
        materialized
            .spec()
            .styles
            .as_ref()
            .expect("styles")
            .iter()
            .filter(|entry| matches!(entry, ThemeRuleSetWireV1::Rule { .. }))
            .count(),
        512
    );

    let exceeded =
        ThemeDefinitionV1::new(ThemeTokensV1::default()).with_styles(vec![authored_rule; 473]);
    let error = ThemeMaterializer::new()
        .materialize_theme(&exceeded)
        .expect_err("the exact-plus-one authored rule must fail closed");
    assert_eq!(error.code(), "theme-authoring.rule-budget-exceeded");
    assert!(matches!(
        error,
        ThemeMaterializationError::RuleBudgetExceeded {
            actual: 473,
            max: 472,
        }
    ));
}

#[test]
fn concrete_effect_references_are_rejected_but_explicit_clear_is_preserved() {
    let rule_with_effect = |effect| ThemeRuleSetWireV1::Rule {
        target: "node".to_owned(),
        family: None,
        variant: None,
        ordinal: None,
        style: ThemeStylePatchWireV1 {
            effect,
            ..ThemeStylePatchWireV1::default()
        },
    };

    let rejected =
        ThemeDefinitionV1::new(ThemeTokensV1::default()).with_styles(vec![rule_with_effect(
            SpecifiedWireV1::Value("soft-shadow".to_owned()),
        )]);
    let error = ThemeMaterializer::new()
        .materialize_theme(&rejected)
        .expect_err("version one authoring carries no effect graph authority");
    assert_eq!(
        error.code(),
        "theme-authoring.effect-reference-not-supported"
    );
    assert!(matches!(
        error,
        ThemeMaterializationError::EffectReferenceNotSupported { authored_index: 0 }
    ));

    let cleared = ThemeDefinitionV1::new(ThemeTokensV1::default())
        .with_styles(vec![rule_with_effect(SpecifiedWireV1::Clear)]);
    let materialized = ThemeMaterializer::new()
        .materialize_theme(&cleared)
        .expect("clear uses the existing atomic facet and names no graph");
    assert!(matches!(
        &materialized.spec().styles.as_ref().expect("styles")[40],
        ThemeRuleSetWireV1::Rule { style, .. }
            if matches!(style.effect, SpecifiedWireV1::Clear)
    ));
}

#[test]
fn explicitly_empty_series_uses_the_dedicated_authoring_error() {
    let definition = ThemeDefinitionV1::new(ThemeTokensV1::default().with_series(Vec::new()));

    let error = ThemeMaterializer::new()
        .materialize_theme(&definition)
        .expect_err("an explicitly empty series cannot produce ordinal palettes");
    assert_eq!(error.code(), "theme-authoring.empty-series");
    assert!(matches!(error, ThemeMaterializationError::EmptySeries));
}
