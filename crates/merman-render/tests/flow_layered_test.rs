#![cfg(all(feature = "diagram-flowchart", feature = "layout-layered"))]

use base64::Engine as _;
use merman_core::{MermaidEngine, ParseOptions};
use merman_model::presentation::PresentationDiagram;
use merman_render::{
    DiagramFamilyId, LayoutOptions, RenderCapability, environment::RenderEnvironment, family,
};

fn parse(source: &str) -> PresentationDiagram {
    MermaidEngine::new()
        .parse_diagram_for_render_model_sync(source, ParseOptions::strict())
        .unwrap()
        .unwrap()
        .into_presentation()
}

#[test]
fn layered_flow_families_keep_axis_paths_labels_and_root_identity() {
    let families = [
        ("flowchart LR", DiagramFamilyId::FLOWCHART, "flowchart-v2"),
        #[cfg(feature = "diagram-agentflow")]
        ("agentflow LR", DiagramFamilyId::AGENTFLOW, "agentflow"),
        #[cfg(feature = "diagram-swimlane")]
        ("swimlane-beta LR", DiagramFamilyId::SWIMLANE, "swimlane"),
    ];
    for (header, family, role) in families {
        let parsed = parse(&format!(
            "\
---
config:
  layout: layered
  htmlLabels: false
  look: classic
  layered:
    edgeRouting: straight
    lineHops: false
---
{header}
A[Read] review@-->|Review| B[Check]
A --> C[Keep]
"
        ));
        let environment = RenderEnvironment::deterministic();
        let session = environment.begin_session().unwrap();
        let plan = family::plan_render(&parsed, &session).unwrap();
        assert_eq!(plan.family_id(), family);
        assert_eq!(
            plan.required_capabilities(),
            [RenderCapability::LayoutLayered]
        );
        let artifact = family::prepare(&parsed, &LayoutOptions::default(), session).unwrap();
        let svg = artifact
            .render_svg(&Default::default(), &Default::default())
            .unwrap();
        let doc = roxmltree::Document::parse(svg.svg()).unwrap();
        let root = doc.root_element();
        assert_eq!(root.attribute("aria-roledescription"), Some(role));
        assert_eq!(root.attribute("data-layout-fallback"), None);
        let edge = root
            .descendants()
            .find(|node| node.has_tag_name("path") && node.attribute("data-id") == Some("review"))
            .unwrap();
        let bytes = base64::engine::general_purpose::STANDARD
            .decode(edge.attribute("data-points").unwrap())
            .unwrap();
        let points: Vec<serde_json::Value> = serde_json::from_slice(&bytes).unwrap();
        assert!(points.len() >= 2);
        assert!(
            points
                .iter()
                .all(|point| matches!(point["axis"].as_str(), Some("h" | "v")))
        );
        let path = edge.attribute("d").unwrap();
        assert!(path.starts_with('M') && path.contains('L'));
        assert!(!path.contains(['C', 'Q', 'A']));
        let labels = root
            .descendants()
            .find(|node| node.attribute("class") == Some("edgeLabels"))
            .unwrap();
        assert!(
            labels
                .descendants()
                .any(|node| node.text() == Some("Review"))
        );
    }
}
