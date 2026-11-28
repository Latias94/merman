#![cfg(feature = "diagram-state")]

use merman_core::{MermaidEngine, ParseOptions};
use merman_model::presentation::PresentationDiagram;
use merman_render::{DiagramFamilyId, RenderCapability, environment::RenderEnvironment, family};

// Mermaid 12.4.0 e2e/diagrams/grid/nightly-check/18-state-diagram-direct-edge-around-label.mmd.
const AUTHORED_PLACEMENTS: &str = r###"---
description:
  A state diagram with a labelled edge back to an earlier state, routed
  directly between the two states and around its label.
config:
  layout: grid
  grid:
    columnGap: 300
    placements:
      Draft: { row: 1, column: 1 }
      Review: { row: 1, column: 2 }
      Published: { row: 2, column: 2 }
---
stateDiagram-v2
  Draft --> Review
  Review --> Published
  Review --> Draft : changes requested
"###;

fn parse(source: &str) -> PresentationDiagram {
    let parsed = MermaidEngine::new()
        .parse_diagram_for_render_model_sync(source, ParseOptions::strict())
        .unwrap()
        .unwrap()
        .into_presentation();
    assert_eq!(parsed.family_id(), Some(DiagramFamilyId::STATE));
    assert_eq!(parsed.metadata().diagram_type, "stateDiagram");
    parsed
}

#[test]
fn state_grid_preserves_default_dagre_and_elk_selection() {
    let session = RenderEnvironment::deterministic().begin_session().unwrap();
    let elk = if merman_render::layout_elk_available() || !merman_render::layout_dagre_available() {
        RenderCapability::LayoutElk
    } else {
        RenderCapability::LayoutDagre
    };
    for (source, expected) in [
        (AUTHORED_PLACEMENTS.replace("  layout: grid\n", ""), elk),
        (
            AUTHORED_PLACEMENTS.replace("layout: grid", "layout: dagre"),
            RenderCapability::LayoutDagre,
        ),
        (
            AUTHORED_PLACEMENTS.replace("layout: grid", "layout: elk"),
            elk,
        ),
    ] {
        let plan = family::plan_render(&parse(&source), &session).unwrap();
        assert_eq!(plan.family_id(), DiagramFamilyId::STATE);
        assert_eq!(plan.required_capabilities(), [expected]);
    }
}

#[cfg(not(feature = "layout-grid"))]
#[test]
fn explicit_state_grid_requires_a_compiled_provider() {
    let session = RenderEnvironment::deterministic().begin_session().unwrap();
    assert!(matches!(
        family::plan_render(&parse(AUTHORED_PLACEMENTS), &session),
        Err(merman_render::Error::UnsupportedLayoutBackend { backend: "grid", diagram_type })
            if diagram_type == "stateDiagram"
    ));
}

#[cfg(feature = "layout-grid")]
mod grid {
    use super::*;
    use base64::{Engine as _, engine::general_purpose::STANDARD};
    use merman_render::{
        Error, LayoutOptions,
        family::FamilyRenderArtifact,
        model::{LayoutNode, LayoutPoint, StateDiagramLayout},
        svg::SvgRenderOptions,
    };
    use roxmltree::{Document, Node};

    const COMPOUND: &str = r###"---
config:
  layout: grid
  htmlLabels: false
  state:
    padding: 14
    wrappingWidth: 92
    minNodeWidth: 28
  flowchart:
    htmlLabels: true
    wrappingWidth: 62
  grid:
    placements:
      Ready: { row: 2, column: 2 }
      Active: { row: 1, column: 2 }
      Done: { row: 1, column: 3 }
      Pick: { row: 1, column: 1 }
      Branch: { row: 1, column: 2 }
      Left: { row: 2, column: 1 }
      Right: { row: 2, column: 2 }
      Gather: { row: 3, column: 2 }
---
stateDiagram-v2
    direction TB
    state "Ready for work" as Ready
    [*] --> Ready
    Ready --> Active: **open** selected item
    state "In progress" as Active {
        direction LR
        state Pick <<choice>>
        state Branch <<fork>>
        state Gather <<join>>
        [*] --> Pick
        Pick --> Branch: choose
        Branch --> Left
        Branch --> Right
        Left --> Gather
        Right --> Gather
        Gather --> [*]
        Left --> Left: again
        Left --> Left: repeat
    }
    Active --> Done: close
    Done --> [*]
    note right of Done: Archived
"###;
    const CROSSING: &str = r###"---
config:
  layout: grid
  htmlLabels: false
  grid:
    lineHops: false
    rowGap: 50
    columnGap: 50
    placements:
      A: { row: 1, column: 2 }
      B: { row: 2, column: 1 }
      C: { row: 2, column: 3 }
      D: { row: 3, column: 2 }
---
stateDiagram-v2
    A --> D
    B --> C
"###;

    fn prepare(parsed: &PresentationDiagram) -> FamilyRenderArtifact<'_> {
        let session = RenderEnvironment::deterministic().begin_session().unwrap();
        family::prepare(parsed, &LayoutOptions::default(), session).unwrap()
    }

    fn layout(artifact: &FamilyRenderArtifact<'_>) -> StateDiagramLayout {
        let geometry = artifact.layout_geometry_json().unwrap();
        assert_eq!(geometry["family_id"], "state");
        assert_eq!(geometry["diagram_type"], "stateDiagram");
        assert_eq!(
            geometry["layout"],
            artifact.layout_json().unwrap()["layout"]
        );
        serde_json::from_value(geometry["layout"]["StateDiagramV2"].clone()).unwrap()
    }

    fn svg(artifact: FamilyRenderArtifact<'_>) -> String {
        let rendered = artifact
            .render_svg(
                &SvgRenderOptions {
                    diagram_id: Some("state-grid".into()),
                    ..Default::default()
                },
                &Default::default(),
            )
            .unwrap();
        assert_eq!(rendered.family_id(), DiagramFamilyId::STATE);
        rendered.svg().to_owned()
    }

    fn node<'a>(layout: &'a StateDiagramLayout, id: &str) -> &'a LayoutNode {
        layout.nodes.iter().find(|node| node.id == id).unwrap()
    }

    fn edge_paths<'a, 'input>(doc: &'a Document<'input>) -> impl Iterator<Item = Node<'a, 'input>> {
        doc.descendants()
            .filter(|node| node.attribute("data-edge") == Some("true"))
    }

    fn close(left: f64, right: f64) {
        assert!((left - right).abs() < 1e-6, "{left} vs {right}");
    }

    #[test]
    fn pinned_state_grid_preserves_placements_and_literal_relation_label() {
        for html_labels in [false, true] {
            let source = AUTHORED_PLACEMENTS
                .replace(
                    "  layout: grid",
                    &format!(
                        "  layout: grid\n  look: classic\n  htmlLabels: {html_labels}\n  flowchart:\n    htmlLabels: {}\n    wrappingWidth: 92",
                        !html_labels,
                    ),
                )
                .replace("changes requested", "**changes** requested");
            let parsed = parse(&source);
            let session = RenderEnvironment::deterministic().begin_session().unwrap();
            let plan = family::plan_render(&parsed, &session).unwrap();
            assert_eq!(plan.required_capabilities(), [RenderCapability::LayoutGrid]);
            assert!(plan.is_ready());
            let artifact = prepare(&parsed);
            let layout = layout(&artifact);
            let [draft, review, published] =
                ["Draft", "Review", "Published"].map(|id| node(&layout, id));
            assert!(draft.x < review.x && review.y < published.y);
            close(draft.y, review.y);
            close(review.x, published.x);
            let reverse = &layout.edges[2];
            assert_eq!(
                (
                    reverse.id.as_str(),
                    reverse.from.as_str(),
                    reverse.to.as_str()
                ),
                ("edge2", "Review", "Draft")
            );
            let label = reverse.label.as_ref().unwrap();
            assert!(draft.x < label.x && label.x < review.x);

            let svg = svg(artifact);
            let doc = Document::parse(&svg).unwrap();
            assert_eq!(
                doc.root_element().attribute("aria-roledescription"),
                Some("stateDiagram")
            );
            let root = doc
                .descendants()
                .find(|node| node.attribute("class") == Some("root"))
                .unwrap();
            let layers: Vec<_> = root.children().filter(Node::is_element).collect();
            assert_eq!(
                layers
                    .iter()
                    .map(|node| node.attribute("class").unwrap())
                    .collect::<Vec<_>>(),
                ["clusters", "edges edgePaths", "edgeLabels", "nodes"],
            );
            assert!(!layers[2].children().any(|node| node.is_element()));
            let helper = layers[3]
                .children()
                .find(|node| node.attribute("id") == Some("edge-label-Review-Draft-edge2"))
                .unwrap();
            assert_eq!(helper.attribute("class"), Some("label edgeLabel"));
            assert_eq!(
                helper
                    .descendants()
                    .any(|node| node.has_tag_name("foreignObject")),
                html_labels
            );
            assert!(
                helper
                    .descendants()
                    .filter(Node::is_text)
                    .filter_map(|node| node.text())
                    .any(|text| text.contains("**changes**"))
            );
            assert_eq!(edge_paths(&doc).count(), 3);
            assert!(edge_paths(&doc).all(|node| node.attribute("marker-end")
                == Some("url(#state-grid_stateDiagram-barbEnd)")));
        }
    }

    #[test]
    fn state_grid_keeps_composite_containment_pseudo_states_and_independent_self_loops() {
        let source = COMPOUND.replace("  layout: grid", "  layout: grid\n  look: classic");
        for (direction, width, height) in [("LR", 10.0, 70.0), ("BT", 70.0, 10.0)] {
            let parsed = parse(&source.replace("direction LR", &format!("direction {direction}")));
            let artifact = prepare(&parsed);
            let layout = layout(&artifact);
            let group = layout
                .clusters
                .iter()
                .find(|cluster| cluster.id == "Active")
                .unwrap();
            assert_eq!(group.requested_dir.as_deref(), Some(direction));
            assert_eq!(group.title, "In progress");
            for id in ["Pick", "Branch", "Gather", "Active_start", "Active_end"] {
                let child = node(&layout, id);
                assert!((child.x - group.x).abs() + child.width / 2.0 < group.width / 2.0);
                assert!((child.y - group.y).abs() + child.height / 2.0 < group.height / 2.0);
            }
            for id in ["Branch", "Gather"] {
                let bar = node(&layout, id);
                assert_eq!((bar.width, bar.height), (width, height));
            }
            for id in ["root_start", "root_end", "Active_start", "Active_end"] {
                let terminal = node(&layout, id);
                assert_eq!((terminal.width, terminal.height), (14.0, 14.0));
            }
            assert_eq!(
                layout
                    .edges
                    .iter()
                    .filter(|edge| edge.from == "Left" && edge.to == "Left")
                    .count(),
                2
            );

            let svg = svg(artifact);
            let doc = Document::parse(&svg).unwrap();
            assert_eq!(
                doc.descendants()
                    .filter(|node| node.attribute("class") == Some("root"))
                    .count(),
                1
            );
            assert_eq!(
                doc.descendants()
                    .filter(|node| node
                        .attribute("id")
                        .is_some_and(|id| id.starts_with("edge-label-Left-Left-")))
                    .count(),
                2
            );
            assert!(edge_paths(&doc).any(|node| {
                node.attribute("class").unwrap().contains("note-edge")
                    && node.attribute("marker-end").is_none()
            }));
            let path = edge_paths(&doc)
                .find(|node| node.attribute("data-id") == Some("edge2"))
                .unwrap();
            let data = STANDARD
                .decode(path.attribute("data-points").unwrap())
                .unwrap();
            let points: Vec<LayoutPoint> = serde_json::from_slice(&data).unwrap();
            let start = &points[0];
            let circle = node(&layout, "Active_start");
            close((start.x - circle.x).hypot(start.y - circle.y), 7.0);
            let end = points.last().unwrap();
            let diamond = node(&layout, "Pick");
            close((end.x - diamond.x).abs() + (end.y - diamond.y).abs(), 14.0);
        }
    }

    #[test]
    fn state_grid_arcs_and_gaps_keep_neo_markers_and_route_points() {
        let source = CROSSING.replace("  layout: grid", "  layout: grid\n  look: neo");
        let render = |setting: &str| {
            let parsed = parse(&source.replace("lineHops: false", &format!("lineHops: {setting}")));
            let svg = svg(prepare(&parsed));
            let doc = Document::parse(&svg).unwrap();
            edge_paths(&doc)
                .map(|node| {
                    ["data-points", "marker-end", "d"]
                        .map(|key| node.attribute(key).unwrap().to_owned())
                })
                .collect::<Vec<_>>()
        };
        let base = render("false");
        let arc = render("arc");
        let gap = render("gap");
        assert_eq!(base.len(), 2);
        for painted in [&arc, &gap] {
            assert_eq!(painted.len(), base.len());
            assert!(base.iter().zip(painted).all(|(a, b)| a[..2] == b[..2]));
        }
        assert!(
            base.iter()
                .all(|edge| edge[1] == "url(#state-grid_stateDiagram-barbEnd-margin)")
        );
        assert!(arc[1][2].contains('A'));
        assert!(gap[1][2].matches('M').count() > 1);
    }

    #[test]
    fn state_grid_propagates_shared_placement_diagnostics() {
        let invalid = AUTHORED_PLACEMENTS.replace("Draft: { row: 1", "Draft: { row: 1.5");
        let err = MermaidEngine::new()
            .parse_diagram_for_render_model_sync(&invalid, ParseOptions::strict());
        assert!(matches!(err,
            Err(merman_core::Error::SourceConfig { diagnostic })
                if diagnostic.code() == Some("GRID_INVALID_COORDINATE")
        ));

        let conflict = AUTHORED_PLACEMENTS
            .replace(
                "Draft: { row: 1, column: 1 }",
                "Draft: { row: 1, column: 1, verticalAlign: top }",
            )
            .replace(
                "Review: { row: 1, column: 2 }",
                "Review: { row: 1, column: 1, verticalAlign: bottom }",
            );
        let parsed = parse(&conflict);
        let session = RenderEnvironment::deterministic().begin_session().unwrap();
        assert!(matches!(
            family::prepare(&parsed, &LayoutOptions::default(), session),
            Err(Error::InvalidModel { message }) if message.starts_with("GRID_CELL_ALIGNMENT_CONFLICT:")
        ));
    }
}
