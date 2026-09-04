mod common;

use common::legacy_init_theme_compat_engine;
use merman_core::ParseOptions;
use merman_render::LayoutOptions;
use merman_render::environment::RenderEnvironment;
use merman_render::family;
use merman_render::model::{XyChartDiagramLayout, XyChartDrawableElem};
use merman_render::svg::{SvgDebugOptions, SvgRenderOptions};
use roxmltree::{Document, Node};

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

fn has_class(node: Node<'_, '_>, class: &str) -> bool {
    node.attribute("class").is_some_and(|classes| {
        classes
            .split_whitespace()
            .any(|candidate| candidate == class)
    })
}

fn group_with_classes<'a>(document: &'a Document<'a>, classes: &[&str]) -> Node<'a, 'a> {
    document
        .descendants()
        .find(|node| {
            node.tag_name().name() == "g" && classes.iter().all(|class| has_class(*node, class))
        })
        .expect("expected canonical XYChart semantic group")
}

#[test]
fn xychart_frontmatter_title_and_accessibility_metadata_match_mermaid() {
    let svg = render_xychart_svg_from_text(
        r#"---
title: Frontmatter XY
---
xychart
  accTitle: Accessible XY
  accDescr: XY description
  x-axis [A]
  y-axis 0 --> 1
  bar [1]
"#,
    );

    assert_contains(&svg, ">Frontmatter XY</text>");
    assert_contains(&svg, r#"aria-labelledby="chart-title-xychart""#);
    assert_contains(&svg, r#"aria-describedby="chart-desc-xychart""#);
    assert_contains(
        &svg,
        r#"<title id="chart-title-xychart">Accessible XY</title>"#,
    );
    assert_contains(
        &svg,
        r#"<desc id="chart-desc-xychart">XY description</desc>"#,
    );
}

#[test]
fn xychart_body_title_overrides_frontmatter_title() {
    let svg = render_xychart_svg_from_text(
        r#"---
title: Frontmatter XY
---
xychart
  title "Body XY"
  x-axis [A]
  y-axis 0 --> 1
  bar [1]
"#,
    );

    assert_contains(&svg, ">Body XY</text>");
    assert!(!svg.contains(">Frontmatter XY</text>"));
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
    assert!(layout.drawables.iter().any(|drawable| matches!(
        drawable,
        XyChartDrawableElem::BarDataLabel { data, .. }
            if data.len() == 1
                && data[0].text == "73"
                && data[0].vertical_pos == "auto"
    )));
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
fn xychart_named_plots_render_a_configurable_legend() {
    let text = r##"---
config:
  themeVariables:
    xyChart:
      legendTextColor: "#123456"
---
xychart
  x-axis [Q1, Q2]
  y-axis 0 --> 100
  line "avg" [40, 50]
  bar "p95" [80, 90]
  line [30, 35]
"##;
    let layout = layout_xychart_from_text(text);

    let legend_labels = layout
        .drawables
        .iter()
        .find_map(|drawable| match drawable {
            XyChartDrawableElem::Text { group_texts, data }
                if group_texts == &["legend".to_string(), "label".to_string()] =>
            {
                Some(data)
            }
            _ => None,
        })
        .expect("legend labels");
    assert_eq!(
        legend_labels
            .iter()
            .map(|label| label.text.as_str())
            .collect::<Vec<_>>(),
        ["avg", "p95"]
    );
    assert!(legend_labels.iter().all(|label| label.fill == "#123456"));

    let disabled = layout_xychart_from_text(
        r#"---
config:
  xyChart:
    showLegend: false
---
xychart
  x-axis [Q1, Q2]
  y-axis 0 --> 100
  line "avg" [40, 50]
  bar "p95" [80, 90]
"#,
    );
    assert!(!disabled.drawables.iter().any(|drawable| match drawable {
        XyChartDrawableElem::Rect { group_texts, .. }
        | XyChartDrawableElem::Text { group_texts, .. }
        | XyChartDrawableElem::BarDataLabel { group_texts, .. }
        | XyChartDrawableElem::Path { group_texts, .. } => {
            group_texts.first().is_some_and(|group| group == "legend")
        }
    }));
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

    let document = Document::parse(&svg).expect("canonical XYChart SVG is XML");
    let plot = group_with_classes(&document, &["plot", "bar-plot-0"]);
    assert!(plot.descendants().any(|node| {
        node.tag_name().name() == "rect"
            && node.attribute("fill") == Some("#008000")
            && node.attribute("stroke") == Some("#008000")
            && node.attribute("stroke-width") == Some("0")
    }));
    let line_plot = group_with_classes(&document, &["plot", "line-plot-1"]);
    assert!(line_plot.descendants().any(|node| {
        node.tag_name().name() == "path"
            && node.attribute("stroke") == Some("#faba63")
            && node.attribute("stroke-width") == Some("2")
    }));

    let bottom_axis_line = group_with_classes(&document, &["bottom-axis", "axis-line"]);
    assert!(bottom_axis_line.descendants().any(|node| {
        node.tag_name().name() == "path" && node.attribute("stroke") == Some("#87ceeb")
    }));
    let bottom_axis_ticks = group_with_classes(&document, &["bottom-axis", "ticks"]);
    assert!(bottom_axis_ticks.descendants().any(|node| {
        node.tag_name().name() == "path" && node.attribute("stroke") == Some("#ff6347")
    }));

    let left_axis_line = group_with_classes(&document, &["left-axis", "axisl-line"]);
    assert!(left_axis_line.descendants().any(|node| {
        node.tag_name().name() == "path" && node.attribute("stroke") == Some("#ff6347")
    }));
    let left_axis_ticks = group_with_classes(&document, &["left-axis", "ticks"]);
    assert!(left_axis_ticks.descendants().any(|node| {
        node.tag_name().name() == "path" && node.attribute("stroke") == Some("#87ceeb")
    }));
}
