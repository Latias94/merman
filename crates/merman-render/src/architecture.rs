use crate::generated::architecture_text_overrides_11_12_2 as architecture_text_overrides;
use crate::json::from_value_ref;
use crate::model::{ArchitectureDiagramLayout, Bounds, LayoutEdge, LayoutNode, LayoutPoint};
use crate::text::TextMeasurer;
use crate::{Error, Result};
use indexmap::IndexMap;
use merman_core::diagrams::architecture::ArchitectureDiagramRenderModel;
use rustc_hash::{FxHashMap, FxHashSet};
use serde::Deserialize;
use serde_json::Value;

fn config_f64(cfg: &Value, path: &[&str]) -> Option<f64> {
    let mut cur = cfg;
    for k in path {
        cur = cur.get(*k)?;
    }
    cur.as_f64().or_else(|| cur.as_i64().map(|v| v as f64))
}

fn config_string(cfg: &Value, path: &[&str]) -> Option<String> {
    let mut cur = cfg;
    for k in path {
        cur = cur.get(*k)?;
    }
    cur.as_str().map(|s| s.to_string())
}

#[derive(Debug, Clone, Deserialize)]
struct ArchitectureNodeModel {
    id: String,
    #[serde(rename = "type")]
    node_type: String,
    #[serde(default)]
    edges: Option<Vec<ArchitectureEdgeModel>>,
    #[serde(default, rename = "edgeIndices")]
    edge_indices: Vec<usize>,
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
    #[serde(default, rename = "lhsInto")]
    lhs_into: Option<bool>,
    #[serde(default, rename = "rhsDir")]
    rhs_dir: Option<String>,
    #[serde(default, rename = "rhsInto")]
    rhs_into: Option<bool>,
    #[serde(default, rename = "lhsGroup")]
    lhs_group: Option<bool>,
    #[serde(default, rename = "rhsGroup")]
    rhs_group: Option<bool>,
    #[serde(default)]
    title: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
struct ArchitectureGroupModel {
    id: String,
    #[serde(default)]
    title: Option<String>,
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ArchitectureNodeType {
    Service,
    Junction,
    Other,
}

#[derive(Debug, Clone, Copy)]
struct ArchitectureNodeView<'a> {
    id: &'a str,
    node_type: ArchitectureNodeType,
    title: Option<&'a str>,
    edges: Option<&'a [ArchitectureEdgeModel]>,
    edge_indices: &'a [usize],
    in_group: Option<&'a str>,
}

#[derive(Debug, Clone, Copy)]
struct ArchitectureGroupView<'a> {
    id: &'a str,
    title: Option<&'a str>,
    in_group: Option<&'a str>,
}

#[derive(Debug, Clone, Copy)]
struct ArchitectureEdgeView<'a> {
    lhs_id: &'a str,
    rhs_id: &'a str,
    lhs_dir: Option<char>,
    lhs_into: Option<bool>,
    rhs_dir: Option<char>,
    rhs_into: Option<bool>,
    lhs_group: Option<bool>,
    rhs_group: Option<bool>,
    title: Option<&'a str>,
}

#[derive(Debug, Clone)]
struct ArchitectureModelView<'a> {
    nodes: Vec<ArchitectureNodeView<'a>>,
    groups: Vec<ArchitectureGroupView<'a>>,
    edges: Vec<ArchitectureEdgeView<'a>>,
}

impl<'a> ArchitectureModelView<'a> {
    fn from_json(model: &'a ArchitectureModel) -> Self {
        let nodes = model
            .nodes
            .iter()
            .map(|n| ArchitectureNodeView {
                id: n.id.as_str(),
                node_type: match n.node_type.as_str() {
                    "service" => ArchitectureNodeType::Service,
                    "junction" => ArchitectureNodeType::Junction,
                    _ => ArchitectureNodeType::Other,
                },
                title: n.title.as_deref(),
                edges: n.edges.as_deref(),
                edge_indices: n.edge_indices.as_slice(),
                in_group: n.in_group.as_deref(),
            })
            .collect();

        let groups = model
            .groups
            .iter()
            .map(|g| ArchitectureGroupView {
                id: g.id.as_str(),
                title: g.title.as_deref(),
                in_group: g.in_group.as_deref(),
            })
            .collect();

        let edges = model
            .edges
            .iter()
            .map(|e| ArchitectureEdgeView {
                lhs_id: e.lhs_id.as_str(),
                rhs_id: e.rhs_id.as_str(),
                lhs_dir: e.lhs_dir.as_deref().and_then(|s| s.chars().next()),
                lhs_into: e.lhs_into,
                rhs_dir: e.rhs_dir.as_deref().and_then(|s| s.chars().next()),
                rhs_into: e.rhs_into,
                lhs_group: e.lhs_group,
                rhs_group: e.rhs_group,
                title: e.title.as_deref(),
            })
            .collect();

        Self {
            nodes,
            groups,
            edges,
        }
    }

    fn from_typed(model: &'a ArchitectureDiagramRenderModel) -> Self {
        let nodes = model
            .nodes
            .iter()
            .map(|n| ArchitectureNodeView {
                id: n.id.as_str(),
                node_type: match n.node_type {
                    merman_core::diagrams::architecture::ArchitectureRenderNodeType::Service => {
                        ArchitectureNodeType::Service
                    }
                    merman_core::diagrams::architecture::ArchitectureRenderNodeType::Junction => {
                        ArchitectureNodeType::Junction
                    }
                },
                title: n.title.as_deref(),
                edges: None,
                edge_indices: n.edge_indices.as_slice(),
                in_group: n.in_group.as_deref(),
            })
            .collect();

        let groups = model
            .groups
            .iter()
            .map(|g| ArchitectureGroupView {
                id: g.id.as_str(),
                title: g.title.as_deref(),
                in_group: g.in_group.as_deref(),
            })
            .collect();

        let edges = model
            .edges
            .iter()
            .map(|e| ArchitectureEdgeView {
                lhs_id: e.lhs_id.as_str(),
                rhs_id: e.rhs_id.as_str(),
                lhs_dir: Some(e.lhs_dir),
                lhs_into: e.lhs_into,
                rhs_dir: Some(e.rhs_dir),
                rhs_into: e.rhs_into,
                lhs_group: e.lhs_group,
                rhs_group: e.rhs_group,
                title: e.title.as_deref(),
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
    let model_json: ArchitectureModel = from_value_ref(model)?;
    let model_view = ArchitectureModelView::from_json(&model_json);
    layout_architecture_diagram_model(
        &model_view,
        effective_config,
        _text_measurer,
        use_manatee_layout,
    )
}

pub fn layout_architecture_diagram_typed(
    model: &ArchitectureDiagramRenderModel,
    effective_config: &Value,
    text_measurer: &dyn TextMeasurer,
    use_manatee_layout: bool,
) -> Result<ArchitectureDiagramLayout> {
    let model = ArchitectureModelView::from_typed(model);
    layout_architecture_diagram_model(&model, effective_config, text_measurer, use_manatee_layout)
}

fn layout_architecture_diagram_model(
    model: &ArchitectureModelView<'_>,
    effective_config: &Value,
    text_measurer: &dyn TextMeasurer,
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

    #[derive(Debug, Clone, Copy)]
    struct BBox {
        min_x: f64,
        min_y: f64,
        max_x: f64,
        max_y: f64,
    }

    impl BBox {
        fn from_rect(x: f64, y: f64, w: f64, h: f64) -> Self {
            Self {
                min_x: x,
                min_y: y,
                max_x: x + w,
                max_y: y + h,
            }
        }

        fn union(self, other: Self) -> Self {
            Self {
                min_x: self.min_x.min(other.min_x),
                min_y: self.min_y.min(other.min_y),
                max_x: self.max_x.max(other.max_x),
                max_y: self.max_y.max(other.max_y),
            }
        }

        fn inflate(self, pad: f64) -> Self {
            Self {
                min_x: self.min_x - pad,
                min_y: self.min_y - pad,
                max_x: self.max_x + pad,
                max_y: self.max_y + pad,
            }
        }

        fn center(self) -> (f64, f64) {
            (
                (self.min_x + self.max_x) / 2.0,
                (self.min_y + self.max_y) / 2.0,
            )
        }
    }

    fn measure_cytoscape_node_bbox_extras(
        title: Option<&str>,
        measurer: &dyn crate::text::TextMeasurer,
        style: &crate::text::TextStyle,
        icon_size: f64,
        font_size_px: f64,
    ) -> manatee::BoundsExtras {
        // Cytoscape `node.boundingBox()` includes a small stroke/padding even when labels are
        // short enough to fit within the node rect.
        //
        // Derived from Chromium/Cytoscape measurements for Mermaid Architecture:
        // - icon 80x80 at (0,0) => bbox extends to ~±41px horizontally and ~[-41, 41] vertically
        // - a single-line label adds ~`fontSize + 1` px below the icon, plus the same 1px border
        let border = 1.0;
        let half_icon = icon_size / 2.0;

        let mut half_w = half_icon + border;
        let mut bottom = border;

        if let Some(title) = title.map(str::trim).filter(|t| !t.is_empty()) {
            // Cytoscape node labels are canvas text and (by default) do not apply SVG-style
            // word-wrapping. Model them as a single line for relocation-center parity.
            let m = measurer.measure(title, style);
            // Cytoscape measures labels via canvas metrics; our deterministic metrics table is
            // SVG-oriented and slightly underestimates widths for the default font stack.
            // Calibrate with a small scale factor to match Chromium `node.boundingBox()` values
            // for Architecture fixtures (notably long service titles like "API Gateway").
            // Calibrated against Chromium/Cytoscape `boundingBox()` for Architecture labels.
            // In practice, Cytoscape canvas metrics run slightly wider than our SVG-oriented
            // deterministic table, but a small scale factor keeps relocation centers stable
            // without requiring a browser.
            const LABEL_W_SCALE: f64 = 1.055;
            let label_half = (m.width.max(0.0) * LABEL_W_SCALE) / 2.0;
            half_w = half_w.max(label_half + border);
            // Cytoscape bounding boxes land on 0.5px increments in Chromium; mirror that so
            // relocation centers match upstream baselines more closely.
            half_w = (half_w * 2.0).round() / 2.0;
            bottom = border + (font_size_px + 1.0).max(0.0);

            if std::env::var("MERMAN_ARCH_DEBUG_CY_BBOX").ok().as_deref() == Some("1") {
                eprintln!(
                    "[arch-cy-bbox] title={:?} width={:.6} label_half={:.6} half_w={:.6} extras_lr={:.6} bottom={:.6}",
                    title,
                    m.width,
                    label_half,
                    half_w,
                    (half_w - half_icon).max(0.0),
                    bottom,
                );
            }
        }

        let extra_lr = (half_w - half_icon).max(0.0);
        manatee::BoundsExtras {
            left: extra_lr,
            right: extra_lr,
            top: border,
            bottom,
        }
    }

    // Approximate Cytoscape `eles.boundingBox()` in the pre-layout state where nodes are not
    // explicitly positioned (default `{x: 0, y: 0}` in Cytoscape). The returned center is used
    // as our initial coordinate frame so FCoSE's relocation step matches upstream outputs.
    //
    // Additionally, capture per-node extra bounds (service label extents). These are later fed
    // into the FCoSE port so compound bounds can include labels (`compound-sizing-wrt-labels:
    // include` parity).
    let (initial_center, node_bounds_extras): ((f64, f64), FxHashMap<&str, manatee::BoundsExtras>) = {
        let text_style = crate::text::TextStyle {
            font_family: Some("\"trebuchet ms\", verdana, arial, sans-serif".to_string()),
            font_size: font_size_px,
            font_weight: None,
        };

        let mut node_type: FxHashMap<&str, ArchitectureNodeType> = FxHashMap::default();
        node_type.reserve(model.nodes.len().saturating_mul(2));
        let mut node_title: FxHashMap<&str, &str> = FxHashMap::default();
        node_title.reserve(model.nodes.len().saturating_mul(2));
        let mut node_group: FxHashMap<&str, &str> = FxHashMap::default();
        node_group.reserve(model.nodes.len().saturating_mul(2));
        for n in &model.nodes {
            node_type.insert(n.id, n.node_type);
            if let Some(t) = n.title {
                node_title.insert(n.id, t);
            }
            if let Some(g) = n.in_group {
                node_group.insert(n.id, g);
            }
        }

        let mut group_parent: FxHashMap<&str, &str> = FxHashMap::default();
        group_parent.reserve(model.groups.len().saturating_mul(2));
        let mut group_title: FxHashMap<&str, &str> = FxHashMap::default();
        group_title.reserve(model.groups.len().saturating_mul(2));
        for g in &model.groups {
            if let Some(p) = g.in_group {
                group_parent.insert(g.id, p);
            }
            if let Some(t) = g.title {
                group_title.insert(g.id, t);
            }
        }

        // Leaf bboxes at Cytoscape default node center (0,0), expressed in our top-left space.
        let node_x = -half_icon;
        let node_y = -half_icon;
        let mut node_bbox: FxHashMap<&str, BBox> = FxHashMap::default();
        node_bbox.reserve(model.nodes.len().saturating_mul(2));
        let mut node_bounds_extras: FxHashMap<&str, manatee::BoundsExtras> = FxHashMap::default();
        node_bounds_extras.reserve(model.nodes.len().saturating_mul(2));
        for n in &model.nodes {
            // Cytoscape `eles.boundingBox()` (used by FCoSE for relocation) includes label bounds
            // by default, even when FCoSE is configured with `nodeDimensionsIncludeLabels: false`.
            // This affects the "original center" used by `aux.relocateComponent(...)` and is
            // observable as a stable vertical offset (e.g. ~8.5px for single-line service titles).
            let mut bb = BBox::from_rect(node_x, node_y, icon_size, icon_size);
            let title = node_title.get(n.id).copied();
            let bounds_extras = measure_cytoscape_node_bbox_extras(
                title,
                text_measurer,
                &text_style,
                icon_size,
                font_size_px,
            );
            bb.min_x -= bounds_extras.left;
            bb.max_x += bounds_extras.right;
            bb.min_y -= bounds_extras.top;
            bb.max_y += bounds_extras.bottom;
            node_bbox.insert(n.id, bb);
            node_bounds_extras.insert(n.id, bounds_extras);
        }

        // Group bboxes: approximate Cytoscape compound bounds as leaf-node bounds + padding.
        //
        // Notably, we do *not* accumulate padding across nested compounds here. This matches the
        // observed behavior of Mermaid/Cytoscape `eles.boundingBox()` in the pre-layout state for
        // deep group chains, where intermediate compounds do not expand the relocation center as
        // if their padding stacked recursively.
        let mut group_to_leaves: FxHashMap<&str, Vec<&str>> = FxHashMap::default();
        group_to_leaves.reserve(model.groups.len().saturating_mul(2));
        for g in &model.groups {
            group_to_leaves.entry(g.id).or_default();
        }
        for n in &model.nodes {
            let mut cur = n.in_group;
            while let Some(gid) = cur {
                group_to_leaves.entry(gid).or_default().push(n.id);
                cur = group_parent.get(gid).copied();
            }
        }

        let mut group_bbox: FxHashMap<&str, BBox> = FxHashMap::default();
        group_bbox.reserve(model.groups.len().saturating_mul(2));
        let base_pad = (icon_size / 2.0) + 2.5;
        for g in &model.groups {
            let Some(members) = group_to_leaves.get(g.id) else {
                continue;
            };
            let mut bb: Option<BBox> = None;
            for &nid in members {
                if let Some(nbb) = node_bbox.get(nid).copied() {
                    bb = Some(bb.map(|b| b.union(nbb)).unwrap_or(nbb));
                }
            }
            if let Some(bb) = bb {
                let mut bb = bb.inflate(base_pad);

                // Group titles are rendered inside the compound bounds in Mermaid/Cytoscape and
                // do not affect the pre-layout `eles.boundingBox()` center used for relocation.

                group_bbox.insert(g.id, bb);
            }
        }

        let mut overall: Option<BBox> = None;
        // Prefer top-level groups if any exist; otherwise fall back to leaf nodes.
        let mut any_group = false;
        for g in &model.groups {
            if g.in_group.is_none() {
                if let Some(bb) = group_bbox.get(g.id).copied() {
                    overall = Some(overall.map(|b| b.union(bb)).unwrap_or(bb));
                    any_group = true;
                }
            }
        }
        if !any_group {
            for bb in node_bbox.values().copied() {
                overall = Some(overall.map(|b| b.union(bb)).unwrap_or(bb));
            }
        }

        // `cose-base` operates in a top-left rect coordinate frame internally (see `rect.x/y`),
        // and Cytoscape FCoSE ends up transferring those coordinates back onto nodes as their
        // `position()` values. Mermaid's Architecture renderer then uses those `position()` values
        // directly as the SVG `<g transform="translate(x,y)">` origin (top-left of the 80x80 icon).
        //
        // Our `BBox` math above is expressed in a "center at (0,0)" frame (leaf rects start at
        // `(-halfIcon,-halfIcon)`). Shift by `halfIcon` so the returned center matches the
        // effective top-left-origin coordinate frame used by upstream outputs.
        let (cx, cy) = overall.map(|b| b.center()).unwrap_or((0.0, 0.0));
        ((cx + half_icon, cy + half_icon), node_bounds_extras)
    };
    if std::env::var("MERMAN_ARCH_DEBUG_INIT_CENTER")
        .ok()
        .as_deref()
        == Some("1")
    {
        eprintln!(
            "[arch-init-center] icon_size={:.3} padding={:.3} font_size={:.3} center=({:.6},{:.6}) groups={} nodes={}",
            icon_size,
            padding_px,
            font_size_px,
            initial_center.0,
            initial_center.1,
            model.groups.len(),
            model.nodes.len(),
        );
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    enum Dir {
        L,
        R,
        T,
        B,
    }

    impl Dir {
        fn from_char(ch: char) -> Option<Self> {
            match ch {
                'L' => Some(Self::L),
                'R' => Some(Self::R),
                'T' => Some(Self::T),
                'B' => Some(Self::B),
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

    // Mermaid's Architecture layout uses Cytoscape FCoSE with constraints derived from BFS spatial
    // maps. As a deterministic scaffold (pre-FCoSE port), we reproduce the BFS spatial maps and
    // place nodes on a grid in a way that is close to upstream fixtures.
    //
    // IMPORTANT: `shiftPositionByArchitectureDirectionPair` uses a y-up convention; when mapping
    // to SVG coordinates we invert the sign to keep y-down in pixel space.
    let node_order: Vec<&str> = model.nodes.iter().map(|n| n.id).collect();

    // Mermaid Architecture derives spatial maps by BFS over a per-node adjacency map:
    // - adjacency keys are direction pairs (e.g. `RL`, `TB`)
    // - multiple edges with the same direction pair overwrite the neighbor, but keep the original
    //   key insertion order (JS object semantics)
    //
    // Reference: `repo-ref/mermaid/packages/mermaid/src/diagrams/architecture/architectureDb.ts`
    let mut incident_edges: FxHashMap<&str, Vec<usize>> = FxHashMap::default();
    incident_edges.reserve(model.nodes.len().saturating_mul(2));
    for (edge_idx, e) in model.edges.iter().enumerate() {
        incident_edges.entry(e.lhs_id).or_default().push(edge_idx);
        incident_edges.entry(e.rhs_id).or_default().push(edge_idx);
    }

    let mut adj_list: FxHashMap<&str, IndexMap<&'static str, &str>> = FxHashMap::default();
    adj_list.reserve(model.nodes.len().saturating_mul(2));
    for &id in &node_order {
        let mut adj: IndexMap<&'static str, &str> = IndexMap::new();
        let Some(edges) = incident_edges.get(id) else {
            adj_list.insert(id, adj);
            continue;
        };
        for &edge_idx in edges {
            let e = &model.edges[edge_idx];
            let (rhs_id, lhs_dir, rhs_dir) = if e.lhs_id == id {
                (e.rhs_id, e.lhs_dir, e.rhs_dir)
            } else {
                (e.lhs_id, e.rhs_dir, e.lhs_dir)
            };
            let (Some(lhs_dir), Some(rhs_dir)) = (
                lhs_dir.and_then(Dir::from_char),
                rhs_dir.and_then(Dir::from_char),
            ) else {
                continue;
            };
            let Some(pair) = dir_pair_key(lhs_dir, rhs_dir) else {
                continue;
            };
            if let Some(existing) = adj.get_mut(pair) {
                *existing = rhs_id;
            } else {
                adj.insert(pair, rhs_id);
            }
        }
        adj_list.insert(id, adj);
    }

    // Deterministic component discovery: mimic Mermaid's `Object.keys(notVisited)[0]` by walking
    // `node_order` and taking the first not-yet-visited id for each component.
    let mut components: Vec<IndexMap<&str, (i32, i32)>> = Vec::new();
    let mut visited: FxHashSet<&str> = FxHashSet::default();
    visited.reserve(model.nodes.len().saturating_mul(2));
    for &start_id in &node_order {
        if visited.contains(start_id) {
            continue;
        }

        let mut spatial: IndexMap<&str, (i32, i32)> = IndexMap::new();
        spatial.insert(start_id, (0, 0));

        let mut queue: std::collections::VecDeque<&str> = std::collections::VecDeque::new();
        queue.push_back(start_id);

        while let Some(id) = queue.pop_front() {
            if !visited.insert(id) {
                continue;
            }
            let Some(&(pos_x, pos_y)) = spatial.get(id) else {
                continue;
            };
            let Some(adj) = adj_list.get(id) else {
                continue;
            };
            for (&pair, &rhs_id) in adj.iter() {
                if visited.contains(rhs_id) {
                    continue;
                }
                let (nx, ny) = shift_position_by_arch_pair(pos_x, pos_y, pair);
                // NOTE: Mermaid updates `spatialMap[rhsId]` even if the node is already enqueued,
                // because `visited[rhsId]` is only set when the node is dequeued.
                spatial.insert(rhs_id, (nx, ny));
                queue.push_back(rhs_id);
            }
        }

        components.push(spatial);
    }
    if let Some(s) = build_adjacency_start {
        timings.build_adjacency_and_components = s.elapsed();
    }

    let positions_start = timing_enabled.then(std::time::Instant::now);
    if let Some(s) = positions_start {
        timings.positions_and_centering = s.elapsed();
    }

    // Emit nodes in Mermaid model order (stable for snapshots and close to upstream).
    let emit_nodes_start = timing_enabled.then(std::time::Instant::now);
    for n in &model.nodes {
        match n.node_type {
            ArchitectureNodeType::Service | ArchitectureNodeType::Junction => {}
            other => {
                return Err(Error::InvalidModel {
                    message: format!("unsupported architecture node type: {other:?}"),
                });
            }
        }

        nodes.push(LayoutNode {
            id: n.id.to_string(),
            // Cytoscape nodes default to `{ x: 0, y: 0 }` centers before the first layout run.
            // Our SVG model uses a top-left anchored `<g transform="translate(x,y)">` for the
            // 80x80 icon box, so convert `(0,0)` center into top-left.
            x: 0.0,
            y: 0.0,
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
            node_group.insert(n.id, n.in_group);
        }

        // Mermaid Architecture junction nodes are "invisible" routing helpers. In the upstream
        // Cytoscape model they live inside groups (compound nodes) when they are semantically
        // attached to grouped services.
        //
        // Our semantic model does not always carry explicit `in_group` for junction nodes, so we
        // infer it from incident non-junction neighbors:
        // - pick the unique group if there is exactly one
        // - otherwise pick the most frequent group (skip ties)
        let has_junction = model
            .nodes
            .iter()
            .any(|n| n.node_type == ArchitectureNodeType::Junction);
        if has_junction {
            let junction_ids: std::collections::BTreeSet<&str> = model
                .nodes
                .iter()
                .filter(|n| n.node_type == ArchitectureNodeType::Junction)
                .map(|n| n.id)
                .collect();
            let mut neighbors: std::collections::BTreeMap<&str, Vec<&str>> =
                std::collections::BTreeMap::new();
            for e in &model.edges {
                neighbors.entry(e.lhs_id).or_default().push(e.rhs_id);
                neighbors.entry(e.rhs_id).or_default().push(e.lhs_id);
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

        // Build spatial maps in Mermaid's coordinate space (y-up), keyed by node id.
        let spatial_maps: &[IndexMap<&str, (i32, i32)>] = &components;

        // AlignmentConstraint.
        let mut horizontal_all: Vec<Vec<String>> = Vec::new();
        let mut vertical_all: Vec<Vec<String>> = Vec::new();
        #[derive(Debug, Clone, Copy, PartialEq, Eq)]
        enum GroupAlignment {
            Horizontal,
            Vertical,
            Bend,
        }

        fn dir_alignment(a: Option<char>, b: Option<char>) -> GroupAlignment {
            let (Some(a), Some(b)) = (a.and_then(Dir::from_char), b.and_then(Dir::from_char))
            else {
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
            let Some(lhs_group) = node_group.get(e.lhs_id).and_then(|v| *v) else {
                continue;
            };
            let Some(rhs_group) = node_group.get(e.rhs_id).and_then(|v| *v) else {
                continue;
            };
            if lhs_group == rhs_group {
                continue;
            }
            let alignment = dir_alignment(e.lhs_dir, e.rhs_dir);
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
            alignment_obj: &IndexMap<i32, IndexMap<String, Vec<String>>>,
            alignment_dir: GroupAlignment,
            group_alignments: &std::collections::BTreeMap<
                String,
                std::collections::BTreeMap<String, GroupAlignment>,
            >,
        ) -> Vec<Vec<String>> {
            // Mirror Mermaid's `flattenAlignments(...)` + `Object.values(...)` ordering.
            //
            // Mermaid uses plain JS objects keyed by row/col number. Enumeration order puts
            // non-negative integer keys first (ascending), then other string keys (insertion
            // order). We reproduce that here to keep the first element of each alignment group
            // stable, since `cose-base` uses it to seed dummy-node positions.
            fn js_object_dir_order(obj: &IndexMap<i32, IndexMap<String, Vec<String>>>) -> Vec<i32> {
                let mut non_neg: Vec<i32> = Vec::new();
                let mut other: Vec<i32> = Vec::new();
                for &k in obj.keys() {
                    if k >= 0 {
                        non_neg.push(k);
                    } else {
                        other.push(k);
                    }
                }
                non_neg.sort_unstable();
                non_neg.extend(other);
                non_neg
            }

            fn is_js_array_index_key(k: &str) -> Option<u32> {
                if k.is_empty() {
                    return None;
                }
                if k.as_bytes().iter().all(|b| b.is_ascii_digit()) {
                    return k.parse::<u32>().ok();
                }
                None
            }

            let mut prev: IndexMap<String, Vec<String>> = IndexMap::new();

            for dir in js_object_dir_order(alignment_obj) {
                let Some(alignments) = alignment_obj.get(&dir) else {
                    continue;
                };
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

            // `Object.values(prev)` ordering.
            let mut numeric_keys: Vec<(u32, String)> = Vec::new();
            let mut other_keys: Vec<String> = Vec::new();
            for k in prev.keys() {
                if let Some(ix) = is_js_array_index_key(k.as_str()) {
                    numeric_keys.push((ix, k.clone()));
                } else {
                    other_keys.push(k.clone());
                }
            }
            numeric_keys.sort_by_key(|(ix, _)| *ix);

            let mut out: Vec<Vec<String>> = Vec::new();
            for (_, k) in numeric_keys {
                if let Some(v) = prev.get(&k) {
                    out.push(v.clone());
                }
            }
            for k in other_keys {
                if let Some(v) = prev.get(&k) {
                    out.push(v.clone());
                }
            }
            out
        }

        for spatial_map in spatial_maps {
            let mut horizontal_alignments: IndexMap<i32, IndexMap<String, Vec<String>>> =
                IndexMap::new();
            let mut vertical_alignments: IndexMap<i32, IndexMap<String, Vec<String>>> =
                IndexMap::new();

            for (id, (x, y)) in spatial_map {
                let id = *id;
                let node_group = node_group
                    .get(id)
                    .and_then(|v| *v)
                    .unwrap_or("default")
                    .to_string();

                horizontal_alignments
                    .entry(*y)
                    .or_insert_with(IndexMap::new)
                    .entry(node_group.clone())
                    .or_insert_with(Vec::new)
                    .push(id.to_string());

                vertical_alignments
                    .entry(*x)
                    .or_insert_with(IndexMap::new)
                    .entry(node_group)
                    .or_insert_with(Vec::new)
                    .push(id.to_string());
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

            for v in &horiz_map {
                if v.len() > 1 {
                    horizontal_all.push(v.clone());
                }
            }
            for v in &vert_map {
                if v.len() > 1 {
                    vertical_all.push(v.clone());
                }
            }
        }

        // RelativePlacementConstraint (gap between borders).
        //
        // Upstream Mermaid derives these by BFS over immediate grid neighbors, starting from the
        // spatial origin `(0, 0)`. We mirror that behavior so constraints match Cytoscape's FCoSE
        // input even when the underlying spatial map discovery is approximate.
        let mut relative: Vec<manatee::RelativePlacementConstraint> = Vec::new();
        let gap = 1.5 * icon_size;
        for spatial_map in spatial_maps {
            let mut inv: FxHashMap<(i32, i32), &str> = FxHashMap::default();
            inv.reserve(spatial_map.len().saturating_mul(2));
            for (id, (x, y)) in spatial_map.iter() {
                inv.insert((*x, *y), *id);
            }

            let mut pos_queue: std::collections::VecDeque<(i32, i32)> =
                std::collections::VecDeque::new();
            let mut visited_pos: FxHashSet<(i32, i32)> = FxHashSet::default();
            visited_pos.reserve(spatial_map.len().saturating_mul(2));
            pos_queue.push_back((0, 0));

            // Preserve Mermaid's direction iteration order: L, R, T, B.
            const DIRS: [(char, (i32, i32)); 4] =
                [('L', (-1, 0)), ('R', (1, 0)), ('T', (0, 1)), ('B', (0, -1))];

            while let Some(curr) = pos_queue.pop_front() {
                if !visited_pos.insert(curr) {
                    continue;
                }
                let Some(&curr_id) = inv.get(&curr) else {
                    continue;
                };
                for (dir, (sx, sy)) in DIRS {
                    let new_pos = (curr.0 + sx, curr.1 + sy);
                    let Some(&new_id) = inv.get(&new_pos) else {
                        continue;
                    };
                    if visited_pos.contains(&new_pos) {
                        continue;
                    }
                    pos_queue.push_back(new_pos);

                    // `ArchitectureDirectionName[dir] = newId`
                    // `ArchitectureDirectionName[getOppositeArchitectureDirection(dir)] = currId`
                    let c = match dir {
                        'L' => manatee::RelativePlacementConstraint {
                            left: Some(new_id.to_string()),
                            right: Some(curr_id.to_string()),
                            top: None,
                            bottom: None,
                            gap,
                        },
                        'R' => manatee::RelativePlacementConstraint {
                            left: Some(curr_id.to_string()),
                            right: Some(new_id.to_string()),
                            top: None,
                            bottom: None,
                            gap,
                        },
                        'T' => manatee::RelativePlacementConstraint {
                            left: None,
                            right: None,
                            top: Some(new_id.to_string()),
                            bottom: Some(curr_id.to_string()),
                            gap,
                        },
                        'B' => manatee::RelativePlacementConstraint {
                            left: None,
                            right: None,
                            top: Some(curr_id.to_string()),
                            bottom: Some(new_id.to_string()),
                            gap,
                        },
                        _ => continue,
                    };
                    relative.push(c);
                }
            }
        }

        // Run `manatee` layout refinement.
        //
        // Mermaid Architecture uses Cytoscape FCoSE with `idealEdgeLength` and `edgeElasticity`
        // callbacks that depend *only* on whether the connected nodes share the same parent
        // compound (group). Avoid adding layout-base "smart" adjustments here: upstream Mermaid
        // does not apply them, and doing so causes `parity-root` viewport drift in group-heavy
        // fixtures.

        let mut edges: Vec<manatee::Edge> = Vec::new();
        let mut default_edge_length_sum = 0.0f64;
        let mut default_edge_length_cnt = 0.0f64;
        let font_family = config_string(effective_config, &["fontFamily"])
            .or_else(|| config_string(effective_config, &["themeVariables", "fontFamily"]))
            .map(|s| s.trim().trim_end_matches(';').trim().to_string());
        let edge_text_style = crate::text::TextStyle {
            font_family: font_family
                .or_else(|| Some("\"trebuchet ms\", verdana, arial, sans-serif".to_string())),
            font_size: font_size_px,
            font_weight: None,
        };

        // Cytoscape FCoSE de-duplicates multiple edges between the same two nodes when building
        // its internal layout graph:
        //
        // `sourceNode.getEdgesBetween(targetNode).length == 0`
        //
        // This means bidirectional/multi edges still render in the final SVG, but only the first
        // edge between each undirected node pair contributes forces to the layout.
        //
        // Without this, our spring forces can cancel in small symmetric graphs, which makes the
        // final spacing (and thus the root `viewBox/max-width`) diverge from Mermaid baselines.
        let mut seen_undirected_layout_edges: FxHashSet<(String, String)> = FxHashSet::default();

        for e in &model.edges {
            let (a, b) = (e.lhs_id, e.rhs_id);
            let (k1, k2) = if a <= b { (a, b) } else { (b, a) };
            if !seen_undirected_layout_edges.insert((k1.to_string(), k2.to_string())) {
                continue;
            }

            let lhs_g = node_group.get(e.lhs_id).and_then(|v| *v);
            let rhs_g = node_group.get(e.rhs_id).and_then(|v| *v);
            let same_parent = lhs_g == rhs_g;

            let base_ideal_length = if same_parent {
                1.5 * icon_size
            } else {
                0.5 * icon_size
            };
            default_edge_length_sum += base_ideal_length;
            default_edge_length_cnt += 1.0;

            let ideal_length = base_ideal_length;

            let elasticity = if same_parent { 0.45 } else { 0.001 };

            let source_anchor = e.lhs_dir.and_then(Dir::from_char).map(|d| match d {
                Dir::L => manatee::Anchor::Left,
                Dir::R => manatee::Anchor::Right,
                Dir::T => manatee::Anchor::Top,
                Dir::B => manatee::Anchor::Bottom,
            });
            let target_anchor = e.rhs_dir.and_then(Dir::from_char).map(|d| match d {
                Dir::L => manatee::Anchor::Left,
                Dir::R => manatee::Anchor::Right,
                Dir::T => manatee::Anchor::Top,
                Dir::B => manatee::Anchor::Bottom,
            });

            let (label_width, label_height) =
                match e.title.as_deref().map(str::trim).filter(|t| !t.is_empty()) {
                    Some(label) => {
                        let m = text_measurer.measure(label, &edge_text_style);
                        let w = m.width.max(0.0);
                        // Cytoscape edge label bounding boxes are slightly taller than the measured
                        // font metrics height (roughly `fontSize + 1px` at Mermaid defaults).
                        let h = (m.height + 1.0).max(0.0);
                        (Some(w), Some(h))
                    }
                    None => (None, None),
                };
            edges.push(manatee::Edge {
                id: format!("edge-{}", edges.len()),
                source: e.lhs_id.to_string(),
                target: e.rhs_id.to_string(),
                label_width,
                label_height,
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
                    // Mermaid Architecture feeds Cytoscape node `position()` values directly
                    // into the SVG `translate(x,y)` for the 80x80 icon box (i.e. it treats the
                    // Cytoscape "center" as a top-left anchor). This creates a consistent
                    // coordinate convention across nodes/edges/viewBox in upstream baselines.
                    //
                    // Mirror that here by passing through our top-left anchored `{x,y}` without
                    // converting to geometric centers.
                    x: n.x,
                    y: n.y,
                    bounds_extras: node_bounds_extras
                        .get(n.id.as_str())
                        .copied()
                        .unwrap_or_default(),
                })
                .collect(),
            edges,
            compounds: model
                .groups
                .iter()
                .map(|g| manatee::Compound {
                    id: g.id.to_string(),
                    parent: g.in_group.map(str::to_string),
                })
                .collect(),
        };

        // Mermaid Architecture styles group nodes with `padding: ${db.getConfigField('padding')}px`
        // before running FCoSE, and CoSE uses that per-compound padding when updating bounds.
        let compound_padding_px = padding_px;

        let opts = manatee::FcoseOptions {
            alignment_constraint: Some(manatee::AlignmentConstraint {
                horizontal: horizontal_all,
                vertical: vertical_all,
            }),
            relative_placement_constraint: relative,
            default_edge_length: Some(default_edge_length),
            compound_padding: Some(compound_padding_px),
            relocate_center: None,
            // Mermaid Architecture runs the layout twice (`layout.run()` inside `layoutstop`),
            // which advances the seeded RNG stream and can change final positions.
            rerun: true,
            // Mermaid@11.12.2 Architecture layout uses Cytoscape FCoSE with a spectral
            // initialization that depends on `Math.random()`. Our upstream SVG baselines are
            // generated with a deterministic RNG seed (see ADR-0055), so we must use the same
            // seed here to match those baselines.
            random_seed: 1,
        };

        if std::env::var("MERMAN_ARCH_DEBUG_FCOSE_CONSTRAINTS")
            .ok()
            .as_deref()
            == Some("1")
        {
            eprintln!(
                "[arch-fcose] nodes={} edges={} compounds={} default_edge_length={:.6} compound_padding={:.6}",
                graph.nodes.len(),
                graph.edges.len(),
                graph.compounds.len(),
                default_edge_length,
                compound_padding_px,
            );
            if let Some(a) = &opts.alignment_constraint {
                eprintln!("[arch-fcose] alignment.horizontal={:?}", a.horizontal);
                eprintln!("[arch-fcose] alignment.vertical={:?}", a.vertical);
            }
            eprintln!(
                "[arch-fcose] relative_placement_constraint={:?}",
                opts.relative_placement_constraint
            );
        }

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
                n.x = p.x;
                n.y = p.y;
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
            model: &ArchitectureModelView<'_>,
            icon_size: f64,
            padding_px: f64,
            font_size_px: f64,
        ) {
            let node_type: FxHashMap<&str, ArchitectureNodeType> =
                model.nodes.iter().map(|n| (n.id, n.node_type)).collect();
            let node_title: FxHashMap<&str, &str> = model
                .nodes
                .iter()
                .filter_map(|n| n.title.map(|t| (n.id, t)))
                .collect();

            let mut group_parent: FxHashMap<&str, &str> = FxHashMap::default();
            group_parent.reserve(model.groups.len().saturating_mul(2));
            for g in &model.groups {
                if let Some(parent) = g.in_group {
                    group_parent.insert(g.id, parent);
                }
            }

            fn root_group<'a>(
                mut g: &'a str,
                group_parent: &FxHashMap<&'a str, &'a str>,
            ) -> &'a str {
                while let Some(p) = group_parent.get(g).copied() {
                    g = p;
                }
                g
            }

            let mut node_root_group: FxHashMap<&str, &str> = FxHashMap::default();
            node_root_group.reserve(model.nodes.len());
            for n in &model.nodes {
                if let Some(g) = n.in_group {
                    let root = root_group(g, &group_parent);
                    node_root_group.insert(n.id, root);
                }
            }

            let mut group_members: FxHashMap<&str, Vec<usize>> = FxHashMap::default();
            group_members.reserve(model.groups.len().saturating_mul(2));
            for (idx, n) in nodes.iter().enumerate() {
                let Some(g) = node_root_group.get(n.id.as_str()).copied() else {
                    continue;
                };
                group_members.entry(g).or_default().push(idx);
            }

            #[derive(Debug, Clone, Copy)]
            enum GroupRel<'a> {
                LeftOf {
                    left: &'a str,
                    right: &'a str,
                    gap: f64,
                },
                Above {
                    top: &'a str,
                    bottom: &'a str,
                    gap: f64,
                },
                AlignTop {
                    a: &'a str,
                    b: &'a str,
                },
            }

            let mut rels: Vec<GroupRel<'_>> = Vec::new();
            let mut left_of_pairs: Vec<(&str, &str)> = Vec::new();
            for e in &model.edges {
                let Some(lhs_g) = node_root_group.get(e.lhs_id).copied() else {
                    continue;
                };
                let Some(rhs_g) = node_root_group.get(e.rhs_id).copied() else {
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
                        node_type.get(e.lhs_id).copied() == Some(ArchitectureNodeType::Junction);
                    let rhs_is_junction =
                        node_type.get(e.rhs_id).copied() == Some(ArchitectureNodeType::Junction);
                    if lhs_is_junction && rhs_is_junction {
                        // Tuned for Mermaid@11.12.2 Architecture fixtures where junction-to-junction
                        // edges with `{group}` endpoints dominate the root viewport (e.g.
                        // `upstream_architecture_cypress_complex_junction_edges_normalized`).
                        gap += 1.445 * padding_px;
                    }
                }

                match (e.lhs_dir, e.rhs_dir) {
                    (Some('R'), Some('L')) => {
                        rels.push(GroupRel::LeftOf {
                            left: lhs_g,
                            right: rhs_g,
                            gap,
                        });
                        left_of_pairs.push((lhs_g, rhs_g));
                    }
                    (Some('L'), Some('R')) => {
                        rels.push(GroupRel::LeftOf {
                            left: rhs_g,
                            right: lhs_g,
                            gap,
                        });
                        left_of_pairs.push((rhs_g, lhs_g));
                    }
                    // Vertical adjacency in SVG y-down coordinates:
                    //
                    // - `lhs:T -- rhs:B` means lhs is *below* rhs (lhs connects from its top to
                    //   rhs's bottom), so rhs is above lhs.
                    // - `lhs:B -- rhs:T` means lhs is *above* rhs (lhs connects from its bottom to
                    //   rhs's top), so lhs is above rhs.
                    (Some('T'), Some('B')) => rels.push(GroupRel::Above {
                        top: rhs_g,
                        bottom: lhs_g,
                        gap: gap + if is_group_edge { 18.0 } else { 0.0 },
                    }),
                    (Some('B'), Some('T')) => rels.push(GroupRel::Above {
                        top: lhs_g,
                        bottom: rhs_g,
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
            let mut has_above: FxHashSet<(&str, &str)> = FxHashSet::default();
            has_above.reserve(rels.len().saturating_mul(2));
            for r in &rels {
                if let GroupRel::Above { top, bottom, .. } = r {
                    has_above.insert((top, bottom));
                    has_above.insert((bottom, top));
                }
            }
            for (left, right) in left_of_pairs {
                if has_above.contains(&(left, right)) {
                    continue;
                }
                let (a, b) = if left <= right {
                    (left, right)
                } else {
                    (right, left)
                };
                rels.push(GroupRel::AlignTop { a, b });
            }

            // Deterministic ordering + dedup without allocating debug strings.
            //
            // For duplicate directional relations between the same group pair, keep the maximum
            // gap (smaller gaps are redundant).
            fn rel_kind(r: &GroupRel<'_>) -> u8 {
                match r {
                    GroupRel::LeftOf { .. } => 0,
                    GroupRel::Above { .. } => 1,
                    GroupRel::AlignTop { .. } => 2,
                }
            }

            fn rel_a<'a>(r: &'a GroupRel<'a>) -> &'a str {
                match r {
                    GroupRel::LeftOf { left, .. } => left,
                    GroupRel::Above { top, .. } => top,
                    GroupRel::AlignTop { a, .. } => a,
                }
            }

            fn rel_b<'a>(r: &'a GroupRel<'a>) -> &'a str {
                match r {
                    GroupRel::LeftOf { right, .. } => right,
                    GroupRel::Above { bottom, .. } => bottom,
                    GroupRel::AlignTop { b, .. } => b,
                }
            }

            fn rel_gap_bits(r: &GroupRel<'_>) -> u64 {
                match r {
                    GroupRel::LeftOf { gap, .. } | GroupRel::Above { gap, .. } => gap.to_bits(),
                    GroupRel::AlignTop { .. } => 0,
                }
            }

            rels.sort_by(|a, b| {
                (rel_kind(a), rel_a(a), rel_b(a), rel_gap_bits(a)).cmp(&(
                    rel_kind(b),
                    rel_a(b),
                    rel_b(b),
                    rel_gap_bits(b),
                ))
            });
            let mut deduped: Vec<GroupRel<'_>> = Vec::with_capacity(rels.len());
            for r in rels.drain(..) {
                if let Some(last) = deduped.last_mut()
                    && rel_kind(last) == rel_kind(&r)
                    && rel_a(last) == rel_a(&r)
                    && rel_b(last) == rel_b(&r)
                {
                    match (last, r) {
                        (GroupRel::LeftOf { gap, .. }, GroupRel::LeftOf { gap: g, .. })
                        | (GroupRel::Above { gap, .. }, GroupRel::Above { gap: g, .. }) => {
                            *gap = gap.max(g);
                        }
                        _ => {}
                    }
                    continue;
                }
                deduped.push(r);
            }
            rels = deduped;

            let measurer = crate::text::VendoredFontMetricsTextMeasurer::default();
            let text_style = crate::text::TextStyle {
                font_family: Some("\"trebuchet ms\", verdana, arial, sans-serif".to_string()),
                font_size: font_size_px,
                font_weight: None,
            };

            #[derive(Debug, Clone, Copy)]
            struct BBox {
                min_x: f64,
                min_y: f64,
                max_x: f64,
                max_y: f64,
            }

            #[derive(Debug, Clone, Copy)]
            struct LabelExtras {
                left: f64,
                right: f64,
                bottom: f64,
            }

            fn measure_service_label_extras(
                title: &str,
                max_width_px: f64,
                measurer: &dyn crate::text::TextMeasurer,
                style: &crate::text::TextStyle,
                icon_size: f64,
                font_size_px: f64,
            ) -> Option<LabelExtras> {
                let title = title.trim();
                if title.is_empty() {
                    return None;
                }

                let mut max_left = 0.0f64;
                let mut max_right = 0.0f64;
                let mut line_count = 0usize;

                // Greedy wrap matching `wrapSvgWordsToLines`, but without allocating intermediate
                // line `String`s and without `format!` churn in the inner loop.
                for raw_line in crate::text::DeterministicTextMeasurer::normalized_text_lines(title)
                {
                    let tokens =
                        crate::text::DeterministicTextMeasurer::split_line_to_words(&raw_line);
                    let mut curr = String::new();
                    for tok in tokens {
                        if curr.is_empty() {
                            curr.push_str(&tok);
                            continue;
                        }

                        let old_len = curr.len();
                        curr.push_str(&tok);
                        let w = measurer.measure(curr.trim_end(), style).width;
                        if w <= max_width_px {
                            continue;
                        }

                        curr.truncate(old_len);
                        let line = curr.trim();
                        if !line.is_empty() {
                            let (l, r) = measurer.measure_svg_text_bbox_x(line, style);
                            max_left = max_left.max(l);
                            max_right = max_right.max(r);
                            line_count += 1;
                        }

                        curr.clear();
                        curr.push_str(&tok);
                    }

                    let line = curr.trim();
                    if !line.is_empty() {
                        let (l, r) = measurer.measure_svg_text_bbox_x(line, style);
                        max_left = max_left.max(l);
                        max_right = max_right.max(r);
                        line_count += 1;
                    }
                }

                if line_count == 0 {
                    return None;
                }

                let bbox_h = architecture_text_overrides::architecture_icon_text_bbox_height_px(
                    font_size_px,
                    line_count,
                );
                let half_icon = icon_size / 2.0;
                Some(LabelExtras {
                    left: (half_icon - max_left).min(0.0),
                    right: (max_right - half_icon).max(0.0),
                    bottom: (bbox_h - 1.0).max(0.0),
                })
            }

            fn group_bbox(
                nodes: &[LayoutNode],
                members: &[usize],
                node_type: &FxHashMap<&str, ArchitectureNodeType>,
                node_title: &FxHashMap<&str, &str>,
                label_extras: &mut FxHashMap<String, LabelExtras>,
                measurer: &dyn crate::text::TextMeasurer,
                text_style: &crate::text::TextStyle,
                icon_size: f64,
                font_size_px: f64,
            ) -> Option<BBox> {
                if members.is_empty() {
                    return None;
                }

                let mut min_x = f64::INFINITY;
                let mut min_y = f64::INFINITY;
                let mut max_x = f64::NEG_INFINITY;
                let mut max_y = f64::NEG_INFINITY;

                for &idx in members {
                    let Some(n) = nodes.get(idx) else {
                        continue;
                    };

                    // Match Architecture Stage B bounds used for group rect sizing:
                    // icon rect + (optional) wrapped service label bbox.
                    let mut nx1 = n.x;
                    let ny1 = n.y;
                    let mut nx2 = n.x + n.width;
                    let mut ny2 = n.y + n.height;

                    let is_service = node_type.get(n.id.as_str()).copied()
                        == Some(ArchitectureNodeType::Service);
                    if is_service {
                        if let Some(title) = node_title.get(n.id.as_str()).copied() {
                            let extras = if let Some(v) = label_extras.get(n.id.as_str()).copied() {
                                Some(v)
                            } else {
                                let computed = measure_service_label_extras(
                                    title,
                                    icon_size * 1.5,
                                    measurer,
                                    text_style,
                                    icon_size,
                                    font_size_px,
                                );
                                if let Some(v) = computed {
                                    label_extras.insert(n.id.clone(), v);
                                }
                                computed
                            };
                            if let Some(extras) = extras {
                                nx1 = nx1.min(n.x + extras.left);
                                nx2 = nx2.max(n.x + icon_size + extras.right);
                                ny2 = ny2.max(n.y + icon_size + extras.bottom);
                            }
                        }
                    }

                    min_x = min_x.min(nx1);
                    min_y = min_y.min(ny1);
                    max_x = max_x.max(nx2);
                    max_y = max_y.max(ny2);
                }

                if !(min_x.is_finite()
                    && min_y.is_finite()
                    && max_x.is_finite()
                    && max_y.is_finite())
                {
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

            let mut label_extras: FxHashMap<String, LabelExtras> = FxHashMap::default();
            label_extras.reserve(node_title.len());

            let mut bboxes: FxHashMap<&str, BBox> = FxHashMap::default();
            bboxes.reserve(group_members.len().saturating_mul(2));
            for (&group, members) in &group_members {
                if let Some(bb) = group_bbox(
                    nodes,
                    members,
                    &node_type,
                    &node_title,
                    &mut label_extras,
                    &measurer,
                    &text_style,
                    icon_size,
                    font_size_px,
                ) {
                    bboxes.insert(group, bb);
                }
            }

            fn translate_group(
                nodes: &mut [LayoutNode],
                group: &str,
                group_members: &FxHashMap<&str, Vec<usize>>,
                bboxes: &mut FxHashMap<&str, BBox>,
                dx: f64,
                dy: f64,
            ) {
                if dx == 0.0 && dy == 0.0 {
                    return;
                }
                if let Some(members) = group_members.get(group) {
                    for &idx in members {
                        if let Some(n) = nodes.get_mut(idx) {
                            n.x += dx;
                            n.y += dy;
                        }
                    }
                }
                if let Some(bb) = bboxes.get_mut(group) {
                    bb.min_x += dx;
                    bb.max_x += dx;
                    bb.min_y += dy;
                    bb.max_y += dy;
                }
            }

            let max_iters = 32usize;
            for _ in 0..max_iters {
                let mut changed = false;
                for rel in &rels {
                    match rel {
                        GroupRel::LeftOf { left, right, gap } => {
                            let Some(a) = bboxes.get(left).copied() else {
                                continue;
                            };
                            let Some(b) = bboxes.get(right).copied() else {
                                continue;
                            };
                            let need = (a.max_x + gap) - b.min_x;
                            if need > 1e-6 {
                                translate_group(
                                    nodes,
                                    left,
                                    &group_members,
                                    &mut bboxes,
                                    -need / 2.0,
                                    0.0,
                                );
                                translate_group(
                                    nodes,
                                    right,
                                    &group_members,
                                    &mut bboxes,
                                    need / 2.0,
                                    0.0,
                                );
                                changed = true;
                            }
                        }
                        GroupRel::Above { top, bottom, gap } => {
                            let Some(a) = bboxes.get(top).copied() else {
                                continue;
                            };
                            let Some(b) = bboxes.get(bottom).copied() else {
                                continue;
                            };
                            let need = (a.max_y + gap) - b.min_y;
                            if need > 1e-6 {
                                translate_group(
                                    nodes,
                                    top,
                                    &group_members,
                                    &mut bboxes,
                                    0.0,
                                    -need / 2.0,
                                );
                                translate_group(
                                    nodes,
                                    bottom,
                                    &group_members,
                                    &mut bboxes,
                                    0.0,
                                    need / 2.0,
                                );
                                changed = true;
                            }
                        }
                        GroupRel::AlignTop { a, b } => {
                            let Some(ba) = bboxes.get(a).copied() else {
                                continue;
                            };
                            let Some(bb) = bboxes.get(b).copied() else {
                                continue;
                            };
                            let dy = ba.min_y - bb.min_y;
                            if dy.abs() > 1e-6 {
                                translate_group(
                                    nodes,
                                    a,
                                    &group_members,
                                    &mut bboxes,
                                    0.0,
                                    -dy / 2.0,
                                );
                                translate_group(
                                    nodes,
                                    b,
                                    &group_members,
                                    &mut bboxes,
                                    0.0,
                                    dy / 2.0,
                                );
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
    }

    let build_edges_start = timing_enabled.then(std::time::Instant::now);
    let mut node_by_id: FxHashMap<&str, &LayoutNode> = FxHashMap::default();
    node_by_id.reserve(nodes.len());
    for n in &nodes {
        node_by_id.insert(n.id.as_str(), n);
    }

    let mut edges: Vec<LayoutEdge> = Vec::new();
    for (idx, e) in model.edges.iter().enumerate() {
        let Some(&a) = node_by_id.get(e.lhs_id) else {
            return Err(Error::InvalidModel {
                message: format!("edge lhs node not found: {}", e.lhs_id),
            });
        };
        let Some(&b) = node_by_id.get(e.rhs_id) else {
            return Err(Error::InvalidModel {
                message: format!("edge rhs node not found: {}", e.rhs_id),
            });
        };

        fn endpoint(
            x: f64,
            y: f64,
            dir: Option<char>,
            icon_size: f64,
            half_icon: f64,
        ) -> (f64, f64) {
            match dir {
                Some('L') => (x, y + half_icon),
                Some('R') => (x + icon_size, y + half_icon),
                Some('T') => (x + half_icon, y),
                Some('B') => (x + half_icon, y + icon_size),
                _ => (x + half_icon, y + half_icon),
            }
        }

        let (sx, sy) = endpoint(a.x, a.y, e.lhs_dir, icon_size, half_icon);
        let (tx, ty) = endpoint(b.x, b.y, e.rhs_dir, icon_size, half_icon);

        fn cytoscape_segments_weight_distance_for_point(
            source: (f64, f64),
            target: (f64, f64),
            point: (f64, f64),
        ) -> Option<(f64, f64)> {
            // Mermaid Architecture uses Cytoscape `curve-style: segments` for XY edges and derives
            // `segment-weights`/`segment-distances` from a chosen 90° bend point.
            //
            // Reference: `repo-ref/mermaid/packages/mermaid/src/diagrams/architecture/architectureRenderer.ts`
            let (s_x, s_y) = source;
            let (t_x, t_y) = target;
            let (p_x, p_y) = point;

            if s_x == t_x || s_y == t_y {
                return None;
            }

            let denom_x = s_x - t_x;
            if denom_x == 0.0 {
                return None;
            }

            let slope = (s_y - t_y) / denom_x;
            let d =
                (p_y - s_y + ((s_x - p_x) * (s_y - t_y)) / denom_x) / (1.0 + slope * slope).sqrt();

            let w = ((p_y - s_y).powi(2) + (p_x - s_x).powi(2) - d.powi(2))
                .max(0.0)
                .sqrt();
            let dist_ab = ((t_x - s_x).powi(2) + (t_y - s_y).powi(2)).sqrt();
            if dist_ab == 0.0 {
                return None;
            }
            let mut w = w / dist_ab;

            // Ensure that the sign of `d` matches the left/right side of the line from source to
            // target, and that the sign of `w` matches whether the point is "behind" the source.
            let delta1 = (t_x - s_x) * (p_y - s_y) - (t_y - s_y) * (p_x - s_x);
            let delta1 = if delta1 >= 0.0 { 1.0 } else { -1.0 };
            let delta2 = (t_x - s_x) * (p_x - s_x) + (t_y - s_y) * (p_y - s_y);
            let delta2 = if delta2 >= 0.0 { 1.0 } else { -1.0 };

            let d = d.abs() * delta1;
            w *= delta2;

            Some((w, d))
        }

        fn cytoscape_segments_point_from_weight_distance(
            source: (f64, f64),
            target: (f64, f64),
            weight: f64,
            distance: f64,
        ) -> Option<(f64, f64)> {
            // Cytoscape "segments" curve point (for a single segment) is defined by:
            // - `weight`: normalized distance along the source->target vector
            // - `distance`: signed perpendicular offset from the line
            //
            // We reconstruct the implied bend point so our headless routing matches the
            // upstream browser output.
            let (s_x, s_y) = source;
            let (t_x, t_y) = target;
            let dx = t_x - s_x;
            let dy = t_y - s_y;
            let dist_ab = (dx * dx + dy * dy).sqrt();
            if dist_ab == 0.0 {
                return None;
            }

            let ux = dx / dist_ab;
            let uy = dy / dist_ab;
            // Left-hand normal of the line.
            let nx = -uy;
            let ny = ux;

            let along = weight * dist_ab;
            Some((
                s_x + ux * along + nx * distance,
                s_y + uy * along + ny * distance,
            ))
        }
        // Mirror Mermaid Architecture edge routing:
        //
        // - Non-XY edges use Cytoscape `curve-style: straight`, and Mermaid draws a 3-point
        //   polyline using `edge.midpoint()`, which is the midpoint of the straight segment.
        // - XY edges (`curve-style: segments`) are post-processed to create a single 90° bend.
        //   Mermaid then draws a 3-point polyline where the midpoint corresponds to that bend.
        //
        // Note: Group/junction endpoint shifts are applied later during SVG emission; these
        // layout points represent the raw Cytoscape endpoints.
        let is_xy = match (
            e.lhs_dir.and_then(Dir::from_char),
            e.rhs_dir.and_then(Dir::from_char),
        ) {
            (Some(a), Some(b)) => a.is_x() != b.is_x(),
            _ => false,
        };
        let mid = if is_xy {
            let (point_x, point_y) = if matches!(e.lhs_dir, Some('T' | 'B')) {
                (sx, ty)
            } else {
                (tx, sy)
            };
            let (w, d) = cytoscape_segments_weight_distance_for_point(
                (sx, sy),
                (tx, ty),
                (point_x, point_y),
            )
            .unwrap_or((0.0, 0.0));
            let (mx, my) = cytoscape_segments_point_from_weight_distance((sx, sy), (tx, ty), w, d)
                .unwrap_or((point_x, point_y));
            LayoutPoint { x: mx, y: my }
        } else {
            LayoutPoint {
                x: (sx + tx) / 2.0,
                y: (sy + ty) / 2.0,
            }
        };
        edges.push(LayoutEdge {
            id: format!("edge-{idx}"),
            from: e.lhs_id.to_string(),
            to: e.rhs_id.to_string(),
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
            "[layout-timing] diagram=architecture total={:?} adjacency={:?} positions={:?} emit_nodes={:?} manatee_prepare={:?} manatee_layout={:?} build_edges={:?} bounds={:?} nodes={} edges={} groups={} use_manatee_layout={}",
            timings.total,
            timings.build_adjacency_and_components,
            timings.positions_and_centering,
            timings.emit_nodes,
            timings.manatee_prepare,
            timings.manatee_layout,
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

#[cfg(test)]
mod tests {
    #[test]
    fn architecture_text_constants_are_generated() {
        assert_eq!(
            crate::generated::architecture_text_overrides_11_12_2::
                architecture_icon_text_bbox_height_px(16.0, 1),
            19.0
        );
        assert!((crate::generated::architecture_text_overrides_11_12_2::
            architecture_create_text_bbox_height_px(16.0, 2)
            - 36.6)
            .abs()
            < 1e-9);
        assert_eq!(
            crate::generated::architecture_text_overrides_11_12_2::
                architecture_create_text_compound_label_extra_bottom_px(16.0),
            17.0
        );
        assert_eq!(
            crate::generated::architecture_text_overrides_11_12_2::
                architecture_create_text_root_label_extra_bottom_px(16.0, 1),
            24.1875
        );
    }
}
