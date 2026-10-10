use std::sync::Arc;

use merman_core::{Engine, MermaidConfig, ParseOptions};
use merman_render::DiagramFamilyId;
use merman_render::LayoutOptions;
use merman_render::diagram_theme::{
    DiagramTheme, DiagramThemeCompiler, DiagramThemeSpec, FontStack, ThemePortabilityRequirement,
    ThemeTextStyle, TypographySpec,
};
use merman_render::environment::{
    MeasurementProfileId, RenderEnvironment, TextMeasurementPolicy, TextMeasurementProfile,
    TextMeasurementProfileIdentity,
};
use merman_render::family;
use merman_render::resources::{
    RenderResourcePolicy, ResourceLimitCause, ResourceLimitId, ResourceLimitPhase,
};
use merman_render::svg::{SvgDebugOptions, SvgRenderOptions};
use merman_render::text::{TextMeasurer, TextMetrics, TextStyle};
use serde_json::Value;

const WARDLEY_COMPLETE_FEATURE_SOURCE: &str = r#"wardley-beta
title Platform Strategy
accTitle: Platform map
accDescr: Strategic platform evolution
size [1100, 800]
evolution Genesis@0.25 -> Custom@0.5 -> Product@0.75 -> Commodity@1.0

anchor Customer [0.90, 0.95]
component App [0.80, 0.85] (build)
component API [0.70, 0.65] (buy)
component Vendor [0.60, 0.55] (outsource)
component Exchange [0.55, 0.50] (market)
component Database [0.50, 0.45] (buy) (inertia)

pipeline Database {
  component File System [0.25] label [-40, 20]
  component SQL DB [0.50]
}

Customer -> App; entry
App +> API
API +<> Vendor
Vendor +'backup'> Database
API -.-> Exchange
Database -> File System
evolve API 0.85
note "Build mobile-first" [0.85, 0.90]
annotations [0.10, 0.20]
annotation 1,[0.78, 0.82] "User touchpoints"
accelerator "Cloud Native" [0.20, 0.85]
deaccelerator "Legacy Data" [0.45, 0.35]
"#;

fn render_wardley(source: &str, site_config: Value, diagram_id: &str) -> String {
    let engine = Engine::new().with_site_config(MermaidConfig::from_value(site_config));
    let parsed = engine
        .parse_diagram_for_render_model_sync(source, ParseOptions::strict())
        .expect("Wardley parse succeeds")
        .expect("Wardley diagram is detected");
    let session = RenderEnvironment::deterministic()
        .begin_session()
        .expect("Wardley render session starts");
    let artifact = family::prepare(parsed, &LayoutOptions::headless_svg_defaults(), session)
        .expect("Wardley family prepares");
    assert_eq!(artifact.family_id(), DiagramFamilyId::WARDLEY);

    let rendered = artifact
        .render_svg(
            &SvgRenderOptions {
                diagram_id: Some(diagram_id.to_string()),
                ..Default::default()
            },
            &SvgDebugOptions::default(),
        )
        .expect("Wardley SVG renders");
    rendered.svg().to_owned()
}

fn try_render_wardley_with_resource_policy(
    source: &str,
    site_config: Value,
    diagram_id: &str,
    resource_policy: RenderResourcePolicy,
) -> merman_render::Result<String> {
    let engine = Engine::new().with_site_config(MermaidConfig::from_value(site_config));
    let parsed = engine
        .parse_diagram_for_render_model_sync(source, ParseOptions::strict())
        .expect("Wardley parse succeeds")
        .expect("Wardley diagram is detected");
    let session = RenderEnvironment::deterministic()
        .with_resource_policy(resource_policy)
        .begin_session()
        .expect("Wardley render session starts");
    let artifact = family::prepare(parsed, &LayoutOptions::headless_svg_defaults(), session)?;
    assert_eq!(artifact.family_id(), DiagramFamilyId::WARDLEY);

    let rendered = artifact.render_svg(
        &SvgRenderOptions {
            diagram_id: Some(diagram_id.to_string()),
            ..Default::default()
        },
        &SvgDebugOptions::default(),
    )?;
    Ok(rendered.svg().to_owned())
}

fn wardley_typography_theme(typography: ThemeTextStyle) -> DiagramTheme {
    DiagramThemeCompiler::new()
        .compile(DiagramThemeSpec::new().with_typography(
            TypographySpec::default().with_family_style(DiagramFamilyId::WARDLEY, typography),
        ))
        .expect("compile Wardley typography theme")
}

fn try_render_wardley_with_theme(
    source: &str,
    theme: &DiagramTheme,
    engine: Engine,
    environment: &RenderEnvironment,
    diagram_id: &str,
) -> merman_render::Result<(Value, family::RenderedFamilySvg)> {
    let parsed = merman_render::__private::install_parse_compatibility(theme, engine)
        .parse_diagram_for_render_model_sync(source, ParseOptions::strict())
        .expect("parse themed Wardley")
        .expect("detect themed Wardley");
    let session = environment.begin_session_with_theme(theme).unwrap();
    let artifact = family::prepare(parsed, &LayoutOptions::headless_svg_defaults(), session)?;
    let projection = artifact.layout_json()?;
    let layout = projection["layout"]["WardleyDiagram"].clone();
    let rendered = artifact.render_svg(
        &SvgRenderOptions {
            diagram_id: Some(diagram_id.to_string()),
            ..SvgRenderOptions::default()
        },
        &SvgDebugOptions::default(),
    )?;
    Ok((layout, rendered))
}

fn portable_wardley_environment() -> RenderEnvironment {
    RenderEnvironment::deterministic()
        .with_theme_portability_requirement(ThemePortabilityRequirement::RequirePortable)
}

fn has_class(node: roxmltree::Node<'_, '_>, class: &str) -> bool {
    node.attribute("class").is_some_and(|classes| {
        classes
            .split_whitespace()
            .any(|candidate| candidate == class)
    })
}

fn element_with_class<'a, 'input>(
    document: &'a roxmltree::Document<'input>,
    tag: &str,
    class: &str,
) -> roxmltree::Node<'a, 'input> {
    document
        .descendants()
        .find(|node| node.has_tag_name(tag) && has_class(*node, class))
        .unwrap_or_else(|| panic!("missing <{tag}>.{class}"))
}

#[test]
fn wardley_svg_serializes_the_complete_1116_feature_surface() {
    let svg = render_wardley(
        WARDLEY_COMPLETE_FEATURE_SOURCE,
        serde_json::json!({ "wardley-beta": { "showGrid": true } }),
        "wardley-feature-rich",
    );
    let document = roxmltree::Document::parse(&svg).expect("valid Wardley XML");
    let root = document.root_element();

    assert_eq!(root.attribute("viewBox"), Some("0 0 1100 800"));
    assert_eq!(root.attribute("aria-roledescription"), Some("wardley"));
    assert_eq!(
        root.attribute("aria-labelledby"),
        Some("chart-title-wardley-feature-rich")
    );
    assert_eq!(
        root.attribute("aria-describedby"),
        Some("chart-desc-wardley-feature-rich")
    );
    for class in [
        "wardley-map",
        "wardley-axes",
        "wardley-stages",
        "wardley-grid",
        "wardley-pipelines",
        "wardley-pipeline-links",
        "wardley-links",
        "wardley-trends",
        "wardley-nodes",
        "wardley-annotations",
        "wardley-annotations-box",
        "wardley-notes",
        "wardley-accelerators",
        "wardley-deaccelerators",
    ] {
        element_with_class(&document, "g", class);
    }
    for class in [
        "wardley-build-overlay",
        "wardley-buy-overlay",
        "wardley-outsource-overlay",
        "wardley-market-overlay",
        "wardley-market-dot",
    ] {
        element_with_class(&document, "circle", class);
    }
    element_with_class(&document, "line", "wardley-inertia");
    element_with_class(&document, "rect", "wardley-pipeline-box");
    element_with_class(&document, "line", "wardley-pipeline-evolution-link");
    element_with_class(&document, "line", "wardley-trend");
    element_with_class(&document, "line", "wardley-link--dashed");
    assert!(svg.contains(">Platform Strategy</text>"));
    assert!(svg.contains(">1. User touchpoints</text>"));
    assert!(svg.contains(">Cloud Native</text>"));
    assert!(svg.contains(">Legacy Data</text>"));
    assert!(svg.contains("link-arrow-end-wardley-feature-rich"));
    assert!(svg.contains("link-arrow-start-wardley-feature-rich"));
}

#[test]
fn wardley_family_svg_accepts_exact_max_svg_bytes_and_rejects_one_byte_less() {
    let site_config = serde_json::json!({ "wardley-beta": { "showGrid": true } });
    let diagram_id = "wardley-bounded";
    let baseline = try_render_wardley_with_resource_policy(
        WARDLEY_COMPLETE_FEATURE_SOURCE,
        site_config.clone(),
        diagram_id,
        RenderResourcePolicy::unbounded_for_trusted_input(),
    )
    .expect("render the unbounded Wardley baseline");
    let exact_bytes = baseline.len();
    assert!(exact_bytes > 1, "Wardley fixture must emit a non-empty SVG");

    let exact_policy = RenderResourcePolicy::unbounded_for_trusted_input()
        .with_limit(ResourceLimitId::MaxSvgBytes, exact_bytes)
        .expect("valid exact Wardley SVG byte ceiling");
    let exact = try_render_wardley_with_resource_policy(
        WARDLEY_COMPLETE_FEATURE_SOURCE,
        site_config.clone(),
        diagram_id,
        exact_policy,
    )
    .expect("the exact Wardley family SVG byte ceiling must succeed");
    assert_eq!(exact.as_bytes(), baseline.as_bytes());

    let below_exact = exact_bytes - 1;
    let below_policy = RenderResourcePolicy::unbounded_for_trusted_input()
        .with_limit(ResourceLimitId::MaxSvgBytes, below_exact)
        .expect("valid below-exact Wardley SVG byte ceiling");
    let error = try_render_wardley_with_resource_policy(
        WARDLEY_COMPLETE_FEATURE_SOURCE,
        site_config,
        diagram_id,
        below_policy,
    )
    .expect_err("one byte below the Wardley family SVG size must fail");
    let merman_render::Error::ResourceLimitExceeded(limit) = error else {
        panic!("expected Wardley MaxSvgBytes rejection, got {error}");
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
fn wardley_svg_uses_family_theme_roles_without_a_css_postpass() {
    let svg = render_wardley(
        "wardley-beta\ncomponent A [0.8, 0.2]\ncomponent B [0.4, 0.8]\nA -> B\nevolve A 0.7\n",
        serde_json::json!({
            "themeVariables": {
                "wardley": {
                    "backgroundColor": "#010203",
                    "axisColor": "#111213",
                    "axisTextColor": "#212223",
                    "gridColor": "#313233",
                    "componentFill": "#414243",
                    "componentStroke": "#515253",
                    "componentLabelColor": "#616263",
                    "linkStroke": "#717273",
                    "evolutionStroke": "#818283"
                }
            },
            "wardley-beta": { "showGrid": true }
        }),
        "wardley-theme",
    );
    let document = roxmltree::Document::parse(&svg).expect("valid Wardley XML");

    assert_eq!(
        element_with_class(&document, "rect", "wardley-background").attribute("fill"),
        Some("#010203")
    );
    assert_eq!(
        element_with_class(&document, "text", "wardley-axis-label").attribute("fill"),
        Some("#212223")
    );
    let component = document
        .descendants()
        .find(|node| {
            node.has_tag_name("circle")
                && node
                    .parent()
                    .is_some_and(|parent| has_class(parent, "wardley-node"))
        })
        .expect("component circle");
    assert_eq!(component.attribute("fill"), Some("#414243"));
    assert_eq!(component.attribute("stroke"), Some("#515253"));
    assert_eq!(
        element_with_class(&document, "line", "wardley-link").attribute("stroke"),
        Some("#717273")
    );
    assert_eq!(
        element_with_class(&document, "line", "wardley-trend").attribute("stroke"),
        Some("#818283")
    );
    assert_eq!(
        element_with_class(&document, "g", "wardley-map").attribute("font-family"),
        None,
        "the unthemed Wardley DOM baseline must not gain an inherited font writer"
    );
    assert!(!svg.contains("<style>"));
}

#[test]
fn wardley_svg_uses_upstream_direct_annotation_theme_roles() {
    let svg = render_wardley(
        concat!(
            "wardley-beta\n",
            "annotations [0.10, 0.20]\n",
            "annotation 1,[0.78, 0.82] \"User touchpoints\"\n",
        ),
        serde_json::json!({
            "themeVariables": {
                "wardley": {
                    "axisColor": "#010203",
                    "axisTextColor": "#111213",
                    // These style-level roles must not override Wardley's direct SVG attributes.
                    "annotationStroke": "#212223",
                    "annotationTextColor": "#313233",
                    "annotationFill": "#414243"
                }
            }
        }),
        "wardley-annotation-theme",
    );
    let document = roxmltree::Document::parse(&svg).expect("valid Wardley XML");

    let annotation_group = element_with_class(&document, "g", "wardley-annotation");
    assert_eq!(
        annotation_group
            .children()
            .find(|node| node.has_tag_name("circle"))
            .and_then(|node| node.attribute("fill")),
        Some("white")
    );
    assert_eq!(
        annotation_group
            .children()
            .find(|node| node.has_tag_name("circle"))
            .and_then(|node| node.attribute("stroke")),
        Some("#010203")
    );
    assert_eq!(
        annotation_group
            .children()
            .find(|node| node.has_tag_name("text"))
            .and_then(|node| node.attribute("fill")),
        Some("#111213")
    );
    let box_group = element_with_class(&document, "g", "wardley-annotations-box");
    assert_eq!(
        box_group
            .children()
            .find(|node| node.has_tag_name("rect"))
            .and_then(|node| node.attribute("fill")),
        Some("white")
    );
    assert_eq!(
        box_group
            .children()
            .find(|node| node.has_tag_name("rect"))
            .and_then(|node| node.attribute("stroke")),
        Some("#010203")
    );
    assert_eq!(
        box_group
            .children()
            .find(|node| node.has_tag_name("text"))
            .and_then(|node| node.attribute("fill")),
        Some("#111213")
    );
}

#[test]
fn wardley_svg_uses_frontmatter_title_when_the_body_has_none() {
    let svg = render_wardley(
        r#"---
title: Frontmatter map
---
wardley-beta
component API [0.6, 0.7]
"#,
        serde_json::json!({}),
        "wardley-frontmatter",
    );

    assert!(svg.contains(">Frontmatter map</text>"));
    assert!(svg.contains(r#"class="wardley-title""#));
}

#[derive(Debug)]
struct WardleyTypedFontProbeMeasurer {
    expected_font_family: String,
}

impl WardleyTypedFontProbeMeasurer {
    fn assert_style(&self, style: &TextStyle) {
        assert_eq!(
            style.font_family.as_deref(),
            Some(self.expected_font_family.as_str()),
            "Wardley annotation measurement must use the inherited writer's resolved winner"
        );
    }
}

impl TextMeasurer for WardleyTypedFontProbeMeasurer {
    fn measure(&self, text: &str, style: &TextStyle) -> TextMetrics {
        self.assert_style(style);
        TextMetrics {
            width: text.chars().count() as f64 * 10.0,
            height: 17.0,
            line_count: 1,
        }
    }

    fn measure_svg_text_computed_length_px(&self, _text: &str, style: &TextStyle) -> f64 {
        self.assert_style(style);
        123.0
    }

    fn measure_svg_raw_text_bbox_height_px(&self, _text: &str, style: &TextStyle) -> f64 {
        self.assert_style(style);
        17.0
    }
}

#[test]
fn wardley_typed_font_stack_drives_measurement_group_inheritance_and_terminal_evidence() {
    let font_stack =
        FontStack::new(["Wardley Typed", "monospace"]).expect("valid Wardley font stack");
    let expected_font = font_stack.as_css();
    let theme = wardley_typography_theme(ThemeTextStyle::default().with_font_stack(font_stack));
    let identity = TextMeasurementProfileIdentity::new(
        MeasurementProfileId::new("test.wardley-typed-font-probe").unwrap(),
        "test",
    )
    .unwrap();
    let environment = portable_wardley_environment().with_text_measurement_policy(
        TextMeasurementPolicy::uniform(TextMeasurementProfile::new(
            identity,
            Arc::new(WardleyTypedFontProbeMeasurer {
                expected_font_family: expected_font.clone(),
            }),
        )),
    );
    let (layout, rendered) = try_render_wardley_with_theme(
        concat!(
            "wardley-beta\n",
            "title Typed map\n",
            "component API [0.6, 0.7]\n",
            "annotations [0.10, 0.20]\n",
            "annotation 1,[0.78, 0.82] \"User touchpoints\"\n",
        ),
        &theme,
        Engine::new(),
        &environment,
        "wardley-typed-font",
    )
    .expect("render directly themed Wardley");

    assert_eq!(
        layout["annotations_box"]["max_text_width"].as_f64(),
        Some(123.0)
    );
    assert_eq!(
        layout["annotations_box"]["max_text_height"].as_f64(),
        Some(17.0)
    );
    let document = roxmltree::Document::parse(rendered.svg()).expect("valid themed Wardley SVG");
    let map = element_with_class(&document, "g", "wardley-map");
    assert_eq!(map.attribute("font-family"), Some(expected_font.as_str()));
    assert!(
        map.descendants()
            .filter(|node| node.has_tag_name("text"))
            .all(|node| node.attribute("font-family").is_none()),
        "Wardley text terminals must inherit from the single family group writer"
    );
    assert!(!rendered.svg().contains("<style>"));

    drop(document);
    let completion = rendered.into_completion();
    let evidence = merman_render::__private::family_evidence(completion.report());
    assert_eq!(evidence.required_count(), 1);
    assert_eq!(evidence.accounted_count(), 1);
    assert_eq!(evidence.applied_count(), 1);
    assert_eq!(evidence.not_applicable_count(), 0);
    assert_eq!(evidence.theme_residual_count(), 0);
    assert_eq!(evidence.compatibility_residual_count(), 0);
    assert_eq!(evidence.mermaid_compatibility_residual_count(), 0);
}

#[test]
fn wardley_direct_font_stack_exceeds_the_retired_legacy_bridge_limit_without_a_style_node() {
    let families = (0..32)
        .map(|index| format!("WardleyFont{index:02}{}", "x".repeat(140)))
        .collect::<Vec<_>>();
    let font_stack = FontStack::new(families).expect("valid large Wardley font stack");
    let expected_font = font_stack.as_css();
    assert!(expected_font.len() > 4 * 1024);
    let theme = wardley_typography_theme(ThemeTextStyle::default().with_font_stack(font_stack));

    let (_, rendered) = try_render_wardley_with_theme(
        "wardley-beta\ncomponent API [0.6, 0.7]\n",
        &theme,
        Engine::new(),
        &portable_wardley_environment(),
        "wardley-large-font",
    )
    .expect("render Wardley beyond the legacy bridge string budget");
    let document =
        roxmltree::Document::parse(rendered.svg()).expect("valid large-font Wardley SVG");
    assert_eq!(
        element_with_class(&document, "g", "wardley-map").attribute("font-family"),
        Some(expected_font.as_str())
    );
    assert!(!rendered.svg().contains("<style>"));

    drop(document);
    let completion = rendered.into_completion();
    let evidence = merman_render::__private::family_evidence(completion.report());
    assert_eq!(evidence.applied_count(), 1);
    assert_eq!(evidence.theme_residual_count(), 0);
    assert_eq!(evidence.compatibility_residual_count(), 0);
}

#[test]
fn wardley_explicit_site_and_source_font_paths_outrank_the_typed_stack() {
    let theme =
        wardley_typography_theme(ThemeTextStyle::default().with_font_stack(
            FontStack::single("TypedWardleyFont").expect("valid typed Wardley font"),
        ));
    let cases = [
        (
            "wardley-site-font",
            Engine::new().with_site_config(MermaidConfig::from_value(serde_json::json!({
                "themeVariables": { "fontFamily": "SiteWardleyFont,serif" }
            }))),
            "wardley-beta\ncomponent API [0.6, 0.7]\n".to_string(),
            "SiteWardleyFont,serif",
        ),
        (
            "wardley-source-font",
            Engine::new().with_site_config(MermaidConfig::from_value(serde_json::json!({
                "secure": []
            }))),
            concat!(
                "%%{init: {\"fontFamily\": \"SourceWardleyFont,monospace\"}}%%\n",
                "wardley-beta\ncomponent API [0.6, 0.7]\n"
            )
            .to_string(),
            "SourceWardleyFont,monospace",
        ),
    ];

    for (diagram_id, engine, source, expected_font) in cases {
        let (_, rendered) = try_render_wardley_with_theme(
            &source,
            &theme,
            engine,
            &portable_wardley_environment(),
            diagram_id,
        )
        .expect("explicit Wardley font ownership remains portable");
        let document =
            roxmltree::Document::parse(rendered.svg()).expect("valid configured Wardley SVG");
        assert_eq!(
            element_with_class(&document, "g", "wardley-map").attribute("font-family"),
            Some(expected_font)
        );
        assert!(!rendered.svg().contains("TypedWardleyFont"));

        drop(document);
        let completion = rendered.into_completion();
        let evidence = merman_render::__private::family_evidence(completion.report());
        assert_eq!(evidence.applied_count(), 0);
        assert_eq!(evidence.not_applicable_count(), 1);
        assert_eq!(evidence.theme_residual_count(), 0);
        assert_eq!(evidence.compatibility_residual_count(), 0);
    }
}

#[test]
fn wardley_font_size_only_keeps_the_baseline_dom_and_mixed_typography_fails_closed() {
    let font_size_theme = wardley_typography_theme(
        ThemeTextStyle::default()
            .with_font_size_px(24.0)
            .expect("valid unsupported Wardley font size"),
    );
    let best_effort = RenderEnvironment::deterministic()
        .with_theme_portability_requirement(ThemePortabilityRequirement::BestEffort);
    let (_, rendered) = try_render_wardley_with_theme(
        "wardley-beta\ncomponent API [0.6, 0.7]\n",
        &font_size_theme,
        Engine::new(),
        &best_effort,
        "wardley-font-size-only",
    )
    .expect("BestEffort renders the unsupported Wardley font-size route");
    let document = roxmltree::Document::parse(rendered.svg()).expect("valid Wardley SVG");
    assert_eq!(
        element_with_class(&document, "g", "wardley-map").attribute("font-family"),
        None,
        "FontSize-only must not activate Wardley's inherited font writer"
    );
    assert!(!rendered.svg().contains("<style>"));
    drop(document);
    let completion = rendered.into_completion();
    let evidence = merman_render::__private::family_evidence(completion.report());
    assert_eq!(evidence.required_count(), 1);
    assert_eq!(evidence.accounted_count(), 1);
    assert_eq!(evidence.applied_count(), 0);
    assert_eq!(evidence.theme_residual_count(), 1);

    let mixed_theme = wardley_typography_theme(
        ThemeTextStyle::default()
            .with_font_stack(
                FontStack::single("MixedWardleyFont").expect("valid mixed Wardley font stack"),
            )
            .with_font_size_px(24.0)
            .expect("valid mixed Wardley font size"),
    );
    let error = try_render_wardley_with_theme(
        "wardley-beta\ncomponent API [0.6, 0.7]\n",
        &mixed_theme,
        Engine::new(),
        &portable_wardley_environment(),
        "wardley-mixed-typography",
    )
    .err()
    .expect("RequirePortable must reject mixed Wardley typography");
    assert_eq!(
        error.unverified_family_theme(),
        Some((DiagramFamilyId::WARDLEY, 1))
    );
    assert_eq!(error.incomplete_family_theme(), None);
}
