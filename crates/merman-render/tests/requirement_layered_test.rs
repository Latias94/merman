#![cfg(all(feature = "diagram-requirement", feature = "layout-layered"))]

use base64::Engine as _;
use merman_core::{MermaidEngine, ParseOptions};
use merman_render::{
    DiagramFamilyId, LayoutOptions, RenderCapability, environment::RenderEnvironment, family,
};

#[test]
fn requirement_layered_keeps_relationships_and_their_label_layer_in_source_order() {
    // The Requirement 12.4.0 Layered fixture uses the common renderer and mixed relations.
    let source = r#"---
config:
  layout: layered
  look: classic
  htmlLabels: false
  layered:
    edgeRouting: straight
    lineHops: false
---
requirementDiagram
  requirement login {
    id: 1
    text: Users can sign in.
  }
  functionalRequirement reset {
    id: 2
    text: Users can reset a password.
  }
  element auth_service {
    type: service
  }
  login - contains -> reset
  auth_service - satisfies -> login
  reset - derives -> login
"#;
    let parsed = MermaidEngine::new()
        .parse_diagram_for_render_model_sync(source, ParseOptions::strict())
        .unwrap()
        .unwrap()
        .into_presentation();
    let session = RenderEnvironment::deterministic().begin_session().unwrap();
    let plan = family::plan_render(&parsed, &session).unwrap();
    assert_eq!(plan.family_id(), DiagramFamilyId::REQUIREMENT);
    assert_eq!(
        plan.required_capabilities(),
        [RenderCapability::LayoutLayered]
    );
    let artifact = family::prepare(&parsed, &LayoutOptions::default(), session).unwrap();
    let layout = artifact.layout_geometry_json().unwrap();
    assert_eq!(
        layout["layout"]["RequirementDiagram"]["edges"]
            .as_array()
            .unwrap()
            .len(),
        3
    );
    let svg = artifact
        .render_svg(&Default::default(), &Default::default())
        .unwrap();
    let doc = roxmltree::Document::parse(svg.svg()).unwrap();
    assert_eq!(doc.root_element().attribute("data-layout-fallback"), None);
    let edges = doc
        .descendants()
        .find(|node| node.attribute("class") == Some("edges edgePaths"))
        .unwrap();
    let paths: Vec<_> = edges
        .children()
        .filter(|node| node.has_tag_name("path"))
        .collect();
    assert_eq!(
        paths
            .iter()
            .map(|path| path.attribute("data-id"))
            .collect::<Vec<_>>(),
        [
            Some("login-reset-0"),
            Some("auth_service-login-1"),
            Some("reset-login-2"),
        ]
    );
    assert!(
        paths[0]
            .attribute("marker-start")
            .unwrap()
            .contains("requirement_containsStart")
    );
    assert!(
        paths[1]
            .attribute("marker-end")
            .unwrap()
            .contains("requirement_arrowEnd")
    );
    for path in paths {
        let bytes = base64::engine::general_purpose::STANDARD
            .decode(path.attribute("data-points").unwrap())
            .unwrap();
        let points: Vec<serde_json::Value> = serde_json::from_slice(&bytes).unwrap();
        assert!(points.len() >= 2);
        assert!(
            points
                .iter()
                .all(|point| matches!(point["axis"].as_str(), Some("h" | "v")))
        );
        assert!(!path.attribute("d").unwrap().contains(['A', 'C', 'Q']));
    }
    let labels = doc
        .descendants()
        .find(|node| node.attribute("class") == Some("edgeLabels"))
        .unwrap();
    for relationship in ["<<contains>>", "<<satisfies>>", "<<derives>>"] {
        assert!(
            labels
                .descendants()
                .any(|node| node.text() == Some(relationship))
        );
    }
}
