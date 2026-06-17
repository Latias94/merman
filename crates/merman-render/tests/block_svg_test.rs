mod common;

use common::legacy_init_theme_compat_engine;
use merman_core::{Engine, ParseOptions};
use merman_render::model::LayoutDiagram;
use merman_render::svg::{SvgRenderOptions, render_block_diagram_svg};
use merman_render::{LayoutOptions, layout_parsed};

fn render_block_svg_from_text(text: &str) -> String {
    let engine = Engine::new();
    render_block_svg_from_text_with_engine(&engine, text)
}

fn render_block_svg_from_text_with_engine(engine: &Engine, text: &str) -> String {
    let parsed = futures::executor::block_on(engine.parse_diagram(text, ParseOptions::default()))
        .expect("parse ok")
        .expect("diagram detected");

    let out = layout_parsed(&parsed, &LayoutOptions::headless_svg_defaults()).expect("layout ok");
    let LayoutDiagram::BlockDiagram(layout) = &out.layout else {
        panic!("expected BlockDiagram layout");
    };

    render_block_diagram_svg(
        layout,
        &out.semantic,
        &out.meta.effective_config,
        &SvgRenderOptions::default(),
    )
    .expect("svg render ok")
}

fn deep_block_chain(depth: usize) -> String {
    let mut input = String::from("block\n");
    for level in 0..depth {
        input.push_str(&format!("block:n{level}[\"n{level}\"]\n"));
    }
    input.push_str("leaf[\"leaf\"]\n");
    for _ in 0..depth {
        input.push_str("end\n");
    }
    input
}

#[test]
fn block_svg_scopes_text_and_edge_colors_for_html_labels() {
    let svg = render_block_svg_from_text(
        r#"block
  A["Alpha"] --> B["Beta"]
"#,
    );

    assert!(
        !svg.contains("<style></style>"),
        "expected block SVG to emit scoped CSS instead of an empty style element"
    );
    assert!(
        svg.contains(r#"#merman .label text,#merman span,#merman p{fill:#333;color:#333;}"#),
        "expected block HTML/SVG labels to avoid inheriting host page text color"
    );
    assert!(
        svg.contains(r#"#merman .flowchart-link{stroke:#333333;fill:none;}"#),
        "expected block edges to carry their scoped stroke color"
    );
}

#[test]
fn block_public_svg_render_handles_deep_chain() {
    const DEPTH: usize = 1200;
    let svg = render_block_svg_from_text(&deep_block_chain(DEPTH));

    assert!(
        svg.contains(r#"id="merman-leaf""#),
        "expected deep Block leaf to render without stack-dependent traversal"
    );
}

#[test]
fn block_svg_honors_visible_edge_stroke_width_theme() {
    let engine = legacy_init_theme_compat_engine();
    let svg = render_block_svg_from_text_with_engine(
        &engine,
        r##"%%{init: {"themeVariables": {"strokeWidth": 4, "lineColor": "#112233"}}}%%
block
  A --> B
"##,
    );

    assert!(
        svg.contains(r#"#merman .edge-thickness-normal{stroke-width:4px;}"#),
        "expected shared Mermaid edge thickness CSS to reach visible Block edges: {svg}"
    );
    assert!(
        svg.contains(r#"class="edge-thickness-normal edge-pattern-solid edge-thickness-normal edge-pattern-solid flowchart-link LS-a1 LE-b1""#),
        "expected Block edge path to carry the themed edge-thickness-normal class: {svg}"
    );
}

#[test]
fn block_svg_uses_mermaid_11_15_dom_ids_and_html_label_shape() {
    let svg = render_block_svg_from_text(
        r#"block
  A["Alpha"] --> B["Beta"]
"#,
    );

    assert!(
        svg.contains(r#"id="merman-A""#),
        "expected Block node DOM id to be diagram-prefixed: {svg}"
    );
    assert!(
        svg.contains(r#"id="merman-1-A-B""#),
        "expected Block edge DOM id to be diagram-prefixed: {svg}"
    );
    assert!(
        svg.contains(r#"style="display: table-cell; white-space: nowrap; line-height: 1.5;"><span class="nodeLabel"><p>Alpha</p></span>"#),
        "expected Block node label to use Mermaid 11.15 XHTML paragraph shape: {svg}"
    );
}

#[test]
fn block_svg_keeps_blank_placeholder_label_paragraph() {
    let svg = render_block_svg_from_text(
        r#"block
  blockArrowId6<["   "]>(down)
"#,
    );

    assert!(
        svg.contains(r#"<span class="nodeLabel"><p>   </p></span>"#),
        "expected blank Block placeholder labels to keep Mermaid's paragraph child: {svg}"
    );
}

#[test]
fn block_svg_honors_configured_node_text_color() {
    let engine = legacy_init_theme_compat_engine();
    let svg = render_block_svg_from_text_with_engine(
        &engine,
        r##"%%{init: {"themeVariables": {"nodeTextColor": "#123456"}}}%%
block
  A["Alpha"]
"##,
    );

    assert!(
        svg.contains(r#"#merman .label text,#merman span,#merman p{fill:#123456;color:#123456;}"#),
        "expected nodeTextColor theme variable to drive block label color"
    );
}

#[test]
fn block_svg_fades_cluster_theme_colors() {
    let engine = legacy_init_theme_compat_engine();
    let svg = render_block_svg_from_text_with_engine(
        &engine,
        r##"%%{init: {"themeVariables": {"clusterBkg": "#112233", "clusterBorder": "#445566"}}}%%
block
  block
    A["Alpha"]
  end
"##,
    );

    assert!(
        svg.contains(
            r#"#merman .node .cluster{fill:rgba(17, 34, 51, 0.5);stroke:rgba(68, 85, 102, 0.2);stroke-width:1px;}"#
        ),
        "expected block composite cluster CSS to follow Mermaid 11.15 fade() colors"
    );
}
