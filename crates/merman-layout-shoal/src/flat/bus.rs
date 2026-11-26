//! Finish unlabelled directed fans in the clearance left by node and port placement.

use super::{Component, Csr, Drawing, canonical_size, physical_side, validate};
use crate::geom::Bounds;
use crate::work::{LayoutResult, Work, filled, push, reserved};
use crate::{
    Axis, Graph, LayoutError, MarkerMetrics, NodeSpec, Options, Point, PortSideHint, Rect,
    RoutePoint, RoutingStyle, Side,
};

const MIN_DEGREE: usize = 6;
const MAX_SHAFTS: usize = 3;
const MAX_CLEARANCE_CHECKS: usize = 256 * 1024;
const MIN_LEAD: f64 = 6.0;

#[derive(Clone, Copy, PartialEq)]
struct Contact {
    at: Point,
    node: u32,
    side: Side,
    marker: MarkerMetrics,
}

impl Contact {
    fn marker_box(self) -> Option<Bounds> {
        let MarkerMetrics { length, width } = self.marker;
        if length == 0.0 || width == 0.0 {
            return None;
        }
        Some(Bounds::marker(self.at, self.side, self.marker))
    }
}

fn overlaps(a: [f64; 2], b: [f64; 2]) -> bool {
    a[0].max(b[0]) < a[1].min(b[1])
}

fn expand([lo, hi]: [f64; 2], gap: f64) -> [f64; 2] {
    [lo - gap, hi + gap]
}

fn between([lo, hi]: [f64; 2], at: f64) -> bool {
    lo < at && at < hi
}

struct Scene {
    walls: Vec<Bounds>,
    ends: Vec<[Contact; 2]>,
}

impl Scene {
    fn new(
        drawing: &Drawing,
        c: &Component,
        graph: &Graph,
        work: &mut Work<'_>,
    ) -> LayoutResult<Self> {
        work.charge(drawing.nodes.len())?;
        let mut walls = reserved(drawing.nodes.len())?;
        walls.extend(drawing.nodes.iter().copied().map(Bounds::of));
        work.charge(drawing.edges.len())?;
        let mut ends = reserved(drawing.edges.len())?;
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
            ends.push([
                Contact {
                    at: Point {
                        x: first.x,
                        y: first.y,
                    },
                    node: nodes[0],
                    side: route.ports.0,
                    marker: spec.start_marker,
                },
                Contact {
                    at: Point {
                        x: last.x,
                        y: last.y,
                    },
                    node: nodes[1],
                    side: route.ports.1,
                    marker: spec.end_marker,
                },
            ]);
            for (center, size) in [
                (route.label_center, spec.label),
                (route.start_label_center, spec.start_label),
                (route.end_label_center, spec.end_label),
            ] {
                if let (Some(p), Some(s)) = (center, size) {
                    push(
                        &mut walls,
                        Bounds::centered(p, canonical_size(graph.direction, s)),
                    )?;
                }
            }
        }
        Ok(Self { walls, ends })
    }
}

#[derive(Clone, Copy)]
struct Runway {
    corner: f64,
    sign: f64,
}

impl Runway {
    fn lead(self, marker: MarkerMetrics) -> f64 {
        marker.length.max(MIN_LEAD) + self.corner * 2.0
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
    marker: Option<Bounds>,
}

impl Shaft {
    fn new(tip: Contact, runway: Runway) -> Self {
        let lateral = match tip.side {
            Side::Left => -runway.lead(tip.marker),
            Side::Right => runway.lead(tip.marker),
            _ => 0.0,
        };
        let knee = Point {
            x: tip.at.x + lateral,
            y: tip.at.y,
        };
        Self {
            tip,
            knee,
            x: [knee.x; 2],
            marker: tip.marker_box(),
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
    work: &mut Work<'_>,
) -> LayoutResult<Option<Vec<Shaft>>> {
    let center = rect.center();
    let distance = |tip: Contact| (across(tip.side, tip.at) - across(tip.side, center)).abs();
    let usable = |tip: Contact| {
        let (len, span) = match tip.side.axis() {
            Axis::Vertical => (rect.width, node.port_span),
            Axis::Horizontal => (rect.height, node.perpendicular_port_span),
        };
        let inset = corner.min(len * 0.5).max(tip.marker.width * 0.5);
        distance(tip) <= (len * 0.5 - inset).min(span.unwrap_or(len) * 0.5)
    };
    let mut preferred: [Option<Contact>; 4] = [None; 4];
    for branch in branches.iter() {
        work.visit(1)?;
        let tip = branch.hub;
        let current = &mut preferred[tip.side as usize];
        if !branch.fixed && usable(tip) && current.is_none_or(|old| distance(tip) < distance(old)) {
            *current = Some(tip);
        }
    }
    let mut shafts: Vec<Shaft> = reserved(MAX_SHAFTS)?;
    // Every shaft reuses an existing contact and marker footprint. Later fans can only
    // remove marker walls from the set that previously checked routes had to clear.
    // Register fixed offsets first, so free contacts can use them when they fit.
    for branch in branches.iter_mut().filter(|b| b.fixed) {
        work.visit(1)?;
        let tip = branch.hub;
        branch.shaft = if let Some(index) = shafts.iter().position(|s| s.tip == tip) {
            index
        } else if shafts.len() < MAX_SHAFTS {
            shafts.push(Shaft::new(tip, runway));
            shafts.len() - 1
        } else {
            return Ok(None);
        };
    }
    for branch in branches.iter_mut().filter(|b| !b.fixed) {
        work.visit(1)?;
        let hub = branch.hub;
        let existing = shafts
            .iter()
            .position(|s| s.tip.side == hub.side && s.tip.marker == hub.marker && usable(s.tip));
        branch.shaft = if let Some(index) = existing {
            index
        } else if let Some(tip) = preferred[hub.side as usize]
            && hub.marker == tip.marker
            && shafts.len() < MAX_SHAFTS
        {
            shafts.push(Shaft::new(tip, runway));
            shafts.len() - 1
        } else {
            return Ok(None);
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

/// Project the obstacles onto the bus axis. A vertical leg can only grow towards the bus, so
/// its first obstruction tightens one end of the available interval. Horizontal obstructions
/// remove intervals from that range. The additional pad contains the guarded corner turns.
fn shelf(
    scene: &Scene,
    hub: u32,
    face: f64,
    branches: &[Branch],
    shafts: &[Shaft],
    runway: Runway,
    work: &mut Work<'_>,
) -> LayoutResult<Option<f64>> {
    let depth = |y| runway.sign * y;
    let pad = 2.0 * runway.corner;
    let mut low = depth(face) + pad;
    let mut high = f64::INFINITY;
    for shaft in shafts {
        let lead = if shaft.tip.side.axis() == Axis::Vertical {
            runway.lead(shaft.tip.marker)
        } else {
            pad
        };
        low = low.max(depth(shaft.knee.y) + lead);
        if !shaft.knee.x.is_finite() || shaft.marker.is_some_and(|b| !b.finite()) {
            return Ok(None);
        }
    }
    work.visit(branches.len())?;
    for branch in branches {
        high = high.min(depth(branch.leaf.at.y) - runway.lead(branch.leaf.marker));
    }
    if !low.is_finite()
        || !high.is_finite()
        || low >= high
        || shafts.iter().all(|s| s.x[0] == s.x[1])
    {
        return Ok(None);
    }
    let preferred = low.midpoint(high);
    work.visit(2 * scene.ends.len())?;
    let markers = scene
        .ends
        .iter()
        .flatten()
        .copied()
        .filter(|p| p.node != hub)
        .chain(shafts.iter().map(|s| s.tip));
    let mut blocked = Vec::new();
    let mut consider = |wall: Bounds, owner: Option<Contact>| -> LayoutResult<bool> {
        work.visit(shafts.len() + 1)?;
        if !wall.finite() {
            return Ok(false);
        }
        let cross = expand(wall.x, pad);
        let [top, bottom] = runway.depths(wall.y);
        let avoid = expand([top, bottom], pad);
        let mut blocks_shelf = false;
        for shaft in shafts {
            if owner != Some(shaft.tip) {
                if shaft
                    .marker
                    .is_some_and(|m| overlaps(wall.x, m.x) && overlaps(wall.y, m.y))
                {
                    return Ok(false);
                }
                if shaft.tip.side.axis() == Axis::Horizontal {
                    let [a, b] = [shaft.tip.at.x, shaft.knee.x];
                    if overlaps(wall.x, [a.min(b), a.max(b)]) && between(avoid, depth(shaft.knee.y))
                    {
                        return Ok(false);
                    }
                }
                if between(cross, shaft.knee.x) && bottom > depth(shaft.knee.y) {
                    high = high.min(avoid[0]);
                }
            }
            blocks_shelf |= overlaps(cross, shaft.x);
        }
        if avoid[1] > low {
            work.visit(branches.len())?;
            for branch in branches {
                let tip = branch.leaf;
                if owner != Some(tip) && between(cross, tip.at.x) && top < depth(tip.at.y) {
                    low = low.max(avoid[1]);
                    break;
                }
            }
        }
        if blocks_shelf && overlaps(avoid, [low, high]) {
            push(&mut blocked, avoid)?;
        }
        Ok(low < high)
    };
    for &wall in &scene.walls {
        if !consider(wall, None)? {
            return Ok(None);
        }
    }
    for tip in markers {
        if let Some(wall) = tip.marker_box()
            && !consider(wall, Some(tip))?
        {
            return Ok(None);
        }
    }
    validate::charge_sort(blocked.len(), work)?;
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

fn point(at: Point, axis: Axis) -> RoutePoint {
    RoutePoint::new(at.x, at.y, axis)
}

fn axis(a: Point, b: Point) -> Axis {
    if a.x == b.x {
        Axis::Vertical
    } else {
        Axis::Horizontal
    }
}

fn guard(a: Point, b: Point, axis: Axis, corner: f64) -> Point {
    let toward = |a: f64, b: f64| {
        if (b - a).abs() <= 2.0 * corner {
            a.midpoint(b)
        } else {
            a + (b - a).signum() * corner
        }
    };
    match axis {
        Axis::Vertical => Point {
            y: toward(a.y, b.y),
            ..a
        },
        Axis::Horizontal => Point {
            x: toward(a.x, b.x),
            ..a
        },
    }
}

fn route(
    shaft: &Shaft,
    leaf: Contact,
    level: f64,
    runway: Runway,
    work: &mut Work<'_>,
) -> LayoutResult<Option<Vec<RoutePoint>>> {
    let hub_room = if shaft.tip.side.axis() == Axis::Vertical {
        (level - shaft.tip.at.y).abs()
    } else {
        (shaft.knee.x - shaft.tip.at.x).abs()
    };
    if hub_room <= shaft.tip.marker.length.max(MIN_LEAD) + runway.corner
        || (level - leaf.at.y).abs() <= leaf.marker.length.max(MIN_LEAD) + runway.corner
    {
        return Ok(None);
    }
    let mut turns = [Point::default(); 5];
    let mut count = 0;
    for at in [
        shaft.tip.at,
        shaft.knee,
        Point {
            x: shaft.knee.x,
            y: level,
        },
        Point {
            x: leaf.at.x,
            y: level,
        },
        leaf.at,
    ] {
        if count > 0 && turns[count - 1] == at {
            continue;
        }
        if count > 1 && axis(turns[count - 2], turns[count - 1]) == axis(turns[count - 1], at) {
            count -= 1;
        }
        turns[count] = at;
        count += 1;
    }
    let turns = &mut turns[..count];
    if runway.sign < 0.0 {
        turns.reverse();
    }
    let [first, second, ..] = turns else {
        return Ok(None);
    };
    let first = point(*first, axis(*first, *second));
    let capacity = 1 + 3 * (turns.len() - 1);
    work.charge(capacity)?;
    let mut points = reserved(capacity)?;
    points.push(first);
    for (index, leg) in turns.windows(2).enumerate() {
        let [a, b] = [leg[0], leg[1]];
        let axis = axis(a, b);
        // Preserve straight guard knots so curved painters cannot consume the shared shaft
        // or cut past the corner clearance checked around the orthogonal centerline.
        for g in [
            (index > 0).then(|| guard(a, b, axis, runway.corner)),
            (index + 2 < turns.len()).then(|| guard(b, a, axis, runway.corner)),
        ]
        .into_iter()
        .flatten()
        {
            if g == a || g == b {
                return Ok(None);
            }
            let knot = RoutePoint::gap_boundary(g.x, g.y, axis);
            if points.last() != Some(&knot) {
                points.push(knot);
            }
        }
        points.push(point(b, axis));
    }
    Ok(Some(points))
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
    let walls = count.saturating_add(drawing.edges.len().saturating_mul(5));
    let mut allowance = MAX_CLEARANCE_CHECKS;
    if walls.saturating_mul(MIN_DEGREE) > allowance {
        return Ok(Vec::new());
    }
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
        let cost = (walls + MAX_SHAFTS).saturating_mul(ids.len() + MAX_SHAFTS + 1);
        let Some(left) = allowance.checked_sub(cost) else {
            continue;
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
        allowance = left;
        let scene = match &mut scene {
            Some(scene) => scene,
            slot => slot.insert(Scene::new(drawing, c, graph, work)?),
        };
        let node = &c.nodes[hub as usize];
        let rect = drawing.nodes[hub as usize];
        let runway = Runway {
            corner: options.spacing.edge.max(2.0),
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
        let start = edits.len();
        for branch in &branches {
            let Some(points) = route(&shafts[branch.shaft], branch.leaf, level, runway, work)?
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

/// Bundle final canonical Down links when orthogonal bus routing is enabled. The caller
/// establishes paint compatibility for the whole fan. A true result requests shared-segment
/// crossing compression.
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
        Ok(None) | Err(LayoutError::AllocationLimit) => return Ok(false),
        Err(error) => return Err(error),
    };
    let shared = !edits.is_empty();
    for edit in edits {
        drawing.edges[edit.edge].1.points = edit.points;
    }
    Ok(shared)
}

#[cfg(test)]
mod tests;
