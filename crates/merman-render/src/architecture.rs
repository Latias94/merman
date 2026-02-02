use crate::model::{ArchitectureDiagramLayout, Bounds, LayoutEdge, LayoutNode, LayoutPoint};
use crate::text::TextMeasurer;
use crate::{Error, Result};
use indexmap::IndexMap;
use serde::Deserialize;
use serde_json::Value;

fn config_f64(cfg: &Value, path: &[&str]) -> Option<f64> {
    let mut cur = cfg;
    for k in path {
        cur = cur.get(*k)?;
    }
    cur.as_f64().or_else(|| cur.as_i64().map(|v| v as f64))
}

#[derive(Debug, Clone, Deserialize)]
struct ArchitectureNodeModel {
    id: String,
    #[serde(rename = "type")]
    node_type: String,
    #[serde(default)]
    edges: Vec<ArchitectureEdgeModel>,
    #[serde(default, rename = "in")]
    in_group: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
struct ArchitectureEdgeModel {
    #[serde(rename = "lhsId", alias = "lhs")]
    lhs_id: String,
    #[serde(rename = "rhsId", alias = "rhs")]
    rhs_id: String,
    #[serde(default, rename = "lhsDir")]
    lhs_dir: Option<String>,
    #[serde(default, rename = "rhsDir")]
    rhs_dir: Option<String>,
    #[serde(default, rename = "lhsGroup")]
    lhs_group: Option<bool>,
    #[serde(default, rename = "rhsGroup")]
    rhs_group: Option<bool>,
}

#[derive(Debug, Clone, Deserialize)]
struct ArchitectureGroupModel {
    id: String,
    #[serde(default, rename = "in")]
    in_group: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
struct ArchitectureModel {
    #[serde(default)]
    nodes: Vec<ArchitectureNodeModel>,
    #[serde(default)]
    groups: Vec<ArchitectureGroupModel>,
    #[serde(default)]
    edges: Vec<ArchitectureEdgeModel>,
}

fn compute_bounds(nodes: &[LayoutNode], edges: &[LayoutEdge]) -> Option<Bounds> {
    let mut pts: Vec<(f64, f64)> = Vec::new();
    for n in nodes {
        // Architecture renderer uses top-left anchored `translate(x, y)` for nodes.
        pts.push((n.x, n.y));
        pts.push((n.x + n.width, n.y + n.height));
    }
    for e in edges {
        for p in &e.points {
            pts.push((p.x, p.y));
        }
    }
    Bounds::from_points(pts)
}

pub fn layout_architecture_diagram(
    model: &Value,
    effective_config: &Value,
    _text_measurer: &dyn TextMeasurer,
) -> Result<ArchitectureDiagramLayout> {
    let model: ArchitectureModel = serde_json::from_value(model.clone())?;

    let icon_size = config_f64(effective_config, &["architecture", "iconSize"]).unwrap_or(80.0);
    let icon_size = icon_size.max(1.0);
    let half_icon = icon_size / 2.0;
    let padding_px = config_f64(effective_config, &["architecture", "padding"]).unwrap_or(40.0);
    let padding_px = padding_px.max(0.0);
    let font_size_px = config_f64(effective_config, &["architecture", "fontSize"]).unwrap_or(16.0);
    let font_size_px = font_size_px.max(1.0);

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    enum Dir {
        L,
        R,
        T,
        B,
    }

    impl Dir {
        fn parse(s: &str) -> Option<Self> {
            match s.trim() {
                "L" => Some(Self::L),
                "R" => Some(Self::R),
                "T" => Some(Self::T),
                "B" => Some(Self::B),
                _ => None,
            }
        }

        fn is_x(self) -> bool {
            matches!(self, Self::L | Self::R)
        }
    }

    fn dir_pair_key(source: Dir, target: Dir) -> Option<&'static str> {
        match (source, target) {
            (Dir::L, Dir::R) => Some("LR"),
            (Dir::L, Dir::T) => Some("LT"),
            (Dir::L, Dir::B) => Some("LB"),
            (Dir::R, Dir::L) => Some("RL"),
            (Dir::R, Dir::T) => Some("RT"),
            (Dir::R, Dir::B) => Some("RB"),
            (Dir::T, Dir::L) => Some("TL"),
            (Dir::T, Dir::R) => Some("TR"),
            (Dir::T, Dir::B) => Some("TB"),
            (Dir::B, Dir::L) => Some("BL"),
            (Dir::B, Dir::R) => Some("BR"),
            (Dir::B, Dir::T) => Some("BT"),
            _ => None,
        }
    }

    fn shift_position_by_arch_pair(x: i32, y: i32, pair: &str) -> (i32, i32) {
        // Port of Mermaid@11.12.2 `shiftPositionByArchitectureDirectionPair`.
        let bytes = pair.as_bytes();
        if bytes.len() != 2 {
            return (x, y);
        }
        let lhs = match bytes[0] as char {
            'L' => Dir::L,
            'R' => Dir::R,
            'T' => Dir::T,
            'B' => Dir::B,
            _ => return (x, y),
        };
        let rhs = match bytes[1] as char {
            'L' => Dir::L,
            'R' => Dir::R,
            'T' => Dir::T,
            'B' => Dir::B,
            _ => return (x, y),
        };

        if lhs.is_x() {
            if !rhs.is_x() {
                (
                    x + if lhs == Dir::L { -1 } else { 1 },
                    y + if rhs == Dir::T { 1 } else { -1 },
                )
            } else {
                (x + if lhs == Dir::L { -1 } else { 1 }, y)
            }
        } else if rhs.is_x() {
            (
                x + if rhs == Dir::L { 1 } else { -1 },
                y + if lhs == Dir::T { 1 } else { -1 },
            )
        } else {
            (x, y + if lhs == Dir::T { 1 } else { -1 })
        }
    }

    let mut nodes: Vec<LayoutNode> = Vec::new();

    // Build adjacency list in Mermaid's insertion order:
    // - Outer order: `model.nodes` order.
    // - Inner order: `node.edges` list order, preserving first-insertion order for each dir pair.
    let mut adjacency: std::collections::HashMap<String, IndexMap<&'static str, String>> =
        std::collections::HashMap::new();
    for n in &model.nodes {
        adjacency.insert(n.id.clone(), IndexMap::new());
    }

    for n in &model.nodes {
        let entry = adjacency.entry(n.id.clone()).or_default();
        for e in &n.edges {
            let (Some(lhs_dir), Some(rhs_dir)) = (
                e.lhs_dir.as_deref().and_then(Dir::parse),
                e.rhs_dir.as_deref().and_then(Dir::parse),
            ) else {
                continue;
            };

            if e.lhs_id == n.id {
                if let Some(pair) = dir_pair_key(lhs_dir, rhs_dir) {
                    // Preserve insertion order for the pair key; overwrites keep the original slot.
                    entry.insert(pair, e.rhs_id.clone());
                }
            } else if e.rhs_id == n.id {
                if let Some(pair) = dir_pair_key(rhs_dir, lhs_dir) {
                    entry.insert(pair, e.lhs_id.clone());
                }
            }
        }
    }

    // Mermaid's Architecture layout uses Cytoscape FCoSE with constraints derived from BFS spatial
    // maps. As a deterministic scaffold (pre-FCoSE port), we reproduce the BFS spatial maps and
    // place nodes on a grid in a way that is close to upstream fixtures.
    //
    // IMPORTANT: `shiftPositionByArchitectureDirectionPair` uses a y-up convention; when mapping
    // to SVG coordinates we invert the sign to keep y-down in pixel space.
    let mut components: Vec<std::collections::BTreeMap<String, (i32, i32)>> = Vec::new();

    // Deterministic component discovery: mimic Mermaid's `Object.keys(notVisited)[0]` by walking
    // `node_order` and taking the first not-yet-assigned id for each component.
    let node_order: Vec<String> = model.nodes.iter().map(|n| n.id.clone()).collect();
    let mut assigned: std::collections::HashSet<String> = std::collections::HashSet::new();
    for start_id in &node_order {
        if assigned.contains(start_id) {
            continue;
        }
        // BFS over this component, assigning coordinates.
        let mut spatial: std::collections::BTreeMap<String, (i32, i32)> =
            std::collections::BTreeMap::new();
        use std::collections::VecDeque;
        let mut q: VecDeque<String> = VecDeque::new();
        spatial.insert(start_id.clone(), (0, 0));
        q.push_back(start_id.clone());
        assigned.insert(start_id.clone());

        while let Some(id) = q.pop_front() {
            let Some(&(x, y)) = spatial.get(&id) else {
                continue;
            };
            let Some(adj) = adjacency.get(&id) else {
                continue;
            };
            for (pair, rhs_id) in adj.iter() {
                if spatial.contains_key(rhs_id) {
                    continue;
                }
                let (nx, ny) = shift_position_by_arch_pair(x, y, pair);
                spatial.insert(rhs_id.clone(), (nx, ny));
                q.push_back(rhs_id.clone());
                assigned.insert(rhs_id.clone());
            }
        }

        components.push(spatial);
    }

    // Grid step heuristic: close to Mermaid@11.12.2 outputs for default config.
    let grid_step = (icon_size + 3.0 * padding_px).max(icon_size);
    let component_gap = (grid_step / 2.0).max(1.0);

    // Convert grid coords to pixel coords, lay out disconnected components left-to-right.
    let mut pos_px: std::collections::HashMap<String, (f64, f64)> =
        std::collections::HashMap::new();
    let mut offset_x = 0.0f64;
    for spatial in &components {
        // Compute component bbox in pixel space before offset.
        let mut min_x = f64::INFINITY;
        let mut max_x = f64::NEG_INFINITY;

        for (id, (gx, gy)) in spatial {
            let x = (*gx as f64) * grid_step;
            let y = -(*gy as f64) * grid_step;
            min_x = min_x.min(x);
            max_x = max_x.max(x + icon_size);
            pos_px.insert(id.clone(), (x, y));
        }

        // Apply component offset.
        let dx = offset_x - min_x;
        for (id, _coords) in spatial {
            let (x, y) = pos_px.get(id).copied().unwrap_or((0.0, 0.0));
            pos_px.insert(id.clone(), (x + dx, y));
        }

        offset_x += (max_x - min_x) + component_gap;
    }

    // Global centering heuristic:
    // - X: center the node centers around `half_icon` (matches Mermaid's group/icon shifts).
    // - Y: if groups exist, bias down by ~1 line to leave room for group headers.
    let mut min_cx = f64::INFINITY;
    let mut max_cx = f64::NEG_INFINITY;
    let mut min_cy = f64::INFINITY;
    let mut max_cy = f64::NEG_INFINITY;
    for (_id, (x, y)) in &pos_px {
        let cx = x + half_icon;
        let cy = y + half_icon;
        min_cx = min_cx.min(cx);
        max_cx = max_cx.max(cx);
        min_cy = min_cy.min(cy);
        max_cy = max_cy.max(cy);
    }
    let center_cx = if min_cx.is_finite() && max_cx.is_finite() {
        (min_cx + max_cx) / 2.0
    } else {
        half_icon
    };
    let center_cy = if min_cy.is_finite() && max_cy.is_finite() {
        (min_cy + max_cy) / 2.0
    } else {
        half_icon
    };

    let has_group_header = !model.groups.is_empty();
    let target_cx = half_icon;
    let target_cy = half_icon + if has_group_header { font_size_px } else { 0.0 };
    let shift_x = target_cx - center_cx;
    let shift_y = target_cy - center_cy;

    // Emit nodes in Mermaid model order (stable for snapshots and close to upstream).
    for n in &model.nodes {
        match n.node_type.as_str() {
            "service" | "junction" => {}
            other => {
                return Err(Error::InvalidModel {
                    message: format!("unsupported architecture node type: {other}"),
                });
            }
        }

        let (x, y) = pos_px.get(&n.id).copied().unwrap_or((0.0, 0.0));
        nodes.push(LayoutNode {
            id: n.id.clone(),
            x: x + shift_x,
            y: y + shift_y,
            width: icon_size,
            height: icon_size,
            is_cluster: false,
        });
    }

    let mut node_by_id: std::collections::BTreeMap<String, LayoutNode> =
        std::collections::BTreeMap::new();
    for n in &nodes {
        node_by_id.insert(n.id.clone(), n.clone());
    }

    let mut edges: Vec<LayoutEdge> = Vec::new();
    for (idx, e) in model.edges.iter().enumerate() {
        let Some(a) = node_by_id.get(&e.lhs_id) else {
            return Err(Error::InvalidModel {
                message: format!("edge lhs node not found: {}", e.lhs_id),
            });
        };
        let Some(b) = node_by_id.get(&e.rhs_id) else {
            return Err(Error::InvalidModel {
                message: format!("edge rhs node not found: {}", e.rhs_id),
            });
        };

        fn endpoint(
            x: f64,
            y: f64,
            dir: Option<&str>,
            icon_size: f64,
            half_icon: f64,
        ) -> (f64, f64) {
            match dir.unwrap_or("") {
                "L" => (x, y + half_icon),
                "R" => (x + icon_size, y + half_icon),
                "T" => (x + half_icon, y),
                "B" => (x + half_icon, y + icon_size),
                _ => (x + half_icon, y + half_icon),
            }
        }

        let (sx, sy) = endpoint(a.x, a.y, e.lhs_dir.as_deref(), icon_size, half_icon);
        let (tx, ty) = endpoint(b.x, b.y, e.rhs_dir.as_deref(), icon_size, half_icon);
        let mid = LayoutPoint {
            x: (sx + tx) / 2.0,
            y: (sy + ty) / 2.0,
        };
        edges.push(LayoutEdge {
            id: format!("edge-{idx}"),
            from: e.lhs_id.clone(),
            to: e.rhs_id.clone(),
            from_cluster: None,
            to_cluster: None,
            points: vec![
                LayoutPoint { x: sx, y: sy },
                mid,
                LayoutPoint { x: tx, y: ty },
            ],
            label: None,
            start_label_left: None,
            start_label_right: None,
            end_label_left: None,
            end_label_right: None,
            start_marker: None,
            end_marker: None,
            stroke_dasharray: None,
        });
    }

    Ok(ArchitectureDiagramLayout {
        bounds: compute_bounds(&nodes, &edges),
        nodes,
        edges,
    })
}
