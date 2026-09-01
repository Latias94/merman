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
    let styles = ThemeRuleSet::default()
        .with_rule(ThemeRule::new(
            ThemeTarget::JourneyTask,
            ThemeStylePatch::default()
                .with_fill(CanvasPaint::solid("#123456").expect("valid Journey task fill")),
        ))
        .with_rule(ThemeRule::new(
            ThemeTarget::JourneyTask,
            ThemeStylePatch::default()
                .with_stroke(CanvasPaint::solid("#654321").expect("valid Journey task stroke")),
        ));
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
