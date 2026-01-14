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
fn class_layout_produces_positions_and_routes() {
    let path = workspace_root()
        .join("fixtures")
        .join("class")
        .join("basic.mmd");
    let text = std::fs::read_to_string(&path).expect("fixture");

    let engine = Engine::new();
    let parsed = futures::executor::block_on(engine.parse_diagram(&text, ParseOptions::default()))
        .expect("parse ok")
        .expect("diagram detected");

    let out = layout_parsed(&parsed, &LayoutOptions::default()).expect("layout ok");
    let merman_render::model::LayoutDiagram::ClassDiagramV2(layout) = out.layout else {
        panic!("expected ClassDiagramV2 layout");
    };

    assert!(layout.nodes.len() >= 2);
    assert!(layout.edges.len() >= 1);

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
fn class_namespaces_contain_member_classes() {
    let path = workspace_root()
        .join("fixtures")
        .join("class")
        .join("upstream_namespaces_and_generics.mmd");
    let text = std::fs::read_to_string(&path).expect("fixture");

    let engine = Engine::new();
    let parsed = futures::executor::block_on(engine.parse_diagram(&text, ParseOptions::default()))
        .expect("parse ok")
        .expect("diagram detected");

    let out = layout_parsed(&parsed, &LayoutOptions::default()).expect("layout ok");
    let merman_render::model::LayoutDiagram::ClassDiagramV2(layout) = out.layout else {
        panic!("expected ClassDiagramV2 layout");
    };

    let mut node_by_id = std::collections::HashMap::new();
    for n in &layout.nodes {
        if !n.is_cluster {
            node_by_id.insert(n.id.as_str(), n);
        }
    }
    let mut cluster_by_id = std::collections::HashMap::new();
    for c in &layout.clusters {
        cluster_by_id.insert(c.id.as_str(), c);
    }

    let semantic = &out.semantic;
    let Some(classes) = semantic.get("classes").and_then(|v| v.as_object()) else {
        panic!("missing semantic.classes");
    };

    for (id, cls) in classes {
        let parent = cls.get("parent").and_then(|v| v.as_str()).unwrap_or("");
        if parent.is_empty() {
            continue;
        }
        let Some(node) = node_by_id.get(id.as_str()) else {
            continue;
        };
        let Some(cluster) = cluster_by_id.get(parent) else {
            panic!("missing cluster {parent}");
        };
        assert!(
            rect_contains(rect_from_cluster(cluster), rect_from_node(node), 0.01),
            "cluster {parent} should contain {id}"
        );
    }
}

#[test]
fn class_terminal_labels_exist_for_cardinalities_fixture() {
    let path = workspace_root()
        .join("fixtures")
        .join("class")
        .join("upstream_relation_types_and_cardinalities_spec.mmd");
    let text = std::fs::read_to_string(&path).expect("fixture");

    let engine = Engine::new();
    let parsed = futures::executor::block_on(engine.parse_diagram(&text, ParseOptions::default()))
        .expect("parse ok")
        .expect("diagram detected");

    let out = layout_parsed(&parsed, &LayoutOptions::default()).expect("layout ok");
    let merman_render::model::LayoutDiagram::ClassDiagramV2(layout) = out.layout else {
        panic!("expected ClassDiagramV2 layout");
    };

    let has_terminal = layout.edges.iter().any(|e| {
        e.start_label_left.is_some()
            || e.start_label_right.is_some()
            || e.end_label_left.is_some()
            || e.end_label_right.is_some()
    });
    assert!(has_terminal, "expected at least one terminal label");
}

fn point_inside(rect: (f64, f64, f64, f64), x: f64, y: f64, eps: f64) -> bool {
    let (min_x, min_y, max_x, max_y) = rect;
    x >= min_x - eps && x <= max_x + eps && y >= min_y - eps && y <= max_y + eps
}

#[test]
fn class_terminal_labels_are_outside_endpoint_nodes_for_cardinalities_fixture() {
    let path = workspace_root()
        .join("fixtures")
        .join("class")
        .join("upstream_relation_types_and_cardinalities_spec.mmd");
    let text = std::fs::read_to_string(&path).expect("fixture");

    let engine = Engine::new();
    let parsed = futures::executor::block_on(engine.parse_diagram(&text, ParseOptions::default()))
        .expect("parse ok")
        .expect("diagram detected");

    let out = layout_parsed(&parsed, &LayoutOptions::default()).expect("layout ok");
    let merman_render::model::LayoutDiagram::ClassDiagramV2(layout) = out.layout else {
        panic!("expected ClassDiagramV2 layout");
    };

    let mut node_rect_by_id = std::collections::HashMap::new();
    for n in &layout.nodes {
        if n.is_cluster {
            continue;
        }
        node_rect_by_id.insert(n.id.as_str(), rect_from_node(n));
    }

    let eps = 0.01;
    let mut checked = 0usize;
    for e in &layout.edges {
        let Some(from_rect) = node_rect_by_id.get(e.from.as_str()) else {
            continue;
        };
        let Some(to_rect) = node_rect_by_id.get(e.to.as_str()) else {
            continue;
        };

        for lbl in [
            e.start_label_left.as_ref(),
            e.start_label_right.as_ref(),
            e.end_label_left.as_ref(),
            e.end_label_right.as_ref(),
        ] {
            let Some(lbl) = lbl else {
                continue;
            };
            checked += 1;
            assert!(
                !point_inside(*from_rect, lbl.x, lbl.y, eps),
                "terminal label center should not be inside start node for edge {}",
                e.id
            );
            assert!(
                !point_inside(*to_rect, lbl.x, lbl.y, eps),
                "terminal label center should not be inside end node for edge {}",
                e.id
            );
        }
    }
    assert!(checked > 0, "expected to check at least one terminal label");
}
