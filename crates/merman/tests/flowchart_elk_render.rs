#![cfg(all(feature = "svg", feature = "layout-elk"))]

use merman::svg::SvgRenderOptions;
use merman::{OperationControl, RenderOutput, RenderRequest, Renderer, SvgRequest};

fn render_svg(diagram_id: &str, source: &str) -> String {
    let output = Renderer::new()
        .render(RenderRequest::svg(
            source,
            OperationControl::new(),
            SvgRequest {
                options: SvgRenderOptions {
                    diagram_id: Some(diagram_id.to_string()),
                    ..Default::default()
                },
                ..Default::default()
            },
        ))
        .expect("render should succeed");
    let RenderOutput::Svg(Some(svg)) = output else {
        panic!("diagram should be detected");
    };
    svg.into_parts().0
}

#[test]
fn renderer_renders_flowchart_elk_svg() {
    let svg = render_svg(
        "flowchart-elk-smoke",
        "flowchart-elk TD\nA[Alpha] --> B[Beta]",
    );

    assert!(svg.starts_with("<svg"));
    assert!(svg.contains("Alpha"));
    assert!(svg.contains("Beta"));
    assert!(!svg.contains("NaN"));
}

#[test]
fn renderer_uses_flowchart_elk_svg_contract() {
    let svg = render_svg(
        "flowchart-elk-contract",
        "flowchart-elk LR\nA --> B\nA --> C",
    );

    assert!(svg.contains(r#"aria-roledescription="flowchart-elk""#));
    assert!(has_marker_kind(&svg, "pointEnd"));
    let d = edge_path_attr(&svg, "L_A_B_0", "d");
    assert!(
        d.contains('L') && !d.contains('C'),
        "expected ELK edges to avoid cubic curves in the default flowchart-elk path: {d}"
    );
}

#[test]
fn renderer_keeps_flowchart_elk_cutter_jog_for_straight_shape_edge() {
    let svg = render_svg(
        "flowchart-elk-straight-cutter",
        "flowchart-elk TD\nA([Start]) ==> B[Step 1]",
    );

    let d = edge_path_attr(&svg, "L_A_B_0", "d");
    assert!(
        d.contains('Q'),
        "expected ELK cutter points to preserve a rounded corner for the stadium endpoint: {d}"
    );
    assert_eq!(
        data_points_len(&edge_path_attr(&svg, "L_A_B_0", "data-points")),
        3,
        "expected Mermaid-style ELK cutter data-points to keep start intersection, jog, and end"
    );
}

#[test]
fn renderer_renders_documented_flowchart_elk_public_config() {
    let source = r#"---
config:
  layout: elk
  elk:
    mergeEdges: true
    nodePlacementStrategy: LINEAR_SEGMENTS
---
flowchart LR
  A[Start] --> B{Choose Path}
  B -->|Option 1| C[Path 1]
  B -->|Option 2| D[Path 2]"#;

    let svg = render_svg("flowchart-elk-public-config", source);

    assert!(svg.starts_with("<svg"));
    for expected in [
        "Start",
        "Choose Path",
        "Option 1",
        "Option 2",
        "Path 1",
        "Path 2",
    ] {
        assert!(
            svg.contains(expected),
            "expected rendered SVG to contain {expected:?}"
        );
    }
    assert!(!svg.contains("NaN"));
}

#[test]
fn renderer_renders_reported_flowchart_elk_linear_segments_case() {
    let source = r#"---
config:
  layout: elk
  elk:
    mergeEdges: true
    nodePlacementStrategy: LINEAR_SEGMENTS
---
flowchart LR
  A[Start] --> B{Choose Path}
  B -->|Option 1| C[Path 1]
  B -->|Option 2| D[Path 2]"#;

    let svg = render_svg("flowchart-elk-linear-segments-reported", source);

    assert!(svg.contains("Start"));
    assert!(svg.contains("Choose Path"));
    assert!(svg.contains("Path 1"));
    assert!(svg.contains("Path 2"));
    assert!(!svg.contains("NaN"));
}

#[test]
fn renderer_renders_public_flowchart_elk_node_placement_strategies() {
    for strategy in [
        "BRANDES_KOEPF",
        "SIMPLE",
        "LINEAR_SEGMENTS",
        "NETWORK_SIMPLEX",
    ] {
        let source = format!(
            r#"---
config:
  layout: elk
  elk:
    nodePlacementStrategy: {strategy}
---
flowchart TD
  A[Alpha] --> B[Beta]
  A --> C[Gamma]"#
        );

        let diagram_id = format!("flowchart-elk-{strategy}");
        let svg = render_svg(&diagram_id, &source);

        assert!(svg.contains("Alpha"));
        assert!(svg.contains("Beta"));
        assert!(svg.contains("Gamma"));
        assert!(!svg.contains("NaN"));
    }
}

#[test]
fn renderer_renders_public_flowchart_elk_node_placement_alignments() {
    for alignment in [
        "NONE",
        "LEFTUP",
        "LEFTDOWN",
        "RIGHTUP",
        "RIGHTDOWN",
        "BALANCED",
    ] {
        let source = format!(
            r#"---
config:
  layout: elk
  elk:
    nodePlacementAlignment: {alignment}
---
flowchart TD
  A[Alpha] --> B[Beta]
  A --> C[Gamma]"#
        );

        let diagram_id = format!("flowchart-elk-alignment-{alignment}");
        let svg = render_svg(&diagram_id, &source);

        assert!(svg.contains("Alpha"));
        assert!(svg.contains("Beta"));
        assert!(svg.contains("Gamma"));
        assert!(!svg.contains("NaN"));
    }
}

#[test]
fn renderer_renders_public_flowchart_elk_cycle_breaking_strategies() {
    for strategy in [
        "GREEDY",
        "DEPTH_FIRST",
        "INTERACTIVE",
        "MODEL_ORDER",
        "GREEDY_MODEL_ORDER",
    ] {
        let source = format!(
            r#"---
config:
  layout: elk
  elk:
    cycleBreakingStrategy: {strategy}
---
flowchart TD
  A[Alpha] --> B[Beta]
  B --> C[Gamma]
  C --> A"#
        );

        let diagram_id = format!("flowchart-elk-cycle-{strategy}");
        let svg = render_svg(&diagram_id, &source);

        assert!(svg.contains("Alpha"));
        assert!(svg.contains("Beta"));
        assert!(svg.contains("Gamma"));
        assert!(!svg.contains("NaN"));
    }
}

#[test]
fn renderer_renders_public_flowchart_elk_alignment_and_entry_config() {
    let source = r#"---
config:
  layout: elk
  elk:
    nodePlacementAlignment: BALANCED
    keepEntryNodeOnTop: true
---
flowchart TD
  A[Entry] --> B[Step]
  B --> C[Loop]
  C --> A"#;

    let svg = render_svg("flowchart-elk-alignment-entry", source);

    assert!(svg.contains("Entry"));
    assert!(svg.contains("Step"));
    assert!(svg.contains("Loop"));
    assert!(!svg.contains("NaN"));
}

fn edge_path_attr(svg: &str, raw_edge_id: &str, attr: &str) -> String {
    let document = roxmltree::Document::parse(svg).expect("valid SVG XML");
    document
        .descendants()
        .find(|node| {
            node.has_tag_name("path")
                && node.attribute("data-et") == Some("edge")
                && node.attribute("data-id") == Some(raw_edge_id)
        })
        .unwrap_or_else(|| panic!("edge path for {raw_edge_id}"))
        .attribute(attr)
        .unwrap_or_else(|| panic!("edge path attribute {attr}"))
        .to_string()
}

fn has_marker_kind(svg: &str, kind: &str) -> bool {
    let document = roxmltree::Document::parse(svg).expect("valid SVG XML");
    document.descendants().any(|node| {
        node.has_tag_name("marker")
            && node
                .attribute("id")
                .is_some_and(|id| id.ends_with(&format!("-{kind}")))
    })
}

fn data_points_len(payload: &str) -> usize {
    use base64::Engine as _;

    let bytes = base64::engine::general_purpose::STANDARD
        .decode(payload.as_bytes())
        .expect("data-points base64");
    let points: Vec<serde_json::Value> = serde_json::from_slice(&bytes).expect("data-points json");
    points.len()
}
