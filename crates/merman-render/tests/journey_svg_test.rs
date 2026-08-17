use merman_core::{Engine, ParseOptions};
use merman_render::LayoutOptions;
use merman_render::diagram_theme::{
    DiagramThemeCompiler, DiagramThemeSpec, Specified, ThemeGeometryPatch,
    ThemePortabilityRequirement, ThemeRule, ThemeRuleSet, ThemeStylePatch, ThemeTarget,
};
use merman_render::environment::RenderEnvironment;
use merman_render::family;
use merman_render::svg::{SvgDebugOptions, SvgRenderOptions};

#[test]
fn journey_task_radius_reaches_terminal_svg_without_changing_sections() {
    for (radius, expected_task_radius) in [(Specified::Value(9.0), "9"), (Specified::Clear, "3")] {
        let theme = DiagramThemeCompiler::new()
            .compile(
                DiagramThemeSpec::new().with_styles(ThemeRuleSet::default().with_rule(
                    ThemeRule::new(
                        ThemeTarget::JourneyTask,
                        ThemeStylePatch {
                            geometry: ThemeGeometryPatch { radius },
                            ..ThemeStylePatch::default()
                        },
                    ),
                )),
            )
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
