//! Finish unlabelled directed fans in the clearance left by node and port placement.

use super::{Component, Csr, Drawing, canonical_size, contour_inset, physical_side};
use crate::geom::{Bounds, Bvh, BvhEntry};
use crate::ortho::comb::{Comb, MIN_DEGREE, corner_guard};
use crate::work::{LayoutResult, Work, filled, push, reserved};
use crate::{
    Axis, Graph, LayoutError, MIN_MARKER_LEAD, MarkerMetrics, NodeSpec, Options, Point,
    PortSideHint, Rect, RoutePoint, RoutingStyle, Side,
};
use std::ops::ControlFlow::{self, Break, Continue};

const MAX_SHAFTS: usize = 3;
const INVALID_BOUNDS: LayoutError = LayoutError::InvalidInput("invalid bus clearance bounds");

#[derive(Clone, Copy, PartialEq)]
struct Contact {
    at: Point,
    node: u32,
    side: Side,
    marker: MarkerMetrics,
    inset: f64,
}

impl Contact {
    fn offset(self, distance: f64) -> Point {
        let Point { x, y } = self.at;
        match self.side {
            Side::Top => Point { x, y: y - distance },
            Side::Right => Point { x: x + distance, y },
            Side::Bottom => Point { x, y: y + distance },
            Side::Left => Point { x: x - distance, y },
        }
    }

    fn marker_box(self) -> Option<Bounds> {
        // Retain both the measured and contour-adjusted footprints. The painter can
        // refine the contact independently while the solved route keeps its box endpoint.
        Bounds::marker_footprint(self.at, self.side, self.marker).map(|bounds| {
            bounds.union(Bounds::marker(
                self.offset(-self.inset),
                self.side,
                self.marker,
            ))
        })
    }
}

#[derive(Clone, Copy)]
struct Face {
    reach: f64,
    min_inset: f64,
}

impl Face {
    fn include(&mut self, tip: Contact, runway: f64) {
        self.reach = self.reach.max(runway.max(MIN_MARKER_LEAD));
        self.min_inset = self.min_inset.min(tip.inset);
    }

    fn lead(self, corner: f64) -> f64 {
        (self.reach - self.min_inset).max(0.0) + corner
    }
}

enum Owner {
    Body(u32),
    Label,
    Marker { edge: usize, end: usize },
}

struct Wall {
    bounds: Bounds,
    owner: Owner,
}

impl Wall {
    fn add(walls: &mut Vec<Self>, bounds: Bounds, owner: Owner) -> LayoutResult<()> {
        if !bounds.finite() {
            return Err(INVALID_BOUNDS);
        }
        if bounds.has_area() {
            push(walls, Self { bounds, owner })?;
        }
        Ok(())
    }
}

struct Scene {
    walls: Bvh<Wall>,
    ends: Vec<[Contact; 2]>,
    faces: Vec<[Face; 4]>,
}

impl Scene {
    fn new(
        drawing: &Drawing,
        c: &Component,
        graph: &Graph,
        work: &mut Work<'_>,
    ) -> LayoutResult<Self> {
        work.charge(drawing.nodes.len())?;
        let mut faces = filled(
            drawing.nodes.len(),
            [Face {
                reach: 0.0,
                min_inset: f64::INFINITY,
            }; 4],
        )?;
        let mut walls = reserved(drawing.nodes.len())?;
        for (node, &rect) in drawing.nodes.iter().enumerate() {
            work.visit(1)?;
            let mut bounds = Bounds::of(rect);
            if let Some(Point { x, y }) =
                super::diamond_offset(c.nodes[node].contour, graph.direction)
                && (x != 0.0 || y != 0.0)
            {
                bounds = bounds.union(Bounds::of(Rect {
                    x: rect.x + x,
                    y: rect.y + y,
                    ..rect
                }));
            }
            Wall::add(&mut walls, bounds, Owner::Body(node as u32))?;
        }
        work.charge(drawing.edges.len())?;
        let mut ends = reserved(drawing.edges.len())?;
        let contact = |knot: RoutePoint, node: u32, side: Side, marker| {
            let at = Point {
                x: knot.x,
                y: knot.y,
            };
            let center = drawing.nodes[node as usize].center();
            let offset = across(side, at) - across(side, center);
            let inset = contour_inset(c.nodes[node as usize], graph.direction, side, offset);
            Contact {
                at,
                node,
                side,
                marker,
                inset,
            }
        };
        for (index, (id, route)) in drawing.edges.iter().enumerate() {
            work.visit(1)?;
            let nodes = if let Some(link) = c.links.get(index) {
                [link.from, link.to]
            } else {
                [c.loops[index - c.links.len()].node; 2]
            };
            let spec = &graph.edges[*id as usize];
            let [first, .., last] = route.points.as_slice() else {
                return Err(LayoutError::InvalidInput("missing bus route contacts"));
            };
            let contacts = [
                contact(*first, nodes[0], route.ports.0, spec.start_marker),
                contact(*last, nodes[1], route.ports.1, spec.end_marker),
            ];
            for (end, (tip, runway)) in contacts.into_iter().zip(spec.marker_runways()).enumerate()
            {
                if ![tip.at.x, tip.at.y, tip.inset]
                    .into_iter()
                    .all(f64::is_finite)
                {
                    return Err(INVALID_BOUNDS);
                }
                faces[tip.node as usize][tip.side as usize].include(tip, runway);
                if let Some(bounds) = tip.marker_box() {
                    Wall::add(&mut walls, bounds, Owner::Marker { edge: index, end })?;
                }
            }
            ends.push(contacts);
            for (center, size) in [
                (route.label_center, spec.label),
                (route.start_label_center, spec.start_label),
                (route.end_label_center, spec.end_label),
            ] {
                if let (Some(p), Some(s)) = (center, size) {
                    Wall::add(
                        &mut walls,
                        Bounds::centered(p, canonical_size(graph.direction, s)),
                        Owner::Label,
                    )?;
                }
            }
        }
        let walls = Bvh::build_with_work(walls, |wall| wall.bounds, work)?;
        Ok(Self { walls, ends, faces })
    }

    fn lead(&self, tip: Contact, corner: f64) -> f64 {
        self.faces[tip.node as usize][tip.side as usize].lead(corner)
    }
}

#[derive(Clone, Copy)]
struct Runway {
    corner: f64,
    sign: f64,
}

impl Runway {
    fn depth(self, y: f64) -> f64 {
        self.sign * y
    }

    fn depths(self, [top, bottom]: [f64; 2]) -> [f64; 2] {
        if self.sign < 0.0 {
            [-bottom, -top]
        } else {
            [top, bottom]
        }
    }
}

struct Shaft {
    tip: Contact,
    knee: Point,
    x: [f64; 2],
}

impl Shaft {
    fn new(tip: Contact, scene: &Scene, corner: f64) -> Self {
        let knee = if tip.side.axis() == Axis::Horizontal {
            tip.offset(scene.lead(tip, corner))
        } else {
            tip.at
        };
        Self {
            tip,
            knee,
            x: [knee.x; 2],
        }
    }
}

struct Branch {
    edge: usize,
    leaf: Contact,
    hub: Contact,
    fixed: bool,
    shaft: usize,
}

fn across(side: Side, p: Point) -> f64 {
    match side.axis() {
        Axis::Vertical => p.x,
        Axis::Horizontal => p.y,
    }
}

fn shafts(
    branches: &mut [Branch],
    rect: Rect,
    node: &NodeSpec,
    corner: f64,
    runway: Runway,
    scene: &Scene,
    work: &mut Work<'_>,
) -> LayoutResult<Option<Vec<Shaft>>> {
    let center = rect.center();
    let distance = |tip: Contact| (across(tip.side, tip.at) - across(tip.side, center)).abs();
    let usable = |tip: Contact| {
        let (len, span) = match tip.side.axis() {
            Axis::Vertical => (rect.width, node.port_span),
            Axis::Horizontal => (rect.height, node.perpendicular_port_span.or(node.port_span)),
        };
        let inset = corner.min(len * 0.5).max(tip.marker.width * 0.5);
        distance(tip) <= (len * 0.5 - inset).min(span.unwrap_or(len) * 0.5)
    };
    let mut shafts: Vec<Shaft> = reserved(MAX_SHAFTS)?;
    // Every shaft reuses an existing contact and marker footprint. Later fans can only
    // remove marker walls from the set that previously checked routes had to clear.
    // Register fixed offsets first, so free contacts can use them when they fit.
    for branch in branches.iter_mut() {
        work.visit(1)?;
        if !branch.fixed {
            continue;
        }
        let tip = branch.hub;
        branch.shaft = if let Some(index) = shafts.iter().position(|s| s.tip == tip) {
            index
        } else if shafts.len() < MAX_SHAFTS {
            shafts.push(Shaft::new(tip, scene, runway.corner));
            shafts.len() - 1
        } else {
            return Ok(None);
        };
    }
    for index in 0..branches.len() {
        work.visit(1)?;
        if branches[index].fixed {
            continue;
        }
        let hub = branches[index].hub;
        let matches =
            |tip: Contact| tip.side == hub.side && tip.marker == hub.marker && usable(tip);
        branches[index].shaft = if let Some(shaft) = shafts.iter().position(|s| matches(s.tip)) {
            shaft
        } else {
            if shafts.len() == MAX_SHAFTS {
                return Ok(None);
            }
            work.visit(branches.len())?;
            let Some(tip) = branches
                .iter()
                .map(|b| b.hub)
                .filter(|&tip| matches(tip))
                .min_by(|&a, &b| distance(a).total_cmp(&distance(b)))
            else {
                return Ok(None);
            };
            shafts.push(Shaft::new(tip, scene, runway.corner));
            shafts.len() - 1
        };
    }
    work.visit(branches.len())?;
    for branch in branches {
        let [lo, hi] = &mut shafts[branch.shaft].x;
        *lo = lo.min(branch.leaf.at.x);
        *hi = hi.max(branch.leaf.at.x);
    }
    Ok(Some(shafts))
}

struct Obstacles<'a> {
    scene: &'a Scene,
    hub: u32,
    shafts: &'a [Shaft],
}

impl Obstacles<'_> {
    /// Visit only obstacles inside a swept leg or shelf. The index keeps its original
    /// bounds because each successful fan reuses a subset of its original marker boxes.
    fn visit(
        &self,
        query: Bounds,
        terminal: Option<Contact>,
        work: &mut Work<'_>,
        mut hit: impl FnMut(Bounds) -> LayoutResult<ControlFlow<()>>,
    ) -> LayoutResult<ControlFlow<()>> {
        if !query.finite() {
            return Err(INVALID_BOUNDS);
        }
        self.scene.walls.visit(|entry| {
            work.visit(1)?;
            let wall = match entry {
                BvhEntry::Node(bounds) => return Ok(Continue(bounds.overlaps(query))),
                BvhEntry::Item(wall) => wall,
            };
            if !wall.bounds.overlaps_strictly(query) {
                return Ok(Continue(true));
            }
            let ignore = match wall.owner {
                Owner::Body(node) => terminal.is_some_and(|tip| tip.node == node),
                Owner::Label => false,
                Owner::Marker { edge, end } => {
                    let tip = self.scene.ends[edge][end];
                    terminal == Some(tip)
                        || tip.marker_box() != Some(wall.bounds)
                        || (tip.node == self.hub && !self.shafts.iter().any(|s| s.tip == tip))
                }
            };
            if !ignore && hit(wall.bounds)?.is_break() {
                return Ok(Break(()));
            }
            Ok(Continue(true))
        })
    }

    fn clear(
        &self,
        query: Bounds,
        terminal: Option<Contact>,
        work: &mut Work<'_>,
    ) -> LayoutResult<bool> {
        Ok(self
            .visit(query, terminal, work, |_| Ok(Break(())))?
            .is_continue())
    }
}

/// Vertical obstructions tighten a shelf limit. Horizontal obstructions remove intervals
/// from the remaining range, including the area swept by the guarded rounded corners.
fn shelf(
    scene: &Scene,
    hub: u32,
    face: f64,
    branches: &[Branch],
    shafts: &[Shaft],
    runway: Runway,
    work: &mut Work<'_>,
) -> LayoutResult<Option<f64>> {
    let depth = |y| runway.depth(y);
    let corner = runway.corner;
    let mut low = depth(face) + corner;
    let mut high = f64::INFINITY;
    for shaft in shafts {
        let lead = if shaft.tip.side.axis() == Axis::Vertical {
            scene.lead(shaft.tip, corner)
        } else {
            2.0 * corner
        };
        low = low.max(depth(shaft.knee.y) + lead);
        if !shaft.knee.x.is_finite() {
            return Ok(None);
        }
    }
    work.visit(branches.len())?;
    for branch in branches {
        high = high.min(depth(branch.leaf.at.y) - scene.lead(branch.leaf, corner));
    }
    if !low.is_finite()
        || !high.is_finite()
        || low >= high
        || shafts.iter().all(|s| s.x[0] == s.x[1])
    {
        return Ok(None);
    }
    let preferred = low.midpoint(high);
    let obstacles = Obstacles { scene, hub, shafts };
    for shaft in shafts {
        let Shaft { tip, knee, .. } = *shaft;
        if let Some(marker) = tip.marker_box()
            && !obstacles.clear(marker, Some(tip), work)?
        {
            return Ok(None);
        }
        let contour = tip.offset(-tip.inset);
        let side = tip.side.axis() == Axis::Horizontal;
        if side {
            let terminal = Bounds::between(tip.at, contour).union(Bounds::between(tip.at, knee));
            let corner_start = corner_guard(knee, tip.at, Axis::Horizontal, corner);
            let rounded = Bounds::between(
                corner_start,
                Point {
                    y: knee.y + runway.sign * corner,
                    ..knee
                },
            );
            if !obstacles.clear(terminal, Some(tip), work)?
                || !obstacles.clear(rounded, None, work)?
            {
                return Ok(None);
            }
        }
        let start = if side {
            depth(knee.y)
        } else {
            depth(knee.y).min(depth(contour.y))
        };
        let query = Bounds {
            x: [knee.x; 2],
            y: runway.depths([start, high]),
        };
        let _ = obstacles.visit(query, (!side).then_some(tip), work, |wall| {
            high = high.min(runway.depths(wall.y)[0] - corner);
            Ok(Continue(()))
        })?;
        if low >= high {
            return Ok(None);
        }
    }
    for branch in branches {
        let tip = branch.leaf;
        let end = depth(tip.at.y).max(depth(tip.offset(-tip.inset).y));
        let query = Bounds {
            x: [tip.at.x; 2],
            y: runway.depths([low, end]),
        };
        let _ = obstacles.visit(query, Some(tip), work, |wall| {
            low = low.max(runway.depths(wall.y)[1] + corner);
            Ok(Continue(()))
        })?;
        if low >= high {
            return Ok(None);
        }
    }
    let mut blocked = Vec::new();
    for shaft in shafts.iter().filter(|s| s.x[0] < s.x[1]) {
        let query = Bounds {
            x: shaft.x,
            y: runway.depths([low - corner, high + corner]),
        };
        let _ = obstacles.visit(query, None, work, |wall| {
            let [start, end] = runway.depths(wall.y);
            push(&mut blocked, [start - corner, end + corner])?;
            Ok(Continue(()))
        })?;
    }
    work.charge_sort(blocked.len())?;
    blocked.sort_unstable_by(|a, b| a[0].total_cmp(&b[0]));
    let mut best: Option<f64> = None;
    for [start, end] in blocked.into_iter().chain([[high, high]]) {
        work.visit(1)?;
        if low <= start.min(high) {
            let candidate = preferred.clamp(low, start.min(high));
            if best.is_none_or(|y| (candidate - preferred).abs() < (y - preferred).abs()) {
                best = Some(candidate);
            }
        }
        low = low.max(end);
        if low > high {
            break;
        }
    }
    Ok(best.map(depth))
}

struct Edit {
    edge: usize,
    points: Vec<RoutePoint>,
}

fn plan(
    drawing: &Drawing,
    c: &Component,
    graph: &Graph,
    options: &Options,
    work: &mut Work<'_>,
) -> LayoutResult<Vec<Edit>> {
    let count = c.nodes.len();
    // Loops are both incoming and outgoing, so they disqualify their hub automatically.
    let loops = || c.loops.iter().map(|l| l.node);
    let incoming = Csr::new(count, c.links.iter().map(|e| e.to).chain(loops()), work)?;
    let outgoing = Csr::new(count, c.links.iter().map(|e| e.from).chain(loops()), work)?;
    let mut scene = None;
    let mut edited = filled(c.links.len(), false)?;
    let mut edits = Vec::new();
    for hub in 0..count as u32 {
        work.visit(1)?;
        let [input, output] = [incoming.at(hub), outgoing.at(hub)];
        let (ids, end, hub_face, leaf_face, sign) = match (input, output) {
            (ids, []) if ids.len() >= MIN_DEGREE => (ids, 1, Side::Top, Side::Bottom, -1.0),
            ([], ids) if ids.len() >= MIN_DEGREE => (ids, 0, Side::Bottom, Side::Top, 1.0),
            _ => continue,
        };
        let mut eligible = true;
        for &id in ids {
            work.visit(1)?;
            let i = id as usize;
            let (id, edge) = &drawing.edges[i];
            let spec = &graph.edges[*id as usize];
            eligible &= !edited[i]
                && !edge.reversed_for_ranking
                && spec.label.is_none()
                && spec.start_label.is_none()
                && spec.end_label.is_none();
        }
        if !eligible {
            continue;
        }
        let scene = match &mut scene {
            Some(scene) => scene,
            slot => slot.insert(Scene::new(drawing, c, graph, work)?),
        };
        let node = &c.nodes[hub as usize];
        let rect = drawing.nodes[hub as usize];
        let runway = Runway {
            // Guard half of the configured separation, with room for the painter's
            // minimum straight lead when the caller requests very tight spacing.
            corner: options.spacing.edge.max(MIN_MARKER_LEAD) * 0.5,
            sign,
        };
        let mut branches = reserved(ids.len())?;
        for &id in ids {
            work.visit(1)?;
            let edge = id as usize;
            let [tip, leaf] = [scene.ends[edge][end], scene.ends[edge][1 - end]];
            if leaf.side != leaf_face || (tip.side.axis() == Axis::Vertical && tip.side != hub_face)
            {
                break;
            }
            let spec = &graph.edges[c.links[edge].edge as usize];
            let hint = if end == 0 {
                spec.source_port
            } else {
                spec.target_port
            };
            let fixed = matches!(hint, PortSideHint::FixedOffset { .. })
                || (node.anchor_y.is_some()
                    && physical_side(graph.direction, tip.side).axis() == Axis::Horizontal);
            branches.push(Branch {
                edge,
                leaf,
                hub: tip,
                fixed,
                shaft: 0,
            });
        }
        if branches.len() != ids.len() {
            continue;
        }
        let Some(shafts) = shafts(
            &mut branches,
            rect,
            node,
            options.spacing.corner_clearance,
            runway,
            scene,
            work,
        )?
        else {
            continue;
        };
        let face = if end == 0 {
            rect.y + rect.height
        } else {
            rect.y
        };
        let Some(level) = shelf(scene, hub, face, &branches, &shafts, runway, work)? else {
            continue;
        };
        let comb = Comb {
            travel: Axis::Vertical,
            shelf: level,
            corner: runway.corner,
        };
        let start = edits.len();
        for branch in &branches {
            let shaft = &shafts[branch.shaft];
            if shaft.tip.side.axis() == Axis::Horizontal && shaft.tip.at == shaft.knee {
                break;
            }
            let Some(points) = comb.branch(
                shaft.tip.at,
                shaft.knee,
                branch.leaf.at,
                runway.sign < 0.0,
                work,
            )?
            else {
                break;
            };
            push(
                &mut edits,
                Edit {
                    edge: branch.edge,
                    points,
                },
            )?;
        }
        if edits.len() - start != branches.len() {
            edits.truncate(start);
            continue;
        }
        work.visit(branches.len())?;
        for branch in &branches {
            edited[branch.edge] = true;
            scene.ends[branch.edge][end] = shafts[branch.shaft].tip;
        }
    }
    Ok(edits)
}

/// Bundle final canonical Down links for orthogonal geometry, including rounded paint.
/// A true result requests shared-segment crossing compression.
pub(super) fn bundle(
    drawing: &mut Drawing,
    c: &Component,
    graph: &Graph,
    options: &Options,
    work: &mut Work<'_>,
) -> LayoutResult<bool> {
    if !options.bus_routing
        || options.routing != RoutingStyle::Orthogonal
        || c.links.len() < MIN_DEGREE
        || c.boundaries
        || c.pinned
        || c.group_proxies
    {
        return Ok(false);
    }
    let edits = match work.compare(|trial| plan(drawing, c, graph, options, trial)) {
        Ok(Some(edits)) => edits,
        Ok(None) | Err(LayoutError::AllocationLimit | LayoutError::InvalidInput(_)) => {
            return Ok(false);
        }
        Err(error) => return Err(error),
    };
    let shared = !edits.is_empty();
    for edit in edits {
        let route = &mut drawing.edges[edit.edge].1;
        route.points = edit.points;
        route.corridor_clearance = 0.0;
        route.shared_bus = true;
    }
    Ok(shared)
}

#[cfg(test)]
mod tests;
