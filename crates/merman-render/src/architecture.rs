use crate::json::from_value_ref;
use crate::model::{ArchitectureDiagramLayout, Bounds, LayoutEdge, LayoutNode, LayoutPoint};
use crate::text::TextMeasurer;
use crate::{Error, Result};
use indexmap::IndexMap;
use merman_core::diagrams::architecture::ArchitectureDiagramRenderModel;
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
    #[serde(default)]
    #[allow(dead_code)]
    icon: Option<String>,
    #[serde(default)]
    title: Option<String>,
    #[serde(default, rename = "iconText")]
    #[allow(dead_code)]
    icon_text: Option<String>,
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

impl ArchitectureModel {
    fn from_typed(model: &ArchitectureDiagramRenderModel) -> Self {
        fn dir_to_string(d: char) -> Option<String> {
            Some(d.to_string())
        }

        let edges: Vec<ArchitectureEdgeModel> = model
            .edges
            .iter()
            .map(|e| ArchitectureEdgeModel {
                lhs_id: e.lhs_id.clone(),
                rhs_id: e.rhs_id.clone(),
                lhs_dir: dir_to_string(e.lhs_dir),
                rhs_dir: dir_to_string(e.rhs_dir),
                lhs_group: e.lhs_group,
                rhs_group: e.rhs_group,
            })
            .collect();

        let mut nodes: Vec<ArchitectureNodeModel> = Vec::with_capacity(model.nodes.len());
        for n in &model.nodes {
            let node_type = match n.node_type {
                merman_core::diagrams::architecture::ArchitectureRenderNodeType::Service => {
                    "service"
                }
                merman_core::diagrams::architecture::ArchitectureRenderNodeType::Junction => {
                    "junction"
                }
            }
            .to_string();

            let mut node_edges: Vec<ArchitectureEdgeModel> =
                Vec::with_capacity(n.edge_indices.len());
            for &idx in &n.edge_indices {
                let Some(e) = model.edges.get(idx) else {
                    continue;
                };
                node_edges.push(ArchitectureEdgeModel {
                    lhs_id: e.lhs_id.clone(),
                    rhs_id: e.rhs_id.clone(),
                    lhs_dir: dir_to_string(e.lhs_dir),
                    rhs_dir: dir_to_string(e.rhs_dir),
                    lhs_group: e.lhs_group,
                    rhs_group: e.rhs_group,
                });
            }

            nodes.push(ArchitectureNodeModel {
                id: n.id.clone(),
                node_type,
                edges: node_edges,
                icon: n.icon.clone(),
                title: n.title.clone(),
                icon_text: n.icon_text.clone(),
                in_group: n.in_group.clone(),
            });
        }

        let groups: Vec<ArchitectureGroupModel> = model
            .groups
            .iter()
            .map(|g| ArchitectureGroupModel {
                id: g.id.clone(),
                in_group: g.in_group.clone(),
            })
            .collect();

        Self {
            nodes,
            groups,
            edges,
        }
    }
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
    use_manatee_layout: bool,
) -> Result<ArchitectureDiagramLayout> {
    let model: ArchitectureModel = from_value_ref(model)?;
    layout_architecture_diagram_model(&model, effective_config, _text_measurer, use_manatee_layout)
}

pub fn layout_architecture_diagram_typed(
    model: &ArchitectureDiagramRenderModel,
    effective_config: &Value,
    text_measurer: &dyn TextMeasurer,
    use_manatee_layout: bool,
) -> Result<ArchitectureDiagramLayout> {
    let model = ArchitectureModel::from_typed(model);
    layout_architecture_diagram_model(&model, effective_config, text_measurer, use_manatee_layout)
}

fn layout_architecture_diagram_model(
    model: &ArchitectureModel,
    effective_config: &Value,
    _text_measurer: &dyn TextMeasurer,
    use_manatee_layout: bool,
) -> Result<ArchitectureDiagramLayout> {
    let timing_enabled = std::env::var("MERMAN_ARCHITECTURE_LAYOUT_TIMING")
        .ok()
        .as_deref()
        == Some("1");
    #[derive(Debug, Default, Clone)]
    struct ArchitectureLayoutTimings {
        total: std::time::Duration,
        build_adjacency_and_components: std::time::Duration,
        positions_and_centering: std::time::Duration,
        emit_nodes: std::time::Duration,
        manatee_prepare: std::time::Duration,
        manatee_layout: std::time::Duration,
        group_separation: std::time::Duration,
        build_edges: std::time::Duration,
        bounds: std::time::Duration,
    }
    let mut timings = ArchitectureLayoutTimings::default();
    let total_start = timing_enabled.then(std::time::Instant::now);

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

    let build_adjacency_start = timing_enabled.then(std::time::Instant::now);

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
    if let Some(s) = build_adjacency_start {
        timings.build_adjacency_and_components = s.elapsed();
    }

    let positions_start = timing_enabled.then(std::time::Instant::now);

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
        for id in spatial.keys() {
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
    for (x, y) in pos_px.values() {
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
    if let Some(s) = positions_start {
        timings.positions_and_centering = s.elapsed();
    }

    // Emit nodes in Mermaid model order (stable for snapshots and close to upstream).
    let emit_nodes_start = timing_enabled.then(std::time::Instant::now);
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
            label_width: None,
            label_height: None,
        });
    }
    if let Some(s) = emit_nodes_start {
        timings.emit_nodes = s.elapsed();
    }

    if use_manatee_layout && !nodes.is_empty() {
        let manatee_prepare_start = timing_enabled.then(std::time::Instant::now);

        // Build Mermaid-like FCoSE constraints from the BFS spatial maps.
        //
        // The full Mermaid renderer uses Cytoscape + FCoSE, which internally combines spectral
        // initialization with a CoSE force-directed refinement step subject to the constraints.
        //
        // `manatee` contains our Rust port entry point; for now we feed it the deterministic BFS
        // grid as initial positions so the subsequent refinement stays stable and fixture-friendly.
        let mut node_group: std::collections::BTreeMap<&str, Option<&str>> =
            std::collections::BTreeMap::new();
        for n in &model.nodes {
            node_group.insert(n.id.as_str(), n.in_group.as_deref());
        }

        // Mermaid Architecture junction nodes are "invisible" routing helpers. In the upstream
        // Cytoscape model they live inside groups (compound nodes) when they are semantically
        // attached to grouped services.
        //
        // Our semantic model does not always carry explicit `in_group` for junction nodes, so we
        // infer it from incident non-junction neighbors:
        // - pick the unique group if there is exactly one
        // - otherwise pick the most frequent group (skip ties)
        let junction_ids: std::collections::BTreeSet<&str> = model
            .nodes
            .iter()
            .filter(|n| n.node_type == "junction")
            .map(|n| n.id.as_str())
            .collect();
        if !junction_ids.is_empty() {
            let mut neighbors: std::collections::BTreeMap<&str, Vec<&str>> =
                std::collections::BTreeMap::new();
            for e in &model.edges {
                neighbors
                    .entry(e.lhs_id.as_str())
                    .or_default()
                    .push(e.rhs_id.as_str());
                neighbors
                    .entry(e.rhs_id.as_str())
                    .or_default()
                    .push(e.lhs_id.as_str());
            }

            for j in &junction_ids {
                if node_group.get(j).and_then(|v| *v).is_some() {
                    continue;
                }
                let Some(neigh) = neighbors.get(j).map(|v| v.as_slice()) else {
                    continue;
                };

                let mut counts: std::collections::BTreeMap<&str, usize> =
                    std::collections::BTreeMap::new();
                for &other in neigh {
                    if junction_ids.contains(other) {
                        continue;
                    }
                    let Some(g) = node_group.get(other).and_then(|v| *v) else {
                        continue;
                    };
                    *counts.entry(g).or_insert(0) += 1;
                }
                if counts.is_empty() {
                    continue;
                }
                let mut best_group: Option<&str> = None;
                let mut best_count: usize = 0;
                let mut tied = false;
                for (g, c) in counts {
                    match c.cmp(&best_count) {
                        std::cmp::Ordering::Greater => {
                            best_group = Some(g);
                            best_count = c;
                            tied = false;
                        }
                        std::cmp::Ordering::Equal => {
                            tied = true;
                        }
                        std::cmp::Ordering::Less => {}
                    }
                }
                if !tied {
                    if let Some(g) = best_group {
                        node_group.insert(j, Some(g));
                    }
                }
            }
        }

        #[derive(Debug, Clone, Copy, PartialEq, Eq)]
        enum GroupAlignment {
            Horizontal,
            Vertical,
            Bend,
        }

        fn dir_alignment(a: Option<&str>, b: Option<&str>) -> GroupAlignment {
            let (Some(a), Some(b)) = (a.and_then(Dir::parse), b.and_then(Dir::parse)) else {
                return GroupAlignment::Bend;
            };
            if a.is_x() != b.is_x() {
                GroupAlignment::Bend
            } else if a.is_x() {
                GroupAlignment::Horizontal
            } else {
                GroupAlignment::Vertical
            }
        }

        // Track how groups connect (used when flattening alignment arrays across groups).
        let mut group_alignments: std::collections::BTreeMap<
            String,
            std::collections::BTreeMap<String, GroupAlignment>,
        > = std::collections::BTreeMap::new();
        for e in &model.edges {
            let Some(lhs_group) = node_group.get(e.lhs_id.as_str()).and_then(|v| *v) else {
                continue;
            };
            let Some(rhs_group) = node_group.get(e.rhs_id.as_str()).and_then(|v| *v) else {
                continue;
            };
            if lhs_group == rhs_group {
                continue;
            }
            let alignment = dir_alignment(e.lhs_dir.as_deref(), e.rhs_dir.as_deref());
            if alignment == GroupAlignment::Bend {
                continue;
            }
            group_alignments
                .entry(lhs_group.to_string())
                .or_default()
                .insert(rhs_group.to_string(), alignment);
            group_alignments
                .entry(rhs_group.to_string())
                .or_default()
                .insert(lhs_group.to_string(), alignment);
        }

        fn flatten_alignments(
            alignment_obj: &std::collections::BTreeMap<
                i32,
                std::collections::BTreeMap<String, Vec<String>>,
            >,
            alignment_dir: GroupAlignment,
            group_alignments: &std::collections::BTreeMap<
                String,
                std::collections::BTreeMap<String, GroupAlignment>,
            >,
        ) -> std::collections::BTreeMap<String, Vec<String>> {
            let mut prev: std::collections::BTreeMap<String, Vec<String>> =
                std::collections::BTreeMap::new();
            for (dir, alignments) in alignment_obj {
                let mut cnt = 0usize;
                let mut arr: Vec<(String, Vec<String>)> = alignments
                    .iter()
                    .map(|(k, v)| (k.clone(), v.clone()))
                    .collect();
                if arr.len() == 1 {
                    prev.insert(dir.to_string(), arr.pop().unwrap().1);
                    continue;
                }
                for i in 0..arr.len().saturating_sub(1) {
                    for j in (i + 1)..arr.len() {
                        let (a_group_id, a_node_ids) = &arr[i];
                        let (b_group_id, b_node_ids) = &arr[j];
                        let alignment = group_alignments
                            .get(a_group_id)
                            .and_then(|m| m.get(b_group_id))
                            .copied();

                        if alignment == Some(alignment_dir)
                            || a_group_id == "default"
                            || b_group_id == "default"
                        {
                            prev.entry(dir.to_string())
                                .or_default()
                                .extend(a_node_ids.iter().cloned());
                            prev.entry(dir.to_string())
                                .or_default()
                                .extend(b_node_ids.iter().cloned());
                        } else {
                            let key_a = format!("{dir}-{cnt}");
                            cnt += 1;
                            prev.insert(key_a, a_node_ids.clone());
                            let key_b = format!("{dir}-{cnt}");
                            cnt += 1;
                            prev.insert(key_b, b_node_ids.clone());
                        }
                    }
                }
            }
            prev
        }

        // Build spatial maps in Mermaid's coordinate space (y-up), keyed by node id.
        let spatial_maps: Vec<std::collections::BTreeMap<String, (i32, i32)>> = components.clone();

        // AlignmentConstraint.
        let mut horizontal_all: Vec<Vec<String>> = Vec::new();
        let mut vertical_all: Vec<Vec<String>> = Vec::new();
        for spatial_map in &spatial_maps {
            let mut horizontal_alignments: std::collections::BTreeMap<
                i32,
                std::collections::BTreeMap<String, Vec<String>>,
            > = std::collections::BTreeMap::new();
            let mut vertical_alignments: std::collections::BTreeMap<
                i32,
                std::collections::BTreeMap<String, Vec<String>>,
            > = std::collections::BTreeMap::new();

            for (id, (x, y)) in spatial_map {
                let node_group = node_group
                    .get(id.as_str())
                    .and_then(|v| *v)
                    .unwrap_or("default")
                    .to_string();

                horizontal_alignments
                    .entry(*y)
                    .or_default()
                    .entry(node_group.clone())
                    .or_default()
                    .push(id.clone());

                vertical_alignments
                    .entry(*x)
                    .or_default()
                    .entry(node_group)
                    .or_default()
                    .push(id.clone());
            }

            let horiz_map = flatten_alignments(
                &horizontal_alignments,
                GroupAlignment::Horizontal,
                &group_alignments,
            );
            let vert_map = flatten_alignments(
                &vertical_alignments,
                GroupAlignment::Vertical,
                &group_alignments,
            );

            for v in horiz_map.values() {
                if v.len() > 1 {
                    horizontal_all.push(v.clone());
                }
            }
            for v in vert_map.values() {
                if v.len() > 1 {
                    vertical_all.push(v.clone());
                }
            }
        }

        // RelativePlacementConstraint (gap between borders).
        let mut relative: Vec<manatee::RelativePlacementConstraint> = Vec::new();
        for spatial_map in &spatial_maps {
            let mut inv: std::collections::BTreeMap<(i32, i32), String> =
                std::collections::BTreeMap::new();
            for (id, (x, y)) in spatial_map {
                inv.insert((*x, *y), id.clone());
            }

            let mut queue: std::collections::VecDeque<(i32, i32)> =
                std::collections::VecDeque::new();
            let mut visited: std::collections::BTreeSet<(i32, i32)> =
                std::collections::BTreeSet::new();
            queue.push_back((0, 0));

            let dirs: [(&str, (i32, i32)); 4] =
                [("L", (-1, 0)), ("R", (1, 0)), ("T", (0, 1)), ("B", (0, -1))];

            while let Some(curr) = queue.pop_front() {
                if visited.contains(&curr) {
                    continue;
                }
                visited.insert(curr);
                let Some(curr_id) = inv.get(&curr).cloned() else {
                    continue;
                };

                for (dir, (dx, dy)) in dirs {
                    let np = (curr.0 + dx, curr.1 + dy);
                    let Some(new_id) = inv.get(&np).cloned() else {
                        continue;
                    };
                    if visited.contains(&np) {
                        continue;
                    }
                    queue.push_back(np);

                    let gap = 1.5 * icon_size;
                    match dir {
                        "L" => relative.push(manatee::RelativePlacementConstraint {
                            left: Some(new_id),
                            right: Some(curr_id.clone()),
                            top: None,
                            bottom: None,
                            gap,
                        }),
                        "R" => relative.push(manatee::RelativePlacementConstraint {
                            left: Some(curr_id.clone()),
                            right: Some(new_id),
                            top: None,
                            bottom: None,
                            gap,
                        }),
                        "T" => relative.push(manatee::RelativePlacementConstraint {
                            left: None,
                            right: None,
                            top: Some(new_id),
                            bottom: Some(curr_id.clone()),
                            gap,
                        }),
                        "B" => relative.push(manatee::RelativePlacementConstraint {
                            left: None,
                            right: None,
                            top: Some(curr_id.clone()),
                            bottom: Some(new_id),
                            gap,
                        }),
                        _ => {}
                    }
                }
            }
        }

        // Run `manatee` layout refinement.
        // Ports of layout-base/CoSE constants used by Cytoscape FCoSE.
        const SIMPLE_NODE_SIZE: f64 = 40.0; // layout-base `LayoutConstants.SIMPLE_NODE_SIZE`
        const NESTING_FACTOR: f64 = 0.1; // cytoscape-fcose default `nestingFactor`

        let mut group_parent_map: std::collections::BTreeMap<&str, &str> =
            std::collections::BTreeMap::new();
        for g in &model.groups {
            if let Some(parent) = g.in_group.as_deref() {
                group_parent_map.insert(g.id.as_str(), parent);
            }
        }

        fn group_chain<'a>(
            mut g: &'a str,
            group_parent_map: &std::collections::BTreeMap<&'a str, &'a str>,
        ) -> Vec<&'a str> {
            let mut out: Vec<&'a str> = Vec::new();
            out.push(g);
            while let Some(p) = group_parent_map.get(g).copied() {
                g = p;
                out.push(g);
            }
            out.reverse();
            out
        }

        let mut node_chain: std::collections::BTreeMap<&str, Vec<&str>> =
            std::collections::BTreeMap::new();
        for n in &model.nodes {
            let chain = node_group
                .get(n.id.as_str())
                .copied()
                .flatten()
                .map(|g| group_chain(g, &group_parent_map))
                .unwrap_or_default();
            node_chain.insert(n.id.as_str(), chain);
        }

        #[derive(Debug, Clone, Copy)]
        enum Child<'a> {
            Node(#[allow(dead_code)] &'a str),
            Group(&'a str),
        }

        let mut group_children: std::collections::BTreeMap<&str, Vec<Child<'_>>> =
            std::collections::BTreeMap::new();
        for g in &model.groups {
            group_children.entry(g.id.as_str()).or_default();
        }
        for g in &model.groups {
            if let Some(parent) = g.in_group.as_deref() {
                group_children
                    .entry(parent)
                    .or_default()
                    .push(Child::Group(g.id.as_str()));
            }
        }
        for n in &model.nodes {
            if let Some(parent) = node_group.get(n.id.as_str()).copied().flatten() {
                group_children
                    .entry(parent)
                    .or_default()
                    .push(Child::Node(n.id.as_str()));
            }
        }

        let mut group_estimated_size: std::collections::BTreeMap<&str, f64> =
            std::collections::BTreeMap::new();
        fn estimated_size_of_group<'a>(
            g: &'a str,
            icon_size: f64,
            group_children: &std::collections::BTreeMap<&'a str, Vec<Child<'a>>>,
            memo: &mut std::collections::BTreeMap<&'a str, f64>,
        ) -> f64 {
            if let Some(v) = memo.get(g).copied() {
                return v;
            }
            let children = group_children.get(g).map(|v| v.as_slice()).unwrap_or(&[]);
            if children.is_empty() {
                // layout-base `LayoutConstants.EMPTY_COMPOUND_NODE_SIZE`
                memo.insert(g, SIMPLE_NODE_SIZE);
                return SIMPLE_NODE_SIZE;
            }

            let mut sum = 0.0f64;
            let mut cnt = 0.0f64;
            for c in children {
                let s = match c {
                    Child::Node(_) => icon_size,
                    Child::Group(id) => {
                        estimated_size_of_group(id, icon_size, group_children, memo)
                    }
                };
                sum += s;
                cnt += 1.0;
            }
            let out = if cnt > 0.0 {
                sum / cnt.sqrt()
            } else {
                SIMPLE_NODE_SIZE
            };
            memo.insert(g, out);
            out
        }
        for g in &model.groups {
            let _ = estimated_size_of_group(
                g.id.as_str(),
                icon_size,
                &group_children,
                &mut group_estimated_size,
            );
        }

        let mut edge_seen: std::collections::BTreeSet<(String, String)> =
            std::collections::BTreeSet::new();
        let mut edges: Vec<manatee::Edge> = Vec::new();
        let mut default_edge_length_sum = 0.0f64;
        let mut default_edge_length_cnt = 0.0f64;

        for e in &model.edges {
            let (u, v) = if e.lhs_id <= e.rhs_id {
                (e.lhs_id.clone(), e.rhs_id.clone())
            } else {
                (e.rhs_id.clone(), e.lhs_id.clone())
            };
            if !edge_seen.insert((u, v)) {
                continue;
            }

            let lhs_g = node_group.get(e.lhs_id.as_str()).and_then(|v| *v);
            let rhs_g = node_group.get(e.rhs_id.as_str()).and_then(|v| *v);
            let same_parent = lhs_g == rhs_g;

            let base_ideal_length = if same_parent {
                1.5 * icon_size
            } else {
                0.5 * icon_size
            };
            default_edge_length_sum += base_ideal_length;
            default_edge_length_cnt += 1.0;

            // Mimic layout-base `FDLayout.calcIdealEdgeLengths()` adjustments for inter-graph edges:
            //
            // - smart ideal edge length: add estimated sizes of the LCA-level endpoints
            // - nesting factor: scale with inclusion tree depth delta
            let ideal_length = if same_parent {
                base_ideal_length
            } else {
                let chain_a = node_chain
                    .get(e.lhs_id.as_str())
                    .map(|v| v.as_slice())
                    .unwrap_or(&[]);
                let chain_b = node_chain
                    .get(e.rhs_id.as_str())
                    .map(|v| v.as_slice())
                    .unwrap_or(&[]);

                let common_len = chain_a
                    .iter()
                    .zip(chain_b.iter())
                    .take_while(|(a, b)| a == b)
                    .count();

                // `edge.getSourceInLca()` / `edge.getTargetInLca()` equivalents.
                let source_in_lca = if chain_a.len() == common_len {
                    e.lhs_id.as_str()
                } else {
                    chain_a[common_len]
                };
                let target_in_lca = if chain_b.len() == common_len {
                    e.rhs_id.as_str()
                } else {
                    chain_b[common_len]
                };

                let est = |id: &str| -> f64 {
                    if group_estimated_size.contains_key(id) {
                        group_estimated_size
                            .get(id)
                            .copied()
                            .unwrap_or(SIMPLE_NODE_SIZE)
                    } else {
                        // Leaf nodes are uniform `icon_size x icon_size` for Architecture.
                        icon_size
                    }
                };

                let size_a = est(source_in_lca);
                let size_b = est(target_in_lca);
                let smart_add = size_a + size_b - (2.0 * SIMPLE_NODE_SIZE);

                let node_depth_a = chain_a.len() + 1; // root graph nodes start at depth=1
                let node_depth_b = chain_b.len() + 1;
                let lca_depth = if common_len == 0 { 1 } else { common_len };
                let depth_delta = (node_depth_a + node_depth_b).saturating_sub(2 * lca_depth);
                let nesting_add = base_ideal_length * NESTING_FACTOR * (depth_delta as f64);

                base_ideal_length + smart_add + nesting_add
            };

            let elasticity = if same_parent { 0.45 } else { 0.001 };

            let source_anchor = e.lhs_dir.as_deref().and_then(Dir::parse).map(|d| match d {
                Dir::L => manatee::Anchor::Left,
                Dir::R => manatee::Anchor::Right,
                Dir::T => manatee::Anchor::Top,
                Dir::B => manatee::Anchor::Bottom,
            });
            let target_anchor = e.rhs_dir.as_deref().and_then(Dir::parse).map(|d| match d {
                Dir::L => manatee::Anchor::Left,
                Dir::R => manatee::Anchor::Right,
                Dir::T => manatee::Anchor::Top,
                Dir::B => manatee::Anchor::Bottom,
            });
            edges.push(manatee::Edge {
                id: format!("edge-{}", edges.len()),
                source: e.lhs_id.clone(),
                target: e.rhs_id.clone(),
                source_anchor,
                target_anchor,
                ideal_length,
                elasticity,
            });
        }

        let default_edge_length = if default_edge_length_cnt > 0.0 {
            default_edge_length_sum / default_edge_length_cnt
        } else {
            50.0
        };

        let graph = manatee::Graph {
            nodes: nodes
                .iter()
                .map(|n| manatee::Node {
                    id: n.id.clone(),
                    parent: node_group
                        .get(n.id.as_str())
                        .copied()
                        .flatten()
                        .map(|g| g.to_string()),
                    width: n.width,
                    height: n.height,
                    x: n.x + n.width / 2.0,
                    y: n.y + n.height / 2.0,
                })
                .collect(),
            edges,
            compounds: model
                .groups
                .iter()
                .map(|g| manatee::Compound {
                    id: g.id.clone(),
                    parent: g.in_group.clone(),
                })
                .collect(),
        };

        let opts = manatee::FcoseOptions {
            alignment_constraint: Some(manatee::AlignmentConstraint {
                horizontal: horizontal_all,
                vertical: vertical_all,
            }),
            relative_placement_constraint: relative,
            default_edge_length: Some(default_edge_length),
            compound_padding: Some(padding_px),
            // Mermaid@11.12.2 Architecture layout uses Cytoscape FCoSE with a spectral
            // initialization that depends on `Math.random()`. Our upstream SVG baselines are
            // generated with a deterministic RNG seed (see ADR-0055), so we must use the same
            // seed here to match those baselines.
            random_seed: 1,
        };

        if let Some(s) = manatee_prepare_start {
            timings.manatee_prepare = s.elapsed();
        }

        let manatee_layout_start = timing_enabled.then(std::time::Instant::now);
        let result = manatee::layout(&graph, manatee::Algorithm::Fcose(opts)).map_err(|e| {
            Error::InvalidModel {
                message: format!("manatee layout failed: {e}"),
            }
        })?;
        if let Some(s) = manatee_layout_start {
            timings.manatee_layout = s.elapsed();
        }

        for n in &mut nodes {
            if let Some(p) = result.positions.get(n.id.as_str()) {
                n.x = p.x - n.width / 2.0;
                n.y = p.y - n.height / 2.0;
            }
        }

        // Approximate Cytoscape compound node behavior for Architecture groups:
        //
        // Mermaid uses Cytoscape FCoSE with compound nodes (groups). Without explicit compound node
        // mechanics, leaf nodes from different groups can collapse into the same coordinate frame,
        // which then dominates `parity-root` viewport mismatches.
        //
        // Here we apply a deterministic post-pass that enforces *top-level* group separation based
        // on inter-group edge directions (e.g. `groupA:R -- L:groupB` implies `groupA` is left of
        // `groupB`). We move the entire groups (all descendant nodes) together to preserve each
        // group's internal relative placements.
        fn resolve_top_level_group_separation(
            nodes: &mut [LayoutNode],
            model: &ArchitectureModel,
            icon_size: f64,
            padding_px: f64,
            font_size_px: f64,
        ) {
            let node_type: std::collections::BTreeMap<&str, &str> = model
                .nodes
                .iter()
                .map(|n| (n.id.as_str(), n.node_type.as_str()))
                .collect();
            let node_title: std::collections::BTreeMap<&str, &str> = model
                .nodes
                .iter()
                .filter_map(|n| n.title.as_deref().map(|t| (n.id.as_str(), t)))
                .collect();

            let mut group_parent: std::collections::BTreeMap<&str, &str> =
                std::collections::BTreeMap::new();
            for g in &model.groups {
                if let Some(parent) = g.in_group.as_deref() {
                    group_parent.insert(g.id.as_str(), parent);
                }
            }

            fn root_group<'a>(
                mut g: &'a str,
                group_parent: &std::collections::BTreeMap<&'a str, &'a str>,
            ) -> &'a str {
                while let Some(p) = group_parent.get(g).copied() {
                    g = p;
                }
                g
            }

            let mut node_root_group: std::collections::BTreeMap<&str, &str> =
                std::collections::BTreeMap::new();
            for n in &model.nodes {
                if let Some(g) = n.in_group.as_deref() {
                    let root = root_group(g, &group_parent);
                    node_root_group.insert(n.id.as_str(), root);
                }
            }

            #[derive(Debug, Clone)]
            enum GroupRel {
                LeftOf {
                    left: String,
                    right: String,
                    gap: f64,
                },
                Above {
                    top: String,
                    bottom: String,
                    gap: f64,
                },
                AlignTop {
                    a: String,
                    b: String,
                },
            }

            let mut rels: Vec<GroupRel> = Vec::new();
            let mut left_of_pairs: Vec<(String, String)> = Vec::new();
            for e in &model.edges {
                let Some(lhs_g) = node_root_group.get(e.lhs_id.as_str()).copied() else {
                    continue;
                };
                let Some(rhs_g) = node_root_group.get(e.rhs_id.as_str()).copied() else {
                    continue;
                };
                if lhs_g == rhs_g {
                    continue;
                }

                let is_group_edge = e.lhs_group.unwrap_or(false) || e.rhs_group.unwrap_or(false);

                // For non-`{group}` edges, keep a larger separation so distinct root groups don't
                // collapse without true compound support in `manatee`.
                //
                // For `{group}` edges, Mermaid shifts endpoints by `architecture.padding + 4` and
                // (effectively) adds ~18px on the bottom side for service labels. Using a smaller
                // base gap keeps stacked groups closer to upstream output (e.g. `docs_group_edges`).
                let mut gap = if is_group_edge {
                    padding_px + 4.0
                } else {
                    1.5 * icon_size
                };

                if is_group_edge {
                    // In Mermaid@11.12.2, junction-to-junction edges that also use `{group}`
                    // endpoints are particularly sensitive to compound node repulsion. Without
                    // true compound support in `manatee`, add an extra padding-derived gap so the
                    // root viewport is closer in `parity-root` mode.
                    let lhs_is_junction =
                        node_type.get(e.lhs_id.as_str()).copied().unwrap_or("") == "junction";
                    let rhs_is_junction =
                        node_type.get(e.rhs_id.as_str()).copied().unwrap_or("") == "junction";
                    if lhs_is_junction && rhs_is_junction {
                        // Tuned for Mermaid@11.12.2 Architecture fixtures where junction-to-junction
                        // edges with `{group}` endpoints dominate the root viewport (e.g.
                        // `upstream_architecture_cypress_complex_junction_edges_normalized`).
                        gap += 1.445 * padding_px;
                    }
                }

                let lhs_dir = e.lhs_dir.as_deref().unwrap_or("");
                let rhs_dir = e.rhs_dir.as_deref().unwrap_or("");
                match (lhs_dir, rhs_dir) {
                    ("R", "L") => {
                        rels.push(GroupRel::LeftOf {
                            left: lhs_g.to_string(),
                            right: rhs_g.to_string(),
                            gap,
                        });
                        left_of_pairs.push((lhs_g.to_string(), rhs_g.to_string()));
                    }
                    ("L", "R") => {
                        rels.push(GroupRel::LeftOf {
                            left: rhs_g.to_string(),
                            right: lhs_g.to_string(),
                            gap,
                        });
                        left_of_pairs.push((rhs_g.to_string(), lhs_g.to_string()));
                    }
                    // Vertical adjacency in SVG y-down coordinates:
                    //
                    // - `lhs:T -- rhs:B` means lhs is *below* rhs (lhs connects from its top to
                    //   rhs's bottom), so rhs is above lhs.
                    // - `lhs:B -- rhs:T` means lhs is *above* rhs (lhs connects from its bottom to
                    //   rhs's top), so lhs is above rhs.
                    ("T", "B") => rels.push(GroupRel::Above {
                        top: rhs_g.to_string(),
                        bottom: lhs_g.to_string(),
                        gap: gap + if is_group_edge { 18.0 } else { 0.0 },
                    }),
                    ("B", "T") => rels.push(GroupRel::Above {
                        top: lhs_g.to_string(),
                        bottom: rhs_g.to_string(),
                        gap: gap + if is_group_edge { 18.0 } else { 0.0 },
                    }),
                    _ => {}
                }
            }

            // When we only have a horizontal ordering signal (L/R), align the top edges of the
            // involved root groups. This approximates the way Cytoscape compound nodes tend to
            // keep sibling groups on the same baseline when they are arranged left-to-right.
            //
            // Do not add this helper constraint when an explicit vertical relation exists.
            let mut has_above: std::collections::BTreeSet<(String, String)> =
                std::collections::BTreeSet::new();
            for r in &rels {
                if let GroupRel::Above { top, bottom, .. } = r {
                    has_above.insert((top.clone(), bottom.clone()));
                    has_above.insert((bottom.clone(), top.clone()));
                }
            }
            for (left, right) in left_of_pairs {
                if has_above.contains(&(left.clone(), right.clone())) {
                    continue;
                }
                let (a, b) = if left <= right {
                    (left, right)
                } else {
                    (right, left)
                };
                rels.push(GroupRel::AlignTop { a, b });
            }

            rels.sort_by(|a, b| format!("{a:?}").cmp(&format!("{b:?}")));
            rels.dedup_by(|a, b| format!("{a:?}") == format!("{b:?}"));

            #[derive(Debug, Clone, Copy)]
            struct BBox {
                min_x: f64,
                min_y: f64,
                max_x: f64,
                max_y: f64,
            }

            fn group_bbox(
                nodes: &[LayoutNode],
                group: &str,
                node_root_group: &std::collections::BTreeMap<&str, &str>,
                node_type: &std::collections::BTreeMap<&str, &str>,
                node_title: &std::collections::BTreeMap<&str, &str>,
                icon_size: f64,
                font_size_px: f64,
            ) -> Option<BBox> {
                let measurer = crate::text::VendoredFontMetricsTextMeasurer::default();
                let text_style = crate::text::TextStyle {
                    font_family: Some("\"trebuchet ms\", verdana, arial, sans-serif".to_string()),
                    font_size: font_size_px,
                    font_weight: None,
                };

                fn wrap_svg_words_to_lines(
                    text: &str,
                    max_width_px: f64,
                    measurer: &dyn crate::text::TextMeasurer,
                    style: &crate::text::TextStyle,
                ) -> Vec<String> {
                    let mut out: Vec<String> = Vec::new();
                    for raw_line in
                        crate::text::DeterministicTextMeasurer::normalized_text_lines(text)
                    {
                        let tokens =
                            crate::text::DeterministicTextMeasurer::split_line_to_words(&raw_line);
                        let mut curr = String::new();
                        for tok in tokens {
                            let candidate = format!("{curr}{tok}");
                            let w = measurer.measure(candidate.trim_end(), style).width;
                            if curr.is_empty() || w <= max_width_px {
                                curr = candidate;
                            } else {
                                out.push(curr.trim().to_string());
                                curr = tok;
                            }
                        }
                        out.push(curr.trim().to_string());
                    }
                    out
                }

                let mut min_x = f64::INFINITY;
                let mut min_y = f64::INFINITY;
                let mut max_x = f64::NEG_INFINITY;
                let mut max_y = f64::NEG_INFINITY;
                let mut any = false;
                for n in nodes {
                    let Some(g) = node_root_group.get(n.id.as_str()).copied() else {
                        continue;
                    };
                    if g != group {
                        continue;
                    }
                    any = true;

                    // Match Architecture Stage B bounds used for group rect sizing:
                    // icon rect + (optional) wrapped service label bbox.
                    let mut nx1 = n.x;
                    let mut ny1 = n.y;
                    let mut nx2 = n.x + n.width;
                    let mut ny2 = n.y + n.height;

                    let is_service =
                        node_type.get(n.id.as_str()).copied().unwrap_or("") == "service";
                    if is_service {
                        if let Some(title) = node_title
                            .get(n.id.as_str())
                            .copied()
                            .map(str::trim)
                            .filter(|t| !t.is_empty())
                        {
                            let lines = wrap_svg_words_to_lines(
                                title,
                                icon_size * 1.5,
                                &measurer,
                                &text_style,
                            );
                            let mut bbox_left = 0.0f64;
                            let mut bbox_right = 0.0f64;
                            for line in &lines {
                                let (l, r) = measurer.measure_svg_text_bbox_x(line, &text_style);
                                bbox_left = bbox_left.max(l);
                                bbox_right = bbox_right.max(r);
                            }
                            let bbox_h = (lines.len().max(1) as f64) * font_size_px * 1.1875;

                            let cx = n.x + icon_size / 2.0;
                            let text_top = n.y + icon_size - 1.0;
                            let text_left = cx - bbox_left;
                            let text_right = cx + bbox_right;
                            let text_bottom = text_top + bbox_h;

                            nx1 = nx1.min(text_left);
                            ny1 = ny1.min(text_top);
                            nx2 = nx2.max(text_right);
                            ny2 = ny2.max(text_bottom);
                        }
                    }

                    min_x = min_x.min(nx1);
                    min_y = min_y.min(ny1);
                    max_x = max_x.max(nx2);
                    max_y = max_y.max(ny2);
                }
                if !any {
                    return None;
                }
                let pad = (icon_size / 2.0) + 2.5;
                Some(BBox {
                    min_x: min_x - pad,
                    min_y: min_y - pad,
                    max_x: max_x + pad,
                    max_y: max_y + pad,
                })
            }

            fn translate_group(
                nodes: &mut [LayoutNode],
                group: &str,
                node_root_group: &std::collections::BTreeMap<&str, &str>,
                dx: f64,
                dy: f64,
            ) {
                if dx == 0.0 && dy == 0.0 {
                    return;
                }
                for n in nodes {
                    let Some(g) = node_root_group.get(n.id.as_str()).copied() else {
                        continue;
                    };
                    if g == group {
                        n.x += dx;
                        n.y += dy;
                    }
                }
            }

            let max_iters = 32usize;
            for _ in 0..max_iters {
                let mut changed = false;
                for rel in &rels {
                    match rel {
                        GroupRel::LeftOf { left, right, gap } => {
                            let Some(a) = group_bbox(
                                nodes,
                                left,
                                &node_root_group,
                                &node_type,
                                &node_title,
                                icon_size,
                                font_size_px,
                            ) else {
                                continue;
                            };
                            let Some(b) = group_bbox(
                                nodes,
                                right,
                                &node_root_group,
                                &node_type,
                                &node_title,
                                icon_size,
                                font_size_px,
                            ) else {
                                continue;
                            };
                            let need = (a.max_x + gap) - b.min_x;
                            if need > 1e-6 {
                                translate_group(nodes, left, &node_root_group, -need / 2.0, 0.0);
                                translate_group(nodes, right, &node_root_group, need / 2.0, 0.0);
                                changed = true;
                            }
                        }
                        GroupRel::Above { top, bottom, gap } => {
                            let Some(a) = group_bbox(
                                nodes,
                                top,
                                &node_root_group,
                                &node_type,
                                &node_title,
                                icon_size,
                                font_size_px,
                            ) else {
                                continue;
                            };
                            let Some(b) = group_bbox(
                                nodes,
                                bottom,
                                &node_root_group,
                                &node_type,
                                &node_title,
                                icon_size,
                                font_size_px,
                            ) else {
                                continue;
                            };
                            let need = (a.max_y + gap) - b.min_y;
                            if need > 1e-6 {
                                translate_group(nodes, top, &node_root_group, 0.0, -need / 2.0);
                                translate_group(nodes, bottom, &node_root_group, 0.0, need / 2.0);
                                changed = true;
                            }
                        }
                        GroupRel::AlignTop { a, b } => {
                            let Some(ba) = group_bbox(
                                nodes,
                                a,
                                &node_root_group,
                                &node_type,
                                &node_title,
                                icon_size,
                                font_size_px,
                            ) else {
                                continue;
                            };
                            let Some(bb) = group_bbox(
                                nodes,
                                b,
                                &node_root_group,
                                &node_type,
                                &node_title,
                                icon_size,
                                font_size_px,
                            ) else {
                                continue;
                            };
                            let dy = ba.min_y - bb.min_y;
                            if dy.abs() > 1e-6 {
                                translate_group(nodes, a, &node_root_group, 0.0, -dy / 2.0);
                                translate_group(nodes, b, &node_root_group, 0.0, dy / 2.0);
                                changed = true;
                            }
                        }
                    }
                }
                if !changed {
                    break;
                }
            }
        }

        let group_separation_start = timing_enabled.then(std::time::Instant::now);
        resolve_top_level_group_separation(&mut nodes, &model, icon_size, padding_px, font_size_px);
        if let Some(s) = group_separation_start {
            timings.group_separation = s.elapsed();
        }
    }

    let build_edges_start = timing_enabled.then(std::time::Instant::now);
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
    if let Some(s) = build_edges_start {
        timings.build_edges = s.elapsed();
    }

    let bounds_start = timing_enabled.then(std::time::Instant::now);
    let bounds = compute_bounds(&nodes, &edges);
    if let Some(s) = bounds_start {
        timings.bounds = s.elapsed();
    }

    if let Some(s) = total_start {
        timings.total = s.elapsed();
        eprintln!(
            "[layout-timing] diagram=architecture total={:?} adjacency={:?} positions={:?} emit_nodes={:?} manatee_prepare={:?} manatee_layout={:?} group_separation={:?} build_edges={:?} bounds={:?} nodes={} edges={} groups={} use_manatee_layout={}",
            timings.total,
            timings.build_adjacency_and_components,
            timings.positions_and_centering,
            timings.emit_nodes,
            timings.manatee_prepare,
            timings.manatee_layout,
            timings.group_separation,
            timings.build_edges,
            timings.bounds,
            nodes.len(),
            edges.len(),
            model.groups.len(),
            use_manatee_layout,
        );
    }

    Ok(ArchitectureDiagramLayout {
        bounds,
        nodes,
        edges,
    })
}
