//! ER paint geometry derived from a borrowed family layout before the SVG root is opened.

use merman_model::families::er::ErRelationship;
#[cfg(any(feature = "layout-dagre", feature = "layout-elk"))]
use merman_render_core::LayoutPoint;
use merman_render_core::graph_layout::{elk_geometry, elk_terminal_jogs};
use merman_render_core::resources::OperationWorkMeter;
use merman_render_core::{Bounds, LayoutCluster, LayoutEdge, LayoutNode, Result};
use rustc_hash::{FxHashMap, FxHashSet};
use serde_json::Value;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ErSvgRouteKind {
    Dagre,
    Elk,
    Swimlane,
    Canonical,
}

impl ErSvgRouteKind {
    const fn uses_rounded_geometry(self) -> bool {
        matches!(self, Self::Elk | Self::Swimlane)
    }
}

pub struct ErSvgLayoutView<'a> {
    pub clusters: &'a [LayoutCluster],
    pub nodes: &'a [LayoutNode],
    pub edges: &'a [LayoutEdge],
    pub render_edges: &'a [LayoutEdge],
}

pub struct ErSvgGeometry<'a> {
    clusters: &'a [LayoutCluster],
    nodes: Vec<LayoutNode>,
    edges: Vec<LayoutEdge>,
    missing_sections: FxHashSet<String>,
}

impl<'a> ErSvgGeometry<'a> {
    pub fn prepare(
        layout: ErSvgLayoutView<'a>,
        route_kind: ErSvgRouteKind,
        relationships: &[ErRelationship],
        effective_config: &Value,
        work_meter: &OperationWorkMeter,
    ) -> Result<Self> {
        let uses_canonical_routes = route_kind == ErSvgRouteKind::Canonical;
        let rounded_layout = route_kind.uses_rounded_geometry();
        let is_elk_layout = route_kind == ErSvgRouteKind::Elk;
        fn parse_trailing_index(id: &str) -> Option<i64> {
            let (_, tail) = id.rsplit_once('-')?;
            tail.parse::<i64>().ok()
        }
        fn er_node_sort_key(id: &str) -> (i64, i64) {
            if id.contains("---") {
                return (1, parse_trailing_index(id).unwrap_or(i64::MAX));
            }
            (0, parse_trailing_index(id).unwrap_or(i64::MAX))
        }

        let mut nodes = layout.nodes.to_vec();
        nodes.sort_by_key(|n| er_node_sort_key(&n.id));

        // Layout keeps Dagre's helper segments for reproducible layout artifacts. The family
        // preparation path also carries Mermaid's SVG-facing projection, where self-loops are
        // already merged into one visible edge. Fall back to the canonical edges for hand-built
        // layouts used by unit tests and older callers.
        let mut edges = if uses_canonical_routes || layout.render_edges.is_empty() {
            layout.edges.to_vec()
        } else {
            layout.render_edges.to_vec()
        };
        fn er_edge_sort_key(edge: &LayoutEdge) -> (i64, i64, i64) {
            // Mermaid's getEdgesToRender() appends merged self-loops after ordinary graph edges.
            let self_loop = i64::from(edge.from == edge.to);
            let id = edge.id.as_str();
            let Some(rest) = id.strip_prefix("er-rel-") else {
                return (self_loop, i64::MAX, i64::MAX);
            };
            let mut digits_len = 0usize;
            for ch in rest.chars() {
                if !ch.is_ascii_digit() {
                    break;
                }
                digits_len += ch.len_utf8();
            }
            if digits_len == 0 {
                return (self_loop, i64::MAX, i64::MAX);
            }
            let Ok(idx) = rest[..digits_len].parse::<i64>() else {
                return (self_loop, i64::MAX, i64::MAX);
            };
            (self_loop, idx, 0)
        }
        edges.sort_by_key(|edge| {
            let (self_loop, index, secondary) = er_edge_sort_key(edge);
            (if rounded_layout { 0 } else { self_loop }, index, secondary)
        });

        // Box and Rectpacking intentionally leave sections empty. Resolve their paint geometry
        // before measuring bounds, while retaining provenance to select a linear curve below.
        let mut missing_sections = FxHashSet::default();
        if is_elk_layout {
            let nodes_by_id: FxHashMap<_, _> =
                nodes.iter().map(|node| (node.id.as_str(), node)).collect();
            for edge in &mut edges {
                if edge.points.is_empty()
                    && let (Some(start), Some(end)) = (
                        nodes_by_id.get(edge.from.as_str()),
                        nodes_by_id.get(edge.to.as_str()),
                    )
                {
                    edge.points = elk_geometry::missing_rect_section_points(start, end);
                    if let Some(label) = &mut edge.label {
                        let first = &edge.points[0];
                        let last = &edge.points[edge.points.len() - 1];
                        label.x = (first.x + last.x) / 2.0;
                        label.y = (first.y + last.y) / 2.0;
                    }
                    missing_sections.insert(edge.id.clone());
                }
            }
        }

        if is_elk_layout {
            for edge in &edges {
                work_meter.charge(edge.points.len().saturating_add(2))?;
            }
            if effective_config
                .pointer("/elk/straightenEdges")
                .and_then(serde_json::Value::as_bool)
                != Some(false)
            {
                elk_terminal_jogs::straighten_edge_terminals(&mut edges, |units| {
                    work_meter.charge(units).map_err(Into::into)
                })?;
            }
            elk_terminal_jogs::separate_opposite_edge_labels(
                edges.iter_mut().map(|edge| {
                    let has_label = er_rel_idx_from_edge_id(&edge.id)
                        .and_then(|index| relationships.get(index))
                        .is_some_and(|relation| !relation.role_a.is_empty());
                    (
                        edge.from.as_str(),
                        edge.to.as_str(),
                        if has_label { edge.label.as_mut() } else { None },
                    )
                }),
                |units| work_meter.charge(units).map_err(Into::into),
            )?;
        }

        Ok(Self {
            clusters: layout.clusters,
            nodes,
            edges,
            missing_sections,
        })
    }

    pub fn nodes(&self) -> &[LayoutNode] {
        &self.nodes
    }

    pub fn edges(&self) -> &[LayoutEdge] {
        &self.edges
    }

    pub fn is_missing_section(&self, edge_id: &str) -> bool {
        self.missing_sections.contains(edge_id)
    }

    pub fn bounds(&self) -> Option<Bounds> {
        compute_layout_bounds(self.clusters, &self.nodes, &self.edges)
    }
}

pub fn er_rel_idx_from_edge_id(edge_id: &str) -> Option<usize> {
    let rest = edge_id.strip_prefix("er-rel-")?;
    let digits_len = rest
        .chars()
        .take_while(|ch| ch.is_ascii_digit())
        .map(char::len_utf8)
        .sum::<usize>();
    if digits_len == 0 {
        return None;
    }
    rest[..digits_len].parse::<usize>().ok()
}

/// Bounds for the layout projection before the SVG painter resolves its edge geometry.
pub fn er_layout_bounds(nodes: &[LayoutNode], edges: &[LayoutEdge]) -> Option<Bounds> {
    let mut points: Vec<(f64, f64)> = Vec::new();
    for node in nodes {
        let half_width = node.width / 2.0;
        let half_height = node.height / 2.0;
        points.push((node.x - half_width, node.y - half_height));
        points.push((node.x + half_width, node.y + half_height));
    }
    for edge in edges {
        points.extend(edge.points.iter().map(|point| (point.x, point.y)));
        if let Some(label) = &edge.label {
            let half_width = label.width / 2.0;
            let half_height = label.height / 2.0;
            points.push((label.x - half_width, label.y - half_height));
            points.push((label.x + half_width, label.y + half_height));
        }
    }
    Bounds::from_points(points)
}

// Layout-engine fallback used before SVG path insertion. This deliberately stays outside the SVG
// parity layer: rendered-path change detection and five-decimal placement belong to
// `svg::parity::edge_label_geometry`, while this function only supplies a missing Dagre/ELK anchor.
#[cfg(any(feature = "layout-dagre", feature = "layout-elk"))]
pub fn calc_label_position(points: &[LayoutPoint]) -> Option<(f64, f64)> {
    if points.is_empty() {
        return None;
    }
    if points.len() == 1 {
        return Some((points[0].x, points[0].y));
    }

    let mut total = 0.0;
    for i in 1..points.len() {
        let dx = points[i].x - points[i - 1].x;
        let dy = points[i].y - points[i - 1].y;
        total += (dx * dx + dy * dy).sqrt();
    }
    let mut remaining = total / 2.0;
    for i in 1..points.len() {
        let p0 = &points[i - 1];
        let p1 = &points[i];
        let dx = p1.x - p0.x;
        let dy = p1.y - p0.y;
        let seg = (dx * dx + dy * dy).sqrt();
        if seg == 0.0 {
            continue;
        }
        if seg < remaining {
            remaining -= seg;
            continue;
        }
        let t = (remaining / seg).clamp(0.0, 1.0);
        return Some((p0.x + t * dx, p0.y + t * dy));
    }
    Some((points.last()?.x, points.last()?.y))
}

fn compute_layout_bounds(
    clusters: &[LayoutCluster],
    nodes: &[LayoutNode],
    edges: &[LayoutEdge],
) -> Option<Bounds> {
    let mut b: Option<Bounds> = None;

    let mut include_rect = |min_x: f64, min_y: f64, max_x: f64, max_y: f64| {
        if let Some(ref mut cur) = b {
            cur.min_x = cur.min_x.min(min_x);
            cur.min_y = cur.min_y.min(min_y);
            cur.max_x = cur.max_x.max(max_x);
            cur.max_y = cur.max_y.max(max_y);
        } else {
            b = Some(Bounds {
                min_x,
                min_y,
                max_x,
                max_y,
            });
        }
    };

    for c in clusters {
        let hw = c.width / 2.0;
        let hh = c.height / 2.0;
        include_rect(c.x - hw, c.y - hh, c.x + hw, c.y + hh);
        let lhw = c.title_label.width / 2.0;
        let lhh = c.title_label.height / 2.0;
        include_rect(
            c.title_label.x - lhw,
            c.title_label.y - lhh,
            c.title_label.x + lhw,
            c.title_label.y + lhh,
        );
    }

    for n in nodes {
        let hw = n.width / 2.0;
        let hh = n.height / 2.0;
        include_rect(n.x - hw, n.y - hh, n.x + hw, n.y + hh);
    }

    for e in edges {
        for p in &e.points {
            include_rect(p.x, p.y, p.x, p.y);
        }
        for lbl in [
            e.label.as_ref(),
            e.start_label_left.as_ref(),
            e.start_label_right.as_ref(),
            e.end_label_left.as_ref(),
            e.end_label_right.as_ref(),
        ]
        .into_iter()
        .flatten()
        {
            let hw = lbl.width / 2.0;
            let hh = lbl.height / 2.0;
            include_rect(lbl.x - hw, lbl.y - hh, lbl.x + hw, lbl.y + hh);
        }
    }

    b
}
