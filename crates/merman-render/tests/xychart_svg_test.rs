mod common;

use common::legacy_init_theme_compat_engine;
use merman_core::presentation::SourcePresentationConfig;
use merman_core::{MermaidEngine, ParseOptions};
use merman_render::LayoutOptions;
use merman_render::environment::RenderEnvironment;
use merman_render::family;
use merman_render::model::{XyChartDiagramLayout, XyChartDrawableElem};
use merman_render::resources::{
    RenderResourcePolicy, ResourceLimitCause, ResourceLimitId, ResourceLimitPhase,
};
use merman_render::svg::{SvgDebugOptions, SvgRenderOptions};
use merman_render::theme::{BuiltinPreset, NativeTheme, ResolvedThemeV2, ThemeCompiler};
use std::sync::Arc;

fn xychart_plot_group<'document, 'input>(
    document: &'document roxmltree::Document<'input>,
    class: &str,
) -> roxmltree::Node<'document, 'input> {
    document
        .descendants()
        .find(|node| node.has_tag_name("g") && node.attribute("class") == Some(class))
        .unwrap_or_else(|| panic!("XY Chart plot group {class}"))
}

fn layout_xychart_from_text(text: &str) -> XyChartDiagramLayout {
    let session = RenderEnvironment::deterministic().begin_session().unwrap();
    let engine = legacy_init_theme_compat_engine();
    let parsed = engine
        .parse_diagram_for_render_model_sync(text, ParseOptions::default())
        .expect("parse ok")
        .expect("diagram detected")
        .into_presentation();
    let artifact = family::prepare(&parsed, &LayoutOptions::default(), session).expect("layout ok");
    let projection = artifact.layout_json().expect("serialize XYChart layout");
    serde_json::from_value(projection["layout"]["XyChartDiagram"].clone())
        .expect("XYChart layout projection")
}

fn render_xychart_svg_from_text(text: &str) -> String {
    try_render_xychart_svg_from_text(text).expect("render svg")
}

fn render_native_xychart_svg(source: &str, theme: Arc<ResolvedThemeV2>) -> String {
    let parsed = MermaidEngine::new()
        .parse_diagram_for_render_model_sync(source, ParseOptions::strict())
        .unwrap()
        .unwrap();
    let session = RenderEnvironment::deterministic()
        .begin_session_with_native_theme(NativeTheme::new(theme))
        .unwrap();
    family::prepare(parsed.presentation(), &LayoutOptions::default(), session)
        .unwrap()
        .render_svg(&SvgRenderOptions::default(), &SvgDebugOptions::default())
        .unwrap()
        .svg()
        .to_owned()
}

fn try_render_xychart_svg_from_text(text: &str) -> Result<String, merman_render::Error> {
    try_render_xychart_svg_with_resource_policy(
        text,
        RenderResourcePolicy::unbounded_for_trusted_input(),
    )
}

fn try_render_xychart_svg_with_resource_policy(
    text: &str,
    resource_policy: RenderResourcePolicy,
) -> Result<String, merman_render::Error> {
    let session = RenderEnvironment::deterministic()
        .with_resource_policy(resource_policy)
        .begin_session()
        .expect("begin XYChart resource-bound session");
    let engine = legacy_init_theme_compat_engine();
    let parsed = engine
        .parse_diagram_for_render_model_sync(text, ParseOptions::default())
        .expect("parse ok")
        .expect("diagram detected")
        .into_presentation();
    let artifact = family::prepare(&parsed, &LayoutOptions::default(), session)?;

    Ok(artifact
        .render_svg(&SvgRenderOptions::default(), &SvgDebugOptions::default())?
        .svg()
        .to_owned())
}

fn xychart_svg_path_points(path: roxmltree::Node<'_, '_>) -> Vec<(f64, f64)> {
    kurbo::BezPath::from_svg(path.attribute("d").expect("SVG path data"))
        .expect("valid SVG path")
        .elements()
        .iter()
        .filter_map(|element| match element {
            kurbo::PathEl::MoveTo(point) | kurbo::PathEl::LineTo(point) => Some((point.x, point.y)),
            _ => None,
        })
        .collect()
}

#[test]
fn xychart_numeric_axis_and_bar_labels_use_javascript_number_text() {
    for (token, expected) in [
        ("0.000001", "0.000001"),
        ("0.0000001", "1e-7"),
        ("1000000000000000000000", "1e+21"),
        ("1000000000000000128", "1000000000000000100"),
        ("-0", "0"),
    ] {
        for (orientation, value_axis) in [("", "left-axis"), (" horizontal", "top-axis")] {
            let source = format!(
                concat!(
                    "---\nconfig:\n  xyChart:\n    showDataLabel: true\n---\n",
                    "xychart{orientation}\nx-axis [A]\nbar [{token}]\n",
                ),
                orientation = orientation,
                token = token,
            );
            let layout = layout_xychart_from_text(&source);
            assert_eq!(layout.label_data, [expected], "bar label for {source}");
            let labels = layout
                .drawables
                .iter()
                .find_map(|drawable| match drawable {
                    XyChartDrawableElem::Text { group_texts, data }
                        if group_texts == &[value_axis, "label"] =>
                    {
                        Some(data)
                    }
                    _ => None,
                })
                .expect("numeric axis labels");
            assert_eq!(
                labels
                    .iter()
                    .map(|label| label.text.as_str())
                    .collect::<Vec<_>>(),
                [expected],
                "axis label for {source}",
            );

            let svg = render_xychart_svg_from_text(&source);
            let document = roxmltree::Document::parse(&svg).unwrap();
            for group in [value_axis, "bar-plot-0"] {
                let text = xychart_plot_group(&document, group)
                    .descendants()
                    .filter(|node| node.has_tag_name("text"))
                    .map(|node| node.text().expect("numeric label text"))
                    .collect::<Vec<_>>();
                assert_eq!(text, [expected], "{group} labels for {source}");
            }
        }
    }
}

#[test]
fn xychart_infinite_y_axis_keeps_zero_size_bars_without_numeric_ticks() {
    let overflow = format!("2{}", "0".repeat(308));
    for (orientation, value_axis, zero_dimension) in [
        ("", "left-axis", "height"),
        (" horizontal", "top-axis", "width"),
    ] {
        let source =
            format!("xychart{orientation}\nx-axis [A]\ny-axis 0 --> {overflow}\nbar [1]\n",);
        let layout = layout_xychart_from_text(&source);
        let bars = layout
            .drawables
            .iter()
            .find_map(|drawable| match drawable {
                XyChartDrawableElem::Rect { group_texts, data }
                    if group_texts == &["plot", "bar-plot-0"] =>
                {
                    Some(data)
                }
                _ => None,
            })
            .expect("bar plot rectangles");
        assert_eq!(bars.len(), 1);
        if orientation.is_empty() {
            assert_eq!(bars[0].height, 0.0);
            assert!(bars[0].width > 0.0);
        } else {
            assert_eq!(bars[0].width, 0.0);
            assert!(bars[0].height > 0.0);
        }
        for drawable in &layout.drawables {
            match drawable {
                XyChartDrawableElem::Text { group_texts, data }
                    if group_texts == &[value_axis, "label"] =>
                {
                    assert!(data.is_empty(), "an infinite domain has no numeric labels");
                }
                XyChartDrawableElem::Path { group_texts, data }
                    if group_texts == &[value_axis, "ticks"] =>
                {
                    assert!(data.is_empty(), "an infinite domain has no numeric ticks");
                }
                _ => {}
            }
        }

        let svg = render_xychart_svg_from_text(&source);
        let document = roxmltree::Document::parse(&svg).unwrap();
        let axis = xychart_plot_group(&document, value_axis);
        assert!(!axis.descendants().any(|node| node.has_tag_name("text")));
        assert!(!axis.descendants().any(|node| {
            node.attribute("class") == Some("ticks")
                && node.descendants().any(|child| child.has_tag_name("path"))
        }));
        let bar = xychart_plot_group(&document, "bar-plot-0")
            .children()
            .find(|node| node.has_tag_name("rect"))
            .expect("rendered zero-size bar");
        assert_eq!(bar.attribute(zero_dimension), Some("0"));
    }
}

#[test]
fn xychart_duplicate_band_line_svg_keeps_endpoints_ticks_and_label_occurrences() {
    let svg = render_xychart_svg_from_text(concat!(
        "xychart\n",
        "x-axis [\" A \", B, A]\n",
        "y-axis 0 --> 4\n",
        "line [1 \"same\", 2, 1 \"same\"]\n",
    ));
    let control = render_xychart_svg_from_text(concat!(
        "xychart\n",
        "x-axis [A, B]\n",
        "y-axis 0 --> 4\n",
        "line [1 \"same\", 2]\n",
    ));
    let document = roxmltree::Document::parse(&svg).unwrap();
    let control_document = roxmltree::Document::parse(&control).unwrap();
    let plot = xychart_plot_group(&document, "line-plot-0");
    let points = xychart_svg_path_points(
        plot.children()
            .find(|node| node.has_tag_name("path"))
            .unwrap(),
    );
    let control_points = xychart_svg_path_points(
        xychart_plot_group(&control_document, "line-plot-0")
            .children()
            .find(|node| node.has_tag_name("path"))
            .unwrap(),
    );
    assert_eq!(points.len(), 3);
    assert_eq!(
        points[0], points[2],
        "equal values at the repeated category share geometry"
    );
    assert_eq!(&points[..2], control_points.as_slice());
    assert!(points[1].0 > points[0].0);

    let labels: Vec<_> = plot
        .descendants()
        .filter(|node| node.has_tag_name("text"))
        .collect();
    assert_eq!(labels.len(), 2);
    assert!(labels.iter().all(|label| label.text() == Some("same")));
    assert_eq!(
        labels[0].attribute("transform"),
        labels[1].attribute("transform")
    );
    let axis = xychart_plot_group(&document, "bottom-axis");
    let ticks: Vec<_> = axis
        .descendants()
        .filter(|node| node.has_tag_name("text"))
        .collect();
    assert_eq!(
        ticks
            .iter()
            .map(|node| node.text().unwrap())
            .collect::<Vec<_>>(),
        ["A", "B", "A"]
    );
    assert_eq!(
        ticks[0].attribute("transform"),
        ticks[2].attribute("transform")
    );
}

#[test]
fn xychart_horizontal_duplicate_band_bar_svg_uses_both_padded_endpoints() {
    let svg = render_xychart_svg_from_text(concat!(
        "xychart horizontal\n",
        "x-axis [A, B, A]\n",
        "y-axis 0 --> 4\n",
        "bar [1, 2, 1]\n",
    ));
    let document = roxmltree::Document::parse(&svg).unwrap();
    let bars: Vec<_> = xychart_plot_group(&document, "bar-plot-0")
        .children()
        .filter(|node| node.has_tag_name("rect"))
        .map(|node| {
            ["x", "y", "width", "height"]
                .map(|attribute| node.attribute(attribute).unwrap().parse::<f64>().unwrap())
        })
        .collect();
    assert_eq!(bars.len(), 3);
    assert_eq!(bars[0], bars[2]);
    assert!(bars[1][2] > bars[0][2]);
    let first_center = bars[0][1] + bars[0][3] / 2.0;
    let second_center = bars[1][1] + bars[1][3] / 2.0;
    assert!(second_center > first_center);
    let axis = xychart_plot_group(&document, "left-axis");
    let endpoints = xychart_svg_path_points(
        axis.descendants()
            .find(|node| node.has_tag_name("path"))
            .unwrap(),
    );
    assert_eq!(endpoints.len(), 2);
    assert!((first_center + second_center - endpoints[0].1 - endpoints[1].1).abs() < 0.001);
    let ticks: Vec<_> = axis
        .descendants()
        .filter(|node| node.has_tag_name("text"))
        .collect();
    assert_eq!(
        ticks
            .iter()
            .map(|node| node.text().unwrap())
            .collect::<Vec<_>>(),
        ["A", "B", "A"]
    );
    assert_eq!(
        ticks[0].attribute("transform"),
        ticks[2].attribute("transform")
    );
}

#[test]
fn xychart_duplicate_band_observes_cancellation() {
    use merman_model::OperationControl;

    let source = "xychart\nx-axis [A, B, A]\ny-axis 0 --> 4\nbar [1, 2, 1]\n";
    let parsed = legacy_init_theme_compat_engine()
        .parse_diagram_for_render_model_sync(source, ParseOptions::strict())
        .unwrap()
        .unwrap()
        .into_presentation();
    let environment = RenderEnvironment::deterministic()
        .with_resource_policy(RenderResourcePolicy::unbounded_for_trusted_input());
    let control = OperationControl::new();
    let session = environment
        .begin_session_with_control(control.clone())
        .unwrap();
    control.cancel();
    assert!(matches!(
        family::prepare(&parsed, &LayoutOptions::default(), session),
        Err(merman_render::Error::Cancelled(_))
    ));

    let control = OperationControl::new();
    let artifact = family::prepare(
        &parsed,
        &LayoutOptions::default(),
        environment
            .begin_session_with_control(control.clone())
            .unwrap(),
    )
    .unwrap();
    control.cancel();
    assert!(matches!(
        artifact.render_svg(&SvgRenderOptions::default(), &SvgDebugOptions::default()),
        Err(merman_render::Error::Cancelled(_))
    ));
}

#[test]
fn xychart_orientation_token_case_reaches_bar_geometry_like_upstream() {
    for (orientation, horizontal) in [
        ("horizontal", true),
        ("HORIZONTAL", false),
        ("Horizontal", false),
        ("vertical", false),
    ] {
        let source = format!("xychart {orientation}\nx-axis [A, B]\ny-axis 0 --> 10\nbar [2, 8]");
        let svg = render_xychart_svg_from_text(&source);
        let document = roxmltree::Document::parse(&svg).unwrap();
        let bars: Vec<_> = xychart_plot_group(&document, "bar-plot-0")
            .children()
            .filter(|node| node.has_tag_name("rect"))
            .map(|node| {
                ["x", "y", "width", "height"]
                    .map(|attribute| node.attribute(attribute).unwrap().parse::<f64>().unwrap())
            })
            .collect();
        assert_eq!(bars.len(), 2);
        let [first, second] = [bars[0], bars[1]];
        if horizontal {
            assert!((first[0] - second[0]).abs() < 0.001);
            assert!(first[1] < second[1]);
            assert!((first[3] - second[3]).abs() < 0.001);
            assert!(first[2] > 0.0 && second[2] > first[2]);
        } else {
            assert!(first[0] < second[0]);
            assert!(first[1] > second[1]);
            assert!((first[2] - second[2]).abs() < 0.001);
            assert!(first[3] > 0.0 && second[3] > first[3]);
            assert!((first[1] + first[3] - second[1] - second[3]).abs() < 0.001);
        }
    }
}

#[test]
fn xychart_same_line_axis_and_bar_statements_reach_visible_svg() {
    let compact = render_xychart_svg_from_text("xychart x-axis [A, B] bar [1, 2]");
    let separated = render_xychart_svg_from_text("xychart\nx-axis [A, B]\nbar [1, 2]");
    let compact_document = roxmltree::Document::parse(&compact).unwrap();
    let separated_document = roxmltree::Document::parse(&separated).unwrap();
    let bar_geometry = |document: &roxmltree::Document<'_>| {
        xychart_plot_group(document, "bar-plot-0")
            .children()
            .filter(|node| node.has_tag_name("rect"))
            .map(|node| {
                ["x", "y", "width", "height"]
                    .map(|attribute| node.attribute(attribute).unwrap().parse::<f64>().unwrap())
            })
            .collect::<Vec<_>>()
    };
    let bars = bar_geometry(&compact_document);
    assert_eq!(bars.len(), 2);
    assert_eq!(bars, bar_geometry(&separated_document));
    assert!(bars[0][0] < bars[1][0]);
    assert!(bars[0][3] > 0.0);
    assert!(bars[1][3] > bars[0][3]);
    for label in ["A", "B"] {
        assert!(
            compact_document
                .descendants()
                .any(|node| node.has_tag_name("text") && node.text() == Some(label))
        );
    }
}

#[test]
fn xychart_family_svg_accepts_exact_max_svg_bytes_and_rejects_one_byte_less() {
    let source = "xychart\n  x-axis [A, B, A]\n  y-axis 0 --> 10\n  bar [4, 7, 4]\n";
    let baseline = try_render_xychart_svg_with_resource_policy(
        source,
        RenderResourcePolicy::unbounded_for_trusted_input(),
    )
    .expect("render the unbounded XYChart baseline");
    let exact_bytes = baseline.len();
    assert!(exact_bytes > 1, "XYChart fixture must emit a non-empty SVG");

    let exact_policy = RenderResourcePolicy::unbounded_for_trusted_input()
        .with_limit(ResourceLimitId::MaxSvgBytes, exact_bytes)
        .expect("valid exact XYChart SVG byte ceiling");
    let exact = try_render_xychart_svg_with_resource_policy(source, exact_policy)
        .expect("the exact XYChart family SVG byte ceiling must succeed");
    assert_eq!(exact.as_bytes(), baseline.as_bytes());

    let below_exact = exact_bytes - 1;
    let below_policy = RenderResourcePolicy::unbounded_for_trusted_input()
        .with_limit(ResourceLimitId::MaxSvgBytes, below_exact)
        .expect("valid below-exact XYChart SVG byte ceiling");
    let error = try_render_xychart_svg_with_resource_policy(source, below_policy)
        .expect_err("one byte below the XYChart family SVG size must fail");
    let merman_render::Error::ResourceLimitExceeded(limit) = error else {
        panic!("expected XYChart MaxSvgBytes rejection, got {error}");
    };
    assert_eq!(limit.cause, ResourceLimitCause::Ceiling);
    assert_eq!(limit.phase, ResourceLimitPhase::SvgOutput);
    assert_eq!(limit.limit, ResourceLimitId::MaxSvgBytes.as_str());
    assert_eq!(limit.max, below_exact);
    assert!(limit.actual > limit.max);
    assert!(limit.explicit_overrides.iter().any(|resource_override| {
        resource_override.id == ResourceLimitId::MaxSvgBytes
            && resource_override.value == below_exact
    }));
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
fn xychart_huge_finite_bar_dimensions_render_bounded_svg_and_exceed_only_the_native_domain() {
    let svg = try_render_xychart_svg_from_text(
        r#"---
config:
  xyChart:
    height: "1e308"
    showDataLabel: true
---
xychart horizontal
  x-axis [A, A]
  y-axis 0 --> 100
  bar [73, 73]
"#,
    )
    .expect("finite geometry must remain available to ordinary SVG generation");
    assert!(svg.len() < 64 * 1024, "one bar must produce bounded output");
    assert!(!svg.contains("NaN") && !svg.contains("Infinity"));
    let document = roxmltree::Document::parse(&svg).expect("well-formed XY Chart SVG");
    let view_box: Vec<f64> = document
        .root_element()
        .attribute("viewBox")
        .unwrap()
        .split_ascii_whitespace()
        .map(|value| value.parse().unwrap())
        .collect();
    assert_eq!(view_box.len(), 4);
    assert!(view_box.iter().all(|value| value.is_finite()));
    assert_eq!(view_box[3], 1e308);
    for node in document.descendants().filter(roxmltree::Node::is_element) {
        for attribute in node.attributes() {
            if matches!(
                attribute.name(),
                "x" | "y" | "width" | "height" | "font-size" | "stroke-width"
            ) && let Ok(value) = attribute.value().parse::<f64>()
            {
                assert!(value.is_finite(), "non-finite {}", attribute.name());
            }
        }
    }

    let session = RenderEnvironment::deterministic().begin_session().unwrap();
    let error = merman_render::svg::finalize_resvg_svg(&svg, &session)
        .expect_err("native preparation must enforce its own coordinate domain");
    let merman_render::Error::ResourceLimitExceeded(limit) = error else {
        panic!("expected native coordinate limit, got {error}");
    };
    assert_eq!(limit.limit, "svg_backend_coordinate_magnitude");
    assert!(limit.actual > limit.max);
}

#[test]
fn xychart_rejects_truly_unrepresentable_plot_geometry() {
    let error = try_render_xychart_svg_from_text(
        r#"---
config:
  xyChart:
    height: "1.7e308"
    plotReservedSpacePercent: 200
---
xychart horizontal
  x-axis [A]
  y-axis 0 --> 100
  bar [73]
"#,
    )
    .expect_err("a plot exceeding the finite number domain cannot be generated");
    assert!(matches!(error, merman_render::Error::InvalidModel { .. }));
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

#[test]
fn xychart_axis_config_colors_follow_logical_axes_in_both_orientations() {
    for horizontal in [false, true] {
        let config = serde_json::json!({"xyChart":{"showDataLabel":true},"themeVariables":{"xyChart":{
            "titleColor":"#100001", "dataLabelColor":"#100002",
            "xAxisTitleColor":"#200001","xAxisLabelColor":"#200002","xAxisLineColor":"#200003","xAxisTickColor":"#200004",
            "yAxisTitleColor":"#300001","yAxisLabelColor":"#300002","yAxisLineColor":"#300003","yAxisTickColor":"#300004"
        }}});
        let source = format!(
            "---\nconfig: {config}\n---\nxychart{}\ntitle Revenue\nx-axis Months [A, B]\ny-axis Amount 0 --> 10\nbar [3, 7]\n",
            if horizontal { " horizontal" } else { "" }
        );
        let rendered = render_xychart_svg_from_text(&source);
        let doc = roxmltree::Document::parse(&rendered).unwrap();
        for (group, prefix, title_text) in if horizontal {
            [
                ("left-axis", "#20000", "Months"),
                ("top-axis", "#30000", "Amount"),
            ]
        } else {
            [
                ("bottom-axis", "#20000", "Months"),
                ("left-axis", "#30000", "Amount"),
            ]
        } {
            let axis = xychart_plot_group(&doc, group);
            for (class, tag, attr, suffix) in [
                ("title", "text", "fill", "1"),
                ("label", "text", "fill", "2"),
                (
                    if group == "left-axis" {
                        "axisl-line"
                    } else {
                        "axis-line"
                    },
                    "path",
                    "stroke",
                    "3",
                ),
                ("ticks", "path", "stroke", "4"),
            ] {
                let subgroup = axis
                    .children()
                    .find(|node| node.attribute("class") == Some(class))
                    .unwrap();
                let terminals = subgroup
                    .children()
                    .filter(|node| node.has_tag_name(tag))
                    .collect::<Vec<_>>();
                assert!(!terminals.is_empty(), "{group}/{class}");
                assert!(
                    terminals
                        .iter()
                        .all(|node| node.attribute(attr)
                            == Some(format!("{prefix}{suffix}").as_str())),
                    "{group}/{class}"
                );
                if class == "title" {
                    assert_eq!(terminals[0].text(), Some(title_text));
                }
            }
        }
        for (group, color) in [("chart-title", "#100001"), ("bar-plot-0", "#100002")] {
            assert!(
                xychart_plot_group(&doc, group)
                    .children()
                    .filter(|node| node.has_tag_name("text"))
                    .all(|node| node.attribute("fill") == Some(color))
            );
        }
    }
}

#[test]
fn xychart_svg_title_uses_plot_center_after_axis_and_legend_space() {
    use kurbo::Shape;
    for orientation in ["vertical", "horizontal"] {
        for legend in [false, true] {
            let source = include_str!("../../../fixtures/xychart/title_partial_theme.mmd")
                .replace("xychart vertical", &format!("xychart {orientation}"))
                .replace("showLegend: true", &format!("showLegend: {legend}"));
            let svg = render_xychart_svg_from_text(&source);
            let document = roxmltree::Document::parse(&svg).unwrap();
            let axis_name = if orientation == "horizontal" {
                "top-axis"
            } else {
                "bottom-axis"
            };
            let axis = document
                .descendants()
                .find(|node| node.attribute("class") == Some(axis_name))
                .unwrap();
            let axis_line = axis
                .descendants()
                .find(|node| node.attribute("class") == Some("axis-line"))
                .unwrap();
            let path = axis_line
                .descendants()
                .find(|node| node.has_tag_name("path"))
                .unwrap();
            let bounds = kurbo::BezPath::from_svg(path.attribute("d").unwrap())
                .unwrap()
                .bounding_box();
            let title = document
                .descendants()
                .find(|node| node.has_tag_name("text") && node.text() == Some("Plot title"))
                .unwrap();
            let title_x: f64 = title
                .attribute("transform")
                .unwrap()
                .strip_prefix("translate(")
                .unwrap()
                .split_once(',')
                .unwrap()
                .0
                .parse()
                .unwrap();
            assert!((title_x - (bounds.x0 + bounds.x1) / 2.0).abs() < 0.001);
        }
    }
}

#[test]
fn xychart_svg_omits_title_when_plot_reservation_leaves_too_little_height() {
    for (height, visible) in [(60, false), (100, true)] {
        let source = format!(
            r#"---
config:
  xyChart:
    height: {height}
---
xychart
  title "Fits when tall"
  x-axis [a, b, c]
  y-axis 0 --> 100
  bar [10, 20, 30]
"#
        );
        let svg = render_xychart_svg_from_text(&source);
        let document = roxmltree::Document::parse(&svg).unwrap();
        assert_eq!(
            document
                .descendants()
                .any(|node| node.attribute("class") == Some("chart-title")),
            visible
        );
        let bars = document
            .descendants()
            .find(|node| node.attribute("class") == Some("bar-plot-0"))
            .unwrap();
        for bar in bars.children().filter(|node| node.has_tag_name("rect")) {
            let y: f64 = bar.attribute("y").unwrap().parse().unwrap();
            let bar_height: f64 = bar.attribute("height").unwrap().parse().unwrap();
            assert!(y + bar_height <= height as f64 + 0.001);
        }
    }
}

#[test]
fn xychart_svg_preserves_db_accessibility_text() {
    for (title, description, expected_title, expected_description) in [
        (
            "Accessible <!--a11y-->",
            "First\n<!--a11y-->",
            Some("Accessible "),
            Some("First\n"),
        ),
        (
            "<!--a11y-->\u{0085}<!--a11y-->",
            "<!--a11y-->\u{0085}<!--a11y-->",
            Some("\u{0085}"),
            Some("\u{0085}"),
        ),
        ("<!--a11y-->", "<!--a11y-->", None, None),
    ] {
        let source = format!(
            "xychart-beta\naccTitle: {title}\naccDescr {{\n{description}\n}}\nx-axis [a,b]\nbar [1,2]\n"
        );
        let svg = render_xychart_svg_from_text(&source);
        common::assert_svg_accessibility(&svg, expected_title, expected_description);
    }
}

#[test]
fn xychart_absolute_title_and_axis_sizes_share_layout_and_svg_sizes() {
    for orientation in ["", " horizontal"] {
        let render = |size| {
            let config = serde_json::json!({"xyChart": {
                "titleFontSize": size,
                "showLegend": false,
                "xAxis": {"titleFontSize": size, "labelFontSize": size},
                "yAxis": {"titleFontSize": size, "labelFontSize": size}
            }});
            let source = format!(
                "---\nconfig: {config}\n---\nxychart{orientation}\ntitle Revenue\nx-axis Months [A, B]\ny-axis Amount 0 --> 10\nbar [3, 7]\n"
            );
            (
                serde_json::to_value(layout_xychart_from_text(&source)).unwrap(),
                render_xychart_svg_from_text(&source),
            )
        };
        for (absolute, pixels, expected) in [
            ("24pt", "32px", "32"),
            ("2pc", "32px", "32"),
            ("0.3333333333333333in", "32px", "32"),
            ("0pt", "0px", "0"),
        ] {
            let (layout, svg) = render(absolute);
            let (px_layout, px_svg) = render(pixels);
            assert_eq!(layout, px_layout, "{orientation}: {absolute}");
            assert_eq!(svg, px_svg, "{orientation}: {absolute}");
            let document = roxmltree::Document::parse(&svg).unwrap();
            for text in ["Revenue", "Months", "Amount", "A", "0"] {
                let label = document
                    .descendants()
                    .find(|node| node.has_tag_name("text") && node.text() == Some(text))
                    .unwrap();
                assert_eq!(
                    label.attribute("font-size"),
                    Some(expected),
                    "{orientation}: {absolute}: {text}"
                );
            }
        }
    }
}

#[test]
fn xychart_preserves_authored_css_in_font_sizes_and_series_paint() {
    let source = "xychart\ntitle Revenue\nx-axis [A]\ny-axis 0 --> 10\nbar [4]\n";
    let palette = "url(\"#missing\") red";
    for (size, emitted_size) in [
        ("24px !important", "inherit"),
        ("-1e-50px", "inherit"),
        ("var(--xy-size, 0px)", "var(--xy-size, 0px)"),
        ("calc(0px + 1vw)", "calc(0px + 1vw)"),
    ] {
        let config = serde_json::json!({
            "themeVariables": {"fontSize": "32px", "xyChart": {"plotColorPalette": palette}},
            "xyChart": {"titleFontSize": size}
        });
        let parsed = MermaidEngine::new()
            .with_site_config(SourcePresentationConfig::from_value(config))
            .parse_diagram_for_render_model_sync(source, ParseOptions::strict())
            .unwrap()
            .unwrap();
        let session = RenderEnvironment::deterministic().begin_session().unwrap();
        let prepared =
            family::prepare(parsed.presentation(), &LayoutOptions::default(), session).unwrap();
        let layout: XyChartDiagramLayout = serde_json::from_value(
            prepared.layout_json().unwrap()["layout"]["XyChartDiagram"].clone(),
        )
        .unwrap();
        assert!(layout.drawables.iter().any(|drawable| matches!(
            drawable,
            XyChartDrawableElem::Text { data, .. }
                if data.iter().any(|label| label.text == "Revenue" && label.font_size == 32.0)
        )));
        let rendered = prepared
            .render_svg(&SvgRenderOptions::default(), &SvgDebugOptions::default())
            .unwrap();
        let document = roxmltree::Document::parse(rendered.svg()).unwrap();
        let label = xychart_plot_group(&document, "chart-title")
            .children()
            .find(|child| child.has_tag_name("text"))
            .unwrap();
        assert_eq!(label.attribute("font-size"), Some(emitted_size), "{size}");
        let bar = xychart_plot_group(&document, "bar-plot-0")
            .children()
            .find(|child| child.has_tag_name("rect"))
            .unwrap();
        assert_eq!(bar.attribute("fill"), Some(palette));
        assert_eq!(bar.attribute("stroke"), Some(palette));
    }
}

#[test]
fn native_xychart_guides_wireframe_and_line_markers_keep_their_paint_and_order() {
    let source = "\
---
config:
  xyChart:
    yAxis:
      tickWidth: 4
---
xychart
  x-axis [A, B]
  y-axis 0 --> 10
  bar Sales [3, 7]
  line Trend [2, 8]
";
    let theme = ThemeCompiler::new()
        .compile_json(
            br##"{"schema_version":2,"extends":"blueprint","roles":{"surface.node":"#12456A","stroke.guide":"#416182"}}"##,
            None,
        )
        .unwrap();
    let svg = render_native_xychart_svg(source, theme);
    let document = roxmltree::Document::parse(&svg).unwrap();
    let grid = xychart_plot_group(&document, "gridlines");
    let plot = xychart_plot_group(&document, "plot");
    assert_eq!(grid.parent(), plot.parent());
    assert!(grid.range().end <= plot.range().start);
    let guides: Vec<_> = grid.children().collect();
    assert!(!guides.is_empty());
    for guide in guides {
        assert!(guide.has_tag_name("path"));
        assert_eq!(guide.attribute("stroke"), Some("#416182"));
        assert_eq!(guide.attribute("stroke-width"), Some("0.8"));
        assert_eq!(guide.attribute("stroke-dasharray"), Some("3 3"));
        let points = xychart_svg_path_points(guide);
        assert_eq!(points.len(), 2);
        assert_eq!(points[0].1, points[1].1);
    }
    let ticks = xychart_plot_group(&document, "left-axis")
        .children()
        .find(|node| node.attribute("class") == Some("ticks"))
        .unwrap();
    assert!(
        ticks
            .children()
            .all(|tick| tick.attribute("stroke-width") == Some("4"))
    );

    let bars = xychart_plot_group(&document, "bar-plot-0");
    assert_eq!(bars.children().count(), 2);
    for bar in bars.children() {
        assert!(bar.has_tag_name("rect"));
        assert_eq!(bar.attribute("fill"), Some("#7AE6FF"));
        assert_eq!(bar.attribute("fill-opacity"), Some("0.12"));
        assert_eq!(bar.attribute("stroke"), bar.attribute("fill"));
        assert_eq!(bar.attribute("stroke-width"), Some("1.5"));
        assert!(bar.attribute("stroke-dasharray").is_none());
    }

    let line = xychart_plot_group(&document, "line-plot-1")
        .first_element_child()
        .unwrap();
    let points = xychart_svg_path_points(line);
    let markers: Vec<_> = xychart_plot_group(&document, "line-markers-1")
        .children()
        .collect();
    assert_eq!(markers.len(), 2);
    assert_eq!(markers.len(), points.len());
    for (marker, (x, y)) in markers.into_iter().zip(points) {
        assert!(marker.has_tag_name("circle"));
        assert_eq!(marker.attribute("r"), Some("3.2"));
        assert_eq!(marker.attribute("fill"), Some("#12456A"));
        assert_eq!(marker.attribute("stroke"), line.attribute("stroke"));
        assert_eq!(marker.attribute("stroke-width"), Some("1.8"));
        for (name, expected) in [("cx", x), ("cy", y)] {
            let actual = marker.attribute(name).unwrap().parse::<f64>().unwrap();
            assert!(
                (actual - expected).abs() < 1e-9,
                "{name}: {actual} vs {expected}"
            );
        }
    }

    let classic_svg = render_xychart_svg_from_text(source);
    let classic = roxmltree::Document::parse(&classic_svg).unwrap();
    assert!(classic.descendants().all(|node| !matches!(
        node.attribute("class"),
        Some("gridlines" | "line-markers-1")
    )));
    assert!(
        xychart_plot_group(&classic, "bar-plot-0")
            .children()
            .all(|bar| bar.has_tag_name("rect") && bar.attribute("fill-opacity").is_none())
    );
}

#[test]
fn native_xychart_rounds_only_the_bar_end_away_from_the_baseline() {
    use kurbo::{Point, Shape};

    let compiler = ThemeCompiler::new();
    let rounded = compiler
        .compile_preset(BuiltinPreset::Studio, None)
        .unwrap();
    let square = compiler
        .compile_json(
            br#"{"schema_version":2,"extends":"studio","form":{"chart":{"bar_radius":0}}}"#,
            None,
        )
        .unwrap();
    let top = [true, true, false, false];
    let right = [false, true, true, false];
    let bottom = [false, false, true, true];
    let left = [true, false, false, true];
    for (orientation, min, max, rounded_corners) in [
        ("vertical", -10, 10, [bottom, top]),
        ("horizontal", -10, 10, [left, right]),
        ("vertical", 10, -10, [top, bottom]),
        ("horizontal", 10, -10, [right, left]),
    ] {
        let source =
            format!("xychart {orientation}\nx-axis [A, B]\ny-axis {min} --> {max}\nbar [-5, 5]\n");
        let svg = render_native_xychart_svg(&source, rounded.clone());
        let control_svg = render_native_xychart_svg(&source, square.clone());
        let document = roxmltree::Document::parse(&svg).unwrap();
        let control = roxmltree::Document::parse(&control_svg).unwrap();
        assert_eq!(
            document.root_element().attribute("viewBox"),
            control.root_element().attribute("viewBox")
        );
        let bars = xychart_plot_group(&document, "bar-plot-0");
        let controls = xychart_plot_group(&control, "bar-plot-0");
        assert_eq!(bars.children().count(), 2);
        assert_eq!(controls.children().count(), 2);
        let depth = document
            .descendants()
            .filter(|node| node.attribute("data-merman-theme-depth") == Some("node"));
        assert_eq!(depth.clone().count(), 2);

        for (((bar, control), depth), rounded) in bars
            .children()
            .zip(controls.children())
            .zip(depth)
            .zip(rounded_corners)
        {
            assert!(bar.has_tag_name("path"));
            assert!(control.has_tag_name("rect"));
            let [x, y, width, height] = ["x", "y", "width", "height"]
                .map(|name| control.attribute(name).unwrap().parse::<f64>().unwrap());
            let silhouette = depth
                .children()
                .find(|node| node.has_tag_name("path"))
                .unwrap();
            for (node, (x, y)) in [(bar, (x, y)), (silhouette, (0.0, 0.0))] {
                let shape = kurbo::BezPath::from_svg(node.attribute("d").unwrap()).unwrap();
                assert_eq!(
                    shape
                        .elements()
                        .iter()
                        .filter(|element| matches!(element, kurbo::PathEl::CurveTo(..)))
                        .count(),
                    2
                );
                let bounds = shape.bounding_box();
                assert!((bounds.x0 - x).abs() < 1e-9 && (bounds.y0 - y).abs() < 1e-9);
                assert!(
                    (bounds.width() - width).abs() < 1e-9
                        && (bounds.height() - height).abs() < 1e-9
                );
                let inset = 0.25;
                let near_corner = [
                    Point::new(x + inset, y + inset),
                    Point::new(x + width - inset, y + inset),
                    Point::new(x + width - inset, y + height - inset),
                    Point::new(x + inset, y + height - inset),
                ];
                assert_eq!(
                    near_corner.map(|point| !shape.contains(point)),
                    rounded,
                    "{orientation} {min} --> {max}"
                );
            }
        }
    }
}

#[test]
fn native_xychart_legend_uses_the_visible_bar_series_pattern() {
    let source = "\
xychart
  title Revenue
  x-axis [A, B]
  y-axis 0 --> 10
  bar [2, 3]
  line Trend [4, 5]
  bar Sales [6, 7]
";
    let theme = ThemeCompiler::new()
        .compile_preset(BuiltinPreset::Monograph, None)
        .unwrap();
    let svg = render_native_xychart_svg(source, theme);
    let document = roxmltree::Document::parse(&svg).unwrap();
    let bar = xychart_plot_group(&document, "bar-plot-2")
        .children()
        .find(|child| child.has_tag_name("rect"))
        .unwrap();
    let legend = xychart_plot_group(&document, "legend");
    let swatches = legend
        .descendants()
        .filter(|child| child.has_tag_name("rect"))
        .collect::<Vec<_>>();
    assert_eq!(swatches.len(), 1);
    assert_eq!(bar.attribute("fill"), Some("url(#xychart-xy-series-2)"));
    assert_eq!(swatches[0].attribute("fill"), bar.attribute("fill"));
    assert!(legend.descendants().any(|child| child.has_tag_name("path")));
}
