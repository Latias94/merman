use merman_core::{Engine, ParseOptions};
use merman_render::Error;
use merman_render::model::{FlowchartV2Layout, LayoutDiagram};
use merman_render::text::{TextMeasurer, VendoredFontMetricsTextMeasurer, WrapMode};
use merman_render::{LayoutOptions, layout_parsed};
use std::path::PathBuf;
use std::sync::Arc;

fn workspace_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
}

fn approx_gt(a: f64, b: f64) -> bool {
    a > b + 1e-6
}

fn approx_eq(a: f64, b: f64) -> bool {
    (a - b).abs() <= 1e-6
}

fn layout_flowchart(text: &str) -> Box<FlowchartV2Layout> {
    let engine = Engine::new();
    let parsed = futures::executor::block_on(engine.parse_diagram(text, ParseOptions::default()))
        .expect("parse ok")
        .expect("diagram detected");
    let out = layout_parsed(&parsed, &LayoutOptions::default()).expect("layout ok");
    let LayoutDiagram::FlowchartV2(layout) = out.layout else {
        panic!("expected FlowchartV2 layout");
    };
    layout
}

fn flowchart_node_center(layout: &FlowchartV2Layout, id: &str) -> (f64, f64) {
    let node = layout
        .nodes
        .iter()
        .find(|node| node.id == id)
        .unwrap_or_else(|| panic!("node {id}"));
    (node.x, node.y)
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
fn flowchart_node_spacing_zero_falls_back_to_mermaid_default() {
    let default = layout_flowchart(
        r#"flowchart TB
A --> B
A --> C
"#,
    );
    let zero = layout_flowchart(
        r#"%%{init: {"flowchart": {"nodeSpacing": 0}}}%%
flowchart TB
A --> B
A --> C
"#,
    );
    let roomy = layout_flowchart(
        r#"%%{init: {"flowchart": {"nodeSpacing": 100}}}%%
flowchart TB
A --> B
A --> C
"#,
    );

    let default_dx = {
        let (bx, _) = flowchart_node_center(&default, "B");
        let (cx, _) = flowchart_node_center(&default, "C");
        (cx - bx).abs()
    };
    let zero_dx = {
        let (bx, _) = flowchart_node_center(&zero, "B");
        let (cx, _) = flowchart_node_center(&zero, "C");
        (cx - bx).abs()
    };
    let roomy_dx = {
        let (bx, _) = flowchart_node_center(&roomy, "B");
        let (cx, _) = flowchart_node_center(&roomy, "C");
        (cx - bx).abs()
    };

    assert!(
        approx_eq(zero_dx, default_dx),
        "Mermaid treats flowchart.nodeSpacing=0 as falsy and falls back to 50; default={default_dx}, zero={zero_dx}"
    );
    assert!(
        roomy_dx > default_dx + 25.0,
        "configured positive nodeSpacing should still affect layout; default={default_dx}, roomy={roomy_dx}"
    );
}

#[test]
fn flowchart_rank_spacing_zero_falls_back_to_mermaid_default() {
    let default = layout_flowchart(
        r#"flowchart TB
A --> B
"#,
    );
    let zero = layout_flowchart(
        r#"%%{init: {"flowchart": {"rankSpacing": 0}}}%%
flowchart TB
A --> B
"#,
    );
    let roomy = layout_flowchart(
        r#"%%{init: {"flowchart": {"rankSpacing": 100}}}%%
flowchart TB
A --> B
"#,
    );

    let default_dy = {
        let (_, ay) = flowchart_node_center(&default, "A");
        let (_, by) = flowchart_node_center(&default, "B");
        by - ay
    };
    let zero_dy = {
        let (_, ay) = flowchart_node_center(&zero, "A");
        let (_, by) = flowchart_node_center(&zero, "B");
        by - ay
    };
    let roomy_dy = {
        let (_, ay) = flowchart_node_center(&roomy, "A");
        let (_, by) = flowchart_node_center(&roomy, "B");
        by - ay
    };

    assert!(
        approx_eq(zero_dy, default_dy),
        "Mermaid treats flowchart.rankSpacing=0 as falsy and falls back to 50; default={default_dy}, zero={zero_dy}"
    );
    assert!(
        roomy_dy > default_dy + 25.0,
        "configured positive rankSpacing should still affect layout; default={default_dy}, roomy={roomy_dy}"
    );
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
    // Base cluster width should come from the layout result (cluster bounds), not from any
    // title-driven widening. Measure it from an otherwise-identical graph with a short title.
    let short_text = "flowchart TB\nsubgraph A[\"`x`\"]\n  a\nend\n";
    let long_text = "flowchart TB\nsubgraph A[\"`This is a very very very very very very very long title that should wrap`\"]\n  a\nend\n";

    let engine = Engine::new();
    let parsed_short =
        futures::executor::block_on(engine.parse_diagram(short_text, ParseOptions::default()))
            .expect("parse ok")
            .expect("diagram detected");
    let out_short = layout_parsed(&parsed_short, &LayoutOptions::default()).expect("layout ok");
    let merman_render::model::LayoutDiagram::FlowchartV2(layout_short) = out_short.layout else {
        panic!("expected FlowchartV2 layout");
    };
    let base_cluster = layout_short
        .clusters
        .iter()
        .find(|c| c.id == "A")
        .expect("cluster A");
    let base_width = base_cluster.width;

    let parsed_long =
        futures::executor::block_on(engine.parse_diagram(long_text, ParseOptions::default()))
            .expect("parse ok")
            .expect("diagram detected");

    let out = layout_parsed(&parsed_long, &LayoutOptions::default()).expect("layout ok");
    let merman_render::model::LayoutDiagram::FlowchartV2(layout) = out.layout else {
        panic!("expected FlowchartV2 layout");
    };

    let cluster = layout
        .clusters
        .iter()
        .find(|c| c.id == "A")
        .expect("cluster A");

    // Mermaid FlowDB encodes subgraph nodes with `padding: 8`.
    let cluster_padding = cluster.padding;
    assert!((cluster_padding - 8.0).abs() < 1e-6);

    // `node.diff` is computed from the (layout) cluster node width and the measured title bbox.
    let title_w = cluster.title_label.width.max(1.0);

    let expected_diff = if base_width <= title_w {
        (title_w - base_width) / 2.0 - cluster_padding / 2.0
    } else {
        -cluster_padding / 2.0
    };
    let expected_offset_y = cluster.title_label.height - cluster_padding / 2.0;

    assert!((cluster.diff - expected_diff).abs() < 1e-6);
    assert!((cluster.offset_y - expected_offset_y).abs() < 1e-6);
}

#[test]
fn flowchart_recursive_cluster_title_bbox_feeds_parent_layout() {
    let text = std::fs::read_to_string(
        workspace_root()
            .join("fixtures")
            .join("flowchart")
            .join("stress_flowchart_subgraph_deep_nesting_title_padding_044.mmd"),
    )
    .expect("read fixture");

    let engine = Engine::new();
    let parsed = futures::executor::block_on(engine.parse_diagram(&text, ParseOptions::default()))
        .expect("parse ok")
        .expect("diagram detected");
    let out = layout_parsed(
        &parsed,
        &LayoutOptions {
            text_measurer: Arc::new(VendoredFontMetricsTextMeasurer::default()),
            ..Default::default()
        },
    )
    .expect("layout ok");
    let merman_render::model::LayoutDiagram::FlowchartV2(layout) = out.layout else {
        panic!("expected FlowchartV2 layout");
    };

    let cluster = |id: &str| {
        layout
            .clusters
            .iter()
            .find(|c| c.id == id)
            .unwrap_or_else(|| panic!("cluster {id}"))
    };
    let node = |id: &str| {
        layout
            .nodes
            .iter()
            .find(|n| n.id == id)
            .unwrap_or_else(|| panic!("node {id}"))
    };

    let c1 = cluster("c1");
    let c2 = cluster("c2");
    let c1a = node("c1a");

    // Mermaid measures the rendered child `<g class="root">` with the title-widened cluster rect
    // before laying out the parent graph. If we only feed the pre-title compound width back to the
    // parent, c1 collapses by about 59px and c1a lands too close to c2.
    assert!(
        c2.width >= c2.title_label.width + c2.padding - 1e-6,
        "c2 should expose the rendered title-widened cluster width"
    );
    assert!(
        c1.width >= c2.width + c1a.width + 100.0,
        "parent cluster width should reflect the rendered child clusterNode bbox"
    );
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
fn flowchart_edge_to_ancestor_cluster_keeps_ancestor_non_recursive() {
    let text = std::fs::read_to_string(
        workspace_root()
            .join("fixtures")
            .join("flowchart")
            .join("stress_flowchart_subgraph_title_margins_extreme_nested_030.mmd"),
    )
    .expect("read fixture");

    let engine = Engine::new();
    let parsed = futures::executor::block_on(engine.parse_diagram(&text, ParseOptions::default()))
        .expect("parse ok")
        .expect("diagram detected");
    let out = layout_parsed(
        &parsed,
        &LayoutOptions {
            text_measurer: Arc::new(VendoredFontMetricsTextMeasurer::default()),
            ..Default::default()
        },
    )
    .expect("layout ok");
    let merman_render::model::LayoutDiagram::FlowchartV2(layout) = out.layout else {
        panic!("expected FlowchartV2 layout");
    };

    let nodes_by_id = layout
        .nodes
        .iter()
        .map(|n| (n.id.as_str(), (n.x, n.y)))
        .collect::<std::collections::HashMap<_, _>>();
    let clusters_by_id = layout
        .clusters
        .iter()
        .map(|c| (c.id.as_str(), c))
        .collect::<std::collections::HashMap<_, _>>();

    let (ax, ay) = nodes_by_id["a"];
    let (bx, by) = nodes_by_id["b"];
    let (cx, cy) = nodes_by_id["c"];
    assert!(
        bx > ax + 60.0 && cx > bx + 60.0,
        "ancestor cluster edge should not make Outer use its TB direction as a recursive layout"
    );
    assert!(
        (cy - ay).abs() < 80.0 && (by - ay).abs() < 80.0,
        "nodes should stay in the root LR layout band"
    );

    let inner = clusters_by_id["Inner"];
    let outer = clusters_by_id["Outer"];
    assert!(
        inner.width > inner.height && outer.width > outer.height,
        "non-recursive nested clusters should keep the upstream wide LR footprint"
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
    // Verify key flowchart shapes follow Mermaid `@11.15.0` sizing rules (headless approximation).
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
P@{ shape: lined-cylinder, label: "Label" }
Q@{ shape: paper-tape, label: "Label" }
R@{ shape: docs, label: "Label" }
S@{ shape: bow-rect, label: "Label" }
T@{ shape: win-pane, label: "Label" }
U@{ shape: doc, label: "Label" }
V@{ shape: delay, label: "Label" }
W@{ shape: lin-doc, label: "Label" }
X@{ shape: tag-doc, label: "Label" }
Y@{ shape: curved-trapezoid, label: "Label" }
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
        let eps = 1e-5;
        assert!(
            (actual - expected).abs() <= eps,
            "{name}: expected {expected}, got {actual}"
        );
    }

    fn assert_close_eps(actual: f64, expected: f64, eps: f64, name: &str) {
        assert!(
            (actual - expected).abs() <= eps,
            "{name}: expected {expected}, got {actual}"
        );
    }

    fn wave_points(
        x1: f64,
        y1: f64,
        x2: f64,
        y2: f64,
        amplitude: f64,
        num_cycles: f64,
    ) -> Vec<(f64, f64)> {
        let steps = 50;
        let delta_x = x2 - x1;
        let delta_y = y2 - y1;
        let cycle_length = delta_x / num_cycles;
        let frequency = std::f64::consts::TAU / cycle_length;
        let mid_y = y1 + delta_y / 2.0;
        (0..=steps)
            .map(|i| {
                let t = (i as f64) / (steps as f64);
                let x = x1 + t * delta_x;
                let y = mid_y + amplitude * (frequency * (x - x1)).sin();
                (x, y)
            })
            .collect()
    }

    fn bbox_size(points: &[(f64, f64)]) -> (f64, f64) {
        let mut min_x = f64::INFINITY;
        let mut min_y = f64::INFINITY;
        let mut max_x = f64::NEG_INFINITY;
        let mut max_y = f64::NEG_INFINITY;
        for &(x, y) in points {
            min_x = min_x.min(x);
            min_y = min_y.min(y);
            max_x = max_x.max(x);
            max_y = max_y.max(y);
        }
        ((max_x - min_x).max(0.0), (max_y - min_y).max(0.0))
    }

    fn bow_tie_rect_bbox_width(w: f64, h: f64) -> f64 {
        fn arc_points(
            x1: f64,
            y1: f64,
            x2: f64,
            y2: f64,
            rx: f64,
            ry: f64,
            clockwise: bool,
        ) -> Vec<(f64, f64)> {
            let num_points = 20usize;
            let mid_x = (x1 + x2) / 2.0;
            let mid_y = (y1 + y2) / 2.0;
            let angle = (y2 - y1).atan2(x2 - x1);
            let dx = (x2 - x1) / 2.0;
            let dy = (y2 - y1) / 2.0;
            let transformed_x = dx / rx;
            let transformed_y = dy / ry;
            let distance = (transformed_x * transformed_x + transformed_y * transformed_y).sqrt();
            if distance > 1.0 {
                return vec![(x1, y1), (x2, y2)];
            }

            let scaled_center_distance = (1.0 - distance * distance).sqrt();
            let sign = if clockwise { -1.0 } else { 1.0 };
            let center_x = mid_x + scaled_center_distance * ry * angle.sin() * sign;
            let center_y = mid_y - scaled_center_distance * rx * angle.cos() * sign;
            let start_angle = ((y1 - center_y) / ry).atan2((x1 - center_x) / rx);
            let end_angle = ((y2 - center_y) / ry).atan2((x2 - center_x) / rx);

            let mut angle_range = end_angle - start_angle;
            if clockwise && angle_range < 0.0 {
                angle_range += 2.0 * std::f64::consts::PI;
            }
            if !clockwise && angle_range > 0.0 {
                angle_range -= 2.0 * std::f64::consts::PI;
            }

            (0..num_points)
                .map(|i| {
                    let t = i as f64 / (num_points - 1) as f64;
                    let a = start_angle + t * angle_range;
                    (center_x + rx * a.cos(), center_y + ry * a.sin())
                })
                .collect()
        }

        let ry = h / 2.0;
        let rx = ry / (2.5 + h / 50.0);
        let mut points = vec![(w / 2.0, -h / 2.0), (-w / 2.0, -h / 2.0)];
        points.extend(arc_points(
            -w / 2.0,
            -h / 2.0,
            -w / 2.0,
            h / 2.0,
            rx,
            ry,
            false,
        ));
        points.push((w / 2.0, h / 2.0));
        points.extend(arc_points(
            w / 2.0,
            h / 2.0,
            w / 2.0,
            -h / 2.0,
            rx,
            ry,
            true,
        ));
        let (min_x, max_x) = points
            .iter()
            .fold((f64::INFINITY, f64::NEG_INFINITY), |(min_x, max_x), p| {
                (min_x.min(p.0), max_x.max(p.0))
            });
        (max_x - min_x).max(0.0)
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
        assert_close(n.width, tw + 2.0 * p, "doublecircle width");
        assert_close(n.height, tw + 2.0 * p, "doublecircle height");
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
        let expected_h = (th + p) as f32 as f64;
        let expected_w = (tw + 2.0 * (expected_h / 4.0) + p) as f32 as f64;
        assert_close(n.width, expected_w, "hexagon width");
        assert_close(n.height, expected_h, "hexagon height");
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

        // Flowchart-v2 stadium nodes are rendered via a roughjs path built from sampled arc points.
        // Mermaid runs `updateNodeBounds(getBBox)` on that path and feeds the resulting bbox width
        // into Dagre layout. Because the arc sampling (50 points over 180deg) does not include the
        // exact extrema, the bbox is slightly narrower than `w`.
        let radius = h / 2.0;
        let mut min_x = f64::INFINITY;
        let mut max_x = f64::NEG_INFINITY;
        let mut include_x = |x: f64| {
            min_x = min_x.min(x);
            max_x = max_x.max(x);
        };
        include_x(-w / 2.0 + radius);
        include_x(w / 2.0 - radius);
        // `generateCirclePoints(...)` returns negated coordinates.
        let step = std::f64::consts::PI / (50_f64 - 1.0); // 180deg / (n-1)
        for i in 0..50 {
            let angle = (std::f64::consts::FRAC_PI_2) + (i as f64) * step; // 90deg..270deg
            let x = (-w / 2.0 + radius) + radius * angle.cos();
            include_x(-x);
        }
        for i in 0..50 {
            let angle = (std::f64::consts::FRAC_PI_2 * 3.0) + (i as f64) * step; // 270deg..450deg
            let x = (w / 2.0 - radius) + radius * angle.cos();
            include_x(-x);
        }
        let expected_w = (max_x - min_x).max(0.0);

        assert_close(n.width, expected_w, "stadium width");
        assert_close(n.height, h, "stadium height");
    }

    // cylinder
    {
        let n = nodes_by_id["I"];
        let w = tw + p;
        let rx = w / 2.0;
        let ry = rx / (2.5 + w / 50.0);
        let expected_h = {
            let h = th + p + 3.0 * ry;
            let h_f32 = h as f32;
            if h_f32.is_finite() && h_f32.is_sign_positive() {
                let bits = h_f32.to_bits();
                if bits < u32::MAX {
                    f32::from_bits(bits + 1) as f64
                } else {
                    h
                }
            } else {
                h
            }
        };
        assert_close(n.width, w, "cylinder width");
        assert_close(n.height, expected_h, "cylinder height");
    }

    // lined cylinder / disk storage
    {
        let n = nodes_by_id["P"];
        let w = merman_render::text::round_to_1_64_px(tw + 2.0 * p);
        let rx = w / 2.0;
        let ry = rx / (2.5 + w / 50.0);
        let expected_h = (th + 2.0 * p + 3.0 * ry) as f32 as f64;
        assert_close(n.width, w, "lined cylinder width");
        assert_close(n.height, expected_h, "lined cylinder height");
    }

    // paper tape / flag
    {
        let n = nodes_by_id["Q"];
        let w = merman_render::text::round_to_1_64_px(tw + 2.0 * p);
        let h = th + p;
        let wave_amplitude = h / 8.0;
        let final_h = h + wave_amplitude * 2.0;
        let sampled_wave_extreme = (0..=50)
            .map(|i| ((i as f64) / 50.0 * std::f64::consts::TAU).sin().abs())
            .fold(0.0, f64::max);
        let expected_h = final_h + 2.0 * wave_amplitude * sampled_wave_extreme;
        assert_close(n.width, w, "paper tape width");
        assert_close(n.height, expected_h as f32 as f64, "paper tape height");
    }

    // stacked document
    {
        let n = nodes_by_id["R"];
        let w = merman_render::text::round_to_1_64_px(tw + 2.0 * p);
        let h = th + 3.0 * p;
        let wave_amplitude = h / 8.0;
        let final_h = h + wave_amplitude / 2.0;
        let rect_offset = 10.0;
        let sampled_wave_max = (0..=50)
            .map(|i| ((i as f64) / 50.0 * std::f64::consts::TAU * 0.8).sin())
            .fold(f64::NEG_INFINITY, f64::max);
        let expected_h = final_h + 2.0 * rect_offset + wave_amplitude * sampled_wave_max;
        assert_close(n.width, w + 2.0 * rect_offset, "docs width");
        assert_close(n.height, expected_h as f32 as f64, "docs height");
    }

    // bow tie rectangle / stored data
    {
        let n = nodes_by_id["S"];
        let w = tw + 2.0 * p;
        let h = th + p;
        assert_close_eps(
            n.width,
            bow_tie_rect_bbox_width(w, h) as f32 as f64,
            0.002,
            "bow tie rectangle width",
        );
        assert_close(n.height, h, "bow tie rectangle height");
    }

    // window pane / internal storage
    {
        let n = nodes_by_id["T"];
        let rect_offset = 10.0;
        let w = merman_render::text::round_to_1_64_px(tw);
        let h = merman_render::text::round_to_1_64_px(th);
        assert_close(
            n.width,
            (w + 2.0 * p + rect_offset) as f32 as f64,
            "window pane width",
        );
        assert_close(
            n.height,
            (h + 2.0 * p + rect_offset) as f32 as f64,
            "window pane height",
        );
    }

    // document / wave-edged rectangle
    {
        let n = nodes_by_id["U"];
        let w = merman_render::text::round_to_1_64_px(tw) + 2.0 * p;
        let h = merman_render::text::round_to_1_64_px(th) + 2.0 * p;
        let wave_amplitude = h / 8.0;
        let final_h = h + wave_amplitude;
        let sampled_wave_extreme = (0..=50)
            .map(|i| {
                ((i as f64) / 50.0 * std::f64::consts::TAU * 0.8)
                    .sin()
                    .abs()
            })
            .fold(0.0, f64::max);
        assert_close(n.width, w as f32 as f64, "document width");
        assert_close_eps(
            n.height,
            (final_h + wave_amplitude * sampled_wave_extreme) as f32 as f64,
            0.005,
            "document height",
        );
    }

    // delay / half-rounded rectangle
    {
        let n = nodes_by_id["V"];
        let w = merman_render::text::round_to_1_64_px(tw) + 2.0 * p;
        let h = merman_render::text::round_to_1_64_px(th) + 2.0 * p;
        let radius = h / 2.0;
        let mut min_x = -w / 2.0;
        let mut max_x = w / 2.0 - radius;
        let step = std::f64::consts::PI / (50_f64 - 1.0);
        for i in 0..50 {
            let angle = std::f64::consts::FRAC_PI_2 + (i as f64) * step;
            let x = (-w / 2.0 + radius) + radius * angle.cos();
            min_x = min_x.min(-x);
            max_x = max_x.max(-x);
        }
        assert_close(n.width, (max_x - min_x) as f32 as f64, "delay width");
        assert_close(n.height, h as f32 as f64, "delay height");
    }

    // lined document
    {
        let n = nodes_by_id["W"];
        let w = merman_render::text::round_to_1_64_px(tw) + 2.0 * p;
        let h = merman_render::text::round_to_1_64_px(th) + 2.0 * p;
        let wave_amplitude = h / 8.0;
        let final_h = h + wave_amplitude;
        let extra = (w / 2.0) * 0.1;
        let mut points = Vec::new();
        points.push((-w / 2.0 - extra, -final_h / 2.0));
        points.push((-w / 2.0 - extra, final_h / 2.0));
        points.extend(wave_points(
            -w / 2.0 - extra,
            final_h / 2.0,
            w / 2.0 + extra,
            final_h / 2.0,
            wave_amplitude,
            0.8,
        ));
        points.push((w / 2.0 + extra, -final_h / 2.0));
        points.push((-w / 2.0 - extra, -final_h / 2.0));
        points.push((-w / 2.0, -final_h / 2.0));
        points.push((-w / 2.0, (final_h / 2.0) * 1.1));
        points.push((-w / 2.0, -final_h / 2.0));

        let (expected_w, expected_h) = bbox_size(&points);
        assert_close(n.width, expected_w as f32 as f64, "lined document width");
        assert_close(n.height, expected_h as f32 as f64, "lined document height");
    }

    // tagged document
    {
        let n = nodes_by_id["X"];
        let w = merman_render::text::round_to_1_64_px(tw) + 2.0 * p;
        let h = merman_render::text::round_to_1_64_px(th) + 2.0 * p;
        let wave_amplitude = h / 8.0;
        let final_h = h + wave_amplitude;
        let extra = (w / 2.0) * 0.1;
        let tag_width = 0.2 * w;
        let tag_height = 0.2 * h;
        let mut points = Vec::new();
        points.push((-w / 2.0 - extra, final_h / 2.0));
        points.extend(wave_points(
            -w / 2.0 - extra,
            final_h / 2.0,
            w / 2.0 + extra,
            final_h / 2.0,
            wave_amplitude,
            0.8,
        ));
        points.push((w / 2.0 + extra, -final_h / 2.0));
        points.push((-w / 2.0 - extra, -final_h / 2.0));

        let x = -w / 2.0 + extra;
        let y = -final_h / 2.0 - tag_height * 0.4;
        points.push((x + w - tag_width, (y + h) * 1.3));
        points.push((x + w, y + h - tag_height));
        points.push((x + w, (y + h) * 0.9));
        points.extend(wave_points(
            x + w,
            (y + h) * 1.25,
            x + w - tag_width,
            (y + h) * 1.3,
            -h * 0.02,
            0.5,
        ));

        let (expected_w, expected_h) = bbox_size(&points);
        assert_close(n.width, expected_w as f32 as f64, "tagged document width");
        assert_close(n.height, expected_h as f32 as f64, "tagged document height");
    }

    // curved trapezoid / display
    {
        let n = nodes_by_id["Y"];
        let min_width = 20.0;
        let min_height = 5.0;
        let w = ((merman_render::text::round_to_1_64_px(tw) + 2.0 * p) * 1.25).max(min_width);
        let h = (merman_render::text::round_to_1_64_px(th) + 2.0 * p).max(min_height);
        let radius = h / 2.0;
        let rw = w - radius;
        let trapezoid_tw = h / 4.0;
        let mut points = vec![
            (rw, 0.0),
            (trapezoid_tw, 0.0),
            (0.0, h / 2.0),
            (trapezoid_tw, h),
            (rw, h),
        ];
        let step = -std::f64::consts::PI / (50_f64 - 1.0);
        for i in 0..50 {
            let angle = std::f64::consts::PI * 1.5 + (i as f64) * step;
            let x = -rw + radius * angle.cos();
            let y = -h / 2.0 + radius * angle.sin();
            points.push((-x, -y));
        }

        let (expected_w, expected_h) = bbox_size(&points);
        assert_close(n.width, expected_w as f32 as f64, "curved trapezoid width");
        assert_close(
            n.height,
            expected_h as f32 as f64,
            "curved trapezoid height",
        );
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
fn flowchart_anchor_shape_ignores_label_for_layout() {
    let text = "flowchart TB\nA@{ shape: anchor, label: 'Ignored by Mermaid' }\n";

    let engine = Engine::new();
    let parsed = futures::executor::block_on(engine.parse_diagram(text, ParseOptions::default()))
        .expect("parse ok")
        .expect("diagram detected");

    let out = layout_parsed(&parsed, &LayoutOptions::default()).expect("layout ok");
    let merman_render::model::LayoutDiagram::FlowchartV2(layout) = out.layout else {
        panic!("expected FlowchartV2 layout");
    };

    let node = layout
        .nodes
        .iter()
        .find(|n| n.id == "A")
        .expect("anchor node");
    assert!((node.width - 2.001_899_003_982_544).abs() <= 1e-9);
    assert!((node.height - 2.0).abs() <= 1e-9);
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
fn flowchart_svglike_markdown_node_labels_wrap_for_shape_layout() {
    let text = r#"---
config:
  htmlLabels: false
  flowchart:
    htmlLabels: false
---
flowchart TB
  n0 --> n00@{ shape: triangle, label: 'This is **bold** </br>and <strong>strong</strong> for triangle shape' }
  n1 --> n11@{ shape: sloped-rectangle, label: 'This is **bold** </br>and <strong>strong</strong> for sloped-rectangle shape' }
"#;

    let engine = Engine::new();
    let parsed = futures::executor::block_on(engine.parse_diagram(text, ParseOptions::default()))
        .expect("parse ok")
        .expect("diagram detected");

    let out = layout_parsed(&parsed, &LayoutOptions::default()).expect("layout ok");
    let merman_render::model::LayoutDiagram::FlowchartV2(layout) = out.layout else {
        panic!("expected FlowchartV2 layout");
    };

    let triangle = layout
        .nodes
        .iter()
        .find(|n| n.id == "n00")
        .expect("triangle node");
    let sloped_rect = layout
        .nodes
        .iter()
        .find(|n| n.id == "n11")
        .expect("sloped rectangle node");

    assert!(
        triangle.width < 320.0,
        "SVG markdown label wrapping should constrain triangle layout width, got {}",
        triangle.width
    );
    assert!(
        sloped_rect.width < 260.0,
        "SVG markdown label wrapping should constrain sloped-rectangle layout width, got {}",
        sloped_rect.width
    );
}

#[test]
fn flowchart_subgraph_title_uses_wrapping_placeholder_metrics() {
    let title = "This is a very long subgraph title that should wrap across multiple lines for layout parity";
    // Subgraph titles only wrap when the label type is `markdown` (Mermaid uses `createText(...)`
    // with the default width=200).
    let text = format!("flowchart TB\nsubgraph A[\"`{title}`\"]\n  a\nend\n");

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
    let style = merman_render::text::TextStyle {
        font_family: Some("\"trebuchet ms\", verdana, arial, sans-serif".to_string()),
        font_size: 16.0,
        font_weight: None,
    };
    let expected = merman_render::text::measure_markdown_with_flowchart_bold_deltas(
        &measurer,
        title,
        &style,
        Some(200.0),
        WrapMode::HtmlLike,
    );

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
        "%%{{init: {{\"htmlLabels\": false, \"flowchart\": {{\"htmlLabels\": false}}}}}}%%\nflowchart TB\nsubgraph A[\"`{title}`\"]\n  a\nend\n"
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
    let style = merman_render::text::TextStyle {
        font_family: Some("\"trebuchet ms\", verdana, arial, sans-serif".to_string()),
        font_size: 16.0,
        font_weight: None,
    };
    let single = measurer.measure_wrapped(title, &style, None, WrapMode::SvgLike);
    let wrapped = measurer.measure_wrapped(title, &style, Some(200.0), WrapMode::SvgLike);

    assert!(
        wrapped.height > single.height + 1e-6,
        "expected SVG-like mode to wrap long-word title"
    );
    assert!((cluster.title_label.height - wrapped.height).abs() < 1e-6);
}

#[test]
fn flowchart_relative_font_size_class_affects_node_label_layout() {
    let text = r#"%%{init: {"flowchart": {"htmlLabels": true}}}%%
flowchart LR
A[Same label]:::small
B[Same label]
classDef small font-size:50%;
"#;

    let engine = Engine::new();
    let parsed = futures::executor::block_on(engine.parse_diagram(text, ParseOptions::default()))
        .expect("parse ok")
        .expect("diagram detected");

    let out = layout_parsed(&parsed, &LayoutOptions::default()).expect("layout ok");
    let merman_render::model::LayoutDiagram::FlowchartV2(layout) = out.layout else {
        panic!("expected FlowchartV2 layout");
    };

    let node = |id: &str| {
        layout
            .nodes
            .iter()
            .find(|n| n.id == id)
            .unwrap_or_else(|| panic!("node {id}"))
    };
    let small = node("A");
    let normal = node("B");

    assert!(
        small.width + 10.0 < normal.width,
        "expected font-size:50% to reduce label-driven node width: small={}, normal={}",
        small.width,
        normal.width
    );
    assert!(
        small.height < normal.height,
        "expected font-size:50% to reduce label-driven node height: small={}, normal={}",
        small.height,
        normal.height
    );
}

#[test]
fn flowchart_whole_label_font_style_italic_affects_node_label_layout() {
    let text = r#"%%{init: {"flowchart": {"htmlLabels": true}}}%%
flowchart LR
A[Moving]:::italic
B[Moving]
classDef italic font-style:italic;
"#;

    let engine = Engine::new();
    let parsed = futures::executor::block_on(engine.parse_diagram(text, ParseOptions::default()))
        .expect("parse ok")
        .expect("diagram detected");

    let out = layout_parsed(&parsed, &LayoutOptions::default()).expect("layout ok");
    let merman_render::model::LayoutDiagram::FlowchartV2(layout) = out.layout else {
        panic!("expected FlowchartV2 layout");
    };

    let node = |id: &str| {
        layout
            .nodes
            .iter()
            .find(|n| n.id == id)
            .unwrap_or_else(|| panic!("node {id}"))
    };
    let italic = node("A");
    let normal = node("B");

    assert!(
        italic.width > normal.width + 0.5,
        "expected whole-label font-style:italic to widen label-driven node width: italic={}, normal={}",
        italic.width,
        normal.width
    );
    assert!(
        (italic.height - normal.height).abs() < 1e-6,
        "italic font-style should not change single-line node height: italic={}, normal={}",
        italic.height,
        normal.height
    );
}

#[test]
fn cyclic_subgraph_membership_reports_recoverable_error() {
    let cases = [
        (
            "self-contained",
            "flowchart TD\n  subgraph A\n    A\n  end",
            "Setting A as parent of A would create a cycle",
        ),
        (
            "two-node cycle",
            "flowchart TD\n  subgraph A\n    B\n  end\n  subgraph B\n    A\n  end",
            "Setting B as parent of A would create a cycle",
        ),
        (
            "three-node cycle",
            "flowchart TD\n  subgraph A\n    B\n  end\n  subgraph B\n    C\n  end\n  subgraph C\n    A\n  end",
            "Setting C as parent of A would create a cycle",
        ),
        (
            "four-node cycle",
            "flowchart TD\n  subgraph A\n    B\n  end\n  subgraph B\n    C\n  end\n  subgraph C\n    D\n  end\n  subgraph D\n    A\n  end",
            "Setting D as parent of A would create a cycle",
        ),
        (
            "reverse-order override cycle",
            "flowchart TD\n  subgraph A\n    B\n  end\n  subgraph X\n    A\n  end\n  subgraph B\n    X\n  end",
            "Setting X as parent of A would create a cycle",
        ),
        (
            "explicit-id title cycle",
            "flowchart TD\n  subgraph sgA[Outer A]\n    sgB\n  end\n  subgraph sgB[Inner B]\n    sgA\n  end",
            "Setting sgB as parent of sgA would create a cycle",
        ),
        (
            "cluster-edge cycle",
            "flowchart TD\n  subgraph A\n    B\n  end\n  subgraph B\n    A\n  end\n  A --> C",
            "Setting B as parent of A would create a cycle",
        ),
    ];

    let engine = Engine::new();
    for (name, text, expected_message) in cases {
        let parsed =
            futures::executor::block_on(engine.parse_diagram(text, ParseOptions::default()))
                .unwrap_or_else(|err| panic!("parse {name}: {err}"))
                .unwrap_or_else(|| panic!("diagram detected for {name}"));

        let err = match layout_parsed(&parsed, &LayoutOptions::default()) {
            Ok(_) => panic!("{name} should be a recoverable error"),
            Err(err) => err,
        };
        let Error::InvalidModel { message } = err else {
            panic!("expected InvalidModel for {name}");
        };
        assert_eq!(
            message, expected_message,
            "expected Mermaid-compatible subgraph-cycle error for {name}"
        );
    }
}

#[test]
fn non_cyclic_subgraph_membership_chain_still_lays_out() {
    let text = "flowchart TD\n  subgraph A\n    B\n  end\n  subgraph B\n    C\n  end\n  C --> D\n";
    let engine = Engine::new();
    let parsed = futures::executor::block_on(engine.parse_diagram(text, ParseOptions::default()))
        .expect("parse ok")
        .expect("diagram detected");

    let out = layout_parsed(&parsed, &LayoutOptions::default()).expect("layout ok");
    let merman_render::model::LayoutDiagram::FlowchartV2(layout) = out.layout else {
        panic!("expected FlowchartV2 layout");
    };

    let cluster_ids = layout
        .clusters
        .iter()
        .map(|c| c.id.as_str())
        .collect::<std::collections::HashSet<_>>();
    assert!(cluster_ids.contains("A"));
    assert!(cluster_ids.contains("B"));
}

#[test]
fn duplicate_subgraph_membership_with_empty_later_group_still_lays_out() {
    let text = "flowchart TD\n  subgraph A\n    B\n  end\n  subgraph X\n    B\n  end\n  B --> C\n";
    let engine = Engine::new();
    let parsed = futures::executor::block_on(engine.parse_diagram(text, ParseOptions::default()))
        .expect("parse ok")
        .expect("diagram detected");

    let out = layout_parsed(&parsed, &LayoutOptions::default()).expect("layout ok");
    let merman_render::model::LayoutDiagram::FlowchartV2(layout) = out.layout else {
        panic!("expected FlowchartV2 layout");
    };

    let cluster_ids = layout
        .clusters
        .iter()
        .map(|c| c.id.as_str())
        .collect::<std::collections::HashSet<_>>();
    let node_ids = layout
        .nodes
        .iter()
        .map(|n| n.id.as_str())
        .collect::<std::collections::HashSet<_>>();

    assert!(cluster_ids.contains("A"));
    assert!(!cluster_ids.contains("X"));
    assert!(node_ids.contains("X"));
}
