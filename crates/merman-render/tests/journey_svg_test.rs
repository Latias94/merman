use merman_core::{Engine, ParseOptions};
use merman_render::LayoutOptions;
use merman_render::diagram_theme::{
    CanvasPaint, DiagramThemeCompiler, DiagramThemeSpec, OrdinalSelector, Specified,
    ThemeGeometryPatch, ThemePortabilityRequirement, ThemeRule, ThemeRuleSet, ThemeStylePatch,
    ThemeTarget, ThemeVariant,
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
