#![cfg(feature = "layout-cytoscape")]

use merman_core::{Engine, ParseOptions, ParsedDiagramRender};
use merman_render::LayoutOptions;
use merman_render::environment::{RenderEnvironment, RenderSession};
use merman_render::family;
use merman_render::model::MindmapDiagramLayout;
use merman_render::resources::RenderResourcePolicy;
use merman_render::svg::{SvgDebugOptions, SvgRenderOptions};

fn render_mindmap_svg_from_text(text: &str, diagram_id: &str) -> String {
    let session = RenderEnvironment::deterministic().begin_session().unwrap();
    let engine = Engine::new();
    let parsed = engine
        .parse_diagram_for_render_model_sync(text, ParseOptions::default())
        .expect("parse ok")
        .expect("diagram detected");

    let layout_options = LayoutOptions::headless_svg_defaults();
    let artifact = family::prepare(parsed, &layout_options, session).expect("layout ok");

    artifact
        .render_svg(
            &SvgRenderOptions {
                diagram_id: Some(diagram_id.to_string()),
                ..SvgRenderOptions::default()
            },
            &SvgDebugOptions::default(),
        )
        .expect("render svg")
        .svg()
        .to_owned()
}

fn layout_mindmap_typed(
    parsed: &ParsedDiagramRender,
    session: RenderSession,
) -> MindmapDiagramLayout {
    let artifact =
        family::prepare(parsed.clone(), &LayoutOptions::default(), session).expect("layout ok");
    let projection = artifact.layout_json().expect("serialize Mindmap layout");
    serde_json::from_value(projection["layout"]["MindmapDiagram"].clone())
        .expect("Mindmap layout projection")
}

fn deep_mindmap_chain(depth: usize) -> String {
    let mut input = String::from("---\nconfig:\n  layout: tidy-tree\n---\nmindmap\n");
    for level in 0..depth {
        input.push_str(&" ".repeat(level));
        input.push_str(&format!("n{level}\n"));
    }
    input
}

#[test]
fn mindmap_svg_emits_mermaid_11_15_classic_dom_surface() {
    let svg = render_mindmap_svg_from_text(
        r#"mindmap
  Root
    Child
"#,
        "m15-mindmap",
    );

    assert!(
        svg.contains(r#"id="m15-mindmap-node_0" data-look="classic""#),
        "expected classic Mindmap node DOM id to be diagram-prefixed and expose data-look: {svg}"
    );
    assert!(
        svg.contains(r#"id="m15-mindmap-edge_0_1""#)
            && svg.contains(r#"data-id="edge_0_1""#)
            && svg.contains(r#"data-look="classic""#),
        "expected Mindmap edge DOM id to be diagram-prefixed while data-id keeps the raw edge id: {svg}"
    );
    assert!(
        svg.contains(r#"<span class="nodeLabel markdown-node-label"><p>Root</p></span>"#),
        "expected Mindmap XHTML labels to keep Mermaid 11.15 class ordering: {svg}"
    );
    assert!(
        svg.contains(r#"id="m15-mindmap_mindmap-pointEnd-margin""#)
            && svg.contains(r#"id="m15-mindmap_mindmap-pointStart-margin""#),
        "expected Mermaid 11.15 Mindmap margin markers: {svg}"
    );
    assert!(
        svg.contains(r#"id="m15-mindmap-drop-shadow""#)
            && svg.contains(r#"id="m15-mindmap-drop-shadow-small""#),
        "expected Mermaid 11.15 Mindmap scoped drop-shadow defs: {svg}"
    );
}

#[test]
fn mindmap_hex_entity_placeholders_remain_literal_well_formed_xml() {
    for (source, expected) in [
        ("mindmap\n  root[&#x41;]\n", "<p>&amp;&amp;x41;</p>"),
        ("mindmap\n  root[&#X41;]\n", "<p>&amp;&amp;X41;</p>"),
    ] {
        let svg = render_mindmap_svg_from_text(source, "mindmap-hex-entity");

        assert!(svg.contains(expected), "expected {expected:?}: {svg}");
        roxmltree::Document::parse(&svg).expect("Mindmap SVG must be well-formed XML");
    }
}

#[test]
fn mindmap_typed_layout_handles_deep_chain() {
    let session = RenderEnvironment::deterministic()
        .with_resource_policy(RenderResourcePolicy::unbounded_for_trusted_input())
        .begin_session()
        .unwrap();
    const DEPTH: usize = 1200;
    let source = deep_mindmap_chain(DEPTH);

    let engine = Engine::new();
    let parsed = engine
        .parse_diagram_for_render_model_sync(&source, ParseOptions::strict())
        .expect("parse ok")
        .expect("diagram detected");

    // Exercise the public JSON projection as well as its ordinary destruction. The semantic
    // projection contains the same deeply nested rootNode tree even though the typed model and
    // layout are flat.
    let layout = layout_mindmap_typed(&parsed, session);

    assert_eq!(layout.nodes.len(), DEPTH);
    assert_eq!(layout.edges.len(), DEPTH - 1);
    let expected_last = format!("{}", DEPTH - 1);
    assert_eq!(
        layout.nodes.last().map(|node| node.id.as_str()),
        Some(expected_last.as_str())
    );
}

#[test]
fn mindmap_svg_wraps_section_classes_after_mermaid_palette_cycle() {
    let svg = render_mindmap_svg_from_text(
        r#"mindmap
  root((Many siblings))
    s01[Node 01]
    s02[Node 02]
    s03[Node 03]
    s04[Node 04]
    s05[Node 05]
    s06[Node 06]
    s07[Node 07]
    s08[Node 08]
    s09[Node 09]
    s10[Node 10]
    s11[Node 11]
    s12[Node 12]
"#,
        "m15-mindmap-cycle",
    );

    assert!(
        svg.contains(r#"class="node mindmap-node section-10" id="m15-mindmap-cycle-node_11""#),
        "expected eleventh sibling to use section-10 before the cycle wraps: {svg}"
    );
    assert!(
        svg.contains(r#"class="node mindmap-node section-0" id="m15-mindmap-cycle-node_12""#),
        "expected twelfth sibling to wrap back to section-0 like Mermaid 11.15: {svg}"
    );
    assert!(
        !svg.contains("section-11") && !svg.contains("section-edge-11"),
        "Mindmap section classes should wrap instead of emitting stale section-11 tokens: {svg}"
    );
}

#[test]
fn mindmap_svg_uses_direct_classic_shapes_for_rounded_and_hexagon_nodes() {
    let svg = render_mindmap_svg_from_text(
        r#"mindmap
  root((Root))
    rounded(Rounded)
    hex{{Hexagon}}
"#,
        "m15-mindmap-shapes",
    );

    assert!(
        svg.contains(r#"<rect class="basic label-container" style="" rx="5" ry="5""#),
        "expected classic rounded Mindmap nodes to render as direct rect DOM: {svg}"
    );
    assert!(
        svg.contains(r#"<polygon points=""#) && svg.contains(r#"class="label-container""#),
        "expected classic hexagon Mindmap nodes to render as direct polygon DOM: {svg}"
    );
    assert!(
        !svg.contains(r#"class="basic label-container outer-path""#),
        "classic Mindmap rounded/hexagon nodes should not use the old rough outer-path wrapper: {svg}"
    );
}

#[test]
fn mindmap_tidy_tree_config_dispatches_bidirectional_layout() {
    let session = RenderEnvironment::deterministic().begin_session().unwrap();
    let engine = Engine::new();
    let parsed = engine
        .parse_diagram_for_render_model_sync(
            r#"---
config:
  layout: tidy-tree
---
mindmap
  root((Root))
    Left
      Left child
    Right
      Right child
    Also left
"#,
            ParseOptions::strict(),
        )
        .expect("parse ok")
        .expect("diagram detected");

    let layout = layout_mindmap_typed(&parsed, session);
    let node = |id: &str| {
        layout
            .nodes
            .iter()
            .find(|node| node.id == id)
            .unwrap_or_else(|| panic!("missing node {id}"))
    };

    let root = node("0");
    let left = node("1");
    let left_child = node("2");
    let right = node("3");
    let right_child = node("4");
    let also_left = node("5");
    assert!(root.x.is_finite() && root.y.is_finite());
    assert!(left.x < root.x && left_child.x < left.x);
    assert!(right.x > root.x && right_child.x > right.x);
    assert!(also_left.x < root.x);

    assert!(layout.edges.iter().all(|edge| edge.points.len() == 4));
    let edge_to_left = layout
        .edges
        .iter()
        .find(|edge| edge.from == "0" && edge.to == "1")
        .expect("root-to-left edge");
    let edge_to_right = layout
        .edges
        .iter()
        .find(|edge| edge.from == "0" && edge.to == "3")
        .expect("root-to-right edge");
    assert!(edge_to_left.points[1].x < root.x);
    assert!(edge_to_right.points[1].x > root.x);
}
