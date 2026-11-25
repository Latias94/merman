#![cfg(feature = "diagram-flowchart")]

use merman_core::{MermaidEngine, ParseOptions};
use merman_render::environment::RenderEnvironment;
use merman_render::svg::{SvgDebugOptions, SvgRenderOptions};
use merman_render::theme::{NativeTheme, ThemeCompiler};
use merman_render::{LayoutOptions, family};
use roxmltree::{Document, Node};

#[test]
fn native_theme_v2_flowchart_markers_follow_direction_and_preserve_link_color() {
    let theme = ThemeCompiler::new()
        .compile_json(
            br##"{
                "schema_version":2,"extends":"paper",
                "roles":{"stroke.connector":"#245678"},"form":{"marker_style":"open"}
            }"##,
            None,
        )
        .unwrap();
    let source = "---\nconfig:\n  look: classic\n---\nflowchart LR\nA <--> B\nB --> C\nlinkStyle 1 stroke:#aa5500\n";
    let parsed = MermaidEngine::new()
        .parse_diagram_for_render_model_sync(source, ParseOptions::strict())
        .unwrap()
        .unwrap()
        .into_presentation();
    let session = RenderEnvironment::deterministic()
        .begin_session_with_native_theme(NativeTheme::new(theme))
        .unwrap();
    let rendered = family::prepare(&parsed, &LayoutOptions::headless_svg_defaults(), session)
        .unwrap()
        .render_svg(&SvgRenderOptions::default(), &SvgDebugOptions::default())
        .unwrap();
    let document = Document::parse(rendered.svg()).unwrap();
    let edge = |id| {
        document
            .descendants()
            .find(|node| {
                node.attribute("data-edge") == Some("true") && node.attribute("data-id") == Some(id)
            })
            .unwrap()
    };
    let two_way = edge("L_A_B_0");
    let authored = edge("L_B_C_0");
    assert!(
        authored
            .attribute("style")
            .unwrap()
            .contains("stroke:#aa5500")
    );
    let start = marker_path(&document, two_way, "marker-start");
    let end = marker_path(&document, two_way, "marker-end");
    let authored_end = marker_path(&document, authored, "marker-end");
    for path in [start, end, authored_end] {
        assert_eq!(path.attribute("d"), Some("M1 1L10 5L1 9"));
        assert!(path.attribute("style").unwrap().contains("fill: none"));
    }
    assert_eq!(start.attribute("transform"), Some("matrix(-1 0 0 1 10 0)"));
    assert_eq!(end.attribute("transform"), Some("matrix(1 0 0 1 0 0)"));
    assert_eq!(start.attribute("stroke"), None);
    assert_eq!(end.attribute("stroke"), None);
    assert_eq!(authored_end.attribute("stroke"), Some("#aa5500"));
    assert!(
        rendered
            .svg()
            .contains(".marker{fill:#245678;stroke:#245678;")
    );
}

fn marker_path<'a, 'input>(
    document: &'a Document<'input>,
    edge: Node<'_, '_>,
    attribute: &str,
) -> Node<'a, 'input> {
    let id = edge
        .attribute(attribute)
        .and_then(|value| value.strip_prefix("url(#"))
        .and_then(|value| value.strip_suffix(')'))
        .unwrap();
    document
        .descendants()
        .find(|node| node.has_tag_name("marker") && node.attribute("id") == Some(id))
        .unwrap()
        .children()
        .find(|node| node.has_tag_name("path"))
        .unwrap()
}
