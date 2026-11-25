#![cfg(feature = "diagram-er")]

use merman_core::{MermaidEngine, ParseOptions};
use merman_render::LayoutOptions;

use merman_render::environment::RenderEnvironment;
use merman_render::family;
use merman_render::svg::{SvgDebugOptions, SvgRenderOptions};
use serde_json::json;

#[test]
fn er_swimlane_source_fonts_follow_each_groups_actual_html_mode() {
    for (root, flowchart) in [(false, true), (true, false)] {
        for (styled_group, html) in [("Outer", flowchart), ("Inner", root)] {
            let source = format!(
                "---\nconfig: {}\n---\nerDiagram\nsubgraph Outer[Outer title]\nsubgraph Inner[Inner title]\nA\nend\nend\nstyle {styled_group} font-family:SourceFace,font-size:30px\n",
                json!({"layout": "swimlane", "htmlLabels": root,
                    "flowchart": {"htmlLabels": flowchart}}),
            );
            let parsed = MermaidEngine::new()
                .parse_diagram_for_render_model_sync(&source, ParseOptions::strict())
                .unwrap()
                .unwrap()
                .into_presentation();
            let session = RenderEnvironment::deterministic().begin_session().unwrap();
            let rendered = family::prepare(&parsed, &LayoutOptions::default(), session)
                .unwrap()
                .render_svg(&SvgRenderOptions::default(), &SvgDebugOptions::default())
                .unwrap();
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
        }
    }
}

#[test]
fn er_html_subgraph_preparation_charges_its_retained_projection() {
    use merman_render::resources::{RenderResourcePolicy, ResourceLimitId};
    for config in [
        json!({"layout": "dagre", "htmlLabels": true}),
        json!({"layout": "swimlane", "htmlLabels": false,
            "flowchart": {"htmlLabels": true}}),
    ] {
        let render = |title: &str, policy: RenderResourcePolicy| {
            let source = format!(
                "---\nconfig: {config}\n---\nerDiagram\nsubgraph Lane[\"{title}\"]\nA\nend\n"
            );
            let parsed = MermaidEngine::new()
                .parse_diagram_for_render_model_sync(&source, ParseOptions::strict())
                .unwrap()
                .unwrap()
                .into_presentation();
            let session = RenderEnvironment::deterministic()
                .with_resource_policy(policy)
                .begin_session()
                .unwrap();
            let rendered = family::prepare(&parsed, &LayoutOptions::default(), session)?
                .render_svg(&SvgRenderOptions::default(), &SvgDebugOptions::default())?;
            Ok::<_, merman_render::Error>(
                rendered
                    .into_completion()
                    .report()
                    .session_report()
                    .prepared_text_retained_bytes_peak(),
            )
        };
        let short_bytes =
            render("short", RenderResourcePolicy::unbounded_for_trusted_input()).unwrap();
        let title = "&".repeat(2048);
        let long_bytes =
            render(&title, RenderResourcePolicy::unbounded_for_trusted_input()).unwrap();
        assert!(long_bytes > short_bytes + title.len(), "{config}");
        let error = render(
            &title,
            RenderResourcePolicy::unbounded_for_trusted_input()
                .with_limit(ResourceLimitId::MaxPreparedTextRetainedBytes, short_bytes)
                .unwrap(),
        )
        .unwrap_err();
        let merman_render::Error::ResourceLimitExceeded(error) = error else {
            panic!("expected retained HTML-title byte rejection");
        };
        assert_eq!(error.limit, "max_prepared_text_retained_bytes");
        assert_eq!(error.max, short_bytes);
        assert!(error.actual > short_bytes);
    }
}
