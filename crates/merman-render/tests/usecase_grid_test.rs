#![cfg(feature = "diagram-usecase")]

use merman_core::{MermaidEngine, ParseOptions};
use merman_model::presentation::PresentationDiagram;
use merman_render::{environment::RenderEnvironment, family};

// Mermaid 12.4.0 e2e/diagrams/grid/pr-check/20-usecase-works.mmd.
const PINNED: &str = r#"---
description:
  A use case diagram with an actor placed beside a column of use cases.
config:
  layout: grid
  grid:
    placements:
      Customer: { row: 2, column: 1 }
      Browse: { row: 1, column: 2 }
      Checkout: { row: 2, column: 2 }
      Track: { row: 3, column: 2 }
---
usecase-beta
  actor Customer
  Browse("Browse products")
  Checkout("Complete checkout")
  Track("Track order")

  Customer --> Browse
  Customer --> Checkout
  Customer --> Track
"#;

fn parse(source: &str) -> PresentationDiagram {
    MermaidEngine::new()
        .parse_diagram_for_render_model_sync(source, ParseOptions::strict())
        .unwrap()
        .unwrap()
        .into_presentation()
}

#[cfg(not(feature = "layout-grid"))]
#[test]
fn explicit_usecase_grid_requires_a_compiled_provider() {
    let parsed = parse(PINNED);
    let session = RenderEnvironment::deterministic().begin_session().unwrap();
    assert!(matches!(
        family::plan_render(&parsed, &session),
        Err(merman_render::Error::UnsupportedLayoutBackend { backend: "grid", diagram_type })
            if diagram_type == "usecase"
    ));
}

#[cfg(feature = "layout-grid")]
mod grid {
    use super::*;
    use merman_render::{
        DiagramFamilyId, Error, LayoutOptions, RenderCapability, RenderCapabilityPolicy,
        family::FamilyRenderArtifact, svg::SvgRenderOptions, usecase::UsecaseDiagramLayout,
    };

    const COMPOUND: &str = r#"---
config:
  layout: grid
  look: classic
  htmlLabels: false
  flowchart:
    wrappingWidth: 86
  grid:
    curve: linear
    lineHops: false
    placements:
      User: { row: 2, column: -1 }
      Auth: { row: 2, column: 1 }
      Login: { row: 1, column: 1 }
      Verify: { row: 1, column: 2 }
      note-0: { row: 3, column: 1 }
      Data: { row: 3, column: -1 }
      Empty: { row: 1, column: 1 }
      "Customer label": { row: -8, column: 8 }
      usecase-User: { row: -8, column: 9 }
---
usecase-beta
direction LR
systemBoundary Auth(Authentication)@{type:package}
  Login("Sign & go")@{business:true}
  Verify[Check]
end
actor User("Customer label")@{type:hollow} <<Human>>
systemBoundary Empty[Empty system]
end
User review@-- "**literal** account review" --> Login
Login ..> : include Verify
Verify ..> : extend Login
note for Login "Remember"
json Data@{"z":1,"a":2}
Data -- Login
"#;

    fn layout(artifact: &FamilyRenderArtifact<'_>) -> UsecaseDiagramLayout {
        let geometry = artifact.layout_geometry_json().unwrap();
        assert_eq!(geometry["family_id"], "usecase");
        assert_eq!(
            geometry["layout"],
            artifact.layout_json().unwrap()["layout"]
        );
        serde_json::from_value(geometry["layout"]["UsecaseDiagram"].clone()).unwrap()
    }

    fn svg(artifact: FamilyRenderArtifact<'_>) -> String {
        artifact
            .render_svg(
                &SvgRenderOptions {
                    diagram_id: Some("usecase-grid".into()),
                    ..Default::default()
                },
                &Default::default(),
            )
            .unwrap()
            .svg()
            .to_owned()
    }

    #[test]
    fn pinned_usecase_uses_authored_grid_columns_and_provider_admission() {
        let parsed = parse(PINNED);
        let environment = RenderEnvironment::deterministic();
        let session = environment.begin_session().unwrap();
        let plan = family::plan_render(&parsed, &session).unwrap();
        assert_eq!(plan.family_id(), DiagramFamilyId::USECASE);
        assert_eq!(plan.required_capabilities(), [RenderCapability::LayoutGrid]);
        let artifact = family::prepare(&parsed, &LayoutOptions::default(), session).unwrap();
        let layout = layout(&artifact);
        let [actor, browse, checkout, track] = layout.nodes.as_slice() else {
            panic!("four nodes")
        };
        assert_eq!(
            [&actor.id, &browse.id, &checkout.id, &track.id],
            ["Customer", "Browse", "Checkout", "Track"]
        );
        assert!(actor.x < browse.x);
        assert_eq!(browse.x, checkout.x);
        assert_eq!(checkout.x, track.x);
        assert_eq!(actor.y, checkout.y);
        assert!(browse.y < checkout.y && checkout.y < track.y);
        assert_eq!(layout.edges.len(), 3);
        for edge in &layout.edges {
            assert!(edge.label.is_none() && edge.points.len() >= 2);
            assert!(
                edge.points
                    .windows(2)
                    .all(|pair| pair[0].x == pair[1].x || pair[0].y == pair[1].y)
            );
        }
        let denied = environment
            .with_capability_policy(
                RenderCapabilityPolicy::unrestricted().with_denied(RenderCapability::LayoutGrid),
            )
            .begin_session()
            .unwrap();
        assert!(matches!(
            family::prepare(&parsed, &LayoutOptions::default(), denied),
            Err(Error::MissingCapability {
                capability: RenderCapability::LayoutGrid,
                ..
            })
        ));
    }

    #[test]
    fn grid_keeps_usecase_roles_and_paints_plain_labels_as_nodes() {
        for html in [false, true] {
            let source = COMPOUND.replace("htmlLabels: false", &format!("htmlLabels: {html}"));
            let parsed = parse(&source);
            let session = RenderEnvironment::deterministic().begin_session().unwrap();
            let artifact = family::prepare(&parsed, &LayoutOptions::default(), session).unwrap();
            let layout = layout(&artifact);
            assert_eq!(
                layout
                    .nodes
                    .iter()
                    .map(|node| node.id.as_str())
                    .collect::<Vec<_>>(),
                ["User", "Login", "Verify", "note-0", "Data", "Auth", "Empty"]
            );
            let [actor, login, verify, _, _, auth, empty] = layout.nodes.as_slice() else {
                unreachable!()
            };
            assert!(actor.x < auth.x && auth.is_cluster && empty.is_cluster);
            assert!(auth.x - auth.width / 2.0 < login.x && verify.x < auth.x + auth.width / 2.0);
            assert!(empty.width > 0.0 && empty.height > 0.0);
            assert_eq!(
                layout
                    .edges
                    .iter()
                    .map(|edge| edge.id.as_str())
                    .collect::<Vec<_>>(),
                ["review", "edge-0", "edge-1", "edge-2", "note-0-edge"]
            );
            assert_eq!(
                layout
                    .edges
                    .iter()
                    .filter(|edge| edge.label.is_some())
                    .count(),
                3
            );

            let svg = svg(artifact);
            let doc = roxmltree::Document::parse(&svg).unwrap();
            let by_id = |id| {
                doc.descendants()
                    .find(|node| node.attribute("id") == Some(id))
                    .unwrap()
            };
            let group = |class| {
                doc.descendants()
                    .find(|node| node.attribute("class") == Some(class))
                    .unwrap()
            };
            let layers: Vec<_> = group("root")
                .children()
                .filter(|node| node.is_element())
                .collect();
            assert_eq!(
                layers
                    .iter()
                    .filter_map(|node| node.attribute("class"))
                    .collect::<Vec<_>>(),
                ["clusters", "edges edgePaths", "edgeLabels", "nodes"]
            );
            assert!(!group("edgeLabels").children().any(|node| node.is_element()));
            let helpers = group("nodes")
                .children()
                .filter(|node| node.attribute("class") == Some("label edgeLabel"))
                .collect::<Vec<_>>();
            assert_eq!(
                helpers
                    .iter()
                    .filter_map(|node| node.attribute("id"))
                    .collect::<Vec<_>>(),
                [
                    "edge-label-User-Login-review",
                    "edge-label-Login-Verify-edge-0",
                    "edge-label-Verify-Login-edge-1"
                ]
            );
            for helper in &helpers {
                assert_eq!(
                    helper
                        .descendants()
                        .any(|node| node.has_tag_name("foreignObject")),
                    html
                );
                assert!(!helper.descendants().any(|node| node.has_tag_name("strong")));
            }
            let review = by_id("usecase-usecase-grid-review");
            assert!(
                review
                    .attribute("aria-label")
                    .unwrap()
                    .starts_with("association **literal** account review")
            );
            assert_eq!(
                review.attribute("marker-end"),
                Some("url(#usecase-grid_usecase-pointEnd)")
            );
            assert_eq!(
                by_id("usecase-usecase-grid-note-0-edge").attribute("aria-hidden"),
                Some("true")
            );
            assert!(
                by_id("usecase-grid-usecase-Auth")
                    .descendants()
                    .any(|node| node
                        .attribute("class")
                        .is_some_and(|class| class.contains("system-boundary-package-tab")))
            );
        }
    }
}
