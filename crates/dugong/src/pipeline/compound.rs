//! Compound-graph helpers for layout pipelines.

use crate::graphlib;
use crate::{EdgeLabel, GraphLabel, NodeLabel};

pub(super) fn remove_border_nodes(g: &mut graphlib::Graph<NodeLabel, EdgeLabel, GraphLabel>) {
    // First pass: update compound-node geometry from its border nodes.
    let node_ids = g.node_ids();
    for v in &node_ids {
        if g.children(v).is_empty() {
            continue;
        }
        let Some(node) = g.node(v).cloned() else {
            continue;
        };
        let (Some(bt), Some(bb)) = (node.border_top.clone(), node.border_bottom.clone()) else {
            continue;
        };

        let bl = node.border_left.last().and_then(|v| v.as_ref()).cloned();
        let br = node.border_right.last().and_then(|v| v.as_ref()).cloned();
        let (Some(bl), Some(br)) = (bl, br) else {
            continue;
        };

        let Some(t) = g.node(&bt) else {
            continue;
        };
        let Some(b) = g.node(&bb) else {
            continue;
        };
        let Some(l) = g.node(&bl) else {
            continue;
        };
        let Some(r) = g.node(&br) else {
            continue;
        };

        let (Some(ty), Some(by)) = (t.y, b.y) else {
            continue;
        };
        let (Some(lx), Some(rx)) = (l.x, r.x) else {
            continue;
        };

        let width = (rx - lx).abs();
        let height = (by - ty).abs();
        if let Some(n) = g.node_mut(v) {
            n.width = width;
            n.height = height;
            n.x = Some(lx + width / 2.0);
            n.y = Some(ty + height / 2.0);
        }
    }

    // Second pass: remove all border dummy nodes.
    let mut to_remove: Vec<String> = Vec::new();
    for v in g.node_ids() {
        let Some(node) = g.node(&v) else {
            continue;
        };
        if node.dummy.as_deref() == Some("border") {
            to_remove.push(v);
        }
    }
    for v in to_remove {
        let _ = g.remove_node(&v);
    }
}
