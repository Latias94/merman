mod common;

use common::legacy_init_theme_compat_engine;
use merman_core::ParseOptions;
use merman_render::LayoutOptions;
use merman_render::environment::RenderEnvironment;
use merman_render::family;
use merman_render::resources::{
    RenderResourcePolicy, ResourceLimitCause, ResourceLimitId, ResourceLimitPhase,
};
use merman_render::svg::{SvgDebugOptions, SvgRenderOptions};

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
