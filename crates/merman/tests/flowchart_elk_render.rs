#![cfg(all(feature = "render", feature = "elk-layout"))]

use merman::render::HeadlessRenderer;

#[test]
fn headless_renderer_renders_flowchart_elk_svg() {
    let svg = HeadlessRenderer::new()
        .with_vendored_text_measurer()
        .with_diagram_id("flowchart-elk-smoke")
        .render_svg_sync("flowchart-elk TD\nA[Alpha] --> B[Beta]")
        .expect("render should succeed")
        .expect("diagram should be detected");

    assert!(svg.starts_with("<svg"));
    assert!(svg.contains("Alpha"));
    assert!(svg.contains("Beta"));
    assert!(!svg.contains("NaN"));
}

#[test]
fn headless_renderer_uses_flowchart_elk_svg_contract() {
    let svg = HeadlessRenderer::new()
        .with_vendored_text_measurer()
        .with_diagram_id("flowchart-elk-contract")
        .render_svg_sync("flowchart-elk LR\nA --> B\nA --> C")
        .expect("render should succeed")
        .expect("diagram should be detected");

    assert!(svg.contains(r#"aria-roledescription="flowchart-elk""#));
    assert!(svg.contains("flowchart-elk-contract_flowchart-elk-pointEnd"));
    let d = edge_path_d(&svg, "flowchart-elk-contract-L_A_B_0");
    assert!(
        d.contains('L') && !d.contains('C'),
        "expected ELK edges to avoid cubic curves in the default flowchart-elk path: {d}"
    );
}

#[test]
fn headless_renderer_keeps_flowchart_elk_cutter_jog_for_straight_shape_edge() {
    let svg = HeadlessRenderer::new()
        .with_vendored_text_measurer()
        .with_diagram_id("flowchart-elk-straight-cutter")
        .render_svg_sync("flowchart-elk TD\nA([Start]) ==> B[Step 1]")
        .expect("render should succeed")
        .expect("diagram should be detected");

    let path = edge_path_chunk(&svg, "flowchart-elk-straight-cutter-L_A_B_0");
    let d = path_attr(path, "d");
    assert!(
        d.contains('Q'),
        "expected ELK cutter points to preserve a rounded corner for the stadium endpoint: {d}"
    );
    assert_eq!(
        data_points_len(path),
        3,
        "expected Mermaid-style ELK cutter data-points to keep start intersection, jog, and end"
    );
}

#[test]
fn headless_renderer_renders_documented_flowchart_elk_public_config() {
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

    let svg = HeadlessRenderer::new()
        .with_vendored_text_measurer()
        .with_diagram_id("flowchart-elk-public-config")
        .render_svg_sync(source)
        .expect("render should succeed")
        .expect("diagram should be detected");

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
fn headless_renderer_renders_public_flowchart_elk_node_placement_strategies() {
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
        let svg = HeadlessRenderer::new()
            .with_vendored_text_measurer()
            .with_diagram_id(&diagram_id)
            .render_svg_sync(&source)
            .unwrap_or_else(|err| panic!("{strategy} render should succeed: {err}"))
            .unwrap_or_else(|| panic!("{strategy} diagram should be detected"));

        assert!(svg.contains("Alpha"));
        assert!(svg.contains("Beta"));
        assert!(svg.contains("Gamma"));
        assert!(!svg.contains("NaN"));
    }
}

fn edge_path_d<'a>(svg: &'a str, edge_id: &str) -> &'a str {
    path_attr(edge_path_chunk(svg, edge_id), "d")
}

fn edge_path_chunk<'a>(svg: &'a str, edge_id: &str) -> &'a str {
    let id_attr = format!(r#"id="{edge_id}""#);
    let id_start = svg.find(&id_attr).expect("edge id");
    let path_start = svg[..id_start].rfind("<path ").expect("edge path start");
    let path_end = svg[id_start..].find("/>").expect("edge path end") + id_start;
    &svg[path_start..path_end]
}

fn path_attr<'a>(path: &'a str, attr: &str) -> &'a str {
    let attr_start = path
        .find(&format!(r#"{attr}=""#))
        .unwrap_or_else(|| panic!("path attr {attr}"))
        + attr.len()
        + r#"=""#.len();
    let attr_end = path[attr_start..]
        .find('"')
        .unwrap_or_else(|| panic!("path attr {attr} end"))
        + attr_start;
    &path[attr_start..attr_end]
}

fn data_points_len(path: &str) -> usize {
    use base64::Engine as _;

    let payload = path_attr(path, "data-points");
    let bytes = base64::engine::general_purpose::STANDARD
        .decode(payload.as_bytes())
        .expect("data-points base64");
    let points: Vec<serde_json::Value> = serde_json::from_slice(&bytes).expect("data-points json");
    points.len()
}
