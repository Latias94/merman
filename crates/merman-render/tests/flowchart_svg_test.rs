use futures::executor::block_on;
use merman_core::{Engine, ParseOptions};
use merman_render::model::LayoutDiagram;
use merman_render::svg::{SvgRenderOptions, render_flowchart_v2_debug_svg};
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
fn flowchart_v2_fontawesome_edge_label_width_matches_upstream() {
    // Mermaid upstream fixture:
    // fixtures/upstream-svgs/flowchart/upstream_flowchart_v2_icons_in_edge_labels_spec.svg
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
    assert_eq!(lbl.width, 45.015625);
    assert_eq!(lbl.height, 24.0);
}
