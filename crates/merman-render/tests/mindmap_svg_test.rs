use merman_core::{Engine, ParseOptions};
use merman_render::model::LayoutDiagram;
use merman_render::svg::{SvgRenderOptions, render_layouted_svg};
use merman_render::{LayoutOptions, layout_parsed};

fn render_mindmap_svg_from_text(text: &str, diagram_id: &str) -> String {
    let engine = Engine::new();
    let parsed = futures::executor::block_on(engine.parse_diagram(text, ParseOptions::default()))
        .expect("parse ok")
        .expect("diagram detected");

    let layout_options = LayoutOptions::headless_svg_defaults();
    let out = layout_parsed(&parsed, &layout_options).expect("layout ok");

    render_layouted_svg(
        &out,
        layout_options.text_measurer.as_ref(),
        &SvgRenderOptions {
            diagram_id: Some(diagram_id.to_string()),
            ..SvgRenderOptions::default()
        },
    )
    .expect("render svg")
}

fn deep_mindmap_chain(depth: usize) -> String {
    let mut input = String::from("mindmap\n");
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
fn mindmap_public_json_layout_handles_deep_chain() {
    const DEPTH: usize = 1200;
    let source = deep_mindmap_chain(DEPTH);

    let engine = Engine::new();
    let parsed = futures::executor::block_on(engine.parse_diagram(&source, ParseOptions::strict()))
        .expect("parse ok")
        .expect("diagram detected");

    let out = layout_parsed(&parsed, &LayoutOptions::default()).expect("layout ok");
    let LayoutDiagram::MindmapDiagram(layout) = &out.layout else {
        panic!("expected MindmapDiagram layout");
    };

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
    let engine = Engine::new();
    let parsed = futures::executor::block_on(engine.parse_diagram(
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
    ))
    .expect("parse ok")
    .expect("diagram detected");

    let layout = layout_parsed(&parsed, &LayoutOptions::headless_svg_defaults())
        .expect("tidy-tree layout ok");
    let LayoutDiagram::MindmapDiagram(layout) = &layout.layout else {
        panic!("expected MindmapDiagram layout");
    };
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
    assert_eq!((root.x, root.y), (0.0, 20.0));
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
