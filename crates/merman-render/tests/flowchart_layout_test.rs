use merman_core::{Engine, ParseOptions};
use merman_render::text::{TextMeasurer, WrapMode};
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

fn rects_overlap(a: (f64, f64, f64, f64), b: (f64, f64, f64, f64), eps: f64) -> bool {
    let (amin_x, amin_y, amax_x, amax_y) = a;
    let (bmin_x, bmin_y, bmax_x, bmax_y) = b;
    let sep_x = amax_x <= bmin_x + eps || bmax_x <= amin_x + eps;
    let sep_y = amax_y <= bmin_y + eps || bmax_y <= amin_y + eps;
    !(sep_x || sep_y)
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
    let merman_render::model::LayoutDiagram::FlowchartV2(layout) = out.layout else {
        panic!("expected FlowchartV2 layout");
    };

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
    let merman_render::model::LayoutDiagram::FlowchartV2(layout) = out.layout else {
        panic!("expected FlowchartV2 layout");
    };

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
    let merman_render::model::LayoutDiagram::FlowchartV2(layout) = out.layout else {
        panic!("expected FlowchartV2 layout");
    };

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

    // Root-level isolated subgraphs are rendered recursively by Mermaid and should not overlap
    // after applying subgraph `dir`/toggle behavior and cluster padding/title extents.
    let semantic_edges = out
        .semantic
        .get("edges")
        .and_then(|v| v.as_array())
        .expect("semantic edges");

    let mut members_by_id: std::collections::HashMap<String, Vec<String>> =
        std::collections::HashMap::new();
    let mut subgraph_ids: std::collections::HashSet<String> = std::collections::HashSet::new();
    for sg in subgraphs {
        let id = sg.get("id").and_then(|v| v.as_str()).expect("subgraph id");
        let members = sg
            .get("nodes")
            .and_then(|v| v.as_array())
            .expect("subgraph nodes")
            .iter()
            .filter_map(|v| v.as_str().map(|s| s.to_string()))
            .collect::<Vec<_>>();
        members_by_id.insert(id.to_string(), members);
        subgraph_ids.insert(id.to_string());
    }

    let mut child_subgraphs: std::collections::HashSet<String> = std::collections::HashSet::new();
    for members in members_by_id.values() {
        for m in members {
            if subgraph_ids.contains(m) {
                child_subgraphs.insert(m.clone());
            }
        }
    }

    fn collect_leaf_nodes(
        id: &str,
        subgraph_ids: &std::collections::HashSet<String>,
        members_by_id: &std::collections::HashMap<String, Vec<String>>,
        out: &mut std::collections::HashSet<String>,
        visiting: &mut std::collections::HashSet<String>,
    ) {
        if !visiting.insert(id.to_string()) {
            return;
        }
        let Some(members) = members_by_id.get(id) else {
            visiting.remove(id);
            return;
        };
        for m in members {
            if subgraph_ids.contains(m) {
                collect_leaf_nodes(m, subgraph_ids, members_by_id, out, visiting);
            } else {
                out.insert(m.clone());
            }
        }
        visiting.remove(id);
    }

    let mut root_isolated_cluster_ids: Vec<String> = Vec::new();
    for id in subgraph_ids.iter() {
        if child_subgraphs.contains(id) {
            continue;
        }
        let mut leaves: std::collections::HashSet<String> = std::collections::HashSet::new();
        let mut visiting: std::collections::HashSet<String> = std::collections::HashSet::new();
        collect_leaf_nodes(
            id,
            &subgraph_ids,
            &members_by_id,
            &mut leaves,
            &mut visiting,
        );
        if leaves.is_empty() {
            continue;
        }

        let mut has_external = false;
        for e in semantic_edges {
            let from = e.get("from").and_then(|v| v.as_str()).expect("edge from");
            let to = e.get("to").and_then(|v| v.as_str()).expect("edge to");
            let in_from = leaves.contains(from);
            let in_to = leaves.contains(to);
            if in_from ^ in_to {
                has_external = true;
                break;
            }
        }

        if !has_external {
            root_isolated_cluster_ids.push(id.clone());
        }
    }

    root_isolated_cluster_ids.sort();
    for i in 0..root_isolated_cluster_ids.len() {
        for j in (i + 1)..root_isolated_cluster_ids.len() {
            let a = &root_isolated_cluster_ids[i];
            let b = &root_isolated_cluster_ids[j];
            let ca = clusters_by_id.get(a.as_str()).expect("cluster output");
            let cb = clusters_by_id.get(b.as_str()).expect("cluster output");
            assert!(
                !rects_overlap(rect_from_cluster(ca), rect_from_cluster(cb), 1e-6),
                "expected clusters {a} and {b} not to overlap"
            );
        }
    }
}

#[test]
fn flowchart_cluster_exposes_mermaid_diff_and_offset_y() {
    let text = "flowchart TB\nsubgraph A[\"This is a very very very very very very very long title that should wrap\"]\n  a\nend\n";

    let engine = Engine::new();
    let parsed = futures::executor::block_on(engine.parse_diagram(text, ParseOptions::default()))
        .expect("parse ok")
        .expect("diagram detected");

    let out = layout_parsed(&parsed, &LayoutOptions::default()).expect("layout ok");
    let merman_render::model::LayoutDiagram::FlowchartV2(layout) = out.layout else {
        panic!("expected FlowchartV2 layout");
    };

    let cluster = layout
        .clusters
        .iter()
        .find(|c| c.id == "A")
        .expect("cluster A");
    let node_a = layout.nodes.iter().find(|n| n.id == "a").expect("node a");

    // Cluster bounds are padded by `nodeSpacing / 2` (Mermaid-like behavior).
    let cluster_padding = 25.0;
    let node_padding = 15.0;
    let base_width = node_a.width + cluster_padding * 2.0;

    let measurer = merman_render::text::DeterministicTextMeasurer::default();
    let title_metrics = measurer.measure_wrapped(
        &cluster.title,
        &merman_render::text::TextStyle::default(),
        Some(200.0),
        WrapMode::HtmlLike,
    );
    let padded_label_width = title_metrics.width + node_padding;

    let expected_diff = if base_width <= padded_label_width {
        (padded_label_width - base_width) / 2.0 - node_padding
    } else {
        -node_padding
    };
    let expected_offset_y = title_metrics.height - node_padding / 2.0;

    assert!((cluster.diff - expected_diff).abs() < 1e-6);
    assert!((cluster.offset_y - expected_offset_y).abs() < 1e-6);

    let expected_width = base_width.max(padded_label_width);
    assert!((cluster.width - expected_width).abs() < 1e-6);
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
    let merman_render::model::LayoutDiagram::FlowchartV2(layout_no_margin) = out_no_margin.layout
    else {
        panic!("expected FlowchartV2 layout");
    };
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
        out_with_margin.layout
    else {
        panic!("expected FlowchartV2 layout");
    };
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
    let merman_render::model::LayoutDiagram::FlowchartV2(layout) = out.layout else {
        panic!("expected FlowchartV2 layout");
    };

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
        "edge label should fit horizontally in cluster A (cluster=[{cmin_x:.3},{cmax_x:.3}] label=[{lmin_x:.3},{lmax_x:.3}])"
    );
    assert!(
        lmin_y + 1e-6 >= cmin_y && lmax_y <= cmax_y + 1e-6,
        "edge label should fit vertically in cluster A (cluster=[{cmin_y:.3},{cmax_y:.3}] label=[{lmin_y:.3},{lmax_y:.3}])"
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
    let merman_render::model::LayoutDiagram::FlowchartV2(layout) = out.layout else {
        panic!("expected FlowchartV2 layout");
    };

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
    let merman_render::model::LayoutDiagram::FlowchartV2(layout) = out.layout else {
        panic!("expected FlowchartV2 layout");
    };

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
    let merman_render::model::LayoutDiagram::FlowchartV2(layout) = out.layout else {
        panic!("expected FlowchartV2 layout");
    };

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

    // If the label node were incorrectly assigned to `Left`/`Right`, those cluster bounds would
    // expand to include the (very wide) label. Instead, only the LCA (`Outer`) should include it.
    assert!(
        left.width < label.width * 0.8,
        "Left cluster should not expand to include cross-subgraph label"
    );
    assert!(
        right.width < label.width * 0.8,
        "Right cluster should not expand to include cross-subgraph label"
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
    let merman_render::model::LayoutDiagram::FlowchartV2(layout) = out.layout else {
        panic!("expected FlowchartV2 layout");
    };

    let edge = layout
        .edges
        .iter()
        .find(|e| e.from == "A" && e.to == "B")
        .expect("edge A->B");
    let label = edge.label.as_ref().expect("edge label");

    let measurer = merman_render::text::DeterministicTextMeasurer::default();
    let one = measurer.measure_wrapped(
        "line1",
        &merman_render::text::TextStyle::default(),
        Some(200.0),
        WrapMode::HtmlLike,
    );
    assert!(
        label.height > one.height + 1e-6,
        "expected multiline label to have larger height"
    );
}

#[test]
fn flowchart_multigraph_edges_keep_distinct_routes_and_labels() {
    // Mermaid flowcharts are multigraphs; ensure we can lay out multiple edges between the same
    // endpoints without collapsing their routes/labels.
    let text = "flowchart TB\nA -->|l1| B\nA -->|l2| B\nA --> B\n";

    let engine = Engine::new();
    let parsed = futures::executor::block_on(engine.parse_diagram(text, ParseOptions::default()))
        .expect("parse ok")
        .expect("diagram detected");

    let out = layout_parsed(&parsed, &LayoutOptions::default()).expect("layout ok");
    let merman_render::model::LayoutDiagram::FlowchartV2(layout) = out.layout else {
        panic!("expected FlowchartV2 layout");
    };

    let edges = layout
        .edges
        .iter()
        .filter(|e| e.from == "A" && e.to == "B")
        .collect::<Vec<_>>();
    assert_eq!(edges.len(), 3, "expected three A->B edges");

    let labeled = edges.iter().filter(|e| e.label.is_some()).count();
    assert_eq!(labeled, 2, "expected two labeled edges");

    for e in edges {
        assert!(e.points.len() >= 2);
    }
}

#[test]
fn flowchart_isolated_cluster_with_multiple_labeled_edges_contains_all_labels() {
    // When a cluster is isolated, we apply recursive layout (dir/toggle) and should still include
    // all internal edge labels in the cluster bounds.
    let text = "flowchart TB\nsubgraph A\n  direction TB\n  a -->|label one that is quite wide| b\n  b -->|another wide label for coverage| c\nend\n";

    let engine = Engine::new();
    let parsed = futures::executor::block_on(engine.parse_diagram(text, ParseOptions::default()))
        .expect("parse ok")
        .expect("diagram detected");

    let out = layout_parsed(&parsed, &LayoutOptions::default()).expect("layout ok");
    let merman_render::model::LayoutDiagram::FlowchartV2(layout) = out.layout else {
        panic!("expected FlowchartV2 layout");
    };

    let cluster = layout
        .clusters
        .iter()
        .find(|c| c.id == "A")
        .expect("cluster A");
    let cluster_rect = rect_from_cluster(cluster);

    let internal_labeled_edges = layout
        .edges
        .iter()
        .filter(|e| e.label.is_some())
        .collect::<Vec<_>>();
    assert_eq!(
        internal_labeled_edges.len(),
        2,
        "expected two labeled edges in cluster"
    );

    for e in internal_labeled_edges {
        let label = e.label.as_ref().expect("label");
        assert!(
            rect_contains(cluster_rect, rect_from_label(label), 1e-6),
            "edge {} label should fit in cluster A",
            e.id
        );
    }
}

#[test]
fn flowchart_various_edge_styles_do_not_break_layout() {
    // The renderer is headless; edge styling should not affect layout stability.
    // This test mainly ensures we don't crash and we always emit routed points.
    let text = "flowchart TB\nA --> B\nA --- C\nA -.-> D\nA ==> E\n";

    let engine = Engine::new();
    let parsed = futures::executor::block_on(engine.parse_diagram(text, ParseOptions::default()))
        .expect("parse ok")
        .expect("diagram detected");

    let out = layout_parsed(&parsed, &LayoutOptions::default()).expect("layout ok");
    let merman_render::model::LayoutDiagram::FlowchartV2(layout) = out.layout else {
        panic!("expected FlowchartV2 layout");
    };

    assert!(layout.nodes.len() >= 5);
    assert_eq!(layout.edges.len(), 4);
    for e in &layout.edges {
        assert!(!e.points.is_empty(), "edge {} should have points", e.id);
    }
}

#[test]
fn flowchart_node_shape_dimensions_follow_mermaid_rules() {
    // Verify key flowchart shapes follow Mermaid `@11.12.2` sizing rules (headless approximation).
    // This mainly protects us from regressions when refactoring shape sizing.
    let text = r#"flowchart TB
A[Label]
B(Label)
C((Label))
D(((Label)))
E{Label}
F{{Label}}
G>Label]
H([Label])
I[(Label)]
J[[Label]]
K[/Label/]
L[\Label\]
M[/Label\]
N[\Label/]
O(-Label-)
"#;

    let engine = Engine::new();
    let parsed = futures::executor::block_on(engine.parse_diagram(text, ParseOptions::default()))
        .expect("parse ok")
        .expect("diagram detected");

    let out = layout_parsed(&parsed, &LayoutOptions::default()).expect("layout ok");
    let merman_render::model::LayoutDiagram::FlowchartV2(layout) = out.layout else {
        panic!("expected FlowchartV2 layout");
    };

    let nodes_by_id = layout
        .nodes
        .iter()
        .map(|n| (n.id.as_str(), n))
        .collect::<std::collections::HashMap<_, _>>();

    let measurer = merman_render::text::DeterministicTextMeasurer::default();
    let metrics = measurer.measure_wrapped(
        "Label",
        &merman_render::text::TextStyle::default(),
        Some(200.0),
        WrapMode::HtmlLike,
    );
    let p = 15.0;
    let tw = metrics.width;
    let th = metrics.height;

    fn assert_close(actual: f64, expected: f64, name: &str) {
        let eps = 1e-6;
        assert!(
            (actual - expected).abs() <= eps,
            "{name}: expected {expected}, got {actual}"
        );
    }

    // squareRect
    {
        let n = nodes_by_id["A"];
        assert_close(n.width, tw + 4.0 * p, "squareRect width");
        assert_close(n.height, th + 2.0 * p, "squareRect height");
    }

    // roundedRect
    {
        let n = nodes_by_id["B"];
        assert_close(n.width, tw + 2.0 * p, "roundedRect width");
        assert_close(n.height, th + 2.0 * p, "roundedRect height");
    }

    // circle / doublecircle
    {
        let n = nodes_by_id["C"];
        assert_close(n.width, tw + p, "circle width");
        assert_close(n.height, tw + p, "circle height");

        let n = nodes_by_id["D"];
        assert_close(n.width, tw + p + 10.0, "doublecircle width");
        assert_close(n.height, tw + p + 10.0, "doublecircle height");
    }

    // diamond/question
    {
        let n = nodes_by_id["E"];
        let s = (tw + p) + (th + p);
        assert_close(n.width, s, "diamond width");
        assert_close(n.height, s, "diamond height");
    }

    // hexagon
    {
        let n = nodes_by_id["F"];
        let w0 = tw + 2.5 * p;
        assert_close(n.width, w0 * (7.0 / 6.0), "hexagon width");
        assert_close(n.height, th + p, "hexagon height");
    }

    // odd (`rect_left_inv_arrow`)
    {
        let n = nodes_by_id["G"];
        let w = tw + p;
        let h = th + p;
        assert_close(n.width, w + h / 4.0, "odd width");
        assert_close(n.height, h, "odd height");
    }

    // stadium
    {
        let n = nodes_by_id["H"];
        let h = th + p;
        let w = tw + h / 4.0 + p;
        assert_close(n.width, w, "stadium width");
        assert_close(n.height, h, "stadium height");
    }

    // cylinder
    {
        let n = nodes_by_id["I"];
        let w = tw + p;
        let rx = w / 2.0;
        let ry = rx / (2.5 + w / 50.0);
        let expected_h = th + p + 3.0 * ry;
        assert_close(n.width, w, "cylinder width");
        assert_close(n.height, expected_h, "cylinder height");
    }

    // subroutine
    {
        let n = nodes_by_id["J"];
        assert_close(n.width, tw + p + 16.0, "subroutine width");
        assert_close(n.height, th + p, "subroutine height");
    }

    // lean right/left
    {
        let n = nodes_by_id["K"];
        let w = tw + p;
        let h = th + p;
        assert_close(n.width, w + h, "lean_right width");
        assert_close(n.height, h, "lean_right height");

        let n = nodes_by_id["L"];
        assert_close(n.width, w + h, "lean_left width");
        assert_close(n.height, h, "lean_left height");
    }

    // trapezoid / inv_trapezoid
    {
        let n = nodes_by_id["M"];
        let w = tw + p;
        let h = th + p;
        assert_close(n.width, w + h, "trapezoid width");
        assert_close(n.height, h, "trapezoid height");

        let n = nodes_by_id["N"];
        let w = tw + 2.0 * p;
        let h = th + 2.0 * p;
        assert_close(n.width, w + h, "inv_trapezoid width");
        assert_close(n.height, h, "inv_trapezoid height");
    }

    // ellipse (broken upstream, but keep stable headless sizing)
    {
        let n = nodes_by_id["O"];
        assert_close(n.width, tw + 2.0 * p, "ellipse width");
        assert_close(n.height, th + 2.0 * p, "ellipse height");
    }
}

#[test]
fn flowchart_wrapping_width_increases_height_for_long_labels() {
    let text = "%%{init: {\"flowchart\": {\"wrappingWidth\": 60}}}%%\nflowchart TB\nA[This is a long label that should wrap]\n";

    let engine = Engine::new();
    let parsed = futures::executor::block_on(engine.parse_diagram(text, ParseOptions::default()))
        .expect("parse ok")
        .expect("diagram detected");

    let out = layout_parsed(&parsed, &LayoutOptions::default()).expect("layout ok");
    let merman_render::model::LayoutDiagram::FlowchartV2(layout) = out.layout else {
        panic!("expected FlowchartV2 layout");
    };

    let a = layout.nodes.iter().find(|n| n.id == "A").expect("node A");

    let measurer = merman_render::text::DeterministicTextMeasurer::default();
    let style = merman_render::text::TextStyle::default();
    let single = measurer.measure_wrapped(
        "This is a long label that should wrap",
        &style,
        None,
        WrapMode::HtmlLike,
    );

    // With wrapping, the node should become taller than the single-line size would indicate.
    assert!(
        a.height > single.height + 1e-6,
        "expected wrapped label to increase node height"
    );

    // Node width should be constrained by wrappingWidth plus the shape's padding rule (squareRect).
    let p = 15.0;
    assert!(
        a.width <= 60.0 + 4.0 * p + 1e-6,
        "expected wrapped label to constrain node width"
    );
}

#[test]
fn flowchart_htmllabels_long_word_is_clamped_but_not_wrapped() {
    // Mermaid HTML labels use `white-space: nowrap` initially and do not split long words; layout
    // width is constrained by `max-width` but height should not increase.
    let text = "%%{init: {\"flowchart\": {\"wrappingWidth\": 60, \"htmlLabels\": true}}}%%\nflowchart TB\nA[Supercalifragilisticexpialidocious]\n";

    let engine = Engine::new();
    let parsed = futures::executor::block_on(engine.parse_diagram(text, ParseOptions::default()))
        .expect("parse ok")
        .expect("diagram detected");

    let out = layout_parsed(&parsed, &LayoutOptions::default()).expect("layout ok");
    let merman_render::model::LayoutDiagram::FlowchartV2(layout) = out.layout else {
        panic!("expected FlowchartV2 layout");
    };

    let a = layout.nodes.iter().find(|n| n.id == "A").expect("node A");

    let measurer = merman_render::text::DeterministicTextMeasurer::default();
    let style = merman_render::text::TextStyle::default();
    let single = measurer.measure_wrapped(
        "Supercalifragilisticexpialidocious",
        &style,
        None,
        WrapMode::HtmlLike,
    );

    // Height should remain single-line in HTML mode (no long-word splitting).
    assert!(
        (a.height - (single.height + 2.0 * 15.0)).abs() < 1e-6,
        "expected long word to remain single-line in HTML mode"
    );

    // Width should be clamped to wrappingWidth plus squareRect padding rule.
    let p = 15.0;
    assert!(
        a.width <= 60.0 + 4.0 * p + 1e-6,
        "expected HTML mode to clamp width"
    );
}

#[test]
fn flowchart_svglike_long_word_is_wrapped_into_multiple_lines() {
    // In SVG-like mode (`htmlLabels=false`), Mermaid's text wrapping logic can split long words to
    // satisfy the width constraint, increasing height.
    let text = "%%{init: {\"flowchart\": {\"wrappingWidth\": 60, \"htmlLabels\": false}}}%%\nflowchart TB\nA[Supercalifragilisticexpialidocious]\n";

    let engine = Engine::new();
    let parsed = futures::executor::block_on(engine.parse_diagram(text, ParseOptions::default()))
        .expect("parse ok")
        .expect("diagram detected");

    let out = layout_parsed(&parsed, &LayoutOptions::default()).expect("layout ok");
    let merman_render::model::LayoutDiagram::FlowchartV2(layout) = out.layout else {
        panic!("expected FlowchartV2 layout");
    };

    let a = layout.nodes.iter().find(|n| n.id == "A").expect("node A");

    let measurer = merman_render::text::DeterministicTextMeasurer::default();
    let style = merman_render::text::TextStyle::default();
    let single = measurer.measure_wrapped(
        "Supercalifragilisticexpialidocious",
        &style,
        None,
        WrapMode::SvgLike,
    );

    // Height should increase vs. the single-line size.
    assert!(
        a.height > single.height + 2.0 * 15.0 + 1e-6,
        "expected long word to wrap and increase height in SVG-like mode"
    );

    // Width should still respect wrappingWidth via wrapping.
    let p = 15.0;
    assert!(
        a.width <= 60.0 + 4.0 * p + 1e-6,
        "expected SVG-like mode to constrain width via wrapping"
    );
}

#[test]
fn flowchart_subgraph_title_uses_wrapping_placeholder_metrics() {
    let title = "This is a very long subgraph title that should wrap across multiple lines for layout parity";
    let text = format!("flowchart TB\nsubgraph A[{title}]\n  a\nend\n");

    let engine = Engine::new();
    let parsed = futures::executor::block_on(engine.parse_diagram(&text, ParseOptions::default()))
        .expect("parse ok")
        .expect("diagram detected");

    let out = layout_parsed(&parsed, &LayoutOptions::default()).expect("layout ok");
    let merman_render::model::LayoutDiagram::FlowchartV2(layout) = out.layout else {
        panic!("expected FlowchartV2 layout");
    };

    let cluster = layout
        .clusters
        .iter()
        .find(|c| c.id == "A")
        .expect("cluster A");

    let measurer = merman_render::text::DeterministicTextMeasurer::default();
    let style = merman_render::text::TextStyle::default();
    let expected = measurer.measure_wrapped(title, &style, Some(200.0), WrapMode::HtmlLike);

    assert!((cluster.title_label.width - expected.width).abs() < 1e-6);
    assert!((cluster.title_label.height - expected.height).abs() < 1e-6);
    assert!(
        cluster.height >= cluster.title_label.height,
        "cluster should be at least as tall as its title placeholder"
    );
}

#[test]
fn flowchart_subgraph_title_wraps_long_word_in_svglike_mode() {
    let title = "Supercalifragilisticexpialidocious";
    let text = format!(
        "%%{{init: {{\"htmlLabels\": false, \"flowchart\": {{\"htmlLabels\": false}}}}}}%%\nflowchart TB\nsubgraph A[{title}]\n  a\nend\n"
    );

    let engine = Engine::new();
    let parsed = futures::executor::block_on(engine.parse_diagram(&text, ParseOptions::default()))
        .expect("parse ok")
        .expect("diagram detected");

    let out = layout_parsed(&parsed, &LayoutOptions::default()).expect("layout ok");
    let merman_render::model::LayoutDiagram::FlowchartV2(layout) = out.layout else {
        panic!("expected FlowchartV2 layout");
    };

    let cluster = layout
        .clusters
        .iter()
        .find(|c| c.id == "A")
        .expect("cluster A");

    let measurer = merman_render::text::DeterministicTextMeasurer::default();
    let style = merman_render::text::TextStyle::default();
    let single = measurer.measure_wrapped(title, &style, None, WrapMode::SvgLike);
    let wrapped = measurer.measure_wrapped(title, &style, Some(200.0), WrapMode::SvgLike);

    assert!(
        wrapped.height > single.height + 1e-6,
        "expected SVG-like mode to wrap long-word title"
    );
    assert!((cluster.title_label.height - wrapped.height).abs() < 1e-6);
}
