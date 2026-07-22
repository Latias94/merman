use merman_core::{Engine, ParseOptions};
use merman_render::LayoutOptions;
use merman_render::environment::RenderEnvironment;
use merman_render::family;
use merman_render::svg::{SvgDebugOptions, SvgRenderOptions};

fn render_sankey(source: &str, options: &SvgRenderOptions) -> String {
    let parsed = Engine::new()
        .parse_diagram_for_render_model_sync(source, ParseOptions::strict())
        .expect("parse ok")
        .expect("diagram detected");
    let session = RenderEnvironment::deterministic().begin_session().unwrap();
    family::prepare(parsed, &LayoutOptions::default(), session)
        .expect("layout ok")
        .render_svg(options, &SvgDebugOptions::default())
        .expect("render SVG")
        .svg()
        .to_owned()
}

#[test]
fn sankey_svg_uses_configured_node_colors_and_outlined_labels() {
    let svg = render_sankey(
        r##"---
config:
  sankey:
    nodeColors:
      A: "#112233"
      B: rebeccapurple
    labelStyle: outlined
---
sankey-beta
A,B,10
"##,
        &SvgRenderOptions::default(),
    );

    assert!(
        svg.contains(r##"fill="#112233""##),
        "expected node A to use configured fill: {svg}"
    );
    assert!(
        svg.contains(r#"fill="rebeccapurple""#),
        "expected node B to use configured fill: {svg}"
    );
    assert!(
        svg.contains(r##"stop-color="#112233""##),
        "expected source gradient stop to use configured color: {svg}"
    );
    assert!(
        svg.contains(r#"stop-color="rebeccapurple""#),
        "expected target gradient stop to use configured color: {svg}"
    );
    assert!(
        svg.contains(r#"class="sankey-label-bg""#),
        "expected outlined label background text: {svg}"
    );
    assert!(
        svg.contains(r#"class="sankey-label-fg""#),
        "expected outlined label foreground text: {svg}"
    );
    assert!(
        svg.contains(".sankey-label-bg"),
        "expected outlined label CSS: {svg}"
    );
}

#[test]
fn sankey_generated_ids_are_prefixed_when_diagram_id_is_provided() {
    let svg = render_sankey(
        "sankey-beta\nA,B,10\n",
        &SvgRenderOptions {
            diagram_id: Some("sankey-inline".to_string()),
            ..SvgRenderOptions::default()
        },
    );

    assert!(
        svg.contains(r#"id="sankey-inline-node-1""#),
        "expected scoped Sankey node id: {svg}"
    );
    assert!(
        svg.contains(r#"id="sankey-inline-linearGradient-3""#),
        "expected scoped Sankey gradient id: {svg}"
    );
    assert!(
        svg.contains(r#"stroke="url(#sankey-inline-linearGradient-3)""#),
        "expected scoped Sankey gradient reference: {svg}"
    );
    assert!(
        !svg.contains(r#"id="node-1""#),
        "expected no bare Sankey node id: {svg}"
    );
    assert!(
        !svg.contains(r#"id="linearGradient-3""#),
        "expected no bare Sankey gradient id: {svg}"
    );
    assert!(
        !svg.contains(r#"stroke="url(#linearGradient-3)""#),
        "expected no bare Sankey gradient reference: {svg}"
    );
}

#[test]
fn sankey_generated_ids_keep_mermaid_style_without_diagram_id() {
    let svg = render_sankey("sankey-beta\nA,B,10\n", &SvgRenderOptions::default());

    assert!(
        svg.contains(r#"id="node-1""#),
        "expected Mermaid-style Sankey node id without explicit diagram_id: {svg}"
    );
    assert!(
        svg.contains(r#"id="linearGradient-3""#),
        "expected Mermaid-style Sankey gradient id without explicit diagram_id: {svg}"
    );
    assert!(
        svg.contains(r#"stroke="url(#linearGradient-3)""#),
        "expected Mermaid-style Sankey gradient reference without explicit diagram_id: {svg}"
    );
    assert!(
        !svg.contains(r#"id="sankey-node-1""#),
        "expected default rendering to avoid implicit node id scoping: {svg}"
    );
    assert!(
        !svg.contains(r#"id="sankey-linearGradient-3""#),
        "expected default rendering to avoid implicit resource id scoping: {svg}"
    );
}
