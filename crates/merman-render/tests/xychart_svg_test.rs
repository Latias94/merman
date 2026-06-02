use merman_core::{Engine, ParseOptions};
use merman_render::model::LayoutDiagram;
use merman_render::svg::{SvgRenderOptions, render_xychart_diagram_svg};
use merman_render::{LayoutOptions, layout_parsed};

fn render_xychart_svg_from_text(text: &str) -> String {
    let engine = Engine::new();
    let parsed = futures::executor::block_on(engine.parse_diagram(text, ParseOptions::default()))
        .expect("parse ok")
        .expect("diagram detected");
    let out = layout_parsed(&parsed, &LayoutOptions::default()).expect("layout ok");
    let LayoutDiagram::XyChartDiagram(layout) = &out.layout else {
        panic!("expected XyChartDiagram layout");
    };

    render_xychart_diagram_svg(
        layout,
        &out.semantic,
        &out.meta.effective_config,
        &SvgRenderOptions::default(),
    )
    .expect("render svg")
}

fn text_tag_by_text<'a>(svg: &'a str, text: &str) -> &'a str {
    let needle = format!(">{text}</text>");
    let end = svg.find(&needle).expect("expected text tag") + needle.len();
    let start = svg[..end].rfind("<text").expect("expected text tag start");
    &svg[start..end]
}

fn assert_contains(haystack: &str, needle: &str) {
    assert!(
        haystack.contains(needle),
        "expected SVG to contain {needle:?}"
    );
}

fn svg_segment<'a>(svg: &'a str, start_needle: &str, end_needle: &str) -> &'a str {
    let start = svg.find(start_needle).expect("expected segment start");
    let rest = &svg[start..];
    let end = rest.find(end_needle).expect("expected segment end");
    &rest[..end]
}

#[test]
fn xychart_vertical_bar_data_label_can_render_outside_with_configured_color() {
    let svg = render_xychart_svg_from_text(
        r##"---
config:
  xyChart:
    showDataLabel: true
    showDataLabelOutsideBar: true
  themeVariables:
    xyChart:
      dataLabelColor: "#1155cc"
---
xychart
  x-axis [A]
  y-axis 0 --> 100
  bar [73]
"##,
    );

    let label = text_tag_by_text(&svg, "73");
    assert!(
        label.contains(r##"fill="#1155cc""##),
        "expected configured data label color: {label}"
    );
    assert!(
        label.contains(r#"dominant-baseline="auto""#),
        "expected vertical outside label baseline: {label}"
    );
}

#[test]
fn xychart_horizontal_bar_data_label_can_render_outside() {
    let svg = render_xychart_svg_from_text(
        r##"---
config:
  xyChart:
    showDataLabel: true
    showDataLabelOutsideBar: true
  themeVariables:
    xyChart:
      dataLabelColor: "#008855"
---
xychart horizontal
  x-axis Categories [A]
  y-axis Value 0 --> 100
  bar [73]
"##,
    );

    let label = text_tag_by_text(&svg, "73");
    assert!(
        label.contains(r#"text-anchor="start""#),
        "expected horizontal outside label anchor: {label}"
    );
    assert!(
        label.contains(r##"fill="#008855""##),
        "expected configured data label color: {label}"
    );
}

#[test]
fn xychart_svg_honors_mermaid_11_15_inline_theme_config() {
    let svg = render_xychart_svg_from_text(include_str!(
        "../../../fixtures/xychart/upstream_cypress_xychart_spec_render_all_the_theme_color_018.mmd"
    ));

    assert_contains(
        &svg,
        r##"<rect width="700" height="500" class="background" fill="#f0f8ff"/>"##,
    );

    let chart_title = text_tag_by_text(&svg, "Sales Revenue");
    assert_contains(chart_title, r##"fill="#ff0000""##);

    let x_axis_title = text_tag_by_text(&svg, "Months");
    assert_contains(x_axis_title, r##"fill="#ee82ee""##);

    let y_axis_title = text_tag_by_text(&svg, "Revenue (in $)");
    assert_contains(y_axis_title, r##"fill="#7fffd4""##);

    let x_axis_label = text_tag_by_text(&svg, "jan");
    assert_contains(x_axis_label, r##"fill="#7fffd4""##);

    let y_axis_label = text_tag_by_text(&svg, "11000");
    assert_contains(y_axis_label, r##"fill="#ee82ee""##);

    let plot = svg_segment(&svg, r#"<g class="plot">"#, r#"<g class="bottom-axis">"#);
    assert_contains(plot, r##"fill="#008000" stroke="#008000""##);
    assert_contains(plot, r##"stroke="#faba63" stroke-width="2""##);

    let bottom_axis = svg_segment(
        &svg,
        r#"<g class="bottom-axis">"#,
        r#"<g class="left-axis">"#,
    );
    assert_contains(bottom_axis, r##"class="axis-line"><path"##);
    assert_contains(bottom_axis, r##"stroke="#87ceeb" stroke-width="2""##);
    assert_contains(bottom_axis, r##"class="ticks"><path"##);
    assert_contains(bottom_axis, r##"stroke="#ff6347" stroke-width="2""##);

    let left_axis = svg_segment(
        &svg,
        r#"<g class="left-axis">"#,
        r#"<g class="mermaid-tmp-group""#,
    );
    assert_contains(left_axis, r##"class="axisl-line"><path"##);
    assert_contains(left_axis, r##"stroke="#ff6347" stroke-width="2""##);
    assert_contains(left_axis, r##"class="ticks"><path"##);
    assert_contains(left_axis, r##"stroke="#87ceeb" stroke-width="2""##);
}
