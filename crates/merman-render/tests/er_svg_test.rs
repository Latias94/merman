mod common;

use common::legacy_init_theme_compat_engine;
use merman_core::{MermaidEngine, ParseOptions};
use merman_render::LayoutOptions;

use merman_render::environment::{
    MeasurementProfileId, RenderEnvironment, TextMeasurementPolicy, TextMeasurementProfileIdentity,
};
use merman_render::family;
use merman_render::resources::{
    RenderResourcePolicy, ResourceLimitCause, ResourceLimitId, ResourceLimitPhase,
};
use merman_render::svg::{SvgDebugOptions, SvgRenderOptions};
use merman_render::text::{TextMeasurer, TextMetrics, TextStyle};

use regex::Regex;
use serde_json::json;
use std::path::PathBuf;
use std::sync::Arc;

fn workspace_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
}

fn edge_labels_group(svg: &str) -> &str {
    let start = svg
        .find(r#"<g class="edgeLabels">"#)
        .expect("expected edgeLabels group");
    let end = svg[start..]
        .find(r#"<g class="nodes">"#)
        .map(|idx| start + idx)
        .expect("expected nodes group after edgeLabels");
    &svg[start..end]
}

fn render_er_svg_from_text(text: &str, options: &SvgRenderOptions) -> String {
    render_er_svg_from_text_with_engine(text, options, MermaidEngine::new())
}

fn render_er_svg_from_text_with_engine(
    text: &str,
    options: &SvgRenderOptions,
    engine: MermaidEngine,
) -> String {
    let session = merman_render::environment::RenderEnvironment::deterministic()
        .begin_session()
        .unwrap();
    let parsed = engine
        .parse_diagram_for_render_model_sync(text, ParseOptions::default())
        .expect("parse ok")
        .expect("diagram detected")
        .into_presentation();
    let artifact = family::prepare(&parsed, &LayoutOptions::default(), session).expect("layout ok");

    artifact
        .render_svg(options, &SvgDebugOptions::default())
        .expect("render svg")
        .svg()
        .to_owned()
}

fn try_render_er_svg_with_resource_policy(
    text: &str,
    resource_policy: RenderResourcePolicy,
) -> merman_render::Result<String> {
    let session = merman_render::environment::RenderEnvironment::deterministic()
        .with_resource_policy(resource_policy)
        .begin_session()
        .expect("begin ER resource-bound session");
    let parsed = MermaidEngine::new()
        .parse_diagram_for_render_model_sync(text, ParseOptions::default())
        .expect("parse ER resource-bound fixture")
        .expect("detect ER resource-bound fixture")
        .into_presentation();
    let artifact = family::prepare(&parsed, &LayoutOptions::default(), session)?;
    let rendered =
        artifact.render_svg(&SvgRenderOptions::default(), &SvgDebugOptions::default())?;
    Ok(rendered.svg().to_owned())
}

fn er_inline_style_property<'a>(style: &'a str, property: &str) -> Option<&'a str> {
    style
        .split(';')
        .filter_map(|part| part.split_once(':'))
        .find_map(|(name, value)| {
            (name.trim() == property)
                .then(|| value.trim().trim_end_matches("!important").trim_end())
        })
}

#[test]
fn er_family_svg_accepts_exact_max_svg_bytes_and_rejects_one_byte_less() {
    let source = "erDiagram\n  CUSTOMER ||--o{ ORDER : places\n";
    let baseline = try_render_er_svg_with_resource_policy(
        source,
        RenderResourcePolicy::unbounded_for_trusted_input(),
    )
    .expect("render the unbounded ER baseline");
    let exact_bytes = baseline.len();
    assert!(exact_bytes > 1, "ER fixture must emit a non-empty SVG");

    let exact_policy = RenderResourcePolicy::unbounded_for_trusted_input()
        .with_limit(ResourceLimitId::MaxSvgBytes, exact_bytes)
        .expect("valid exact ER SVG byte ceiling");
    let exact = try_render_er_svg_with_resource_policy(source, exact_policy)
        .expect("the exact ER family SVG byte ceiling must succeed");
    assert_eq!(exact.as_bytes(), baseline.as_bytes());

    let below_exact = exact_bytes - 1;
    let below_policy = RenderResourcePolicy::unbounded_for_trusted_input()
        .with_limit(ResourceLimitId::MaxSvgBytes, below_exact)
        .expect("valid below-exact ER SVG byte ceiling");
    let error = try_render_er_svg_with_resource_policy(source, below_policy)
        .expect_err("one byte below the ER family SVG size must fail");
    let merman_render::Error::ResourceLimitExceeded(limit) = error else {
        panic!("expected ER MaxSvgBytes rejection, got {error}");
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

fn entity_transform(svg: &str, entity_id: &str) -> (f64, f64) {
    let re = Regex::new(&format!(
        r#"id="merman-{}"[^>]*transform="translate\(([^,]+), ([^)]+)\)""#,
        regex::escape(entity_id)
    ))
    .expect("entity transform regex");
    let captures = re
        .captures(svg)
        .unwrap_or_else(|| panic!("missing transform for {entity_id}: {svg}"));
    (
        captures[1].parse().expect("entity x"),
        captures[2].parse().expect("entity y"),
    )
}

fn root_view_box(svg: &str) -> [f64; 4] {
    let re = Regex::new(r#"\bviewBox="([^\"]+)""#).expect("viewBox regex");
    let captures = re.captures(svg).expect("root viewBox");
    let values = captures[1]
        .split_ascii_whitespace()
        .map(|value| value.parse::<f64>().expect("viewBox number"))
        .collect::<Vec<_>>();
    values.try_into().expect("four viewBox numbers")
}

fn er_entity_rect_size(svg: &str, entity_id: &str) -> (f64, f64) {
    let document = roxmltree::Document::parse(svg).expect("valid ER SVG");
    let entity = document
        .descendants()
        .find(|node| node.has_tag_name("g") && node.attribute("id") == Some(entity_id))
        .unwrap_or_else(|| panic!("missing ER entity {entity_id}: {svg}"));
    let rect = entity
        .descendants()
        .find(|node| {
            node.has_tag_name("rect")
                && node.attribute("class").is_some_and(|class| {
                    class
                        .split_ascii_whitespace()
                        .any(|token| token == "label-container")
                })
        })
        .unwrap_or_else(|| panic!("missing shell rectangle for ER entity {entity_id}: {svg}"));
    (
        rect.attribute("width")
            .expect("ER entity width")
            .parse()
            .expect("ER entity width number"),
        rect.attribute("height")
            .expect("ER entity height")
            .parse()
            .expect("ER entity height number"),
    )
}

fn er_relationship_label_background_size(svg: &str, label_text: &str) -> (f64, f64) {
    let document = roxmltree::Document::parse(svg).expect("valid ER SVG");
    let label = document
        .descendants()
        .find(|node| {
            node.has_tag_name("g")
                && node.attribute("class") == Some("edgeLabel")
                && node
                    .descendants()
                    .filter_map(|descendant| descendant.text())
                    .any(|text| text == label_text)
        })
        .unwrap_or_else(|| panic!("missing ER relationship label {label_text}: {svg}"));
    let background = label
        .descendants()
        .find(|node| node.has_tag_name("rect") && node.attribute("class") == Some("background"))
        .unwrap_or_else(|| panic!("missing ER relationship background for {label_text}: {svg}"));
    (
        background
            .attribute("width")
            .expect("ER relationship label width")
            .parse()
            .expect("ER relationship label width number"),
        background
            .attribute("height")
            .expect("ER relationship label height")
            .parse()
            .expect("ER relationship label height number"),
    )
}

#[test]
fn er_svg_renders_entities_and_relationships() {
    let path = workspace_root()
        .join("fixtures")
        .join("er")
        .join("upstream_attributes_styles_classes.mmd");
    let text = std::fs::read_to_string(&path).expect("fixture");

    let svg = render_er_svg_from_text(&text, &SvgRenderOptions::default());

    assert!(svg.contains(r#"id="merman-entity-BOOK-0""#));
    assert!(svg.contains(r#"data-look="neo""#));
    assert!(svg.contains(r#"id="merman-id_entity-BOOK-0_entity-PAGE-1_0""#));
    assert!(svg.contains(r#"id="merman-drop-shadow""#));
    assert!(svg.contains("relationshipLine"));
    let edge_style_pattern = if cfg!(feature = "layout-elk") {
        r#"<path[^>]*class="[^"]*relationshipLine[^"]*" style="stroke-dasharray: [^"]*; stroke-dashoffset: 0;fill:none;;;fill:none"[^>]*>"#
    } else {
        r#"<path[^>]*class="[^"]*relationshipLine[^"]*" style="stroke-dasharray: [^"]*; stroke-dashoffset: 0;undefined;;;undefined"[^>]*>"#
    };
    assert!(
        Regex::new(edge_style_pattern)
            .expect("relationship path regex")
            .is_match(&svg),
        "relationship paths should preserve the selected renderer's pathStyle serialization"
    );
    assert!(svg.contains("relationshipLabelBox"));
    assert!(
        svg.contains("marker") && svg.contains("merman_er-zeroOrMoreStart"),
        "expected Mermaid-like marker ids"
    );
    assert!(
        {
            let path_re = Regex::new(r#"<path[^>]*relationshipLine[^>]*>"#).expect("regex");
            let d_re = Regex::new(r#"\bd="([^"]*)"#).expect("regex");
            path_re.find_iter(&svg).any(|m| {
                let d = &d_re.captures(m.as_str()).expect("path data")[1];
                if cfg!(feature = "layout-elk") {
                    d.contains('L') && !d.contains('C')
                } else {
                    d.contains('C')
                }
            })
        },
        "expected ELK rounded segments or Dagre basis curves in relationship paths"
    );
    assert!(
        svg.contains("color: rgb(255, 255, 255) !important;"),
        "expected classDef text color to use the ER HTML label CSSOM path"
    );
}

#[test]
fn er_svg_recursive_relationship_is_one_logical_edge_and_one_label() {
    let path = workspace_root()
        .join("fixtures")
        .join("er")
        .join(
            "upstream_cypress_erdiagram_spec_should_render_an_er_diagram_with_a_recursive_relationship_002.mmd",
        );
    let text = std::fs::read_to_string(path).expect("fixture");

    let svg = render_er_svg_from_text(&text, &SvgRenderOptions::default());

    assert!(svg.contains(r#"data-id="id_entity-CUSTOMER-0_entity-CUSTOMER-0_0""#));
    assert!(!svg.contains("cyclic-special"));
    assert_eq!(
        svg.matches(">refers<").count(),
        1,
        "the logical recursive relationship should own one relationship label: {svg}"
    );
}

#[test]
fn er_svg_uses_configured_look_in_dom_attributes() {
    let text = r#"%%{init: {"look": "neo"}}%%
erDiagram
  CUSTOMER ||--o{ ORDER : places
"#;

    let svg = render_er_svg_from_text_with_engine(
        text,
        &SvgRenderOptions::default(),
        legacy_init_theme_compat_engine(),
    );

    assert!(
        svg.contains(r#"data-look="neo""#),
        "expected ER SVG to propagate configured look: {svg}"
    );
    assert!(
        !svg.contains(r#"data-look="classic""#),
        "configured ER look must not leave classic DOM attributes: {svg}"
    );
}

#[test]
fn er_svg_neo_markers_use_neo_geometry_units_and_theme_values() {
    let text = r##"%%{init: {"look": "neo", "themeVariables": {"mainBkg": "#fedcba", "strokeWidth": 3.5}}}%%
erDiagram
  CUSTOMER ||--o{ ORDER : places
"##;

    let svg = render_er_svg_from_text_with_engine(
        text,
        &SvgRenderOptions::default(),
        legacy_init_theme_compat_engine(),
    );
    let document = roxmltree::Document::parse(&svg).expect("valid Neo ER SVG");

    let zero_or_one_start = document
        .descendants()
        .find(|node| {
            node.has_tag_name("marker") && node.attribute("id") == Some("merman_er-zeroOrOneStart")
        })
        .expect("expected Neo zeroOrOneStart marker");
    assert_eq!(
        zero_or_one_start.attribute("markerUnits"),
        Some("userSpaceOnUse")
    );
    let circle = zero_or_one_start
        .children()
        .find(|node| node.has_tag_name("circle"))
        .expect("expected Neo zeroOrOneStart circle");
    assert_eq!(circle.attribute("fill"), Some("#fedcba"));
    assert_eq!(circle.attribute("cx"), Some("21"));
    assert_eq!(circle.attribute("stroke-width"), Some("3.5"));
    let path = zero_or_one_start
        .children()
        .find(|node| node.has_tag_name("path"))
        .expect("expected Neo zeroOrOneStart path");
    assert_eq!(path.attribute("d"), Some("M9,0 L9,18"));
    assert_eq!(path.attribute("stroke-width"), Some("3.5"));

    let zero_or_more_start = document
        .descendants()
        .find(|node| {
            node.has_tag_name("marker") && node.attribute("id") == Some("merman_er-zeroOrMoreStart")
        })
        .expect("expected Neo zeroOrMoreStart marker");
    assert_eq!(
        zero_or_more_start.attribute("markerUnits"),
        Some("userSpaceOnUse")
    );
    let circle = zero_or_more_start
        .children()
        .find(|node| node.has_tag_name("circle"))
        .expect("expected Neo zeroOrMoreStart circle");
    assert_eq!(circle.attribute("fill"), Some("#fedcba"));
    assert_eq!(circle.attribute("cx"), Some("45.5"));
    assert_eq!(circle.attribute("stroke-width"), Some("3.5"));
}

#[test]
fn er_svg_classic_markers_keep_classic_geometry_and_styling() {
    let text = r##"%%{init: {"look": "classic", "themeVariables": {"mainBkg": "#fedcba", "strokeWidth": 3.5}}}%%
erDiagram
  CUSTOMER ||--o{ ORDER : places
"##;

    let svg = render_er_svg_from_text(text, &SvgRenderOptions::default());
    let document = roxmltree::Document::parse(&svg).expect("valid classic ER SVG");

    let zero_or_one_start = document
        .descendants()
        .find(|node| {
            node.has_tag_name("marker") && node.attribute("id") == Some("merman_er-zeroOrOneStart")
        })
        .expect("expected classic zeroOrOneStart marker");
    assert_eq!(zero_or_one_start.attribute("markerUnits"), None);
    let circle = zero_or_one_start
        .children()
        .find(|node| node.has_tag_name("circle"))
        .expect("expected classic zeroOrOneStart circle");
    assert_eq!(circle.attribute("fill"), Some("white"));
    assert_eq!(circle.attribute("cx"), Some("21"));
    assert_eq!(circle.attribute("stroke-width"), None);
    let path = zero_or_one_start
        .children()
        .find(|node| node.has_tag_name("path"))
        .expect("expected classic zeroOrOneStart path");
    assert_eq!(path.attribute("d"), Some("M9,0 L9,18"));
    assert_eq!(path.attribute("stroke-width"), None);

    let zero_or_more_start = document
        .descendants()
        .find(|node| {
            node.has_tag_name("marker") && node.attribute("id") == Some("merman_er-zeroOrMoreStart")
        })
        .expect("expected classic zeroOrMoreStart marker");
    assert_eq!(zero_or_more_start.attribute("markerUnits"), None);
    let circle = zero_or_more_start
        .children()
        .find(|node| node.has_tag_name("circle"))
        .expect("expected classic zeroOrMoreStart circle");
    assert_eq!(circle.attribute("fill"), Some("white"));
    assert_eq!(circle.attribute("cx"), Some("48"));
    assert_eq!(circle.attribute("stroke-width"), None);
}

#[test]
fn er_svg_renders_diagram_title_and_viewbox_includes_it() {
    let text = r#"---
title: Diagram Title
---
erDiagram
  A ||--o{ B : has
"#;

    let svg = render_er_svg_from_text(text, &SvgRenderOptions::default());

    assert!(svg.contains(r#"class="erDiagramTitleText""#));
    assert!(svg.contains(">Diagram Title<"));
    assert!(svg.contains("viewBox="));
}

#[test]
fn er_svg_title_expands_negative_viewbox_without_rebasing_graph_content() {
    let untitled = r#"erDiagram
  A ||--o{ B : has
"#;
    let titled = r#"---
title: A deliberately wide diagram title
---
erDiagram
  A ||--o{ B : has
"#;

    let untitled_svg = render_er_svg_from_text(untitled, &SvgRenderOptions::default());
    let titled_svg = render_er_svg_from_text(titled, &SvgRenderOptions::default());

    assert_eq!(
        entity_transform(&untitled_svg, "entity-A-0"),
        entity_transform(&titled_svg, "entity-A-0")
    );
    assert!(root_view_box(&titled_svg)[1] < 0.0);
}

#[test]
fn er_svg_forest_theme_renders_root_gradient() {
    let text = r#"---
config:
  theme: forest
---
erDiagram
  A ||--|| B : owns
"#;

    let svg = render_er_svg_from_text(
        text,
        &SvgRenderOptions {
            diagram_id: Some("er_theme_gradient".to_string()),
            ..SvgRenderOptions::default()
        },
    );

    assert!(
        svg.contains(r#"<linearGradient id="er_theme_gradient-gradient" gradientUnits="objectBoundingBox" x1="0%" y1="0%" x2="100%" y2="0%">"#),
        "expected Mermaid 11.15 ER root gradient element: {svg}"
    );
}

#[test]
fn er_svg_relationship_labels_follow_root_htmllabels_not_flowchart_htmllabels() {
    let text = r#"%%{init: {"htmlLabels": true, "flowchart": {"htmlLabels": false}}}%%
erDiagram
  A ||--|| B : owns
"#;

    let svg = render_er_svg_from_text(text, &SvgRenderOptions::default());

    let edge_labels = edge_labels_group(&svg);
    assert!(svg.contains(r#"class="nodeLabel markdown-node-label""#));
    assert!(
        edge_labels.contains(r#"class="labelBkg""#)
            && edge_labels.contains(r#"<foreignObject width=""#),
        "expected ER relationship labels to keep HTML foreignObject output when root htmlLabels=true"
    );
}

#[test]
fn er_svg_relationship_labels_follow_flowchart_htmllabels_when_root_unset() {
    let text = r#"%%{init: {"flowchart": {"htmlLabels": false}}}%%
erDiagram
  A ||--|| B : owns
"#;

    let svg = render_er_svg_from_text(text, &SvgRenderOptions::default());

    let edge_labels = edge_labels_group(&svg);
    assert!(
        edge_labels.contains(r#"<rect class="background""#)
            && edge_labels.contains(">owns</tspan>")
            && edge_labels.contains(r#"text-anchor="middle""#)
            && !edge_labels.contains("<foreignObject"),
        "expected ER relationship labels to switch to SVG text when flowchart htmlLabels=false and root htmlLabels is unset"
    );
}

#[test]
fn er_svg_flowchart_htmllabels_only_changes_relationship_labels() {
    let path = workspace_root()
        .join("fixtures")
        .join("er")
        .join(
            "upstream_cypress_erdiagram_spec_should_render_edge_labels_correctly_when_flowchart_htmllabels_is_019.mmd",
        );
    let text = std::fs::read_to_string(path).expect("ER flowchart htmlLabels fixture");

    let svg = render_er_svg_from_text_with_engine(
        &text,
        &SvgRenderOptions::default(),
        MermaidEngine::new().with_site_config(
            merman_core::presentation::SourcePresentationConfig::from_value(
                json!({"theme":"default", "look":"classic", "layout":"dagre"}),
            ),
        ),
    );
    let document = roxmltree::Document::parse(&svg).expect("valid ER SVG");
    let edge_labels = edge_labels_group(&svg);

    assert!(
        edge_labels.contains(">places</tspan>") && !edge_labels.contains("<foreignObject"),
        "flowchart.htmlLabels=false must switch only relationship labels: {svg}"
    );

    let relationship_labels = document
        .descendants()
        .filter(|node| node.has_tag_name("g") && node.attribute("class") == Some("edgeLabel"))
        .collect::<Vec<_>>();
    assert_eq!(relationship_labels.len(), 5);
    assert!(relationship_labels.iter().all(|label| {
        label.descendants().any(|node| node.has_tag_name("text"))
            && !label
                .descendants()
                .any(|node| node.has_tag_name("foreignObject"))
    }));

    let (label_width, label_height) = er_relationship_label_background_size(&svg, "places");
    assert!(label_width > 0.0);
    assert!(label_height > 0.0);

    let entities = document
        .descendants()
        .filter(|node| {
            node.has_tag_name("g")
                && node
                    .attribute("id")
                    .is_some_and(|id| id.starts_with("merman-entity-"))
        })
        .collect::<Vec<_>>();
    assert_eq!(entities.len(), 5);
    assert!(
        entities.iter().all(|entity| entity
            .descendants()
            .any(|node| node.has_tag_name("foreignObject"))),
        "root htmlLabels unset must keep ER entity labels as foreignObject: {svg}"
    );
    for entity_id in [
        "merman-entity-CUSTOMER-0",
        "merman-entity-ORDER-1",
        "merman-entity-LINE-ITEM-2",
        "merman-entity-ADDRESS-3",
        "merman-entity-INVOICE-4",
    ] {
        let (_, height) = er_entity_rect_size(&svg, entity_id);
        assert_eq!(height, 84.0, "unexpected height for {entity_id}");
    }
}

#[test]
fn er_svg_html_labels_false_uses_svg_text_for_entity_and_attribute_labels() {
    let text = r#"%%{init: {"htmlLabels": false, "er": {"diagramPadding": 20}}}%%
erDiagram
subgraph Orders [Order Domain]
  CUSTOMER {
    string id PK
    string name
  }
end
CUSTOMER ||--|| ORDER : owns
"#;

    let html_text = r#"%%{init: {"htmlLabels": true, "er": {"diagramPadding": 20}}}%%
erDiagram
subgraph Orders [Order Domain]
  CUSTOMER {
    string id PK
    string name
  }
end
CUSTOMER ||--|| ORDER : owns
"#;

    let svg = render_er_svg_from_text_with_engine(
        text,
        &SvgRenderOptions::default(),
        MermaidEngine::new().with_site_config(
            merman_core::presentation::SourcePresentationConfig::from_value(
                json!({"theme":"default", "look":"classic", "layout":"dagre"}),
            ),
        ),
    );
    let html_svg = render_er_svg_from_text_with_engine(
        html_text,
        &SvgRenderOptions::default(),
        MermaidEngine::new().with_site_config(
            merman_core::presentation::SourcePresentationConfig::from_value(
                json!({"theme":"default", "look":"classic", "layout":"dagre"}),
            ),
        ),
    );
    let document = roxmltree::Document::parse(&svg).expect("valid ER SVG");

    assert!(
        !document
            .descendants()
            .any(|node| node.has_tag_name("foreignObject")),
        "htmlLabels=false must not emit foreignObject labels: {svg}"
    );

    let label_text = |class_name: &str| {
        let label = document
            .descendants()
            .find(|node| node.attribute("class") == Some(class_name))
            .unwrap_or_else(|| panic!("missing ER label group {class_name}: {svg}"));
        assert!(
            label.descendants().any(|node| node.has_tag_name("text")),
            "ER label {class_name} must use an SVG text terminal: {svg}"
        );
        label
            .descendants()
            .filter_map(|node| node.text())
            .find(|text| !text.is_empty())
            .unwrap_or_default()
            .to_owned()
    };

    assert_eq!(label_text("label name"), "CUSTOMER");
    assert_eq!(label_text("label attribute-type"), "string");
    assert_eq!(label_text("label attribute-name"), "id");

    let edge_labels = edge_labels_group(&svg);
    assert!(
        edge_labels.contains(">owns</tspan>") && !edge_labels.contains("<foreignObject"),
        "root htmlLabels=false must switch relationship labels to SVG text: {svg}"
    );
    let (label_width, label_height) = er_relationship_label_background_size(&svg, "owns");
    assert!(label_width > 0.0);
    assert!(label_height > 0.0);

    let svg_entity_size = er_entity_rect_size(&svg, "merman-entity-ORDER-1");
    let html_entity_size = er_entity_rect_size(&html_svg, "merman-entity-ORDER-1");
    assert_ne!(
        svg_entity_size, html_entity_size,
        "root htmlLabels=false must feed SVG text metrics into ER entity geometry"
    );
    let style = TextStyle {
        font_size: 16.0,
        ..TextStyle::default()
    };
    let measurer = merman_render::text::DeterministicTextMeasurer::default();
    let svg_ink_height = measurer.measure_svg_tspan_text_bbox_height_px("ORDER", &style);
    let html_line_height = measurer.measure_html_line_height_px("ORDER", &style);
    // erBox's attribute-free drawRect branch adds 1.5 * diagramPadding on each side.
    let vertical_padding = 2.0 * 1.5 * 20.0;
    assert!((svg_entity_size.1 - (svg_ink_height + vertical_padding)).abs() < 1e-9);
    assert!((html_entity_size.1 - (html_line_height + vertical_padding)).abs() < 1e-9);

    let cluster_label = document
        .descendants()
        .find(|node| node.attribute("class") == Some("cluster-label"))
        .expect("missing ER cluster label");
    assert!(
        cluster_label
            .descendants()
            .any(|node| node.has_tag_name("text")),
        "ER cluster labels must use SVG text when htmlLabels=false: {svg}"
    );
    assert!(
        cluster_label
            .descendants()
            .filter_map(|node| node.text())
            .collect::<String>()
            .contains("Order Domain"),
        "ER cluster title must be emitted as SVG text: {svg}"
    );
}

#[test]
fn er_entity_source_depth_and_authored_breaks_reach_public_svg_consumers() {
    use merman_model::presentation::RenderSemanticModel;
    use merman_render::text::DeterministicTextMeasurer;

    let style = TextStyle {
        font_size: 16.0,
        ..TextStyle::default()
    };
    let measurer = DeterministicTextMeasurer::default();
    for html_labels in [false, true] {
        for with_attributes in [false, true] {
            for (label, expected_rows) in [
                ("AXXXXB", &["AXXXXB"][..]),
                ("A&lt;br&gt;B", &["A<br>B"][..]),
                ("A&amp;lt;br&amp;gt;B", &["A&lt;br&gt;B"][..]),
                (
                    "A&amp;amp;lt;br&amp;amp;gt;B",
                    &["A&amp;lt;br&amp;gt;B"][..],
                ),
                ("A#lt;br#gt;B", &["A<br>B"][..]),
                ("book &lt;br&gt; title", &["book <br> title"][..]),
                ("A<br/>B", &["A", "B"][..]),
            ] {
                let entity = if with_attributes {
                    format!("E[\"{label}\"] {{\n  string title PK \"{label}\"\n}}")
                } else {
                    format!("\"{label}\"")
                };
                let source = format!(
                    "---\nconfig:\n  layout: dagre\n  look: classic\n  htmlLabels: {html_labels}\n  flowchart:\n    wrappingWidth: 1000\n  er:\n    diagramPadding: 20\n    entityPadding: 15\n  themeVariables:\n    fontSize: 16px\n---\nerDiagram\n{entity}\n"
                );
                let parsed = MermaidEngine::new()
                    .parse_diagram_for_render_model_sync(&source, ParseOptions::strict())
                    .expect("parse ER entity source depth")
                    .expect("detect ER entity source depth")
                    .into_presentation();
                let RenderSemanticModel::Er(model) = parsed.model() else {
                    panic!("expected typed ER model");
                };
                let entity = model.entities.values().next().expect("parsed ER entity");
                let restored = merman_model::text::restore_mermaid_entity_spelling(label);
                let source_field = if with_attributes {
                    assert_eq!(entity.label, "E");
                    assert_eq!(
                        merman_model::text::restore_mermaid_entity_spelling(
                            &entity.attributes[0].comment
                        ),
                        restored
                    );
                    &entity.alias
                } else {
                    assert!(entity.alias.is_empty());
                    &entity.label
                };
                assert_eq!(
                    merman_model::text::restore_mermaid_entity_spelling(source_field),
                    restored,
                    "the parser must retain entity depth: {label}"
                );
                let artifact = family::prepare(
                    &parsed,
                    &LayoutOptions::default(),
                    RenderEnvironment::deterministic().begin_session().unwrap(),
                )
                .expect("prepare ER entity source depth");
                let layout = artifact.layout_json().expect("ER layout projection");
                let rendered = artifact
                    .render_svg(&SvgRenderOptions::default(), &SvgDebugOptions::default())
                    .expect("render ER entity source depth");
                let document = roxmltree::Document::parse(rendered.svg()).expect("valid ER SVG");
                let classes = if with_attributes {
                    &["label name", "label attribute-comment"][..]
                } else {
                    &["label"][..]
                };
                for class in classes {
                    let terminal = document
                        .descendants()
                        .find(|node| node.attribute("class") == Some(*class))
                        .unwrap_or_else(|| panic!("missing {class}: {label}"));
                    let visible = terminal
                        .descendants()
                        .filter(|node| node.is_text())
                        .filter_map(|node| node.text())
                        .collect::<String>();
                    assert_eq!(visible, expected_rows.concat(), "{class}/{label}");
                    if html_labels {
                        assert_eq!(
                            terminal
                                .descendants()
                                .filter(|node| node.has_tag_name("br"))
                                .count(),
                            expected_rows.len() - 1,
                            "{class}/{label}"
                        );
                    } else {
                        let rows = terminal
                            .descendants()
                            .filter(|node| {
                                node.has_tag_name("tspan")
                                    && node
                                        .parent()
                                        .is_some_and(|parent| parent.has_tag_name("text"))
                            })
                            .map(|row| {
                                row.descendants()
                                    .filter(|node| node.is_text())
                                    .filter_map(|node| node.text())
                                    .collect::<String>()
                            })
                            .collect::<Vec<_>>();
                        assert_eq!(rows, expected_rows, "{class}/{label}");
                    }
                }
                let label_height = if html_labels {
                    expected_rows.len() as f64 * 16.0 * 1.5
                } else {
                    measurer.measure_svg_tspan_text_bbox_height_px("A", &style)
                        + (expected_rows.len() - 1) as f64 * 16.0 * 1.1
                };
                let expected_height = if with_attributes {
                    let padding = if html_labels { 15.0 } else { 15.0 * 1.25 };
                    2.0 * (label_height + padding)
                } else {
                    label_height + 2.0 * 20.0 * 1.5
                };
                let actual_height = layout["layout"]["ErDiagram"]["nodes"][0]["height"]
                    .as_f64()
                    .expect("ER entity layout height");
                assert!(
                    (actual_height - expected_height).abs() < 1e-9,
                    "{html_labels}/{with_attributes}/{label}: {actual_height} != {expected_height}"
                );
            }
        }
    }
}

#[test]
fn er_generic_html_entity_depth_follows_text_content_restoration() {
    for (generic, escaped, expected) in [
        ("A~br~B", "A&lt;br&gt;B", "A<br>B"),
        ("*A~br~B*", "A&lt;br&gt;B", "A<br>B"),
        (
            "A~br~B&amp;lt;br&amp;gt;C",
            "A&lt;br&gt;B&lt;br&gt;C",
            "A<br>B<br>C",
        ),
        (
            "A~br~B&amp;amp;lt;br&amp;amp;gt;C",
            "A&lt;br&gt;B&amp;amp;lt;br&amp;amp;gt;C",
            "A<br>B&amp;lt;br&amp;gt;C",
        ),
    ] {
        for label in [generic, escaped] {
            let source = format!(
                "---\nconfig:\n  layout: dagre\n  look: classic\n  htmlLabels: true\n  themeVariables:\n    fontSize: 16px\n---\nerDiagram\nE[\"{label}\"] {{\n  string id\n}}\n"
            );
            let svg = render_er_svg_from_text(&source, &SvgRenderOptions::default());
            let document = roxmltree::Document::parse(&svg).expect("valid generic ER SVG");
            let terminal = document
                .descendants()
                .find(|node| node.has_tag_name("foreignObject"))
                .expect("HTML entity name terminal");
            let visible = terminal
                .descendants()
                .filter(|node| node.is_text())
                .filter_map(|node| node.text())
                .collect::<String>();
            assert_eq!(visible, expected, "{label}");
            assert!(
                !terminal
                    .descendants()
                    .any(|node| node.has_tag_name("br") || node.has_tag_name("em")),
                "textContent restoration must retain literal text: {label}"
            );
            assert_eq!(terminal.attribute("height"), Some("24"), "{label}");
        }
    }
}

#[test]
fn er_generic_html_literal_height_uses_selected_line_measurement() {
    use merman_render::environment::{
        HostMeasurementResult, HostTextMeasurement, HostTextMeasurementRequest, HostTextMeasurer,
        TextMeasurementOperation, TextMeasurementPhase,
    };
    use std::sync::Mutex;

    #[derive(Default)]
    struct LiteralHeightHost(Mutex<Vec<(TextMeasurementOperation, Option<f64>)>>);

    impl HostTextMeasurer for LiteralHeightHost {
        fn measure(&self, request: HostTextMeasurementRequest<'_>) -> HostMeasurementResult {
            if request.text != "A<br>B" {
                return Ok(None);
            }
            assert_eq!(request.phase, TextMeasurementPhase::Wrap);
            let mut calls = self.0.lock().unwrap();
            match request.operation {
                TextMeasurementOperation::Wrapped => {
                    assert!(
                        !calls.iter().any(|(operation, _)| {
                            *operation == TextMeasurementOperation::HtmlLineHeight
                        }),
                        "createText precedes the final literal textContent measurement"
                    );
                    if calls.is_empty() {
                        assert_eq!(request.max_width, None, "initial nowrap width probe");
                    } else {
                        assert!(
                            request.max_width.is_some_and(|width| width > 0.0),
                            "the final table measurement consumes the root-probe budget"
                        );
                    }
                    calls.push((request.operation, request.max_width));
                    Ok(Some(HostTextMeasurement::Metrics(TextMetrics {
                        width: 64.0,
                        height: 99.0,
                        line_count: 1,
                    })))
                }
                TextMeasurementOperation::HtmlLineHeight => {
                    assert_eq!(request.max_width, None);
                    assert!(
                        calls.iter().any(|(operation, max_width)| {
                            *operation == TextMeasurementOperation::Wrapped && max_width.is_some()
                        }),
                        "literal line height must follow createText"
                    );
                    calls.push((request.operation, request.max_width));
                    Ok(Some(HostTextMeasurement::Length(31.25)))
                }
                _ => Ok(None),
            }
        }
    }

    let source = r#"%%{init: {"htmlLabels": true, "layout": "dagre"}}%%
erDiagram
  A["A~br~B"] {
    string id
  }
"#;
    let host = Arc::new(LiteralHeightHost::default());
    let identity = TextMeasurementProfileIdentity::new(
        MeasurementProfileId::new("test.er-html-line-height").unwrap(),
        "1",
    )
    .unwrap();
    let session = RenderEnvironment::deterministic()
        .with_text_measurement_policy(TextMeasurementPolicy::host_display(
            identity,
            host.clone(),
            [TextMeasurementPhase::Wrap],
        ))
        .begin_session()
        .unwrap();
    let parsed = MermaidEngine::new()
        .parse_diagram_for_render_model_sync(source, ParseOptions::default())
        .expect("parse generic ER entity")
        .expect("detect ER diagram")
        .into_presentation();
    let artifact =
        family::prepare(&parsed, &LayoutOptions::default(), session).expect("prepare ER labels");
    let prepared_calls = host.0.lock().unwrap().clone();
    assert_eq!(
        prepared_calls.len(),
        3,
        "initial nowrap and bounded createText requests precede the final literal line height"
    );
    assert_eq!(prepared_calls[0], (TextMeasurementOperation::Wrapped, None));
    assert_eq!(prepared_calls[1].0, TextMeasurementOperation::Wrapped);
    assert!(prepared_calls[1].1.is_some_and(|width| width > 0.0));
    assert_eq!(
        prepared_calls[2],
        (TextMeasurementOperation::HtmlLineHeight, None)
    );
    let rendered = artifact
        .render_svg(&SvgRenderOptions::default(), &SvgDebugOptions::default())
        .expect("render generic ER entity");
    let document = roxmltree::Document::parse(rendered.svg()).expect("valid ER SVG");
    let terminal = document
        .descendants()
        .find(|node| {
            node.has_tag_name("foreignObject")
                && node
                    .descendants()
                    .filter(|node| node.is_text())
                    .filter_map(|node| node.text())
                    .collect::<String>()
                    == "A<br>B"
        })
        .expect("literal generic HTML terminal");
    assert!(!terminal.descendants().any(|node| {
        node.has_tag_name("br") || node.has_tag_name("span") || node.has_tag_name("p")
    }));
    assert_eq!(terminal.attribute("height"), Some("31.25"));
    assert_eq!(
        *host.0.lock().unwrap(),
        prepared_calls,
        "emission must consume the prepared terminal without replaying createText"
    );
}

#[test]
fn er_svg_emits_empty_and_whitespace_relationship_label_wrappers() {
    let body = r#"erDiagram
BOOK }|..|{ AUTHOR : ""
BOOK }|..|{ GENRE : " "
AUTHOR }|..|{ GENRE : "  "
"#;
    let providers = [
        (
            "default",
            None,
            if cfg!(feature = "layout-elk") { 2 } else { 3 },
        ),
        ("dagre", Some("dagre"), 3),
        #[cfg(feature = "layout-elk")]
        ("elk", Some("elk"), 2),
    ];
    for (provider, layout, expected_wrappers) in providers {
        let config = layout.map_or_else(|| json!({}), |layout| json!({"layout": layout}));
        let source = format!("---\nconfig: {config}\n---\n{body}");
        let svg = render_er_svg_from_text(&source, &SvgRenderOptions::default());
        let labels = edge_labels_group(&svg);
        assert_eq!(
            labels.matches(r#"<g class="edgeLabel""#).count(),
            expected_wrappers,
            "{provider}"
        );
        assert!(!labels.contains("undefined"), "{provider}");
        assert!(!labels.contains("NaN"), "{provider}");
        assert_eq!(
            labels.matches(r#"width="0" height="0""#).count(),
            expected_wrappers,
            "{provider}"
        );
    }
}

#[test]
fn er_svg_row_fills_follow_optional_theme_colors() {
    let row_re = Regex::new(r#"<g[^>]*class="row-rect-(?:odd|even)"[^>]*>(.*?)</g>"#).unwrap();
    for (variables, expected_fills) in [
        (serde_json::json!({}), vec![]),
        (
            serde_json::json!({"rowOdd": "#123456", "rowEven": "#abcdef"}),
            vec!["#123456", "#abcdef"],
        ),
        (serde_json::json!({"rowOdd": "none", "rowEven": ""}), vec![]),
    ] {
        let config = serde_json::json!({"theme": "redux", "themeVariables": variables});
        let text = format!(
            "%%{{init: {config}}}%%\nerDiagram\n BOOK {{\n string title\n int pages\n }}\n"
        );
        let svg = render_er_svg_from_text(&text, &SvgRenderOptions::default());
        let rows = row_re.captures_iter(&svg).collect::<Vec<_>>();
        assert_eq!(rows.len(), 2);
        for (index, row) in rows.iter().enumerate() {
            assert_eq!(
                row[1].matches("<path ").count(),
                if expected_fills.is_empty() { 1 } else { 2 }
            );
            if let Some(fill) = expected_fills.get(index) {
                assert!(row[1].contains(&format!(r#"fill="{fill}""#)));
            }
        }
    }
}

#[test]
fn er_redux_explicit_styles_override_palette_on_every_table_path() {
    for theme in ["redux", "redux-dark", "redux-color", "redux-dark-color"] {
        for look in ["classic", "neo"] {
            for html_labels in [false, true] {
                let source = format!(
                    r#"---
config:
  theme: {theme}
  look: {look}
  layout: dagre
  htmlLabels: {html_labels}
---
erDiagram
  BOOK["Book"]:::core {{
    string title PK
    string author FK
  }}
  classDef core fill:#f96,stroke:#456,stroke-width:3px,color:#fff
  class BOOK core
  style BOOK fill:#f9f,stroke:#333,stroke-width:2px
"#
                );
                let svg = render_er_svg_from_text(&source, &SvgRenderOptions::default());
                let document = roxmltree::Document::parse(&svg).unwrap();
                let entity = document
                    .descendants()
                    .find(|node| node.attribute("id") == Some("merman-entity-BOOK-0"))
                    .expect("BOOK entity");
                for path in entity
                    .descendants()
                    .filter(|node| node.has_tag_name("path"))
                {
                    let style = path.attribute("style").unwrap_or_default();
                    assert!(
                        style.contains("fill:#f9f !important"),
                        "{theme}/{look}: {style}"
                    );
                    assert!(
                        style.contains("stroke:#333 !important"),
                        "{theme}/{look}: {style}"
                    );
                    assert!(
                        style.contains("stroke-width:2px !important"),
                        "{theme}/{look}: {style}"
                    );
                    assert!(
                        !style.contains("#f96"),
                        "class fill must lose to explicit style"
                    );
                }
            }
        }
    }
}

#[test]
fn er_non_redux_preserves_row_colors_and_styles_both_divider_paths() {
    let source = r#"---
config:
  theme: default
  look: neo
  layout: dagre
---
erDiagram
  BOOK {
    string title PK
    string author FK
  }
  style BOOK fill:#f9f,stroke:#333,stroke-width:2px,stroke-dasharray:4 2
"#;
    let svg = render_er_svg_from_text(source, &SvgRenderOptions::default());
    let document = roxmltree::Document::parse(&svg).unwrap();
    for group in document.descendants().filter(|node| {
        matches!(
            node.attribute("class"),
            Some("outer-path" | "row-rect-odd" | "row-rect-even" | "divider")
        )
    }) {
        for path in group.children().filter(|node| node.has_tag_name("path")) {
            let style = path.attribute("style").unwrap_or_default();
            assert!(
                style.contains("stroke:#333 !important"),
                "{}: {style}",
                group.attribute("class").unwrap()
            );
            // Mermaid erDiagram.jison skips whitespace in style blocks and joins tokens.
            assert!(style.contains("stroke-dasharray:42 !important"), "{style}");
            assert_eq!(
                style.contains("fill:#f9f !important"),
                group.attribute("class") == Some("row-rect-even")
            );
        }
    }
}

#[cfg(feature = "layout-elk")]
#[test]
fn er_elk_paints_ancestors_before_descendants_in_stable_model_order() {
    let source = r#"---
config:
  layout: elk
---
erDiagram
subgraph ZOuter [Outer]
  subgraph AInner [Inner]
    subgraph Deepest [Deep]
      A
    end
    B
  end
  subgraph ZSibling [Sibling]
    C
  end
end
subgraph BPeer [Peer]
  D
end
A ||--|| C : crosses_siblings
B ||--|| D : crosses_roots
"#;
    let svg = render_er_svg_from_text(source, &SvgRenderOptions::default());
    let document = roxmltree::Document::parse(&svg).expect("valid SVG");
    let clusters = document
        .descendants()
        .find(|node| node.attribute("class") == Some("clusters"))
        .expect("clusters group");
    let ids: Vec<_> = clusters
        .children()
        .filter_map(|node| node.attribute("id"))
        .collect();
    // ER builds its model from reverse subgraph completion order. Stable depth ordering
    // preserves that sibling order while moving every parent before its descendants.
    assert_eq!(
        ids,
        [
            "merman-BPeer",
            "merman-ZOuter",
            "merman-ZSibling",
            "merman-AInner",
            "merman-Deepest"
        ]
    );
    let last_cluster = svg.find(r#"id="merman-Deepest""#).unwrap();
    let leaf_nodes = svg.find(r#"<g class="nodes">"#).unwrap();
    assert!(last_cluster < leaf_nodes);
    assert_eq!(
        document
            .descendants()
            .filter(|node| node.attribute("data-edge") == Some("true"))
            .count(),
        2
    );
    assert!(svg.contains("crosses_siblings") && svg.contains("crosses_roots"));
}

#[test]
fn er_svg_label_mode_covers_containers_entities_and_attribute_cells() {
    let body = r#"erDiagram
subgraph ZOuter [Outer Namespace]
  subgraph AInner [Inner Namespace]
    BOOK["Book **Title**"] {
      string name PK "visible comment"
      int edition FK
      type~T~ generic UK "generic value"
    }
    AUTHOR["Author Name"]
  end
end
BOOK ||--|| AUTHOR : writes
style ZOuter color:#123456
"#;
    let layouts = if cfg!(feature = "layout-elk") {
        vec!["dagre", "elk"]
    } else {
        vec!["dagre"]
    };
    for layout in layouts {
        for html_labels in [false, true] {
            let source = format!(
                "---\nconfig:\n  layout: {layout}\n  htmlLabels: {html_labels}\n  themeVariables:\n    titleColor: '#abcdef'\n---\n{body}"
            );
            let svg = render_er_svg_from_text(&source, &SvgRenderOptions::default());
            let document = roxmltree::Document::parse(&svg).expect("valid SVG");
            let foreign_objects = document
                .descendants()
                .filter(|node| node.has_tag_name("foreignObject"))
                .count();
            assert_eq!(
                foreign_objects > 0,
                html_labels,
                "{layout}/{html_labels}: {svg}"
            );
            for id in [
                "merman-ZOuter",
                "merman-AInner",
                "merman-entity-BOOK-0",
                "merman-entity-AUTHOR-1",
            ] {
                let owner = document
                    .descendants()
                    .find(|node| node.attribute("id") == Some(id))
                    .expect("label owner");
                assert_eq!(
                    owner
                        .descendants()
                        .any(|node| node.has_tag_name("foreignObject")),
                    html_labels,
                    "{id}: {svg}"
                );
                assert_eq!(
                    owner.descendants().any(|node| node.has_tag_name("text")),
                    !html_labels,
                    "{id}: {svg}"
                );
            }
            let visible: String = document
                .descendants()
                .filter(|node| node.is_text())
                .filter_map(|node| node.text())
                .collect();
            for text in [
                "Outer Namespace",
                "Inner Namespace",
                "Book Title",
                "Author Name",
                "string",
                "name",
                "PK",
                "visible comment",
                "int",
                "edition",
                "FK",
                "type<T>",
                "generic value",
                "writes",
            ] {
                assert!(
                    visible.contains(text),
                    "missing {text:?} in {layout}/{html_labels}: {visible}"
                );
            }
            let outer = document
                .descendants()
                .find(|node| node.attribute("id") == Some("merman-ZOuter"))
                .expect("outer group");
            let title = outer
                .descendants()
                .find(|node| node.has_tag_name(if html_labels { "span" } else { "text" }))
                .expect("styled title");
            if html_labels {
                assert_eq!(
                    er_inline_style_property(title.attribute("style").unwrap_or_default(), "color"),
                    Some("#123456"),
                    "{layout}: HTML title retains its source color"
                );
            } else {
                assert_eq!(title.attribute("style").unwrap_or_default(), "");
                let label = outer
                    .children()
                    .find(|node| node.attribute("class") == Some("cluster-label"))
                    .expect("outer SVG cluster label");
                assert_eq!(label.attribute("style").unwrap_or_default(), "");
                let css = document
                    .descendants()
                    .find(|node| node.has_tag_name("style"))
                    .and_then(|node| node.text())
                    .expect("ER stylesheet");
                assert!(
                    css.contains("#merman .cluster-label text{fill:#abcdef;}"),
                    "{layout}: SVG title follows its configured theme color"
                );
            }
            for class in [
                "label name",
                "label attribute-type",
                "label attribute-name",
                "label attribute-keys",
                "label attribute-comment",
            ] {
                let label = document
                    .descendants()
                    .find(|node| node.attribute("class") == Some(class))
                    .expect("attribute label");
                assert!(
                    label
                        .descendants()
                        .any(|node| node.has_tag_name(if html_labels {
                            "foreignObject"
                        } else {
                            "text"
                        })),
                    "{class}: {svg}"
                );
            }
        }
    }
}

#[cfg(feature = "layout-elk")]
#[test]
fn er_elk_line_hops_change_only_crossing_paths() {
    use std::collections::BTreeMap;
    let body = r#"erDiagram
A ||--|| X : links
A ||--|| Y : links
A ||--|| Z : links
B ||--|| X : links
B ||--|| Y : links
B ||--|| Z : links
C ||--|| X : links
C ||--|| Y : links
C ||--|| Z : links
"#;
    let render = |mode: &str| {
        render_er_svg_from_text(
            &format!("---\nconfig:\n  layout: elk\n  elk:\n    lineHops: {mode}\n---\n{body}"),
            &SvgRenderOptions::default(),
        )
    };
    let without = render("false");
    let arcs = render("true");
    let gaps = render("gap");
    let paths = |svg: &str| {
        let document = roxmltree::Document::parse(svg).expect("valid SVG");
        document
            .descendants()
            .filter(|node| node.attribute("data-edge") == Some("true"))
            .map(|node| {
                (
                    node.attribute("id").unwrap().to_string(),
                    (
                        node.attribute("d").unwrap().to_string(),
                        node.attribute("marker-start").unwrap().to_string(),
                        node.attribute("marker-end").unwrap().to_string(),
                        node.attribute("data-points").unwrap().to_string(),
                    ),
                )
            })
            .collect::<BTreeMap<_, _>>()
    };
    let plain_paths = paths(&without);
    let arc_paths = paths(&arcs);
    let gap_paths = paths(&gaps);
    assert_eq!(plain_paths.len(), 9);
    let mut changed = 0;
    let mut unchanged = 0;
    for (id, plain) in &plain_paths {
        let arc = &arc_paths[id];
        let gap = &gap_paths[id];
        assert_eq!((&plain.1, &plain.2, &plain.3), (&arc.1, &arc.2, &arc.3));
        assert_eq!((&plain.1, &plain.2, &plain.3), (&gap.1, &gap.2, &gap.3));
        if plain.0 != arc.0 {
            changed += 1;
            assert_ne!(plain.0, gap.0);
            assert_ne!(arc.0, gap.0);
            assert!(arc.0.contains('A'), "expected jump arc for {id}: {}", arc.0);
            assert!(
                gap.0.matches('M').count() > 1,
                "expected a discontinuity for {id}: {}",
                gap.0
            );
        } else {
            unchanged += 1;
            assert_eq!(plain.0, gap.0);
        }
    }
    assert!(changed > 0 && unchanged > 0);
    assert_eq!(edge_labels_group(&without), edge_labels_group(&arcs));
    assert_eq!(edge_labels_group(&without), edge_labels_group(&gaps));
}

#[cfg(feature = "layout-elk")]
#[test]
fn er_elk_neo_hops_update_solid_and_dashed_masks_from_final_path_length() {
    use kurbo::Shape;
    use std::collections::BTreeMap;

    for dashed in [false, true] {
        let mut body = String::from("erDiagram\n");
        let relationship = if dashed { "||..||" } else { "||--||" };
        for source in ["A", "B", "C"] {
            for target in ["X", "Y", "Z"] {
                body.push_str(&format!("{source} {relationship} {target} : links\n"));
            }
        }
        let render = |mode: &str| {
            render_er_svg_from_text(
                &format!(
                    "---\nconfig:\n  layout: elk\n  look: neo\n  elk:\n    lineHops: {mode}\n---\n{body}"
                ),
                &SvgRenderOptions::default(),
            )
        };
        let without = render("false");
        let original = roxmltree::Document::parse(&without).expect("original SVG");
        let original_edges: BTreeMap<_, _> = original
            .descendants()
            .filter(|node| node.attribute("data-edge") == Some("true"))
            .map(|node| (node.attribute("id").unwrap(), node))
            .collect();

        for mode in ["true", "gap"] {
            let svg = render(mode);
            let document = roxmltree::Document::parse(&svg).expect("hopped SVG");
            let mut changed = 0;
            let mut unchanged = 0;
            for edge in document
                .descendants()
                .filter(|node| node.attribute("data-edge") == Some("true"))
            {
                let id = edge.attribute("id").unwrap();
                let before = original_edges[id];
                let d = edge.attribute("d").unwrap();
                let style = edge.attribute("style").unwrap();
                for attribute in ["marker-start", "marker-end", "data-points"] {
                    assert_eq!(
                        before.attribute(attribute),
                        edge.attribute(attribute),
                        "{id}/{mode}/{attribute}"
                    );
                }
                if d == before.attribute("d").unwrap() {
                    unchanged += 1;
                    assert_eq!(style, before.attribute("style").unwrap(), "{id}/{mode}");
                    continue;
                }
                changed += 1;
                // getTotalLength() returns an SVG DOM float before marker-offset arithmetic.
                let length =
                    f64::from(kurbo::BezPath::from_svg(d).unwrap().perimeter(1.0e-6) as f32);
                let dasharray: Vec<f64> = style
                    .split(';')
                    .find_map(|declaration| declaration.trim().strip_prefix("stroke-dasharray:"))
                    .expect("Neo mask")
                    .split_whitespace()
                    .map(|value| value.parse().expect("dash length"))
                    .collect();
                // lineJump.ts reads offsets from the first four original dash values.
                // For a repeated Neo 2/2 pattern the fourth value is 2, not its final 0.
                let end_offset = if dashed { 2.0 } else { 0.0 };
                assert_eq!(dasharray.len(), 4, "{id}/{mode}: {style}");
                assert_eq!(
                    (dasharray[0], dasharray[1], dasharray[3]),
                    (0.0, 0.0, end_offset)
                );
                assert!(
                    (dasharray[2] - (length - end_offset).max(0.0)).abs() < 0.001,
                    "{id}/{mode}: length={length}, style={style}"
                );
                assert!(style.contains("stroke-dashoffset: 0;"));
                assert!(
                    !style.contains(";;"),
                    "upstream collapses empty declarations: {style}"
                );
            }
            assert!(changed > 0 && unchanged > 0, "dashed={dashed}/{mode}");
            assert_eq!(edge_labels_group(&without), edge_labels_group(&svg));
        }
    }
}

#[test]
fn er_svg_nested_relationships_and_empty_groups_keep_dom_order() {
    let source = concat!(
        "erDiagram\n",
        "subgraph Outer [Outer Domain]\n",
        "PRE\n",
        "subgraph Inner [Inner Domain]\n",
        "A ||--|| B : inner_relation\n",
        "end\n",
        "PRE ||--|| C : outer_relation\n",
        "subgraph Empty [Empty Domain]\nend\n",
        "end\n",
        "B ||--|| D : cross_relation\n",
    );
    let svg = render_er_svg_from_text(source, &SvgRenderOptions::default());
    let document = roxmltree::Document::parse(&svg).expect("valid SVG");
    for id in ["merman-Outer", "merman-Inner", "merman-Empty"] {
        assert!(
            document
                .descendants()
                .any(|node| node.attribute("id") == Some(id)),
            "missing {id}"
        );
    }
    let nodes = document
        .descendants()
        .find(|node| node.attribute("class") == Some("nodes"))
        .expect("entity nodes");
    let ids = nodes
        .children()
        .filter_map(|node| node.attribute("id"))
        .collect::<Vec<_>>();
    assert_eq!(
        ids,
        [
            "merman-entity-PRE-0",
            "merman-entity-A-1",
            "merman-entity-B-2",
            "merman-entity-C-3",
            "merman-entity-D-4",
        ]
    );
    let labels = edge_labels_group(&svg);
    let inner = labels.find("inner_relation").unwrap();
    let outer = labels.find("outer_relation").unwrap();
    let cross = labels.find("cross_relation").unwrap();
    assert!(inner < outer && outer < cross);
    assert_eq!(
        document
            .descendants()
            .filter(|node| node.attribute("data-edge") == Some("true"))
            .count(),
        3
    );
}

#[test]
fn er_svg_source_color_is_lowered_for_entity_and_attribute_text() {
    for source_style in [
        "style CUSTOMER color:#123456",
        "classDef accent color:#123456\nclass CUSTOMER accent",
    ] {
        let source = format!(
            "---\nconfig:\n  htmlLabels: false\n  layout: dagre\n---\nerDiagram\n CUSTOMER {{\n string id PK \"key\"\n }}\n{source_style}\n"
        );
        let svg = render_er_svg_from_text(&source, &SvgRenderOptions::default());
        let document = roxmltree::Document::parse(&svg).unwrap();
        for class in [
            "label name",
            "label attribute-type",
            "label attribute-name",
            "label attribute-keys",
            "label attribute-comment",
        ] {
            let labels: Vec<_> = document
                .descendants()
                .filter(|n| {
                    n.attribute("class") == Some(class)
                        && n.descendants().any(|n| n.has_tag_name("text"))
                })
                .collect();
            assert!(!labels.is_empty(), "missing {class}");
            assert!(
                labels.iter().any(|n| n
                    .attribute("style")
                    .unwrap_or("")
                    .contains("fill:#123456 !important")),
                "{class}: {svg}"
            );
        }
    }
}
