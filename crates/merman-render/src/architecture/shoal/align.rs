use std::collections::{BTreeMap, BTreeSet};

use crate::Result;
use crate::graph_layout::ir::{Endpoint, MeasuredGraph, Point, Rect, Side, SolvedGraph};
use crate::layout_frame::{FrameDirection, GraphPlacement};
use crate::layout_work::OperationLayoutWorkControl;
use crate::model::LayoutLabel;
use merman_model::families::architecture::{ArchitectureDiagramModel, ArchitectureLayoutDirection};
use rustc_hash::FxHashMap;

use super::node_index;

mod route;

const MAX_MEMBERS: usize = 128;
const MAX_GEOMETRIES: usize = 512;

#[derive(Clone, Copy)]
struct Pair {
    first: usize,
    next: usize,
    row: bool,
}

/// Project explicit row and column equalities and ordering on top of Shoal's placement. A scope
/// accepts the projection together with its route changes only if it fits the solved geometry.
pub(super) fn apply(
    model: &ArchitectureDiagramModel,
    indices: &FxHashMap<&str, u32>,
    graph: &MeasuredGraph,
    solved: &mut SolvedGraph,
    placement: &mut GraphPlacement,
    work: &mut OperationLayoutWorkControl,
) -> Result<()> {
    if model.layout_hints.is_empty() {
        return Ok(());
    }
    let geometry_count = work.checked_add(
        work.checked_add(solved.nodes.len(), solved.groups.len())?,
        solved.edges.len(),
    )?;
    if geometry_count > MAX_GEOMETRIES {
        return Ok(());
    }
    let mut by_parent = BTreeMap::<Option<u32>, Vec<Pair>>::new();
    for hint in &model.layout_hints {
        let mut before: Option<usize> = None;
        for id in &hint.members {
            work.charge_adapter(id.len())?;
            let index = node_index(indices, id)? as usize;
            if let Some(first) = before {
                let parent = graph.nodes[index].parent;
                if graph.nodes[first].parent == parent {
                    by_parent.entry(parent).or_default().push(Pair {
                        first,
                        next: index,
                        row: hint.direction == ArchitectureLayoutDirection::Row,
                    });
                }
            }
            before = Some(index);
        }
    }
    let mut any = false;
    for (scope, pairs) in by_parent {
        work.charge_adapter(geometry_count)?;
        if apply_scope(scope, &pairs, graph, solved, work)? {
            any = true;
        }
    }
    if any {
        if let Some(observation) = &mut placement.adaptation {
            *observation = crate::LayoutAdaptationObservation::fit_width(
                observation.requested_width(),
                solved.bounds.width,
            );
        }
        if let Some(frame) = &mut placement.frame {
            let frame = std::sync::Arc::make_mut(frame);
            let levels = frame.nodes.len().checked_ilog2().unwrap_or(0) as usize + 1;
            work.charge_adapter(work.checked_mul(model.nodes.len(), levels)?)?;
            for (semantic, rect) in model.nodes.iter().zip(&solved.nodes) {
                let Some(node) = frame.nodes.get_mut(semantic.id.as_str()) else {
                    continue;
                };
                let center = rect.center();
                node.cross_key = match node.direction {
                    FrameDirection::Down | FrameDirection::Up => center.x,
                    FrameDirection::Right | FrameDirection::Left => center.y,
                };
                // Component-local anchor minima belong to the pre-projection geometry.
                node.position = None;
            }
        }
    }
    Ok(())
}

fn apply_scope(
    scope: Option<u32>,
    pairs: &[Pair],
    graph: &MeasuredGraph,
    solved: &mut SolvedGraph,
    work: &mut OperationLayoutWorkControl,
) -> Result<bool> {
    work.charge_adapter(work.checked_mul(pairs.len(), 4)?)?;
    let members: BTreeSet<_> = pairs
        .iter()
        .flat_map(|pair| [pair.first, pair.next])
        .collect();
    if members.len() > MAX_MEMBERS {
        return Ok(false);
    }
    let mut local = vec![None; solved.nodes.len()];
    let members: Vec<_> = members.into_iter().collect();
    for (slot, &node) in members.iter().enumerate() {
        local[node] = Some(slot);
    }
    let rects: Vec<_> = members.iter().map(|&index| solved.nodes[index]).collect();
    let bounds = scope.map(|id| {
        let parent = solved.groups[id as usize].rect;
        let padding = graph.groups[id as usize].padding;
        kurbo::Rect::new(
            parent.x + padding.x0,
            parent.y + padding.y0,
            parent.x + parent.width - padding.x1,
            parent.y + parent.height - padding.y1,
        )
    });
    let gap = graph.node_spacing.min(graph.rank_spacing);
    let Some(xs) = axis_centers(&rects, &local, pairs, gap, true, work)? else {
        return Ok(false);
    };
    let Some(ys) = axis_centers(&rects, &local, pairs, gap, false, work)? else {
        return Ok(false);
    };
    let proposed: Vec<_> = rects
        .iter()
        .zip(xs.into_iter().zip(ys))
        .map(|(&rect, (x, y))| Rect {
            x: x - rect.width * 0.5,
            y: y - rect.height * 0.5,
            ..rect
        })
        .collect();
    let new = |index: usize| local[index].map_or(solved.nodes[index], |slot| proposed[slot]);
    let changed = |index| new(index) != solved.nodes[index];
    work.charge_adapter(
        work.checked_mul(proposed.len(), solved.nodes.len() + solved.groups.len())?,
    )?;
    for (&index, &rect) in members.iter().zip(&proposed) {
        let rect = route::bbox(rect);
        if !rect.is_finite()
            || rect.x0 < 0.0
            || rect.y0 < 0.0
            || bounds.is_some_and(|bounds| !route::contains(bounds, rect))
            || solved
                .nodes
                .iter()
                .enumerate()
                .any(|(other, _)| other != index && route::overlaps(rect, route::bbox(new(other))))
            || solved.groups.iter().any(|group| {
                group.parent == scope && route::overlaps(rect, route::bbox(group.rect))
            })
        {
            return Ok(false);
        }
    }
    if !members.iter().copied().any(changed) {
        return Ok(false);
    }
    let Some(paths) = route::reconnect(scope, graph, solved, &new, &changed, work)? else {
        return Ok(false);
    };
    for (index, points) in paths {
        for point in &points {
            solved.bounds.width = solved.bounds.width.max(point.x);
            solved.bounds.height = solved.bounds.height.max(point.y);
        }
        solved.edges[index].points = points;
    }
    for (index, rect) in members.into_iter().zip(proposed) {
        solved.nodes[index] = rect;
        solved.bounds.width = solved.bounds.width.max(rect.x + rect.width);
        solved.bounds.height = solved.bounds.height.max(rect.y + rect.height);
    }
    Ok(true)
}

fn axis_centers(
    rects: &[Rect],
    local: &[Option<usize>],
    pairs: &[Pair],
    gap: f64,
    x: bool,
    work: &mut OperationLayoutWorkControl,
) -> Result<Option<Vec<f64>>> {
    work.charge_adapter(work.checked_mul(rects.len(), rects.len())?)?;
    let mut class: Vec<_> = (0..rects.len()).collect();
    work.charge_adapter(work.checked_mul(pairs.len(), rects.len())?)?;
    let pair_slots = |pair: &Pair| (local[pair.first].unwrap(), local[pair.next].unwrap());
    for pair in pairs.iter().filter(|pair| pair.row != x) {
        let (first, next) = pair_slots(pair);
        let (root, replaced) = (class[first], class[next]);
        if root != replaced {
            for id in &mut class {
                if *id == replaced {
                    *id = root;
                }
            }
        }
    }
    let mut next = vec![Vec::new(); rects.len()];
    let mut incoming = vec![0; rects.len()];
    for pair in pairs.iter().filter(|pair| pair.row == x) {
        let (first, following) = pair_slots(pair);
        let (a, b) = (class[first], class[following]);
        next[a].push(b);
        incoming[b] += 1;
    }
    let dims = |rect: Rect| {
        if x {
            (rect.x, rect.width)
        } else {
            (rect.y, rect.height)
        }
    };
    let mut half: Vec<f64> = vec![0.0; rects.len()];
    let mut order_key: Vec<f64> = vec![f64::INFINITY; rects.len()];
    let (mut low, mut high) = (f64::INFINITY, f64::NEG_INFINITY);
    for (&id, &rect) in class.iter().zip(rects) {
        let (start, size) = dims(rect);
        half[id] = half[id].max(size * 0.5);
        order_key[id] = order_key[id].min(start + size * 0.5);
        low = low.min(start);
        high = high.max(start + size);
    }
    let mut sorted: Vec<_> = (0..rects.len())
        .filter(|&v| order_key[v].is_finite())
        .collect();
    sorted.sort_unstable_by(|&a, &b| order_key[a].total_cmp(&order_key[b]).then(a.cmp(&b)));
    let mut centers = vec![None; rects.len()];
    let mut cursor = low;
    for _ in 0..sorted.len() {
        let Some(&v) = sorted
            .iter()
            .find(|&&v| incoming[v] == 0 && centers[v].is_none())
        else {
            return Ok(None);
        };
        centers[v] = Some(cursor + half[v]);
        cursor += 2.0 * half[v] + gap;
        for &to in &next[v] {
            incoming[to] -= 1;
        }
    }
    let shift = ((high - cursor + gap) * 0.5).max(0.0);
    Ok(Some(
        class
            .into_iter()
            .map(|id| centers[id].unwrap() + shift)
            .collect(),
    ))
}

fn label_rect(label: &LayoutLabel) -> kurbo::Rect {
    kurbo::Rect::from_center_size((label.x, label.y), (label.width, label.height))
}

fn node_endpoint(endpoint: Endpoint) -> Option<usize> {
    match endpoint {
        Endpoint::Node(node) => Some(node as usize),
        Endpoint::Group(_) => None,
    }
}

fn normal(side: Side) -> (f64, f64) {
    match side {
        Side::Top => (0.0, -1.0),
        Side::Right => (1.0, 0.0),
        Side::Bottom => (0.0, 1.0),
        Side::Left => (-1.0, 0.0),
    }
}

fn point(p: crate::graph_layout::ir::RoutePoint) -> Point {
    Point { x: p.x, y: p.y }
}
