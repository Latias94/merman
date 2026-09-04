mod common;

use common::legacy_init_theme_compat_engine;
use merman_core::ParseOptions;
use merman_render::environment::{RenderEnvironment, TextMeasurementPolicy};
use merman_render::family;
use merman_render::model::PieDiagramLayout;
use merman_render::svg::{SvgDebugOptions, SvgRenderOptions};
use merman_render::{
    Error, LayoutOptions, RenderResourcePolicy, ResourceLimitCause, ResourceLimitId,
    ResourceLimitPhase,
};
use roxmltree::{Document, Node};

fn layout_pie_from_text(text: &str) -> PieDiagramLayout {
    let engine = legacy_init_theme_compat_engine();
    let parsed = engine
        .parse_diagram_for_render_model_sync(text, ParseOptions::default())
        .expect("parse ok")
        .expect("diagram detected");
    let session = RenderEnvironment::deterministic()
        .with_text_measurement_policy(TextMeasurementPolicy::deterministic())
        .begin_session()
        .unwrap();
    let artifact = family::prepare(parsed, &LayoutOptions::default(), session).expect("layout ok");
    let projection = artifact.layout_json().expect("serialize Pie layout");
    serde_json::from_value(projection["layout"]["PieDiagram"].clone())
        .expect("Pie layout projection")
}

fn render_pie_from_text(text: &str) -> String {
    render_pie_from_text_with_options(text, &SvgRenderOptions::default())
}

fn render_pie_from_text_with_options(text: &str, options: &SvgRenderOptions) -> String {
    let engine = legacy_init_theme_compat_engine();
    let parsed = engine
        .parse_diagram_for_render_model_sync(text, ParseOptions::default())
        .expect("parse ok")
        .expect("diagram detected");
    let session = RenderEnvironment::deterministic()
        .with_text_measurement_policy(TextMeasurementPolicy::deterministic())
        .begin_session()
        .unwrap();
    let artifact = family::prepare(parsed, &LayoutOptions::default(), session).expect("layout ok");

    artifact
        .render_svg(options, &SvgDebugOptions::default())
        .expect("svg render ok")
        .svg()
        .to_owned()
}

fn render_pie_error_with_svg_limit(text: &str, diagram_id: &str, maximum: usize) -> Error {
    let engine = legacy_init_theme_compat_engine();
    let parsed = engine
        .parse_diagram_for_render_model_sync(text, ParseOptions::default())
        .expect("parse ok")
        .expect("diagram detected");
    let policy = RenderResourcePolicy::unbounded_for_trusted_input()
        .with_limit(ResourceLimitId::MaxSvgBytes, maximum)
        .unwrap();
    let session = RenderEnvironment::deterministic()
        .with_resource_policy(policy)
        .with_text_measurement_policy(TextMeasurementPolicy::deterministic())
        .begin_session()
        .unwrap();
    let artifact = family::prepare(parsed, &LayoutOptions::default(), session).expect("layout ok");

    match artifact.render_svg(
        &SvgRenderOptions {
            diagram_id: Some(diagram_id.to_string()),
            ..SvgRenderOptions::default()
        },
        &SvgDebugOptions::default(),
    ) {
        Ok(_) => panic!("bounded Pie SVG must exceed the test budget"),
        Err(error) => error,
    }
}

fn root_viewbox_width(svg: &str) -> f64 {
    let start = svg.find(r#"viewBox=""#).expect("viewBox start") + r#"viewBox=""#.len();
    let end = svg[start..].find('"').expect("viewBox end") + start;
    svg[start..end]
        .split_whitespace()
        .nth(2)
        .expect("viewBox width")
        .parse::<f64>()
        .expect("viewBox width parses")
}

fn pie_resource_translate(svg: &str, resource_id: &str) -> (f64, f64) {
    let document = Document::parse(svg).expect("valid Pie SVG");
    let transform = document
        .descendants()
        .find(|node| node.attribute("data-merman-resource") == Some(resource_id))
        .and_then(|node| node.attribute("transform"))
        .expect("translated Pie resource");
    let values = transform
        .strip_prefix("translate(")
        .and_then(|value| value.split_once(')'))
        .map(|(value, _)| value)
        .expect("translate transform")
        .split([',', ' '])
        .filter(|value| !value.is_empty())
        .map(|value| value.parse::<f64>().expect("numeric translate component"))
        .collect::<Vec<_>>();
    assert_eq!(values.len(), 2, "two-dimensional translate transform");
    (values[0], values[1])
}

fn semantic_group<'a>(document: &'a Document<'a>, semantic_id: &str) -> Node<'a, 'a> {
    document
        .descendants()
        .find(|node| {
            node.has_tag_name("g") && node.attribute("data-merman-semantic-id") == Some(semantic_id)
        })
        .expect("semantic group")
}

fn class_contains(node: Node<'_, '_>, class: &str) -> bool {
    node.attribute("class").is_some_and(|classes| {
        classes
            .split_whitespace()
            .any(|candidate| candidate == class)
    })
}

fn text_with_class<'a>(document: &'a Document<'a>, class: &str) -> Vec<&'a str> {
    document
        .descendants()
        .filter(|node| node.has_tag_name("text") && class_contains(*node, class))
        .filter_map(|node| node.text())
        .collect()
}

#[test]
fn pie_slices_follow_input_order_like_mermaid_11_16() {
    let layout = layout_pie_from_text(
        r#"pie
  "A" : 10
  "B" : 100
  "C" : 50
"#,
    );

    let labels: Vec<&str> = layout
        .slices
        .iter()
        .map(|slice| slice.label.as_str())
        .collect();

    assert_eq!(labels, vec!["A", "B", "C"]);
}

#[test]
fn pie_large_diagram_id_stylesheet_is_preflighted_at_exact_n() {
    let source = "pie\n  \"A\" : 1\n";
    // Host-special characters exercise the public normalization boundary. Counting and
    // materialization share the same writer over the resulting CSS-safe ID rather than assuming
    // a byte slope.
    let diagram_id = "diagram<&:.id".repeat(128);
    let full_svg = render_pie_from_text_with_options(
        source,
        &SvgRenderOptions {
            diagram_id: Some(diagram_id.clone()),
            ..SvgRenderOptions::default()
        },
    );
    let style_start = full_svg.find("<style>").expect("Pie style open") + "<style>".len();
    let style_end = full_svg[style_start..]
        .find("</style>")
        .map(|offset| style_start + offset)
        .expect("Pie style close");
    let projected_css_bytes = style_end - style_start;
    let n_minus_one_maximum = projected_css_bytes
        .checked_sub(1)
        .expect("Pie CSS projection must not be empty");

    let Error::ResourceLimitExceeded(n_minus_one) =
        render_pie_error_with_svg_limit(source, &diagram_id, n_minus_one_maximum)
    else {
        panic!("expected N-1 Pie CSS byte projection error");
    };
    assert_eq!(n_minus_one.cause, ResourceLimitCause::Ceiling);
    assert_eq!(n_minus_one.phase, ResourceLimitPhase::SvgOutput);
    assert_eq!(n_minus_one.limit, ResourceLimitId::MaxSvgBytes.as_str());
    assert_eq!(n_minus_one.actual, projected_css_bytes);
    assert_eq!(n_minus_one.max, n_minus_one_maximum);

    let Error::ResourceLimitExceeded(exact) =
        render_pie_error_with_svg_limit(source, &diagram_id, projected_css_bytes)
    else {
        panic!("expected final whole-document Pie SVG byte error");
    };
    assert_eq!(exact.max, projected_css_bytes);
    assert!(
        exact.actual > projected_css_bytes,
        "the exact CSS projection must pass early admission and reach final SVG admission"
    );
}

#[test]
fn pie_chart_content_is_grouped_before_title_and_legend_like_mermaid_11_16() {
    let svg = render_pie_from_text(
        r#"pie
  "A" : 3
  "B" : 2
"#,
    );

    let document = Document::parse(&svg).expect("canonical Pie SVG is XML");
    let root = semantic_group(&document, "pie.document");
    let plot = semantic_group(&document, "pie.plot");
    assert!(
        plot.descendants()
            .any(|node| { node.has_tag_name("circle") && class_contains(node, "pieOuterCircle") })
    );

    let child_semantics = root
        .children()
        .filter_map(|node| node.attribute("data-merman-semantic-id"))
        .collect::<Vec<_>>();
    let plot_index = child_semantics
        .iter()
        .position(|id| *id == "pie.plot")
        .expect("Pie plot semantic group");
    let first_legend_index = child_semantics
        .iter()
        .position(|id| id.starts_with("pie.legend."))
        .expect("Pie legend semantic group");
    assert!(
        plot_index < first_legend_index,
        "plot geometry must precede legend groups: {child_semantics:?}"
    );
}

#[test]
fn pie_frontmatter_title_renders_unless_the_body_overrides_it() {
    let frontmatter_svg = render_pie_from_text(
        r#"---
title: Frontmatter pie
---
pie
  "A" : 1
"#,
    );
    let frontmatter_document = Document::parse(&frontmatter_svg).expect("frontmatter Pie SVG");
    assert_eq!(
        text_with_class(&frontmatter_document, "pieTitleText"),
        vec!["Frontmatter pie"]
    );

    let body_svg = render_pie_from_text(
        r#"---
title: Frontmatter pie
---
pie title Body pie
  "A" : 1
"#,
    );
    let body_document = Document::parse(&body_svg).expect("body-title Pie SVG");
    assert_eq!(
        text_with_class(&body_document, "pieTitleText"),
        vec!["Body pie"]
    );
}

#[test]
fn pie_frontmatter_title_preserves_common_db_boundary_whitespace() {
    for title in ["  Frontmatter pie  ", "\u{a0}Frontmatter pie\u{a0}"] {
        let source = format!("---\ntitle: \"{title}\"\n---\npie\n  \"A\" : 1\n");
        let svg = render_pie_from_text(&source);

        let document = Document::parse(&svg).expect("whitespace-title Pie SVG");
        assert_eq!(text_with_class(&document, "pieTitleText"), vec![title]);
    }
}

#[test]
fn pie_hidden_slices_still_reserve_color_domain_slots() {
    let layout = layout_pie_from_text(
        r#"pie
  "A" : 10
  "B" : 100
  "C" : 0.1
  "D" : 50
"#,
    );

    let slices: Vec<(&str, &str)> = layout
        .slices
        .iter()
        .map(|slice| (slice.label.as_str(), slice.fill.as_str()))
        .collect();

    assert_eq!(
        slices,
        vec![
            ("A", "#ECECFF"),
            ("B", "#ffffde"),
            ("D", "hsl(240, 100%, 86.2745098039%)")
        ]
    );
}

#[test]
fn pie_redux_dark_primary_override_derives_first_slice_color() {
    let layout = layout_pie_from_text(
        r##"%%{init: {"theme": "redux-dark", "themeVariables": {"primaryColor": "#123456"}}}%%
pie
  "A" : 10
  "B" : 20
"##,
    );

    let first = layout.slices.first().expect("first slice");
    assert_eq!(first.fill, "#123456");
}

#[test]
fn pie_text_position_config_moves_slice_labels() {
    let layout = layout_pie_from_text(
        r#"%%{init: {"pie": {"textPosition": 0.5}}}%%
pie
  "A" : 1
  "B" : 1
"#,
    );

    let first = layout
        .slices
        .iter()
        .find(|slice| slice.label == "A")
        .expect("slice A exists");

    assert!((first.text_x - 92.5).abs() < 1e-9);
    assert!(first.text_y.abs() < 1e-9);
}

#[test]
fn pie_donut_hole_config_renders_annular_slice_paths() {
    let svg = render_pie_from_text(
        r#"%%{init: {"pie": {"donutHole": 0.4}}}%%
pie
  "A" : 1
  "B" : 1
"#,
    );

    let document = Document::parse(&svg).expect("donut Pie SVG");
    let slice = document
        .descendants()
        .find(|node| node.attribute("data-merman-resource") == Some("pie.slice.0.shape"))
        .expect("donut slice path");
    let path = slice.attribute("d").expect("donut path data");
    assert!(
        path.contains("A 74 74"),
        "expected inner-radius arc: {path}"
    );
    assert!(
        !path.contains("L 0 0 Z"),
        "donut slices must not close through center: {path}"
    );
}

#[test]
fn pie_invalid_donut_hole_config_falls_back_to_solid_slices() {
    let svg = render_pie_from_text(
        r#"%%{init: {"pie": {"donutHole": 1.2}}}%%
pie
  "A" : 1
  "B" : 1
"#,
    );

    let document = Document::parse(&svg).expect("solid Pie SVG");
    let slice = document
        .descendants()
        .find(|node| node.attribute("data-merman-resource") == Some("pie.slice.0.shape"))
        .expect("solid slice path");
    let path = slice.attribute("d").expect("solid path data");
    assert!(
        path.contains("L 0 0 Z"),
        "invalid donutHole should fall back to solid slices: {path}"
    );
}

#[test]
fn pie_legend_position_config_controls_layout_regions() {
    let diagram = |position: &str| {
        layout_pie_from_text(&format!(
            r#"%%{{init: {{"pie": {{"legendPosition": "{position}"}}}}}}%%
pie
  "A" : 1
  "B" : 1
"#
        ))
    };

    let right = diagram("right");
    let right_bounds = right.bounds.as_ref().expect("right bounds");
    assert!(right_bounds.max_x > 490.0);
    assert_eq!(right_bounds.max_y, 450.0);
    assert_eq!(right.legend_x, 216.0);
    assert_eq!(right.legend_items[0].y, -22.0);

    let top = diagram("top");
    let top_bounds = top.bounds.as_ref().expect("top bounds");
    assert_eq!(top_bounds.max_x, 490.0);
    assert_eq!(top_bounds.max_y, 494.0);
    assert!(top.legend_x < 0.0);
    assert_eq!(top.legend_items[0].y, -185.0);

    let bottom = diagram("bottom");
    let bottom_bounds = bottom.bounds.as_ref().expect("bottom bounds");
    assert_eq!(bottom_bounds.max_x, 490.0);
    assert_eq!(bottom_bounds.max_y, 494.0);
    assert!(bottom.legend_x < 0.0);
    assert_eq!(bottom.legend_items[0].y, 207.0);

    let left = diagram("left");
    let left_bounds = left.bounds.as_ref().expect("left bounds");
    assert!(left_bounds.max_x > 490.0);
    assert_eq!(left_bounds.max_y, 450.0);
    assert_eq!(left.legend_x, -207.0);
    assert_eq!(left.legend_items[0].y, -22.0);

    let center = diagram("center");
    let center_bounds = center.bounds.as_ref().expect("center bounds");
    assert_eq!(center_bounds.max_x, 490.0);
    assert_eq!(center_bounds.max_y, 450.0);
    assert!(center.legend_x < 0.0);
    assert_eq!(center.legend_items[0].y, -22.0);
}

#[test]
fn pie_legend_position_top_and_left_move_the_pie_group() {
    let top_svg = render_pie_from_text(
        r#"%%{init: {"pie": {"legendPosition": "top"}}}%%
pie
  "A" : 1
  "B" : 1
"#,
    );
    assert!(top_svg.contains(r#"viewBox="0 0 490 494""#));
    let top_offset = pie_resource_translate(&top_svg, "pie.slice.0.shape");
    assert!(
        (top_offset.0 - 225.0).abs() <= f64::EPSILON
            && (top_offset.1 - 291.0).abs() <= f64::EPSILON,
        "top legend should move the canonical pie geometry below the legend: {top_svg}"
    );

    let left_svg = render_pie_from_text(
        r#"%%{init: {"pie": {"legendPosition": "left"}}}%%
pie
  "A" : 1
  "B" : 1
"#,
    );
    let left_offset = pie_resource_translate(&left_svg, "pie.slice.0.shape");
    let expected_left_offset = root_viewbox_width(&left_svg) - 490.0;
    assert!(
        (left_offset.0 - (225.0 + expected_left_offset)).abs() <= 1.0e-9
            && (left_offset.1 - 225.0).abs() <= f64::EPSILON,
        "left legend should move the canonical pie geometry right by legend width: {left_svg}"
    );
    let left_document = Document::parse(&left_svg).expect("left-legend Pie SVG");
    assert!(class_contains(
        semantic_group(&left_document, "pie.legend.0"),
        "legend"
    ));
}

#[test]
fn empty_pie_root_viewport_is_finite_for_headless_rendering() {
    let svg = render_pie_from_text("pie");

    assert!(
        svg.contains(r#"viewBox="0 0 450 450""#),
        "empty pie should keep the finite Mermaid empty-root viewport: {svg}"
    );
    assert!(
        !svg.contains("Infinity") && !svg.contains("NaN"),
        "empty pie should not leak non-finite SVG values: {svg}"
    );
}

#[test]
fn empty_pie_with_title_keeps_title_widened_root_viewport() {
    let svg = render_pie_from_text("pie title sample title");
    let viewbox_width = root_viewbox_width(&svg);

    assert!(
        viewbox_width > 250.0,
        "empty pie title should widen the root viewport instead of falling back to 225px: {svg}"
    );
    assert!(
        !svg.contains("Infinity") && !svg.contains("NaN"),
        "titled empty pie should not leak non-finite SVG values: {svg}"
    );
}

#[test]
fn pie_highlight_slice_config_marks_matching_slice_and_emits_css() {
    let svg = render_pie_from_text(
        r#"%%{init: {"pie": {"highlightSlice": "A"}}}%%
pie
  "A" : 1
  "B" : 1
"#,
    );

    let document = Document::parse(&svg).expect("highlighted Pie SVG");
    let highlighted = document
        .descendants()
        .find(|node| class_contains(*node, "highlighted"))
        .expect("highlighted slice");
    assert_eq!(
        highlighted.attribute("class"),
        Some("pieCircle highlighted")
    );
    assert_eq!(
        highlighted.attribute("transform"),
        Some("matrix(1.05 0 0 1.05 225 225)")
    );
    assert!(
        document.descendants().any(|node| {
            node.has_tag_name("path") && node.attribute("class") == Some("pieCircle")
        }),
        "non-matching slices should keep the ordinary pieCircle class"
    );
    assert!(
        !svg.contains(".pieCircle.highlighted{scale:"),
        "canonical SVG must not apply the static highlight transform a second time"
    );
}

#[test]
fn pie_hover_highlight_slice_config_marks_all_slices_and_emits_css() {
    let svg = render_pie_from_text(
        r#"%%{init: {"pie": {"highlightSlice": "hover"}}}%%
pie
  "A" : 1
  "B" : 1
"#,
    );

    assert!(
        svg.contains(
            r#".pieCircle.highlightedOnHover:hover{transition-duration:250ms;scale:1.05;opacity:1;}"#
        ),
        "Mermaid 11.16 pie CSS should include hover-highlight styling: {svg}"
    );
    assert!(
        svg.matches(r#"class="pieCircle highlightedOnHover""#)
            .count()
            >= 2,
        "Mermaid 11.16 should mark every slice as hover-highlightable: {svg}"
    );
}
