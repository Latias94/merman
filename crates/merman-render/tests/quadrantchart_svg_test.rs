mod common;

use common::legacy_init_theme_compat_engine;
use merman_core::{Engine, ParseOptions};
use merman_render::LayoutOptions;
use merman_render::diagram_theme::{
    DiagramTheme, DiagramThemeCompiler, DiagramThemeSpec, Specified, ThemeGeometryPatch,
    ThemePortabilityRequirement, ThemeRule, ThemeRuleSet, ThemeStylePatch, ThemeTarget,
};
use merman_render::environment::RenderEnvironment;
use merman_render::family;
use merman_render::resources::{
    RenderResourcePolicy, ResourceLimitCause, ResourceLimitId, ResourceLimitPhase,
};
use merman_render::svg::{SvgDebugOptions, SvgRenderOptions};

fn quadrantchart_series_radius_theme(radius: Specified<f32>) -> DiagramTheme {
    let styles = ThemeRuleSet::default().with_rule(ThemeRule::new(
        ThemeTarget::ChartSeries,
        ThemeStylePatch {
            geometry: ThemeGeometryPatch { radius },
            ..ThemeStylePatch::default()
        },
    ));
    DiagramThemeCompiler::new()
        .compile(DiagramThemeSpec::new().with_styles(styles))
        .expect("compile Quadrant Chart series radius theme")
}

fn render_quadrantchart_svg_with_theme(source: &str, theme: &DiagramTheme) -> String {
    let parsed = merman_render::__private::install_parse_compatibility(theme, Engine::new())
        .parse_diagram_for_render_model_sync(source, ParseOptions::strict())
        .expect("parse themed Quadrant Chart")
        .expect("detect themed Quadrant Chart");
    let session = RenderEnvironment::deterministic()
        .with_theme_portability_requirement(ThemePortabilityRequirement::RequirePortable)
        .begin_session_with_theme(theme)
        .expect("begin strict portable Quadrant Chart session");
    let artifact = family::prepare(parsed, &LayoutOptions::default(), session)
        .expect("prepare themed Quadrant Chart artifact");

    artifact
        .render_svg(&SvgRenderOptions::default(), &SvgDebugOptions::default())
        .expect("render themed Quadrant Chart SVG")
        .svg()
        .to_owned()
}

fn first_quadrantchart_point_circle(svg: &str) -> &str {
    let points_start = svg
        .find(r#"<g class="data-points">"#)
        .expect("Quadrant Chart data-points group");
    let circle_start = svg[points_start..]
        .find("<circle ")
        .map(|offset| points_start + offset)
        .expect("Quadrant Chart point circle");
    let circle_end = svg[circle_start..]
        .find("/>")
        .map(|offset| circle_start + offset + 2)
        .expect("Quadrant Chart point circle end");
    &svg[circle_start..circle_end]
}

fn render_quadrantchart_svg_from_text(text: &str) -> String {
    let engine = legacy_init_theme_compat_engine();
    let parsed = engine
        .parse_diagram_for_render_model_sync(text, ParseOptions::default())
        .expect("parse ok")
        .expect("diagram detected");
    let session = RenderEnvironment::deterministic().begin_session().unwrap();
    let artifact = family::prepare(parsed, &LayoutOptions::default(), session).expect("layout ok");

    artifact
        .render_svg(&SvgRenderOptions::default(), &SvgDebugOptions::default())
        .expect("render svg")
        .svg()
        .to_owned()
}

fn try_render_quadrantchart_svg_with_resource_policy(
    text: &str,
    resource_policy: RenderResourcePolicy,
) -> merman_render::Result<String> {
    let parsed = legacy_init_theme_compat_engine()
        .parse_diagram_for_render_model_sync(text, ParseOptions::default())
        .expect("parse QuadrantChart resource-bound fixture")
        .expect("detect QuadrantChart resource-bound fixture");
    let session = RenderEnvironment::deterministic()
        .with_resource_policy(resource_policy)
        .begin_session()
        .expect("begin QuadrantChart resource-bound session");
    let artifact = family::prepare(parsed, &LayoutOptions::default(), session)?;
    let rendered =
        artifact.render_svg(&SvgRenderOptions::default(), &SvgDebugOptions::default())?;
    Ok(rendered.svg().to_owned())
}

#[test]
fn quadrantchart_family_svg_accepts_exact_max_svg_bytes_and_rejects_one_byte_less() {
    let source = r#"quadrantChart
  title Bounded priorities
  x-axis Low --> High
  y-axis Low --> High
  quadrant-1 Plan
  quadrant-2 Build
  Feature: [0.7, 0.8]
"#;
    let baseline = try_render_quadrantchart_svg_with_resource_policy(
        source,
        RenderResourcePolicy::unbounded_for_trusted_input(),
    )
    .expect("render the unbounded QuadrantChart baseline");
    let exact_bytes = baseline.len();
    assert!(
        exact_bytes > 1,
        "QuadrantChart fixture must emit a non-empty SVG"
    );

    let exact_policy = RenderResourcePolicy::unbounded_for_trusted_input()
        .with_limit(ResourceLimitId::MaxSvgBytes, exact_bytes)
        .expect("valid exact QuadrantChart SVG byte ceiling");
    let exact = try_render_quadrantchart_svg_with_resource_policy(source, exact_policy)
        .expect("the exact QuadrantChart family SVG byte ceiling must succeed");
    assert_eq!(exact.as_bytes(), baseline.as_bytes());

    let below_exact = exact_bytes - 1;
    let below_policy = RenderResourcePolicy::unbounded_for_trusted_input()
        .with_limit(ResourceLimitId::MaxSvgBytes, below_exact)
        .expect("valid below-exact QuadrantChart SVG byte ceiling");
    let error = try_render_quadrantchart_svg_with_resource_policy(source, below_policy)
        .expect_err("one byte below the QuadrantChart family SVG size must fail");
    let merman_render::Error::ResourceLimitExceeded(limit) = error else {
        panic!("expected QuadrantChart MaxSvgBytes rejection, got {error}");
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

fn assert_contains(haystack: &str, needle: &str) {
    assert!(
        haystack.contains(needle),
        "expected SVG to contain {needle:?}"
    );
}

#[test]
fn quadrantchart_frontmatter_title_and_accessibility_metadata_match_mermaid() {
    let svg = render_quadrantchart_svg_from_text(
        r#"---
title: Frontmatter quadrant
---
quadrantChart
  accTitle: Accessible quadrant
  accDescr: Quadrant description
  x-axis Low --> High
  y-axis Low --> High
  Feature: [0.5, 0.5]
"#,
    );

    assert_contains(&svg, ">Frontmatter quadrant</text>");
    assert_contains(&svg, r#"aria-labelledby="chart-title-quadrantchart""#);
    assert_contains(&svg, r#"aria-describedby="chart-desc-quadrantchart""#);
    assert_contains(
        &svg,
        r#"<title id="chart-title-quadrantchart">Accessible quadrant</title>"#,
    );
    assert_contains(
        &svg,
        r#"<desc id="chart-desc-quadrantchart">Quadrant description</desc>"#,
    );
}

#[test]
fn quadrantchart_body_title_overrides_frontmatter_title() {
    let svg = render_quadrantchart_svg_from_text(
        r#"---
title: Frontmatter quadrant
---
quadrantChart
  title Body quadrant
  Feature: [0.5, 0.5]
"#,
    );

    assert_contains(&svg, ">Body quadrant</text>");
    assert!(!svg.contains(">Frontmatter quadrant</text>"));
}

#[test]
fn quadrantchart_default_point_fill_matches_mermaid_11_16_theme_output() {
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

    assert_contains(
        &svg,
        r#"<circle cx="31" cy="469" r="5" fill="hsl(240, 100%, NaN%)" stroke="hsl(240, 100%, NaN%)" stroke-width="0px"/>"#,
    );
}

#[test]
fn quadrantchart_static_series_radius_reaches_svg_without_overriding_existing_owners() {
    let theme = quadrantchart_series_radius_theme(Specified::Value(13.0));
    let cases = [
        ("typed", "quadrantChart\nTyped: [0.5, 0.5]\n", "13"),
        (
            "inline",
            "quadrantChart\nInline: [0.5, 0.5] radius: 7\n",
            "7",
        ),
        (
            "classDef",
            concat!(
                "quadrantChart\n",
                "classDef large radius: 8\n",
                "Classed:::large: [0.5, 0.5]\n",
            ),
            "8",
        ),
        (
            "config",
            concat!(
                "---\n",
                "config:\n",
                "  quadrantChart:\n",
                "    pointRadius: 9\n",
                "---\n",
                "quadrantChart\n",
                "Configured: [0.5, 0.5]\n",
            ),
            "9",
        ),
    ];

    for (owner, source, expected_radius) in cases {
        let svg = render_quadrantchart_svg_with_theme(source, &theme);
        let circle = first_quadrantchart_point_circle(&svg);
        assert!(
            circle.contains(&format!(r#" r="{expected_radius}""#)),
            "{owner} radius owner did not win in the terminal point circle: {circle}"
        );
    }

    let cleared = render_quadrantchart_svg_with_theme(
        "quadrantChart\nCleared: [0.5, 0.5]\n",
        &quadrantchart_series_radius_theme(Specified::Clear),
    );
    let circle = first_quadrantchart_point_circle(&cleared);
    assert!(
        circle.contains(r#" r="5""#),
        "Clear did not restore the Mermaid point-radius baseline: {circle}"
    );
    let cleared_document =
        roxmltree::Document::parse(&cleared).expect("valid cleared Quadrant SVG XML");
    let cleared_points = cleared_document
        .descendants()
        .find(|node| node.has_tag_name("g") && node.attribute("class") == Some("data-points"))
        .expect("cleared Quadrant data-points group");
    let cleared_point = cleared_points
        .descendants()
        .find(|node| node.has_tag_name("circle"))
        .expect("cleared Quadrant point");
    let cleared_center = (
        cleared_point
            .attribute("cx")
            .expect("cleared point cx")
            .parse::<f64>()
            .expect("numeric cleared point cx"),
        cleared_point
            .attribute("cy")
            .expect("cleared point cy")
            .parse::<f64>()
            .expect("numeric cleared point cy"),
    );

    let large_radius = render_quadrantchart_svg_with_theme(
        "quadrantChart\nLarge: [0.5, 0.5]\n",
        &quadrantchart_series_radius_theme(Specified::Value(1_000.0)),
    );
    let large_document =
        roxmltree::Document::parse(&large_radius).expect("valid large-radius Quadrant SVG XML");
    let root = large_document.root_element();
    let view_box = root
        .attribute("viewBox")
        .expect("Quadrant root viewBox")
        .split_ascii_whitespace()
        .map(|value| value.parse::<f64>().expect("numeric Quadrant viewBox"))
        .collect::<Vec<_>>();
    let large_points = large_document
        .descendants()
        .find(|node| node.has_tag_name("g") && node.attribute("class") == Some("data-points"))
        .expect("large-radius Quadrant data-points group");
    let point = large_points
        .descendants()
        .find(|node| node.has_tag_name("circle"))
        .expect("large-radius Quadrant point");
    let cx = point
        .attribute("cx")
        .expect("point cx")
        .parse::<f64>()
        .expect("numeric point cx");
    let cy = point
        .attribute("cy")
        .expect("point cy")
        .parse::<f64>()
        .expect("numeric point cy");
    let radius = point
        .attribute("r")
        .expect("point radius")
        .parse::<f64>()
        .expect("numeric point radius");

    assert_eq!(view_box.len(), 4);
    assert_eq!((cx, cy), cleared_center, "radius changed point layout");
    assert!(view_box[0] <= cx - radius, "{view_box:?}");
    assert!(view_box[1] <= cy - radius, "{view_box:?}");
    assert!(view_box[0] + view_box[2] >= cx + radius, "{view_box:?}");
    assert!(view_box[1] + view_box[3] >= cy + radius, "{view_box:?}");
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

#[test]
fn quadrantchart_redux_dark_primary_override_derives_quadrant_fill() {
    let svg = render_quadrantchart_svg_from_text(
        r##"%%{init: {"theme": "redux-dark", "themeVariables": {"primaryColor": "#123456"}}}%%
quadrantChart
  title Priority
  x-axis Low --> High
  y-axis Low --> High
  quadrant-1 Plan
  quadrant-2 Build
  Feature: [0.8, 0.8]
"##,
    );

    assert_contains(&svg, r##"fill="#123456""##);
}
