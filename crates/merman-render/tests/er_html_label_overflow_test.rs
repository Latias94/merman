#![cfg(feature = "diagram-er")]

use merman_core::{MermaidEngine, ParseOptions};
use merman_render::environment::RenderEnvironment;
use merman_render::svg::SvgPipeline;
use merman_render::{LayoutOptions, family};
use roxmltree::{Document, Node};

fn viewport_text(viewport: Node<'_, '_>) -> String {
    viewport
        .descendants()
        .filter(Node::is_text)
        .filter_map(|node| node.text())
        .collect()
}

fn assert_html_label_overflow(backend: &str, overflow_visible: bool) {
    let source = format!(
        r#"---
config:
  layout: {backend}
  look: classic
  htmlLabels: true
---
erDiagram
  subgraph Catalog[Catalog domain]
    subgraph Orders[Order domain]
      ORDER {{
        uuid customer_account_id PK "Account identifier"
        string note
      }}
      DELIVERY_ADDRESS
    end
  end
  ORDER ||--o{{ DELIVERY_ADDRESS : ships_to
"#
    );
    let diagram = MermaidEngine::new()
        .parse_diagram_for_render_model_sync(&source, ParseOptions::strict())
        .unwrap()
        .unwrap()
        .into_presentation();
    let session = RenderEnvironment::deterministic().begin_session().unwrap();
    let artifact = family::prepare(&diagram, &LayoutOptions::default(), session).unwrap();
    let rendered = artifact
        .render_svg(&Default::default(), &Default::default())
        .unwrap();
    let svg = Document::parse(rendered.svg()).unwrap();
    let mut labels = Vec::new();
    let mut empty_viewports = 0;
    for viewport in svg
        .descendants()
        .filter(|n| n.has_tag_name("foreignObject"))
    {
        let label = viewport_text(viewport);
        let width: f64 = viewport.attribute("width").unwrap().parse().unwrap();
        let height: f64 = viewport.attribute("height").unwrap().parse().unwrap();
        if label.is_empty() {
            assert_eq!((width, height), (0.0, 0.0), "{backend}");
            assert_eq!(viewport.attribute("style"), None, "{backend}");
            empty_viewports += 1;
        } else {
            assert!(width > 0.0 && height > 0.0, "{backend}: {label}");
            assert_eq!(
                viewport.attribute("style"),
                overflow_visible.then_some("overflow: visible;"),
                "{backend}: {label}",
            );
            labels.push(label);
        }
    }
    assert_eq!(empty_viewports, 2, "{backend}");
    labels.sort_unstable();
    assert_eq!(
        labels,
        [
            "Account identifier",
            "Catalog domain",
            "DELIVERY_ADDRESS",
            "ORDER",
            "Order domain",
            "PK",
            "customer_account_id",
            "note",
            "ships_to",
            "string",
            "uuid",
        ],
        "{backend}",
    );

    let finalized = rendered.finalize_resvg(&SvgPipeline::resvg_safe()).unwrap();
    let svg = Document::parse(finalized.svg().as_str()).unwrap();
    assert!(!svg.descendants().any(|n| n.has_tag_name("foreignObject")));
    let expected_overflow = if overflow_visible {
        "visible"
    } else {
        "hidden"
    };
    let mut fallback_labels = Vec::new();
    for viewport in svg.descendants().filter(|n| n.has_tag_name("svg")).skip(1) {
        let label = viewport_text(viewport);
        let overflow = viewport
            .attribute("style")
            .unwrap()
            .split(';')
            .filter_map(|declaration| declaration.split_once(':'))
            .find_map(|(property, value)| (property.trim() == "overflow").then(|| value.trim()));
        assert_eq!(overflow, Some(expected_overflow), "{backend}: {label}");
        fallback_labels.push(label);
    }
    fallback_labels.sort_unstable();
    assert_eq!(fallback_labels, labels, "{backend}");
}

#[cfg(feature = "layout-shoal")]
#[test]
fn er_shoal_allows_html_label_overflow() {
    assert_html_label_overflow("shoal", true);
}

#[test]
fn er_swimlane_allows_html_label_overflow() {
    assert_html_label_overflow("swimlane", true);
}

#[cfg(feature = "layout-dagre")]
#[test]
fn er_dagre_preserves_html_label_viewports() {
    assert_html_label_overflow("dagre", false);
}

#[cfg(feature = "layout-elk")]
#[test]
fn er_elk_preserves_html_label_viewports() {
    assert_html_label_overflow("elk", false);
}
