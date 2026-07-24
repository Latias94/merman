mod common;

use common::legacy_init_theme_compat_engine;
use merman_core::ParseOptions;
use merman_render::LayoutOptions;
use merman_render::environment::RenderEnvironment;
use merman_render::family;
use merman_render::model::{XyChartDiagramLayout, XyChartDrawableElem};
use merman_render::svg::{SvgDebugOptions, SvgRenderOptions};

fn layout_xychart_from_text(text: &str) -> XyChartDiagramLayout {
    let session = RenderEnvironment::deterministic().begin_session().unwrap();
    let engine = legacy_init_theme_compat_engine();
    let parsed = engine
        .parse_diagram_for_render_model_sync(text, ParseOptions::default())
        .expect("parse ok")
        .expect("diagram detected");
    let artifact = family::prepare(parsed, &LayoutOptions::default(), session).expect("layout ok");
    let projection = artifact.layout_json().expect("serialize XYChart layout");
    serde_json::from_value(projection["layout"]["XyChartDiagram"].clone())
        .expect("XYChart layout projection")
}

fn render_xychart_svg_from_text(text: &str) -> String {
    let session = RenderEnvironment::deterministic().begin_session().unwrap();
    let engine = legacy_init_theme_compat_engine();
    let parsed = engine
        .parse_diagram_for_render_model_sync(text, ParseOptions::default())
        .expect("parse ok")
        .expect("diagram detected");
    let artifact = family::prepare(parsed, &LayoutOptions::default(), session).expect("layout ok");

    artifact
        .render_svg(&SvgRenderOptions::default(), &SvgDebugOptions::default())
        .expect("render svg")
        .svg()
        .to_owned()
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
fn xychart_layout_carries_data_label_outside_policy() {
    let layout = layout_xychart_from_text(
        r"---
config:
  xyChart:
    showDataLabel: true
    showDataLabelOutsideBar: true
---
xychart
  x-axis [A]
  y-axis 0 --> 100
  bar [73]
",
    );

    assert!(layout.show_data_label);
    assert!(layout.show_data_label_outside_bar);
    assert_eq!(layout.label_data, vec!["73"]);
}

#[test]
fn xychart_horizontal_line_point_label_offsets_from_the_screen_point() {
    let layout = layout_xychart_from_text(
        r#"xychart horizontal
  x-axis [A]
  y-axis 0 --> 100
  line [73 "point"]
"#,
    );

    let path = layout
        .drawables
        .iter()
        .find_map(|drawable| match drawable {
            XyChartDrawableElem::Path { data, .. } => data.first(),
            _ => None,
        })
        .expect("line path");
    let point = path
        .path
        .strip_prefix('M')
        .and_then(|value| value.strip_suffix('Z'))
        .and_then(|value| value.split_once(','))
        .map(|(x, y)| (x.parse::<f64>().unwrap(), y.parse::<f64>().unwrap()))
        .expect("single-point path coordinates");
    let label = layout
        .drawables
        .iter()
        .find_map(|drawable| match drawable {
            XyChartDrawableElem::Text { data, .. } => {
                data.iter().find(|label| label.text == "point")
            }
            _ => None,
        })
        .expect("point label");

    assert_eq!(label.x, point.0 + 10.0);
    assert_eq!(label.y, point.1);
    assert_eq!(label.horizontal_pos, "left");
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
fn xychart_huge_finite_bar_dimensions_do_not_spin_the_data_label_renderer() {
    let svg = render_xychart_svg_from_text(
        r#"---
config:
  xyChart:
    height: "1e308"
    showDataLabel: true
---
xychart horizontal
  x-axis [A]
  y-axis 0 --> 100
  bar [73]
"#,
    );

    let label = text_tag_by_text(&svg, "73");
    assert!(!label.contains("font-size=\"NaNpx\""), "label: {label}");
    assert!(!label.contains("Infinity"), "label: {label}");
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
