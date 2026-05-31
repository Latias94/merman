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
