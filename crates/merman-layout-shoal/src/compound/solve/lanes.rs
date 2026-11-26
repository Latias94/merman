//! Give root lane shells a common depth and route their authored endpoints after fitting.

mod compact;
mod routing;
#[cfg(test)]
mod tests;

use super::*;
use crate::compound::hierarchy::LaneMembership;
use crate::flat::{BoundaryRoutes, lanes::Column};
use crate::work::push;
use crate::{Direction, GroupSpec, LayoutOutput, Options};

#[derive(Clone, Copy)]
struct Axes {
    start: Side,
    finish: Side,
    cross: [Side; 2],
}

impl Axes {
    fn new(direction: Direction) -> Self {
        let (start, finish) = flow_sides(direction);
        let cross = match start.axis() {
            Axis::Vertical => [Side::Left, Side::Right],
            Axis::Horizontal => [Side::Top, Side::Bottom],
        };
        Self {
            start,
            finish,
            cross,
        }
    }

    fn physical(self, cross: f64, along: f64) -> Point {
        match self.start.axis() {
            Axis::Vertical => Point { x: cross, y: along },
            Axis::Horizontal => Point { x: along, y: cross },
        }
    }

    fn along(self, point: Point) -> f64 {
        cross(self.cross[0], point)
    }
}

struct Shell {
    title: Option<Size>,
    header: f64,
    body_pad: f64,
    padding: [f64; 4],
    min: Size,
}

impl Shell {
    fn new(spec: &GroupSpec, axes: Axes) -> Self {
        let header = spec
            .title
            .map_or(0.0, |title| depth(axes.start, title) + 2.0 * TITLE_CLEAR);
        let padding = [
            spec.padding.top,
            spec.padding.right,
            spec.padding.bottom,
            spec.padding.left,
        ];
        let body_pad = padding[axes.start as usize].max(TITLE_CLEAR);
        let mut shell = Self {
            title: spec.title,
            header,
            body_pad,
            padding,
            min: spec.min_size,
        };
        if header > 0.0 {
            // Reserve the header band PLUS the lane's inner body padding so the first
            // rank of nodes keeps clear breathing room below the title divider.
            shell.reserve(axes.start, header + body_pad);
        }
        shell
    }

    fn reserve(&mut self, side: Side, padding: f64) {
        let current = &mut self.padding[side as usize];
        *current = current.max(padding);
    }

    fn keep_offset(&mut self, side: Side, offset: f64) -> LayoutResult<()> {
        let required = finite(2.0 * offset.abs())?;
        let length = match side.axis() {
            Axis::Vertical => &mut self.min.width,
            Axis::Horizontal => &mut self.min.height,
        };
        *length = length.max(required);
        Ok(())
    }

    fn column(&self, axes: Axes) -> Column {
        let padding = axes.cross.map(|side| self.padding[side as usize]);
        let title_width = self.title.map_or(0.0, |title| {
            across(axes.start, title) + 2.0 * padding[0].max(padding[1])
        });
        Column {
            min_width: across(axes.start, self.min).max(title_width).max(1.0),
            padding,
        }
    }
}

fn choose_sides(
    wires: &mut [Wire],
    shells: &mut [Shell],
    owners: &[usize],
    axes: Axes,
    options: &Options,
    work: &mut Work<'_>,
) -> LayoutResult<()> {
    let gap = options.spacing.edge.max(1.0);
    for wire in wires {
        work.visit(1)?;
        let markers = [wire.spec.start_marker, wire.spec.end_marker];
        let ends = wire.ends;
        for end in 0..2 {
            let End::Lane { index, hint, .. } = ends[end] else {
                continue;
            };
            let (side, inward) = match ends[1 - end] {
                End::Item { slot, .. } if owners[slot] == index => {
                    (if end == 0 { axes.start } else { axes.finish }, true)
                }
                End::Item { slot, .. } => (axes.cross[usize::from(owners[slot] > index)], false),
                End::Lane { .. } => (axes.finish, false),
                End::Border { .. } => unreachable!("the root has no compound boundary ports"),
            };
            let side = named(hint).unwrap_or(side);
            if let PortSideHint::FixedOffset { offset, .. } = hint {
                shells[index].keep_offset(side, offset)?;
            }
            let room = finite(markers[end].length + gap)?;
            let peer_room = finite(markers[1 - end].length + gap)?;
            if inward {
                if side == axes.start
                    && let Some(title) = shells[index].title
                {
                    let after = peer_room.max(room).max(TITLE_CLEAR);
                    let needed_header = finite(depth(side, title) + 2.0 * after)?;
                    shells[index].header = shells[index].header.max(needed_header);
                    let total = finite(needed_header + shells[index].body_pad)?;
                    shells[index].reserve(side, total);
                } else {
                    shells[index].reserve(side, finite(room + peer_room)?);
                }
            } else if side.axis() != axes.start.axis() {
                // A normal leaving a shared wall travels through the adjacent lane's padding.
                let adjacent = if side == axes.cross[0] {
                    index.checked_sub(1)
                } else {
                    Some(index + 1)
                };
                if let Some(shell) = adjacent.and_then(|index| shells.get_mut(index)) {
                    shell.reserve(routes::opposite(side), finite(room + peer_room)?);
                }
            }
            wire.ends[end] = End::Lane {
                index,
                hint,
                side,
                inward,
            };
        }
    }
    Ok(())
}

fn shell_rects(
    shells: &[Shell],
    columns: &[[f64; 2]],
    content: Size,
    axes: Axes,
    work: &mut Work<'_>,
) -> LayoutResult<Vec<(Rect, Option<Point>)>> {
    let (mut before, mut after, mut min_depth, mut shared_header) =
        (0.0_f64, 0.0_f64, 1.0_f64, 0.0_f64);
    for shell in shells {
        work.visit(1)?;
        before = before.max(shell.padding[axes.start as usize]);
        after = after.max(shell.padding[axes.finish as usize]);
        min_depth = min_depth.max(depth(axes.start, shell.min));
        shared_header = shared_header.max(shell.header);
    }
    let content = depth(axes.start, content);
    let length = finite(content + before + after)?.max(min_depth);
    let positive = matches!(axes.start, Side::Top | Side::Left);
    let along = if positive {
        -before
    } else {
        finite(content + before - length)?
    };
    let end = finite(along + length)?;
    let mut rects = reserved(shells.len())?;
    for (shell, &[low, high]) in shells.iter().zip(columns) {
        work.visit(1)?;
        let from = axes.physical(low, along);
        let size = axes.physical(high - low, length);
        let rect = Rect {
            x: from.x,
            y: from.y,
            width: size.x,
            height: size.y,
        };
        let title = shell.title.map(|_| {
            // Center every titled lane inside the shared header band so adjacent lanes
            // keep a flush divider line while leaving body padding between header and nodes.
            let header = if shared_header > 0.0 {
                shared_header
            } else {
                shell.padding[axes.start as usize]
            };
            let inset = header / 2.0;
            axes.physical(
                low.midpoint(high),
                if positive { along + inset } else { end - inset },
            )
        });
        rects.push((rect, title));
    }
    Ok(rects)
}

fn shell_contacts(
    wires: &mut [Wire],
    shells: &[(Rect, Option<Point>)],
    options: &Options,
    work: &mut Work<'_>,
) -> LayoutResult<()> {
    let mut faces = filled::<[Vec<ports::Slot>; 4]>(shells.len(), Default::default())?;
    for (index, wire) in wires.iter().enumerate() {
        work.visit(1)?;
        for end in 0..2 {
            let End::Lane {
                index: lane,
                hint,
                side,
                ..
            } = wire.ends[end]
            else {
                continue;
            };
            let rect = shells[lane].0;
            let center = cross(side, rect.center());
            let (at, fixed) = if let PortSideHint::FixedOffset { offset, .. } = hint {
                crate::validate::port(
                    Size {
                        width: rect.width,
                        height: rect.height,
                    },
                    hint,
                )?;
                (finite(center + offset)?, true)
            } else {
                let peer = wire.contacts[1 - end].map_or_else(
                    || match wire.ends[1 - end] {
                        End::Lane { index, .. } => shells[index].0.center(),
                        _ => unreachable!("items already have contacts"),
                    },
                    |contact| contact.at,
                );
                (cross(side, peer), false)
            };
            push(
                &mut faces[lane][side as usize],
                ports::Slot {
                    wire: index,
                    end,
                    at,
                    fixed,
                },
            )?;
        }
    }
    for (faces, &(rect, _)) in faces.iter_mut().zip(shells) {
        for (slots, side) in faces.iter_mut().zip(SIDES) {
            work.visit(slots.len() + 1)?;
            if slots.is_empty() {
                continue;
            }
            let mut inset = options.spacing.corner_clearance;
            for slot in slots.iter() {
                let spec = &wires[slot.wire].spec;
                inset = inset.max([spec.start_marker, spec.end_marker][slot.end].width / 2.0);
            }
            let center = cross(side, rect.center());
            let half = across(
                side,
                Size {
                    width: rect.width,
                    height: rect.height,
                },
            ) / 2.0;
            let span = (half - inset).max(0.0);
            ports::spread(
                slots,
                center - span,
                center + span,
                options.spacing.port_pitch,
                work,
            )?;
            for slot in slots {
                wires[slot.wire].contacts[slot.end] = Some(Contact {
                    at: face(rect, side, slot.at),
                    side,
                });
            }
        }
    }
    Ok(())
}

fn travel(wire: &Wire, end: usize) -> Contact {
    let contact = wire.contacts[end].expect("resolved root contact");
    Contact {
        side: normal(wire.ends[end], contact.side),
        ..contact
    }
}

fn normal(end: End, side: Side) -> Side {
    if matches!(end, End::Lane { inward: true, .. }) {
        routes::opposite(side)
    } else {
        side
    }
}

fn movable_span(
    end: End,
    side: Side,
    marker: crate::MarkerMetrics,
    nodes: &[Rect],
    shells: &[(Rect, Option<Point>)],
    graph: &Graph,
    options: &Options,
) -> Option<(Rect, f64)> {
    if matches!(end.port(), PortSideHint::FixedOffset { .. }) {
        return None;
    }
    match end {
        End::Lane { index, .. } => {
            let rect = shells[index].0;
            let length = across(
                side,
                Size {
                    width: rect.width,
                    height: rect.height,
                },
            );
            let inset = options.spacing.corner_clearance.max(marker.width / 2.0);
            Some((rect, (length - 2.0 * inset).max(0.0)))
        }
        End::Item {
            slot, child: None, ..
        } if side.axis() != Axis::Horizontal || graph.nodes[slot].anchor_y.is_none() => Some((
            nodes[slot],
            ports::item_span(&graph.nodes[slot], side, graph, options),
        )),
        _ => None,
    }
}

fn candidate_faces(end: End, preferred: Side) -> ([Side; 4], usize) {
    let lateral = match preferred.axis() {
        Axis::Vertical => [Side::Left, Side::Right],
        Axis::Horizontal => [Side::Top, Side::Bottom],
    };
    let allowed = if matches!(
        end,
        End::Item {
            child: None,
            hint: PortSideHint::Auto,
            ..
        }
    ) {
        4
    } else {
        1
    };
    (
        [
            preferred,
            lateral[0],
            lateral[1],
            routes::opposite(preferred),
        ],
        allowed,
    )
}

fn reserve_shell_leads(
    wires: &mut [Wire],
    nodes: &[Rect],
    shells: &[(Rect, Option<Point>)],
    graph: &Graph,
    options: &Options,
    router: &mut BoundaryRoutes,
    work: &mut Work<'_>,
) -> LayoutResult<()> {
    let mut blocked = Vec::new();
    // Exact offsets and existing child stitches reserve their space before any automatic ports.
    for fixed in [true, false] {
        for (id, wire) in wires.iter_mut().enumerate() {
            work.visit(1)?;
            if wire.flat() {
                continue;
            }
            for (end, marker) in [wire.spec.start_marker, wire.spec.end_marker]
                .into_iter()
                .enumerate()
            {
                let public = wire.contacts[end].expect("shell wire contact");
                let mut contact = travel(wire, end);
                let range = movable_span(
                    wire.ends[end],
                    public.side,
                    marker,
                    nodes,
                    shells,
                    graph,
                    options,
                );
                if range.is_none() != fixed {
                    continue;
                }
                if !fixed {
                    let terminal = wire.ends[end];
                    let (sides, len) = candidate_faces(terminal, public.side);
                    for &side in &sides[..len] {
                        let Some((rect, span)) =
                            movable_span(terminal, side, marker, nodes, shells, graph, options)
                        else {
                            continue;
                        };
                        let center = cross(side, rect.center());
                        let desired = if side == public.side {
                            cross(side, public.at)
                        } else {
                            center
                        };
                        let stem = normal(terminal, side);
                        let at = face(rect, side, desired);
                        router.blocked_contacts(
                            id as u32,
                            (at, stem),
                            marker,
                            &mut blocked,
                            work,
                        )?;
                        work.visit(blocked.len())?;
                        let chosen = if blocked.iter().any(|&(lo, hi)| lo < desired && desired < hi)
                        {
                            ports::closest_clear_port(
                                desired,
                                (center - span / 2.0, center + span / 2.0),
                                &blocked,
                                work,
                            )?
                        } else {
                            Some(desired)
                        };
                        if let Some(chosen) = chosen {
                            contact = Contact {
                                at: face(rect, side, chosen),
                                side: stem,
                            };
                        } else if marker.length > 0.0
                            && marker.width > 0.0
                            && router.marker_clear(id as u32, (at, stem), marker, work)?
                        {
                            contact = Contact { at, side: stem };
                        } else {
                            continue;
                        }
                        wire.contacts[end] = Some(Contact {
                            at: contact.at,
                            side,
                        });
                        break;
                    }
                }
                router.reserve_lead(id as u32, (contact.at, contact.side), marker, work)?;
            }
        }
    }
    Ok(())
}

struct Extent {
    low: Point,
    high: Point,
}

impl Extent {
    fn new(content: Size) -> Self {
        Self {
            low: Point::default(),
            high: Point {
                x: content.width,
                y: content.height,
            },
        }
    }

    fn include(&mut self, rect: Rect) -> LayoutResult<()> {
        self.low.x = self.low.x.min(finite(rect.x)?);
        self.low.y = self.low.y.min(finite(rect.y)?);
        self.high.x = self.high.x.max(finite(rect.x + rect.width)?);
        self.high.y = self.high.y.max(finite(rect.y + rect.height)?);
        Ok(())
    }

    fn route(&mut self, edge: &EdgeGeometry, wire: &Wire, work: &mut Work<'_>) -> LayoutResult<()> {
        for &knot in &edge.points {
            work.visit(1)?;
            self.low.x = self.low.x.min(finite(knot.x)?);
            self.low.y = self.low.y.min(finite(knot.y)?);
            self.high.x = self.high.x.max(knot.x);
            self.high.y = self.high.y.max(knot.y);
        }
        for (center, size) in [
            (edge.label_center, wire.spec.label),
            (edge.start_label_center, wire.spec.start_label),
            (edge.end_label_center, wire.spec.end_label),
        ] {
            if let (Some(center), Some(size)) = (center, size) {
                self.include(centered(center, size))?;
            }
        }
        for (end, marker) in [wire.spec.start_marker, wire.spec.end_marker]
            .into_iter()
            .enumerate()
        {
            if marker.length > 0.0 && marker.width > 0.0 {
                self.include(routes::marker_rect(travel(wire, end), marker))?;
            }
        }
        Ok(())
    }

    fn normalize(&self, frame: &mut Frame, work: &mut Work<'_>) -> LayoutResult<()> {
        frame.bounds = Size {
            width: finite(self.high.x - self.low.x)?,
            height: finite(self.high.y - self.low.y)?,
        };
        let offset = Point {
            x: -self.low.x,
            y: -self.low.y,
        };
        work.visit(frame.items.len() + frame.lane_shells.len())?;
        for item in &mut frame.items {
            item.x += offset.x;
            item.y += offset.y;
        }
        for (rect, title) in &mut frame.lane_shells {
            rect.x += offset.x;
            rect.y += offset.y;
            *title = title.map(|p| translated(p, offset));
        }
        for edge in &mut frame.legs {
            work.visit(edge.points.len() + 1)?;
            for knot in &mut edge.points {
                knot.x += offset.x;
                knot.y += offset.y;
            }
            for center in [
                &mut edge.label_center,
                &mut edge.start_label_center,
                &mut edge.end_label_center,
            ]
            .into_iter()
            .flatten()
            {
                *center = translated(*center, offset);
            }
        }
        Ok(())
    }
}

fn finish(
    ctx: &Pass<'_>,
    graph: &Graph,
    drawing: LayoutOutput,
    lane_geometry: routing::Lanes<'_>,
    mut wires: Vec<Wire>,
    work: &mut Work<'_>,
) -> LayoutResult<Frame> {
    let options = ctx.options;
    let shells = &lane_geometry.rects;
    let local = drawing.edges.into_iter();
    let custom = wires.iter().any(|wire| !wire.flat());
    for (wire, edge) in wires.iter_mut().filter(|wire| wire.flat()).zip(local) {
        work.visit(1)?;
        wire.route = Some(edge);
    }
    ports::items(&mut wires, graph, &drawing.nodes, options, work)?;
    let mut router = BoundaryRoutes::new();
    let mut extent = Extent::new(drawing.bounds);
    for (&id, &(rect, title)) in ctx.graph.lane_order.iter().zip(shells) {
        work.visit(1)?;
        extent.include(rect)?;
        if let (Some(center), Some(size)) = (title, ctx.graph.groups[id.0 as usize].title) {
            let label = centered(center, size);
            extent.include(label)?;
            if custom {
                router.obstacle(label, work)?;
            }
        }
    }
    if custom {
        shell_contacts(&mut wires, shells, options, work)?;
        for &rect in &drawing.nodes {
            router.obstacle(rect, work)?;
        }
    }
    // Record all ordinary legs before routing explicit shell endpoints, irrespective of input order.
    for (id, wire) in wires.iter_mut().enumerate() {
        work.visit(1)?;
        if let Some(edge) = &mut wire.route {
            for (end, contact) in wire.contacts.into_iter().enumerate() {
                routes::snap(edge, end, contact.expect("flat item contact"));
            }
            if custom {
                router.record(id as u32, edge, &wire.spec, work)?;
            }
        }
    }
    if custom {
        reserve_shell_leads(
            &mut wires,
            &drawing.nodes,
            shells,
            graph,
            options,
            &mut router,
            work,
        )?;
    }
    for (id, wire) in wires.iter_mut().enumerate() {
        work.visit(1)?;
        if wire.route.is_none() {
            let contacts = [travel(wire, 0), travel(wire, 1)].map(|p| (p.at, p.side));
            let mut edge = router.draw(id as u32, &wire.spec, contacts, options, work)?;
            let [source, target] = wire
                .contacts
                .map(|c| c.expect("resolved root contact").side);
            edge.ports = (source, target);
            wire.route = Some(edge);
        }
    }
    routing::improve(ctx, graph, &drawing.nodes, &lane_geometry, &mut wires, work)?;
    let mut legs = reserved(wires.len())?;
    for mut wire in wires {
        work.visit(1)?;
        let edge = wire.route.take().expect("resolved lane route");
        extent.route(&edge, &wire, work)?;
        legs.push(edge);
    }
    let mut frame = Frame {
        bounds: drawing.bounds,
        items: drawing.nodes,
        item_ranks: if options.capture_node_ranks {
            drawing.node_ranks
        } else {
            Vec::new()
        },
        title: None,
        legs,
        ports: Vec::new(),
        stats: drawing.stats,
        lane_shells: lane_geometry.rects,
    };
    extent.normalize(&mut frame, work)?;
    Ok(frame)
}

pub(super) fn root(
    ctx: &Pass<'_>,
    membership: &LaneMembership,
    graph: &Graph,
    mut wires: Vec<Wire>,
    expanded: &mut Expanded,
    work: &mut Work<'_>,
) -> LayoutResult<Frame> {
    let items = &ctx.hierarchy.levels[ctx.graph.groups.len()].items;
    work.charge(items.len())?;
    let mut owners = reserved(items.len())?;
    owners.extend(items.iter().map(|&item| membership.item(ctx.graph, item)));
    let axes = Axes::new(graph.direction);
    work.charge(ctx.graph.lane_order.len())?;
    let mut shells = reserved(ctx.graph.lane_order.len())?;
    for &id in &ctx.graph.lane_order {
        work.visit(1)?;
        shells.push(Shell::new(&ctx.graph.groups[id.0 as usize], axes));
    }
    choose_sides(&mut wires, &mut shells, &owners, axes, ctx.options, work)?;
    let mut columns = reserved(shells.len())?;
    columns.extend(shells.iter().map(|shell| shell.column(axes)));
    let mut rank_options = *ctx.options;
    // Lane compaction moves complete rank bands and needs their membership.
    rank_options.capture_node_ranks |= rank_options.compact_ranks;
    let (mut drawing, mut placed) =
        flat::lanes::run(graph, &rank_options, expanded, &owners, &columns, work)?;
    let (feedback, widened) = routing::reserve_feedback(
        &drawing,
        graph,
        &wires,
        &owners,
        &mut shells,
        ctx.options,
        work,
    )?;
    if widened {
        for (column, shell) in columns.iter_mut().zip(&shells) {
            work.visit(1)?;
            *column = shell.column(axes);
        }
        (drawing, placed) =
            flat::lanes::run(graph, &rank_options, expanded, &owners, &columns, work)?;
    }
    if ctx.options.compact_ranks {
        compact::close(
            &mut drawing,
            graph,
            expanded,
            axes,
            &ctx.options.spacing,
            work,
        )?;
    }
    let rects = shell_rects(&shells, &placed, drawing.bounds, axes, work)?;
    let lane_geometry = routing::Lanes {
        axes,
        owners: &owners,
        rects,
        feedback: &feedback,
    };
    finish(ctx, graph, drawing, lane_geometry, wires, work)
}
