use merman_core::{Engine, ParseOptions};
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
