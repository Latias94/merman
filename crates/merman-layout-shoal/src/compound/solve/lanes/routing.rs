//! Route simple handoffs and same-lane returns against the occupied drawing.

use super::*;
use crate::geom::{Bounds, Bvh, BvhEntry};
use crate::{MIN_MARKER_LEAD, MarkerMetrics, Spacing};
use std::ops::ControlFlow;

mod align;
pub(super) use align::align;

pub(super) struct Lanes<'a> {
    pub(super) axes: Axes,
    pub(super) owners: &'a [usize],
    pub(super) rects: Vec<(Rect, Option<Point>)>,
    pub(super) feedback: &'a [[Option<f64>; 2]],
}

fn items(wire: &Wire, graph: &Graph) -> Option<[usize; 2]> {
    let [
        End::Item {
            slot: a,
            child: None,
            ..
        },
        End::Item {
            slot: b,
            child: None,
            ..
        },
    ] = wire.ends
    else {
        return None;
    };
    (a != b
        && !graph.nodes[a].group_proxy
        && !graph.nodes[b].group_proxy
        && wire.spec.start_label.is_none()
        && wire.spec.end_label.is_none())
    .then_some([a, b])
}

pub(super) fn runway(distance: f64, spacing: &Spacing) -> LayoutResult<f64> {
    finite(distance.max(MIN_MARKER_LEAD) + spacing.edge * 0.5)
}

fn fits_runway(contact: Contact, next: Point, length: f64) -> bool {
    let limit = towards(contact.at, contact.side, length);
    cross(contact.side, next) == cross(contact.side, contact.at)
        && match contact.side {
            Side::Top => next.y < limit.y,
            Side::Right => next.x > limit.x,
            Side::Bottom => next.y > limit.y,
            Side::Left => next.x < limit.x,
        }
}

pub(super) fn reserve_feedback(
    drawing: &LayoutOutput,
    graph: &Graph,
    wires: &[Wire],
    owners: &[usize],
    shells: &mut [Shell],
    options: &Options,
    work: &mut Work<'_>,
) -> LayoutResult<(Vec<[Option<f64>; 2]>, bool)> {
    let axes = Axes::new(graph.direction);
    work.charge(wires.len() + shells.len())?;
    let mut feedback = filled(wires.len(), [None; 2])?;
    let pitch = options
        .spacing
        .port_pitch
        .max(options.spacing.edge)
        .max(1.0);
    let mut next_rail = filled(shells.len(), [pitch * 0.5; 2])?;
    let mut widened = false;
    let mut routes = drawing.edges.iter();
    for (id, wire) in wires.iter().enumerate() {
        work.visit(1)?;
        if !wire.flat() || !routes.next().expect("flat lane route").reversed_for_ranking {
            continue;
        }
        let Some([a, b]) = items(wire, graph) else {
            continue;
        };
        let lane = owners[a];
        if lane != owners[b] {
            continue;
        }
        let [start, end] = wire.spec.marker_runways();
        let stem = runway(start.max(end), &options.spacing)?;
        for (i, side) in axes.cross.into_iter().enumerate() {
            if wire
                .ends
                .iter()
                .any(|end| named(end.port()).is_some_and(|hint| hint != side))
            {
                continue;
            }
            let at = &mut next_rail[lane][i];
            feedback[id][i] = Some(*at);
            let required = finite(*at + stem + pitch * 0.5)?;
            widened |= shells[lane].padding[side as usize] < required;
            shells[lane].reserve(side, required);
            *at = finite(*at + pitch)?;
        }
    }
    Ok((feedback, widened))
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Owner {
    Body(usize),
    Header,
    Wire(usize),
}

struct Occupied {
    owner: Owner,
    bounds: Bounds,
}

fn add(occupied: &mut Vec<Occupied>, owner: Owner, bounds: Bounds) -> LayoutResult<()> {
    if !bounds.finite() {
        return Err(WorkError::ArithmeticOverflow.into());
    }
    push(occupied, Occupied { owner, bounds })
}

fn clear(
    occupied: &Bvh<Occupied>,
    bounds: Bounds,
    ignore: impl Fn(&Occupied) -> bool,
    work: &mut Work<'_>,
) -> LayoutResult<bool> {
    let visit = occupied.visit(|entry| -> LayoutResult<_> {
        work.visit(1)?;
        Ok(match entry {
            BvhEntry::Node(node) => ControlFlow::Continue(bounds.overlaps(node)),
            BvhEntry::Item(item) if bounds.overlaps(item.bounds) && !ignore(item) => {
                ControlFlow::Break(())
            }
            BvhEntry::Item(_) => ControlFlow::Continue(true),
        })
    })?;
    Ok(visit.is_continue())
}

fn marker(
    contact: Contact,
    metrics: MarkerMetrics,
    item: Option<(&NodeSpec, Rect)>,
) -> Option<Bounds> {
    if metrics.width <= 0.0 || metrics.length <= 0.0 {
        return None;
    }
    let at = item.map_or(contact.at, |(node, rect)| {
        let side = contact.side;
        let offset = cross(side, contact.at) - cross(side, rect.center());
        let inset = node.contour.inset_at(node.size, side, offset);
        towards(contact.at, routes::opposite(side), inset)
    });
    Bounds::marker_footprint(at, contact.side, metrics)
}

fn occupied(
    ctx: &Pass<'_>,
    graph: &Graph,
    nodes: &[Rect],
    lanes: &Lanes<'_>,
    frames: &[Option<Frame>],
    wires: &[Wire],
    work: &mut Work<'_>,
) -> LayoutResult<Bvh<Occupied>> {
    let mut occupied = reserved(nodes.len())?;
    let root = &ctx.hierarchy.levels[ctx.graph.groups.len()];
    for (slot, (&rect, item)) in nodes.iter().zip(&root.items).enumerate() {
        work.visit(1)?;
        let mut bounds = Bounds::of(rect);
        if let Endpoint::Group(id) = item {
            let child = frames[id.0 as usize].as_ref().expect("solved lane group");
            if let (Some(title), Some(size)) = (&child.title, ctx.graph.groups[id.0 as usize].title)
            {
                let origin = Point {
                    x: rect.x,
                    y: rect.y,
                };
                add(
                    &mut occupied,
                    Owner::Header,
                    Bounds::centered(translated(title.center, origin), size),
                )?;
            }
            // A handoff or feedback candidate only terminates at ordinary nodes.
            if bounds.has_area() {
                let gap = ctx.options.spacing.edge;
                bounds = Bounds {
                    x: [bounds.x[0] - gap, bounds.x[1] + gap],
                    y: [bounds.y[0] - gap, bounds.y[1] + gap],
                };
            }
        }
        if bounds.has_area() {
            add(&mut occupied, Owner::Body(slot), bounds)?;
        }
    }
    for (&id, &(_, title)) in ctx.graph.lane_order.iter().zip(&lanes.rects) {
        work.visit(1)?;
        if let (Some(at), Some(size)) = (title, ctx.graph.groups[id.0 as usize].title) {
            add(&mut occupied, Owner::Header, Bounds::centered(at, size))?;
        }
    }
    for (id, wire) in wires.iter().enumerate() {
        work.visit(1)?;
        let route = wire.route.as_ref().expect("resolved lane route");
        let owner = Owner::Wire(id);
        for pair in route.points.windows(2) {
            work.visit(1)?;
            let [a, b] = [point(pair[0]), point(pair[1])];
            if a != b {
                add(&mut occupied, owner, Bounds::between(a, b))?;
            }
        }
        for (at, size) in [
            (route.label_center, wire.spec.label),
            (route.start_label_center, wire.spec.start_label),
            (route.end_label_center, wire.spec.end_label),
        ] {
            if let (Some(at), Some(size)) = (at, size) {
                add(&mut occupied, owner, Bounds::centered(at, size))?;
            }
        }
        for (end, metrics) in [wire.spec.start_marker, wire.spec.end_marker]
            .into_iter()
            .enumerate()
        {
            let item = wire.ends[end].node().map(|id| {
                let slot = id.0 as usize;
                (&graph.nodes[slot], nodes[slot])
            });
            if let Some(bounds) = marker(travel(wire, end), metrics, item) {
                add(&mut occupied, owner, bounds)?;
            }
        }
    }
    Bvh::build_with_work(occupied, |item| item.bounds, work)
}

struct Proposal {
    id: usize,
    contacts: [Contact; 2],
    points: Vec<RoutePoint>,
    label: Option<Point>,
    ink: [Option<Bounds>; 3],
}

impl Proposal {
    fn bounds(&self) -> impl Iterator<Item = Bounds> + '_ {
        self.points
            .windows(2)
            .map(|leg| Bounds::between(point(leg[0]), point(leg[1])))
            .chain(self.ink.iter().copied().flatten())
    }

    fn apply(self, wire: &mut Wire) {
        wire.contacts = self.contacts.map(Some);
        let route = wire.route.as_mut().expect("resolved lane route");
        route.ports = (self.contacts[0].side, self.contacts[1].side);
        route.points = self.points;
        route.label_center = self.label;
        route.corridor_clearance = 0.0;
    }
}

struct Scene<'a> {
    graph: &'a Graph,
    nodes: &'a [Rect],
    spacing: &'a Spacing,
    occupied: Bvh<Occupied>,
}

impl Scene<'_> {
    fn contact(&self, wire: &Wire, end: usize, slot: usize, side: Side) -> Option<Contact> {
        let old = travel(wire, end);
        if side == old.side {
            return Some(old);
        }
        if named(wire.ends[end].port()).is_some() {
            return None;
        }
        let rect = self.nodes[slot];
        let at = match side.axis() {
            Axis::Horizontal => self.graph.nodes[slot]
                .anchor_y
                .map_or(rect.center().y, |y| rect.y + y),
            Axis::Vertical => rect.center().x,
        };
        Some(Contact {
            at: face(rect, side, at),
            side,
        })
    }

    fn label(
        &self,
        id: usize,
        size: Size,
        points: &[Point],
        guards: [f64; 2],
        work: &mut Work<'_>,
    ) -> LayoutResult<Option<Point>> {
        let mut best = None;
        let mut longest = 0.0;
        for (i, leg) in points.windows(2).enumerate() {
            let [a, b] = [leg[0], leg[1]];
            let horizontal = a.y == b.y;
            let distance = if horizontal {
                (b.x - a.x).abs()
            } else {
                (b.y - a.y).abs()
            };
            let before = if i == 0 { guards[0] } else { 0.0 } + self.spacing.edge;
            let after = if i + 2 == points.len() {
                guards[1]
            } else {
                0.0
            } + self.spacing.edge;
            let room = distance - before - after;
            if room <= longest || room <= if horizontal { size.width } else { size.height } {
                continue;
            }
            let mut at = Point {
                x: a.x.midpoint(b.x),
                y: a.y.midpoint(b.y),
            };
            if horizontal {
                at.x += (b.x - a.x).signum() * (before - after) * 0.5;
            } else {
                at.y += (b.y - a.y).signum() * (before - after) * 0.5;
            }
            let bounds = Bounds::centered(at, size);
            if bounds.finite()
                && clear(
                    &self.occupied,
                    bounds,
                    |item| item.owner == Owner::Wire(id),
                    work,
                )?
            {
                best = Some(at);
                longest = room;
            }
        }
        Ok(best)
    }

    fn propose(
        &self,
        id: usize,
        wire: &Wire,
        (slots, rects): ([usize; 2], [Rect; 2]),
        contacts: [Contact; 2],
        corners: &[Point],
        work: &mut Work<'_>,
    ) -> LayoutResult<Option<Proposal>> {
        let [source, target] = contacts;
        let mut raw = [source.at, target.at, target.at, target.at];
        raw[1..1 + corners.len()].copy_from_slice(corners);
        let points = &raw[..corners.len() + 2];
        let metrics = [wire.spec.start_marker, wire.spec.end_marker];
        let runways = wire.spec.marker_runways();
        let mut guards = [0.0; 2];
        let mut ink = [None; 3];
        for (end, next) in [points[1], points[points.len() - 2]]
            .into_iter()
            .enumerate()
        {
            let contact = contacts[end];
            let required = runway(runways[end], self.spacing)?;
            if !fits_runway(contact, next, required) {
                return Ok(None);
            }
            let slot = slots[end];
            if let Some(footprint) = marker(
                contact,
                metrics[end],
                Some((&self.graph.nodes[slot], rects[end])),
            ) {
                if !footprint.finite()
                    || !clear(
                        &self.occupied,
                        footprint,
                        |item| item.owner == Owner::Wire(id) || item.owner == Owner::Body(slot),
                        work,
                    )?
                {
                    return Ok(None);
                }
                ink[end] = Some(footprint);
            }
            guards[end] = required;
        }
        for leg in points.windows(2) {
            let a = leg[0];
            let b = leg[1];
            let bounds = Bounds::between(a, b);
            if !bounds.finite()
                || a == b
                || bounds.has_area()
                || !clear(
                    &self.occupied,
                    bounds,
                    |item| {
                        item.owner == Owner::Wire(id)
                            || matches!(item.owner, Owner::Body(slot) if slots.contains(&slot)
                                && !bounds.overlaps_strictly(item.bounds))
                    },
                    work,
                )?
            {
                return Ok(None);
            }
        }
        let mut label = None;
        if let Some(size) = wire.spec.label {
            let Some(at) = self.label(id, size, points, guards, work)? else {
                return Ok(None);
            };
            let bounds = Bounds::centered(at, size);
            if ink.iter().flatten().any(|&marker| bounds.overlaps(marker)) {
                return Ok(None);
            }
            ink[2] = Some(bounds);
            label = Some(at);
        }
        work.charge(points.len())?;
        let mut knots = reserved(points.len())?;
        knots.push(RoutePoint::new(
            source.at.x,
            source.at.y,
            source.side.axis(),
        ));
        for leg in points.windows(2) {
            let [a, b] = [leg[0], leg[1]];
            let axis = if a.x == b.x {
                Axis::Vertical
            } else {
                Axis::Horizontal
            };
            knots.push(RoutePoint::new(b.x, b.y, axis));
        }
        Ok(Some(Proposal {
            id,
            contacts,
            points: knots,
            label,
            ink,
        }))
    }

    fn handoff(
        &self,
        id: usize,
        wire: &Wire,
        slots: [usize; 2],
        lanes: &Lanes<'_>,
        work: &mut Work<'_>,
    ) -> LayoutResult<Option<Proposal>> {
        let [a, b] = slots.map(|slot| self.nodes[slot].center());
        let [from, to] = slots.map(|slot| lanes.owners[slot]);
        let across = lanes.axes.cross[usize::from(from < to)];
        let flow = if lanes.axes.along(a) < lanes.axes.along(b) {
            match lanes.axes.start.axis() {
                Axis::Vertical => Side::Bottom,
                Axis::Horizontal => Side::Right,
            }
        } else {
            match lanes.axes.start.axis() {
                Axis::Vertical => Side::Top,
                Axis::Horizontal => Side::Left,
            }
        };
        for (leave, enter) in [
            (across, routes::opposite(flow)),
            (flow, routes::opposite(across)),
        ] {
            let [Some(source), Some(target)] = [
                self.contact(wire, 0, slots[0], leave),
                self.contact(wire, 1, slots[1], enter),
            ] else {
                continue;
            };
            let turn = if leave.axis() == Axis::Horizontal {
                Point {
                    x: target.at.x,
                    y: source.at.y,
                }
            } else {
                Point {
                    x: source.at.x,
                    y: target.at.y,
                }
            };
            if let Some(candidate) = self.propose(
                id,
                wire,
                (slots, slots.map(|slot| self.nodes[slot])),
                [source, target],
                &[turn],
                work,
            )? {
                return Ok(Some(candidate));
            }
        }
        Ok(None)
    }

    fn feedback(
        &self,
        id: usize,
        wire: &Wire,
        slots: [usize; 2],
        lanes: &Lanes<'_>,
        work: &mut Work<'_>,
    ) -> LayoutResult<Option<Proposal>> {
        let axes = lanes.axes;
        let shell = lanes.rects[lanes.owners[slots[0]]].0;
        let border = |side| cross(axes.start, face(shell, side, 0.0));
        let center = cross(axes.start, self.nodes[slots[0]].center())
            .midpoint(cross(axes.start, self.nodes[slots[1]].center()));
        let right = center > cross(axes.start, shell.center());
        for i in [usize::from(right), usize::from(!right)] {
            let Some(inset) = lanes.feedback[id][i] else {
                continue;
            };
            let side = axes.cross[i];
            let [Some(source), Some(target)] = [
                self.contact(wire, 0, slots[0], side),
                self.contact(wire, 1, slots[1], side),
            ] else {
                continue;
            };
            let track = border(side) + if i == 0 { inset } else { -inset };
            let corners =
                [source, target].map(|contact| axes.physical(track, axes.along(contact.at)));
            if let Some(candidate) = self.propose(
                id,
                wire,
                (slots, slots.map(|slot| self.nodes[slot])),
                [source, target],
                &corners,
                work,
            )? {
                return Ok(Some(candidate));
            }
        }
        Ok(None)
    }
}

pub(super) fn improve(
    ctx: &Pass<'_>,
    graph: &Graph,
    nodes: &[Rect],
    lanes: &Lanes<'_>,
    frames: &[Option<Frame>],
    wires: &mut [Wire],
    work: &mut Work<'_>,
) -> LayoutResult<()> {
    let mut candidates = Vec::new();
    for (id, wire) in wires.iter().enumerate() {
        work.visit(1)?;
        let Some(slots) = items(wire, graph) else {
            continue;
        };
        let [a, b] = slots.map(|slot| lanes.owners[slot]);
        if a != b || lanes.feedback[id].iter().any(Option::is_some) {
            push(&mut candidates, (id, slots, a == b))?;
        }
    }
    if candidates.is_empty() {
        return Ok(());
    }
    let scene = Scene {
        graph,
        nodes,
        spacing: &ctx.options.spacing,
        occupied: occupied(ctx, graph, nodes, lanes, frames, wires, work)?,
    };
    let mut proposals = reserved(candidates.len())?;
    let mut bounds = Vec::new();
    for (id, slots, feedback) in candidates {
        work.visit(1)?;
        let wire = &wires[id];
        let proposal = if feedback {
            scene.feedback(id, wire, slots, lanes, work)?
        } else {
            scene.handoff(id, wire, slots, lanes, work)?
        };
        if let Some(proposal) = proposal {
            for footprint in proposal.bounds() {
                work.visit(1)?;
                add(&mut bounds, Owner::Wire(id), footprint)?;
            }
            proposals.push(proposal);
        }
    }
    // Each accepted route clears the original scene and all previously accepted replacements.
    let index = Bvh::build_with_work(bounds, |item| item.bounds, work)?;
    work.charge(wires.len())?;
    let mut accepted = filled(wires.len(), false)?;
    for proposal in proposals {
        let mut safe = true;
        for footprint in proposal.bounds() {
            if !clear(
                &index,
                footprint,
                |item| matches!(item.owner, Owner::Wire(id) if !accepted[id]),
                work,
            )? {
                safe = false;
                break;
            }
        }
        if safe {
            accepted[proposal.id] = true;
            let id = proposal.id;
            proposal.apply(&mut wires[id]);
        }
    }
    Ok(())
}
