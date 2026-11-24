mod common;

use common::legacy_init_theme_compat_engine;
use merman_core::{MermaidEngine, ParseOptions};
use merman_render::LayoutOptions;

use merman_render::environment::RenderEnvironment;
use merman_render::family;
use merman_render::resources::{
    RenderResourcePolicy, ResourceLimitCause, ResourceLimitId, ResourceLimitPhase,
};
use merman_render::svg::{SvgDebugOptions, SvgRenderOptions};

#[test]
fn quadrantchart_source_theme_preserves_point_foreground() {
    let config = serde_json::json!({
        "theme": "base",
        "themeVariables": { "quadrant1Fill": "#e8f5e9" }
    });
    let body = "quadrantChart\n    title Value vs. cost\n    x-axis Low cost --> High cost\n    y-axis Low value --> High value\n    quadrant-1 Plan\n    quadrant-2 Do now\n    quadrant-3 Maybe\n    quadrant-4 Drop\n    Alpha: [0.75, 0.75]\n";
    for source in [
        format!("---\nconfig: {config}\n---\n{body}"),
        format!("%%{{init: {config}}}%%\n{body}"),
    ] {
        let svg = render_quadrantchart_svg_from_text(&source);
        let document = roxmltree::Document::parse(&svg).unwrap();
        let label = document
            .descendants()
            .find(|node| node.has_tag_name("text") && node.text() == Some("Alpha"))
            .expect("source point label");
        assert_eq!(label.attribute("fill"), Some("#333"), "{source}");
        assert!(document.descendants().any(|node| {
            node.has_tag_name("rect") && node.attribute("fill") == Some("#e8f5e9")
        }));
    }
}

fn render_quadrantchart_svg_from_text(text: &str) -> String {
    let engine = legacy_init_theme_compat_engine();
    let parsed = engine
        .parse_diagram_for_render_model_sync(text, ParseOptions::default())
        .expect("parse ok")
        .expect("diagram detected")
        .into_presentation();
    let session = RenderEnvironment::deterministic().begin_session().unwrap();
    let artifact = family::prepare(&parsed, &LayoutOptions::default(), session).expect("layout ok");

    artifact
        .render_svg(&SvgRenderOptions::default(), &SvgDebugOptions::default())
        .expect("render svg")
        .svg()
        .to_owned()
}

#[test]
fn quadrantchart_wildcard_coordinates_use_number_coercion_and_d3_scaling() {
    let svg = render_quadrantchart_svg_from_text("quadrantChart\nP: [0x1,001]\n");
    let document = roxmltree::Document::parse(&svg).unwrap();
    let circle = document
        .descendants()
        .find(|node| node.has_tag_name("circle"))
        .unwrap();
    assert_eq!(circle.attribute("cx"), Some("495"));
    assert_eq!(circle.attribute("cy"), Some("5"));
    let text = document
        .descendants()
        .find(|node| node.has_tag_name("text") && node.text() == Some("P"))
        .unwrap();
    assert_eq!(
        text.attribute("transform"),
        Some("translate(495, 10) rotate(0)")
    );
}

#[test]
fn quadrantchart_unknown_coordinates_remove_circle_attributes_and_preserve_label_arithmetic() {
    for (coordinates, expected_x, expected_y, transform) in [
        (
            "0,5,0.5",
            None,
            Some("250"),
            "translate(undefined, 255) rotate(0)",
        ),
        (
            "0.5,0;5",
            Some("250"),
            None,
            "translate(250, NaN) rotate(0)",
        ),
        (
            "0\"5,0]5",
            None,
            None,
            "translate(undefined, NaN) rotate(0)",
        ),
    ] {
        let svg =
            render_quadrantchart_svg_from_text(&format!("quadrantChart\nP: [{coordinates}]\n"));
        let document = roxmltree::Document::parse(&svg).unwrap();
        let circle = document
            .descendants()
            .find(|node| node.has_tag_name("circle"))
            .unwrap();
        assert_eq!(circle.attribute("cx"), expected_x);
        assert_eq!(circle.attribute("cy"), expected_y);
        let text = document
            .descendants()
            .find(|node| node.has_tag_name("text") && node.text() == Some("P"))
            .unwrap();
        assert_eq!(text.attribute("transform"), Some(transform));
    }
}

#[test]
fn quadrantchart_numeric_overflow_is_distinct_from_an_unknown_scale_input() {
    let source = format!("quadrantChart\nP: [0{},0.5]\n", "9".repeat(350));
    let svg = render_quadrantchart_svg_from_text(&source);
    let document = roxmltree::Document::parse(&svg).unwrap();
    let circle = document
        .descendants()
        .find(|node| node.has_tag_name("circle"))
        .unwrap();
    assert_eq!(circle.attribute("cx"), Some("NaN"));
    let text = document
        .descendants()
        .find(|node| node.has_tag_name("text") && node.text() == Some("P"))
        .unwrap();
    assert_eq!(
        text.attribute("transform"),
        Some("translate(NaN, 255) rotate(0)")
    );
}

#[test]
fn quadrantchart_radius_preserves_js_rounding_overflow_and_zero_override() {
    for (style, expected) in [
        ("radius:9007199254740993".to_string(), "9007199254740992"),
        (
            "radius:9223372036854775808".to_string(),
            "9223372036854776000",
        ),
        (format!("radius:{}", "9".repeat(309)), "Infinity"),
        ("radius:0".to_string(), "0"),
    ] {
        let source = format!(
            "quadrantChart\nclassDef C radius:{}\nP:::C: [0.5,0.5] {style}\n",
            "9".repeat(309)
        );
        let svg = render_quadrantchart_svg_from_text(&source);
        let document = roxmltree::Document::parse(&svg).unwrap();
        let circle = document
            .descendants()
            .find(|node| node.has_tag_name("circle"))
            .unwrap();
        assert_eq!(circle.attribute("r"), Some(expected));
    }
}

fn try_render_quadrantchart_svg_with_resource_policy(
    text: &str,
    resource_policy: RenderResourcePolicy,
) -> merman_render::Result<String> {
    let parsed = legacy_init_theme_compat_engine()
        .parse_diagram_for_render_model_sync(text, ParseOptions::default())
        .expect("parse QuadrantChart resource-bound fixture")
        .expect("detect QuadrantChart resource-bound fixture")
        .into_presentation();
    let session = RenderEnvironment::deterministic()
        .with_resource_policy(resource_policy)
        .begin_session()
        .expect("begin QuadrantChart resource-bound session");
    let artifact = family::prepare(&parsed, &LayoutOptions::default(), session)?;
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
fn quadrantchart_title_semicolons_reach_the_title_dom_without_creating_quadrants() {
    let svg = render_quadrantchart_svg_from_text(
        "quadrantChart\ntitle Plan; quadrant-1 Hidden\nquadrant-2 Visible\n",
    );
    let document = roxmltree::Document::parse(&svg).unwrap();
    let title = document
        .descendants()
        .find(|node| node.has_tag_name("g") && node.attribute("class") == Some("title"))
        .unwrap();
    assert_eq!(
        title
            .children()
            .find(|node| node.has_tag_name("text"))
            .unwrap()
            .text(),
        Some("Plan; quadrant-1 Hidden")
    );
    let quadrant_texts = document
        .descendants()
        .filter(|node| node.has_tag_name("g") && node.attribute("class") == Some("quadrant"))
        .map(|node| {
            node.children()
                .find(|child| child.has_tag_name("text"))
                .unwrap()
                .text()
                .unwrap_or("")
        })
        .collect::<Vec<_>>();
    assert_eq!(quadrant_texts, ["", "Visible", "", ""]);
}

#[test]
fn quadrantchart_multiline_literal_tokens_reach_svg_text_nodes() {
    for ending in ["\n", "\r\n"] {
        for (open, close) in [("\"", "\""), ("\"`", "`\"")] {
            let source = format!(
                "quadrantChart{ending}\
                 x-axis {open}Low{ending}Effort{close} --> High{ending}\
                 quadrant-1 {open}**First**{ending}Second{close} ???{ending}\
                 quadrant-2 {open}Literal\\nValue{close}{ending}\
                 {open}Point{ending}Name{close}: [0.2, 0.8]{ending}"
            );
            let svg = render_quadrantchart_svg_from_text(&source);
            let document = roxmltree::Document::parse(&svg).unwrap();
            for (class, expected) in [
                ("label", "Low\nEffort"),
                ("quadrant", "**First**\nSecond ???"),
                ("quadrant", r"Literal\nValue"),
                ("data-point", "Point\nName"),
            ] {
                assert!(
                    document.descendants().any(|node| {
                        node.has_tag_name("g")
                            && node.attribute("class") == Some(class)
                            && node.children().any(|child| {
                                child.has_tag_name("text") && child.text() == Some(expected)
                            })
                    }),
                    "missing literal {class} text {expected:?}"
                );
            }
            assert!(
                !document.descendants().any(|node| {
                    node.has_tag_name("foreignObject") || node.has_tag_name("tspan")
                })
            );
        }
    }
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
    let document = roxmltree::Document::parse(&svg).unwrap();
    let circle = document
        .descendants()
        .find(|node| node.has_tag_name("circle"))
        .unwrap();
    assert_eq!(circle.attribute("fill"), Some("#facc15"));
    assert_eq!(circle.attribute("stroke"), Some("#facc15"));
    assert_eq!(circle.attribute("r"), Some("5"));
    assert_eq!(circle.attribute("stroke-width"), Some("0px"));
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

#[test]
fn quadrantchart_svg_preserves_db_accessibility_text() {
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
            "quadrantChart\naccTitle: {title}\naccDescr {{\n{description}\n}}\nA: [0.5,0.5]\n"
        );
        let svg = render_quadrantchart_svg_from_text(&source);
        common::assert_svg_accessibility(&svg, expected_title, expected_description);
    }
}
