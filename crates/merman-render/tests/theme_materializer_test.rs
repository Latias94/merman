use merman_render::diagram_theme::{
    DiagramThemeCompiler, ThemeMaterializationError, ThemeMaterializer,
};
use merman_theme_contract::{
    SpecifiedWireV1, ThemeAuthoringTypographyV1, ThemeCanvasPaintWireV1, ThemeColorTokenV1,
    ThemeDefinitionV1, ThemeLineHeightWireV1, ThemeRuleSetWireV1, ThemeStrokePatchWireV1,
    ThemeStylePatchWireV1, ThemeTokensV1,
};

#[test]
fn tokens_only_definition_materializes_the_complete_version_one_spec() {
    let definition = ThemeDefinitionV1::new(ThemeTokensV1::default());

    let first = ThemeMaterializer::new()
        .materialize_theme(&definition)
        .expect("version one defaults should materialize");

    assert_eq!(first.schema_version(), 1);
    assert_eq!(first.authoring_schema_version(), 1);
    assert_eq!(first.expansion_version(), 1);
    assert_eq!(first.spec_schema_version(), 1);

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
    assert_eq!(styles.len(), 25);
    let generated_targets = styles[..23]
        .iter()
        .map(|entry| match entry {
            ThemeRuleSetWireV1::Rule { target, .. } => target.as_str(),
            ThemeRuleSetWireV1::OrdinalPalette { .. } => {
                panic!("the generated rule tranche must precede palettes")
            }
        })
        .collect::<Vec<_>>();
    assert_eq!(
        generated_targets,
        [
            "text",
            "title",
            "node",
            "edge",
            "cluster",
            "actor",
            "lifeline",
            "message",
            "state",
            "state-label",
            "transition",
            "transition-marker",
            "transition-label",
            "transition-label-background",
            "composite",
            "composite-header",
            "composite-label",
            "special-state",
            "special-state-inner",
            "note",
            "note-label",
            "activation",
            "entity",
        ]
    );
    for target in ["edge", "lifeline", "message", "transition"] {
        assert!(matches!(
            styles[..23].iter().find(|entry| matches!(
                entry,
                ThemeRuleSetWireV1::Rule { target: actual, .. } if actual == target
            )),
            Some(ThemeRuleSetWireV1::Rule { style, .. })
                if matches!(style.fill, SpecifiedWireV1::Unspecified)
                    && matches!(
                        style.stroke.as_ref().map(|stroke| &stroke.paint),
                        Some(SpecifiedWireV1::Value(_))
                    )
        ));
    }
    assert!(matches!(
        &styles[22],
        ThemeRuleSetWireV1::Rule { target, style, .. }
            if target == "entity"
                && matches!(style.fill, SpecifiedWireV1::Value(_))
                && matches!(
                    style.stroke.as_ref().map(|stroke| &stroke.paint),
                    Some(SpecifiedWireV1::Value(_))
                )
    ));
    assert!(matches!(
        &styles[23],
        ThemeRuleSetWireV1::OrdinalPalette { target, colors }
            if target == "node"
                && colors == &["#2563eb", "#16a34a", "#d97706", "#9333ea"]
    ));
    assert!(matches!(
        &styles[24],
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

    assert_eq!(styles.len(), 27);
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
    assert_eq!(&styles[23], &authored_rule);
    assert!(matches!(
        &styles[24],
        ThemeRuleSetWireV1::OrdinalPalette { target, colors }
            if target == "node" && colors == &["#333333", "#444444"]
    ));
    assert!(matches!(
        &styles[25],
        ThemeRuleSetWireV1::OrdinalPalette { target, colors }
            if target == "pie-slice" && colors == &["#111111", "#222222"]
    ));
    assert!(matches!(
        &styles[26],
        ThemeRuleSetWireV1::OrdinalPalette { target, colors }
            if target == "chart-series" && colors == &["#555555"]
    ));
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
fn authored_rule_budget_accepts_489_and_rejects_490_before_expansion() {
    let authored_rule = ThemeRuleSetWireV1::Rule {
        target: "node".to_owned(),
        family: None,
        variant: None,
        ordinal: None,
        style: ThemeStylePatchWireV1::default(),
    };
    let boundary =
        ThemeDefinitionV1::new(ThemeTokensV1::default())
            .with_styles(vec![authored_rule.clone(); 489]);
    let materialized = ThemeMaterializer::new()
        .materialize_theme(&boundary)
        .expect("23 generated plus 489 authored rules should fit the compiler ceiling");
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
        ThemeDefinitionV1::new(ThemeTokensV1::default()).with_styles(vec![authored_rule; 490]);
    let error = ThemeMaterializer::new()
        .materialize_theme(&exceeded)
        .expect_err("the exact-plus-one authored rule must fail closed");
    assert_eq!(error.code(), "theme-authoring.rule-budget-exceeded");
    assert!(matches!(
        error,
        ThemeMaterializationError::RuleBudgetExceeded {
            actual: 490,
            max: 489,
        }
    ));
}

#[test]
fn authored_palette_budget_counts_generated_replacements_before_expansion() {
    let palette = |index: usize| ThemeRuleSetWireV1::OrdinalPalette {
        target: format!("custom-palette-{index}"),
        colors: vec!["#123456".to_owned()],
    };
    let boundary = ThemeDefinitionV1::new(ThemeTokensV1::default())
        .with_styles((0..62).map(palette).collect());
    ThemeMaterializer::new()
        .materialize_theme(&boundary)
        .expect("two generated palettes plus 62 authored palettes should fit");

    let exceeded = ThemeDefinitionV1::new(ThemeTokensV1::default())
        .with_styles((0..63).map(palette).collect());
    let error = ThemeMaterializer::new()
        .materialize_theme(&exceeded)
        .expect_err("the 65th materialized palette must fail before expansion");
    assert!(matches!(
        error,
        ThemeMaterializationError::OrdinalPaletteBudgetExceeded {
            actual: 65,
            max: 64,
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
        &materialized.spec().styles.as_ref().expect("styles")[23],
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

#[test]
fn authored_non_finite_style_numbers_fail_before_materialization_returns() {
    let cases = [
        (
            "radius",
            ThemeStylePatchWireV1 {
                radius: SpecifiedWireV1::Value(f32::NAN),
                ..ThemeStylePatchWireV1::default()
            },
        ),
        (
            "opacity",
            ThemeStylePatchWireV1 {
                opacity: SpecifiedWireV1::Value(f32::INFINITY),
                ..ThemeStylePatchWireV1::default()
            },
        ),
        (
            "stroke.width",
            ThemeStylePatchWireV1 {
                stroke: Some(ThemeStrokePatchWireV1 {
                    width: SpecifiedWireV1::Value(f32::NEG_INFINITY),
                    ..ThemeStrokePatchWireV1::default()
                }),
                ..ThemeStylePatchWireV1::default()
            },
        ),
    ];

    for (facet, style) in cases {
        let definition = ThemeDefinitionV1::new(ThemeTokensV1::default()).with_styles(vec![
            ThemeRuleSetWireV1::Rule {
                target: "node".to_owned(),
                family: None,
                variant: None,
                ordinal: None,
                style,
            },
        ]);

        let error = ThemeMaterializer::new()
            .materialize_theme(&definition)
            .unwrap_err();
        assert_eq!(
            error.code(),
            "theme-authoring.invalid-token-value",
            "{facet}"
        );
        assert!(
            matches!(
                error,
                ThemeMaterializationError::InvalidTokenValue { path: "/styles" }
            ),
            "{facet} must fail through the materialized-wire construction gate"
        );
    }
}
