//! Align a lane's simple forward link when its terminal can move inside the routed scene.

use super::*;
use crate::KnotKind;

type Incidence = [Option<(usize, usize)>; 3];

struct Tail {
    edge: usize,
    end: usize,
    contact: Contact,
    ink: [Option<Bounds>; 3],
}

struct Alignment {
    slots: [usize; 2],
    moved: usize,
    rect: Rect,
    straight: Proposal,
    tail: Option<Tail>,
}

impl Alignment {
    fn edges(&self) -> impl Iterator<Item = usize> {
        std::iter::once(self.straight.id).chain(self.tail.as_ref().map(|tail| tail.edge))
    }

    fn ink(&self) -> impl Iterator<Item = Bounds> + '_ {
        self.straight.bounds().chain(
            self.tail
                .iter()
                .flat_map(|tail| tail.ink.into_iter().flatten()),
        )
    }

    fn clears_plans(
        &self,
        index: &Bvh<Occupied>,
        accepted: &[bool],
        spacing: &Spacing,
        work: &mut Work<'_>,
    ) -> LayoutResult<bool> {
        let inactive = |owner| match owner {
            Owner::Body(plan) | Owner::Wire(plan) => !accepted[plan],
            Owner::Header => true,
        };
        let gap = BodyClearance::new(Bounds::of(self.rect), spacing);
        if !clear(
            index,
            gap.search(),
            |item| {
                let guard = match item.owner {
                    Owner::Body(_) => gap.nodes,
                    _ => gap.ink,
                };
                inactive(item.owner) || !guard.overlaps_strictly(item.bounds)
            },
            work,
        )? {
            return Ok(false);
        }
        for ink in self.ink() {
            work.visit(1)?;
            if !clear_ink(index, ink, spacing.edge, &[], inactive, work)? {
                return Ok(false);
            }
        }
        Ok(true)
    }
}

struct BodyClearance {
    nodes: Bounds,
    ink: Bounds,
}

impl BodyClearance {
    fn new(body: Bounds, spacing: &Spacing) -> Self {
        Self {
            nodes: inflate(body, spacing.node),
            ink: inflate(body, spacing.edge),
        }
    }

    fn search(&self) -> Bounds {
        self.nodes.union(self.ink)
    }
}

fn doubles_back(a: Point, join: Point, b: Point) -> bool {
    let same_side = |a, join, b| (a < join && b < join) || (a > join && b > join);
    (a.x == join.x && b.x == join.x && same_side(a.y, join.y, b.y))
        || (a.y == join.y && b.y == join.y && same_side(a.x, join.x, b.x))
}

impl Scene<'_> {
    fn move_tail(
        &self,
        (edge, end): (usize, usize),
        wire: &Wire,
        (slot, rect, delta): (usize, Rect, Point),
        work: &mut Work<'_>,
    ) -> LayoutResult<Option<Tail>> {
        let route = wire.route.as_ref().expect("resolved companion lane route");
        if route.points.len() < 3
            || route.shared_bus
            || named(wire.ends[end].port()).is_some()
            || wire.spec.start_label.is_some()
            || wire.spec.end_label.is_some()
        {
            return Ok(None);
        }
        let old = travel(wire, end);
        let at = cross(old.side, translated(old.at, delta));
        let contact = Contact {
            at: face(rect, old.side, at),
            ..old
        };
        let index = if end == 0 { 0 } else { route.points.len() - 1 };
        let next = if end == 0 { 1 } else { index - 1 };
        let far = if end == 0 { 2 } else { index - 2 };
        let near = point(route.points[next]);
        if !fits_runway(old, near, 0.0) {
            return Ok(None);
        }
        let elbow = point(route.points[far]);
        let mut swept = None;
        let mut added = None;
        let mut new_near = near;
        if cross(old.side, old.at) != at {
            // Move the first arm of a simple elbow. A sweep or a second port remains fixed.
            if route.points[next].kind != KnotKind::Free {
                return Ok(None);
            }
            let (from, to) = match old.side.axis() {
                Axis::Horizontal if near.x == elbow.x => {
                    new_near.y = at;
                    (near.y, elbow.y)
                }
                Axis::Vertical if near.y == elbow.y => {
                    new_near.x = at;
                    (near.x, elbow.x)
                }
                _ => return Ok(None),
            };
            if !((from < to && at < to) || (from > to && at > to)) {
                return Ok(None);
            }
            swept = Some(Bounds::between(near, new_near));
            // Only the extension of the second arm needs new space in the scene.
            if (at < from && from < to) || (at > from && from > to) {
                added = swept;
            }
        }
        let runways = wire.spec.marker_runways();
        let peer = 1 - end;
        if !fits_runway(contact, new_near, runway(runways[end], self.spacing)?)
            || (route.points.len() == 3
                && !fits_runway(
                    travel(wire, peer),
                    new_near,
                    runway(runways[peer], self.spacing)?,
                ))
            || doubles_back(contact.at, new_near, elbow)
        {
            return Ok(None);
        }
        let first = Bounds::between(contact.at, new_near);
        let second = Bounds::between(new_near, elbow);
        let body = Bounds::of(rect);
        let body_guard = inflate(body, self.spacing.edge);
        let near_marker = marker(
            contact,
            [wire.spec.start_marker, wire.spec.end_marker][end],
            Some((&self.graph.nodes[slot], rect)),
        );
        let ink = [Some(first), added, near_marker];
        if second.overlaps_strictly(body)
            || near_marker.is_some_and(|footprint| footprint.overlaps(second))
            || terminal_marker(self.graph, self.nodes, wire, peer).is_some_and(|footprint| {
                body_guard.overlaps_strictly(footprint)
                    || ink.iter().flatten().any(|part| footprint.overlaps(*part))
            })
        {
            return Ok(None);
        }
        if let Some((at, size)) = route.label_center.zip(wire.spec.label) {
            let label = Bounds::centered(at, size);
            if label.overlaps(Bounds::between(old.at, near))
                || swept.is_some_and(|arm| label.overlaps(arm))
                || body_guard.overlaps_strictly(label)
                || ink.iter().flatten().any(|part| label.overlaps(*part))
            {
                return Ok(None);
            }
        }
        // Keep a shared corner, but reject retracing it or crossing a later leg. The bounds
        // are exact for orthogonal legs and enclose the ink of a retained sweep.
        let retained = if end == 0 {
            &route.points[far..]
        } else {
            &route.points[..=far]
        };
        for (i, pair) in retained.windows(2).enumerate() {
            work.visit(1)?;
            let [a, b] = [point(pair[0]), point(pair[1])];
            let leg = Bounds::between(a, b);
            let adjacent = if end == 0 {
                i == 0
            } else {
                i + 2 == retained.len()
            };
            let crosses_second = if adjacent {
                doubles_back(new_near, elbow, if end == 0 { b } else { a })
            } else {
                second.overlaps(leg)
            };
            if first.overlaps(leg)
                || crosses_second
                || near_marker.is_some_and(|footprint| footprint.overlaps(leg))
                || body_guard.overlaps_strictly(leg)
            {
                return Ok(None);
            }
        }
        let peer_slot = wire.ends[peer].node().map(|id| id.0 as usize);
        for &footprint in ink.iter().flatten() {
            if !clear_ink(
                &self.occupied,
                footprint,
                self.spacing.edge,
                peer_slot.as_slice(),
                |owner| owner == Owner::Wire(edge) || owner == Owner::Body(slot),
                work,
            )? {
                return Ok(None);
            }
        }
        Ok(Some(Tail {
            edge,
            end,
            contact,
            ink,
        }))
    }

    fn align_link(
        &self,
        edge: usize,
        slots: [usize; 2],
        incident: &[Incidence],
        wires: &[Wire],
        lanes: &Lanes<'_>,
        work: &mut Work<'_>,
    ) -> LayoutResult<Option<Alignment>> {
        let wire = &wires[edge];
        let contacts = [travel(wire, 0), travel(wire, 1)];
        let axes = lanes.axes;
        let cross_axis = |at| cross(axes.start, at);
        let positions = slots.map(|slot| cross_axis(self.nodes[slot].center()));
        let lane = lanes.content(lanes.owners[slots[0]]);
        for moved in [0, 1] {
            work.visit(1)?;
            let slot = slots[moved];
            let peer = 1 - moved;
            let node = &self.graph.nodes[slot];
            if incident[slot][2].is_some()
                || node.position_hint.is_some()
                || named(wire.ends[moved].port()).is_some()
            {
                continue;
            }
            let delta = axes.physical(positions[peer] - positions[moved], 0.0);
            let mut rects = slots.map(|slot| self.nodes[slot]);
            let rect = &mut rects[moved];
            rect.x += delta.x;
            rect.y += delta.y;
            let rect = *rect;
            let body = Bounds::of(rect);
            if !body.finite()
                || body.x[0] < lane.x[0]
                || body.x[1] > lane.x[1]
                || body.y[0] < lane.y[0]
                || body.y[1] > lane.y[1]
            {
                continue;
            }
            let side = contacts[moved].side;
            let dock = cross_axis(contacts[peer].at);
            let ([lo, hi], anchor) = if side.axis() == Axis::Horizontal {
                (
                    body.y,
                    node.anchor_y.map_or(rect.center().y, |y| rect.y + y),
                )
            } else {
                (body.x, rect.center().x)
            };
            let span = across(side, node.size);
            let corner = if node.port_span.is_none() {
                self.spacing.corner_clearance.min(span / 2.0)
            } else {
                0.0
            };
            let half = node.port_span.unwrap_or(span - 2.0 * corner).min(span) / 2.0;
            let other = incident[slot].iter().flatten().find(|&&(id, _)| id != edge);
            let gap = BodyClearance::new(body, self.spacing);
            if !((lo + corner).max(anchor - half)..=(hi - corner).min(anchor + half))
                .contains(&dock)
                || !clear(
                    &self.occupied,
                    gap.search(),
                    |item| match item.owner {
                        Owner::Body(other) => {
                            other == slot || !gap.nodes.overlaps_strictly(item.bounds)
                        }
                        Owner::Header => !gap.ink.overlaps_strictly(item.bounds),
                        Owner::Wire(id) => {
                            edge == id
                                || other.is_some_and(|&(companion, _)| id == companion)
                                || !gap.ink.overlaps_strictly(item.bounds)
                        }
                    },
                    work,
                )?
            {
                continue;
            }
            let mut adjusted = contacts;
            adjusted[moved].at = face(rect, side, dock);
            let Some(straight) = self.propose(edge, wire, (slots, rects), adjusted, &[], work)?
            else {
                continue;
            };
            let tail = if let Some(&(id, end)) = other {
                let Some(tail) =
                    self.move_tail((id, end), &wires[id], (slot, rect, delta), work)?
                else {
                    continue;
                };
                if straight
                    .bounds()
                    .any(|shaft| tail.ink.iter().flatten().any(|&part| shaft.overlaps(part)))
                {
                    continue;
                }
                Some(tail)
            } else {
                None
            };
            return Ok(Some(Alignment {
                slots,
                moved,
                rect,
                straight,
                tail,
            }));
        }
        Ok(None)
    }
}

pub(in crate::compound::solve::lanes) fn align(
    ctx: &Pass<'_>,
    graph: &Graph,
    nodes: &mut [Rect],
    lanes: &Lanes<'_>,
    frames: &[Option<Frame>],
    wires: &mut [Wire],
    work: &mut Work<'_>,
) -> LayoutResult<()> {
    let Some(plans) = work.compare(|work| {
        let axes = lanes.axes;
        let mut candidates = Vec::new();
        for (id, wire) in wires.iter().enumerate() {
            work.visit(1)?;
            if wire.spec.weight <= 1.0 {
                continue;
            }
            let Some(slots) = items(wire, graph) else {
                continue;
            };
            let route = wire.route.as_ref().expect("resolved lane route");
            let [a, b] = slots.map(|slot| cross(axes.start, nodes[slot].center()));
            if lanes.owners[slots[0]] == lanes.owners[slots[1]]
                && !route.shared_bus
                && !route.reversed_for_ranking
                && route.ports == (axes.finish, axes.start)
                && (a - b).abs() > 1e-6
            {
                push(&mut candidates, (id, slots))?;
            }
        }
        if candidates.is_empty() {
            return Ok(Vec::new());
        }
        work.charge(nodes.len())?;
        let mut incidence = filled::<Incidence>(nodes.len(), [None; 3])?;
        for (id, wire) in wires.iter().enumerate() {
            work.visit(1)?;
            for (end, node) in wire.ends.map(End::node).into_iter().enumerate() {
                if let Some(node) = node
                    && let Some(entry) = incidence[node.0 as usize]
                        .iter_mut()
                        .find(|entry| entry.is_none())
                {
                    *entry = Some((id, end));
                }
            }
        }
        let scene = Scene {
            graph,
            nodes,
            spacing: &ctx.options.spacing,
            occupied: occupied(ctx, graph, nodes, lanes, frames, wires, work)?,
        };
        let mut plans = reserved(candidates.len())?;
        let mut bounds = Vec::new();
        for (id, slots) in candidates {
            work.visit(1)?;
            if let Some(plan) = scene.align_link(id, slots, &incidence, wires, lanes, work)? {
                let owner = plans.len();
                work.visit(1)?;
                add(&mut bounds, Owner::Body(owner), Bounds::of(plan.rect))?;
                for footprint in plan.ink() {
                    work.visit(1)?;
                    add(&mut bounds, Owner::Wire(owner), footprint)?;
                }
                plans.push(plan);
            }
        }
        let index = Bvh::build_with_work(bounds, |item| item.bounds, work)?;
        work.charge(plans.len() + nodes.len() + wires.len())?;
        let mut accepted = filled(plans.len(), false)?;
        let mut claimed_nodes = filled(nodes.len(), false)?;
        let mut claimed_edges = filled(wires.len(), false)?;
        let mut result = Vec::new();
        for (id, plan) in plans.into_iter().enumerate() {
            work.visit(1)?;
            if plan.slots.iter().any(|&slot| claimed_nodes[slot])
                || plan.edges().any(|edge| claimed_edges[edge])
            {
                continue;
            }
            if plan.clears_plans(&index, &accepted, scene.spacing, work)? {
                accepted[id] = true;
                for slot in plan.slots {
                    claimed_nodes[slot] = true;
                }
                for edge in plan.edges() {
                    claimed_edges[edge] = true;
                }
                push(&mut result, plan)?;
            }
        }
        Ok(result)
    })?
    else {
        return Ok(());
    };
    for plan in plans {
        work.visit(1)?;
        nodes[plan.slots[plan.moved]] = plan.rect;
        let id = plan.straight.id;
        plan.straight.apply(&mut wires[id]);
        if let Some(Tail {
            edge, end, contact, ..
        }) = plan.tail
        {
            let wire = &mut wires[edge];
            wire.contacts[end] = Some(contact);
            let route = wire.route.as_mut().expect("resolved lane route");
            routes::snap(route, end, contact);
            route.corridor_clearance = 0.0;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::NoopWorkControl;

    fn rect(x: f64, y: f64, width: f64, height: f64) -> Rect {
        Rect {
            x,
            y,
            width,
            height,
        }
    }

    fn graph(nodes: &[Rect]) -> Graph {
        Graph {
            direction: Direction::Right,
            nodes: nodes
                .iter()
                .enumerate()
                .map(|(i, node)| {
                    NodeSpec::new(
                        Size {
                            width: node.width,
                            height: node.height,
                        },
                        i as u32,
                    )
                })
                .collect(),
            ..Graph::default()
        }
    }

    fn wire([source, target]: [Contact; 2]) -> Wire {
        Wire {
            ends: [0, 1].map(|slot| End::Item {
                slot,
                hint: PortSideHint::Auto,
                child: None,
            }),
            spec: EdgeSpec::new(NodeId(0).into(), NodeId(1).into()),
            route: None,
            contacts: [Some(source), Some(target)],
        }
    }

    fn companion(knots: &[(f64, f64)], [start, finish]: [Side; 2]) -> Wire {
        let at = |(x, y)| Point { x, y };
        let mut wire = wire([
            Contact {
                side: start,
                at: at(knots[0]),
            },
            Contact {
                side: finish,
                at: at(*knots.last().unwrap()),
            },
        ]);
        let mut points = vec![RoutePoint::new(knots[0].0, knots[0].1, start.axis())];
        points.extend(knots.windows(2).map(|pair| {
            let (x, y) = pair[1];
            let axis = if pair[0].0 == x {
                Axis::Vertical
            } else {
                Axis::Horizontal
            };
            RoutePoint::new(x, y, axis)
        }));
        wire.route = Some(EdgeGeometry {
            points,
            ports: (start, finish),
            label_center: None,
            start_label_center: None,
            end_label_center: None,
            corridor_clearance: 0.0,
            shared_bus: false,
            reversed_for_ranking: false,
        });
        wire
    }

    fn try_tail(wire: &Wire, end: usize, delta: Point) -> Option<Tail> {
        let nodes = wire.contacts.map(|contact| {
            let contact = contact.unwrap();
            centered(
                towards(contact.at, routes::opposite(contact.side), 10.0),
                Size {
                    width: 20.0,
                    height: 20.0,
                },
            )
        });
        let graph = graph(&nodes);
        let scene = Scene {
            graph: &graph,
            nodes: &nodes,
            spacing: &Spacing::default(),
            occupied: Bvh::build(Vec::new(), |item: &Occupied| item.bounds).unwrap(),
        };
        let mut moved = nodes[end];
        moved.x += delta.x;
        moved.y += delta.y;
        scene
            .move_tail(
                (0, end),
                wire,
                (end, moved, delta),
                &mut Work::new(&mut NoopWorkControl),
            )
            .unwrap()
    }

    #[test]
    fn moving_an_elbow_preserves_the_runway_at_either_remote_terminal() {
        let mut knots = [(0.0, 0.0), (0.0, 60.0), (100.0, 60.0)];
        let mut sides = [Side::Bottom, Side::Left];
        for end in [0, 1] {
            let mut wire = companion(&knots, sides);
            let metrics = MarkerMetrics {
                length: 10.0,
                width: 8.0,
            };
            if end == 0 {
                wire.spec.end_marker = metrics;
            } else {
                wire.spec.start_marker = metrics;
            }
            assert!(try_tail(&wire, end, Point { x: 72.0, y: 0.0 }).is_some());
            assert!(try_tail(&wire, end, Point { x: 77.0, y: 0.0 }).is_none());
            knots.reverse();
            sides.reverse();
        }
    }

    #[test]
    fn a_translated_shaft_cannot_cross_its_retained_route_or_remote_marker() {
        let crossing = companion(
            &[
                (0.0, 0.0),
                (0.0, 60.0),
                (80.0, 60.0),
                (80.0, 30.0),
                (15.0, 30.0),
            ],
            [Side::Bottom, Side::Right],
        );
        assert!(try_tail(&crossing, 0, Point { x: -20.0, y: 0.0 }).is_some());
        assert!(try_tail(&crossing, 0, Point { x: 35.0, y: 0.0 }).is_none());

        // Extend the arm across the marker without crossing its centerline.
        let mut marker = companion(
            &[
                (73.0, 0.0),
                (73.0, 24.0),
                (80.0, 24.0),
                (80.0, 30.0),
                (50.0, 30.0),
            ],
            [Side::Bottom, Side::Right],
        );
        let shift = Point { x: -18.0, y: 0.0 };
        assert!(try_tail(&marker, 0, shift).is_some());
        marker.spec.end_marker = MarkerMetrics {
            length: 20.0,
            width: 16.0,
        };
        assert!(try_tail(&marker, 0, shift).is_none());
    }

    #[test]
    fn a_side_departure_cannot_drop_or_run_through_its_caption() {
        let mut wire = companion(
            &[(20.0, 0.0), (130.0, 0.0), (130.0, 100.0)],
            [Side::Right, Side::Top],
        );
        wire.spec.label = Some(Size {
            width: 12.0,
            height: 8.0,
        });
        let route = wire.route.as_mut().unwrap();
        route.label_center = Some(Point { x: 35.0, y: 0.0 });
        assert!(try_tail(&wire, 0, Point { x: 60.0, y: 0.0 }).is_none());
        wire.route.as_mut().unwrap().label_center = Some(Point { x: -20.0, y: 0.0 });
        assert!(try_tail(&wire, 0, Point { x: -60.0, y: 0.0 }).is_none());
        wire.route.as_mut().unwrap().label_center = Some(Point { x: 130.0, y: 45.0 });
        assert!(try_tail(&wire, 0, Point { x: 60.0, y: 0.0 }).is_some());
    }

    fn try_link(
        scene: &Scene<'_>,
        lane_box: Rect,
        top_padding: f64,
        dock: f64,
    ) -> Option<Alignment> {
        let lanes = Lanes {
            axes: Axes::new(scene.graph.direction),
            owners: &[0, 0],
            rects: vec![(lane_box, None)],
            feedback: &LaneReservations {
                rails: Vec::new(),
                padding: vec![[top_padding, 0.0, 0.0, 0.0]],
                depth_padding: [0.0; 2],
            },
        };
        let mut wire = wire([
            Contact {
                side: Side::Right,
                at: face(scene.nodes[0], Side::Right, dock),
            },
            Contact {
                side: Side::Left,
                at: face(scene.nodes[1], Side::Left, scene.nodes[1].center().y),
            },
        ]);
        wire.ends[0] = End::Item {
            slot: 0,
            hint: PortSideHint::FixedOffset {
                side: Side::Right,
                offset: dock - scene.nodes[0].center().y,
            },
            child: None,
        };
        scene
            .align_link(
                0,
                [0, 1],
                &[[None; 3]; 2],
                &[wire],
                &lanes,
                &mut Work::new(&mut NoopWorkControl),
            )
            .unwrap()
    }

    #[test]
    fn alignment_intersects_an_offset_anchor_span_with_the_physical_face() {
        let nodes = [rect(0.0, 60.0, 40.0, 80.0), rect(130.0, 110.0, 40.0, 20.0)];
        let mut graph = graph(&nodes);
        graph.nodes[1].anchor_y = Some(0.0);
        graph.nodes[1].port_span = Some(20.0);
        let scene = Scene {
            graph: &graph,
            nodes: &nodes,
            spacing: &Spacing::default(),
            occupied: Bvh::build(Vec::new(), |item: &Occupied| item.bounds).unwrap(),
        };
        let lane_box = rect(0.0, 0.0, 200.0, 160.0);
        assert!(try_link(&scene, lane_box, 0.0, 85.0).is_none());
        let aligned = try_link(&scene, lane_box, 0.0, 95.0).unwrap();
        assert_eq!(aligned.straight.contacts[1].at, Point { x: 130.0, y: 95.0 });
    }

    #[test]
    fn alignment_uses_corner_clearance_when_the_face_has_no_measured_span() {
        let nodes = [rect(0.0, 60.0, 40.0, 80.0), rect(130.0, 110.0, 40.0, 20.0)];
        let graph = graph(&nodes);
        let scene = Scene {
            graph: &graph,
            nodes: &nodes,
            spacing: &Spacing {
                corner_clearance: 6.0,
                ..Spacing::default()
            },
            occupied: Bvh::build(Vec::new(), |item: &Occupied| item.bounds).unwrap(),
        };
        let lane_box = rect(0.0, 0.0, 200.0, 160.0);
        assert!(try_link(&scene, lane_box, 0.0, 95.0).is_none());
        assert!(try_link(&scene, lane_box, 0.0, 96.0).is_some());
    }

    #[test]
    fn alignment_preserves_lane_padding_and_drawing_clearance() {
        let nodes = [
            rect(20.0, 24.0, 40.0, 20.0),
            rect(130.0, 48.0, 40.0, 60.0),
            rect(194.0, 6.0, 10.0, 6.0),
            rect(80.0, 40.0, 15.0, 10.0),
            rect(80.0, 48.0, 15.0, 10.0),
        ];
        let graph = graph(&nodes);
        let spacing = Spacing {
            node: 35.0,
            ..Spacing::default()
        };
        let blocker = |owner, bounds| Occupied { owner, bounds };
        let body = || blocker(Owner::Body(2), Bounds::of(nodes[2]));
        let stroke = || blocker(Owner::Wire(2), Bounds::of(rect(181.0, 24.0, 0.0, 12.0)));
        let trial = |top, items| {
            let scene = Scene {
                graph: &graph,
                nodes: &nodes,
                spacing: &spacing,
                occupied: Bvh::build(items, |item: &Occupied| item.bounds).unwrap(),
            };
            try_link(&scene, rect(0.0, 0.0, 220.0, 120.0), top, 34.0)
        };
        assert!(trial(8.0, vec![]).is_none());
        assert!(trial(4.0, vec![body()]).is_none());
        assert!(trial(4.0, vec![stroke()]).is_none());
        // This body meets the node gap and leaves only 6 px beside the new shaft.
        let near_shaft = Bounds::of(nodes[3]);
        assert!(trial(4.0, vec![blocker(Owner::Body(3), near_shaft)]).is_none());
        assert!(trial(4.0, vec![blocker(Owner::Wire(3), near_shaft)]).is_some());
        assert!(trial(4.0, vec![blocker(Owner::Body(4), Bounds::of(nodes[4]))]).is_some());
        let plan = trial(4.0, vec![]).unwrap();
        assert_eq!(plan.rect.y, 4.0);

        let clear_plan = |item, accepted| {
            let index = Bvh::build(vec![item], |item: &Occupied| item.bounds).unwrap();
            plan.clears_plans(
                &index,
                accepted,
                &spacing,
                &mut Work::new(&mut NoopWorkControl),
            )
            .unwrap()
        };
        assert!(!clear_plan(blocker(Owner::Body(0), body().bounds), &[true]));
        assert!(!clear_plan(
            blocker(Owner::Wire(0), stroke().bounds),
            &[true]
        ));
        assert!(!clear_plan(blocker(Owner::Body(0), near_shaft), &[true]));
        assert!(clear_plan(blocker(Owner::Body(0), near_shaft), &[false]));
    }
}
