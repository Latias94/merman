#![cfg(feature = "diagram-swimlane")]

use merman_core::{MermaidEngine, ParseOptions};
use merman_model::presentation::PresentationDiagram;
use merman_render::environment::RenderEnvironment;
use merman_render::{DiagramFamilyId, Error, LayoutOptions, family};

// These signed Grid placements reverse the Swimlane's authored LR order.
const GRID: &str = "\
---
config:
  layout: grid
  look: neo
  htmlLabels: false
  flowchart:
    htmlLabels: false
  grid:
    curve: linear
    lineHops: false
    placements:
      A: { row: 0, column: 2 }
      B: { row: 0, column: -1 }
---
swimlane-beta LR
subgraph Team[Request Team]
A[Request] approve@-->|Approved| B[Answer]
end
style Team fill:#eeeeef,stroke:#112233
classDef highlight fill:#ffdc88,stroke:#005577,color:#123456;
class A highlight;
linkStyle 0 stroke:#124578,stroke-width:3px
";

const SWIMLANE: &str = "swimlane-beta LR\nsubgraph Team[Request Team]\nA --> B\nend\n";

fn parse(source: &str) -> PresentationDiagram {
    MermaidEngine::new()
        .parse_diagram_for_render_model_sync(source, ParseOptions::strict())
        .expect("parse Swimlane")
        .expect("detect Swimlane")
        .into_presentation()
}

#[cfg(feature = "layout-grid")]
#[test]
fn explicit_grid_swimlane_keeps_its_family_with_flow_geometry_and_source_css() {
    use merman_render::model::FlowchartLayout;
    use merman_render::svg::SvgRenderOptions;
    use merman_render::{RenderCapability, RenderCapabilityPolicy};

    let parsed = parse(GRID);
    assert_eq!(parsed.family_id(), Some(DiagramFamilyId::SWIMLANE));
    assert_eq!(parsed.metadata().diagram_type, "swimlane");
    let environment = RenderEnvironment::deterministic();
    let session = environment.begin_session().unwrap();
    let plan = family::plan_render(&parsed, &session).unwrap();
    assert_eq!(plan.family_id(), DiagramFamilyId::SWIMLANE);
    assert_eq!(plan.required_capabilities(), [RenderCapability::LayoutGrid]);
    assert!(plan.is_ready());

    let options = LayoutOptions::default();
    let layout = family::prepare_layout(&parsed, &options, session).unwrap();
    let projection = layout.layout_geometry_json().unwrap();
    assert_eq!(projection["family_id"], "swimlane");
    assert_eq!(projection["diagram_type"], "swimlane");
    let payload = &projection["layout"];
    assert!(payload.get("SwimlaneDiagram").is_none());
    let layout: FlowchartLayout = serde_json::from_value(payload["FlowchartV2"].clone()).unwrap();
    assert_eq!(layout.clusters.len(), 1);
    assert_eq!(layout.clusters[0].id, "Team");
    let find = |id| layout.nodes.iter().find(|node| node.id == id).unwrap();
    let (a, b) = (find("A"), find("B"));
    assert!(a.x > b.x);
    assert_eq!(a.y, b.y);
    let [edge] = layout.edges.as_slice() else {
        panic!("expected a single Grid edge");
    };
    assert_eq!(
        (edge.id.as_str(), edge.from.as_str(), edge.to.as_str()),
        ("approve", "A", "B")
    );

    let prepare =
        || family::prepare(&parsed, &options, environment.begin_session().unwrap()).unwrap();
    let artifact = prepare();
    assert_eq!(artifact.family_id(), DiagramFamilyId::SWIMLANE);
    let snapshot = artifact.layout_json().unwrap();
    assert_eq!(snapshot["meta"]["diagram_type"], "swimlane");
    assert_eq!(&snapshot["layout"], payload);
    let geometry = prepare().edge_geometry_json().unwrap();
    let [paint] = geometry.as_slice() else {
        panic!("expected a single paint route");
    };
    assert_eq!(paint.id, "approve");
    let points = paint.points.as_ref().unwrap();
    assert!(points.first().unwrap().x > points.last().unwrap().x);
    let label = paint.label_position.as_ref().unwrap();
    assert!(b.x < label.x && label.x < a.x);

    let rendered = artifact
        .render_svg(
            &SvgRenderOptions {
                diagram_id: Some("grid-swimlane".into()),
                ..Default::default()
            },
            &Default::default(),
        )
        .unwrap();
    assert_eq!(rendered.family_id(), DiagramFamilyId::SWIMLANE);
    let doc = roxmltree::Document::parse(rendered.svg()).unwrap();
    let root = doc.root_element();
    assert_eq!(root.attribute("aria-roledescription"), Some("swimlane"));
    assert!(root.attribute("data-layout-fallback").is_none());
    let css = root
        .children()
        .find(|node| node.has_tag_name("style"))
        .unwrap()
        .text()
        .unwrap();
    assert!(css.contains(".swimlane.cluster:not([data-color-id]) rect"));
    assert!(css.contains(r#"[data-look="neo"].cluster rect{filter:none;}"#));
    assert!(css.contains(".highlight"));
    assert!(
        root.descendants()
            .any(|node| has_class(node, "node") && has_class(node, "highlight"))
    );
    let cluster = root
        .descendants()
        .find(|node| has_class(*node, "cluster"))
        .unwrap();
    assert!(!has_class(cluster, "swimlane"));
    assert!(
        !cluster
            .descendants()
            .any(|node| has_class(node, "swimlane-title"))
    );
    let cluster_style = cluster
        .children()
        .find(|node| node.has_tag_name("rect"))
        .unwrap()
        .attribute("style")
        .unwrap();
    assert!(cluster_style.contains("fill:#eeeeef"));
    assert!(cluster_style.contains("stroke:#112233"));
    let shaft = root
        .descendants()
        .find(|node| node.has_tag_name("path") && node.attribute("data-id") == Some("approve"))
        .unwrap();
    assert!(shaft.attribute("data-points").is_some());
    let style = shaft.attribute("style").unwrap_or_default();
    assert!(style.contains("stroke:#124578") || shaft.attribute("stroke") == Some("#124578"));
    assert!(
        style.contains("stroke-width:3px")
            || matches!(shaft.attribute("stroke-width"), Some("3" | "3px"))
    );
    let edge_labels = root
        .descendants()
        .find(|node| node.attribute("class") == Some("edgeLabels"))
        .unwrap();
    assert!(!edge_labels.children().any(|node| node.is_element()));
    let helper = root
        .descendants()
        .find(|node| {
            node.attribute("data-et") == Some("edge-label")
                && node.attribute("data-id") == Some("approve")
        })
        .unwrap();
    assert_eq!(
        helper.parent_element().unwrap().attribute("class"),
        Some("nodes")
    );
    assert!(helper.descendants().any(|node| node.has_tag_name("text")));
    assert!(
        helper
            .descendants()
            .any(|node| node.text() == Some("Approved"))
    );

    let denied = environment
        .with_capability_policy(
            RenderCapabilityPolicy::unrestricted().with_denied(RenderCapability::LayoutGrid),
        )
        .begin_session()
        .unwrap();
    let plan = family::plan_render(&parsed, &denied).unwrap();
    assert_eq!(plan.family_id(), DiagramFamilyId::SWIMLANE);
    assert_eq!(plan.missing_capabilities(), [RenderCapability::LayoutGrid]);
    assert!(matches!(
        family::prepare(&parsed, &options, denied),
        Err(Error::MissingCapability { capability: RenderCapability::LayoutGrid, diagram_type })
            if diagram_type == "swimlane"
    ));
}

#[cfg(not(feature = "layout-grid"))]
#[test]
fn explicit_grid_swimlane_with_no_grid_feature_is_rejected() {
    let parsed = parse(GRID);
    assert_eq!(parsed.family_id(), Some(DiagramFamilyId::SWIMLANE));
    let session = RenderEnvironment::deterministic().begin_session().unwrap();
    assert!(matches!(
        family::plan_render(&parsed, &session),
        Err(Error::UnsupportedLayoutBackend { backend: "grid", diagram_type })
            if diagram_type == "swimlane"
    ));
}

#[test]
fn older_swimlane_layout_overrides_keep_their_presentation_owners() {
    for (layout, owner) in [
        ("dagre", DiagramFamilyId::FLOWCHART),
        ("elk", DiagramFamilyId::FLOWCHART),
        ("shoal", DiagramFamilyId::SWIMLANE),
    ] {
        let source = format!("---\nconfig:\n  layout: {layout}\n---\n{SWIMLANE}");
        let parsed = parse(&source);
        assert_eq!(parsed.metadata().diagram_type, "swimlane");
        assert_eq!(parsed.family_id(), Some(owner), "layout: {layout}");
    }
}

#[test]
fn ordinary_swimlane_keeps_its_layout_discriminant_and_lane_svg() {
    let session = RenderEnvironment::deterministic().begin_session().unwrap();
    let parsed = parse(SWIMLANE);
    let artifact = family::prepare(&parsed, &LayoutOptions::default(), session).unwrap();
    let projection = artifact.layout_geometry_json().unwrap();
    assert_eq!(projection["family_id"], "swimlane");
    assert!(projection["layout"]["SwimlaneDiagram"]["lanes"].is_array());
    assert!(projection["layout"].get("FlowchartV2").is_none());
    let rendered = artifact
        .render_svg(&Default::default(), &Default::default())
        .unwrap();
    let doc = roxmltree::Document::parse(rendered.svg()).unwrap();
    assert!(
        doc.descendants()
            .any(|node| has_class(node, "swimlane-title"))
    );
}

fn has_class(node: roxmltree::Node<'_, '_>, name: &str) -> bool {
    node.attribute("class")
        .is_some_and(|class| class.split_ascii_whitespace().any(|class| class == name))
}
