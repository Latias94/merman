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
