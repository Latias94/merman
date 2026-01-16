use merman_core::{Engine, ParseOptions};
use merman_render::{LayoutOptions, layout_parsed};
use std::path::PathBuf;

fn workspace_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
}

fn rect_from_node(n: &merman_render::model::LayoutNode) -> (f64, f64, f64, f64) {
    let hw = n.width / 2.0;
    let hh = n.height / 2.0;
    (n.x - hw, n.y - hh, n.x + hw, n.y + hh)
}

fn rect_from_cluster(c: &merman_render::model::LayoutCluster) -> (f64, f64, f64, f64) {
    let hw = c.width / 2.0;
    let hh = c.height / 2.0;
    (c.x - hw, c.y - hh, c.x + hw, c.y + hh)
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
fn state_layout_produces_positions_and_routes() {
    let path = workspace_root()
        .join("fixtures")
        .join("state")
        .join("basic.mmd");
    let text = std::fs::read_to_string(&path).expect("fixture");

    let engine = Engine::new();
    let parsed = futures::executor::block_on(engine.parse_diagram(&text, ParseOptions::default()))
        .expect("parse ok")
        .expect("diagram detected");

    let out = layout_parsed(&parsed, &LayoutOptions::default()).expect("layout ok");
    let merman_render::model::LayoutDiagram::StateDiagramV2(layout) = out.layout else {
        panic!("expected StateDiagramV2 layout");
    };

    assert!(layout.nodes.len() >= 3);
    assert!(layout.edges.len() >= 3);

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
fn state_start_and_end_have_fixed_size() {
    let text = "stateDiagram-v2\n[*] --> A\nA --> [*]\n";
    let engine = Engine::new();
    let parsed = futures::executor::block_on(engine.parse_diagram(text, ParseOptions::default()))
        .expect("parse ok")
        .expect("diagram detected");

    let out = layout_parsed(&parsed, &LayoutOptions::default()).expect("layout ok");
    let merman_render::model::LayoutDiagram::StateDiagramV2(layout) = out.layout else {
        panic!("expected StateDiagramV2 layout");
    };

    let mut by_id = std::collections::HashMap::new();
    for n in &layout.nodes {
        by_id.insert(n.id.as_str(), (n.width, n.height));
    }

    let (sw, sh) = by_id["root_start"];
    let (ew, eh) = by_id["root_end"];
    assert!((sw - 14.0).abs() < 1e-6 && (sh - 14.0).abs() < 1e-6);
    assert!((ew - 14.0).abs() < 1e-6 && (eh - 14.0).abs() < 1e-6);
}

#[test]
fn state_layout_note_groups_contain_notes() {
    let path = workspace_root()
        .join("fixtures")
        .join("state")
        .join("upstream_stateDiagram_v2_note_statements_spec.mmd");
    let text = std::fs::read_to_string(&path).expect("fixture");

    let engine = Engine::new();
    let parsed = futures::executor::block_on(engine.parse_diagram(&text, ParseOptions::default()))
        .expect("parse ok")
        .expect("diagram detected");

    let out = layout_parsed(&parsed, &LayoutOptions::default()).expect("layout ok");
    let merman_render::model::LayoutDiagram::StateDiagramV2(layout) = out.layout else {
        panic!("expected StateDiagramV2 layout");
    };

    let mut node_by_id = std::collections::HashMap::new();
    for n in &layout.nodes {
        node_by_id.insert(n.id.as_str(), n);
    }
    let mut cluster_by_id = std::collections::HashMap::new();
    for c in &layout.clusters {
        cluster_by_id.insert(c.id.as_str(), c);
    }

    let parent = cluster_by_id["Active----parent"];
    let note = node_by_id["Active----note-2"];

    assert!(
        rect_contains(rect_from_cluster(parent), rect_from_node(note), 1e-6),
        "note should be inside its noteGroup cluster"
    );
}

#[test]
fn state_layout_composite_and_dividers_contain_children() {
    let path = workspace_root()
        .join("fixtures")
        .join("state")
        .join("upstream_stateDiagram_v2_concurrent_state_spec.mmd");
    let text = std::fs::read_to_string(&path).expect("fixture");

    let engine = Engine::new();
    let parsed = futures::executor::block_on(engine.parse_diagram(&text, ParseOptions::default()))
        .expect("parse ok")
        .expect("diagram detected");

    let out = layout_parsed(&parsed, &LayoutOptions::default()).expect("layout ok");
    let merman_render::model::LayoutDiagram::StateDiagramV2(layout) = out.layout else {
        panic!("expected StateDiagramV2 layout");
    };

    let mut node_by_id = std::collections::HashMap::new();
    for n in &layout.nodes {
        node_by_id.insert(n.id.as_str(), n);
    }
    let mut cluster_by_id = std::collections::HashMap::new();
    for c in &layout.clusters {
        cluster_by_id.insert(c.id.as_str(), c);
    }

    let active = cluster_by_id["Active"];
    let div1 = cluster_by_id["divider-id-1"];
    let div2 = cluster_by_id["divider-id-2"];
    let div3_id = cluster_by_id
        .keys()
        .copied()
        .find(|id| id.starts_with("id-"))
        .expect("expected generated divider id (id-*)");
    let div3 = cluster_by_id[div3_id];

    let active_rect = rect_from_cluster(active);
    let div1_rect = rect_from_cluster(div1);
    let div2_rect = rect_from_cluster(div2);
    let div3_rect = rect_from_cluster(div3);

    assert!(rect_contains(active_rect, div1_rect, 1e-6));
    assert!(rect_contains(active_rect, div2_rect, 1e-6));
    assert!(rect_contains(active_rect, div3_rect, 1e-6));

    let num_lock_off = node_by_id["NumLockOff"];
    let num_lock_on = node_by_id["NumLockOn"];
    assert!(rect_contains(div1_rect, rect_from_node(num_lock_off), 1e-6));
    assert!(rect_contains(div1_rect, rect_from_node(num_lock_on), 1e-6));

    let caps_lock_off = node_by_id["CapsLockOff"];
    let caps_lock_on = node_by_id["CapsLockOn"];
    assert!(rect_contains(
        div2_rect,
        rect_from_node(caps_lock_off),
        1e-6
    ));
    assert!(rect_contains(div2_rect, rect_from_node(caps_lock_on), 1e-6));

    let scroll_lock_off = node_by_id["ScrollLockOff"];
    let scroll_lock_on = node_by_id["ScrollLockOn"];
    assert!(rect_contains(
        div3_rect,
        rect_from_node(scroll_lock_off),
        1e-6
    ));
    assert!(rect_contains(
        div3_rect,
        rect_from_node(scroll_lock_on),
        1e-6
    ));
}

#[test]
fn state_layout_merges_self_loop_edges() {
    let path = workspace_root()
        .join("fixtures")
        .join("state")
        .join("upstream_stateDiagram_v2_composite_self_link_spec.mmd");
    let text = std::fs::read_to_string(&path).expect("fixture");

    let engine = Engine::new();
    let parsed = futures::executor::block_on(engine.parse_diagram(&text, ParseOptions::default()))
        .expect("parse ok")
        .expect("diagram detected");

    let out = layout_parsed(&parsed, &LayoutOptions::default()).expect("layout ok");
    let merman_render::model::LayoutDiagram::StateDiagramV2(layout) = out.layout else {
        panic!("expected StateDiagramV2 layout");
    };

    assert_eq!(layout.edges.len(), 2);
    let self_loop = layout
        .edges
        .iter()
        .find(|e| e.id == "edge1")
        .expect("edge1");
    assert_eq!(self_loop.from, "Active");
    assert_eq!(self_loop.to, "Active");
    assert!(self_loop.points.len() >= 2);
    assert!(self_loop.label.is_some());
}
