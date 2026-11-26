#![cfg(all(
    feature = "layout-shoal",
    any(feature = "diagram-flowchart", feature = "diagram-agentflow")
))]

use merman_core::{MermaidEngine, ParseOptions};
use merman_render::environment::RenderEnvironment;
use merman_render::svg::SvgPipeline;
use merman_render::theme::{NativeTheme, ThemeCompiler};
use merman_render::{LayoutOptions, family};
use roxmltree::{Document, Node};

const THEME: &[u8] = br##"{
    "schema_version":2,"extends":"studio",
    "roles":{"canvas":"#FAF7EC","surface.container":"#FAF7EC","stroke.connector":"#202938"},
    "form":{"container_radius":4,"elevation":"flat","marker_style":"filled"}
}"##;

const COMPOUND: &str = "subgraph G[Group]\nA inside@<--> B\nB authored@--> C\nend\n\
    X cross@--> A\nX dotted@-.-> B\n\
    classDef midnight fill:#161D2E,stroke:#657086\nclass G midnight\n\
    linkStyle 1 stroke:#D97706\nlinkStyle 2 stroke-width:2px";

fn render(keyword: &str, backend: &str, look: &str, body: &str, resvg: bool) -> String {
    let source = format!(
        "---\nconfig:\n  layout: {backend}\n  look: {look}\n  htmlLabels: false\n---\n{keyword} LR\n{body}"
    );
    let parsed = MermaidEngine::new()
        .parse_diagram_for_render_model_sync(&source, ParseOptions::strict())
        .unwrap()
        .unwrap()
        .into_presentation();
    let theme = ThemeCompiler::new().compile_json(THEME, None).unwrap();
    let session = RenderEnvironment::deterministic()
        .begin_session_with_native_theme(NativeTheme::new(theme))
        .unwrap();
    let rendered = family::prepare(&parsed, &LayoutOptions::headless_svg_defaults(), session)
        .unwrap()
        .render_svg(&Default::default(), &Default::default())
        .unwrap();
    if resvg {
        rendered
            .finalize_resvg(&SvgPipeline::resvg_safe())
            .unwrap()
            .svg()
            .as_str()
            .to_owned()
    } else {
        rendered.svg().to_owned()
    }
}

fn edge<'a, 'input>(doc: &'a Document<'input>, id: &str) -> Node<'a, 'input> {
    doc.descendants()
        .find(|node| {
            node.attribute("data-edge") == Some("true") && node.attribute("data-id") == Some(id)
        })
        .unwrap_or_else(|| panic!("missing edge {id}"))
}

fn marker_shape<'a, 'input>(
    doc: &'a Document<'input>,
    edge: Node<'a, 'input>,
    attribute: &str,
) -> Node<'a, 'input> {
    let id = edge
        .attribute(attribute)
        .and_then(|value| value.strip_prefix("url(#"))
        .and_then(|value| value.strip_suffix(')'))
        .unwrap();
    doc.descendants()
        .find(|node| node.has_tag_name("marker") && node.attribute("id") == Some(id))
        .and_then(|marker| marker.children().find(Node::is_element))
        .unwrap()
}

fn assert_compound_paint(svg: &str, bidirectional: bool) {
    let doc = Document::parse(svg).unwrap();
    let inside = edge(&doc, "inside");
    let authored = edge(&doc, "authored");
    let cross = edge(&doc, "cross");
    assert!(
        inside
            .attribute("style")
            .unwrap()
            .contains("stroke:#FFFFFF")
    );
    let expected_markers = if bidirectional {
        &["marker-start", "marker-end"][..]
    } else {
        assert!(inside.attribute("marker-start").is_none());
        &["marker-end"][..]
    };
    for end in expected_markers {
        let shape = marker_shape(&doc, inside, end);
        assert_eq!(shape.attribute("stroke"), Some("#FFFFFF"));
        assert_eq!(shape.attribute("fill"), Some("#FFFFFF"));
    }
    assert!(
        authored
            .attribute("style")
            .unwrap()
            .contains("stroke:#D97706")
    );
    assert_eq!(
        marker_shape(&doc, authored, "marker-end").attribute("stroke"),
        Some("#D97706")
    );
    let casings = doc
        .descendants()
        .find(|node| node.attribute("class") == Some("merman-edge-casings"))
        .unwrap();
    let paths: Vec<_> = casings.children().filter(Node::is_element).collect();
    let [path] = paths.as_slice() else {
        panic!("expected a casing only for the visible solid crossing");
    };
    assert_eq!(path.attribute("d"), cross.attribute("d"));
    assert_eq!(path.attribute("stroke"), Some("#FAF7EC"));
    assert_eq!(path.attribute("stroke-width"), Some("5"));
    assert_eq!(path.attribute("marker-end"), None);
    assert_eq!(path.attribute("data-edge"), None);
    assert!(casings.range().end < inside.range().start);
}

#[cfg(feature = "diagram-flowchart")]
#[test]
fn shoal_flowchart_paints_fill_boundaries_and_adapts_internal_markers() {
    assert_compound_paint(&render("flowchart", "shoal", "neo", COMPOUND, false), true);
    assert_compound_paint(
        &render("flowchart", "shoal", "classic", COMPOUND, true),
        true,
    );
}

#[cfg(feature = "diagram-flowchart")]
#[test]
fn shoal_flowchart_uses_the_parent_paint_at_a_nested_boundary() {
    let svg = render(
        "flowchart",
        "shoal",
        "classic",
        concat!(
            "subgraph Outer\nA[Outside inner]\nsubgraph Inner\nB inside@--> C\nend\n",
            "A cross@--> B\nend\n",
            "style Outer fill:#DAE9F7\nstyle Inner fill:#141C31,fill-opacity:0.8\n",
        ),
        false,
    );
    let doc = Document::parse(&svg).unwrap();
    let casings = doc
        .descendants()
        .find(|node| node.attribute("class") == Some("merman-edge-casings"))
        .unwrap();
    let casing = casings.first_element_child().unwrap();
    assert_eq!(casing.attribute("d"), edge(&doc, "cross").attribute("d"));
    assert_eq!(casing.attribute("stroke"), Some("#DAE9F7"));
    assert!(
        edge(&doc, "inside")
            .attribute("style")
            .unwrap()
            .contains("stroke:#FFFFFF")
    );
}

#[cfg(feature = "diagram-flowchart")]
#[test]
fn shoal_flowchart_skips_transparent_fills_and_authored_invisible_strokes() {
    let svg = render(
        "flowchart",
        "shoal",
        "classic",
        concat!(
            "subgraph G\nA inside@--> B\nend\nX transparent@--> A\nstyle G fill:#000000,fill-opacity:0\n",
            "subgraph H\nC --> D\nend\nY hidden@--> C\nstyle H fill:#101A24\n",
            "linkStyle 3 stroke:none\n",
        ),
        false,
    );
    assert!(!svg.contains("merman-edge-casings"));
    let doc = Document::parse(&svg).unwrap();
    assert!(
        !edge(&doc, "inside")
            .attribute("style")
            .unwrap()
            .contains("stroke:#FFFFFF")
    );
    assert!(
        edge(&doc, "hidden")
            .attribute("style")
            .unwrap()
            .contains("stroke:none")
    );
}

#[cfg(feature = "diagram-agentflow")]
#[test]
fn shoal_agentflow_paints_resolved_subgraph_fill() {
    let source = COMPOUND
        .replacen("subgraph ", "flow ", 1)
        .replace("<-->", "-->")
        .replace("-.->", "-.-");
    assert_compound_paint(
        &render("agentflow-beta", "shoal", "classic", &source, false),
        false,
    );
}

#[cfg(all(
    feature = "diagram-flowchart",
    any(feature = "layout-dagre", feature = "layout-elk")
))]
#[test]
fn flowchart_parity_backends_keep_the_original_edge_paint_with_the_same_theme() {
    for backend in [
        #[cfg(feature = "layout-dagre")]
        "dagre",
        #[cfg(feature = "layout-elk")]
        "elk",
    ] {
        let svg = render("flowchart", backend, "classic", COMPOUND, false);
        assert!(!svg.contains("merman-edge-casings"));
        let doc = Document::parse(&svg).unwrap();
        assert!(
            !edge(&doc, "inside")
                .attribute("style")
                .unwrap()
                .contains("stroke:#FFFFFF")
        );
    }
}
