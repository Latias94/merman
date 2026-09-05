use merman_core::{Engine, ParseOptions};
use merman_render::LayoutOptions;
use merman_render::family;
use merman_render::svg::{SvgDebugOptions, SvgRenderOptions};
use regex::Regex;
use std::path::PathBuf;

fn workspace_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
}

fn edge_labels_group(svg: &str) -> &str {
    let edge_labels = Regex::new(r#"<g\b[^>]*class="[^"]*edgeLabels[^"]*"[^>]*>"#)
        .expect("edgeLabels regex")
        .find(svg)
        .expect("expected edgeLabels group");
    let nodes = Regex::new(r#"<g\b[^>]*class="[^"]*nodes[^"]*"[^>]*>"#)
        .expect("nodes regex")
        .find_at(svg, edge_labels.end())
        .expect("expected nodes group after edgeLabels");
    &svg[edge_labels.start()..nodes.start()]
}

fn render_er_svg_from_text(text: &str, options: &SvgRenderOptions) -> String {
    let session = merman_render::environment::RenderEnvironment::deterministic()
        .begin_session()
        .unwrap();
    let parsed = Engine::new()
        .parse_diagram_for_render_model_sync(text, ParseOptions::default())
        .expect("parse ok")
        .expect("diagram detected");
    let artifact = family::prepare(parsed, &LayoutOptions::default(), session).expect("layout ok");

    artifact
        .render_svg(options, &SvgDebugOptions::default())
        .expect("render svg")
        .svg()
        .to_owned()
}

fn entity_box_position(svg: &str, entity_id: &str) -> (f64, f64) {
    let re = Regex::new(&format!(
        r#"(?s)<g id="merman-{}"[^>]*>.*?<rect x="([^"]+)" y="([^"]+)"[^>]*data-merman-resource="er\.entity\.\d+\.box""#,
        regex::escape(entity_id)
    ))
    .expect("entity box regex");
    let captures = re
        .captures(svg)
        .unwrap_or_else(|| panic!("missing box for {entity_id}: {svg}"));
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

#[test]
fn er_svg_renders_entities_and_relationships() {
    let path = workspace_root()
        .join("fixtures")
        .join("er")
        .join("upstream_attributes_styles_classes.mmd");
    let text = std::fs::read_to_string(&path).expect("fixture");

    let svg = render_er_svg_from_text(&text, &SvgRenderOptions::default());

    assert!(svg.contains(r#"id="merman-entity-BOOK-0""#));
    assert!(svg.contains(r#"data-look="classic""#));
    assert!(svg.contains(r#"id="merman-id_entity-BOOK-0_entity-PAGE-1_0""#));
    assert!(svg.contains(r#"id="merman-drop-shadow""#));
    assert!(svg.contains("relationshipLine"));
    assert!(
        Regex::new(
            r#"<path[^>]*class="[^"]*relationshipLine[^"]*" style="undefined;;;undefined"[^>]*>"#
        )
        .expect("relationship path regex")
        .is_match(&svg),
        "relationship paths should preserve Mermaid's exact empty pathStyle serialization"
    );
    assert!(svg.contains("relationshipLabelBox"));
    assert!(
        svg.contains("marker") && svg.contains("merman_er-zeroOrMoreStart"),
        "expected Mermaid-like marker ids"
    );
    assert!(
        {
            let path_re = Regex::new(r#"<path[^>]*relationshipLine[^>]*>"#).expect("regex");
            let d_re = Regex::new(r#"\bd="[^"]*C"#).expect("regex");
            path_re.find_iter(&svg).any(|m| d_re.is_match(m.as_str()))
        },
        "expected curveBasis cubic bezier commands in relationship paths"
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
        svg.matches(r#"data-merman-semantic-id="er.edge.0.label""#)
            .count(),
        1,
        "the logical recursive relationship should own one label semantic: {svg}"
    );
    assert!(
        edge_labels_group(&svg).contains(r#"<span class="edgeLabel">refers</span>"#),
        "the relationship label semantic should expose one visible label"
    );
}

#[test]
fn er_svg_uses_configured_look_in_dom_attributes() {
    let text = r#"%%{init: {"look": "neo"}}%%
erDiagram
  CUSTOMER ||--o{ ORDER : places
"#;

    let svg = render_er_svg_from_text(text, &SvgRenderOptions::default());

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
        entity_box_position(&untitled_svg, "entity-A-0"),
        entity_box_position(&titled_svg, "entity-A-0")
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
    assert!(
        svg.contains(r#"class="nodeLabel markdown-node-label""#),
        "expected ER entity labels to keep the HTML label class when root htmlLabels=true: {svg}"
    );
    assert!(
        edge_labels.contains(r#"class="labelBkg""#)
            && edge_labels.contains("<foreignObject")
            && edge_labels.contains(r#"class="edgeLabel""#),
        "expected ER relationship labels to keep HTML foreignObject output when root htmlLabels=true: {edge_labels}"
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
    let background_re =
        Regex::new(r#"<rect\b[^>]*class="background"[^>]*>"#).expect("background regex");
    assert!(
        background_re.is_match(edge_labels)
            && edge_labels.contains(">owns</text>")
            && edge_labels.contains(r#"text-anchor="middle""#)
            && !edge_labels.contains("<foreignObject"),
        "expected ER relationship labels to switch to SVG text when flowchart htmlLabels=false and root htmlLabels is unset: {edge_labels}"
    );
}

#[test]
fn er_svg_entity_labels_keep_mermaid_foreignobject_shell_when_disabled() {
    let text = r#"%%{init: {"htmlLabels": false}}%%
erDiagram
  A {
    string id
  }
  A ||--|| B : owns
"#;

    let svg = render_er_svg_from_text(text, &SvgRenderOptions::default());

    assert!(svg.contains("<foreignObject"));
    assert!(
        svg.contains(r#"class="nodeLabel markdown-node-label""#),
        "expected ER entity labels to retain Mermaid's foreignObject label shell: {svg}"
    );
}
