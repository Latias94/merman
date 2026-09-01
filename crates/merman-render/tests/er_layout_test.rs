use merman_core::{Engine, ParseOptions};
use merman_render::LayoutOptions;
use merman_render::environment::RenderEnvironment;
use merman_render::family;
use merman_render::model::ErDiagramLayout;
use std::path::PathBuf;

fn workspace_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
}

fn layout_er(text: &str) -> ErDiagramLayout {
    let parsed = Engine::new()
        .parse_diagram_for_render_model_sync(text, ParseOptions::default())
        .expect("parse ok")
        .expect("diagram detected");
    let session = RenderEnvironment::deterministic().begin_session().unwrap();
    let artifact = family::prepare(parsed, &LayoutOptions::default(), session).expect("ER layout");
    let projection = artifact.layout_json().expect("serialize ER layout");
    serde_json::from_value(projection["layout"]["ErDiagram"].clone()).expect("ER layout projection")
}

#[test]
fn er_layout_produces_positions_and_routes() {
    let path = workspace_root()
        .join("fixtures")
        .join("er")
        .join("basic.mmd");
    let text = std::fs::read_to_string(&path).expect("fixture");

    let layout = layout_er(&text);

    assert!(layout.nodes.len() >= 3);
    assert!(layout.edges.len() >= 2);

    for n in &layout.nodes {
        assert!(n.width.is_finite() && n.width > 0.0);
        assert!(n.height.is_finite() && n.height > 0.0);
        assert!(n.x.is_finite() && n.y.is_finite());
    }

    for e in &layout.edges {
        assert!(
            e.points.len() >= 2,
            "edge {} should have at least two points",
            e.id
        );
        for p in &e.points {
            assert!(p.x.is_finite() && p.y.is_finite());
        }
    }
}

#[test]
fn er_layout_emits_markers_and_dashes_from_rel_spec() {
    let path = workspace_root()
        .join("fixtures")
        .join("er")
        .join("upstream_relationship_aliases.mmd");
    let text = std::fs::read_to_string(&path).expect("fixture");

    let layout = layout_er(&text);

    let mut has_marker = false;
    let mut has_dashed = false;
    for e in &layout.edges {
        if e.start_marker.is_some() || e.end_marker.is_some() {
            has_marker = true;
        }
        if e.stroke_dasharray.as_deref() == Some("8,8") {
            has_dashed = true;
        }
    }

    assert!(has_marker, "expected at least one edge to have ER markers");
    assert!(
        has_dashed,
        "expected at least one NON_IDENTIFYING relationship to be dashed"
    );
}

#[test]
fn er_dagre_recursive_relationship_hides_internal_helper_ranks() {
    let path = workspace_root()
        .join("fixtures")
        .join("er")
        .join(
            "upstream_cypress_erdiagram_spec_should_render_an_er_diagram_with_a_recursive_relationship_002.mmd",
        );
    let text = std::fs::read_to_string(&path).expect("fixture");

    let layout = layout_er(&text);
    let node = |id: &str| {
        layout
            .nodes
            .iter()
            .find(|node| node.id == id)
            .unwrap_or_else(|| panic!("missing ER layout node {id}"))
    };
    let customer = node("entity-CUSTOMER-0");
    let order = node("entity-ORDER-1");
    let line_item = node("entity-LINE-ITEM-2");

    assert!(layout.nodes.iter().all(|node| !node.id.contains("---")));
    let loop_edge = layout
        .edges
        .iter()
        .find(|edge| edge.from == customer.id && edge.to == customer.id)
        .expect("logical recursive relationship edge");
    assert_eq!(
        layout
            .edges
            .iter()
            .filter(|edge| edge.from == customer.id && edge.to == customer.id)
            .count(),
        1
    );
    assert_eq!(loop_edge.points.len(), 4);
    assert!(loop_edge.label.is_some());
    let loop_start = &loop_edge.points[0];
    let loop_inner_start = &loop_edge.points[1];
    let loop_inner_end = &loop_edge.points[2];
    let loop_end = &loop_edge.points[3];
    let customer_bottom = customer.y + customer.height / 2.0;
    assert!((loop_start.y - customer_bottom).abs() < 1e-6);
    assert!((loop_end.y - customer_bottom).abs() < 1e-6);
    assert!(loop_inner_start.y > customer_bottom && loop_inner_end.y > customer_bottom);
    assert!(customer.y < order.y && order.y < line_item.y);
}

#[test]
fn er_layout_materializes_compound_subgraphs() {
    let text = concat!(
        "erDiagram\n",
        "subgraph Orders [Order Domain]\n",
        "  CUSTOMER ||--o{ ORDER : places\n",
        "end\n",
        "WAREHOUSE ||--o{ Orders : ships\n",
    );

    let layout = layout_er(text);
    println!("{layout:#?}");
    assert_eq!(layout.clusters.len(), 1);
    let cluster = &layout.clusters[0];
    assert_eq!(cluster.id, "Orders");
    assert!(cluster.width.is_finite() && cluster.width > 0.0);
    assert!(cluster.height.is_finite() && cluster.height > 0.0);
    assert!(
        layout
            .nodes
            .iter()
            .any(|node| node.id == "Orders" && node.is_cluster)
    );
    assert!(layout.edges.iter().any(|edge| edge.to == "Orders"));
}

#[cfg(feature = "layout-elk")]
#[test]
fn er_layout_config_selects_source_ported_elk_geometry() {
    let elk_source = r#"---
config:
  layout: elk
---
erDiagram
  CUSTOMER ||--o{ ORDER : places
  ORDER ||--|{ LINE-ITEM : contains
  CUSTOMER }|..|{ DELIVERY-ADDRESS : uses
"#;
    let dagre_source = r#"erDiagram
  CUSTOMER ||--o{ ORDER : places
  ORDER ||--|{ LINE-ITEM : contains
  CUSTOMER }|..|{ DELIVERY-ADDRESS : uses
"#;

    let elk = layout_er(elk_source);
    let dagre = layout_er(dagre_source);

    let elk_positions = elk
        .nodes
        .iter()
        .map(|node| (node.id.as_str(), node.x, node.y))
        .collect::<Vec<_>>();
    let dagre_positions = dagre
        .nodes
        .iter()
        .map(|node| (node.id.as_str(), node.x, node.y))
        .collect::<Vec<_>>();
    assert_ne!(elk_positions, dagre_positions);
    assert!(elk.edges.iter().all(|edge| {
        edge.points.windows(2).all(|segment| {
            (segment[0].x - segment[1].x).abs() < 1e-9 || (segment[0].y - segment[1].y).abs() < 1e-9
        })
    }));
}

#[cfg(feature = "layout-elk")]
#[test]
fn er_elk_layout_materializes_compound_subgraphs_and_svg_groups() {
    let text = concat!(
        "---\nconfig:\n  layout: elk\n---\n",
        "erDiagram\n",
        "subgraph Orders [Order Domain]\n",
        "  CUSTOMER ||--o{ ORDER : places\n",
        "end\n",
        "WAREHOUSE ||--o{ Orders : ships\n",
    );
    let parsed = Engine::new()
        .parse_diagram_for_render_model_sync(text, ParseOptions::default())
        .expect("parse ok")
        .expect("diagram detected");
    let session = RenderEnvironment::deterministic().begin_session().unwrap();
    let artifact = family::prepare(parsed, &LayoutOptions::default(), session).expect("ER layout");
    let projection = artifact.layout_json().expect("serialize ER layout");
    let layout: ErDiagramLayout =
        serde_json::from_value(projection["layout"]["ErDiagram"].clone()).expect("layout");
    assert_eq!(layout.clusters.len(), 1);
    assert!(
        layout
            .nodes
            .iter()
            .any(|node| node.id == "Orders" && node.is_cluster)
    );
    assert!(layout.edges.iter().any(|edge| edge.to == "Orders"));

    let svg = artifact
        .render_svg(
            &merman_render::svg::SvgRenderOptions {
                diagram_id: Some("er-elk-subgraph".to_string()),
                ..Default::default()
            },
            &merman_render::svg::SvgDebugOptions::default(),
        )
        .expect("render ER SVG")
        .svg()
        .to_owned();
    assert!(svg.contains(r#"class="clusters""#), "{svg}");
    assert!(svg.contains("er-elk-subgraph-Orders"), "{svg}");
    assert!(svg.contains("Order Domain"), "{svg}");
}
