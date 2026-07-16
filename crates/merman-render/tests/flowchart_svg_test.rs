use futures::executor::block_on;
mod common;

use common::legacy_init_theme_compat_engine;
use merman_core::{Engine, MermaidConfig, ParseOptions};
use merman_render::model::LayoutDiagram;
use merman_render::svg::{
    SvgRenderOptions, render_flowchart_v2_debug_svg, render_flowchart_v2_svg,
};
use merman_render::text::{
    TextMeasurer, TextMetrics, TextStyle, VendoredFontMetricsTextMeasurer, WrapMode,
};
use merman_render::{LayoutOptions, layout_parsed};
use std::path::PathBuf;
#[cfg(feature = "ratex-math")]
use std::sync::Arc;

fn fmt(v: f64) -> String {
    if !v.is_finite() {
        return "0".to_string();
    }
    if v.abs() < 0.0005 {
        return "0".to_string();
    }
    let mut r = (v * 1000.0).round() / 1000.0;
    if r.abs() < 0.0005 {
        r = 0.0;
    }
    let mut s = format!("{r:.3}");
    if s.contains('.') {
        while s.ends_with('0') {
            s.pop();
        }
        if s.ends_with('.') {
            s.pop();
        }
    }
    if s == "-0" { "0".to_string() } else { s }
}

fn render_flowchart_svg_from_text(text: &str) -> String {
    render_flowchart_svg_from_text_with_engine(Engine::new(), text)
}

fn render_flowchart_svg_from_text_with_engine(engine: Engine, text: &str) -> String {
    let parsed = block_on(engine.parse_diagram(text, ParseOptions::default()))
        .expect("parse ok")
        .expect("diagram detected");

    let layout_options = LayoutOptions::default();
    let out = layout_parsed(&parsed, &layout_options).expect("layout ok");
    let LayoutDiagram::FlowchartV2(layout) = out.layout else {
        panic!("expected FlowchartV2 layout");
    };

    render_flowchart_v2_svg(
        &layout,
        &out.semantic,
        &out.meta.effective_config,
        out.meta.title.as_deref(),
        layout_options.text_measurer.as_ref(),
        &SvgRenderOptions::default(),
    )
    .expect("render svg")
}

#[test]
fn flowchart_missing_icon_uses_mermaid_unknown_icon_at_requested_size() {
    let svg = render_flowchart_svg_from_text(
        "flowchart TD\nA@{ icon: \"missing:icon\", label: \"Missing\" }\n",
    );
    let unknown_icon = r#"<svg xmlns="http://www.w3.org/2000/svg" width="48" height="48" viewBox="0 0 80 80"><g><rect width="80" height="80" style="fill: #087ebf; stroke-width: 0px;"/><text transform="translate(21.16 64.67)" style="fill: #fff; font-family: ArialMT, Arial; font-size: 67.75px;"><tspan x="0" y="0">?</tspan></text></g></svg>"#;

    assert!(svg.contains(unknown_icon), "{svg}");
}

#[test]
fn flowchart_svg_renders_one_logical_self_loop_edge() {
    let svg = render_flowchart_svg_from_text("flowchart TB\nA -->|again| A\n");

    assert_eq!(svg.matches(r#"id="merman-L_A_A_0""#).count(), 1, "{svg}");
    assert!(svg.contains(r#"data-id="L_A_A_0""#), "{svg}");
    assert!(
        !svg.contains("cyclic-special"),
        "Dagre self-loop segments must not leak into the rendered SVG: {svg}"
    );
}

fn flowchart_svg_edge_data_points(
    svg: &str,
    edge_id: &str,
) -> Vec<merman_render::model::LayoutPoint> {
    use base64::Engine as _;

    let marker = format!(r#"data-id="{edge_id}""#);
    let marker_pos = svg
        .find(&marker)
        .unwrap_or_else(|| panic!("edge {edge_id}: {svg}"));
    let tag_start = svg[..marker_pos].rfind("<path").expect("edge path start");
    let tag_end = svg[marker_pos..]
        .find('>')
        .map(|offset| marker_pos + offset)
        .expect("edge path end");
    let tag = &svg[tag_start..=tag_end];
    let attr = r#"data-points=""#;
    let value_start = tag.find(attr).expect("data-points") + attr.len();
    let value_end = tag[value_start..]
        .find('"')
        .map(|offset| value_start + offset)
        .expect("data-points end");
    let bytes = base64::engine::general_purpose::STANDARD
        .decode(&tag[value_start..value_end])
        .expect("data-points base64");
    serde_json::from_slice(&bytes).expect("data-points JSON")
}

#[test]
fn flowchart_svg_intersects_compact_self_loop_with_rendered_shape() {
    let text = "flowchart TD\nA[box] --> A\n";
    let engine = Engine::new();
    let parsed = block_on(engine.parse_diagram(text, ParseOptions::default()))
        .expect("parse ok")
        .expect("diagram detected");
    let layout_options = LayoutOptions::default();
    let out = layout_parsed(&parsed, &layout_options).expect("layout ok");
    let LayoutDiagram::FlowchartV2(layout) = out.layout else {
        panic!("expected FlowchartV2 layout");
    };
    let node = layout
        .nodes
        .iter()
        .find(|node| node.id == "A")
        .expect("node A");
    let edge = layout.edges.first().expect("self-loop edge");
    assert_eq!(edge.points.len(), 4);

    let outer = &edge.points[1];
    let dx = outer.x - node.x;
    let dy = outer.y - node.y;
    let scale = (node.width / 2.0 / dx.abs()).min(node.height / 2.0 / dy.abs());
    let expected_x = node.x + dx * scale;
    let expected_y = node.y + dy * scale;
    assert!(
        (edge.points[0].x - expected_x).abs() > 1e-3
            || (edge.points[0].y - expected_y).abs() > 1e-3,
        "the compact layout point should still be the provisional bbox endpoint"
    );

    let svg = render_flowchart_v2_svg(
        &layout,
        &out.semantic,
        &out.meta.effective_config,
        out.meta.title.as_deref(),
        layout_options.text_measurer.as_ref(),
        &SvgRenderOptions::default(),
    )
    .expect("render svg");
    let points = flowchart_svg_edge_data_points(&svg, &edge.id);
    assert_eq!(points.len(), 4);
    assert!((points[0].x - expected_x).abs() <= 1e-3, "{points:?}");
    assert!((points[0].y - expected_y).abs() <= 1e-3, "{points:?}");
}

#[test]
fn flowchart_svg_renders_regular_edges_before_compact_self_loops() {
    let svg =
        render_flowchart_svg_from_text("flowchart TD\nA loop-edge@--> A\nA normal-edge@--> B\n");

    let normal = svg.find(r#"data-id="normal-edge""#).expect("normal edge");
    let self_loop = svg.find(r#"data-id="loop-edge""#).expect("self-loop edge");
    assert!(
        normal < self_loop,
        "regular edges must render before compact self-loops"
    );
}

#[test]
fn flowchart_svg_renders_explicit_direction_cluster_as_recursive_root() {
    let svg = render_flowchart_svg_from_text(
        "flowchart TB\nsubgraph A\n  direction LR\n  a --> b\nend\na --> c\n",
    );

    assert_eq!(
        svg.matches(r#"<g class="root""#).count(),
        2,
        "the extracted cluster should render as one nested root: {svg}"
    );
    assert_eq!(
        svg.matches(r#"id="merman-A""#).count(),
        1,
        "the cluster should render exactly once inside its recursive root: {svg}"
    );
    assert_eq!(
        svg.matches(r#"id="merman-flowchart-a-"#).count(),
        1,
        "the extracted cluster's internal node should remain in the SVG DOM: {svg}"
    );
    assert_eq!(
        svg.matches(r#"id="merman-flowchart-b-"#).count(),
        1,
        "the extracted cluster's internal node should remain in the SVG DOM: {svg}"
    );
}

#[test]
fn flowchart_svg_renders_edge_to_ancestor_cluster_inside_that_root() {
    let svg = render_flowchart_svg_from_text(
        "flowchart LR\nsubgraph Outer\n  direction TB\n  subgraph Inner\n    direction LR\n    a --> b\n  end\n  b --> c\nend\nc --> Outer\n",
    );

    assert!(
        svg.contains(
            r#"<g class="root"><g class="clusters"/><g class="edgePaths"/><g class="edgeLabels"/><g class="nodes"><g class="root""#
        ),
        "the edge to an ancestor cluster must not be promoted into the top-level root: {svg}"
    );
    assert_eq!(
        svg.matches(r#"data-id="L_c_Outer_0""#).count(),
        2,
        "the ancestor edge should have one path and one label entry inside Outer: {svg}"
    );
}

#[test]
fn flowchart_svg_renders_recursive_cluster_self_loop_in_parent_root() {
    let svg = render_flowchart_svg_from_text(
        "flowchart TB\nsubgraph Outer\n  subgraph Inner\n    x\n  end\n  Inner --> Inner\nend\n",
    );

    let outer_cluster = svg.find(r#"id="merman-Outer""#).expect("Outer cluster");
    let self_loop = svg
        .find(r#"data-id="L_Inner_Inner_0""#)
        .expect("Inner self-loop");
    let inner_cluster = svg.find(r#"id="merman-Inner""#).expect("Inner cluster");
    assert!(
        outer_cluster < self_loop && self_loop < inner_cluster,
        "a recursive cluster self-loop should render in its parent root: {svg}"
    );
}

fn deep_flowchart_subgraph_chain(depth: usize) -> String {
    let mut input = String::from("flowchart TB\n");
    for level in 0..depth {
        input.push_str(&format!("subgraph S{level}\n"));
    }
    input.push_str("Leaf\n");
    for _ in 0..depth {
        input.push_str("end\n");
    }
    input
}

fn flowchart_svg_viewbox_values(svg: &str) -> [f64; 4] {
    let viewbox_start = svg.find(r#"viewBox=""#).expect("viewBox") + r#"viewBox=""#.len();
    let viewbox_end = svg[viewbox_start..].find('"').expect("viewBox end") + viewbox_start;
    let viewbox = &svg[viewbox_start..viewbox_end];
    let values = viewbox
        .split_whitespace()
        .map(|part| part.parse::<f64>().expect("viewBox number"))
        .collect::<Vec<_>>();
    assert_eq!(values.len(), 4, "expected four viewBox values: {viewbox}");
    [values[0], values[1], values[2], values[3]]
}

fn foreign_object_width_for_data_id(svg: &str, data_id: &str) -> f64 {
    let data_marker = format!(r#"<g class="label" data-id="{data_id}""#);
    let data_start = svg.find(&data_marker).expect("data-id marker");
    let width_marker = r#"<foreignObject width=""#;
    let width_start = svg[data_start..]
        .find(width_marker)
        .map(|idx| data_start + idx + width_marker.len())
        .expect("foreignObject width");
    let width_end = svg[width_start..]
        .find('"')
        .map(|idx| width_start + idx)
        .expect("foreignObject width end");
    svg[width_start..width_end]
        .parse::<f64>()
        .expect("foreignObject width number")
}

#[derive(Debug, Clone)]
struct WidthScaledTextMeasurer {
    inner: VendoredFontMetricsTextMeasurer,
    width_scale: f64,
}

impl WidthScaledTextMeasurer {
    fn new(width_scale: f64) -> Self {
        Self {
            inner: VendoredFontMetricsTextMeasurer::default(),
            width_scale,
        }
    }

    fn scale_width(&self, metrics: TextMetrics) -> TextMetrics {
        TextMetrics {
            width: metrics.width * self.width_scale,
            ..metrics
        }
    }
}

impl TextMeasurer for WidthScaledTextMeasurer {
    fn measure(&self, text: &str, style: &TextStyle) -> TextMetrics {
        self.scale_width(self.inner.measure(text, style))
    }

    fn measure_wrapped(
        &self,
        text: &str,
        style: &TextStyle,
        max_width: Option<f64>,
        wrap_mode: WrapMode,
    ) -> TextMetrics {
        self.scale_width(
            self.inner
                .measure_wrapped(text, style, max_width, wrap_mode),
        )
    }

    fn measure_wrapped_with_raw_width(
        &self,
        text: &str,
        style: &TextStyle,
        max_width: Option<f64>,
        wrap_mode: WrapMode,
    ) -> (TextMetrics, Option<f64>) {
        let (metrics, raw_width) = self
            .inner
            .measure_wrapped_with_raw_width(text, style, max_width, wrap_mode);
        (
            self.scale_width(metrics),
            raw_width.map(|width| width * self.width_scale),
        )
    }

    fn measure_wrapped_raw(
        &self,
        text: &str,
        style: &TextStyle,
        max_width: Option<f64>,
        wrap_mode: WrapMode,
    ) -> TextMetrics {
        self.scale_width(
            self.inner
                .measure_wrapped_raw(text, style, max_width, wrap_mode),
        )
    }
}

#[test]
fn flowchart_svg_security_level_controls_unsafe_click_href_rendering() {
    let strict = render_flowchart_svg_from_text(
        r#"%%{init: {"securityLevel": "strict"}}%%
flowchart TD
    A[Alpha] --> B[Beta]
    click A href "javascript:alert(1)" "tip" _blank
"#,
    );
    assert!(
        strict.contains(r#"<a transform=""#),
        "expected strict mode to keep Mermaid's anchor wrapper for a declared link: {strict}"
    );
    assert!(
        !strict.contains(r#"xlink:href="javascript:alert(1)""#),
        "expected strict mode to omit unsafe click href from SVG: {strict}"
    );
    assert!(
        !strict.contains(r#"xlink:href="about:blank""#),
        "expected Mermaid-compatible strict SVG to omit sanitized about:blank href: {strict}"
    );

    let loose = render_flowchart_svg_from_text_with_engine(
        Engine::new().with_site_config(MermaidConfig::from_value(serde_json::json!({
            "securityLevel": "loose"
        }))),
        r#"%%{init: {"securityLevel": "loose"}}%%
flowchart TD
    A[Alpha] --> B[Beta] --> C[Gamma]
    click A href "mailto:user@user.user" "mail" _blank
    click B href "notes://do-your-thing/id" "custom" _blank
    click C href "javascript:alert(1)" "script" _blank
"#,
    );
    assert!(
        loose.contains(r#"xlink:href="mailto:user@user.user""#),
        "expected loose mode to preserve Mermaid-renderable mailto links: {loose}"
    );
    assert!(
        loose.contains(r#"<a transform=""#),
        "expected loose mode to keep Mermaid's anchor wrappers for declared links: {loose}"
    );
    assert!(
        !loose.contains(r#"xlink:href="notes://do-your-thing/id""#)
            && !loose.contains(r#"xlink:href="javascript:alert(1)""#),
        "expected loose mode SVG sanitizer parity to omit unknown and script hrefs: {loose}"
    );
}

#[test]
fn flowchart_parse_for_render_model_handles_deep_subgraph_chain() {
    const DEPTH: usize = 1200;
    let text = deep_flowchart_subgraph_chain(DEPTH);

    let parsed = Engine::new()
        .parse_diagram_for_render_model_sync(&text, ParseOptions::strict())
        .expect("parse ok")
        .expect("diagram detected");

    assert_eq!(parsed.meta.diagram_type, "flowchart-v2");
}

#[test]
fn flowchart_layout_handles_deep_subgraph_chain() {
    const DEPTH: usize = 1200;
    let text = deep_flowchart_subgraph_chain(DEPTH);
    let engine = Engine::new();
    let parsed = block_on(engine.parse_diagram(&text, ParseOptions::strict()))
        .expect("parse ok")
        .expect("diagram detected");

    let out = layout_parsed(&parsed, &LayoutOptions::default()).expect("layout ok");
    let LayoutDiagram::FlowchartV2(layout) = out.layout else {
        panic!("expected FlowchartV2 layout");
    };

    assert!(layout.nodes.iter().any(|node| node.id == "Leaf"));
    assert!(layout.clusters.iter().any(|cluster| cluster.id == "S0"));
}

#[test]
fn flowchart_svg_handles_deep_subgraph_chain() {
    const DEPTH: usize = 1200;
    let text = deep_flowchart_subgraph_chain(DEPTH);

    let svg = render_flowchart_svg_from_text(&text);

    assert!(svg.contains(r#"id="merman-flowchart-Leaf-"#));
    assert!(svg.contains(r#"id="merman-S0""#));
}

#[test]
fn flowchart_diagram_padding_zero_is_preserved() {
    let default = render_flowchart_svg_from_text(
        r#"flowchart TB
A
"#,
    );
    let zero = render_flowchart_svg_from_text(
        r#"%%{init: {"flowchart": {"diagramPadding": 0}}}%%
flowchart TB
A
"#,
    );

    let default_viewbox = flowchart_svg_viewbox_values(&default);
    let zero_viewbox = flowchart_svg_viewbox_values(&zero);

    assert!(
        (default_viewbox[2] - zero_viewbox[2] - 16.0).abs() < 1e-6,
        "default diagramPadding=8 should add 16px width over diagramPadding=0; default={default_viewbox:?}, zero={zero_viewbox:?}"
    );
    assert!(
        (default_viewbox[3] - zero_viewbox[3] - 16.0).abs() < 1e-6,
        "default diagramPadding=8 should add 16px height over diagramPadding=0; default={default_viewbox:?}, zero={zero_viewbox:?}"
    );
}

#[test]
fn flowchart_svg_uses_configured_look_for_subgraph_clusters() {
    let svg = render_flowchart_svg_from_text(
        r#"%%{init: {"look": "neo"}}%%
flowchart TB
subgraph Group
  A
end
"#,
    );

    assert!(
        svg.contains(r#"<g class="cluster" id="merman-Group" data-look="neo""#),
        "expected flowchart subgraph cluster to propagate configured look: {svg}"
    );
    assert!(
        !svg.contains(r#"data-look="classic""#),
        "configured flowchart look must not leave classic DOM attributes: {svg}"
    );
}

#[test]
fn flowchart_debug_svg_includes_cluster_positioning_metadata() {
    let text = "flowchart TB\nsubgraph A[\"This is a very very very very very very very long title that should wrap\"]\n  a\nend\n";
    let engine = Engine::new();
    let parsed = block_on(engine.parse_diagram(text, ParseOptions::default()))
        .expect("parse ok")
        .expect("diagram detected");

    let out = layout_parsed(&parsed, &LayoutOptions::default()).expect("layout ok");
    let LayoutDiagram::FlowchartV2(layout) = out.layout else {
        panic!("expected FlowchartV2 layout");
    };

    let cluster = layout
        .clusters
        .iter()
        .find(|c| c.id == "A")
        .expect("cluster A");

    let svg = render_flowchart_v2_debug_svg(&layout, &SvgRenderOptions::default());
    let expected = format!(
        r#"id="cluster-A" data-diff="{}" data-offset-y="{}""#,
        fmt(cluster.diff),
        fmt(cluster.offset_y)
    );
    assert!(
        svg.contains(&expected),
        "expected debug SVG to include cluster diff/offset-y metadata"
    );
}

#[test]
fn flowchart_v2_fontawesome_edge_label_width_uses_nominal_icon_boundary() {
    // Mermaid 11.15 uses a clean 1.25em inline box for FontAwesome labels instead of
    // browser-specific per-icon advance drift.
    let mmd_path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .join("fixtures")
        .join("flowchart")
        .join("upstream_flowchart_v2_icons_in_edge_labels_spec.mmd");
    let text = std::fs::read_to_string(&mmd_path).expect("read fixture .mmd");

    let engine = Engine::new();
    let parsed = block_on(engine.parse_diagram(&text, ParseOptions::default()))
        .expect("parse ok")
        .expect("diagram detected");

    let out = layout_parsed(
        &parsed,
        &LayoutOptions {
            text_measurer: std::sync::Arc::new(VendoredFontMetricsTextMeasurer::default()),
            ..Default::default()
        },
    )
    .expect("layout ok");
    let LayoutDiagram::FlowchartV2(layout) = out.layout else {
        panic!("expected FlowchartV2 layout");
    };

    let edge = layout
        .edges
        .iter()
        .find(|e| e.id == "L_C_F_0")
        .expect("edge L_C_F_0");
    let lbl = edge.label.as_ref().expect("edge label");
    assert_eq!(lbl.width, 49.03125);
    assert_eq!(lbl.height, 24.0);
}

#[test]
fn flowchart_wrapping_width_is_reflected_in_html_label_max_width_style() {
    let text = "%%{init: {\"flowchart\": {\"htmlLabels\": true, \"wrappingWidth\": 120}}}%%\nflowchart TB\nA[\"Hello\"]\n";
    let engine = Engine::new();
    let parsed = block_on(engine.parse_diagram(text, ParseOptions::default()))
        .expect("parse ok")
        .expect("diagram detected");

    let layout_options = LayoutOptions::default();
    let out = layout_parsed(&parsed, &layout_options).expect("layout ok");
    let LayoutDiagram::FlowchartV2(layout) = out.layout else {
        panic!("expected FlowchartV2 layout");
    };

    let svg = render_flowchart_v2_svg(
        &layout,
        &out.semantic,
        &out.meta.effective_config,
        out.meta.title.as_deref(),
        layout_options.text_measurer.as_ref(),
        &SvgRenderOptions::default(),
    )
    .expect("render svg");
    assert!(
        svg.contains("max-width: 120px"),
        "expected flowchart.wrappingWidth=120 to affect html label max-width style"
    );
}

#[test]
fn flowchart_html_node_labels_wrap_at_mermaid_default_width() {
    let svg = render_flowchart_svg_from_text(
        r##"flowchart LR
    Security[Import / WebSurface / Data Egress Gates] --> PDF
"##,
    );

    assert!(
        svg.contains(
            r#"foreignObject width="200" height="48" style="overflow: visible;"><div xmlns="http://www.w3.org/1999/xhtml" style="display: table; white-space: break-spaces; line-height: 1.5; max-width: 200px; text-align: center; width: 200px;""#
        ),
        "expected long flowchart HTML node label to wrap at Mermaid's default 200px width: {svg}"
    );
}

#[test]
fn flowchart_html_labels_allow_browser_font_fallback_overflow() {
    let text = r#"flowchart TD
    A[Start] --> B{Condition?}
    B -->|Yes| C[Execute]
    B -->|No| D[End]
    C --> D"#;
    let engine = Engine::new();
    let parsed = block_on(engine.parse_diagram(text, ParseOptions::default()))
        .expect("parse ok")
        .expect("diagram detected");
    let layout_options = LayoutOptions {
        text_measurer: std::sync::Arc::new(VendoredFontMetricsTextMeasurer::default()),
        ..Default::default()
    };
    let out = layout_parsed(&parsed, &layout_options).expect("layout ok");
    let LayoutDiagram::FlowchartV2(layout) = out.layout else {
        panic!("expected FlowchartV2 layout");
    };

    let svg = render_flowchart_v2_svg(
        &layout,
        &out.semantic,
        &out.meta.effective_config,
        out.meta.title.as_deref(),
        layout_options.text_measurer.as_ref(),
        &SvgRenderOptions::default(),
    )
    .expect("render svg");

    assert!(
        svg.contains(r#"<foreignObject width="35.015625" height="24" style="overflow: visible;"><div xmlns="http://www.w3.org/1999/xhtml" style="display: table-cell; white-space: nowrap; line-height: 1.5; max-width: 200px; text-align: center;"><span class="nodeLabel"><p>Start</p></span>"#),
        "expected Start label foreignObject to remain non-clipping for browser font fallback: {svg}"
    );
    assert!(
        svg.contains(r#"<foreignObject width="74.484375" height="24" style="overflow: visible;"><div xmlns="http://www.w3.org/1999/xhtml" style="display: table-cell; white-space: nowrap; line-height: 1.5; max-width: 200px; text-align: center;"><span class="nodeLabel"><p>Condition?</p></span>"#),
        "expected Condition? label foreignObject to remain non-clipping for browser font fallback: {svg}"
    );
    assert!(
        svg.contains(r#"<foreignObject width="26.65625" height="24" style="overflow: visible;"><div xmlns="http://www.w3.org/1999/xhtml" class="labelBkg" style="display: table-cell; white-space: nowrap; line-height: 1.5; max-width: 200px; text-align: center;"><span class="edgeLabel"><p>Yes</p></span>"#),
        "expected edge labels to use the same non-clipping foreignObject contract: {svg}"
    );
}

#[test]
fn flowchart_layout_uses_host_text_measurer_for_font_widths() {
    let text = r#"flowchart TD
    A[Start] --> B{Condition?}
    B -->|Yes| C[Execute]"#;
    let engine = Engine::new();
    let parsed = block_on(engine.parse_diagram(text, ParseOptions::default()))
        .expect("parse ok")
        .expect("diagram detected");

    let baseline_options = LayoutOptions {
        text_measurer: std::sync::Arc::new(VendoredFontMetricsTextMeasurer::default()),
        ..Default::default()
    };
    let wide_options = LayoutOptions {
        text_measurer: std::sync::Arc::new(WidthScaledTextMeasurer::new(1.35)),
        ..Default::default()
    };

    let baseline_out = layout_parsed(&parsed, &baseline_options).expect("baseline layout ok");
    let wide_out = layout_parsed(&parsed, &wide_options).expect("wide layout ok");

    let LayoutDiagram::FlowchartV2(baseline_layout) = baseline_out.layout else {
        panic!("expected FlowchartV2 layout");
    };
    let LayoutDiagram::FlowchartV2(wide_layout) = wide_out.layout else {
        panic!("expected FlowchartV2 layout");
    };

    let baseline_condition = baseline_layout
        .nodes
        .iter()
        .find(|node| node.id == "B")
        .expect("baseline Condition? node");
    let wide_condition = wide_layout
        .nodes
        .iter()
        .find(|node| node.id == "B")
        .expect("wide Condition? node");
    let baseline_label_width = baseline_condition
        .label_width
        .expect("baseline Condition? label width");
    let wide_label_width = wide_condition
        .label_width
        .expect("wide Condition? label width");

    assert!(
        wide_label_width > baseline_label_width * 1.3,
        "expected host-provided wider font metrics to affect flowchart label layout; baseline={baseline_label_width}, wide={wide_label_width}"
    );
    assert!(
        wide_condition.width > baseline_condition.width,
        "expected host-provided wider font metrics to enlarge the node shape; baseline={}, wide={}",
        baseline_condition.width,
        wide_condition.width
    );
}

#[test]
fn flowchart_svg_honors_mermaid_11_15_numeric_stroke_width_theme() {
    let svg = render_flowchart_svg_from_text_with_engine(
        legacy_init_theme_compat_engine(),
        r##"%%{init: {"themeVariables": {"strokeWidth": 4, "lineColor": "#112233", "nodeBorder": "#445566"}}}%%
flowchart TB
    A --> B
"##,
    );

    assert!(
        svg.contains(
            r#"#merman .node rect,#merman .node circle,#merman .node ellipse,#merman .node polygon,#merman .node path{fill:#ECECFF;stroke:#445566;stroke-width:4px;}"#
        ),
        "expected numeric themeVariables.strokeWidth to drive Flowchart node stroke width CSS: {svg}"
    );
    assert!(
        svg.contains(r#"#merman .edgePath .path{stroke:#112233;stroke-width:4px;}"#),
        "expected numeric themeVariables.strokeWidth to drive Flowchart edge path stroke width CSS: {svg}"
    );
    assert!(
        svg.contains(r#"#merman .edge-thickness-normal{stroke-width:4px;}"#),
        "expected visible Flowchart edge class width to follow Mermaid 11.15 theme strokeWidth: {svg}"
    );
    assert!(
        svg.contains(
            r#"class="edge-thickness-normal edge-pattern-solid edge-thickness-normal edge-pattern-solid flowchart-link""#
        ),
        "expected the visible Flowchart edge path to carry the themed edge-thickness-normal class: {svg}"
    );
}

#[test]
fn flowchart_link_style_stroke_width_overrides_theme_default_edge_width() {
    let svg = render_flowchart_svg_from_text_with_engine(
        legacy_init_theme_compat_engine(),
        r##"%%{init: {"themeVariables": {"strokeWidth": 4, "lineColor": "#112233"}}}%%
flowchart TB
    A --> B
    linkStyle 0 stroke-width:7px,stroke:#abcdef
"##,
    );

    assert!(
        svg.contains(r#"#merman .edge-thickness-normal{stroke-width:4px;}"#),
        "expected themeVariables.strokeWidth to remain the default Flowchart edge width: {svg}"
    );

    let edge_start = svg.find(r#"id="merman-L_A_B_0""#).expect("edge path");
    let edge_end = svg[edge_start..].find("/>").expect("edge path end") + edge_start;
    let edge_chunk = &svg[edge_start..edge_end];

    assert!(
        edge_chunk.contains("stroke-width:7px"),
        "expected linkStyle stroke-width to stay on the visible Flowchart edge path: {edge_chunk}"
    );
    assert!(
        edge_chunk.contains("stroke:#abcdef"),
        "expected linkStyle stroke color to stay on the visible Flowchart edge path: {edge_chunk}"
    );
}

#[test]
fn flowchart_svg_honors_node_text_color_theme_variable() {
    let svg = render_flowchart_svg_from_text_with_engine(
        legacy_init_theme_compat_engine(),
        r##"%%{init: {"themeVariables": {"mainBkg": "#111827", "nodeTextColor": "#f8fafc", "textColor": "#fde68a"}}}%%
flowchart TD
    A[Dark Node] --> B[Other]
"##,
    );

    assert!(
        svg.contains(
            r##"#merman .label{font-family:"trebuchet ms",verdana,arial,sans-serif;color:#f8fafc;}"##
        ),
        "expected themeVariables.nodeTextColor to drive Flowchart label color CSS: {svg}"
    );
    assert!(
        svg.contains(r##"#merman .label text,#merman span{fill:#f8fafc;color:#f8fafc;}"##),
        "expected themeVariables.nodeTextColor to drive Flowchart label text fill CSS: {svg}"
    );
    assert!(
        svg.contains(
            r##"#merman{font-family:"trebuchet ms",verdana,arial,sans-serif;font-size:16px;fill:#fde68a;}"##
        ),
        "expected themeVariables.textColor to continue driving root SVG text fill CSS: {svg}"
    );
}

#[test]
fn flowchart_svg_uses_extended_theme_derived_secondary_color_overrides() {
    let svg = render_flowchart_svg_from_text_with_engine(
        legacy_init_theme_compat_engine(),
        r##"%%{init: {"theme": "redux", "themeVariables": {"primaryColor": "#123456"}}}%%
flowchart TD
    A[Redux Node] -- Edge Label --> B[Other]
"##,
    );

    assert!(
        svg.contains("fill:#ffffff;stroke:#28253D;stroke-width:2px;"),
        "expected Mermaid redux mainBkg default to remain the visible node fill: {svg}"
    );
    assert!(
        svg.contains(
            "#merman .edgeLabel{background-color:hsl(90, 65.3846153846%, 20.3921568627%);"
        ),
        "expected Mermaid redux primaryColor override to derive visible secondary edge-label color: {svg}"
    );
}

#[test]
fn flowchart_neo_uses_configurable_radius_shadow_and_round_edges() {
    let svg = render_flowchart_svg_from_text_with_engine(
        legacy_init_theme_compat_engine(),
        r##"%%{init: {"theme": "redux", "look": "neo", "flowchart": {"curve": "rounded", "edgeCornerRadius": 14}}}%%
flowchart TD
    A[Start] --> B{Condition?}
    B -->|Yes| C[Execute]
    B -->|No| D[End]
    C --> D
"##,
    );

    assert!(
        svg.contains(r#"rx="12" ry="12""#),
        "expected Redux radius on ordinary Neo rectangles: {svg}"
    );
    assert!(
        svg.contains(r#".node[data-look="neo"] .label-container{filter:url(#merman-drop-shadow);stroke-linejoin:round;}"#),
        "expected scoped Redux node shadow: {svg}"
    );
    assert!(
        svg.contains(
            r#".flowchart-link[data-look="neo"]{stroke-linecap:round;stroke-linejoin:round;}"#
        ),
        "expected rounded Neo edge caps and joins: {svg}"
    );
    assert!(
        svg.contains(".edgeLabel rect{opacity:1;}"),
        "expected opaque Neo label backgrounds to mask the edge cleanly: {svg}"
    );
}

#[test]
fn flowchart_node_labels_use_root_html_labels_when_flowchart_html_labels_is_false() {
    let text =
        "%%{init: {\"flowchart\": {\"htmlLabels\": false}}}%%\nflowchart TB\nA[\"`**Node**`\"]\n";
    let engine = Engine::new();
    let parsed = block_on(engine.parse_diagram(text, ParseOptions::default()))
        .expect("parse ok")
        .expect("diagram detected");

    let layout_options = LayoutOptions::default();
    let out = layout_parsed(&parsed, &layout_options).expect("layout ok");
    let LayoutDiagram::FlowchartV2(layout) = out.layout else {
        panic!("expected FlowchartV2 layout");
    };

    let svg = render_flowchart_v2_svg(
        &layout,
        &out.semantic,
        &out.meta.effective_config,
        out.meta.title.as_deref(),
        layout_options.text_measurer.as_ref(),
        &SvgRenderOptions::default(),
    )
    .expect("render svg");
    assert!(
        svg.contains("<foreignObject "),
        "expected node label to remain in the HTML label path: {svg}"
    );
    assert!(
        svg.contains(r#"class="nodeLabel markdown-node-label""#),
        "expected markdown node label class in HTML label path: {svg}"
    );
}

#[test]
fn flowchart_classic_hexagon_renders_polygon_container() {
    let text = "flowchart TB\nA{{\"`**Hex**`\"}}\n";
    let engine = Engine::new();
    let parsed = block_on(engine.parse_diagram(text, ParseOptions::default()))
        .expect("parse ok")
        .expect("diagram detected");

    let layout_options = LayoutOptions::default();
    let out = layout_parsed(&parsed, &layout_options).expect("layout ok");
    let LayoutDiagram::FlowchartV2(layout) = out.layout else {
        panic!("expected FlowchartV2 layout");
    };

    let svg = render_flowchart_v2_svg(
        &layout,
        &out.semantic,
        &out.meta.effective_config,
        out.meta.title.as_deref(),
        layout_options.text_measurer.as_ref(),
        &SvgRenderOptions::default(),
    )
    .expect("render svg");
    assert!(
        svg.contains(r#"<polygon "#) && svg.contains(r#"class="label-container""#),
        "expected classic hexagon to render as a polygon label-container: {svg}"
    );
    assert!(
        !svg.contains(r#"<g class="basic label-container"><path "#),
        "expected classic hexagon not to use the hand-drawn RoughJS path branch: {svg}"
    );
}

#[test]
fn flowchart_no_label_special_shapes_render_outer_path_group() {
    let text = "flowchart TB\nA@{ shape: stop }\nB@{ shape: lightning-bolt }\nC@{ shape: crossed-circle }\n";
    let engine = Engine::new();
    let parsed = block_on(engine.parse_diagram(text, ParseOptions::default()))
        .expect("parse ok")
        .expect("diagram detected");

    let layout_options = LayoutOptions::default();
    let out = layout_parsed(&parsed, &layout_options).expect("layout ok");
    let LayoutDiagram::FlowchartV2(layout) = out.layout else {
        panic!("expected FlowchartV2 layout");
    };

    let svg = render_flowchart_v2_svg(
        &layout,
        &out.semantic,
        &out.meta.effective_config,
        out.meta.title.as_deref(),
        layout_options.text_measurer.as_ref(),
        &SvgRenderOptions::default(),
    )
    .expect("render svg");
    assert!(
        svg.matches(r#"class="outer-path""#).count() >= 3,
        "expected no-label special shapes to expose Mermaid 11.15 outer-path groups: {svg}"
    );
}

#[test]
fn flowchart_hourglass_preserves_markdown_label_class_after_clearing_label() {
    let text = r#"flowchart TB
A@{ shape: hourglass, label: "Hourglass label" }
"#;
    let engine = Engine::new();
    let parsed = block_on(engine.parse_diagram(text, ParseOptions::default()))
        .expect("parse ok")
        .expect("diagram detected");

    let layout_options = LayoutOptions::default();
    let out = layout_parsed(&parsed, &layout_options).expect("layout ok");
    let LayoutDiagram::FlowchartV2(layout) = out.layout else {
        panic!("expected FlowchartV2 layout");
    };

    let svg = render_flowchart_v2_svg(
        &layout,
        &out.semantic,
        &out.meta.effective_config,
        out.meta.title.as_deref(),
        layout_options.text_measurer.as_ref(),
        &SvgRenderOptions::default(),
    )
    .expect("render svg");

    assert!(
        svg.contains(r#"<span class="nodeLabel markdown-node-label"></span>"#),
        "expected Mermaid 11.15 hourglass to keep markdown label class on the empty label: {svg}"
    );
}

#[test]
fn flowchart_base_theme_renders_root_gradient() {
    let text = r##"%%{init: {"theme": "base", "themeVariables": {"primaryColor": "#BB2528", "primaryBorderColor": "#7C0000", "secondaryColor": "#006100"}}}%%
flowchart TB
A --> B
"##;
    let engine = legacy_init_theme_compat_engine();
    let parsed = block_on(engine.parse_diagram(text, ParseOptions::default()))
        .expect("parse ok")
        .expect("diagram detected");

    let layout_options = LayoutOptions::default();
    let out = layout_parsed(&parsed, &layout_options).expect("layout ok");
    let LayoutDiagram::FlowchartV2(layout) = out.layout else {
        panic!("expected FlowchartV2 layout");
    };

    let svg = render_flowchart_v2_svg(
        &layout,
        &out.semantic,
        &out.meta.effective_config,
        out.meta.title.as_deref(),
        layout_options.text_measurer.as_ref(),
        &SvgRenderOptions {
            diagram_id: Some("flowchart_theme_gradient".to_string()),
            ..SvgRenderOptions::default()
        },
    )
    .expect("render svg");

    assert!(
        svg.contains(r#"<linearGradient id="flowchart_theme_gradient-gradient" gradientUnits="objectBoundingBox" x1="0%" y1="0%" x2="100%" y2="0%">"#),
        "expected Mermaid 11.15 root gradient element: {svg}"
    );
    assert!(
        svg.contains(r##"<stop offset="0%" stop-color="#7C0000" stop-opacity="1"/>"##),
        "expected gradientStart to use primaryBorderColor: {svg}"
    );
    assert!(
        svg.contains(
            r#"<stop offset="100%" stop-color="hsl(120, 60%, 9.0196078431%)" stop-opacity="1"/>"#
        ),
        "expected gradientStop to use derived secondaryBorderColor: {svg}"
    );
}

#[test]
fn flowchart_note_shape_renders_note_label_class() {
    let text = r#"flowchart TB
A@{ shape: note, label: "Note" }
"#;
    let engine = Engine::new();
    let parsed = block_on(engine.parse_diagram(text, ParseOptions::default()))
        .expect("parse ok")
        .expect("diagram detected");

    let layout_options = LayoutOptions::default();
    let out = layout_parsed(&parsed, &layout_options).expect("layout ok");
    let LayoutDiagram::FlowchartV2(layout) = out.layout else {
        panic!("expected FlowchartV2 layout");
    };

    let svg = render_flowchart_v2_svg(
        &layout,
        &out.semantic,
        &out.meta.effective_config,
        out.meta.title.as_deref(),
        layout_options.text_measurer.as_ref(),
        &SvgRenderOptions::default(),
    )
    .expect("render svg");

    assert!(
        svg.contains(r#"<g class="label noteLabel""#),
        "expected Mermaid 11.15 note labels to carry the noteLabel class: {svg}"
    );
}

#[test]
fn flowchart_svg_markdown_node_labels_wrap_when_html_labels_false() {
    let text = r#"%%{init: {"htmlLabels": false, "flowchart": {"wrappingWidth": 80}}}%%
flowchart TB
A["`**Alpha beta gamma delta epsilon zeta eta theta**`"]
"#;
    let engine = Engine::new();
    let parsed = block_on(engine.parse_diagram(text, ParseOptions::default()))
        .expect("parse ok")
        .expect("diagram detected");

    let layout_options = LayoutOptions::default();
    let out = layout_parsed(&parsed, &layout_options).expect("layout ok");
    let LayoutDiagram::FlowchartV2(layout) = out.layout else {
        panic!("expected FlowchartV2 layout");
    };

    let svg = render_flowchart_v2_svg(
        &layout,
        &out.semantic,
        &out.meta.effective_config,
        out.meta.title.as_deref(),
        layout_options.text_measurer.as_ref(),
        &SvgRenderOptions::default(),
    )
    .expect("render svg");

    assert!(
        svg.matches(r#"class="row text-outer-tspan""#).count() > 1,
        "expected Mermaid 11.15 SVG markdown node labels to wrap into multiple rows: {svg}"
    );
}

#[test]
fn flowchart_svg_plain_subgraph_titles_do_not_wrap_when_html_labels_false() {
    let text = r#"%%{init: {"htmlLabels": false, "flowchart": {"htmlLabels": false}}}%%
flowchart TB
subgraph A[SupercalifragilisticexpialidociousSupercalifragilisticexpialidocious]
  x
end
"#;
    let engine = Engine::new();
    let parsed = block_on(engine.parse_diagram(text, ParseOptions::default()))
        .expect("parse ok")
        .expect("diagram detected");

    let layout_options = LayoutOptions::default();
    let out = layout_parsed(&parsed, &layout_options).expect("layout ok");
    let LayoutDiagram::FlowchartV2(layout) = out.layout else {
        panic!("expected FlowchartV2 layout");
    };

    let svg = render_flowchart_v2_svg(
        &layout,
        &out.semantic,
        &out.meta.effective_config,
        out.meta.title.as_deref(),
        layout_options.text_measurer.as_ref(),
        &SvgRenderOptions::default(),
    )
    .expect("render svg");

    let cluster_start = svg.find(r#"<g class="cluster""#).expect("cluster");
    let cluster_label_start = svg[cluster_start..]
        .find(r#"<g class="cluster-label""#)
        .map(|idx| cluster_start + idx)
        .expect("cluster label");
    let cluster_label_end = svg[cluster_label_start..]
        .find(r#"</text>"#)
        .map(|idx| cluster_label_start + idx)
        .expect("cluster label text end");
    let cluster_label = &svg[cluster_label_start..cluster_label_end];

    assert_eq!(
        cluster_label.matches("text-outer-tspan").count(),
        1,
        "expected Mermaid 11.15 plain SVG subgraph titles to remain one unwrapped row: {cluster_label}"
    );
}

#[test]
fn flowchart_html_labels_treat_decoded_backslash_n_as_line_break() {
    let text = "%%{init: {\"flowchart\": {\"htmlLabels\": true}}}%%\nflowchart TB\nA[\"line1\\\\nline2\"]\n";
    let engine = Engine::new();
    let parsed = block_on(engine.parse_diagram(text, ParseOptions::default()))
        .expect("parse ok")
        .expect("diagram detected");

    let layout_options = LayoutOptions::default();
    let out = layout_parsed(&parsed, &layout_options).expect("layout ok");
    let LayoutDiagram::FlowchartV2(layout) = out.layout else {
        panic!("expected FlowchartV2 layout");
    };

    let svg = render_flowchart_v2_svg(
        &layout,
        &out.semantic,
        &out.meta.effective_config,
        out.meta.title.as_deref(),
        layout_options.text_measurer.as_ref(),
        &SvgRenderOptions::default(),
    )
    .expect("render svg");
    assert!(
        svg.contains("<p>line1<br />line2</p>"),
        "expected Mermaid 11.15 nonMarkdownToHTML to treat decoded `\\\\n` as a line break: {svg}"
    );
    assert!(
        !svg.contains("line1\\nline2"),
        "expected output to not contain a literal backslash-n escape"
    );
}

#[test]
fn flowchart_html_single_image_label_uses_paragraph_wrapper() {
    let text = r#"flowchart TB
B[<img src='https://mermaid.js.org/mermaid-logo.svg'>]
"#;
    let engine = Engine::new();
    let parsed = block_on(engine.parse_diagram(text, ParseOptions::default()))
        .expect("parse ok")
        .expect("diagram detected");

    let layout_options = LayoutOptions::default();
    let out = layout_parsed(&parsed, &layout_options).expect("layout ok");
    let LayoutDiagram::FlowchartV2(layout) = out.layout else {
        panic!("expected FlowchartV2 layout");
    };

    let svg = render_flowchart_v2_svg(
        &layout,
        &out.semantic,
        &out.meta.effective_config,
        out.meta.title.as_deref(),
        layout_options.text_measurer.as_ref(),
        &SvgRenderOptions::default(),
    )
    .expect("render svg");

    assert!(
        svg.contains(r#"<span class="nodeLabel"><p><img "#),
        "expected Mermaid 11.15 non-markdown image labels to keep the nonMarkdownToHTML paragraph wrapper: {svg}"
    );
}

#[test]
fn flowchart_image_shape_label_bbox_includes_mermaid_padding() {
    let text = r#"flowchart TD
A@{ img: "https://mermaid.js.org/favicon.svg", label: "My example image label", pos: "t", h: 60, constraint: "on" }
"#;
    let engine = Engine::new();
    let parsed = block_on(engine.parse_diagram(text, ParseOptions::default()))
        .expect("parse ok")
        .expect("diagram detected");

    let layout_options = LayoutOptions {
        text_measurer: std::sync::Arc::new(VendoredFontMetricsTextMeasurer::default()),
        ..Default::default()
    };
    let out = layout_parsed(&parsed, &layout_options).expect("layout ok");
    let LayoutDiagram::FlowchartV2(layout) = out.layout else {
        panic!("expected FlowchartV2 layout");
    };

    let node = layout.nodes.iter().find(|n| n.id == "A").expect("node A");
    assert_eq!(node.width, 176.984375);
    assert_eq!(node.height, 96.0);

    let svg = render_flowchart_v2_svg(
        &layout,
        &out.semantic,
        &out.meta.effective_config,
        out.meta.title.as_deref(),
        layout_options.text_measurer.as_ref(),
        &SvgRenderOptions::default(),
    )
    .expect("render svg");

    assert!(
        svg.contains(
            r#"<foreignObject width="176.984375" height="28" style="overflow: visible;">"#
        ),
        "expected image-shape label bbox to include Mermaid 11.15 paragraph padding: {svg}"
    );
    assert!(
        svg.contains(r#"<image href="https://mermaid.js.org/favicon.svg" width="60" height="60" preserveAspectRatio="none" transform="translate(-30,-12)"/>"#),
        "expected top image placement to use the padded label bbox: {svg}"
    );
}

#[test]
fn flowchart_shape_data_multiline_markdown_trims_trailing_block_newline() {
    let text = r#"flowchart TB
A@{
  label: |
    This is a
    multiline string
}
"#;
    let engine = Engine::new();
    let parsed = block_on(engine.parse_diagram(text, ParseOptions::default()))
        .expect("parse ok")
        .expect("diagram detected");

    let layout_options = LayoutOptions::default();
    let out = layout_parsed(&parsed, &layout_options).expect("layout ok");
    let LayoutDiagram::FlowchartV2(layout) = out.layout else {
        panic!("expected FlowchartV2 layout");
    };

    let svg = render_flowchart_v2_svg(
        &layout,
        &out.semantic,
        &out.meta.effective_config,
        out.meta.title.as_deref(),
        layout_options.text_measurer.as_ref(),
        &SvgRenderOptions::default(),
    )
    .expect("render svg");

    assert_eq!(
        svg.matches("<br").count(),
        1,
        "expected Mermaid 11.15 shapeData block labels to ignore the YAML trailing newline: {svg}"
    );
}

#[test]
fn flowchart_html_plain_multiline_labels_trim_source_indentation() {
    let text = "%%{init: {\"flowchart\": {\"htmlLabels\": true}}}%%\nflowchart TB\nA[\"\n  First\n      Second\n  \"]\n";
    let engine = Engine::new();
    let parsed = block_on(engine.parse_diagram(text, ParseOptions::default()))
        .expect("parse ok")
        .expect("diagram detected");

    let layout_options = LayoutOptions::default();
    let out = layout_parsed(&parsed, &layout_options).expect("layout ok");
    let LayoutDiagram::FlowchartV2(layout) = out.layout else {
        panic!("expected FlowchartV2 layout");
    };

    let svg = render_flowchart_v2_svg(
        &layout,
        &out.semantic,
        &out.meta.effective_config,
        out.meta.title.as_deref(),
        layout_options.text_measurer.as_ref(),
        &SvgRenderOptions::default(),
    )
    .expect("render svg");
    assert!(
        svg.contains("<p>First<br />Second</p>"),
        "expected plain multiline HTML label to trim indentation: {svg}"
    );
    assert!(
        !svg.contains("<br />      Second"),
        "expected no source indentation after HTML line break"
    );
}

#[test]
fn flowchart_html_plain_node_labels_can_span_indented_lines() {
    let svg = render_flowchart_svg_from_text(
        "     flowchart TB
     foo[**Bold Foo**] --> bar
     bar[Multiline
     bar]",
    );

    assert!(
        svg.contains("<p>Multiline<br />bar</p>"),
        "expected indented multiline node label to render as an HTML line break: {svg}"
    );
    assert!(
        svg.contains("<p>**Bold Foo**</p>"),
        "expected plain flowchart labels to keep Markdown delimiters literal like Mermaid's nonMarkdownToHTML: {svg}"
    );
    assert!(
        !svg.contains("<strong>Bold Foo</strong>"),
        "plain flowchart text labels must not be treated as Markdown strings: {svg}"
    );
}

#[test]
fn flowchart_svg_plain_text_labels_do_not_apply_markdown_weight() {
    let text = r#"%%{init: {"htmlLabels": false}}%%
flowchart TB
foo[**Bold Foo**]
"#;
    let engine = Engine::new();
    let parsed = block_on(engine.parse_diagram(text, ParseOptions::default()))
        .expect("parse ok")
        .expect("diagram detected");

    let layout_options = LayoutOptions::default();
    let out = layout_parsed(&parsed, &layout_options).expect("layout ok");
    let LayoutDiagram::FlowchartV2(layout) = out.layout else {
        panic!("expected FlowchartV2 layout");
    };

    let svg = render_flowchart_v2_svg(
        &layout,
        &out.semantic,
        &out.meta.effective_config,
        out.meta.title.as_deref(),
        layout_options.text_measurer.as_ref(),
        &SvgRenderOptions::default(),
    )
    .expect("render svg");

    assert!(
        svg.contains(">**Bold</tspan>"),
        "expected plain SVG text label to keep leading Markdown delimiter literal: {svg}"
    );
    assert!(
        svg.contains("> Foo**</tspan>"),
        "expected plain SVG text label to keep trailing Markdown delimiter literal: {svg}"
    );
    assert!(
        !svg.contains(r#"font-weight="bold""#),
        "plain SVG text labels must not apply Markdown strong styling: {svg}"
    );
}

#[test]
fn flowchart_html_plain_labels_treat_literal_backslash_n_as_line_breaks() {
    let text =
        "flowchart TB\nA[\"Remove trailing whitespace<br/>src.replace(/}\\s*\\n/g, '}\\n')\"]\n";
    let engine = Engine::new();
    let parsed = block_on(engine.parse_diagram(text, ParseOptions::default()))
        .expect("parse ok")
        .expect("diagram detected");

    let layout_options = LayoutOptions::default();
    let out = layout_parsed(&parsed, &layout_options).expect("layout ok");
    let LayoutDiagram::FlowchartV2(layout) = out.layout else {
        panic!("expected FlowchartV2 layout");
    };

    let svg = render_flowchart_v2_svg(
        &layout,
        &out.semantic,
        &out.meta.effective_config,
        out.meta.title.as_deref(),
        layout_options.text_measurer.as_ref(),
        &SvgRenderOptions::default(),
    )
    .expect("render svg");

    assert!(
        svg.contains(
            "<p>Remove trailing whitespace<br />src.replace(/}\\s*<br />/g, '}<br />')</p>"
        ),
        "expected literal backslash-n sequences to match Mermaid nonMarkdownToHTML line breaks: {svg}"
    );
}

#[test]
fn flowchart_html_edge_labels_preserve_edge_order_with_empty_labels() {
    let text = "flowchart TB\nA -->|Get money| B\nB --> C\nC -->|One| D\n";
    let engine = Engine::new();
    let parsed = block_on(engine.parse_diagram(text, ParseOptions::default()))
        .expect("parse ok")
        .expect("diagram detected");

    let layout_options = LayoutOptions {
        text_measurer: std::sync::Arc::new(VendoredFontMetricsTextMeasurer::default()),
        ..Default::default()
    };
    let out = layout_parsed(&parsed, &layout_options).expect("layout ok");
    let LayoutDiagram::FlowchartV2(layout) = out.layout else {
        panic!("expected FlowchartV2 layout");
    };

    let svg = render_flowchart_v2_svg(
        &layout,
        &out.semantic,
        &out.meta.effective_config,
        out.meta.title.as_deref(),
        layout_options.text_measurer.as_ref(),
        &SvgRenderOptions::default(),
    )
    .expect("render svg");
    let edge_labels_start = svg.find(r#"<g class="edgeLabels">"#).expect("edgeLabels");
    let nodes_start = svg[edge_labels_start..]
        .find(r#"<g class="nodes">"#)
        .map(|idx| edge_labels_start + idx)
        .expect("nodes after edgeLabels");
    let edge_labels = &svg[edge_labels_start..nodes_start];

    let ab = edge_labels.find(r#"data-id="L_A_B_0""#).expect("A-B label");
    let bc = edge_labels.find(r#"data-id="L_B_C_0""#).expect("B-C label");
    let cd = edge_labels.find(r#"data-id="L_C_D_0""#).expect("C-D label");

    assert!(
        ab < bc && bc < cd,
        "expected HTML edgeLabels to preserve graph edge order: {edge_labels}"
    );
}

#[test]
fn flowchart_html_edge_labels_use_non_markdown_paragraph_wrapper() {
    let text = "flowchart TB\nA -->|plain edge label| B\n";
    let engine = Engine::new();
    let parsed = block_on(engine.parse_diagram(text, ParseOptions::default()))
        .expect("parse ok")
        .expect("diagram detected");

    let layout_options = LayoutOptions {
        text_measurer: std::sync::Arc::new(VendoredFontMetricsTextMeasurer::default()),
        ..Default::default()
    };
    let out = layout_parsed(&parsed, &layout_options).expect("layout ok");
    let LayoutDiagram::FlowchartV2(layout) = out.layout else {
        panic!("expected FlowchartV2 layout");
    };

    let svg = render_flowchart_v2_svg(
        &layout,
        &out.semantic,
        &out.meta.effective_config,
        out.meta.title.as_deref(),
        layout_options.text_measurer.as_ref(),
        &SvgRenderOptions::default(),
    )
    .expect("render svg");

    assert!(
        svg.contains(r#"<span class="edgeLabel"><p>plain edge label</p></span>"#),
        "expected plain HTML edge labels to use Mermaid nonMarkdownToHTML paragraph wrapper: {svg}"
    );
}

#[test]
fn flowchart_html_edge_labels_include_browser_font_fallback_slack() {
    let text = "flowchart TD\n    A[Start] --> B{Condition ?}\n    B -->|Yes| C[Execute]\n    B -->|No| D[End]\n    C --> D\n";
    let engine = Engine::new();
    let parsed = block_on(engine.parse_diagram(text, ParseOptions::default()))
        .expect("parse ok")
        .expect("diagram detected");

    let layout_options = LayoutOptions {
        text_measurer: std::sync::Arc::new(VendoredFontMetricsTextMeasurer::default()),
        ..Default::default()
    };
    let out = layout_parsed(&parsed, &layout_options).expect("layout ok");
    let LayoutDiagram::FlowchartV2(layout) = out.layout else {
        panic!("expected FlowchartV2 layout");
    };

    let yes_label = layout
        .edges
        .iter()
        .find(|edge| edge.id == "L_B_C_0")
        .and_then(|edge| edge.label.as_ref())
        .expect("Yes edge label");
    let no_label = layout
        .edges
        .iter()
        .find(|edge| edge.id == "L_B_D_0")
        .and_then(|edge| edge.label.as_ref())
        .expect("No edge label");

    let svg = render_flowchart_v2_svg(
        &layout,
        &out.semantic,
        &out.meta.effective_config,
        out.meta.title.as_deref(),
        layout_options.text_measurer.as_ref(),
        &SvgRenderOptions::default(),
    )
    .expect("render svg");

    assert!(
        svg.contains(r#"<span class="edgeLabel"><p>Yes</p></span>"#)
            && svg.contains(r#"<span class="edgeLabel"><p>No</p></span>"#),
        "expected issue #2 edge labels to render as HTML labels: {svg}"
    );
    assert_eq!(
        foreign_object_width_for_data_id(&svg, "L_B_C_0"),
        yes_label.width + 4.0
    );
    assert_eq!(
        foreign_object_width_for_data_id(&svg, "L_B_D_0"),
        no_label.width + 4.0
    );
}

#[test]
fn flowchart_nested_root_viewbox_includes_empty_subgraph_node() {
    let text = "flowchart LR\nsubgraph A\na -->b\nend\nsubgraph B\nb\nend\n";
    let engine = Engine::new();
    let parsed = block_on(engine.parse_diagram(text, ParseOptions::default()))
        .expect("parse ok")
        .expect("diagram detected");

    let layout_options = LayoutOptions {
        text_measurer: std::sync::Arc::new(VendoredFontMetricsTextMeasurer::default()),
        ..Default::default()
    };
    let out = layout_parsed(&parsed, &layout_options).expect("layout ok");
    let LayoutDiagram::FlowchartV2(layout) = out.layout else {
        panic!("expected FlowchartV2 layout");
    };

    let svg = render_flowchart_v2_svg(
        &layout,
        &out.semantic,
        &out.meta.effective_config,
        out.meta.title.as_deref(),
        layout_options.text_measurer.as_ref(),
        &SvgRenderOptions {
            diagram_id: Some(
                "upstream_cypress_flowchart_v2_spec_57_handle_nested_subgraphs_with_outgoing_links_4_015"
                    .to_string(),
            ),
            apply_root_overrides: false,
            ..Default::default()
        },
    )
    .expect("render svg");

    assert!(
        svg.contains(r#"viewBox="0 0 154.921875 364""#),
        "expected computed root viewBox to include top-level empty subgraph node: {svg}"
    );
    assert!(
        svg.contains(
            r#"<g class="node" id="upstream_cypress_flowchart_v2_spec_57_handle_nested_subgraphs_with_outgoing_links_4_015-B""#
        ),
        "expected empty subgraph node DOM id to be scoped by the diagram id: {svg}"
    );
}

#[test]
fn flowchart_empty_subgraph_node_applies_inline_style() {
    let text = "flowchart TD\nsubgraph Empty\nend\nclassDef hot fill:#0f0,color:#111\nclass Empty hot\nstyle Empty fill:#f00,stroke:#00f,color:#fff\n";
    let engine = Engine::new();
    let parsed = block_on(engine.parse_diagram(text, ParseOptions::default()))
        .expect("parse ok")
        .expect("diagram detected");

    let layout_options = LayoutOptions {
        text_measurer: std::sync::Arc::new(VendoredFontMetricsTextMeasurer::default()),
        ..Default::default()
    };
    let out = layout_parsed(&parsed, &layout_options).expect("layout ok");
    let LayoutDiagram::FlowchartV2(layout) = out.layout else {
        panic!("expected FlowchartV2 layout");
    };

    let svg = render_flowchart_v2_svg(
        &layout,
        &out.semantic,
        &out.meta.effective_config,
        out.meta.title.as_deref(),
        layout_options.text_measurer.as_ref(),
        &SvgRenderOptions::default(),
    )
    .expect("render svg");

    assert!(
        svg.contains(r#"<g class="node hot" id="merman-Empty""#),
        "expected empty subgraph to render as a scoped node with its assigned class: {svg}"
    );
    assert!(
        svg.contains(r#"style="fill:#f00 !important;stroke:#00f !important""#),
        "expected empty subgraph inline shape style to be applied: {svg}"
    );
    assert!(
        svg.contains(r#"<span class="nodeLabel" style="color:#fff !important">"#),
        "expected empty subgraph inline label style to be applied: {svg}"
    );
}

#[test]
fn flowchart_crossed_circle_aliases_share_root_bbox_asymmetry() {
    let text = r#"flowchart
 n0@{ shape: cross-circ, label: "cross-circ" }
 n1@{ shape: summary, label: "summary" }
 n2@{ shape: crossed-circle, label: "crossed-circle" }
"#;
    let engine = Engine::new();
    let parsed = block_on(engine.parse_diagram(text, ParseOptions::default()))
        .expect("parse ok")
        .expect("diagram detected");

    let layout_options = LayoutOptions {
        text_measurer: std::sync::Arc::new(VendoredFontMetricsTextMeasurer::default()),
        ..Default::default()
    };
    let out = layout_parsed(&parsed, &layout_options).expect("layout ok");
    let LayoutDiagram::FlowchartV2(layout) = out.layout else {
        panic!("expected FlowchartV2 layout");
    };

    let svg = render_flowchart_v2_svg(
        &layout,
        &out.semantic,
        &out.meta.effective_config,
        out.meta.title.as_deref(),
        layout_options.text_measurer.as_ref(),
        &SvgRenderOptions {
            diagram_id: Some(
                "upstream_cypress_flowchart_shape_alias_spec_shape_alias_aliasset37_037"
                    .to_string(),
            ),
            apply_root_overrides: false,
            ..Default::default()
        },
    )
    .expect("render svg");

    let viewbox_start = svg.find(r#"viewBox=""#).expect("viewBox") + r#"viewBox=""#.len();
    let viewbox_end = svg[viewbox_start..].find('"').expect("viewBox end") + viewbox_start;
    let viewbox = &svg[viewbox_start..viewbox_end];
    let values = viewbox
        .split_whitespace()
        .map(|part| part.parse::<f64>().expect("viewBox number"))
        .collect::<Vec<_>>();
    assert_eq!(values.len(), 4, "expected four viewBox values: {viewbox}");
    assert!(
        (values[0] - 0.028_488).abs() < 0.000_01
            && values[1] == 0.0
            && (values[2] - 296.170_9).abs() < 0.000_1
            && values[3] == 76.0,
        "expected crossed-circle aliases to share RoughJS bbox asymmetry: {svg}"
    );
}

#[test]
fn flowchart_label_styles_follow_mermaid_label_style_whitelist() {
    let text = r#"%%{init: {"flowchart": {"htmlLabels": true}}}%%
flowchart LR
A[Styled node] -->|Styled edge| B[Plain]
style A fill:#eee,stroke:#111,font-style:italic,text-decoration:underline,letter-spacing:1px,white-space:break-spaces,text-align:left,line-height:2
linkStyle 0 font-style:italic,text-decoration:underline,letter-spacing:1px,color:#123456
"#;
    let engine = Engine::new();
    let parsed = block_on(engine.parse_diagram(text, ParseOptions::default()))
        .expect("parse ok")
        .expect("diagram detected");

    let layout_options = LayoutOptions {
        text_measurer: std::sync::Arc::new(VendoredFontMetricsTextMeasurer::default()),
        ..Default::default()
    };
    let out = layout_parsed(&parsed, &layout_options).expect("layout ok");
    let LayoutDiagram::FlowchartV2(layout) = out.layout else {
        panic!("expected FlowchartV2 layout");
    };

    let svg = render_flowchart_v2_svg(
        &layout,
        &out.semantic,
        &out.meta.effective_config,
        out.meta.title.as_deref(),
        layout_options.text_measurer.as_ref(),
        &SvgRenderOptions::default(),
    )
    .expect("render svg");

    assert!(
        svg.contains("font-style:italic !important"),
        "expected font-style to be routed to label styles: {svg}"
    );
    assert!(
        svg.contains("text-decoration:underline !important"),
        "expected text-decoration to be routed to label styles: {svg}"
    );
    assert!(
        svg.contains("letter-spacing:1px !important"),
        "expected letter-spacing to be routed to label styles: {svg}"
    );
    assert!(
        svg.contains("white-space:break-spaces !important"),
        "expected white-space to be preserved on the label span/group style: {svg}"
    );
    assert!(
        svg.contains(r#"style="fill:#eee !important;stroke:#111 !important""#),
        "expected shape styles to stay on the node shape: {svg}"
    );
    assert!(
        !svg.contains("fill:#eee !important;stroke:#111 !important;font-style"),
        "expected text-only styles not to be mixed into node shape style: {svg}"
    );
    assert!(
        svg.contains(r#"class="edgeLabel" style="font-style:italic !important;text-decoration:underline !important;letter-spacing:1px !important;color:#123456 !important""#),
        "expected edge label span to receive Mermaid label styles: {svg}"
    );
}

#[test]
fn flowchart_default_curve_renders_basis_edges_while_rounded_remains_available() {
    fn render(text: &str) -> String {
        let engine = Engine::new();
        let parsed = block_on(engine.parse_diagram(text, ParseOptions::default()))
            .expect("parse ok")
            .expect("diagram detected");

        let layout_options = LayoutOptions {
            text_measurer: std::sync::Arc::new(VendoredFontMetricsTextMeasurer::default()),
            ..Default::default()
        };
        let out = layout_parsed(&parsed, &layout_options).expect("layout ok");
        let LayoutDiagram::FlowchartV2(layout) = out.layout else {
            panic!("expected FlowchartV2 layout");
        };

        render_flowchart_v2_svg(
            &layout,
            &out.semantic,
            &out.meta.effective_config,
            out.meta.title.as_deref(),
            layout_options.text_measurer.as_ref(),
            &SvgRenderOptions::default(),
        )
        .expect("render svg")
    }

    fn edge_path_d<'a>(svg: &'a str, edge_id: &str) -> &'a str {
        let id_attr = format!(r#"id="{edge_id}""#);
        let id_start = svg.find(&id_attr).expect("edge id");
        let path_start = svg[..id_start].rfind("<path ").expect("edge path start");
        let path_end = svg[id_start..].find("/>").expect("edge path end") + id_start;
        let path = &svg[path_start..path_end];
        let d_start = path.find(r#"d=""#).expect("edge path d") + r#"d=""#.len();
        let d_end = path[d_start..].find('"').expect("edge path d end") + d_start;
        &path[d_start..d_end]
    }

    let diagram = "flowchart LR\nA --> B\nA --> C\n";
    let basis_svg = render(diagram);
    let basis_d = edge_path_d(&basis_svg, "L_A_B_0");
    assert!(
        basis_d.contains('C'),
        "expected default flowchart curve to preserve smooth basis output in Mermaid 11.15: {basis_d}"
    );

    let rounded_svg = render(&format!(
        "%%{{init: {{\"flowchart\": {{\"curve\": \"rounded\"}}}}}}%%\n{diagram}"
    ));
    let rounded_d = edge_path_d(&rounded_svg, "L_A_B_0");
    assert!(
        rounded_d.contains('Q') && !rounded_d.contains('C'),
        "expected explicit flowchart.curve=rounded to render rounded corners: {rounded_d}"
    );

    let tight_radius_svg = render(&format!(
        "%%{{init: {{\"flowchart\": {{\"curve\": \"rounded\", \"edgeCornerRadius\": 1}}}}}}%%\n{diagram}"
    ));
    let tight_radius_d = edge_path_d(&tight_radius_svg, "L_A_B_0");
    assert_ne!(
        rounded_d, tight_radius_d,
        "expected flowchart.edgeCornerRadius to alter rounded edge geometry"
    );
}

#[test]
fn flowchart_datastore_shape_renders_top_and_bottom_border_rect() {
    let text = r#"flowchart TB
D@{ shape: datastore, label: "Datastore" }
"#;
    let engine = Engine::new();
    let parsed = block_on(engine.parse_diagram(text, ParseOptions::default()))
        .expect("parse ok")
        .expect("diagram detected");

    let layout_options = LayoutOptions {
        text_measurer: std::sync::Arc::new(VendoredFontMetricsTextMeasurer::default()),
        ..Default::default()
    };
    let out = layout_parsed(&parsed, &layout_options).expect("layout ok");
    let LayoutDiagram::FlowchartV2(layout) = out.layout else {
        panic!("expected FlowchartV2 layout");
    };

    let svg = render_flowchart_v2_svg(
        &layout,
        &out.semantic,
        &out.meta.effective_config,
        out.meta.title.as_deref(),
        layout_options.text_measurer.as_ref(),
        &SvgRenderOptions::default(),
    )
    .expect("render svg");

    let rect_start = svg
        .find(r#"<rect class="basic label-container""#)
        .expect("datastore rect");
    let rect_end = svg[rect_start..].find("/>").expect("rect end") + rect_start;
    let rect = &svg[rect_start..rect_end];
    let attr = |name: &str| {
        let needle = format!(r#"{name}=""#);
        let start = rect.find(&needle).expect("attribute") + needle.len();
        let end = rect[start..].find('"').expect("attribute end") + start;
        &rect[start..end]
    };
    let expected_dasharray = format!("{} {}", attr("width"), attr("height"));
    assert!(
        attr("stroke-dasharray") == expected_dasharray,
        "expected datastore rect to hide vertical borders with width/height stroke-dasharray: {svg}"
    );
    assert!(
        !rect.contains("<path"),
        "expected datastore to render as a dashed-border rect, not bow-tie path: {svg}"
    );
}

#[cfg(feature = "ratex-math")]
#[test]
fn flowchart_svg_renders_ratex_math_labels_end_to_end() {
    let text = r#"%%{init: {"flowchart": {"htmlLabels": true}}}%%
flowchart LR
A["$$x^2$$"] -->|$$x^2$$| B[Done]
"#;
    let engine = Engine::new();
    let parsed = block_on(engine.parse_diagram(text, ParseOptions::default()))
        .expect("parse ok")
        .expect("diagram detected");

    let math_renderer = Arc::new(merman_render::math::RatexMathRenderer);
    let layout_options = LayoutOptions::default().with_math_renderer(math_renderer.clone());
    let out = layout_parsed(&parsed, &layout_options).expect("layout ok");
    let LayoutDiagram::FlowchartV2(layout) = out.layout else {
        panic!("expected FlowchartV2 layout");
    };

    let svg = render_flowchart_v2_svg(
        &layout,
        &out.semantic,
        &out.meta.effective_config,
        out.meta.title.as_deref(),
        layout_options.text_measurer.as_ref(),
        &SvgRenderOptions {
            math_renderer: Some(math_renderer),
            ..SvgRenderOptions::default()
        },
    )
    .expect("render svg");

    assert!(
        svg.contains(r#"width="0.97153em""#),
        "expected RaTeX inline SVG sizing in flowchart labels: {svg}"
    );
    assert!(
        svg.contains("<path"),
        "expected RaTeX glyph paths in flowchart SVG: {svg}"
    );
    assert!(
        !svg.contains("$$x^2$$"),
        "expected math source delimiters to be replaced by rendered SVG: {svg}"
    );
}

#[cfg(feature = "ratex-math")]
#[test]
fn flowchart_svg_renders_ratex_mixed_math_labels_end_to_end() {
    let text = r#"%%{init: {"flowchart": {"htmlLabels": true}}}%%
flowchart LR
A["value: $$x^2$$"] -->|Solve: $$\sqrt{2+2}$$| B[Done]
"#;
    let engine = Engine::new();
    let parsed = block_on(engine.parse_diagram(text, ParseOptions::default()))
        .expect("parse ok")
        .expect("diagram detected");

    let math_renderer = Arc::new(merman_render::math::RatexMathRenderer);
    let layout_options = LayoutOptions::default().with_math_renderer(math_renderer.clone());
    let out = layout_parsed(&parsed, &layout_options).expect("layout ok");
    let LayoutDiagram::FlowchartV2(layout) = out.layout else {
        panic!("expected FlowchartV2 layout");
    };

    let svg = render_flowchart_v2_svg(
        &layout,
        &out.semantic,
        &out.meta.effective_config,
        out.meta.title.as_deref(),
        layout_options.text_measurer.as_ref(),
        &SvgRenderOptions {
            math_renderer: Some(math_renderer),
            ..SvgRenderOptions::default()
        },
    )
    .expect("render svg");

    assert!(
        svg.contains("value: ") && svg.contains("Solve: ") && svg.contains("<path"),
        "expected mixed prose/math labels to render as RaTeX HTML fragments: {svg}"
    );
    assert!(
        !svg.contains(r#"value: $$x^2$$"#) && !svg.contains(r#"Solve: $$\sqrt{2+2}$$"#),
        "expected mixed flowchart labels to replace source delimiters: {svg}"
    );
}

#[cfg(feature = "ratex-math")]
#[test]
fn flowchart_docs_math_fixture_renders_supported_ratex_formulas() {
    let mmd_path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .join("fixtures")
        .join("flowchart")
        .join("upstream_docs_math_flowcharts_001.mmd");
    let text = std::fs::read_to_string(&mmd_path).expect("read fixture .mmd");
    let engine = Engine::new();
    let parsed = block_on(engine.parse_diagram(&text, ParseOptions::default()))
        .expect("parse ok")
        .expect("diagram detected");

    let math_renderer = Arc::new(merman_render::math::RatexMathRenderer);
    let layout_options = LayoutOptions::default().with_math_renderer(math_renderer.clone());
    let out = layout_parsed(&parsed, &layout_options).expect("layout ok");
    let LayoutDiagram::FlowchartV2(layout) = out.layout else {
        panic!("expected FlowchartV2 layout");
    };

    let svg = render_flowchart_v2_svg(
        &layout,
        &out.semantic,
        &out.meta.effective_config,
        out.meta.title.as_deref(),
        layout_options.text_measurer.as_ref(),
        &SvgRenderOptions {
            math_renderer: Some(math_renderer),
            ..SvgRenderOptions::default()
        },
    )
    .expect("render svg");

    let inline_formula_count = svg
        .matches(r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 "#)
        .count();
    assert_eq!(
        inline_formula_count, 7,
        "expected every pure math label in the docs fixture to render through RaTeX: {svg}"
    );
    assert!(
        !svg.contains("$$"),
        "expected supported flowchart fixture formulas to replace source delimiters: {svg}"
    );
}
