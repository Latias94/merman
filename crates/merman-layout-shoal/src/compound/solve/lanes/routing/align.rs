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

    fn bounds(&self) -> impl Iterator<Item = Bounds> + '_ {
        std::iter::once(Bounds::of(self.rect))
            .chain(self.straight.bounds())
            .chain(
                self.tail
                    .iter()
                    .flat_map(|tail| tail.ink.into_iter().flatten()),
            )
    }
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
        let contact = Contact {
            at: translated(old.at, delta),
            ..old
        };
        let index = if end == 0 { 0 } else { route.points.len() - 1 };
        let next = if end == 0 { 1 } else { index - 1 };
        let far = if end == 0 { 2 } else { index - 2 };
        let near = point(route.points[next]);
        if !fits_runway(old, near, 0.0) {
            return Ok(None);
        }
        let mut ink = [None; 3];
        let mut new_near = near;
        if cross(old.side, old.at) != cross(contact.side, contact.at) {
            // Move the first arm of a simple elbow. A sweep or a second port remains fixed.
            if route.points[next].kind != KnotKind::Free {
                return Ok(None);
            }
            let elbow = point(route.points[far]);
            let advance = match old.side.axis() {
                Axis::Horizontal if near.x == elbow.x => (near.y, elbow.y, delta.y),
                Axis::Vertical if near.y == elbow.y => (near.x, elbow.x, delta.x),
                _ => return Ok(None),
            };
            let (from, to, shift) = advance;
            if (to - from) * (to - from - shift) <= 0.0 {
                return Ok(None);
            }
            new_near = translated(near, delta);
            let changed_arm = Bounds::between(near, new_near);
            let extends = (to - from - shift).abs() > (to - from).abs();
            if let Some((at, size)) = route.label_center.zip(wire.spec.label)
                && (Bounds::between(old.at, near).overlaps(Bounds::centered(at, size))
                    || (!extends && changed_arm.overlaps(Bounds::centered(at, size))))
            {
                return Ok(None);
            }
            // The rest of the second arm keeps its established clearance and far contact.
            ink[1] = extends.then_some(changed_arm);
        }
        let required = runway(wire.spec.marker_runways()[end], self.spacing)?;
        if !fits_runway(contact, new_near, required) {
            return Ok(None);
        }
        let metrics = [wire.spec.start_marker, wire.spec.end_marker][end];
        ink[0] = Some(Bounds::between(contact.at, new_near));
        ink[2] = marker(contact, metrics, Some((&self.graph.nodes[slot], rect)));
        for &footprint in ink.iter().flatten() {
            if !clear(
                &self.occupied,
                footprint,
                |item| item.owner == Owner::Wire(edge) || item.owner == Owner::Body(slot),
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
        let lane = Bounds::of(lanes.rects[lanes.owners[slots[0]]].0);
        for moved in [0, 1] {
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
            if body.x[0] < lane.x[0]
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
            let half = node.port_span.unwrap_or(span).min(span) / 2.0;
            if !(lo.max(anchor - half)..=hi.min(anchor + half)).contains(&dock)
                || !clear(
                    &self.occupied,
                    body,
                    |item| item.owner == Owner::Body(slot) || !body.overlaps_strictly(item.bounds),
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
            let other = incident[slot].iter().flatten().find(|&&(id, _)| id != edge);
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
                for footprint in plan.bounds() {
                    work.visit(1)?;
                    add(&mut bounds, Owner::Wire(plans.len()), footprint)?;
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
            let mut safe = true;
            for footprint in plan.bounds() {
                if !clear(
                    &index,
                    footprint,
                    |item| matches!(item.owner, Owner::Wire(other) if !accepted[other]),
                    work,
                )? {
                    safe = false;
                    break;
                }
            }
            if safe {
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

    #[test]
    fn alignment_intersects_an_offset_anchor_span_with_the_physical_face() {
        let rect = |x, y, width, height| Rect {
            x,
            y,
            width,
            height,
        };
        let nodes = [rect(0.0, 60.0, 40.0, 80.0), rect(130.0, 110.0, 40.0, 20.0)];
        let mut graph = Graph {
            direction: Direction::Right,
            ..Graph::default()
        };
        for node in nodes {
            graph.nodes.push(NodeSpec::new(
                Size {
                    width: node.width,
                    height: node.height,
                },
                0,
            ));
        }
        graph.nodes[1].anchor_y = Some(0.0);
        graph.nodes[1].port_span = Some(20.0);
        let mut control = NoopWorkControl;
        let mut work = Work::new(&mut control);
        let spacing = Spacing::default();
        let scene = Scene {
            graph: &graph,
            nodes: &nodes,
            spacing: &spacing,
            occupied: Bvh::build(Vec::new(), |item: &Occupied| item.bounds).unwrap(),
        };
        let lanes = Lanes {
            axes: Axes::new(graph.direction),
            owners: &[0, 0],
            rects: vec![(rect(0.0, 0.0, 200.0, 160.0), None)],
            feedback: &[],
        };
        let trial = |dock, work: &mut Work<'_>| {
            let wire = Wire {
                ends: [
                    End::Item {
                        slot: 0,
                        hint: PortSideHint::FixedOffset {
                            side: Side::Right,
                            offset: dock - 100.0,
                        },
                        child: None,
                    },
                    End::Item {
                        slot: 1,
                        hint: PortSideHint::Auto,
                        child: None,
                    },
                ],
                spec: EdgeSpec::new(NodeId(0).into(), NodeId(1).into()),
                route: None,
                contacts: [
                    Some(Contact {
                        side: Side::Right,
                        at: Point { x: 40.0, y: dock },
                    }),
                    Some(Contact {
                        side: Side::Left,
                        at: Point { x: 130.0, y: 110.0 },
                    }),
                ],
            };
            scene
                .align_link(0, [0, 1], &[[None; 3]; 2], &[wire], &lanes, work)
                .unwrap()
        };
        assert!(trial(85.0, &mut work).is_none());
        let aligned = trial(95.0, &mut work).unwrap();
        assert_eq!(aligned.straight.contacts[1].at, Point { x: 130.0, y: 95.0 });
    }
}
