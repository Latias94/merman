#![cfg(feature = "diagram-flowchart")]

use merman_core::presentation::SourcePresentationConfig;
use merman_core::{MermaidEngine, ParseOptions};
use merman_render::environment::RenderEnvironment;
use merman_render::svg::{SvgDebugOptions, SvgRenderOptions};
use merman_render::{LayoutOptions, family};
use serde_json::json;

const LAYOUTS: &[&str] = &[
    "dagre",
    #[cfg(feature = "layout-elk")]
    "elk",
];

fn label_size(source: &str, id: &str, layout: &str) -> (f64, f64) {
    let parsed = MermaidEngine::new()
        .with_site_config(SourcePresentationConfig::from_value(json!({
            "layout": layout, "look": "classic", "htmlLabels": true,
            // Isolate glyph measurement from the default 120px minimum node width.
            "flowchart": {"htmlLabels": true, "minNodeWidth": 0},
        })))
        .parse_diagram_for_render_model_sync(source, ParseOptions::strict())
        .unwrap()
        .unwrap()
        .into_presentation();
    let rendered = family::prepare(
        &parsed,
        &LayoutOptions::headless_svg_defaults(),
        RenderEnvironment::deterministic().begin_session().unwrap(),
    )
    .unwrap()
    .render_svg(&SvgRenderOptions::default(), &SvgDebugOptions::default())
    .unwrap();
    let document = roxmltree::Document::parse(rendered.svg()).unwrap();
    let label = document
        .descendants()
        .find(|node| node.attribute("data-id") == Some(id))
        .unwrap()
        .descendants()
        .find(|node| node.has_tag_name("foreignObject"))
        .unwrap();
    (
        label.attribute("width").unwrap().parse().unwrap(),
        label.attribute("height").unwrap().parse().unwrap(),
    )
}

#[test]
fn structural_fonts_measure_the_authored_size_for_nodes_and_collapsed_subgraphs() {
    for layout in LAYOUTS {
        for (diagram, id) in [
            ("flowchart LR\nA[WIDE]\n", "A"),
            (
                "flowchart LR\nsubgraph G[WIDE WIDE]\nA\nend\nG@{ view: collapsed }\n",
                "G",
            ),
        ] {
            let baseline = label_size(diagram, id, layout);
            let expected = label_size(&format!("{diagram}style {id} font-size:32px\n"), id, layout);
            assert!(
                expected.0 > baseline.0 && expected.1 > baseline.1,
                "{layout}: {diagram}"
            );
            for class in ["root", "nodes", "label", "nodeLabel"] {
                let source = format!("{diagram}classDef {class} font-size:32px\n");
                assert_eq!(
                    label_size(&source, id, layout),
                    expected,
                    "{layout}, {id}, {class}"
                );
            }
        }
    }
}

#[test]
fn bpmn_wrappers_apply_source_fonts_to_html_label_measurements() {
    let diagram = "flowchart LR\nA@{ shape: bpmn-start, label: \"WIDE\" }\n";
    for layout in LAYOUTS {
        let baseline = label_size(diagram, "A", layout);
        let larger = label_size(&format!("{diagram}style A font-size:32px\n"), "A", layout);
        assert!(larger.0 > baseline.0 && larger.1 > baseline.1, "{layout}");
        for (classes, expected) in [
            (
                ["label", "nodeLabel", "bpmn-event", "bpmn-event-start"],
                larger,
            ),
            (
                [
                    "bpmn-event-body",
                    "bpmn-annotation-body",
                    "text-inner-tspan",
                    "bpmn-event-end",
                ],
                baseline,
            ),
        ] {
            for class in classes {
                let source = format!("{diagram}classDef {class} font-size:32px\n");
                assert_eq!(
                    label_size(&source, "A", layout),
                    expected,
                    "{layout}, {class}"
                );
            }
        }
    }
}
