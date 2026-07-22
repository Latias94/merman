use merman_core::{Engine, MermaidConfig, ParseOptions};
use merman_render::LayoutOptions;
use merman_render::environment::RenderEnvironment;
use merman_render::family::{self, RenderFamilyKind};
use merman_render::svg::{SvgDebugOptions, SvgRenderOptions};
use serde_json::Value;

fn render_wardley(source: &str, site_config: Value, diagram_id: &str) -> String {
    let engine = Engine::new().with_site_config(MermaidConfig::from_value(site_config));
    let parsed = engine
        .parse_diagram_for_render_model_sync(source, ParseOptions::strict())
        .expect("Wardley parse succeeds")
        .expect("Wardley diagram is detected");
    let session = RenderEnvironment::deterministic().begin_session().unwrap();
    let artifact = family::prepare(parsed, &LayoutOptions::headless_svg_defaults(), session)
        .expect("Wardley layout succeeds");
    assert_eq!(artifact.family_kind(), RenderFamilyKind::Wardley);

    artifact
        .render_svg(
            &SvgRenderOptions {
                diagram_id: Some(diagram_id.to_string()),
                ..Default::default()
            },
            &SvgDebugOptions::default(),
        )
        .expect("Wardley SVG renders")
        .svg()
        .to_owned()
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
    let source = r#"wardley-beta
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
    let svg = render_wardley(
        source,
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
    assert!(!svg.contains("<style>"));
}

#[test]
fn wardley_svg_projects_upstream_annotation_computed_styles_into_attributes() {
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
        Some("#414243")
    );
    assert_eq!(
        annotation_group
            .children()
            .find(|node| node.has_tag_name("circle"))
            .and_then(|node| node.attribute("stroke")),
        Some("#212223")
    );
    assert_eq!(
        annotation_group
            .children()
            .find(|node| node.has_tag_name("text"))
            .and_then(|node| node.attribute("fill")),
        Some("#313233")
    );
    let box_group = element_with_class(&document, "g", "wardley-annotations-box");
    assert_eq!(
        box_group
            .children()
            .find(|node| node.has_tag_name("rect"))
            .and_then(|node| node.attribute("fill")),
        Some("#414243")
    );
    assert_eq!(
        box_group
            .children()
            .find(|node| node.has_tag_name("rect"))
            .and_then(|node| node.attribute("stroke")),
        Some("#212223")
    );
    assert_eq!(
        box_group
            .children()
            .find(|node| node.has_tag_name("text"))
            .and_then(|node| node.attribute("fill")),
        Some("#313233")
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
