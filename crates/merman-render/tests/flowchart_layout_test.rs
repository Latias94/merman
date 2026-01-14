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

fn rect_from_cluster(c: &merman_render::model::LayoutCluster) -> (f64, f64, f64, f64) {
    let hw = c.width / 2.0;
    let hh = c.height / 2.0;
    (c.x - hw, c.y - hh, c.x + hw, c.y + hh)
}

fn rect_from_label(l: &merman_render::model::LayoutLabel) -> (f64, f64, f64, f64) {
    let hw = l.width / 2.0;
    let hh = l.height / 2.0;
    (l.x - hw, l.y - hh, l.x + hw, l.y + hh)
}

fn rect_contains(outer: (f64, f64, f64, f64), inner: (f64, f64, f64, f64), eps: f64) -> bool {
    let (omin_x, omin_y, omax_x, omax_y) = outer;
    let (imin_x, imin_y, imax_x, imax_y) = inner;
    imin_x + eps >= omin_x
        && imax_x <= omax_x + eps
        && imin_y + eps >= omin_y
        && imax_y <= omax_y + eps
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

    // Mermaid's modern flowchart layout represents edge labels as label nodes. Ensure we emit
    // stable label placeholders for labeled edges.
    let labeled = layout.edges.iter().filter(|e| e.label.is_some()).count();
    assert!(labeled >= 2, "expected at least two labeled edges");
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

    let clusters_by_id = layout
        .clusters
        .iter()
        .map(|c| (c.id.as_str(), c))
        .collect::<std::collections::HashMap<_, _>>();

    // Default `inheritDir` is false; when a subgraph does not specify `dir`, Mermaid toggles
    // the layout direction for isolated clusters (TB -> LR).
    assert_eq!(clusters_by_id["A"].effective_dir, "LR");
    assert_eq!(clusters_by_id["id1"].effective_dir, "LR");
    assert_eq!(clusters_by_id["subGraph2"].effective_dir, "RL");
    assert_eq!(clusters_by_id["child"].effective_dir, "BT");

    fn rect_from_layout_node(n: &merman_render::model::LayoutNode) -> (f64, f64, f64, f64) {
        let hw = n.width / 2.0;
        let hh = n.height / 2.0;
        (n.x - hw, n.y - hh, n.x + hw, n.y + hh)
    }

    fn rect_from_layout_cluster(c: &merman_render::model::LayoutCluster) -> (f64, f64, f64, f64) {
        let hw = c.width / 2.0;
        let hh = c.height / 2.0;
        (c.x - hw, c.y - hh, c.x + hw, c.y + hh)
    }

    let nodes_by_id = layout
        .nodes
        .iter()
        .map(|n| (n.id.as_str(), n))
        .collect::<std::collections::HashMap<_, _>>();
    let clusters_by_id = clusters_by_id;

    // Verify that cluster `dir` (or toggled direction) affects internal node layout when the
    // cluster has no external connections.
    {
        let a = nodes_by_id.get("a").expect("node a");
        let b = nodes_by_id.get("b").expect("node b");
        assert!(b.x > a.x, "cluster A should lay out a->b left-to-right");

        let c = nodes_by_id.get("c").expect("node c");
        let d = nodes_by_id.get("d").expect("node d");
        assert!(d.x > c.x, "cluster id1 should lay out c->d left-to-right");

        let e = nodes_by_id.get("e").expect("node e");
        let f = nodes_by_id.get("f").expect("node f");
        assert!(
            f.x < e.x,
            "cluster subGraph2 dir=RL should lay out e->f right-to-left"
        );

        let g = nodes_by_id.get("g").expect("node g");
        let h = nodes_by_id.get("h").expect("node h");
        assert!(
            h.y < g.y,
            "cluster child dir=BT should lay out g->h bottom-to-top"
        );
    }

    let subgraphs = out
        .semantic
        .get("subgraphs")
        .and_then(|v| v.as_array())
        .expect("semantic subgraphs");
    for sg in subgraphs {
        let id = sg.get("id").and_then(|v| v.as_str()).expect("subgraph id");
        let members = sg
            .get("nodes")
            .and_then(|v| v.as_array())
            .expect("subgraph nodes");
        let cluster = clusters_by_id.get(id).expect("cluster output");
        let (cmin_x, cmin_y, cmax_x, cmax_y) = rect_from_layout_cluster(cluster);

        for m in members {
            let mid = m.as_str().expect("member id");

            let (min_x, min_y, max_x, max_y) = if let Some(child_cluster) = clusters_by_id.get(mid)
            {
                rect_from_layout_cluster(child_cluster)
            } else if let Some(node) = nodes_by_id.get(mid) {
                rect_from_layout_node(node)
            } else {
                continue;
            };

            assert!(
                min_x + 1e-6 >= cmin_x && max_x <= cmax_x + 1e-6,
                "member {mid} should fit horizontally in cluster {id}"
            );
            assert!(
                min_y + 1e-6 >= cmin_y && max_y <= cmax_y + 1e-6,
                "member {mid} should fit vertically in cluster {id}"
            );
        }
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

#[test]
fn flowchart_edge_label_is_included_in_subgraph_bounds() {
    // Ensure edge labels participate in cluster bounding box calculation. Without including the
    // label node (used internally for layout), a very wide label in TB direction can extend
    // beyond the union of the member node rectangles.
    let text = "flowchart TB\nsubgraph A\n  direction TB\n  a -->|this is a very very very very very long label| b\nend\n";

    let engine = Engine::new();
    let parsed = futures::executor::block_on(engine.parse_diagram(text, ParseOptions::default()))
        .expect("parse ok")
        .expect("diagram detected");

    let out = layout_parsed(&parsed, &LayoutOptions::default()).expect("layout ok");
    let merman_render::model::LayoutDiagram::FlowchartV2(layout) = out.layout;

    let cluster = layout
        .clusters
        .iter()
        .find(|c| c.id == "A")
        .expect("cluster A");

    let edge = layout
        .edges
        .iter()
        .find(|e| e.from == "a" && e.to == "b")
        .expect("edge a->b");
    let label = edge.label.as_ref().expect("edge label");

    let c_hw = cluster.width / 2.0;
    let c_hh = cluster.height / 2.0;
    let cmin_x = cluster.x - c_hw;
    let cmax_x = cluster.x + c_hw;
    let cmin_y = cluster.y - c_hh;
    let cmax_y = cluster.y + c_hh;

    let l_hw = label.width / 2.0;
    let l_hh = label.height / 2.0;
    let lmin_x = label.x - l_hw;
    let lmax_x = label.x + l_hw;
    let lmin_y = label.y - l_hh;
    let lmax_y = label.y + l_hh;

    assert!(
        lmin_x + 1e-6 >= cmin_x && lmax_x <= cmax_x + 1e-6,
        "edge label should fit horizontally in cluster A"
    );
    assert!(
        lmin_y + 1e-6 >= cmin_y && lmax_y <= cmax_y + 1e-6,
        "edge label should fit vertically in cluster A"
    );
}

#[test]
fn flowchart_subgraph_dir_is_not_applied_when_cluster_has_external_edges() {
    // Mermaid only applies subgraph `dir` as a recursive layout when the cluster is isolated
    // (no external connections). When there is an external edge, internal layout follows the
    // diagram direction.
    let text = "flowchart TB\nsubgraph A\n  direction LR\n  a --> b\nend\na --> c\n";

    let engine = Engine::new();
    let parsed = futures::executor::block_on(engine.parse_diagram(text, ParseOptions::default()))
        .expect("parse ok")
        .expect("diagram detected");

    let out = layout_parsed(&parsed, &LayoutOptions::default()).expect("layout ok");
    let merman_render::model::LayoutDiagram::FlowchartV2(layout) = out.layout;

    let nodes_by_id = layout
        .nodes
        .iter()
        .map(|n| (n.id.as_str(), (n.x, n.y)))
        .collect::<std::collections::HashMap<_, _>>();

    let (_ax, ay) = nodes_by_id["a"];
    let (_bx, by) = nodes_by_id["b"];
    assert!(
        by > ay + 5.0,
        "node b should be below a (TB) when cluster A has external edges"
    );
}

#[test]
fn flowchart_nested_subgraph_labeled_edge_label_is_inside_inner_cluster() {
    let text = "flowchart TB\nsubgraph Outer\n  subgraph Inner\n    a -->|this is a very very long label| b\n  end\nend\n";

    let engine = Engine::new();
    let parsed = futures::executor::block_on(engine.parse_diagram(text, ParseOptions::default()))
        .expect("parse ok")
        .expect("diagram detected");

    let out = layout_parsed(&parsed, &LayoutOptions::default()).expect("layout ok");
    let merman_render::model::LayoutDiagram::FlowchartV2(layout) = out.layout;

    let clusters_by_id = layout
        .clusters
        .iter()
        .map(|c| (c.id.as_str(), c))
        .collect::<std::collections::HashMap<_, _>>();
    let inner = clusters_by_id.get("Inner").expect("cluster Inner");
    let outer = clusters_by_id.get("Outer").expect("cluster Outer");

    let edge = layout
        .edges
        .iter()
        .find(|e| e.from == "a" && e.to == "b")
        .expect("edge a->b");
    let label = edge.label.as_ref().expect("edge label");

    let label_rect = rect_from_label(label);
    assert!(
        rect_contains(rect_from_cluster(inner), label_rect, 1e-6),
        "edge label should fit in Inner cluster"
    );
    assert!(
        rect_contains(rect_from_cluster(outer), label_rect, 1e-6),
        "edge label should fit in Outer cluster"
    );
}

#[test]
fn flowchart_cross_subgraph_labeled_edge_label_belongs_to_outer_cluster() {
    // The edge spans two different subgraphs; the label node should be assigned to the lowest
    // common compound parent (the outer subgraph), so only the outer cluster must include it.
    let text = "flowchart TB\nsubgraph Outer\n  subgraph Left\n    a\n  end\n  subgraph Right\n    b\n  end\n  a -->|this is a very very very long cross-subgraph label| b\nend\n";

    let engine = Engine::new();
    let parsed = futures::executor::block_on(engine.parse_diagram(text, ParseOptions::default()))
        .expect("parse ok")
        .expect("diagram detected");

    let out = layout_parsed(&parsed, &LayoutOptions::default()).expect("layout ok");
    let merman_render::model::LayoutDiagram::FlowchartV2(layout) = out.layout;

    let clusters_by_id = layout
        .clusters
        .iter()
        .map(|c| (c.id.as_str(), c))
        .collect::<std::collections::HashMap<_, _>>();
    let outer = clusters_by_id.get("Outer").expect("cluster Outer");
    let left = clusters_by_id.get("Left").expect("cluster Left");
    let right = clusters_by_id.get("Right").expect("cluster Right");

    let edge = layout
        .edges
        .iter()
        .find(|e| e.from == "a" && e.to == "b")
        .expect("edge a->b");
    let label = edge.label.as_ref().expect("edge label");

    let label_rect = rect_from_label(label);
    assert!(
        rect_contains(rect_from_cluster(outer), label_rect, 1e-6),
        "cross-subgraph edge label should fit in Outer cluster"
    );
    assert!(
        !rect_contains(rect_from_cluster(left), label_rect, 1e-6),
        "cross-subgraph edge label should not be required to fit in Left cluster"
    );
    assert!(
        !rect_contains(rect_from_cluster(right), label_rect, 1e-6),
        "cross-subgraph edge label should not be required to fit in Right cluster"
    );
}

#[test]
fn flowchart_html_multiline_edge_label_has_multiple_lines() {
    // The deterministic measurer normalizes `<br/>` into `\\n`, so multiline labels should get
    // larger height than a single-line label.
    let text = "flowchart TB\nA -->|line1<br/>line2| B\n";

    let engine = Engine::new();
    let parsed = futures::executor::block_on(engine.parse_diagram(text, ParseOptions::default()))
        .expect("parse ok")
        .expect("diagram detected");

    let out = layout_parsed(&parsed, &LayoutOptions::default()).expect("layout ok");
    let merman_render::model::LayoutDiagram::FlowchartV2(layout) = out.layout;

    let edge = layout
        .edges
        .iter()
        .find(|e| e.from == "A" && e.to == "B")
        .expect("edge A->B");
    let label = edge.label.as_ref().expect("edge label");

    assert!(
        label.height > 45.0,
        "expected multiline label to have larger height"
    );
}
