#![cfg(feature = "diagram-flowchart")]

use merman_core::presentation::SourcePresentationConfig;
use merman_core::{MermaidEngine, ParseOptions};
use merman_render::environment::RenderEnvironment;
use merman_render::svg::{SvgDebugOptions, SvgRenderOptions};
use merman_render::theme::{NativeTheme, ThemeCompiler};
use merman_render::{LayoutOptions, family};
use serde_json::json;

#[test]
fn native_theme_v2_bpmn_keeps_notation_widths_and_source_selectors() {
    let theme = NativeTheme::new(
        ThemeCompiler::new()
            .compile_json(
                br#"{"schema_version":2,"extends":"paper","form":{"stroke_width":3}}"#,
                None,
            )
            .unwrap(),
    );
    let source = r#"flowchart LR
Start@{ shape: bpmn-start, label: "Start" }
Intermediate@{ shape: bpmn-intermediate, label: "Intermediate" }
Boundary@{ shape: bpmn-boundary, label: "Boundary" }
End@{ shape: bpmn-end, label: "End" }
Note@{ shape: bpmn-annotation, label: "Note" }
style Start stroke-width:9px
style Note stroke-width:7px
classDef bpmn-event-body stroke:#abcdef
classDef bpmn-annotation-body stroke:#fedcba
"#;
    for look in ["classic", "neo", "handDrawn"] {
        for html in [false, true] {
            let parsed = MermaidEngine::new()
                .with_site_config(SourcePresentationConfig::from_value(json!({
                    "layout": "dagre", "look": look, "htmlLabels": html, "handDrawnSeed": 1,
                })))
                .parse_diagram_for_render_model_sync(source, ParseOptions::strict())
                .unwrap()
                .unwrap()
                .into_presentation();
            let session = RenderEnvironment::deterministic()
                .begin_session_with_native_theme(theme.clone())
                .unwrap();
            let rendered =
                family::prepare(&parsed, &LayoutOptions::headless_svg_defaults(), session)
                    .unwrap()
                    .render_svg(&SvgRenderOptions::default(), &SvgDebugOptions::default())
                    .unwrap();
            let document = roxmltree::Document::parse(rendered.svg()).unwrap();
            let node = |id| {
                document
                    .descendants()
                    .find(|node| node.attribute("data-id") == Some(id))
                    .unwrap()
            };
            for (id, count, width) in [
                ("Start", 1, "1.6"),
                ("Intermediate", 2, "1.4"),
                ("Boundary", 2, "1.4"),
                ("End", 1, "3.4"),
            ] {
                let rings = node(id)
                    .descendants()
                    .filter(|ring| has_class(*ring, "bpmn-event-ring"))
                    .collect::<Vec<_>>();
                assert_eq!(rings.len(), count, "{look}, html={html}, {id}");
                for ring in rings {
                    assert_eq!(
                        ring.attribute("style"),
                        Some(format!("stroke-width: {width}px;").as_str()),
                        "{look}, html={html}, {id}"
                    );
                }
            }
            let start_body = node("Start")
                .children()
                .find(|child| has_class(*child, "bpmn-event-body"))
                .unwrap();
            assert!(
                start_body
                    .attribute("style")
                    .unwrap()
                    .contains("stroke-width:9px")
            );
            let bracket = node("Note")
                .descendants()
                .find(|child| has_class(*child, "bpmn-annotation-bracket"))
                .unwrap();
            assert!(bracket.has_tag_name("path"));
            assert_eq!(bracket.attribute("style"), None);

            let css = document
                .descendants()
                .find(|node| node.has_tag_name("style"))
                .and_then(|node| node.text())
                .unwrap();
            let (ring_selector, bracket_selector) = if html {
                (">*", ">*")
            } else {
                (" circle", " path")
            };
            assert!(css.contains(&format!(
                ".bpmn-event-body{ring_selector}{{stroke:rgb(171, 205, 239)!important;}}"
            )));
            assert!(css.contains(&format!(
                ".bpmn-annotation-body{bracket_selector}{{stroke:rgb(254, 220, 186)!important;}}"
            )));
        }
    }
}

fn has_class(node: roxmltree::Node<'_, '_>, class: &str) -> bool {
    node.attribute("class")
        .is_some_and(|classes| classes.split_ascii_whitespace().any(|part| part == class))
}
