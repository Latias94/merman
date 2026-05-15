use futures::executor::block_on;
use merman_core::{Engine, ParseOptions};
use merman_render::model::LayoutDiagram;
use merman_render::svg::{
    SvgRenderOptions, render_flowchart_v2_debug_svg, render_flowchart_v2_svg,
};
use merman_render::text::VendoredFontMetricsTextMeasurer;
use merman_render::{LayoutOptions, layout_parsed};
use std::path::PathBuf;

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
    // We intentionally model FontAwesome icon labels with a clean nominal inline width instead of
    // browser-specific per-icon advance drift. Exact upstream root parity remains covered by root
    // viewport guards where that drift matters.
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
    assert_eq!(lbl.width, 45.03125);
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
fn flowchart_html_labels_unescape_double_backslashes() {
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
        svg.contains("line1\\nline2"),
        "expected output to contain a single backslash in `\\\\n` escape"
    );
    assert!(
        !svg.contains("line1\\\\nline2"),
        "expected output to not contain the raw double-backslash input"
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
