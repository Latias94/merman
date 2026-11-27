mod common;
#[path = "pie_svg_test/native.rs"]
mod native;

use common::legacy_init_theme_compat_engine;
use merman_core::ParseOptions;
use merman_render::environment::{RenderEnvironment, TextMeasurementPolicy};
use merman_render::family;
use merman_render::model::PieDiagramLayout;
use merman_render::svg::{SvgDebugOptions, SvgRenderOptions};
use merman_render::theme::{BuiltinPreset, Color, NativeTheme, ThemeCompiler};
use merman_render::{
    Error, LayoutOptions, RenderResourcePolicy, ResourceLimitCause, ResourceLimitId,
    ResourceLimitPhase,
};

fn layout_pie_from_text(text: &str) -> PieDiagramLayout {
    let engine = legacy_init_theme_compat_engine();
    let parsed = engine
        .parse_diagram_for_render_model_sync(text, ParseOptions::default())
        .expect("parse ok")
        .expect("diagram detected")
        .into_presentation();
    let session = RenderEnvironment::deterministic()
        .with_text_measurement_policy(TextMeasurementPolicy::deterministic())
        .begin_session()
        .unwrap();
    let artifact = family::prepare(&parsed, &LayoutOptions::default(), session).expect("layout ok");
    let projection = artifact.layout_json().expect("serialize Pie layout");
    serde_json::from_value(projection["layout"]["PieDiagram"].clone())
        .expect("Pie layout projection")
}

fn render_pie_from_text(text: &str) -> String {
    render_pie_from_text_with_options(text, &SvgRenderOptions::default())
}

fn render_pie_from_text_with_options(text: &str, options: &SvgRenderOptions) -> String {
    let session = RenderEnvironment::deterministic()
        .with_text_measurement_policy(TextMeasurementPolicy::deterministic())
        .begin_session()
        .unwrap();
    render_pie_with_session(text, options, session)
}

fn render_pie_with_session(
    text: &str,
    options: &SvgRenderOptions,
    session: merman_render::environment::RenderSession,
) -> String {
    let parsed = legacy_init_theme_compat_engine()
        .parse_diagram_for_render_model_sync(text, ParseOptions::default())
        .expect("parse ok")
        .expect("diagram detected")
        .into_presentation();
    let artifact = family::prepare(&parsed, &LayoutOptions::default(), session).expect("layout ok");

    artifact
        .render_svg(options, &SvgDebugOptions::default())
        .expect("svg render ok")
        .svg()
        .to_owned()
}

#[test]
fn native_pie_percentage_ink_uses_host_canvas_and_slice_opacity() {
    let source = "pie\n\"Opaque\": 2\n\"Translucent\": 1\n";
    let options = SvgRenderOptions::default();
    let environment = RenderEnvironment::deterministic();
    let authored_ink = "#0F172A";
    let compile = |fill_ink| {
        let document = serde_json::json!({
            "schema_version": 2,
            "appearances": {"light": {
                "seed": {"canvas": "#FFFFFF", "ink": "#151515", "accent": "#224466"},
                "data": {
                    "fill": ["#999999", "#EEEEEE80", "#AAAAAA", "#BBBBBB"],
                    "fill_ink": fill_ink,
                },
            }},
            "form": {"chart": {"pie_opacity": 0.65}},
        });
        ThemeCompiler::new()
            .compile_json(&serde_json::to_vec(&document).unwrap(), None)
            .unwrap()
    };
    let automatic = compile("auto");
    let authored = compile(authored_ink);
    assert_eq!(
        serde_json::to_value(automatic.as_ref()).unwrap(),
        serde_json::to_value(authored.as_ref()).unwrap(),
    );

    for (fill_ink, theme) in [("auto", automatic), (authored_ink, authored)] {
        let native = NativeTheme::new(theme);
        for (background, automatic_ink) in [
            (None, "#0F172A"),
            (Some(Color::rgb(0, 0, 0)), "#F8FAFC"),
            (Some(Color::parse("#00000080").unwrap()), "#0F172A"),
            (Some(Color::TRANSPARENT), "#0F172A"),
        ] {
            let session = environment
                .begin_session_with_native_theme(native.clone())
                .unwrap();
            let session = match background {
                Some(color) => session.with_canvas_background(color),
                None => session,
            };
            let svg = render_pie_with_session(source, &options, session);
            let svg = roxmltree::Document::parse(&svg).unwrap();
            let inks: Vec<_> = svg
                .descendants()
                .filter(|node| node.attribute("class") == Some("slice"))
                .map(|node| node.attribute("fill").unwrap())
                .collect();
            let expected = if fill_ink == "auto" {
                automatic_ink
            } else {
                authored_ink
            };
            assert_eq!(inks, [expected, expected], "{fill_ink} over {background:?}");
        }
    }
}

#[test]
fn pie_show_data_preserves_number_precision_in_text_and_measured_bounds() {
    let theme = ThemeCompiler::new()
        .compile_preset(BuiltinPreset::Studio, None)
        .unwrap();
    let native = NativeTheme::new(theme);
    for (token, expected) in [
        ("0.0000001", "1e-7"),
        ("0.000001", "0.000001"),
        ("1.0000001", "1.0000001"),
        ("42", "42"),
        ("9007199254740993", "9007199254740992"),
        ("1000000000000000000000", "1e+21"),
    ] {
        let source = format!("pie showData\n\"Data\": {token}\n");
        let legend = format!("Data [{expected}]");
        let literal_source = format!("pie\n\"{legend}\": 1\n");
        for themed in [false, true] {
            let render = |source: &str| {
                if themed {
                    let session = RenderEnvironment::deterministic()
                        .begin_session_with_native_theme(native.clone())
                        .unwrap();
                    render_pie_with_session(source, &SvgRenderOptions::default(), session)
                } else {
                    render_pie_from_text(source)
                }
            };
            let actual = render(&source);
            let literal = render(&literal_source);
            let document = roxmltree::Document::parse(&actual).unwrap();
            let literal_document = roxmltree::Document::parse(&literal).unwrap();
            let rendered_legend = document
                .descendants()
                .find(|node| node.attribute("class") == Some("legend"))
                .and_then(|node| node.children().find(|child| child.has_tag_name("text")))
                .and_then(|node| node.text());
            assert_eq!(
                rendered_legend,
                Some(legend.as_str()),
                "{token}, themed={themed}"
            );
            assert_eq!(
                document.root_element().attribute("viewBox"),
                literal_document.root_element().attribute("viewBox"),
                "measurement must consume the same Number text as output, {token}, themed={themed}"
            );
            assert!(document.descendants().any(|node| node.has_tag_name("text")
                && node.attribute("class") == Some("slice")
                && node.text() == Some("100%")));
        }
    }
}

#[test]
fn pie_multiline_string_reaches_visible_legend_and_preserves_slice_values() {
    let svg = render_pie_from_text("pie showData\n\"First\nSecond\": 10\n\"Other\": 5\n");
    let document = roxmltree::Document::parse(&svg).expect("valid multiline Pie SVG");
    let legends: Vec<_> = document
        .descendants()
        .filter(|node| node.attribute("class") == Some("legend"))
        .map(|legend| {
            legend
                .children()
                .find(|node| node.has_tag_name("text"))
                .unwrap()
                .text()
                .unwrap()
        })
        .collect();
    assert_eq!(legends, ["First\nSecond [10]", "Other [5]"]);
    let percentages: Vec<_> = document
        .descendants()
        .filter(|node| node.has_tag_name("text") && node.attribute("class") == Some("slice"))
        .map(|node| node.text().unwrap())
        .collect();
    assert_eq!(percentages, ["67%", "33%"]);
    assert_eq!(
        document
            .descendants()
            .filter(|node| {
                node.has_tag_name("path") && node.attribute("class") == Some("pieCircle")
            })
            .count(),
        2
    );
}

#[test]
fn pie_configured_text_sizes_preserve_css_and_expand_the_horizontal_viewport() {
    let diagram = "pie title TitleTitleTitleTitle\n  \"LegendLegendLegend\" : 1\n";
    let baseline = render_pie_from_text(diagram);
    let enlarged = render_pie_from_text(&format!(
        "%%{{init: {{\"themeVariables\": {{\"pieTitleTextSize\": \"60px\", \"pieLegendTextSize\": \"40px\"}}}}}}%%\n{diagram}"
    ));
    let width = |svg: &str| {
        let document = roxmltree::Document::parse(svg).unwrap();
        document
            .root_element()
            .attribute("viewBox")
            .unwrap()
            .split_whitespace()
            .nth(2)
            .unwrap()
            .parse::<f64>()
            .unwrap()
    };
    assert!(enlarged.contains("font-size:60px;"));
    assert!(enlarged.contains("font-size:40px;"));
    assert!(width(&enlarged) > width(&baseline));
}

#[test]
fn pie_absolute_role_sizes_preserve_css_and_share_the_px_viewport() {
    let diagram = "pie title Typography sizing comparison comparison\n  \"WideLegendWideLegendWideLegendWideLegend\" : 1\n";
    for (key, selector) in [
        ("pieTitleTextSize", "#merman .pieTitleText"),
        ("pieLegendTextSize", "#merman .legend text"),
    ] {
        for (css, pixels, from_site) in [
            ("24pt", "32px", false),
            ("2PC", "32px", false),
            ("1in", "96px", false),
            ("2.54cm", "96px", false),
            ("25.4mm", "96px", false),
            ("101.6q", "96px", false),
            (r"24p\74", "32px", false),
            (r"24p\74 !important", "32px", true),
        ] {
            let render = |size| {
                let config = serde_json::json!({"themeVariables": {key: size}});
                let (source, engine) = if from_site {
                    (
                        diagram.to_owned(),
                        merman_core::MermaidEngine::new().with_site_config(
                            merman_core::presentation::SourcePresentationConfig::from_value(config),
                        ),
                    )
                } else {
                    (
                        format!("---\nconfig: {config}\n---\n{diagram}"),
                        merman_core::MermaidEngine::new(),
                    )
                };
                let parsed = engine
                    .parse_diagram_for_render_model_sync(&source, ParseOptions::strict())
                    .unwrap()
                    .unwrap()
                    .into_presentation();
                assert_eq!(
                    parsed
                        .metadata()
                        .effective_config
                        .get_str(&format!("themeVariables.{key}")),
                    Some(size),
                    "the admitted size must reach measurement, site={from_site}"
                );
                let session = RenderEnvironment::deterministic().begin_session().unwrap();
                family::prepare(&parsed, &LayoutOptions::default(), session)
                    .unwrap()
                    .render_svg(&SvgRenderOptions::default(), &SvgDebugOptions::default())
                    .unwrap()
                    .svg()
                    .to_owned()
            };
            let actual = render(css);
            let expected = render(pixels);
            assert_eq!(
                pie_stylesheet_property_values(&actual, selector, "font-size"),
                [css]
            );
            let viewport = |svg: &str| {
                let document = roxmltree::Document::parse(svg).unwrap();
                document
                    .root_element()
                    .attribute("viewBox")
                    .unwrap()
                    .split_whitespace()
                    .map(|value| value.parse::<f64>().unwrap())
                    .collect::<Vec<_>>()
            };
            for (actual, expected) in viewport(&actual).into_iter().zip(viewport(&expected)) {
                assert!(
                    (actual - expected).abs() < 1e-9,
                    "{key}/{css}: {actual} != {expected}"
                );
            }
        }
    }
}

#[test]
fn pie_outer_stroke_config_reaches_layout_circle_and_stylesheet() {
    assert_eq!(
        pie_outer_circle_radius(&render_pie_from_text("pie\n\"A\": 1\n")),
        186.0
    );
    for (width, css, expected_radius) in [
        ("0", "0", 185.0),
        ("20.5", "20.5", 195.25),
        ("'20.5px'", "20.5px", 195.0),
        ("'5em'", "5em", 187.5),
        ("'100%'", "100%", 235.0),
        ("'calc(1px + 1em)'", "calc(1px + 1em)", 186.0),
        ("'2px'", "2px", 186.0),
    ] {
        let source = format!(
            "---\nconfig:\n  themeVariables:\n    pieOuterStrokeWidth: {width}\n---\npie\n\"A\": 1\n"
        );
        let layout = layout_pie_from_text(&source);
        let svg = render_pie_from_text(&source);
        assert_eq!(layout.outer_radius, expected_radius, "{width}");
        assert_eq!(pie_outer_circle_radius(&svg), expected_radius, "{width}");
        assert_eq!(
            pie_stylesheet_property_values(&svg, "#merman .pieOuterCircle", "stroke-width"),
            [css],
            "the authored CSS spelling must survive its separate geometry consumer"
        );
    }
}

#[test]
fn pie_title_remains_valid_xml_when_control_character_removal_joins_cdata_terminators() {
    let svg = render_pie_from_text("pie title Ω]]\u{1f}>β >\n  \"Alpha\" : 1\n");
    let document = roxmltree::Document::parse(&svg).expect("the Pie producer must emit valid XML");
    let title = document
        .descendants()
        .find(|node| node.attribute("class") == Some("pieTitleText"))
        .expect("visible Pie title");
    assert_eq!(title.text(), Some("Ω]]>β >"));
    assert!(svg.contains("Ω]]&gt;β >"));
}

fn pie_fills(svg: &str) -> (Vec<String>, Vec<String>) {
    let document = roxmltree::Document::parse(svg).expect("valid Pie SVG");
    let slice_fills = document
        .descendants()
        .filter(|node| {
            node.has_tag_name("path")
                && node.attribute("class").is_some_and(|class| {
                    class
                        .split_ascii_whitespace()
                        .any(|part| part == "pieCircle")
                })
        })
        .map(|node| node.attribute("fill").expect("Pie slice fill").to_string())
        .collect();
    let legend_styles = document
        .descendants()
        .filter(|node| {
            node.has_tag_name("rect")
                && node.parent().is_some_and(|parent| {
                    parent.has_tag_name("g") && parent.attribute("class") == Some("legend")
                })
        })
        .map(|node| {
            node.attribute("style")
                .expect("Pie legend style")
                .to_string()
        })
        .collect();
    (slice_fills, legend_styles)
}

fn pie_stylesheet_property_values(svg: &str, selector: &str, property: &str) -> Vec<String> {
    let document = roxmltree::Document::parse(svg).expect("valid Pie SVG");
    document
        .descendants()
        .filter(|node| node.has_tag_name("style"))
        .filter_map(|node| node.text())
        .flat_map(|css| css.split('}'))
        .filter_map(|rule| rule.split_once('{'))
        .filter(|(candidate, _)| candidate.trim() == selector)
        .flat_map(|(_, declarations)| declarations.split(';'))
        .filter_map(|declaration| declaration.split_once(':'))
        .filter(|(name, _)| name.trim() == property)
        .map(|(_, value)| value.trim().to_string())
        .collect()
}

fn try_render_pie_svg_with_resource_policy(
    text: &str,
    resource_policy: RenderResourcePolicy,
) -> merman_render::Result<String> {
    let engine = legacy_init_theme_compat_engine();
    let parsed = engine
        .parse_diagram_for_render_model_sync(text, ParseOptions::default())
        .expect("parse Pie resource-bound fixture")
        .expect("detect Pie resource-bound fixture")
        .into_presentation();
    let session = RenderEnvironment::deterministic()
        .with_text_measurement_policy(TextMeasurementPolicy::deterministic())
        .with_resource_policy(resource_policy)
        .begin_session()
        .expect("begin Pie resource-bound session");
    let artifact = family::prepare(&parsed, &LayoutOptions::default(), session)?;
    let rendered =
        artifact.render_svg(&SvgRenderOptions::default(), &SvgDebugOptions::default())?;
    Ok(rendered.svg().to_owned())
}

#[test]
fn pie_family_svg_accepts_exact_max_svg_bytes_and_rejects_one_byte_less() {
    for source in [
        r#"pie showData title Bounded distribution
  "Alpha" : 3
  "Beta" : 2
  "Gamma" : 1
"#,
        "---\nconfig:\n  themeVariables:\n    pieOuterStrokeWidth: 20px\n---\npie\n\"A\": 1\n",
        "---\nconfig:\n  pie:\n    legendPosition: left\n  themeVariables:\n    pieTitleTextSize: 80px\n---\npie showData title WWWWWWWWWWWWWWWW\n\"A\": 1\n",
    ] {
        let baseline = try_render_pie_svg_with_resource_policy(
            source,
            RenderResourcePolicy::unbounded_for_trusted_input(),
        )
        .expect("render the unbounded Pie baseline");
        let exact_bytes = baseline.len();
        assert!(exact_bytes > 1, "Pie fixture must emit a non-empty SVG");

        let exact_policy = RenderResourcePolicy::unbounded_for_trusted_input()
            .with_limit(ResourceLimitId::MaxSvgBytes, exact_bytes)
            .expect("valid exact Pie SVG byte ceiling");
        let exact = try_render_pie_svg_with_resource_policy(source, exact_policy)
            .expect("the exact Pie family SVG byte ceiling must succeed");
        assert_eq!(exact.as_bytes(), baseline.as_bytes());

        let below_exact = exact_bytes - 1;
        let below_policy = RenderResourcePolicy::unbounded_for_trusted_input()
            .with_limit(ResourceLimitId::MaxSvgBytes, below_exact)
            .expect("valid below-exact Pie SVG byte ceiling");
        let error = try_render_pie_svg_with_resource_policy(source, below_policy)
            .expect_err("one byte below the Pie family SVG size must fail");
        let merman_render::Error::ResourceLimitExceeded(limit) = error else {
            panic!("expected Pie MaxSvgBytes rejection, got {error}");
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
}

fn render_pie_with_svg_limit(
    text: &str,
    diagram_id: &str,
    maximum: usize,
) -> merman_render::Result<String> {
    let engine = legacy_init_theme_compat_engine();
    let parsed = engine
        .parse_diagram_for_render_model_sync(text, ParseOptions::default())
        .expect("parse ok")
        .expect("diagram detected")
        .into_presentation();
    let policy = RenderResourcePolicy::unbounded_for_trusted_input()
        .with_limit(ResourceLimitId::MaxSvgBytes, maximum)
        .unwrap();
    let session = RenderEnvironment::deterministic()
        .with_resource_policy(policy)
        .with_text_measurement_policy(TextMeasurementPolicy::deterministic())
        .begin_session()
        .expect("begin bounded Pie session");
    let artifact = family::prepare(&parsed, &LayoutOptions::default(), session).expect("layout ok");

    artifact
        .render_svg(
            &SvgRenderOptions {
                diagram_id: Some(diagram_id.to_string()),
                ..SvgRenderOptions::default()
            },
            &SvgDebugOptions::default(),
        )
        .map(|rendered| rendered.svg().to_owned())
}

fn render_pie_error_with_svg_limit(text: &str, diagram_id: &str, maximum: usize) -> Error {
    match render_pie_with_svg_limit(text, diagram_id, maximum) {
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

fn pie_content_translate(svg: &str) -> (f64, f64) {
    let document = roxmltree::Document::parse(svg).expect("valid Pie SVG");
    let centered = document
        .root_element()
        .children()
        .find(|node| node.is_element() && node.attribute("transform").is_some())
        .expect("centered pie group");
    let transform = centered
        .children()
        .find(|node| node.is_element() && node.attribute("transform").is_some())
        .and_then(|node| node.attribute("transform"))
        .expect("translated pie content group");
    let values = transform
        .strip_prefix("translate(")
        .and_then(|value| value.strip_suffix(')'))
        .expect("translate transform")
        .split(',')
        .map(|value| value.parse::<f64>().expect("numeric translate component"))
        .collect::<Vec<_>>();
    assert_eq!(values.len(), 2, "two-dimensional translate transform");
    (values[0], values[1])
}

fn pie_outer_circle_radius(svg: &str) -> f64 {
    let document = roxmltree::Document::parse(svg).expect("valid Pie SVG");
    document
        .descendants()
        .find(|node| node.attribute("class") == Some("pieOuterCircle"))
        .and_then(|node| node.attribute("r"))
        .expect("outer circle radius")
        .parse()
        .expect("numeric outer circle radius")
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
    let projected_svg_bytes = full_svg.len();
    let n_minus_one_maximum = projected_svg_bytes
        .checked_sub(1)
        .expect("Pie SVG projection must not be empty");

    let Error::ResourceLimitExceeded(n_minus_one) =
        render_pie_error_with_svg_limit(source, &diagram_id, n_minus_one_maximum)
    else {
        panic!("expected N-1 Pie CSS byte projection error");
    };
    assert_eq!(n_minus_one.cause, ResourceLimitCause::Ceiling);
    assert_eq!(n_minus_one.phase, ResourceLimitPhase::SvgOutput);
    assert_eq!(n_minus_one.limit, ResourceLimitId::MaxSvgBytes.as_str());
    assert_eq!(n_minus_one.actual, projected_svg_bytes);
    assert_eq!(n_minus_one.max, n_minus_one_maximum);

    let exact = render_pie_with_svg_limit(source, &diagram_id, projected_svg_bytes)
        .expect("the exact Pie CSS byte projection must succeed");
    assert_eq!(exact, full_svg);
}

#[test]
fn pie_chart_content_is_grouped_before_title_and_legend_like_mermaid_11_16() {
    let svg = render_pie_from_text(
        r#"pie
  "A" : 3
  "B" : 2
"#,
    );

    assert!(
        svg.contains(
            r#"<g transform="translate(225,225)"><g><circle cx="0" cy="0" r="186" class="pieOuterCircle"/>"#
        ),
        "pie geometry should start in its own attribute-free group: {svg}"
    );
    assert!(
        svg.contains(
            r#">40%</text></g><text x="0" y="-200" class="pieTitleText"/><g class="legend""#
        ),
        "the pie group should close before the sibling title and legend nodes: {svg}"
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
    assert!(
        frontmatter_svg.contains(r#"class="pieTitleText">Frontmatter pie</text>"#),
        "frontmatter title should render when the Pie body has none: {frontmatter_svg}"
    );

    let body_svg = render_pie_from_text(
        r#"---
title: Frontmatter pie
---
pie title Body pie
  "A" : 1
"#,
    );
    assert!(body_svg.contains(r#"class="pieTitleText">Body pie</text>"#));
    assert!(!body_svg.contains(">Frontmatter pie</text>"));
}

#[test]
fn pie_frontmatter_title_preserves_common_db_boundary_whitespace() {
    for title in ["  Frontmatter pie  ", "\u{a0}Frontmatter pie\u{a0}"] {
        let source = format!("---\ntitle: \"{title}\"\n---\npie\n  \"A\" : 1\n");
        let svg = render_pie_from_text(&source);

        assert!(
            svg.contains(&format!(r#"class="pieTitleText">{title}</text>"#)),
            "frontmatter title should be emitted exactly: {svg}"
        );
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
fn pie_full_circles_keep_closed_outer_and_reversed_inner_rings() {
    for (hole, expected) in [
        (0.0, "M0,-185A185,185,0,1,1,0,185A185,185,0,1,1,0,-185Z"),
        (
            0.4,
            "M0,-185A185,185,0,1,1,0,185A185,185,0,1,1,0,-185M0,-74A74,74,0,1,0,0,74A74,74,0,1,0,0,-74Z",
        ),
    ] {
        for sections in ["  \"A\" : 1\n", "  \"A\" : 1000\n  \"Filtered\" : 1\n"] {
            let source =
                format!("%%{{init: {{\"pie\": {{\"donutHole\": {hole}}}}}}}%%\npie\n{sections}");
            let svg = render_pie_from_text(&source);
            let document = roxmltree::Document::parse(&svg).expect("valid Pie SVG");
            let paths: Vec<_> = document
                .descendants()
                .filter(|node| node.attribute("class") == Some("pieCircle"))
                .map(|node| node.attribute("d").expect("slice path data"))
                .collect();
            assert_eq!(paths, [expected], "hole={hole}, sections={sections}");
        }
    }
}

#[test]
fn pie_donut_sector_paths_match_d3_rounding_and_winding() {
    // d3-shape 3.2.1 / d3-path 3.1.1: non-full 2:1 sectors retain opposite ring winding.
    let svg = render_pie_from_text(
        r#"%%{init: {"pie": {"donutHole": 0.4}}}%%
pie
  "A" : 2
  "B" : 1
"#,
    );
    let document = roxmltree::Document::parse(&svg).expect("valid Pie SVG");
    let paths: Vec<_> = document
        .descendants()
        .filter(|node| node.attribute("class") == Some("pieCircle"))
        .map(|node| node.attribute("d").expect("slice path data"))
        .collect();
    assert_eq!(
        paths,
        [
            "M0,-185A185,185,0,1,1,-160.215,92.5L-64.086,37A74,74,0,1,0,0,-74Z",
            "M-160.215,92.5A185,185,0,0,1,0,-185L0,-74A74,74,0,0,0,-64.086,37Z",
        ]
    );
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

    assert!(
        svg.contains("A74,74"),
        "expected inner-radius arc in donut slice path: {svg}"
    );
    assert!(
        !svg.contains("L0,0Z"),
        "donut slices should not close through the center: {svg}"
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

    assert!(
        !svg.contains("A222,222"),
        "invalid donutHole should not be used as an inner radius: {svg}"
    );
    assert!(
        svg.contains("L0,0Z"),
        "invalid donutHole should fall back to solid slices: {svg}"
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
    let top_offset = pie_content_translate(&top_svg);
    assert!(
        top_offset.0.abs() <= f64::EPSILON && (top_offset.1 - 66.0).abs() <= f64::EPSILON,
        "top legend should move the pie group below the legend: {top_svg}"
    );

    let left_svg = render_pie_from_text(
        r#"%%{init: {"pie": {"legendPosition": "left"}}}%%
pie
  "A" : 1
  "B" : 1
"#,
    );
    let left_offset = pie_content_translate(&left_svg);
    assert!(
        left_offset.0 > 22.0 && left_offset.1.abs() <= f64::EPSILON,
        "left legend should move the pie group right by legend width: {left_svg}"
    );
    assert!(left_svg.contains(r#"class="legend" transform="translate(-207,-22)""#));
}

#[test]
fn pie_left_legend_offset_is_independent_of_title_viewport_expansion() {
    use merman_render::environment::{
        MeasurementProfileId, TextMeasurementProfile, TextMeasurementProfileIdentity,
    };
    use merman_render::text::{TextMeasurer, TextMetrics, TextStyle};
    use std::sync::Arc;

    struct FixedPieMeasurer;

    impl TextMeasurer for FixedPieMeasurer {
        fn measure(&self, text: &str, style: &TextStyle) -> TextMetrics {
            TextMetrics {
                width: self.measure_svg_text_bounding_client_rect_width_px(text, style),
                height: style.font_size,
                line_count: 1,
            }
        }

        fn measure_svg_text_bounding_client_rect_width_px(
            &self,
            text: &str,
            _style: &TextStyle,
        ) -> f64 {
            match text {
                "A" => 12.0,
                "A [1]" => 40.0,
                "T" => 80.0,
                "WWWWWWWWWWWWWWWW" => 1_280.0,
                other => panic!("unexpected Pie measurement: {other:?}"),
            }
        }
    }

    for (header, expected_viewbox, expected_offset) in [
        ("", "0 0 524 450", 34.0),
        (" title T", "0 0 524 450", 34.0),
        (" title WWWWWWWWWWWWWWWW", "-415 0 1280 450", 34.0),
        (" showData", "0 0 552 450", 62.0),
        (" showData title T", "0 0 552 450", 62.0),
        (" showData title WWWWWWWWWWWWWWWW", "-415 0 1280 450", 62.0),
    ] {
        let source =
            format!("---\nconfig:\n  pie:\n    legendPosition: left\n---\npie{header}\n\"A\": 1\n");
        let parsed = legacy_init_theme_compat_engine()
            .parse_diagram_for_render_model_sync(&source, ParseOptions::strict())
            .unwrap()
            .unwrap()
            .into_presentation();
        let profile = TextMeasurementProfile::new(
            TextMeasurementProfileIdentity::new(
                MeasurementProfileId::new("pie-title-legend-geometry").unwrap(),
                "test",
            )
            .unwrap(),
            Arc::new(FixedPieMeasurer),
        );
        let session = RenderEnvironment::deterministic()
            .with_text_measurement_policy(TextMeasurementPolicy::uniform(profile))
            .begin_session()
            .unwrap();
        let artifact = family::prepare(&parsed, &LayoutOptions::default(), session).unwrap();
        let rendered = artifact
            .render_svg(&SvgRenderOptions::default(), &SvgDebugOptions::default())
            .unwrap();
        assert_eq!(
            pie_content_translate(rendered.svg()),
            (expected_offset, 0.0),
            "{header}"
        );
        let document = roxmltree::Document::parse(rendered.svg()).unwrap();
        assert_eq!(
            document.root_element().attribute("viewBox"),
            Some(expected_viewbox),
            "the root must still contain the measured title"
        );
    }
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

    assert!(
        svg.contains(r#".pieCircle.highlighted{scale:1.05;opacity:1;}"#),
        "Mermaid 11.16 pie CSS should include highlighted slice styling: {svg}"
    );
    assert!(
        svg.contains(r#"class="pieCircle highlighted""#),
        "Mermaid 11.16 should mark the configured highlighted slice: {svg}"
    );
    assert!(
        svg.contains(r#"class="pieCircle"/>"#),
        "non-matching slices should keep the ordinary pieCircle class: {svg}"
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

#[test]
fn pie_classic_palette_cycles_across_hidden_slices_and_legend_entries() {
    let mut source =
        "---\nconfig: {themeVariables: {pie1: '#125634', pie3: '#c2188b'}}\n---\npie\n".to_owned();
    for index in 0..13 {
        let value = usize::from(index != 1);
        source.push_str(&format!("  \"Category {index}\" : {value}\n"));
    }
    let (slices, legends) = pie_fills(&render_pie_from_text(&source));
    assert_eq!(slices.len(), 12);
    assert_eq!(legends.len(), 13);
    assert_eq!(slices[0], "#125634");
    assert_eq!(slices[1], "#c2188b");
    assert_eq!(slices[11], slices[0]);
    assert_eq!(
        legends[0],
        "fill: rgb(18, 86, 52); stroke: rgb(18, 86, 52);"
    );
    assert_eq!(
        legends[2],
        "fill: rgb(194, 24, 139); stroke: rgb(194, 24, 139);"
    );
    assert_eq!(legends[12], legends[0]);
}

#[test]
fn pie_classic_text_uses_css_fallbacks_for_nonstring_role_colors() {
    let source = r#"---
config:
  themeVariables:
    pieTitleTextColor: false
    pieSectionTextColor: false
    pieLegendTextColor: false
    textColor: '#125634'
    taskTextDarkColor: currentColor
---
pie title Ownership
  "Alpha": 1
"#;
    let svg = render_pie_from_text(source);
    for (selector, expected) in [
        ("#merman .pieTitleText", "currentColor"),
        ("#merman .slice", "#125634"),
        ("#merman .legend text", "currentColor"),
    ] {
        assert_eq!(
            pie_stylesheet_property_values(&svg, selector, "fill"),
            [expected]
        );
    }
}
