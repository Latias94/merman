use merman_core::{Engine, ParseOptions};
use merman_render::{LayoutOptions, layout_parsed};
use std::path::PathBuf;

fn workspace_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
}

fn approx_gt(a: f64, b: f64) -> bool {
    a > b + 1e-6
}

#[test]
fn flowchart_layout_produces_positions_and_routes() {
    let path = workspace_root()
        .join("fixtures")
        .join("flowchart")
        .join("basic.mmd");
    let text = std::fs::read_to_string(&path).expect("fixture");
    let engine = Engine::new();
    let parsed = futures::executor::block_on(engine.parse_diagram(&text, ParseOptions::default()))
        .expect("parse ok")
        .expect("diagram detected");

    let out = layout_parsed(&parsed, &LayoutOptions::default()).expect("layout ok");
    let merman_render::model::LayoutDiagram::FlowchartV2(layout) = out.layout;

    assert_eq!(layout.nodes.len(), 4);
    assert_eq!(layout.edges.len(), 3);

    let mut by_id = std::collections::HashMap::new();
    for n in &layout.nodes {
        by_id.insert(n.id.as_str(), (n.x, n.y));
        assert!(n.width.is_finite() && n.width > 0.0);
        assert!(n.height.is_finite() && n.height > 0.0);
    }

    let (ax, ay) = by_id["A"];
    let (bx, by) = by_id["B"];
    let (_cx, cy) = by_id["C"];
    let (_dx, dy) = by_id["D"];

    assert!(approx_gt(by, ay), "B should be below A in TB direction");
    assert!(approx_gt(cy, by), "C should be below B in TB direction");
    assert!(approx_gt(dy, by), "D should be below B in TB direction");
    assert!(ax.is_finite() && bx.is_finite());

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
fn flowchart_layout_respects_lr_direction() {
    let text = "flowchart LR\nA-->B\nB-->C\n";
    let engine = Engine::new();
    let parsed = futures::executor::block_on(engine.parse_diagram(text, ParseOptions::default()))
        .expect("parse ok")
        .expect("diagram detected");

    let out = layout_parsed(&parsed, &LayoutOptions::default()).expect("layout ok");
    let merman_render::model::LayoutDiagram::FlowchartV2(layout) = out.layout;

    let mut by_id = std::collections::HashMap::new();
    for n in &layout.nodes {
        by_id.insert(n.id.as_str(), (n.x, n.y));
    }
    let (ax, _ay) = by_id["A"];
    let (bx, _by) = by_id["B"];
    let (cx, _cy) = by_id["C"];

    assert!(approx_gt(bx, ax), "B should be right of A in LR direction");
    assert!(approx_gt(cx, bx), "C should be right of B in LR direction");
}

#[test]
fn flowchart_layout_includes_clusters_with_title_placeholders() {
    let path = workspace_root()
        .join("fixtures")
        .join("flowchart")
        .join("upstream_subgraphs.mmd");
    let text = std::fs::read_to_string(&path).expect("fixture");

    let engine = Engine::new();
    let parsed = futures::executor::block_on(engine.parse_diagram(&text, ParseOptions::default()))
        .expect("parse ok")
        .expect("diagram detected");

    let out = layout_parsed(&parsed, &LayoutOptions::default()).expect("layout ok");
    let merman_render::model::LayoutDiagram::FlowchartV2(layout) = out.layout;

    assert_eq!(layout.clusters.len(), 5);
    let ids = layout
        .clusters
        .iter()
        .map(|c| c.id.as_str())
        .collect::<Vec<_>>();
    assert_eq!(ids, vec!["A", "child", "id1", "parent", "subGraph2"]);

    for c in &layout.clusters {
        assert!(c.width.is_finite() && c.width > 0.0);
        assert!(c.height.is_finite() && c.height > 0.0);
        assert!(c.title_label.width.is_finite() && c.title_label.width >= 0.0);
        assert!(c.title_label.height.is_finite() && c.title_label.height >= 0.0);

        // Title placeholder should be horizontally centered relative to the cluster.
        assert!((c.title_label.x - c.x).abs() < 1e-6);
        // Title placeholder should be at or above the cluster center (towards the top).
        assert!(c.title_label.y <= c.y + 1e-6);
        // Cluster width should be large enough to fit the title placeholder.
        assert!(c.width + 1e-6 >= c.title_label.width);
    }
}

#[test]
fn flowchart_cluster_title_margins_increase_cluster_height() {
    let text_no_margin = "flowchart TD\nsubgraph A\na-->b\nend\n";
    let text_with_margin = "%%{init: {\"flowchart\": {\"subGraphTitleMargin\": {\"top\": 10, \"bottom\": 5}}}}%%\nflowchart TD\nsubgraph A\na-->b\nend\n";

    let engine = Engine::new();

    let parsed_no_margin =
        futures::executor::block_on(engine.parse_diagram(text_no_margin, ParseOptions::default()))
            .expect("parse ok")
            .expect("diagram detected");
    let out_no_margin =
        layout_parsed(&parsed_no_margin, &LayoutOptions::default()).expect("layout");
    let merman_render::model::LayoutDiagram::FlowchartV2(layout_no_margin) = out_no_margin.layout;
    let h0 = layout_no_margin
        .clusters
        .iter()
        .find(|c| c.id == "A")
        .expect("cluster A")
        .height;

    let parsed_with_margin = futures::executor::block_on(
        engine.parse_diagram(text_with_margin, ParseOptions::default()),
    )
    .expect("parse ok")
    .expect("diagram detected");
    let out_with_margin =
        layout_parsed(&parsed_with_margin, &LayoutOptions::default()).expect("layout");
    let merman_render::model::LayoutDiagram::FlowchartV2(layout_with_margin) =
        out_with_margin.layout;
    let c = layout_with_margin
        .clusters
        .iter()
        .find(|c| c.id == "A")
        .expect("cluster A");

    assert!((c.height - h0 - 15.0).abs() < 1e-6);
    assert!((c.title_margin_top - 10.0).abs() < 1e-6);
    assert!((c.title_margin_bottom - 5.0).abs() < 1e-6);
}
