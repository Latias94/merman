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
        d.contains('Q') && !d.contains('C'),
        "expected ELK edges to use rounded right-angle paths by default: {d}"
    );
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
