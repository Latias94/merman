#![cfg(feature = "diagram-mindmap")]

use merman_core::{MermaidEngine, ParseOptions};
use merman_render::LayoutOptions;
use merman_render::environment::RenderEnvironment;
use merman_render::family;
use merman_render::svg::{SvgDebugOptions, SvgRenderOptions};
use serde_json::Value;

const LAYOUTS: &[&str] = &[
    "dagre",
    "tidy-tree",
    "swimlane",
    #[cfg(feature = "layout-elk")]
    "elk",
];

fn render(label: &str, root_size: u32, layout: &str, circle: bool) -> (String, Value) {
    let node = if circle {
        format!("root((\"{label}\"))")
    } else {
        format!("root[\"{label}\"]")
    };
    let source = format!(
        "---\nconfig:\n  layout: {layout}\n  look: classic\n  htmlLabels: true\n  themeVariables:\n    fontSize: {root_size}px\n---\nmindmap\n  {node}\n"
    );
    let parsed = MermaidEngine::new()
        .parse_diagram_for_render_model_sync(&source, ParseOptions::strict())
        .unwrap()
        .unwrap()
        .into_presentation();
    let session = RenderEnvironment::deterministic().begin_session().unwrap();
    let artifact = family::prepare(&parsed, &LayoutOptions::default(), session).unwrap();
    let geometry = artifact.layout_json().unwrap()["layout"]["MindmapDiagram"].clone();
    let svg = artifact
        .render_svg(&SvgRenderOptions::default(), &SvgDebugOptions::default())
        .unwrap()
        .svg()
        .to_owned();
    (svg, geometry)
}

fn label_box(svg: &str) -> (f64, f64) {
    let document = roxmltree::Document::parse(svg).unwrap();
    let foreign = document
        .descendants()
        .find(|node| {
            node.has_tag_name("foreignObject")
                && node
                    .descendants()
                    .any(|child| child.is_text() && child.text() == Some("WideText"))
        })
        .unwrap();
    (
        foreign.attribute("width").unwrap().parse().unwrap(),
        foreign.attribute("height").unwrap().parse().unwrap(),
    )
}

#[test]
fn mindmap_html_rem_matches_document_pixels_and_em_follows_the_parent() {
    for layout in LAYOUTS {
        for root_size in [10, 24] {
            for circle in [false, true] {
                for (relative, pixels) in [("2rem", 32), ("2em", 2 * root_size)] {
                    let label = format!("<span style='font-size:{relative}'>WideText</span>");
                    let (relative_svg, relative_geometry) =
                        render(&label, root_size, layout, circle);
                    let (pixel_svg, pixel_geometry) = render(
                        &format!("<span style='font-size:{pixels}px'>WideText</span>"),
                        root_size,
                        layout,
                        circle,
                    );
                    assert_eq!(
                        label_box(&relative_svg),
                        label_box(&pixel_svg),
                        "HTML terminal geometry: {layout}, {root_size}px, {relative}"
                    );
                    assert_eq!(
                        relative_geometry, pixel_geometry,
                        "shape and root bounds: {layout}, {root_size}px, {relative}"
                    );
                }
            }
        }
    }
}

#[test]
fn mindmap_nested_html_fonts_keep_document_and_parent_contexts_separate() {
    for (relative, pixels) in [
        (
            "<span style='font-size:2em'><span style='font-size:2rem'>WideText</span></span>",
            "<span style='font-size:20px'><span style='font-size:32px'>WideText</span></span>",
        ),
        (
            "<span style='font-size:2rem'><span style='font-size:1.5em'>WideText</span></span>",
            "<span style='font-size:32px'><span style='font-size:48px'>WideText</span></span>",
        ),
    ] {
        let (relative_svg, relative_geometry) = render(relative, 10, "dagre", false);
        let (pixel_svg, pixel_geometry) = render(pixels, 10, "dagre", false);
        assert_eq!(label_box(&relative_svg), label_box(&pixel_svg));
        assert_eq!(relative_geometry, pixel_geometry);
    }
}

#[test]
fn mindmap_plain_html_collapses_spaces_before_deciding_to_wrap() {
    let mut geometry: Option<Value> = None;
    for label in ["A B", "A  B", "A   B"] {
        let source = format!(
            "---\nconfig:\n  layout: dagre\n  htmlLabels: true\n  themeVariables:\n    fontSize: 16px\n  flowchart:\n    wrappingWidth: 27\n---\nmindmap\n  root[\"{label}\"]\n"
        );
        let parsed = MermaidEngine::new()
            .parse_diagram_for_render_model_sync(&source, ParseOptions::strict())
            .unwrap()
            .unwrap()
            .into_presentation();
        let prepared = family::prepare(
            &parsed,
            &LayoutOptions::default(),
            RenderEnvironment::deterministic().begin_session().unwrap(),
        )
        .unwrap();
        let actual = prepared.layout_json().unwrap()["layout"]["MindmapDiagram"].clone();
        if let Some(expected) = &geometry {
            // Compare only rendered geometry: the label still retains its authored spaces.
            assert_eq!(actual["bounds"], expected["bounds"], "{label}");
            assert_eq!(
                actual["nodes"][0]["width"], expected["nodes"][0]["width"],
                "{label}"
            );
            assert_eq!(
                actual["nodes"][0]["height"], expected["nodes"][0]["height"],
                "{label}"
            );
        } else {
            geometry = Some(actual);
        }
        let rendered = prepared
            .render_svg(&SvgRenderOptions::default(), &SvgDebugOptions::default())
            .unwrap();
        let document = roxmltree::Document::parse(rendered.svg()).unwrap();
        let foreign = document
            .descendants()
            .find(|node| node.has_tag_name("foreignObject"))
            .unwrap();
        assert_eq!(foreign.attribute("height"), Some("24"));
        let div = foreign
            .children()
            .find(|node| node.has_tag_name("div"))
            .unwrap();
        assert!(
            div.attribute("style")
                .unwrap()
                .contains("white-space: nowrap")
        );
    }
}
