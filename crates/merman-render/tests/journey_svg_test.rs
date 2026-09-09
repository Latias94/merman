use merman_core::{Engine, MermaidConfig, ParseOptions};
use merman_render::LayoutOptions;
use merman_render::diagram_theme::{
    CanvasPaint, DiagramThemeCompiler, DiagramThemeSpec, OrdinalPalette, OrdinalSelector,
    Specified, ThemeGeometryPatch, ThemePortabilityRequirement, ThemeRule, ThemeRuleSet,
    ThemeStylePatch, ThemeTarget, ThemeTextStyle, ThemeVariant, TypographySpec,
};
use merman_render::environment::RenderEnvironment;
use merman_render::family;
use merman_render::svg::{SvgDebugOptions, SvgRenderOptions};

fn journey_task_paint_theme() -> merman_render::diagram_theme::DiagramTheme {
    journey_task_paint_theme_with_variant(
        None,
        CanvasPaint::solid("#123456").expect("valid Journey task fill"),
        CanvasPaint::solid("#654321").expect("valid Journey task stroke"),
    )
}

fn journey_task_paint_theme_with_variant(
    variant: Option<ThemeVariant>,
    fill: CanvasPaint,
    stroke: CanvasPaint,
) -> merman_render::diagram_theme::DiagramTheme {
    let mut fill_rule = ThemeRule::new(
        ThemeTarget::JourneyTask,
        ThemeStylePatch::default().with_fill(fill),
    );
    let mut stroke_rule = ThemeRule::new(
        ThemeTarget::JourneyTask,
        ThemeStylePatch::default().with_stroke(stroke),
    );
    if let Some(variant) = variant {
        fill_rule = fill_rule.with_variant(variant);
        stroke_rule = stroke_rule.with_variant(variant);
    }
    let styles = ThemeRuleSet::default()
        .with_rule(fill_rule)
        .with_rule(stroke_rule);
    DiagramThemeCompiler::new()
        .compile(DiagramThemeSpec::new().with_styles(styles))
        .expect("compile Journey task paint theme")
}

fn journey_typography_theme(font_size: f32) -> merman_render::diagram_theme::DiagramTheme {
    let typography = ThemeTextStyle::default()
        .with_font_stack(
            merman_render::diagram_theme::FontStack::single("Journey Typed")
                .expect("valid Journey font stack"),
        )
        .with_font_size_px(font_size)
        .expect("valid Journey font size");
    DiagramThemeCompiler::new()
        .compile(
            DiagramThemeSpec::new().with_typography(
                TypographySpec::default()
                    .with_family_style(merman_render::DiagramFamilyId::JOURNEY, typography),
            ),
        )
        .expect("compile Journey typography theme")
}

#[test]
fn journey_text_fill_has_portable_terminal_evidence() {
    for variant in [None, Some(ThemeVariant::Default)] {
        for paint in [
            CanvasPaint::solid("#123456").unwrap(),
            CanvasPaint::Transparent,
        ] {
            let mut rule = ThemeRule::new(
                ThemeTarget::Text,
                ThemeStylePatch::default().with_fill(paint),
            );
            if let Some(variant) = variant {
                rule = rule.with_variant(variant);
            }
            let theme = DiagramThemeCompiler::new()
                .compile(
                    DiagramThemeSpec::new().with_styles(ThemeRuleSet::default().with_rule(rule)),
                )
                .expect("compile Journey Text.fill");
            let parsed = merman_render::__private::install_parse_compatibility(
                &theme,
                Engine::new(),
            )
            .parse_diagram_for_render_model_sync(
                "journey\n  title Release\n  section Delivery\n    Ship release: 5: Maintainer\n",
                ParseOptions::strict(),
            )
            .expect("parse Journey")
            .expect("detect Journey");
            let session = RenderEnvironment::deterministic()
                .with_theme_portability_requirement(ThemePortabilityRequirement::RequirePortable)
                .begin_session_with_theme(&theme)
                .expect("start portable Journey session");
            let artifact =
                family::prepare(parsed, &LayoutOptions::headless_svg_defaults(), session)
                    .expect("prepare Journey");
            let rendered = artifact
                .render_svg(&SvgRenderOptions::default(), &SvgDebugOptions::default())
                .expect("Journey Text.fill has direct portable evidence");
            roxmltree::Document::parse(rendered.svg()).expect("valid Journey SVG");
            let completion = rendered.into_completion();
            let evidence = merman_render::__private::family_evidence(completion.report());
            assert_eq!(evidence.required_count(), 1);
            assert_eq!(evidence.accounted_count(), 1);
            assert_eq!(evidence.applied_count(), 1);
            assert_eq!(evidence.theme_residual_count(), 0);
            assert_eq!(evidence.compatibility_residual_count(), 0);
        }
    }
}

fn render_journey_text_rules(
    source: &str,
    styles: ThemeRuleSet,
    config: serde_json::Value,
) -> merman_render::Result<family::RenderedFamilySvg> {
    let theme = DiagramThemeCompiler::new()
        .compile(DiagramThemeSpec::new().with_styles(styles))
        .expect("compile Journey rules");
    let parsed = merman_render::__private::install_parse_compatibility(
        &theme,
        Engine::new().with_site_config(MermaidConfig::from_value(config)),
    )
    .parse_diagram_for_render_model_sync(source, ParseOptions::strict())
    .expect("parse Journey")
    .expect("detect Journey");
    let session = RenderEnvironment::deterministic()
        .with_theme_portability_requirement(ThemePortabilityRequirement::RequirePortable)
        .begin_session_with_theme(&theme)
        .expect("begin Journey session");
    family::prepare(parsed, &LayoutOptions::headless_svg_defaults(), session)?
        .render_svg(&SvgRenderOptions::default(), &SvgDebugOptions::default())
}

#[test]
fn journey_text_fill_preserves_unsectioned_tasks_and_empty_diagram_lines() {
    for (source, tasks) in [
        ("journey\n", 0),
        ("journey\n  title Release\n", 0),
        ("journey\n  Unsectioned: 5: Actor\n", 1),
        (
            "journey\n  Unsectioned: 5: Actor\n  section Delivery\n  Named: 3: Actor\n",
            2,
        ),
    ] {
        let styles = ThemeRuleSet::default().with_rule(ThemeRule::new(
            ThemeTarget::Text,
            ThemeStylePatch::default().with_fill(CanvasPaint::solid("#123456").unwrap()),
        ));
        let rendered = render_journey_text_rules(source, styles, serde_json::json!({}))
            .unwrap_or_else(|error| panic!("{source}: {error}"));
        let document = roxmltree::Document::parse(rendered.svg()).expect("valid Journey SVG");
        let task_count = document
            .descendants()
            .filter(|node| {
                node.has_tag_name("text")
                    && node.attribute("class").is_some_and(|classes| {
                        classes.split_whitespace().any(|class| class == "task")
                    })
            })
            .count();
        assert_eq!(task_count, tasks, "{source}");
        let lines = document
            .descendants()
            .filter(|node| node.has_tag_name("line") && node.attribute("class") != Some("mouth"))
            .count();
        assert_eq!(lines, tasks + 1, "task lines and persistent activity line");
        let completion = rendered.into_completion();
        let evidence = merman_render::__private::family_evidence(completion.report());
        assert_eq!(evidence.applied_count(), 1);
        assert_eq!(evidence.compatibility_residual_count(), 0);
    }
}

#[test]
fn journey_text_fill_does_not_certify_unsupported_siblings() {
    for config in [
        serde_json::json!({}),
        serde_json::json!({"themeVariables": {"textColor": "#abcdef"}}),
    ] {
        for font_sibling in [false, true] {
            let mut patch =
                ThemeStylePatch::default().with_fill(CanvasPaint::solid("#123456").unwrap());
            if font_sibling {
                patch.typography.font_size_px = Specified::Value(40.0);
            } else {
                patch = patch.with_stroke(CanvasPaint::solid("#654321").unwrap());
            }
            let styles =
                ThemeRuleSet::default().with_rule(ThemeRule::new(ThemeTarget::Text, patch));
            let error = render_journey_text_rules(
                "journey\n  section Delivery\n  Ship: 5: Actor\n",
                styles,
                config.clone(),
            )
            .err()
            .expect("typed fill must not certify an unsupported sibling");
            assert!(
                matches!(error, merman_render::Error::UnverifiedFamilyTheme { .. }),
                "{error}"
            );
        }
    }
}

#[test]
fn journey_text_fill_config_owner_and_shadowed_palette_are_accounted() {
    for config_owned in [false, true] {
        let styles = ThemeRuleSet::default()
            .with_ordinal_palette(
                ThemeTarget::Text,
                OrdinalPalette::new([merman_render::diagram_theme::ThemeColorValue::parse(
                    "#fedcba",
                )
                .unwrap()])
                .unwrap(),
            )
            .with_rule(ThemeRule::new(
                ThemeTarget::Text,
                ThemeStylePatch::default().with_fill(CanvasPaint::solid("#123456").unwrap()),
            ));
        let config = if config_owned {
            serde_json::json!({"themeVariables": {"textColor": "#abcdef"}})
        } else {
            serde_json::json!({})
        };
        let rendered = render_journey_text_rules(
            "journey\n  section Delivery\n  Ship: 5: Actor\n",
            styles,
            config,
        )
        .expect("source owner and inactive palette are portable");
        let completion = rendered.into_completion();
        let evidence = merman_render::__private::family_evidence(completion.report());
        assert_eq!(evidence.required_count(), 2);
        assert_eq!(evidence.accounted_count(), 2);
        assert_eq!(evidence.applied_count(), usize::from(!config_owned));
        assert_eq!(
            evidence.not_applicable_count(),
            1 + usize::from(config_owned)
        );
        assert_eq!(evidence.theme_residual_count(), 0);
        assert_eq!(evidence.compatibility_residual_count(), 0);
    }
}

#[test]
fn journey_text_paint_preserves_labels_lines_and_title_local_ownership() {
    for variant in [None, Some(ThemeVariant::Default)] {
        for (paint, expected_paint) in [("#123456", "#123456"), ("transparent", "#00000000")] {
            let mut rule = ThemeRule::new(
                ThemeTarget::Text,
                ThemeStylePatch::default()
                    .with_fill(CanvasPaint::solid(paint).expect("valid text paint")),
            );
            if let Some(variant) = variant {
                rule = rule.with_variant(variant);
            }
            let theme = DiagramThemeCompiler::new()
                .compile(DiagramThemeSpec::new().with_styles(
                    ThemeRuleSet::default().with_rule(rule).with_rule(
                        ThemeRule::new(
                            ThemeTarget::Title,
                            ThemeStylePatch::default().with_fill(
                                CanvasPaint::solid("#987654").expect("valid title paint"),
                            ),
                        ),
                    ),
                ))
                .expect("compile Journey text paint");
            for (config, expected_text, expected_title) in [
                (serde_json::json!({}), expected_paint, ""),
                (
                    serde_json::json!({"themeVariables": {"textColor": "#abcdef"}}),
                    "#abcdef",
                    "",
                ),
                (
                    serde_json::json!({"journey": {"titleColor": "#fedcba"}}),
                    expected_paint,
                    "#fedcba",
                ),
            ] {
                let parsed = merman_render::__private::install_parse_compatibility(
                    &theme,
                    Engine::new().with_site_config(MermaidConfig::from_value(config)),
                )
                .parse_diagram_for_render_model_sync(
                    "journey\n  title Release\n  section Delivery\n    Ship release: 5: Maintainer\n",
                    ParseOptions::strict(),
                )
                .expect("parse Journey text paint fixture")
                .expect("detect Journey");
                let session = RenderEnvironment::deterministic()
                    .begin_session_with_theme(&theme)
                    .expect("begin Journey session");
                let artifact =
                    family::prepare(parsed, &LayoutOptions::headless_svg_defaults(), session)
                        .expect("prepare Journey text paint fixture");
                let rendered = artifact
                    .render_svg(
                        &SvgRenderOptions {
                            diagram_id: Some("journey-text-paint".to_owned()),
                            ..SvgRenderOptions::default()
                        },
                        &SvgDebugOptions::default(),
                    )
                    .expect("render Journey text paint fixture");
                let document =
                    roxmltree::Document::parse(rendered.svg()).expect("valid Journey SVG");
                let css = document
                    .descendants()
                    .find(|node| node.has_tag_name("style"))
                    .and_then(|node| node.text())
                    .expect("Journey stylesheet");
                let root_rule = css.split_once('}').expect("complete root CSS rule").0;
                assert!(root_rule.starts_with("#journey-text-paint{"));
                assert!(
                    root_rule.ends_with(&format!(";fill:{expected_text};")),
                    "root rule: {root_rule}"
                );
                assert!(css.contains(&format!(
                    "#journey-text-paint line{{stroke:{expected_text};}}"
                )));
                assert!(css.contains(&format!(
                    "#journey-text-paint .legend{{fill:{expected_text};"
                )));
                assert!(css.contains(&format!(
                    "#journey-text-paint .label{{color:{expected_text};}}"
                )));
                // The legacy Title projection only styles nonexistent cluster text. It does
                // not own the diagram title, which inherits Text unless journey.titleColor wins.
                assert!(css.contains("#journey-text-paint .cluster text{fill:#987654;}"));
                assert!(!document.descendants().any(|node| {
                    node.attribute("class").is_some_and(|classes| {
                        classes.split_whitespace().any(|class| class == "cluster")
                    })
                }));

                for (class, expected_label) in [
                    ("task", "    Ship release"),
                    ("journey-section", "Delivery"),
                ] {
                    let text = document
                        .descendants()
                        .find(|node| {
                            node.has_tag_name("text")
                                && node.attribute("class").is_some_and(|classes| {
                                    classes.split_whitespace().any(|item| item == class)
                                })
                        })
                        .expect("native Journey text terminal");
                    assert_eq!(
                        text.descendants()
                            .find(|node| node.is_text())
                            .and_then(|node| node.text()),
                        Some(expected_label)
                    );
                    assert!(
                        text.attribute("style")
                            .expect("native text style")
                            .ends_with(&format!("fill: {expected_text};"))
                    );
                    let html = document
                        .descendants()
                        .find(|node| {
                            node.has_tag_name("div")
                                && node.attribute("class").is_some_and(|classes| {
                                    classes.split_whitespace().any(|item| item == class)
                                })
                        })
                        .expect("HTML Journey text terminal");
                    let label = html
                        .children()
                        .find(|node| node.attribute("class") == Some("label"))
                        .expect("HTML label uses the color rule");
                    assert_eq!(label.text(), Some(expected_label));
                }
                let legend = document
                    .descendants()
                    .find(|node| {
                        node.has_tag_name("text") && node.attribute("class") == Some("legend")
                    })
                    .expect("actor legend terminal");
                assert_eq!(
                    legend
                        .descendants()
                        .find(|node| node.is_text())
                        .and_then(|node| node.text()),
                    Some("Maintainer")
                );
                let title = document
                    .descendants()
                    .find(|node| node.has_tag_name("text") && node.text() == Some("Release"))
                    .expect("diagram title terminal");
                assert_eq!(title.attribute("fill"), Some(expected_title));
                assert_eq!(
                    document
                        .descendants()
                        .filter(|node| node.has_tag_name("line"))
                        .count(),
                    2,
                    "task and activity lines consume the line rule"
                );
            }
        }
    }
}

#[test]
fn journey_task_radius_reaches_terminal_svg_without_changing_sections() {
    for (radius, expected_task_radius) in [(Specified::Value(9.0), "9"), (Specified::Clear, "3")] {
        let styles = ThemeRuleSet::default()
            .with_rule(ThemeRule::new(
                ThemeTarget::JourneyTask,
                ThemeStylePatch {
                    geometry: ThemeGeometryPatch { radius },
                    ..ThemeStylePatch::default()
                },
            ))
            .with_rule(
                ThemeRule::new(
                    ThemeTarget::JourneyTask,
                    ThemeStylePatch {
                        geometry: ThemeGeometryPatch {
                            radius: Specified::Value(99.0),
                        },
                        ..ThemeStylePatch::default()
                    },
                )
                .with_variant(ThemeVariant::Warning)
                .with_ordinal(
                    OrdinalSelector::exact(999).expect("valid out-of-range Journey ordinal"),
                ),
            );
        let theme = DiagramThemeCompiler::new()
            .compile(DiagramThemeSpec::new().with_styles(styles))
            .expect("compile Journey task radius theme");
        let parsed = merman_render::__private::install_parse_compatibility(&theme, Engine::new())
            .parse_diagram_for_render_model_sync(
                "journey\n  section Delivery\n    Ship release: 5: Maintainer\n",
                ParseOptions::strict(),
            )
            .expect("parse themed Journey")
            .expect("detect themed Journey");
        let session = RenderEnvironment::deterministic()
            .with_theme_portability_requirement(ThemePortabilityRequirement::RequirePortable)
            .begin_session_with_theme(&theme)
            .expect("begin strict portable Journey session");
        let artifact = family::prepare(parsed, &LayoutOptions::headless_svg_defaults(), session)
            .expect("prepare themed Journey");
        let svg = artifact
            .render_svg(
                &SvgRenderOptions {
                    diagram_id: Some("journey-task-radius".to_string()),
                    ..SvgRenderOptions::default()
                },
                &SvgDebugOptions::default(),
            )
            .expect("render themed Journey SVG")
            .svg()
            .to_owned();
        let document = roxmltree::Document::parse(&svg).expect("valid themed Journey SVG XML");
        let task = document
            .descendants()
            .find(|node| {
                node.has_tag_name("rect")
                    && node.attribute("class").is_some_and(|class| {
                        class.split_ascii_whitespace().any(|token| token == "task")
                    })
            })
            .expect("Journey task rect");
        let section = document
            .descendants()
            .find(|node| {
                node.has_tag_name("rect")
                    && node.attribute("class").is_some_and(|class| {
                        class
                            .split_ascii_whitespace()
                            .any(|token| token == "journey-section")
                    })
            })
            .expect("Journey section rect");

        assert_eq!(task.attribute("rx"), Some(expected_task_radius));
        assert_eq!(task.attribute("ry"), Some(expected_task_radius));
        assert_eq!(section.attribute("rx"), Some("3"));
        assert_eq!(section.attribute("ry"), Some("3"));
    }
}

#[test]
fn journey_task_static_paint_reaches_terminal_rect_and_is_fully_accounted() {
    let theme = journey_task_paint_theme();
    let parsed = merman_render::__private::install_parse_compatibility(&theme, Engine::new())
        .parse_diagram_for_render_model_sync(
            "journey\n  section Delivery\n    Ship release: 5: Maintainer\n",
            ParseOptions::strict(),
        )
        .expect("parse themed Journey")
        .expect("detect themed Journey");
    let session = RenderEnvironment::deterministic()
        .with_theme_portability_requirement(ThemePortabilityRequirement::RequirePortable)
        .begin_session_with_theme(&theme)
        .expect("begin strict portable Journey session");
    let artifact = family::prepare(parsed, &LayoutOptions::headless_svg_defaults(), session)
        .expect("prepare themed Journey");
    let rendered = artifact
        .render_svg(
            &SvgRenderOptions {
                diagram_id: Some("journey-task-paint".to_string()),
                ..SvgRenderOptions::default()
            },
            &SvgDebugOptions::default(),
        )
        .expect("render themed Journey SVG");
    let document = roxmltree::Document::parse(rendered.svg()).expect("valid Journey SVG XML");
    let task = document
        .descendants()
        .find(|node| {
            node.has_tag_name("rect")
                && node.attribute("class").is_some_and(|class| {
                    class.split_ascii_whitespace().any(|token| token == "task")
                })
        })
        .expect("Journey task rect");
    let style = task.attribute("style").expect("typed task paint style");
    assert!(style.contains("fill:#123456;"), "task style: {style}");
    assert!(style.contains("stroke:#654321;"), "task style: {style}");

    let evidence = merman_render::__private::family_evidence(rendered.into_completion().report());
    assert_eq!(evidence.required_count(), 2);
    assert_eq!(evidence.accounted_count(), 2);
    assert_eq!(evidence.applied_count(), 2);
    assert_eq!(evidence.theme_residual_count(), 0);
    assert_eq!(evidence.compatibility_residual_count(), 0);
}

#[test]
fn journey_task_explicit_default_paint_reaches_terminal_rect_and_is_fully_accounted() {
    for (fill, stroke, expected_fill, expected_stroke) in [
        (
            CanvasPaint::solid("#123456").expect("valid Journey task fill"),
            CanvasPaint::solid("#654321").expect("valid Journey task stroke"),
            "#123456",
            "#654321",
        ),
        (
            CanvasPaint::Transparent,
            CanvasPaint::Transparent,
            "transparent",
            "transparent",
        ),
    ] {
        let theme =
            journey_task_paint_theme_with_variant(Some(ThemeVariant::Default), fill, stroke);
        let parsed = merman_render::__private::install_parse_compatibility(&theme, Engine::new())
            .parse_diagram_for_render_model_sync(
                "journey\n  section Delivery\n    Ship release: 5: Maintainer\n",
                ParseOptions::strict(),
            )
            .expect("parse explicitly default-themed Journey")
            .expect("detect explicitly default-themed Journey");
        let session = RenderEnvironment::deterministic()
            .with_theme_portability_requirement(ThemePortabilityRequirement::RequirePortable)
            .begin_session_with_theme(&theme)
            .expect("begin strict portable Journey session");
        let artifact = family::prepare(parsed, &LayoutOptions::headless_svg_defaults(), session)
            .expect("prepare explicitly default-themed Journey");
        let rendered = artifact
            .render_svg(
                &SvgRenderOptions {
                    diagram_id: Some("journey-task-default-paint".to_string()),
                    ..SvgRenderOptions::default()
                },
                &SvgDebugOptions::default(),
            )
            .expect("render explicitly default-themed Journey SVG");
        let document = roxmltree::Document::parse(rendered.svg()).expect("valid Journey SVG XML");
        let task = document
            .descendants()
            .find(|node| {
                node.has_tag_name("rect")
                    && node.attribute("class").is_some_and(|class| {
                        class.split_ascii_whitespace().any(|token| token == "task")
                    })
            })
            .expect("Journey task rect");
        let style = task.attribute("style").expect("typed task paint style");
        assert!(
            style.contains(&format!("fill:{expected_fill};")),
            "task style: {style}"
        );
        assert!(
            style.contains(&format!("stroke:{expected_stroke};")),
            "task style: {style}"
        );

        let evidence =
            merman_render::__private::family_evidence(rendered.into_completion().report());
        assert_eq!(evidence.required_count(), 2);
        assert_eq!(evidence.accounted_count(), 2);
        assert_eq!(evidence.applied_count(), 2);
        assert_eq!(evidence.theme_residual_count(), 0);
        assert_eq!(evidence.compatibility_residual_count(), 0);
    }
}

#[test]
fn journey_task_explicit_default_paint_is_not_applicable_without_tasks() {
    let theme = journey_task_paint_theme_with_variant(
        Some(ThemeVariant::Default),
        CanvasPaint::solid("#123456").expect("valid Journey task fill"),
        CanvasPaint::solid("#654321").expect("valid Journey task stroke"),
    );
    let parsed = merman_render::__private::install_parse_compatibility(&theme, Engine::new())
        .parse_diagram_for_render_model_sync("journey\n", ParseOptions::strict())
        .expect("parse empty Journey")
        .expect("detect empty Journey");
    let session = RenderEnvironment::deterministic()
        .with_theme_portability_requirement(ThemePortabilityRequirement::RequirePortable)
        .begin_session_with_theme(&theme)
        .expect("begin strict portable empty Journey session");
    let artifact = family::prepare(parsed, &LayoutOptions::headless_svg_defaults(), session)
        .expect("prepare empty Journey");
    let rendered = artifact
        .render_svg(
            &SvgRenderOptions {
                diagram_id: Some("journey-task-default-paint-empty".to_string()),
                ..SvgRenderOptions::default()
            },
            &SvgDebugOptions::default(),
        )
        .expect("render empty Journey SVG");
    let evidence = merman_render::__private::family_evidence(rendered.into_completion().report());
    assert_eq!(evidence.required_count(), 2);
    assert_eq!(evidence.accounted_count(), 2);
    assert_eq!(evidence.not_applicable_count(), 2);
    assert_eq!(evidence.theme_residual_count(), 0);
    assert_eq!(evidence.compatibility_residual_count(), 0);
}

#[test]
fn journey_base_typography_drives_scoped_css_and_is_fully_accounted() {
    let theme = journey_typography_theme(21.0);
    let parsed = merman_render::__private::install_parse_compatibility(&theme, Engine::new())
        .parse_diagram_for_render_model_sync(
            "journey\n  section Delivery\n    Ship release: 5: Maintainer\n",
            ParseOptions::strict(),
        )
        .expect("parse themed Journey")
        .expect("detect themed Journey");
    let session = RenderEnvironment::deterministic()
        .with_theme_portability_requirement(ThemePortabilityRequirement::RequirePortable)
        .begin_session_with_theme(&theme)
        .expect("begin strict portable Journey session");
    let artifact = family::prepare(parsed, &LayoutOptions::headless_svg_defaults(), session)
        .expect("prepare themed Journey");
    let rendered = artifact
        .render_svg(
            &SvgRenderOptions {
                diagram_id: Some("journey-base-typography".to_string()),
                ..SvgRenderOptions::default()
            },
            &SvgDebugOptions::default(),
        )
        .expect("render themed Journey SVG");

    let svg = rendered.svg();
    assert!(
        svg.contains("#journey-base-typography .legend"),
        "Journey actor legend selector missing from SVG CSS"
    );
    assert!(
        svg.contains("font-family:\"Journey Typed\""),
        "typed Journey font family missing from SVG CSS: {svg}"
    );
    assert!(
        svg.contains("font-size:21px"),
        "typed Journey font size missing from SVG CSS: {svg}"
    );

    let document = roxmltree::Document::parse(svg).expect("valid Journey SVG");
    assert!(
        document.descendants().any(|node| {
            node.has_tag_name("text")
                && node.attribute("class").is_some_and(|class| {
                    class
                        .split_ascii_whitespace()
                        .any(|token| token == "legend")
                })
        }),
        "typed Journey actor legend terminal missing"
    );

    let evidence = merman_render::__private::family_evidence(rendered.into_completion().report());
    assert_eq!(evidence.required_count(), 2);
    assert_eq!(evidence.accounted_count(), 2);
    assert_eq!(evidence.applied_count(), 2);
    assert_eq!(evidence.not_applicable_count(), 0);
    assert_eq!(evidence.theme_residual_count(), 0);
    assert_eq!(evidence.compatibility_residual_count(), 0);
}

#[test]
fn journey_explicit_theme_font_size_owns_only_the_size_route() {
    let theme = journey_typography_theme(21.0);
    let parsed = merman_render::__private::install_parse_compatibility(
        &theme,
        Engine::new().with_site_config(MermaidConfig::from_value(serde_json::json!({
            "themeVariables": { "fontSize": "24px" }
        }))),
    )
    .parse_diagram_for_render_model_sync(
        "journey\n  section Delivery\n    Ship release: 5: Maintainer\n",
        ParseOptions::strict(),
    )
    .expect("parse configured Journey")
    .expect("detect configured Journey");
    let session = RenderEnvironment::deterministic()
        .with_theme_portability_requirement(ThemePortabilityRequirement::RequirePortable)
        .begin_session_with_theme(&theme)
        .expect("begin strict portable Journey session");
    let artifact = family::prepare(parsed, &LayoutOptions::headless_svg_defaults(), session)
        .expect("prepare configured Journey");
    let rendered = artifact
        .render_svg(
            &SvgRenderOptions {
                diagram_id: Some("journey-configured-font-size".to_string()),
                ..SvgRenderOptions::default()
            },
            &SvgDebugOptions::default(),
        )
        .expect("render configured Journey SVG");

    assert!(rendered.svg().contains("font-family:\"Journey Typed\""));
    assert!(rendered.svg().contains("font-size:24px"));
    let evidence = merman_render::__private::family_evidence(rendered.into_completion().report());
    assert_eq!(evidence.required_count(), 2);
    assert_eq!(evidence.accounted_count(), 2);
    assert_eq!(evidence.applied_count(), 1);
    assert_eq!(evidence.not_applicable_count(), 1);
    assert_eq!(evidence.theme_residual_count(), 0);
    assert_eq!(evidence.compatibility_residual_count(), 0);
}

#[test]
fn journey_task_palette_reaches_section_and_task_terminals_without_legacy_projection() {
    let palette = OrdinalPalette::new([
        merman_render::diagram_theme::ThemeColorValue::parse("#123456")
            .expect("valid first Journey palette color"),
        merman_render::diagram_theme::ThemeColorValue::parse("#654321")
            .expect("valid second Journey palette color"),
    ])
    .expect("valid Journey task palette");
    let theme = DiagramThemeCompiler::new()
        .compile(DiagramThemeSpec::new().with_styles(
            ThemeRuleSet::default().with_ordinal_palette(ThemeTarget::JourneyTask, palette),
        ))
        .expect("compile Journey task palette theme");
    let parsed = merman_render::__private::install_parse_compatibility(&theme, Engine::new())
        .parse_diagram_for_render_model_sync(
            "journey\n  section Delivery\n    Ship release: 5: Maintainer\n  section Verify\n    Verify release: 4: Maintainer\n",
            ParseOptions::strict(),
        )
        .expect("parse themed Journey")
        .expect("detect themed Journey");
    let session = RenderEnvironment::deterministic()
        .with_theme_portability_requirement(ThemePortabilityRequirement::RequirePortable)
        .begin_session_with_theme(&theme)
        .expect("begin strict portable Journey session");
    let artifact = family::prepare(parsed, &LayoutOptions::headless_svg_defaults(), session)
        .expect("prepare themed Journey");
    let rendered = artifact
        .render_svg(
            &SvgRenderOptions {
                diagram_id: Some("journey-task-palette".to_string()),
                ..SvgRenderOptions::default()
            },
            &SvgDebugOptions::default(),
        )
        .expect("render themed Journey SVG");
    let document = roxmltree::Document::parse(rendered.svg()).expect("valid Journey SVG XML");
    let mut sections = document.descendants().filter(|node| {
        node.has_tag_name("rect")
            && node.attribute("class").is_some_and(|class| {
                class
                    .split_ascii_whitespace()
                    .any(|token| token == "journey-section")
            })
    });
    let mut tasks = document.descendants().filter(|node| {
        node.has_tag_name("rect")
            && node
                .attribute("class")
                .is_some_and(|class| class.split_ascii_whitespace().any(|token| token == "task"))
    });
    for expected in ["#123456", "#654321"] {
        let section = sections.next().expect("Journey section rect");
        let task = tasks.next().expect("Journey task rect");
        let expected_style = format!("fill:{expected};");
        assert_eq!(section.attribute("style"), Some(expected_style.as_str()));
        assert_eq!(
            task.attribute("style")
                .map(|style| style.split("stroke:").next().unwrap_or(style)),
            Some(expected_style.as_str())
        );
    }

    let evidence = merman_render::__private::family_evidence(rendered.into_completion().report());
    assert_eq!(evidence.required_count(), 1);
    assert_eq!(evidence.accounted_count(), 1);
    assert_eq!(evidence.applied_count(), 1);
    assert_eq!(evidence.theme_residual_count(), 0);
    assert_eq!(evidence.compatibility_residual_count(), 0);
}

#[test]
fn journey_task_palette_preserves_source_owned_slots_and_counts_typed_slots() {
    let palette = OrdinalPalette::new([
        merman_render::diagram_theme::ThemeColorValue::parse("#123456")
            .expect("valid first Journey palette color"),
        merman_render::diagram_theme::ThemeColorValue::parse("#654321")
            .expect("valid second Journey palette color"),
    ])
    .expect("valid Journey task palette");
    let theme = DiagramThemeCompiler::new()
        .compile(DiagramThemeSpec::new().with_styles(
            ThemeRuleSet::default().with_ordinal_palette(ThemeTarget::JourneyTask, palette),
        ))
        .expect("compile Journey task palette theme");
    let parsed = merman_render::__private::install_parse_compatibility(
        &theme,
        Engine::new().with_site_config(MermaidConfig::from_value(serde_json::json!({
            "secure": []
        }))),
    )
    .parse_diagram_for_render_model_sync(
        r##"%%{init: {"themeVariables": {"fillType0": "#f59e0b"}}}%%
journey
  section Delivery
    Ship release: 5: Maintainer
  section Verify
    Verify release: 4: Maintainer
"##,
        ParseOptions::strict(),
    )
    .expect("parse themed Journey")
    .expect("detect themed Journey");
    let session = RenderEnvironment::deterministic()
        .with_theme_portability_requirement(ThemePortabilityRequirement::RequirePortable)
        .begin_session_with_theme(&theme)
        .expect("begin strict portable Journey session");
    let artifact = family::prepare(parsed, &LayoutOptions::headless_svg_defaults(), session)
        .expect("prepare themed Journey");
    let rendered = artifact
        .render_svg(
            &SvgRenderOptions {
                diagram_id: Some("journey-task-palette-source".to_string()),
                ..SvgRenderOptions::default()
            },
            &SvgDebugOptions::default(),
        )
        .expect("render themed Journey SVG");
    let document = roxmltree::Document::parse(rendered.svg()).expect("valid Journey SVG XML");
    let section_fills = document
        .descendants()
        .filter(|node| {
            node.has_tag_name("rect")
                && node.attribute("class").is_some_and(|class| {
                    class
                        .split_ascii_whitespace()
                        .any(|token| token == "journey-section")
                })
        })
        .map(|node| node.attribute("style").unwrap_or_default().to_string())
        .collect::<Vec<_>>();
    let task_fills = document
        .descendants()
        .filter(|node| {
            node.has_tag_name("rect")
                && node.attribute("class").is_some_and(|class| {
                    class.split_ascii_whitespace().any(|token| token == "task")
                })
        })
        .map(|node| node.attribute("style").unwrap_or_default().to_string())
        .collect::<Vec<_>>();
    assert_eq!(section_fills, ["fill:#f59e0b;", "fill:#654321;"]);
    assert_eq!(task_fills, ["fill:#f59e0b;", "fill:#654321;"]);
    assert!(
        !rendered
            .svg()
            .contains("#journey-task-palette-source .task-type-0")
            && !rendered
                .svg()
                .contains("#journey-task-palette-source .section-type-0")
    );

    let evidence = merman_render::__private::family_evidence(rendered.into_completion().report());
    assert_eq!(evidence.required_count(), 1);
    assert_eq!(evidence.accounted_count(), 1);
    assert_eq!(evidence.applied_count(), 1);
    assert_eq!(evidence.not_applicable_count(), 0);
    assert_eq!(evidence.theme_residual_count(), 0);
    assert_eq!(evidence.compatibility_residual_count(), 0);
}

#[test]
fn journey_task_transparent_palette_reaches_both_terminals() {
    let palette = OrdinalPalette::new([
        merman_render::diagram_theme::ThemeColorValue::parse("transparent")
            .expect("valid transparent Journey palette color"),
        merman_render::diagram_theme::ThemeColorValue::parse("#654321")
            .expect("valid solid Journey palette color"),
    ])
    .expect("valid Journey task palette");
    let theme = DiagramThemeCompiler::new()
        .compile(DiagramThemeSpec::new().with_styles(
            ThemeRuleSet::default().with_ordinal_palette(ThemeTarget::JourneyTask, palette),
        ))
        .expect("compile Journey task palette theme");
    let parsed = merman_render::__private::install_parse_compatibility(&theme, Engine::new())
        .parse_diagram_for_render_model_sync(
            "journey\n  section Delivery\n    Ship release: 5: Maintainer\n  section Verify\n    Verify release: 4: Maintainer\n",
            ParseOptions::strict(),
        )
        .expect("parse themed Journey")
        .expect("detect themed Journey");
    let session = RenderEnvironment::deterministic()
        .with_theme_portability_requirement(ThemePortabilityRequirement::RequirePortable)
        .begin_session_with_theme(&theme)
        .expect("begin strict portable Journey session");
    let artifact = family::prepare(parsed, &LayoutOptions::headless_svg_defaults(), session)
        .expect("prepare themed Journey");
    let rendered = artifact
        .render_svg(
            &SvgRenderOptions {
                diagram_id: Some("journey-task-palette-transparent".to_string()),
                ..SvgRenderOptions::default()
            },
            &SvgDebugOptions::default(),
        )
        .expect("render themed Journey SVG");
    let document = roxmltree::Document::parse(rendered.svg()).expect("valid Journey SVG XML");
    let section = document
        .descendants()
        .find(|node| {
            node.has_tag_name("rect")
                && node.attribute("class").is_some_and(|class| {
                    class
                        .split_ascii_whitespace()
                        .any(|token| token == "journey-section")
                })
        })
        .expect("first Journey section rect");
    let task = document
        .descendants()
        .find(|node| {
            node.has_tag_name("rect")
                && node.attribute("class").is_some_and(|class| {
                    class.split_ascii_whitespace().any(|token| token == "task")
                })
        })
        .expect("first Journey task rect");
    assert_eq!(section.attribute("style"), Some("fill:#00000000;"));
    assert_eq!(
        task.attribute("style")
            .and_then(|style| style.split("stroke:").next()),
        Some("fill:#00000000;")
    );
    let evidence = merman_render::__private::family_evidence(rendered.into_completion().report());
    assert_eq!(evidence.required_count(), 1);
    assert_eq!(evidence.accounted_count(), 1);
    assert_eq!(evidence.applied_count(), 1);
    assert_eq!(evidence.theme_residual_count(), 0);
    assert_eq!(evidence.compatibility_residual_count(), 0);
}

#[test]
fn journey_task_palette_preserves_source_owned_fill_type_slots() {
    let palette = OrdinalPalette::new([
        merman_render::diagram_theme::ThemeColorValue::parse("#123456")
            .expect("valid first Journey palette color"),
        merman_render::diagram_theme::ThemeColorValue::parse("#654321")
            .expect("valid second Journey palette color"),
    ])
    .expect("valid Journey task palette");
    let theme = DiagramThemeCompiler::new()
        .compile(DiagramThemeSpec::new().with_styles(
            ThemeRuleSet::default().with_ordinal_palette(ThemeTarget::JourneyTask, palette),
        ))
        .expect("compile Journey task palette theme");
    let parsed = merman_render::__private::install_parse_compatibility(
        &theme,
        Engine::new().with_site_config(MermaidConfig::from_value(serde_json::json!({
            "secure": []
        }))),
    )
        .parse_diagram_for_render_model_sync(
            "%%{init: {\"themeVariables\": {\"fillType0\": \"#abcdef\"}}}%%\njourney\n  section Delivery\n    Ship release: 5: Maintainer\n  section Verify\n    Verify release: 4: Maintainer\n",
            ParseOptions::strict(),
        )
        .expect("parse source-owned Journey")
        .expect("detect source-owned Journey");
    let session = RenderEnvironment::deterministic()
        .with_theme_portability_requirement(ThemePortabilityRequirement::RequirePortable)
        .begin_session_with_theme(&theme)
        .expect("begin strict portable Journey session");
    let artifact = family::prepare(parsed, &LayoutOptions::headless_svg_defaults(), session)
        .expect("prepare source-owned Journey");
    let rendered = artifact
        .render_svg(
            &SvgRenderOptions {
                diagram_id: Some("journey-source-owned-palette".to_string()),
                ..SvgRenderOptions::default()
            },
            &SvgDebugOptions::default(),
        )
        .expect("render source-owned Journey SVG");
    let document = roxmltree::Document::parse(rendered.svg()).expect("valid Journey SVG XML");
    let section = document
        .descendants()
        .find(|node| {
            node.has_tag_name("rect")
                && node.attribute("class").is_some_and(|class| {
                    class
                        .split_ascii_whitespace()
                        .any(|token| token == "journey-section")
                })
        })
        .expect("Journey source-owned section rect");
    let task = document
        .descendants()
        .find(|node| {
            node.has_tag_name("rect")
                && node.attribute("class").is_some_and(|class| {
                    class.split_ascii_whitespace().any(|token| token == "task")
                })
        })
        .expect("Journey source-owned task rect");
    assert_eq!(section.attribute("style"), Some("fill:#abcdef;"));
    assert_eq!(task.attribute("style"), Some("fill:#abcdef;"));

    let evidence = merman_render::__private::family_evidence(rendered.into_completion().report());
    assert_eq!(evidence.required_count(), 1);
    assert_eq!(evidence.accounted_count(), 1);
    assert_eq!(evidence.applied_count(), 1);
    assert_eq!(evidence.theme_residual_count(), 0);
    assert_eq!(evidence.compatibility_residual_count(), 0);
}
