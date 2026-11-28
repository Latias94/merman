#![cfg(feature = "diagram-requirement")]

use merman_core::{MermaidEngine, ParseOptions};
use merman_model::presentation::PresentationDiagram;
use merman_render::{environment::RenderEnvironment, family};

// Mermaid 12.4.0 e2e/diagrams/grid/nightly-check/17-requirement-diagram-using-automatic-placement.mmd.
fn parse(html_labels: bool) -> PresentationDiagram {
    let source = format!(
        r#"---
config:
  layout: grid
  htmlLabels: {html_labels}
  look: classic
  flowchart:
    htmlLabels: {}
  grid:
    columnGap: 200
---
requirementDiagram
  requirement checkout {{
    id: 1
    text: "Customers can complete an order"
    risk: high
    verifymethod: test
  }}
  element storefront {{
    type: service
    docref: "checkout"
  }}
  storefront - satisfies -> checkout
"#,
        !html_labels
    );
    MermaidEngine::new()
        .parse_diagram_for_render_model_sync(&source, ParseOptions::strict())
        .unwrap()
        .unwrap()
        .into_presentation()
}

#[cfg(feature = "layout-grid")]
#[test]
fn requirement_grid_paints_the_measured_label_rect_on_the_route() {
    use merman_render::{
        DiagramFamilyId, LayoutOptions, RenderCapability, model::RequirementDiagramLayout,
        svg::SvgRenderOptions,
    };

    for html_labels in [false, true] {
        let parsed = parse(html_labels);
        let session = RenderEnvironment::deterministic().begin_session().unwrap();
        let plan = family::plan_render(&parsed, &session).unwrap();
        assert_eq!(plan.family_id(), DiagramFamilyId::REQUIREMENT);
        assert_eq!(plan.required_capabilities(), [RenderCapability::LayoutGrid]);
        let artifact = family::prepare(&parsed, &LayoutOptions::default(), session).unwrap();
        let geometry = artifact.layout_geometry_json().unwrap();
        assert_eq!(geometry["family_id"], "requirement");
        let layout: RequirementDiagramLayout =
            serde_json::from_value(geometry["layout"]["RequirementDiagram"].clone()).unwrap();
        let [left, right] = &layout.nodes[..] else {
            panic!("expected two Requirement nodes")
        };
        let edge = &layout.edges[0];
        let label = edge.label.as_ref().unwrap();
        let start = &edge.points[0];
        let end = edge.points.last().unwrap();
        assert_eq!((&edge.from, &edge.to), (&right.id, &left.id));
        assert!(left.x < label.x && label.x < right.x);
        assert_eq!(start.y, end.y);
        assert_eq!(label.y, start.y);
        assert!((start.x - end.x - 200.0).abs() < 0.001);

        let svg = artifact
            .render_svg(
                &SvgRenderOptions {
                    diagram_id: Some("requirement-grid".to_owned()),
                    ..Default::default()
                },
                &Default::default(),
            )
            .unwrap();
        let doc = roxmltree::Document::parse(svg.svg()).unwrap();
        let root_group = doc
            .descendants()
            .find(|node| node.attribute("class") == Some("root"))
            .unwrap();
        let layers: Vec<_> = root_group
            .children()
            .filter(|node| node.is_element())
            .collect();
        let classes: Vec<_> = layers.iter().map(|node| node.attribute("class")).collect();
        assert_eq!(
            classes,
            [
                Some("clusters"),
                Some("edges edgePaths"),
                Some("edgeLabels"),
                Some("nodes")
            ]
        );
        assert!(!layers[2].children().any(|node| node.is_element()));
        let shaft = layers[1]
            .children()
            .find(|node| node.has_tag_name("path"))
            .unwrap();
        assert_eq!(shaft.attribute("data-id"), Some("storefront-checkout-0"));
        assert_eq!(
            shaft.attribute("marker-end"),
            Some("url(#requirement-grid_requirement-requirement_arrowEnd)")
        );
        let label = layers[3]
            .children()
            .find(|node| {
                node.attribute("id") == Some("edge-label-storefront-checkout-storefront-checkout-0")
            })
            .unwrap();
        assert_eq!(label.attribute("class"), Some("label edgeLabel"));
        assert_eq!(
            label
                .descendants()
                .any(|node| node.has_tag_name("foreignObject")),
            html_labels
        );
        assert!(
            label
                .descendants()
                .any(|node| node.text() == Some("<<satisfies>>"))
        );
    }
}

#[cfg(not(feature = "layout-grid"))]
#[test]
fn explicit_requirement_grid_requires_a_compiled_provider() {
    let parsed = parse(false);
    let session = RenderEnvironment::deterministic().begin_session().unwrap();
    assert!(matches!(
        family::plan_render(&parsed, &session),
        Err(merman_render::Error::UnsupportedLayoutBackend { backend: "grid", diagram_type })
            if diagram_type == "requirement"
    ));
}
