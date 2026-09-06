mod common;

use std::sync::Arc;

use common::legacy_init_theme_compat_engine;
use merman_core::{Engine, MermaidConfig, ParseOptions};
use merman_render::DiagramFamilyId;
use merman_render::LayoutOptions;
use merman_render::diagram_theme::{
    DiagramTheme, DiagramThemeCompiler, DiagramThemeSpec, FontStack, OrdinalPalette,
    ThemeColorValue, ThemePortabilityRequirement, ThemeRuleSet, ThemeTarget, ThemeTextStyle,
    TypographySpec,
};
use merman_render::environment::{
    MeasurementProfileId, RenderEnvironment, TextMeasurementPolicy, TextMeasurementProfile,
    TextMeasurementProfileIdentity,
};
use merman_render::family;
use merman_render::model::{XyChartDiagramLayout, XyChartDrawableElem};
use merman_render::resources::{
    RenderResourcePolicy, ResourceLimitCause, ResourceLimitId, ResourceLimitPhase,
};
use merman_render::svg::{SvgDebugOptions, SvgRenderOptions};
use merman_render::text::{TextMeasurer, TextMetrics, TextStyle};

fn xychart_typography_theme(typography: ThemeTextStyle) -> DiagramTheme {
    DiagramThemeCompiler::new()
        .compile(DiagramThemeSpec::new().with_typography(
            TypographySpec::default().with_family_style(DiagramFamilyId::XY_CHART, typography),
        ))
        .expect("compile XY Chart typography theme")
}

fn xychart_series_palette_theme(colors: &[&str]) -> DiagramTheme {
    let palette =
        OrdinalPalette::new(colors.iter().map(|color| {
            ThemeColorValue::parse(*color).expect("valid XY Chart series palette color")
        }))
        .expect("non-empty XY Chart series palette");
    DiagramThemeCompiler::new()
        .compile(DiagramThemeSpec::new().with_styles(
            ThemeRuleSet::default().with_ordinal_palette(ThemeTarget::ChartSeries, palette),
        ))
        .expect("compile XY Chart series palette theme")
}

fn render_xychart_with_theme_and_engine(
    source: &str,
    theme: &DiagramTheme,
    engine: Engine,
) -> family::RenderedFamilySvg {
    let parsed = merman_render::__private::install_parse_compatibility(theme, engine)
        .parse_diagram_for_render_model_sync(source, ParseOptions::strict())
        .expect("parse themed XY Chart")
        .expect("detect themed XY Chart");
    let session = RenderEnvironment::deterministic()
        .with_theme_portability_requirement(ThemePortabilityRequirement::RequirePortable)
        .begin_session_with_theme(theme)
        .expect("begin strict portable XY Chart session");
    family::prepare(parsed, &LayoutOptions::default(), session)
        .expect("prepare themed XY Chart")
        .render_svg(&SvgRenderOptions::default(), &SvgDebugOptions::default())
        .expect("render themed XY Chart")
}

fn try_render_xychart_with_theme(
    source: &str,
    theme: &DiagramTheme,
    engine: Engine,
    environment: &RenderEnvironment,
    diagram_id: &str,
) -> merman_render::Result<(XyChartDiagramLayout, family::RenderedFamilySvg)> {
    let parsed = merman_render::__private::install_parse_compatibility(theme, engine)
        .parse_diagram_for_render_model_sync(source, ParseOptions::strict())
        .expect("parse themed XY Chart")
        .expect("detect themed XY Chart");
    let session = environment
        .begin_session_with_theme(theme)
        .expect("begin themed XY Chart session");
    let artifact = family::prepare(parsed, &LayoutOptions::default(), session)?;
    let projection = artifact.layout_json()?;
    let layout = serde_json::from_value(projection["layout"]["XyChartDiagram"].clone())
        .expect("XY Chart layout projection");
    let rendered = artifact.render_svg(
        &SvgRenderOptions {
            diagram_id: Some(diagram_id.to_string()),
            ..SvgRenderOptions::default()
        },
        &SvgDebugOptions::default(),
    )?;
    Ok((layout, rendered))
}

#[derive(Debug)]
struct XyChartTypedFontProbeMeasurer {
    expected_font_family: String,
}

impl TextMeasurer for XyChartTypedFontProbeMeasurer {
    fn measure(&self, text: &str, style: &TextStyle) -> TextMetrics {
        assert_eq!(
            style.font_family.as_deref(),
            Some(self.expected_font_family.as_str()),
            "XY Chart layout measurement must use the final font-family winner"
        );
        TextMetrics {
            width: text.chars().count() as f64 * 10.0,
            height: style.font_size,
            line_count: 1,
        }
    }
}

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
        .expect("diagram detected");
    let artifact = family::prepare(parsed, &LayoutOptions::default(), session).expect("layout ok");
    let projection = artifact.layout_json().expect("serialize XYChart layout");
    serde_json::from_value(projection["layout"]["XyChartDiagram"].clone())
        .expect("XYChart layout projection")
}

fn render_xychart_svg_from_text(text: &str) -> String {
    try_render_xychart_svg_from_text(text).expect("render svg")
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
        .expect("diagram detected");
    let artifact = family::prepare(parsed, &LayoutOptions::default(), session).expect("layout ok");

    Ok(artifact
        .render_svg(&SvgRenderOptions::default(), &SvgDebugOptions::default())?
        .svg()
        .to_owned())
}

#[test]
fn xychart_family_svg_accepts_exact_max_svg_bytes_and_rejects_one_byte_less() {
    let source = "xychart\n  x-axis [A, B]\n  y-axis 0 --> 10\n  bar [4, 7]\n";
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
fn xychart_huge_finite_bar_dimensions_do_not_spin_the_data_label_renderer() {
    let error = try_render_xychart_svg_from_text(
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
    )
    .expect_err("huge finite geometry must fail before entering the SVG backend");
    let merman_render::Error::ResourceLimitExceeded(limit) = error else {
        panic!("expected typed resource-limit error, got {error}");
    };
    assert_eq!(limit.limit, "svg_backend_coordinate_magnitude");
    assert!(limit.actual > limit.max);
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
fn xychart_series_palette_reaches_bar_line_and_point_label_terminal_svg() {
    let theme = xychart_series_palette_theme(&["#123456", "#abcdef"]);
    let rendered = render_xychart_with_theme_and_engine(
        r##"---
config:
  xyChart:
    showDataLabel: true
---
xychart
  x-axis [A]
  y-axis 0 --> 10
  bar "Bars" [4]
  line "Trend" [7 "Trend point"]
"##,
        &theme,
        Engine::new(),
    );
    let document = roxmltree::Document::parse(rendered.svg()).expect("valid themed XY Chart SVG");

    let bar = xychart_plot_group(&document, "bar-plot-0");
    let bar_rect = bar
        .children()
        .find(|node| node.has_tag_name("rect"))
        .expect("themed XY Chart bar rect");
    assert_eq!(bar_rect.attribute("fill"), Some("#123456"));
    assert_eq!(bar_rect.attribute("stroke"), Some("#123456"));
    let bar_label = bar
        .children()
        .find(|node| node.has_tag_name("text"))
        .expect("XY Chart bar data label");
    assert_ne!(
        bar_label.attribute("fill"),
        Some("#123456"),
        "bar data labels are not ChartSeries palette surfaces"
    );

    let line = xychart_plot_group(&document, "line-plot-1");
    let line_path = line
        .children()
        .find(|node| node.has_tag_name("path"))
        .expect("themed XY Chart line path");
    assert_eq!(line_path.attribute("stroke"), Some("#abcdef"));
    let point_label = line
        .descendants()
        .find(|node| node.has_tag_name("text") && node.text() == Some("Trend point"))
        .expect("themed XY Chart line point label");
    assert_eq!(point_label.attribute("fill"), Some("#abcdef"));

    let legend = document
        .descendants()
        .find(|node| node.has_tag_name("g") && node.attribute("class") == Some("legend"))
        .expect("XY Chart legend");
    let legend_markers = legend
        .children()
        .find(|node| node.has_tag_name("g") && node.attribute("class") == Some("markers"))
        .expect("XY Chart legend markers");
    let legend_bar = legend_markers
        .children()
        .find(|node| node.has_tag_name("rect"))
        .expect("themed XY Chart legend bar marker");
    assert_eq!(legend_bar.attribute("fill"), Some("#123456"));
    assert_eq!(legend_bar.attribute("stroke"), Some("#123456"));
    let legend_line = legend_markers
        .children()
        .find(|node| node.has_tag_name("path"))
        .expect("themed XY Chart legend line marker");
    assert_eq!(legend_line.attribute("stroke"), Some("#abcdef"));

    let legend_labels = legend
        .children()
        .find(|node| node.has_tag_name("g") && node.attribute("class") == Some("label"))
        .expect("XY Chart legend labels");
    assert_eq!(
        legend_labels
            .children()
            .filter(|node| node.has_tag_name("text"))
            .filter_map(|node| node.text())
            .collect::<Vec<_>>(),
        ["Bars", "Trend"]
    );

    let completion = rendered.into_completion();
    let evidence = merman_render::__private::family_evidence(completion.report());
    assert_eq!(evidence.required_count(), 1);
    assert_eq!(evidence.applied_count(), 1);
    assert_eq!(evidence.not_applicable_count(), 0);
    assert_eq!(evidence.theme_residual_count(), 0);
}

#[test]
fn xychart_series_palette_supports_transparent_paint_and_cycles() {
    let theme = xychart_series_palette_theme(&["transparent", "#00aa00"]);
    let rendered = render_xychart_with_theme_and_engine(
        "xychart\n  x-axis [A]\n  y-axis 0 --> 10\n  bar [2]\n  line [4]\n  bar [6]\n",
        &theme,
        Engine::new(),
    );
    let document = roxmltree::Document::parse(rendered.svg()).expect("valid palette-cycle SVG");

    let first_bar = xychart_plot_group(&document, "bar-plot-0")
        .children()
        .find(|node| node.has_tag_name("rect"))
        .expect("first XY Chart bar");
    assert_eq!(first_bar.attribute("fill"), Some("#00000000"));
    assert_eq!(first_bar.attribute("stroke"), Some("#00000000"));

    let line = xychart_plot_group(&document, "line-plot-1")
        .children()
        .find(|node| node.has_tag_name("path"))
        .expect("second XY Chart line");
    assert_eq!(line.attribute("stroke"), Some("#00aa00"));

    let third_bar = xychart_plot_group(&document, "bar-plot-2")
        .children()
        .find(|node| node.has_tag_name("rect"))
        .expect("third XY Chart bar");
    assert_eq!(third_bar.attribute("fill"), Some("#00000000"));
    assert_eq!(third_bar.attribute("stroke"), Some("#00000000"));
}

#[test]
fn xychart_explicit_plot_palette_owners_outrank_typed_series_palette() {
    let theme = xychart_series_palette_theme(&["#123456"]);
    let cases = [
        (
            "xychart\n  x-axis [A]\n  y-axis 0 --> 10\n  bar [4]\n",
            Engine::new().with_site_config(MermaidConfig::from_value(serde_json::json!({
                "themeVariables": {
                    "xyChart": { "plotColorPalette": "#fedcba" }
                }
            }))),
        ),
        (
            concat!(
                "%%{init: {\"themeVariables\": {\"xyChart\": ",
                "{\"plotColorPalette\": \"#fedcba\"}}}}%%\n",
                "xychart\n  x-axis [A]\n  y-axis 0 --> 10\n  bar [4]\n",
            ),
            Engine::new().with_site_config(MermaidConfig::from_value(serde_json::json!({
                "secure": []
            }))),
        ),
    ];

    for (source, engine) in cases {
        let rendered = render_xychart_with_theme_and_engine(source, &theme, engine);
        let document =
            roxmltree::Document::parse(rendered.svg()).expect("valid owner-precedence SVG");
        let bar = xychart_plot_group(&document, "bar-plot-0")
            .children()
            .find(|node| node.has_tag_name("rect"))
            .expect("owner-precedence XY Chart bar");
        assert_eq!(bar.attribute("fill"), Some("#fedcba"));
        assert_eq!(bar.attribute("stroke"), Some("#fedcba"));

        let completion = rendered.into_completion();
        let evidence = merman_render::__private::family_evidence(completion.report());
        assert_eq!(evidence.required_count(), 1);
        assert_eq!(evidence.applied_count(), 0);
        assert_eq!(evidence.not_applicable_count(), 1);
        assert_eq!(evidence.theme_residual_count(), 0);
    }
}

#[test]
fn xychart_series_palette_is_not_applicable_without_plots() {
    let theme = xychart_series_palette_theme(&["#123456"]);
    let rendered = render_xychart_with_theme_and_engine(
        "xychart\n  x-axis [A]\n  y-axis 0 --> 10\n",
        &theme,
        Engine::new(),
    );

    let completion = rendered.into_completion();
    let evidence = merman_render::__private::family_evidence(completion.report());
    assert_eq!(evidence.required_count(), 1);
    assert_eq!(evidence.applied_count(), 0);
    assert_eq!(evidence.not_applicable_count(), 1);
    assert_eq!(evidence.theme_residual_count(), 0);
}

#[test]
fn xychart_typed_font_stack_reaches_layout_css_and_terminal_evidence() {
    let font_stack =
        FontStack::new(["XY Chart Typed", "monospace"]).expect("valid XY Chart font stack");
    let expected_font = font_stack.as_css();
    let theme = xychart_typography_theme(ThemeTextStyle::default().with_font_stack(font_stack));
    let identity = TextMeasurementProfileIdentity::new(
        MeasurementProfileId::new("test.xychart-typed-font").unwrap(),
        "1",
    )
    .unwrap();
    let environment = RenderEnvironment::deterministic()
        .with_theme_portability_requirement(ThemePortabilityRequirement::RequirePortable)
        .with_text_measurement_policy(TextMeasurementPolicy::uniform(TextMeasurementProfile::new(
            identity,
            Arc::new(XyChartTypedFontProbeMeasurer {
                expected_font_family: expected_font.clone(),
            }),
        )));
    let (layout, rendered) = try_render_xychart_with_theme(
        "xychart\n  title Typed chart\n  x-axis [Jan, Feb]\n  y-axis 0 --> 10\n  bar [4, 7]\n",
        &theme,
        Engine::new(),
        &environment,
        "xychart-typed-font",
    )
    .expect("render directly themed XY Chart");

    assert!(layout.drawables.iter().any(|drawable| {
        matches!(drawable, XyChartDrawableElem::Text { data, .. } if data.iter().any(|text| !text.text.is_empty()))
    }));
    let svg = rendered.svg();
    for rule in [
        format!("#xychart-typed-font{{font-family:{expected_font};"),
        format!("#xychart-typed-font svg{{font-family:{expected_font};"),
        format!("#xychart-typed-font :root{{--mermaid-font-family:{expected_font};}}"),
    ] {
        assert!(
            svg.contains(&rule),
            "missing resolved XY Chart font rule {rule:?}"
        );
    }

    let evidence = merman_render::__private::family_evidence(rendered.into_completion().report());
    assert_eq!(evidence.required_count(), 1);
    assert_eq!(evidence.accounted_count(), 1);
    assert_eq!(evidence.applied_count(), 1);
    assert_eq!(evidence.not_applicable_count(), 0);
    assert_eq!(evidence.theme_residual_count(), 0);
    assert_eq!(evidence.compatibility_residual_count(), 0);
    assert_eq!(evidence.mermaid_compatibility_residual_count(), 0);
}

#[test]
fn xychart_explicit_site_and_source_font_paths_outrank_typed_font_stack() {
    let theme = xychart_typography_theme(ThemeTextStyle::default().with_font_stack(
        FontStack::single("XY Chart Typed").expect("valid typed XY Chart font stack"),
    ));
    let cases = [
        (
            "xychart-site-font",
            Engine::new().with_site_config(MermaidConfig::from_value(serde_json::json!({
                "themeVariables": { "fontFamily": "XYChartSite,sans-serif" }
            }))),
            "xychart\n  x-axis [A]\n  y-axis 0 --> 10\n  bar [4]\n".to_string(),
            "XYChartSite,sans-serif",
        ),
        (
            "xychart-source-font",
            Engine::new().with_site_config(MermaidConfig::from_value(serde_json::json!({
                "secure": []
            }))),
            concat!(
                "%%{init: {\"themeVariables\": {\"fontFamily\": \"XYChartSource,monospace\"}}}%%\n",
                "xychart\n  x-axis [A]\n  y-axis 0 --> 10\n  bar [4]\n",
            )
            .to_string(),
            "XYChartSource,monospace",
        ),
    ];

    for (diagram_id, engine, source, expected_font) in cases {
        let (_, rendered) = try_render_xychart_with_theme(
            &source,
            &theme,
            engine,
            &RenderEnvironment::deterministic()
                .with_theme_portability_requirement(ThemePortabilityRequirement::RequirePortable),
            diagram_id,
        )
        .expect("render XY Chart with explicitly owned font family");

        assert!(
            rendered
                .svg()
                .contains(&format!("#{diagram_id}{{font-family:{expected_font};"))
        );
        assert!(!rendered.svg().contains("XY Chart Typed"));
        let evidence =
            merman_render::__private::family_evidence(rendered.into_completion().report());
        assert_eq!(evidence.required_count(), 1);
        assert_eq!(evidence.accounted_count(), 1);
        assert_eq!(evidence.applied_count(), 0);
        assert_eq!(evidence.not_applicable_count(), 1);
        assert_eq!(evidence.theme_residual_count(), 0);
    }
}

#[test]
fn xychart_mixed_base_typography_fails_closed() {
    let typography = ThemeTextStyle::default()
        .with_font_stack(
            FontStack::single("XY Chart Mixed").expect("valid mixed XY Chart font stack"),
        )
        .with_font_size_px(24.0)
        .expect("valid unsupported XY Chart font size");
    let theme = xychart_typography_theme(typography);
    let error = try_render_xychart_with_theme(
        "xychart\n  x-axis [A]\n  y-axis 0 --> 10\n  bar [4]\n",
        &theme,
        Engine::new(),
        &RenderEnvironment::deterministic()
            .with_theme_portability_requirement(ThemePortabilityRequirement::RequirePortable),
        "xychart-mixed-typography",
    )
    .err()
    .expect("RequirePortable must reject mixed XY Chart typography");

    assert_eq!(
        error.unverified_family_theme(),
        Some((DiagramFamilyId::XY_CHART, 1))
    );
    assert_eq!(error.incomplete_family_theme(), None);
}
