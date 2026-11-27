mod common;
#[path = "radar_svg_test/native.rs"]
mod native;

use common::legacy_init_theme_compat_engine;
use merman_core::ParseOptions;

use merman_render::LayoutOptions;
use merman_render::environment::RenderEnvironment;
use merman_render::family;
use merman_render::resources::{
    RenderResourcePolicy, ResourceLimitCause, ResourceLimitId, ResourceLimitPhase,
};
use merman_render::svg::{SvgDebugOptions, SvgRenderOptions};

fn radar_stylesheet(svg: &str) -> &str {
    svg.split_once("<style>")
        .and_then(|(_, tail)| tail.split_once("</style>"))
        .map(|(stylesheet, _)| stylesheet)
        .unwrap_or_else(|| panic!("Radar SVG must contain one stylesheet: {svg}"))
}

fn render_radar_svg_from_text(text: &str) -> String {
    let parsed = legacy_init_theme_compat_engine()
        .parse_diagram_for_render_model_sync(text, ParseOptions::default())
        .expect("parse radar")
        .expect("detect radar")
        .into_presentation();
    let session = RenderEnvironment::deterministic().begin_session().unwrap();
    let artifact =
        family::prepare(&parsed, &LayoutOptions::default(), session).expect("layout radar");

    artifact
        .render_svg(&SvgRenderOptions::default(), &SvgDebugOptions::default())
        .expect("render radar")
        .svg()
        .to_owned()
}

fn try_render_radar_svg_with_resource_policy(
    text: &str,
    resource_policy: RenderResourcePolicy,
) -> merman_render::Result<String> {
    let parsed = legacy_init_theme_compat_engine()
        .parse_diagram_for_render_model_sync(text, ParseOptions::default())
        .expect("parse Radar resource-bound fixture")
        .expect("detect Radar resource-bound fixture")
        .into_presentation();
    let session = RenderEnvironment::deterministic()
        .with_resource_policy(resource_policy)
        .begin_session()
        .expect("begin Radar resource-bound session");
    let artifact = family::prepare(&parsed, &LayoutOptions::default(), session)?;
    let rendered =
        artifact.render_svg(&SvgRenderOptions::default(), &SvgDebugOptions::default())?;
    Ok(rendered.svg().to_owned())
}

#[test]
fn radar_svg_skips_mismatched_curves_and_preserves_authored_series_indices() {
    for graticule in ["circle", "polygon"] {
        for show_legend in [false, true] {
            for (curves, expected_indices, series_count) in [
                ("curve Short{1,2}", Vec::new(), 1),
                ("curve Long{1,2,3,4}", Vec::new(), 1),
                (
                    "curve Short{100,1}, Complete{1,2,3}, Long{1,2,3,200}",
                    vec![1],
                    3,
                ),
            ] {
                let source = format!(
                    "radar-beta\naxis A,B,C\n{curves}\ngraticule {graticule}\nticks 0\nshowLegend {show_legend}\n"
                );
                let svg = render_radar_svg_from_text(&source);
                let document = roxmltree::Document::parse(&svg).unwrap();
                let curve_nodes = document
                    .descendants()
                    .filter(|node| {
                        node.attribute("class")
                            .is_some_and(|class| class.starts_with("radarCurve-"))
                    })
                    .collect::<Vec<_>>();
                assert_eq!(
                    curve_nodes
                        .iter()
                        .map(|node| node.attribute("class").unwrap())
                        .collect::<Vec<_>>(),
                    expected_indices
                        .iter()
                        .map(|index| format!("radarCurve-{index}"))
                        .collect::<Vec<_>>(),
                    "{source}"
                );
                for node in curve_nodes {
                    assert_eq!(
                        node.tag_name().name(),
                        if graticule == "polygon" {
                            "polygon"
                        } else {
                            "path"
                        }
                    );
                }
                let legend_classes = document
                    .descendants()
                    .filter_map(|node| node.attribute("class"))
                    .filter(|class| class.starts_with("radarLegendBox-"))
                    .collect::<Vec<_>>();
                let legend_count = if show_legend { series_count } else { 0 };
                assert_eq!(
                    legend_classes,
                    (0..legend_count)
                        .map(|index| format!("radarLegendBox-{index}"))
                        .collect::<Vec<_>>()
                );
            }
        }
    }
}

#[test]
fn radar_svg_preserves_fractional_tick_counts_and_radii_in_both_graticules() {
    for graticule in ["circle", "polygon"] {
        for (ticks, expected_radii) in [
            ("2", vec![150.0, 300.0]),
            ("2.0", vec![150.0, 300.0]),
            ("2.5", vec![120.0, 240.0, 360.0]),
            ("0", Vec::new()),
        ] {
            let source = format!(
                "radar-beta\naxis A,B,C\ncurve Complete{{1,2,3}}\ngraticule {graticule}\nticks {ticks}\n"
            );
            let svg = render_radar_svg_from_text(&source);
            let document = roxmltree::Document::parse(&svg).unwrap();
            let graticules = document
                .descendants()
                .filter(|node| node.attribute("class") == Some("radarGraticule"))
                .collect::<Vec<_>>();
            assert_eq!(graticules.len(), expected_radii.len(), "{source}");
            for (node, expected_radius) in graticules.iter().zip(expected_radii) {
                assert_eq!(node.tag_name().name(), graticule);
                let radius = if graticule == "circle" {
                    node.attribute("r").unwrap().parse::<f64>().unwrap()
                } else {
                    -node
                        .attribute("points")
                        .unwrap()
                        .split_whitespace()
                        .next()
                        .unwrap()
                        .split(',')
                        .nth(1)
                        .unwrap()
                        .parse::<f64>()
                        .unwrap()
                };
                assert!((radius - expected_radius).abs() < 1e-9, "{source}");
            }
        }
    }
}

#[test]
fn radar_family_svg_accepts_exact_max_svg_bytes_and_rejects_one_byte_less() {
    let source = r#"radar-beta
title Bounded capability
axis Reliability,Performance,Clarity
curve Current{3,4,2}
curve Target{5,5,5}
"#;
    let baseline = try_render_radar_svg_with_resource_policy(
        source,
        RenderResourcePolicy::unbounded_for_trusted_input(),
    )
    .expect("render the unbounded Radar baseline");
    let exact_bytes = baseline.len();
    assert!(exact_bytes > 1, "Radar fixture must emit a non-empty SVG");

    let exact_policy = RenderResourcePolicy::unbounded_for_trusted_input()
        .with_limit(ResourceLimitId::MaxSvgBytes, exact_bytes)
        .expect("valid exact Radar SVG byte ceiling");
    let exact = try_render_radar_svg_with_resource_policy(source, exact_policy)
        .expect("the exact Radar family SVG byte ceiling must succeed");
    assert_eq!(exact.as_bytes(), baseline.as_bytes());

    let below_exact = exact_bytes - 1;
    let below_policy = RenderResourcePolicy::unbounded_for_trusted_input()
        .with_limit(ResourceLimitId::MaxSvgBytes, below_exact)
        .expect("valid below-exact Radar SVG byte ceiling");
    let error = try_render_radar_svg_with_resource_policy(source, below_policy)
        .expect_err("one byte below the Radar family SVG size must fail");
    let merman_render::Error::ResourceLimitExceeded(limit) = error else {
        panic!("expected Radar MaxSvgBytes rejection, got {error}");
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

#[test]
fn radar_frontmatter_title_renders_unless_the_body_overrides_it() {
    let frontmatter_svg = render_radar_svg_from_text(
        r#"---
title: Frontmatter radar
---
radar-beta
axis A,B,C
curve score{1,2,3}
"#,
    );
    assert!(
        frontmatter_svg.contains(r#"class="radarTitle""#)
            && frontmatter_svg.contains(">Frontmatter radar</text>"),
        "frontmatter title should render when the Radar body has none: {frontmatter_svg}"
    );

    let body_svg = render_radar_svg_from_text(
        r#"---
title: Frontmatter radar
---
radar-beta
title Body radar
axis A,B,C
curve score{1,2,3}
"#,
    );
    assert!(body_svg.contains(">Body radar</text>"));
    assert!(!body_svg.contains(">Frontmatter radar</text>"));
}

#[test]
fn radar_interpolated_role_sizes_preserve_whitespace_arrays_and_option_priority() {
    for (value, visible) in [
        (serde_json::json!(24), true),
        (serde_json::json!("24"), true),
        (serde_json::json!(" 24"), true),
        (serde_json::json!("24 "), false),
        (serde_json::json!("24\n"), false),
        (serde_json::json!("24\t"), false),
        (serde_json::json!("24pt"), false),
        (serde_json::json!([24]), true),
        (serde_json::json!(["24"]), true),
        (serde_json::json!([[24]]), true),
    ] {
        for scoped in [false, true] {
            let roles = serde_json::json!({"axisLabelFontSize":value,"legendFontSize":value});
            let mut config = serde_json::json!({"themeVariables":{"fontSize":"0px"}});
            if scoped {
                config["themeVariables"]["radar"] = roles;
            } else {
                config["themeVariables"]["radar"] =
                    serde_json::json!({"axisLabelFontSize":24,"legendFontSize":24});
                config["radar"] = roles;
            }
            let source = format!("---\nconfig: {config}\n---\n{RADAR_TEXT_SOURCE}");
            let svg = render_radar_svg_from_text(&source);
            let css = radar_stylesheet(&svg);
            let size = if visible { "24px" } else { "inherit" };
            assert!(
                css.contains(&format!(".radarAxisLabel{{font-size:{size};")),
                "{config}"
            );
            assert!(
                css.contains(&format!(
                    ".radarLegendText{{text-anchor:start;font-size:{size};"
                )),
                "{config}"
            );
        }
    }
}

const RADAR_TEXT_SOURCE: &str =
    "radar-beta\ntitle Radar heading\naxis Alpha,Beta,Gamma\ncurve Current{3,4,2}\n";

#[test]
fn radar_svg_preserves_db_accessibility_text() {
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
            "radar-beta\naccTitle: {title}\naccDescr {{\n{description}\n}}\naxis A,B,C\ncurve Metric{{1,2,3}}\n"
        );
        let svg = render_radar_svg_from_text(&source);
        common::assert_svg_accessibility(&svg, expected_title, expected_description);
    }
}
