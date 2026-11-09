#![cfg(feature = "diagram-er")]

use merman_core::{MermaidEngine, ParseOptions};
use merman_model::family::DiagramFamilyId;
use merman_render::LayoutOptions;
use merman_render::diagram_theme::{
    DiagramThemeCompiler, DiagramThemeSpec, FontStack, ThemePortabilityRequirement, ThemeTextStyle,
    TypographySpec,
};
use merman_render::environment::RenderEnvironment;
use merman_render::family;
use merman_render::svg::{SvgDebugOptions, SvgRenderOptions};
use serde_json::json;

mod common;

#[test]
fn er_swimlane_source_fonts_follow_each_groups_actual_html_mode() {
    let theme = DiagramThemeCompiler::new()
        .compile(
            DiagramThemeSpec::new().with_typography(
                TypographySpec::default().with_family_style(
                    DiagramFamilyId::ER,
                    ThemeTextStyle::default()
                        .with_font_stack(FontStack::single("TypedFace").unwrap())
                        .with_font_size_px(24.0)
                        .unwrap(),
                ),
            ),
        )
        .unwrap();
    for (root, flowchart) in [(false, true), (true, false)] {
        for (styled_group, html) in [("Outer", flowchart), ("Inner", root)] {
            let source = format!(
                "---\nconfig: {}\n---\nerDiagram\nsubgraph Outer[Outer title]\nsubgraph Inner[Inner title]\nA\nend\nend\nstyle {styled_group} font-family:SourceFace,font-size:30px\n",
                json!({"layout": "swimlane", "htmlLabels": root,
                    "flowchart": {"htmlLabels": flowchart}}),
            );
            let parsed = common::install_parse_compatibility(&theme, MermaidEngine::new())
                .parse_diagram_for_render_model_sync(&source, ParseOptions::strict())
                .unwrap()
                .unwrap()
                .into_presentation();
            for requirement in [
                ThemePortabilityRequirement::BestEffort,
                ThemePortabilityRequirement::RequirePortable,
            ] {
                let session = RenderEnvironment::deterministic()
                    .with_theme_portability_requirement(requirement)
                    .begin_session_with_theme(&theme)
                    .unwrap();
                let result = family::prepare(&parsed, &LayoutOptions::default(), session)
                    .unwrap()
                    .render_svg(&SvgRenderOptions::default(), &SvgDebugOptions::default());
                if html && matches!(requirement, ThemePortabilityRequirement::RequirePortable) {
                    let error = match result {
                        Ok(_) => panic!("expected unverified ER source typography"),
                        Err(error) => error,
                    };
                    assert_eq!(
                        error.unverified_family_theme(),
                        Some((DiagramFamilyId::ER, 2)),
                        "{styled_group}: root={root}, flowchart={flowchart}"
                    );
                    continue;
                }
                let rendered = result.unwrap();
                let document = roxmltree::Document::parse(rendered.svg()).unwrap();
                let dom_id = format!("merman-{styled_group}");
                let group = document
                    .descendants()
                    .find(|element| element.attribute("id") == Some(dom_id.as_str()))
                    .unwrap();
                assert_eq!(
                    group.descendants().any(|element| {
                        element
                            .attribute("style")
                            .is_some_and(|style| style.contains("SourceFace"))
                    }),
                    html,
                );
                let completion = rendered.into_completion();
                let evidence = merman_render::__private::family_evidence(completion.report());
                assert_eq!(evidence.theme_residual_count(), if html { 2 } else { 0 });
                assert_eq!(evidence.applied_count(), if html { 0 } else { 2 });
            }
        }
    }
}
