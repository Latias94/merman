#![cfg(feature = "diagram-mindmap")]

use merman_core::{MermaidEngine, ParseOptions};
use merman_model::presentation::PresentationDiagram;
use merman_render::{environment::RenderEnvironment, family};

// Mermaid 12.4.0 e2e/diagrams/grid/nightly-check/10-mindmap-authored-placement-ids.mmd.
const SOURCE: &str = "\
---
config:
  layout: grid
  look: classic
  htmlLabels: false
  grid:
    placements:
      root: { row: 2, column: 2 }
      Plan: { row: 1, column: 1 }
      Build: { row: 3, column: 1 }
      Test: { row: 1, column: 3 }
      Deploy: { row: 3, column: 3 }
---
mindmap
  root((Release))
    Plan
    Build
      Test
    Deploy
";

fn parse() -> PresentationDiagram {
    MermaidEngine::new()
        .parse_diagram_for_render_model_sync(SOURCE, ParseOptions::strict())
        .unwrap()
        .unwrap()
        .into_presentation()
}

#[cfg(feature = "layout-grid")]
#[test]
fn mindmap_grid_keeps_authored_positions_and_the_tree_edge_layers() {
    use merman_render::{
        DiagramFamilyId, LayoutOptions, RenderCapability, model::MindmapDiagramLayout,
        svg::SvgRenderOptions,
    };

    let parsed = parse();
    let session = RenderEnvironment::deterministic().begin_session().unwrap();
    let plan = family::plan_render(&parsed, &session).unwrap();
    assert_eq!(plan.family_id(), DiagramFamilyId::MINDMAP);
    assert_eq!(plan.required_capabilities(), [RenderCapability::LayoutGrid]);
    let artifact = family::prepare(&parsed, &LayoutOptions::default(), session).unwrap();
    let geometry = artifact.layout_geometry_json().unwrap();
    assert_eq!(geometry["family_id"], "mindmap");
    let layout: MindmapDiagramLayout =
        serde_json::from_value(geometry["layout"]["MindmapDiagram"].clone()).unwrap();
    let [root, plan, build, test, deploy] = &layout.nodes[..] else {
        panic!("expected five preorder Mindmap nodes");
    };
    assert!(plan.x < root.x && root.x < test.x);
    assert!(plan.y < root.y && root.y < build.y);
    assert_eq!(plan.y, test.y);
    assert_eq!(build.y, deploy.y);
    assert_eq!(plan.x, build.x);
    assert_eq!(test.x, deploy.x);
    assert_eq!(layout.edges[2].from, build.id);
    assert_eq!(layout.edges[2].to, test.id);

    let svg = artifact
        .render_svg(
            &SvgRenderOptions {
                diagram_id: Some("mindmap-grid".to_owned()),
                ..SvgRenderOptions::default()
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
    let names: Vec<_> = layers.iter().map(|node| node.attribute("class")).collect();
    assert_eq!(
        names,
        [
            Some("clusters"),
            Some("edges edgePaths"),
            Some("edgeLabels"),
            Some("nodes")
        ]
    );
    assert!(!layers[2].children().any(|node| node.is_element()));
    let edges: Vec<_> = layers[1]
        .children()
        .filter(|node| node.has_tag_name("path"))
        .collect();
    assert_eq!(edges.len(), layout.edges.len());
    assert!(
        edges
            .iter()
            .all(|node| node.attribute("data-points").is_some())
    );
    assert!(
        layers[3]
            .children()
            .any(|node| node.attribute("id") == Some("mindmap-grid-node_3"))
    );
}

#[cfg(not(feature = "layout-grid"))]
#[test]
fn explicit_mindmap_grid_requires_a_compiled_provider() {
    let parsed = parse();
    let session = RenderEnvironment::deterministic().begin_session().unwrap();
    assert!(matches!(
        family::plan_render(&parsed, &session),
        Err(merman_render::Error::UnsupportedLayoutBackend { backend: "grid", diagram_type })
            if diagram_type == "mindmap"
    ));
}
