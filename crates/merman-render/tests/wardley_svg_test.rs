use std::sync::Arc;

use merman_core::{MermaidEngine, ParseOptions};
use merman_render::DiagramFamilyId;
use merman_render::LayoutOptions;
use merman_render::environment::{
    MeasurementProfileId, RenderEnvironment, RenderSession, TextMeasurementPolicy,
    TextMeasurementProfile, TextMeasurementProfileIdentity,
};
use merman_render::family;
use merman_render::resources::{
    RenderResourcePolicy, ResourceLimitCause, ResourceLimitId, ResourceLimitPhase,
};
use merman_render::svg::{SvgDebugOptions, SvgRenderOptions};
use merman_render::text::{TextMeasurer, TextMetrics, TextStyle};
use merman_render::theme::{Color, NativeTheme, ThemeCompiler};
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
    let session = RenderEnvironment::deterministic()
        .begin_session()
        .expect("Wardley render session starts");
    render_wardley_with_session(source, site_config, diagram_id, session).1
}

fn render_wardley_with_session(
    source: &str,
    site_config: Value,
    diagram_id: &str,
    session: RenderSession,
) -> (Value, String) {
    let engine = MermaidEngine::new().with_site_config(
        merman_core::presentation::SourcePresentationConfig::from_value(site_config),
    );
    let parsed = engine
        .parse_diagram_for_render_model_sync(source, ParseOptions::strict())
        .expect("Wardley parse succeeds")
        .expect("Wardley diagram is detected")
        .into_presentation();
    let artifact = family::prepare(&parsed, &LayoutOptions::headless_svg_defaults(), session)
        .expect("Wardley family prepares");
    assert_eq!(artifact.family_id(), DiagramFamilyId::WARDLEY);

    let projection = artifact.layout_json().expect("serialize Wardley layout");
    let layout = projection["layout"]["WardleyDiagram"].clone();
    let rendered = artifact
        .render_svg(
            &SvgRenderOptions {
                diagram_id: Some(diagram_id.to_string()),
                ..Default::default()
            },
            &SvgDebugOptions::default(),
        )
        .expect("Wardley SVG renders");
    (layout, rendered.svg().to_owned())
}

fn try_render_wardley_with_resource_policy(
    source: &str,
    site_config: Value,
    diagram_id: &str,
    resource_policy: RenderResourcePolicy,
) -> merman_render::Result<String> {
    let engine = MermaidEngine::new().with_site_config(
        merman_core::presentation::SourcePresentationConfig::from_value(site_config),
    );
    let parsed = engine
        .parse_diagram_for_render_model_sync(source, ParseOptions::strict())
        .expect("Wardley parse succeeds")
        .expect("Wardley diagram is detected")
        .into_presentation();
    let session = RenderEnvironment::deterministic()
        .with_resource_policy(resource_policy)
        .begin_session()
        .expect("Wardley render session starts");
    let artifact = family::prepare(&parsed, &LayoutOptions::headless_svg_defaults(), session)?;
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
fn wardley_native_roles_and_gridlines_reach_svg() {
    let theme = ThemeCompiler::new()
        .compile_json(
            br##"{
              "schema_version": 2,
              "extends": "blueprint",
              "roles": {
                "canvas": "#F8FAFC", "surface.node": "#EEF1F2", "surface.callout": "#FEF5DB",
                "stroke.border": "#223344", "stroke.guide": "#526474", "stroke.connector": "#36485A",
                "text.title": "#19364D", "text.callout": "#48300B"
              },
              "appearances": {"dark": {"data": {"ink": ["#2C4C74", "#934531", "#346052", "#454D85"]}}},
              "typography": {
                "sans": ["Map Sans", "sans-serif"], "body_family": "sans", "base_size_px": 20, "label_weight": 550
              },
              "form": {"connector_width": 2.5, "chart": {"gridlines": true}}
            }"##,
            None,
        )
        .unwrap();
    let environment = RenderEnvironment::deterministic();
    let (_, svg) = render_wardley_with_session(
        WARDLEY_COMPLETE_FEATURE_SOURCE,
        serde_json::json!({}),
        "wardley-native",
        environment
            .begin_session_with_native_theme(NativeTheme::new(theme.clone()))
            .unwrap(),
    );
    let doc = roxmltree::Document::parse(&svg).unwrap();
    let canvas = doc.root_element().first_element_child().unwrap();
    assert_eq!(canvas.attribute("class"), Some("merman-theme-canvas"));
    assert_eq!(
        canvas.first_element_child().unwrap().attribute("fill"),
        Some("#F8FAFC")
    );
    assert!(
        canvas
            .descendants()
            .any(|node| node.has_tag_name("pattern"))
    );
    assert!(!svg.contains("wardley-background"));

    let map = element_with_class(&doc, "g", "wardley-map");
    assert_eq!(
        map.attribute("font-family"),
        Some("\"Map Sans\", sans-serif")
    );
    let grid = element_with_class(&doc, "g", "wardley-grid");
    assert_eq!(
        grid.first_element_child().unwrap().attribute("stroke"),
        Some("#526474")
    );
    let component = doc
        .descendants()
        .find(|node| {
            node.has_tag_name("circle")
                && node.attribute("class").is_none()
                && node
                    .parent()
                    .is_some_and(|parent| has_class(parent, "wardley-node"))
        })
        .unwrap();
    assert_eq!(component.attribute("fill"), Some("#EEF1F2"));
    assert_eq!(component.attribute("stroke"), Some("#223344"));
    let link = element_with_class(&doc, "line", "wardley-link");
    assert_eq!(link.attribute("stroke"), Some("#36485A"));
    assert_eq!(link.attribute("stroke-width"), Some("2.5"));
    let trend = element_with_class(&doc, "line", "wardley-trend");
    assert_eq!(trend.attribute("stroke"), Some("#19364D"));
    assert_eq!(
        element_with_class(&doc, "circle", "wardley-build-overlay").attribute("fill"),
        Some("#2C4C74")
    );
    let annotation = element_with_class(&doc, "g", "wardley-annotations-box");
    assert_eq!(
        annotation.first_element_child().unwrap().attribute("fill"),
        Some("#FEF5DB")
    );
    let text = annotation
        .children()
        .find(|node| node.has_tag_name("text"))
        .unwrap();
    assert_eq!(text.attribute("fill"), Some("#48300B"));
    assert_eq!(text.attribute("font-size"), Some("20"));
    assert_eq!(text.attribute("font-weight"), Some("550"));

    let source = format!(
        "---\nconfig: {{wardley-beta: {{showGrid: false}}, themeVariables: {{wardley: {{linkStroke: '#B04225', componentFill: '#FFECCD'}}}}}}\n---\n{WARDLEY_COMPLETE_FEATURE_SOURCE}"
    );
    let (_, svg) = render_wardley_with_session(
        &source,
        serde_json::json!({}),
        "wardley-source",
        environment
            .begin_session_with_native_theme(NativeTheme::new(theme))
            .unwrap(),
    );
    assert!(!svg.contains("wardley-grid"));
    let doc = roxmltree::Document::parse(&svg).unwrap();
    assert_eq!(
        element_with_class(&doc, "line", "wardley-link").attribute("stroke"),
        Some("#B04225")
    );
    assert!(
        doc.descendants()
            .any(|node| node.has_tag_name("circle") && node.attribute("fill") == Some("#FFECCD"))
    );
}

#[test]
fn wardley_host_canvas_is_visible_in_mermaid_mode() {
    let config = serde_json::json!({"themeVariables": {"wardley": {"backgroundColor": "#BBAACC"}}});
    let classic = render_wardley(
        WARDLEY_COMPLETE_FEATURE_SOURCE,
        config.clone(),
        "wardley-classic",
    );
    let classic = roxmltree::Document::parse(&classic).unwrap();
    assert_eq!(
        element_with_class(&classic, "rect", "wardley-background").attribute("fill"),
        Some("#BBAACC")
    );

    for (background, expected) in [
        (Color::rgb(19, 37, 61), Some("#13253D")),
        (Color::TRANSPARENT, None),
    ] {
        let session = RenderEnvironment::deterministic()
            .begin_session()
            .unwrap()
            .with_canvas_background(background);
        let (_, svg) = render_wardley_with_session(
            WARDLEY_COMPLETE_FEATURE_SOURCE,
            config.clone(),
            "wardley-background",
            session,
        );
        let doc = roxmltree::Document::parse(&svg).unwrap();
        assert!(
            !doc.descendants()
                .any(|node| has_class(node, "wardley-background"))
        );
        let canvas = element_with_class(&doc, "g", "merman-theme-canvas");
        assert_eq!(
            canvas
                .first_element_child()
                .and_then(|node| node.attribute("fill")),
            expected
        );
    }
}

const WARDLEY_ANCHOR_SOURCE: &str = concat!(
    "wardley-beta\n",
    "anchor Business [0.95, 0.80]\n",
    "anchor Public [0.90, 0.20]\n",
    "component API [0.65, 0.35]\n",
    "evolve API 0.80\n",
);

#[test]
fn wardley_source_font_and_anchor_colors_override_native_roles() {
    let theme = ThemeCompiler::new()
        .compile_json(br#"{"schema_version":2,"extends":"blueprint"}"#, None)
        .unwrap();
    let session = RenderEnvironment::deterministic()
        .begin_session_with_native_theme(NativeTheme::new(theme))
        .unwrap();
    let source = format!(
        "---\nconfig:\n  themeVariables:\n    wardley:\n      fontFamily: 'MapSourceFont, monospace'\n      anchorLabelColor: '#aabbcc'\n      componentLabelColor: '#112233'\n      evolutionStroke: '#445566'\n---\n{WARDLEY_ANCHOR_SOURCE}"
    );
    let (_, svg) = render_wardley_with_session(
        &source,
        serde_json::json!({}),
        "wardley-explicit-anchors",
        session,
    );
    let document = roxmltree::Document::parse(&svg).unwrap();
    assert_eq!(
        element_with_class(&document, "g", "wardley-map").attribute("font-family"),
        Some("MapSourceFont,monospace")
    );
    for (class, expected) in [
        ("wardley-node--anchor", "#aabbcc"),
        ("wardley-node--component", "#112233"),
    ] {
        let group = element_with_class(&document, "g", class);
        assert_eq!(
            group
                .children()
                .find(|node| node.has_tag_name("text"))
                .unwrap()
                .attribute("fill"),
            Some(expected),
        );
    }
    assert_eq!(
        element_with_class(&document, "line", "wardley-trend").attribute("stroke"),
        Some("#445566"),
    );
}

#[test]
fn wardley_without_host_theme_retains_upstream_anchor_black() {
    let svg = render_wardley(
        WARDLEY_ANCHOR_SOURCE,
        serde_json::json!({}),
        "wardley-plain-anchors",
    );
    let document = roxmltree::Document::parse(&svg).unwrap();
    let anchor = element_with_class(&document, "g", "wardley-node--anchor");
    assert_eq!(
        anchor
            .children()
            .find(|node| node.has_tag_name("text"))
            .unwrap()
            .attribute("fill"),
        Some("#000"),
    );
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
struct WardleyFontProbeMeasurer {
    expected_font_family: &'static str,
    expected_font_size: f64,
}

impl WardleyFontProbeMeasurer {
    fn assert_style(&self, style: &TextStyle) {
        assert_eq!(
            style.font_family.as_deref(),
            Some(self.expected_font_family),
            "Wardley annotation measurement must use the inherited writer's resolved winner"
        );
        assert_eq!(style.font_size, self.expected_font_size);
    }
}

impl TextMeasurer for WardleyFontProbeMeasurer {
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
fn wardley_native_font_drives_annotation_measurement_and_svg_inheritance() {
    let theme = ThemeCompiler::new()
        .compile_json(
            br#"{
                "schema_version": 2,
                "extends": "blueprint",
                "typography": {"sans": ["Wardley Native", "monospace"], "body_family": "sans", "base_size_px": 20}
            }"#,
            None,
        )
        .unwrap();
    let expected_font = r#""Wardley Native", monospace"#;
    let environment = RenderEnvironment::deterministic().with_text_measurement_policy(
        TextMeasurementPolicy::uniform(TextMeasurementProfile::new(
            TextMeasurementProfileIdentity::new(
                MeasurementProfileId::new("test.wardley-native-font").unwrap(),
                "test",
            )
            .unwrap(),
            Arc::new(WardleyFontProbeMeasurer {
                expected_font_family: expected_font,
                expected_font_size: 20.0,
            }),
        )),
    );
    let (layout, svg) = render_wardley_with_session(
        "wardley-beta\nannotations [0.1, 0.2]\nannotation 1,[0.7, 0.6] \"Font winner\"\n",
        serde_json::json!({}),
        "wardley-native-font",
        environment
            .begin_session_with_native_theme(NativeTheme::new(theme))
            .unwrap(),
    );
    assert_eq!(layout["annotations_box"]["max_text_width"], 123.0);
    assert_eq!(layout["annotations_box"]["max_text_height"], 17.0);
    let document = roxmltree::Document::parse(&svg).unwrap();
    let map = element_with_class(&document, "g", "wardley-map");
    assert_eq!(map.attribute("font-family"), Some(expected_font));
    let annotations = element_with_class(&document, "g", "wardley-annotations-box");
    let label = annotations
        .children()
        .find(|node| node.has_tag_name("text"))
        .unwrap();
    assert_eq!(label.attribute("font-size"), Some("20"));
    assert_eq!(label.attribute("font-family"), None);
}

#[test]
fn wardley_explicit_root_font_reaches_unthemed_svg_and_annotation_measurement() {
    let source = "wardley-beta\nannotations [0.1, 0.2]\nannotation 1,[0.7, 0.6] \"Font winner\"\n";
    let environment = RenderEnvironment::deterministic().with_text_measurement_policy(
        TextMeasurementPolicy::uniform(TextMeasurementProfile::new(
            TextMeasurementProfileIdentity::new(
                MeasurementProfileId::new("test.wardley-explicit-root-font").unwrap(),
                "test",
            )
            .unwrap(),
            Arc::new(WardleyFontProbeMeasurer {
                expected_font_family: "Cousine,monospace",
                expected_font_size: 11.0,
            }),
        )),
    );
    for invalid_family_font in ["", "serif,,monospace", "24px", "serif !important"] {
        let config = serde_json::json!({"themeVariables": {
            "fontFamily": "Cousine, monospace",
            "wardley": {"fontFamily": invalid_family_font}
        }});
        let (layout, svg) = render_wardley_with_session(
            source,
            config,
            "wardley-explicit-root-font",
            environment.begin_session().unwrap(),
        );
        assert_eq!(layout["annotations_box"]["max_text_width"], 123.0);
        assert_eq!(layout["annotations_box"]["max_text_height"], 17.0);
        let document = roxmltree::Document::parse(&svg).unwrap();
        assert_eq!(
            element_with_class(&document, "g", "wardley-map").attribute("font-family"),
            Some("Cousine,monospace"),
            "family font {invalid_family_font:?}"
        );
    }
}

#[test]
fn wardley_unknown_role_sizes_preserve_attributes() {
    let source = "wardley-beta\ncomponent API [0.7, 0.4]\n";
    for (size, expected) in [
        (serde_json::json!(-1), "-1"),
        (
            serde_json::json!("var(--wardley-size)"),
            "var(--wardley-size)",
        ),
        (serde_json::json!("invalid"), "invalid"),
        (serde_json::json!("0px !important"), "0px !important"),
    ] {
        let svg = render_wardley(
            source,
            serde_json::json!({"wardley-beta": {"labelFontSize": size}}),
            "wardley-unknown-text",
        );
        let document = roxmltree::Document::parse(&svg).unwrap();
        assert_eq!(
            element_with_class(&document, "text", "wardley-node-label").attribute("font-size"),
            Some(expected)
        );
    }
}

#[test]
fn wardley_role_sizes_keep_source_priority_units_arithmetic_and_fixed_annotations() {
    let diagram = "wardley-beta\ntitle Role sizes\ncomponent API [0.7, 0.4]\nannotations [0.1, 0.2]\nannotation 1,[0.7, 0.4] \"Fixed annotation\"\nnote \"Fixed note\" [0.4, 0.6]\naccelerator \"Faster\" [0.2, 0.8]\n";
    for (size, title, axis, stage, label, used_px) in [
        (
            serde_json::json!(24),
            "25.200000000000003",
            "24",
            "22",
            "24",
            24.0,
        ),
        (
            serde_json::json!("24pt"),
            "NaN",
            "24pt",
            "NaN",
            "24pt",
            32.0,
        ),
    ] {
        for root in ["0px", "32px"] {
            let config = serde_json::json!({
                "themeVariables": {"fontSize": root},
                "wardley-beta": {"axisFontSize": size, "labelFontSize": size}
            });
            let source = format!("---\nconfig: {config}\n---\n{diagram}");
            let (layout, svg) = render_wardley_with_session(
                &source,
                serde_json::json!({
                    "wardley-beta": {"axisFontSize": 18, "labelFontSize": 16}
                }),
                "wardley-role-sizes",
                RenderEnvironment::deterministic().begin_session().unwrap(),
            );
            let document = roxmltree::Document::parse(&svg).unwrap();
            for (class, expected) in [
                ("wardley-title", title),
                ("wardley-axis-label", axis),
                ("wardley-stage-label", stage),
                ("wardley-node-label", label),
            ] {
                assert_eq!(
                    element_with_class(&document, "text", class).attribute("font-size"),
                    Some(expected)
                );
            }
            assert_eq!(layout["axes"]["x_label"]["font_size"], used_px);
            assert_eq!(layout["nodes"][0]["label_layout"]["font_size"], used_px);
            for (class, expected) in [
                ("wardley-annotation", "10"),
                ("wardley-annotations-box", "11"),
                ("wardley-notes", "11"),
                ("wardley-accelerators", "10"),
            ] {
                let group = element_with_class(&document, "g", class);
                let label = group
                    .children()
                    .find(|node| node.has_tag_name("text"))
                    .unwrap();
                assert_eq!(label.attribute("font-size"), Some(expected));
            }
        }
    }
}
