use merman_render::sankey::layout_sankey_diagram;
use merman_render::svg::{SvgRenderOptions, render_sankey_diagram_svg};
use merman_render::text::DeterministicTextMeasurer;
use serde_json::json;

#[test]
fn sankey_svg_uses_configured_node_colors_and_outlined_labels() {
    let semantic = json!({
        "graph": {
            "nodes": [{"id": "A"}, {"id": "B"}],
            "links": [{"source": "A", "target": "B", "value": 10.0}]
        }
    });
    let config = json!({
        "sankey": {
            "nodeColors": {
                "A": "#112233",
                "B": "rebeccapurple"
            },
            "labelStyle": "outlined"
        }
    });
    let measurer = DeterministicTextMeasurer {
        char_width_factor: 8.0,
        line_height_factor: 16.0,
    };

    let layout = layout_sankey_diagram(&semantic, &config, &measurer).unwrap();
    let svg = render_sankey_diagram_svg(&layout, &semantic, &config, &SvgRenderOptions::default())
        .unwrap();

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
fn sankey_gradient_ids_are_prefixed_when_diagram_id_is_provided() {
    let semantic = json!({
        "graph": {
            "nodes": [{"id": "A"}, {"id": "B"}],
            "links": [{"source": "A", "target": "B", "value": 10.0}]
        }
    });
    let config = json!({});
    let measurer = DeterministicTextMeasurer {
        char_width_factor: 8.0,
        line_height_factor: 16.0,
    };

    let layout = layout_sankey_diagram(&semantic, &config, &measurer).unwrap();
    let svg = render_sankey_diagram_svg(
        &layout,
        &semantic,
        &config,
        &SvgRenderOptions {
            diagram_id: Some("sankey-inline".to_string()),
            ..SvgRenderOptions::default()
        },
    )
    .unwrap();

    assert!(
        svg.contains(r#"id="sankey-inline-linearGradient-3""#),
        "expected scoped Sankey gradient id: {svg}"
    );
    assert!(
        svg.contains(r#"stroke="url(#sankey-inline-linearGradient-3)""#),
        "expected scoped Sankey gradient reference: {svg}"
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
fn sankey_gradient_ids_keep_mermaid_style_without_diagram_id() {
    let semantic = json!({
        "graph": {
            "nodes": [{"id": "A"}, {"id": "B"}],
            "links": [{"source": "A", "target": "B", "value": 10.0}]
        }
    });
    let config = json!({});
    let measurer = DeterministicTextMeasurer {
        char_width_factor: 8.0,
        line_height_factor: 16.0,
    };

    let layout = layout_sankey_diagram(&semantic, &config, &measurer).unwrap();
    let svg = render_sankey_diagram_svg(&layout, &semantic, &config, &SvgRenderOptions::default())
        .unwrap();

    assert!(
        svg.contains(r#"id="linearGradient-3""#),
        "expected Mermaid-style Sankey gradient id without explicit diagram_id: {svg}"
    );
    assert!(
        svg.contains(r#"stroke="url(#linearGradient-3)""#),
        "expected Mermaid-style Sankey gradient reference without explicit diagram_id: {svg}"
    );
    assert!(
        !svg.contains(r#"id="sankey-linearGradient-3""#),
        "expected default rendering to avoid implicit resource id scoping: {svg}"
    );
}
