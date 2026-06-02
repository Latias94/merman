use merman_core::{Engine, ParseOptions};
use merman_render::model::LayoutDiagram;
use merman_render::svg::{SvgRenderOptions, render_quadrantchart_diagram_svg};
use merman_render::{LayoutOptions, layout_parsed};

fn render_quadrantchart_svg_from_text(text: &str) -> String {
    let engine = Engine::new();
    let parsed = futures::executor::block_on(engine.parse_diagram(text, ParseOptions::default()))
        .expect("parse ok")
        .expect("diagram detected");
    let out = layout_parsed(&parsed, &LayoutOptions::default()).expect("layout ok");
    let LayoutDiagram::QuadrantChartDiagram(layout) = &out.layout else {
        panic!("expected QuadrantChartDiagram layout");
    };

    render_quadrantchart_diagram_svg(
        layout,
        &out.semantic,
        &out.meta.effective_config,
        &SvgRenderOptions::default(),
    )
    .expect("render svg")
}

fn assert_contains(haystack: &str, needle: &str) {
    assert!(
        haystack.contains(needle),
        "expected SVG to contain {needle:?}"
    );
}

#[test]
fn quadrantchart_default_point_fill_is_valid_for_headless_renderability() {
    let svg = render_quadrantchart_svg_from_text(
        r#"quadrantChart
  title Boundary points
  x-axis Left --> Right
  y-axis Bottom --> Top
  quadrant-1 Q1
  quadrant-2 Q2
  quadrant-3 Q3
  quadrant-4 Q4
  P0: [0, 0]
  P1: [1, 1]
"#,
    );

    assert!(!svg.contains("NaN"), "SVG leaked invalid color: {svg}");
    assert_contains(
        &svg,
        r#"<circle cx="31" cy="469" r="5" fill="rgb(185, 185, 255)" stroke="rgb(185, 185, 255)" stroke-width="0px"/>"#,
    );
}

#[test]
fn quadrantchart_theme_variable_can_override_default_point_fill() {
    let svg = render_quadrantchart_svg_from_text(
        r##"%%{init: {"themeVariables": {"quadrantPointFill": "#facc15", "quadrantPointTextFill": "#111827"}}}%%
quadrantChart
  title Priority
  x-axis Low --> High
  y-axis Low --> High
  quadrant-1 Plan
  Feature: [0.7, 0.8]
"##,
    );

    assert!(!svg.contains("NaN"), "SVG leaked invalid color: {svg}");
    assert_contains(
        &svg,
        r##"<circle cx="355.79999999999995" cy="129.79999999999995" r="5" fill="#facc15" stroke="#facc15" stroke-width="0px"/>"##,
    );
    assert_contains(&svg, r##"fill="#111827" font-size="12""##);
}
