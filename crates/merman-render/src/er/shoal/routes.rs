use crate::Result;
use crate::graph_layout::ir::{
    Axis, Direction, Endpoint, MeasuredEdge, MeasuredGraph, Rect as NodeRect, RoutePoint, Side,
    SolvedEdge, SolvedGraph,
};
use crate::graph_layout::route_paint::marker_bounds;
use crate::layout_work::OperationLayoutWorkControl;
use crate::model::LayoutLabel;
use kurbo::{Line, Point, Rect, Shape};
use merman_layout_shoal::geom::{Bounds, Bvh, BvhEntry};
use std::ops::ControlFlow::{Break, Continue};

use super::ErPreparedLabels;

const CAPTION_MARKER_GAP: f64 = 12.0;
const PARALLEL_JITTER_LIMIT: f64 = 3.0;

pub(super) fn reserve_caption_space(
    graph: &mut MeasuredGraph,
    directions: &[Direction],
    work: &mut OperationLayoutWorkControl,
) -> Result<()> {
    work.charge_adapter(graph.edges.len())?;
    for edge in &mut graph.edges {
        work.charge_adapter(0)?;
        let Some(label) = &mut edge.label else {
            continue;
        };
        if edge.source == edge.target || edge.start_marker.depth.max(edge.end_marker.depth) == 0.0 {
            continue;
        }
        let direction = match (edge.source, edge.target) {
            (Endpoint::Node(a), Endpoint::Node(b))
                if graph.nodes[a as usize].parent == graph.nodes[b as usize].parent =>
            {
                directions[a as usize]
            }
            _ => graph.direction,
        };
        // The solver seats the padded box against the marker runways. Only the measured
        // label is painted, so the spacing survives even when its anchor is near a port.
        match direction {
            Direction::Left | Direction::Right => label.width += 2.0 * CAPTION_MARKER_GAP,
            Direction::Down | Direction::Up => label.height += 2.0 * CAPTION_MARKER_GAP,
        }
    }
    Ok(())
}

#[derive(Clone, Copy, Default)]
struct Face {
    count: usize,
    peer: Option<(u32, Side)>,
}

impl Face {
    fn add(&mut self, peer: Option<(u32, Side)>) {
        if self.count == 0 {
            self.peer = peer;
        } else if self.peer != peer {
            self.peer = None;
        }
        self.count += 1;
    }

    fn parallel(self, peer: u32, side: Side) -> bool {
        self.count >= 2 && self.peer == Some((peer, side))
    }
}

pub(super) fn finish(
    graph: &MeasuredGraph,
    solved: &mut SolvedGraph,
    prepared: &ErPreparedLabels,
    shaft_outset: f64,
    work: &mut OperationLayoutWorkControl,
) -> Result<()> {
    work.charge_adapter(solved.edges.len())?;
    for (index, edge) in solved.edges.iter_mut().enumerate() {
        work.charge_adapter(0)?;
        if let (Some(label), Some(prepared)) = (&mut edge.label, prepared.relationship(index)) {
            (label.width, label.height) = prepared.dimensions();
        }
    }
    align_parallel(graph, solved, shaft_outset, work)
}

fn align_parallel(
    graph: &MeasuredGraph,
    solved: &mut SolvedGraph,
    shaft_outset: f64,
    work: &mut OperationLayoutWorkControl,
) -> Result<()> {
    work.charge_adapter(work.checked_add(solved.nodes.len(), solved.edges.len())?)?;
    let mut faces = vec![[Face::default(); 4]; solved.nodes.len()];
    for edge in &solved.edges {
        work.charge_adapter(0)?;
        for (endpoint, side, peer, peer_side) in [
            (edge.source, edge.ports.0, edge.target, edge.ports.1),
            (edge.target, edge.ports.1, edge.source, edge.ports.0),
        ] {
            if let Endpoint::Node(node) = endpoint {
                let peer = match peer {
                    Endpoint::Node(peer) => Some((peer, peer_side)),
                    Endpoint::Group(_) => None,
                };
                faces[node as usize][side as usize].add(peer);
            }
        }
    }
    work.charge_adapter(solved.edges.len())?;
    let mut proposals = Vec::new();
    let mut banks = Vec::new();
    let mut stitches = solved.border_stitches.iter().peekable();
    for (index, edge) in solved.edges.iter().enumerate() {
        work.charge_adapter(0)?;
        let (Endpoint::Node(a), Endpoint::Node(b)) = (edge.source, edge.target) else {
            continue;
        };
        let (from, to) = edge.ports;
        // A face shared with a different destination keeps its solver-assigned lanes.
        if a != b
            && from != to
            && from.axis() == to.axis()
            && graph.nodes[a as usize].parent == graph.nodes[b as usize].parent
            && faces[a as usize][from as usize].parallel(b, to)
            && faces[b as usize][to as usize].parallel(a, from)
        {
            while stitches.next_if(|stitch| stitch.edge < index).is_some() {
                work.charge_adapter(1)?;
            }
            if stitches.peek().is_some_and(|stitch| stitch.edge == index) {
                continue;
            }
            work.charge_adapter(edge.points.len())?;
            let bodies = [a, b].map(|node| solved.nodes[node as usize]);
            if let Some(alignment) = Alignment::propose(edge, from, bodies) {
                proposals
                    .try_reserve(1)
                    .and_then(|()| banks.try_reserve(1))
                    .map_err(|_| {
                        work.map_shoal_error(merman_layout_shoal::LayoutError::AllocationLimit)
                    })?;
                let bank = if a < b { (a, from) } else { (b, to) };
                banks.push(((bank.0, bank.1 as usize), proposals.len()));
                proposals.push((index, alignment));
            }
        }
    }
    if proposals.is_empty() {
        return Ok(());
    }

    let mut clearance = Clearance::new(graph, solved, &proposals, shaft_outset, work)?;
    work.charge_adapter(work.checked_mul(banks.len(), banks.len().ilog2() as usize + 3)?)?;
    banks.sort_unstable();
    for bank in banks.chunk_by(|a, b| a.0 == b.0) {
        let alignments = || bank.iter().map(|&(_, proposal)| proposals[proposal]);
        // Markers that already occupy their full pitch can move together. Validate the
        // proposed bank as a unit before trying individual moves against the old routes.
        let mut clear = bank.len() > 1;
        if clear {
            for (index, _) in alignments() {
                clearance.accepted[index] = true;
            }
            for (index, alignment) in alignments() {
                if !clearance.permits_alignment(
                    index,
                    &solved.edges[index],
                    &graph.edges[index],
                    alignment,
                    work,
                )? {
                    clear = false;
                    break;
                }
            }
            if !clear {
                for (index, _) in alignments() {
                    clearance.accepted[index] = false;
                }
            }
        }
        for (index, alignment) in alignments() {
            let edge = &mut solved.edges[index];
            if clear
                || clearance.permits_alignment(index, edge, &graph.edges[index], alignment, work)?
            {
                work.charge_adapter(if alignment.shaft {
                    edge.points.len()
                } else {
                    1
                })?;
                alignment.apply(edge);
                clearance.accepted[index] = true;
            }
        }
    }
    Ok(())
}

#[derive(Clone, Copy)]
struct Alignment {
    axis: Axis,
    center: f64,
    shaft: bool,
    caption: bool,
}

fn coordinates(axis: Axis, x: f64, y: f64) -> (f64, f64) {
    match axis {
        Axis::Horizontal => (x, y),
        Axis::Vertical => (y, x),
    }
}

impl Alignment {
    fn propose(edge: &SolvedEdge, from: Side, bodies: [NodeRect; 2]) -> Option<Self> {
        let [first, .., last] = edge.points.as_slice() else {
            return None;
        };
        let axis = from.axis();
        let (start_along, start) = coordinates(axis, first.x, first.y);
        let (end_along, end) = coordinates(axis, last.x, last.y);
        let direction = match from {
            Side::Top | Side::Left => -1.0,
            Side::Bottom | Side::Right => 1.0,
        };
        if (end_along - start_along) * direction <= 0.0
            || (start - end).abs() >= PARALLEL_JITTER_LIMIT
        {
            return None;
        }
        let [[a_min, a_max], [b_min, b_max]] = bodies.map(|body| {
            let lo = coordinates(axis, body.x, body.y).1;
            let span = coordinates(axis, body.width, body.height).1;
            [lo, lo + span]
        });
        let (lo, hi) = (a_min.max(b_min), a_max.min(b_max));
        if !lo.is_finite() || !hi.is_finite() || lo > hi {
            return None;
        }
        // A partly shared face can exclude the midpoint even when the two terminals are
        // within one pixel. Use the nearest legal shared row before checking the ink.
        let center = start.midpoint(end).clamp(lo, hi);
        if !center.is_finite() {
            return None;
        }
        let mut previous = start_along;
        let mut shaft = false;
        for p in &edge.points {
            let (along, cross) = coordinates(axis, p.x, p.y);
            if (cross - center).abs() >= PARALLEL_JITTER_LIMIT
                || (along - previous) * direction < 0.0
            {
                return None;
            }
            shaft |= cross != center;
            previous = along;
        }
        let caption = edge.label.as_ref().is_some_and(|label| {
            let cross = coordinates(axis, label.x, label.y).1;
            cross != center && (cross - center).abs() < PARALLEL_JITTER_LIMIT
        });
        (shaft || caption).then_some(Self {
            axis,
            center,
            shaft,
            caption,
        })
    }

    fn move_cross(self, x: &mut f64, y: &mut f64) {
        match self.axis {
            Axis::Horizontal => *y = self.center,
            Axis::Vertical => *x = self.center,
        }
    }

    fn port(self, mut port: RoutePoint) -> RoutePoint {
        self.move_cross(&mut port.x, &mut port.y);
        port.axis = self.axis;
        port
    }

    fn apply(self, edge: &mut SolvedEdge) {
        if self.caption
            && let Some(label) = &mut edge.label
        {
            self.move_cross(&mut label.x, &mut label.y);
        }
        if self.shaft {
            for port in &mut edge.points {
                *port = self.port(*port);
            }
            edge.points.dedup_by(|a, b| a.x == b.x && a.y == b.y);
            edge.corridor_clearance = 0.0;
        }
    }
}

fn route_boxes(
    edge: &SolvedEdge,
    spec: &MeasuredEdge,
    alignment: Option<Alignment>,
) -> [(Part, Option<Rect>); 3] {
    let caption = edge.label.as_ref().map(|label| {
        let (mut x, mut y) = (label.x, label.y);
        if let Some(alignment) = alignment.filter(|a| a.caption) {
            alignment.move_cross(&mut x, &mut y);
        }
        Rect::from_center_size((x, y), (label.width, label.height))
    });
    let [start, end] = [
        (
            edge.source,
            edge.points.first(),
            edge.ports.0,
            spec.start_marker,
        ),
        (
            edge.target,
            edge.points.last(),
            edge.ports.1,
            spec.end_marker,
        ),
    ]
    .map(|(endpoint, port, side, marker)| {
        let ink = port.and_then(|&port| {
            let port = alignment.filter(|a| a.shaft).map_or(port, |a| a.port(port));
            marker_bounds(port, side, marker)
        });
        (Part::Marker(endpoint), ink)
    });
    [(Part::Caption, caption), start, end]
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Part {
    Body(Endpoint),
    Frame,
    Title,
    Shaft,
    Caption,
    Marker(Endpoint),
}

#[derive(Clone, Copy)]
enum Ink {
    Box(Rect),
    Shaft(Line),
}

impl Ink {
    fn extent(self, outset: f64) -> Rect {
        match self {
            Self::Box(rect) => rect,
            Self::Shaft(line) => line.bounding_box().inflate(outset, outset),
        }
    }

    fn hits(self, area: Rect, outset: f64) -> bool {
        match self {
            Self::Box(rect) => rect.intersect(area).area() > 0.0,
            Self::Shaft(line) => {
                let area = area.inflate(outset, outset);
                area.overlaps(line.bounding_box())
                    && (area.contains(line.p0)
                        || area.contains(line.p1)
                        || area
                            .path_segments(0.0)
                            .any(|side| !side.intersect_line(line).is_empty()))
            }
        }
    }
}

struct Obstacle {
    part: Part,
    owner: Option<usize>,
    before: Ink,
    after: Option<Ink>,
}

impl Obstacle {
    fn fixed(part: Part, rect: Rect) -> Self {
        Self {
            part,
            owner: None,
            before: Ink::Box(rect),
            after: None,
        }
    }
}

struct Clearance {
    obstacles: Bvh<Obstacle>,
    accepted: Vec<bool>,
    shaft_outset: f64,
}

impl Clearance {
    fn new(
        graph: &MeasuredGraph,
        solved: &SolvedGraph,
        proposals: &[(usize, Alignment)],
        shaft_outset: f64,
        work: &mut OperationLayoutWorkControl,
    ) -> Result<Self> {
        let allocation = |work: &OperationLayoutWorkControl| {
            work.map_shoal_error(merman_layout_shoal::LayoutError::AllocationLimit)
        };
        let fixed = work.checked_add(
            solved.nodes.len(),
            work.checked_mul(solved.groups.len(), 2)?,
        )?;
        work.charge_adapter(fixed)?;
        let mut obstacles = Vec::new();
        obstacles
            .try_reserve_exact(fixed)
            .map_err(|_| allocation(work))?;
        let body =
            |rect: NodeRect| Rect::new(rect.x, rect.y, rect.x + rect.width, rect.y + rect.height);
        for (id, &node) in solved.nodes.iter().enumerate() {
            work.charge_adapter(0)?;
            obstacles.push(Obstacle::fixed(
                Part::Body(Endpoint::Node(id as u32)),
                body(node),
            ));
        }
        for group in &solved.groups {
            work.charge_adapter(0)?;
            obstacles.push(Obstacle::fixed(Part::Frame, body(group.rect)));
            if let Some(label) = &group.title {
                obstacles.push(Obstacle::fixed(Part::Title, label_rect(label)));
            }
        }
        let mut proposals = proposals.iter().peekable();
        for (index, (edge, spec)) in solved.edges.iter().zip(&graph.edges).enumerate() {
            let count = work.checked_add(edge.points.len(), 3)?;
            work.charge_adapter(count)?;
            obstacles.try_reserve(count).map_err(|_| allocation(work))?;
            let alignment = proposals
                .next_if(|(candidate, _)| *candidate == index)
                .map(|&(_, a)| a);
            let boxes = route_boxes(edge, spec, None);
            let aligned_boxes = alignment.map_or(boxes, |a| route_boxes(edge, spec, Some(a)));
            for ((part, before), (_, after)) in boxes.into_iter().zip(aligned_boxes) {
                if let Some(rect) = before {
                    obstacles.push(Obstacle {
                        part,
                        owner: Some(index),
                        before: Ink::Box(rect),
                        after: after.filter(|&next| next != rect).map(Ink::Box),
                    });
                }
            }
            for pair in edge.points.windows(2) {
                let [a, b] = [pair[0], pair[1]];
                obstacles.push(Obstacle {
                    part: Part::Shaft,
                    owner: Some(index),
                    before: Ink::Shaft(Line::new(point(a), point(b))),
                    after: alignment.filter(|a| a.shaft).map(|alignment| {
                        Ink::Shaft(Line::new(
                            point(alignment.port(a)),
                            point(alignment.port(b)),
                        ))
                    }),
                });
            }
        }
        // Each entry indexes both positions. Later proposals see the actual positions
        // of previous routes without rebuilding the index.
        let obstacles = Bvh::build_controlled(
            obstacles,
            |item| {
                let before = item.before.extent(shaft_outset);
                let rect = item
                    .after
                    .map_or(before, |after| before.union(after.extent(shaft_outset)));
                Bounds {
                    x: [rect.x0, rect.x1],
                    y: [rect.y0, rect.y1],
                }
            },
            work,
        )
        .map_err(|error| work.map_shoal_error(error))?;
        work.charge_adapter(solved.edges.len())?;
        let mut accepted = Vec::new();
        accepted
            .try_reserve_exact(solved.edges.len())
            .map_err(|_| allocation(work))?;
        accepted.resize(solved.edges.len(), false);
        Ok(Self {
            obstacles,
            accepted,
            shaft_outset,
        })
    }

    fn permits_alignment(
        &self,
        index: usize,
        edge: &SolvedEdge,
        spec: &MeasuredEdge,
        alignment: Alignment,
        work: &mut OperationLayoutWorkControl,
    ) -> Result<bool> {
        work.charge_adapter(1)?;
        let [first, .., last] = edge.points.as_slice() else {
            return Ok(false);
        };
        let shaft = alignment.shaft.then(|| {
            let start = point(alignment.port(*first));
            let end = point(alignment.port(*last));
            (
                Part::Shaft,
                Rect::from_points(start, end).inflate(self.shaft_outset, self.shaft_outset),
            )
        });
        let boxes = route_boxes(edge, spec, Some(alignment))
            .into_iter()
            .zip(route_boxes(edge, spec, None))
            .filter_map(|((part, next), (_, previous))| {
                next.filter(|&rect| Some(rect) != previous)
                    .map(|rect| (part, rect))
            });
        for (part, area) in shaft.into_iter().chain(boxes) {
            if !self.permits(index, edge, part, area, work)? {
                return Ok(false);
            }
        }
        Ok(true)
    }

    fn permits(
        &self,
        index: usize,
        edge: &SolvedEdge,
        part: Part,
        area: Rect,
        work: &mut OperationLayoutWorkControl,
    ) -> Result<bool> {
        let hit = self.obstacles.visit(|entry| -> Result<_> {
            work.charge_adapter(1)?;
            let item = match entry {
                BvhEntry::Node(bounds) => {
                    let [x0, x1] = bounds.x;
                    let [y0, y1] = bounds.y;
                    return Ok(Continue(area.overlaps(Rect::new(x0, y0, x1, y1))));
                }
                BvhEntry::Item(item) => item,
            };
            if let Part::Body(endpoint) = item.part
                && ((part == Part::Shaft && (endpoint == edge.source || endpoint == edge.target))
                    || part == Part::Marker(endpoint))
            {
                return Ok(Continue(true));
            }
            let ink = match item.owner {
                Some(owner) if owner == index => {
                    // The shaft, caption and markers share a centerline. Only the
                    // marker-to-caption and marker-to-marker clearances must be disjoint.
                    if part == item.part || part == Part::Shaft || item.part == Part::Shaft {
                        return Ok(Continue(true));
                    }
                    item.after.unwrap_or(item.before)
                }
                Some(owner) if self.accepted[owner] => item.after.unwrap_or(item.before),
                _ => item.before,
            };
            if item.part == Part::Frame && ink.extent(self.shaft_outset).contains_rect(area) {
                return Ok(Continue(true));
            }
            Ok(if ink.hits(area, self.shaft_outset) {
                Break(())
            } else {
                Continue(true)
            })
        })?;
        Ok(!hit.is_break())
    }
}

fn point(p: RoutePoint) -> Point {
    Point::new(p.x, p.y)
}

fn label_rect(label: &LayoutLabel) -> Rect {
    Rect::from_center_size((label.x, label.y), (label.width, label.height))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::graph_layout::ir::{MeasuredNode, RoutingStyle, Size};
    use crate::resources::{OperationWorkMeter, RenderResourcePolicy};
    use std::sync::Arc;

    fn relationship(points: &[(f64, f64)], label: Option<LayoutLabel>) -> SolvedEdge {
        SolvedEdge {
            source: Endpoint::Node(0),
            target: Endpoint::Node(1),
            points: points
                .iter()
                .map(|&(x, y)| RoutePoint::new(x, y, Axis::Horizontal))
                .collect(),
            ports: (Side::Right, Side::Left),
            label,
            start_label: None,
            end_label: None,
            corridor_clearance: 0.0,
            reversed_for_ranking: false,
        }
    }

    fn parallel_layout(edges: Vec<SolvedEdge>, obstacle: Option<NodeRect>) -> SolvedGraph {
        let mut nodes = [0.0, 80.0]
            .map(|x| NodeRect {
                x,
                y: 0.0,
                width: 20.0,
                height: 60.0,
            })
            .to_vec();
        nodes.extend(obstacle);
        let graph = MeasuredGraph {
            direction: Direction::Right,
            routing: RoutingStyle::Polyline,
            node_spacing: 10.0,
            rank_spacing: 10.0,
            component_spacing: 10.0,
            inherit_crossed_group_direction: false,
            nodes: nodes
                .iter()
                .enumerate()
                .map(|(i, node)| MeasuredNode {
                    size: Size {
                        width: node.width,
                        height: node.height,
                    },
                    contour: Default::default(),
                    seq: i as u32,
                    parent: None,
                    port_span: None,
                    perpendicular_port_span: None,
                    anchor_y: None,
                })
                .collect(),
            groups: Vec::new(),
            edges: edges
                .iter()
                .map(|edge| MeasuredEdge {
                    source: edge.source,
                    target: edge.target,
                    min_rank_span: 1,
                    weight: 1.0,
                    label: edge.label.as_ref().map(|label| Size {
                        width: label.width,
                        height: label.height,
                    }),
                    start_label: None,
                    end_label: None,
                    start_marker: Default::default(),
                    end_marker: Default::default(),
                })
                .collect(),
        };
        let mut solved = SolvedGraph {
            node_ranks: vec![0; nodes.len()],
            nodes,
            groups: Vec::new(),
            edges,
            border_stitches: Vec::new(),
            direction: graph.direction,
            bounds: Size {
                width: 100.0,
                height: 60.0,
            },
        };
        let mut work = OperationLayoutWorkControl::new(Arc::new(OperationWorkMeter::new(
            RenderResourcePolicy::constrained(),
        )));
        align_parallel(&graph, &mut solved, 0.5, &mut work).unwrap();
        solved
    }

    #[test]
    fn er_parallel_alignment_rejects_a_nan_center() {
        let edge = relationship(&[(20.0, f64::NAN), (80.0, 20.0)], None);
        let bodies = [0.0, 80.0].map(|x| NodeRect {
            x,
            y: 0.0,
            width: 20.0,
            height: 60.0,
        });
        assert!(Alignment::propose(&edge, Side::Right, bodies).is_none());
    }

    #[test]
    fn er_parallel_alignment_keeps_a_detour_around_a_foreign_entity() {
        let detour = relationship(
            &[
                (20.0, 19.0),
                (30.0, 19.0),
                (30.0, 18.0),
                (60.0, 18.0),
                (60.0, 21.0),
                (80.0, 21.0),
            ],
            None,
        );
        let original = detour.points.clone();
        let neighbor = relationship(&[(20.0, 40.0), (80.0, 42.0)], None);
        let obstacle = NodeRect {
            x: 42.0,
            y: 19.5,
            width: 14.0,
            height: 2.5,
        };
        let solved = parallel_layout(vec![detour, neighbor], Some(obstacle));
        assert_eq!(solved.edges[0].points, original);
        assert!(solved.edges[1].points.iter().all(|point| point.y == 41.0));
    }

    #[test]
    fn er_parallel_alignment_checks_a_caption_only_move_against_a_foreign_shaft() {
        let label = LayoutLabel {
            x: 50.0,
            y: 18.0,
            width: 10.0,
            height: 2.0,
        };
        let own = relationship(&[(20.0, 20.0), (80.0, 20.0)], Some(label));
        let neighbor = relationship(&[(20.0, 21.25), (80.0, 21.25)], None);
        let solved = parallel_layout(vec![own, neighbor], None);
        assert_eq!(solved.edges[0].label.as_ref().unwrap().y, 18.0);
    }

    #[test]
    fn er_parallel_alignment_respects_an_already_aligned_caption() {
        let caption = |y| {
            Some(LayoutLabel {
                x: 50.0,
                y,
                width: 10.0,
                height: 8.0,
            })
        };
        let upper = relationship(&[(20.0, 20.0), (80.0, 20.0)], caption(17.5));
        let lower = relationship(&[(20.0, 26.0), (80.0, 26.0)], caption(28.5));
        let solved = parallel_layout(vec![upper, lower], None);
        assert_eq!(solved.edges[0].label.as_ref().unwrap().y, 20.0);
        assert_eq!(solved.edges[1].label.as_ref().unwrap().y, 28.5);
    }
}
