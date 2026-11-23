mod fit;
mod ports;
mod routes;
#[cfg(test)]
mod tests;

use super::Pass;
use super::hierarchy::Terminal;
use super::sides::flow_sides;
use crate::flat::{self, BoundaryRank, Expanded};
use crate::work::{LayoutResult, Work, filled, reserved};
use crate::{
    Axis, EdgeGeometry, EdgeSpec, Endpoint, Graph, LayoutError, LayoutStats, NodeId, NodeSpec,
    OrderHint, Point, PortSideHint, Rect, RoutePoint, Side, Size, WorkError,
};

#[derive(Debug)]
pub(super) struct Frame {
    pub(super) bounds: Size,
    pub(super) items: Vec<Rect>,
    pub(super) item_ranks: Vec<u32>,
    pub(super) title: Option<Point>,
    pub(super) legs: Vec<EdgeGeometry>,
    pub(super) ports: Vec<Point>,
    pub(super) stats: LayoutStats,
}

const SIDES: [Side; 4] = [Side::Top, Side::Right, Side::Bottom, Side::Left];

fn finite(value: f64) -> LayoutResult<f64> {
    if value.is_finite() {
        Ok(value)
    } else {
        Err(WorkError::ArithmeticOverflow.into())
    }
}

fn cross(side: Side, point: Point) -> f64 {
    match side.axis() {
        Axis::Vertical => point.x,
        Axis::Horizontal => point.y,
    }
}

fn across(side: Side, size: Size) -> f64 {
    match side.axis() {
        Axis::Vertical => size.width,
        Axis::Horizontal => size.height,
    }
}

fn depth(side: Side, size: Size) -> f64 {
    match side.axis() {
        Axis::Vertical => size.height,
        Axis::Horizontal => size.width,
    }
}

fn face(rect: Rect, side: Side, at: f64) -> Point {
    match side {
        Side::Top => Point { x: at, y: rect.y },
        Side::Right => Point {
            x: rect.x + rect.width,
            y: at,
        },
        Side::Bottom => Point {
            x: at,
            y: rect.y + rect.height,
        },
        Side::Left => Point { x: rect.x, y: at },
    }
}

fn towards(point: Point, side: Side, distance: f64) -> Point {
    match side {
        Side::Top => Point {
            y: point.y - distance,
            ..point
        },
        Side::Right => Point {
            x: point.x + distance,
            ..point
        },
        Side::Bottom => Point {
            y: point.y + distance,
            ..point
        },
        Side::Left => Point {
            x: point.x - distance,
            ..point
        },
    }
}

fn translated(point: Point, offset: Point) -> Point {
    Point {
        x: point.x + offset.x,
        y: point.y + offset.y,
    }
}

fn point(knot: RoutePoint) -> Point {
    Point {
        x: knot.x,
        y: knot.y,
    }
}

fn centered(point: Point, size: Size) -> Rect {
    Rect {
        x: point.x - size.width / 2.0,
        y: point.y - size.height / 2.0,
        width: size.width,
        height: size.height,
    }
}

fn named(hint: PortSideHint) -> Option<Side> {
    match hint {
        PortSideHint::Auto => None,
        PortSideHint::Side(side) | PortSideHint::FixedOffset { side, .. } => Some(side),
    }
}

#[derive(Clone, Copy)]
struct Contact {
    at: Point,
    side: Side,
}

#[derive(Clone, Copy)]
enum End {
    Item {
        slot: usize,
        hint: PortSideHint,
        child: Option<Contact>,
    },
    Border {
        slot: usize,
        side: Side,
        flat: Option<NodeId>,
        hint: PortSideHint,
    },
}

impl End {
    fn node(self) -> Option<NodeId> {
        match self {
            Self::Item { slot, .. } => Some(NodeId(slot as u32)),
            Self::Border { flat, .. } => flat,
        }
    }

    fn port(self) -> PortSideHint {
        match self {
            Self::Item { hint, .. } => hint,
            Self::Border { .. } => PortSideHint::Auto,
        }
    }
}

struct Wire {
    ends: [End; 2],
    spec: EdgeSpec,
    own: bool,
    band: bool,
    route: Option<EdgeGeometry>,
    contacts: [Option<Contact>; 2],
}

impl Wire {
    fn border(&self) -> Option<(usize, usize, Side)> {
        self.ends.iter().enumerate().find_map(|(end, &terminal)| {
            if let End::Border { slot, side, .. } = terminal {
                Some((end, slot, side))
            } else {
                None
            }
        })
    }

    fn flat(&self) -> bool {
        self.ends.iter().all(|end| end.node().is_some())
    }

    fn manual_end_label(&self, end: usize) -> bool {
        !self.flat() || self.own || matches!(self.ends[end], End::Border { .. })
    }
}

fn end(
    terminal: Terminal,
    original: Endpoint,
    hint: PortSideHint,
    ctx: &Pass<'_>,
    frames: &[Option<Frame>],
    dummies: &[Option<NodeId>],
) -> LayoutResult<End> {
    let hierarchy = ctx.hierarchy;
    Ok(match terminal {
        Terminal::Boundary(border) => {
            let slot = hierarchy.borders[border].slot;
            End::Border {
                slot,
                side: ctx.sides[border],
                flat: dummies[slot],
                hint,
            }
        }
        Terminal::Item(item, border) => {
            let slot = hierarchy.item_slot(item);
            let mut child = None;
            let hint = if let Some(border) = border {
                let record = &hierarchy.borders[border];
                let frame = frames[record.level]
                    .as_ref()
                    .expect("child solved before its parent");
                let at = frame.ports[record.slot];
                let side = ctx.sides[border];
                child = Some(Contact { at, side });
                PortSideHint::FixedOffset {
                    side,
                    offset: finite(cross(side, at) - across(side, frame.bounds) / 2.0)?,
                }
            } else if item == original {
                hint
            } else {
                PortSideHint::Auto
            };
            End::Item { slot, hint, child }
        }
    })
}

fn measure(
    id: usize,
    ctx: &Pass<'_>,
    frames: &[Option<Frame>],
    work: &mut Work<'_>,
) -> LayoutResult<(Graph, Vec<Wire>)> {
    let level = &ctx.hierarchy.levels[id];
    let count = level
        .items
        .len()
        .checked_add(level.ports.len())
        .ok_or(WorkError::ArithmeticOverflow)?;
    u32::try_from(count).map_err(|_| WorkError::ArithmeticOverflow)?;
    work.charge(
        count
            .checked_add(level.legs.len())
            .ok_or(WorkError::ArithmeticOverflow)?,
    )?;
    let mut graph = Graph {
        direction: level.direction,
        nodes: reserved(count)?,
        edges: reserved(level.legs.len())?,
        ..Graph::default()
    };
    for (slot, &item) in level.items.iter().enumerate() {
        work.visit(1)?;
        let mut node = match item {
            Endpoint::Node(node) => {
                let mut measured = ctx.graph.nodes[node.0 as usize];
                if flow_sides(level.direction).0.axis() != flow_sides(ctx.graph.direction).0.axis()
                {
                    let primary = measured.port_span;
                    measured.port_span = measured.perpendicular_port_span.or(primary);
                    measured.perpendicular_port_span = primary;
                }
                measured
            }
            Endpoint::Group(group) => {
                let child = frames[group.0 as usize]
                    .as_ref()
                    .expect("child solved before its parent");
                let mut node = NodeSpec::new(child.bounds, slot as u32);
                node.group_proxy = true;
                node
            }
        };
        node.parent = None;
        node.seq = slot as u32;
        node.align_flow(level.direction);
        node.order_hint = node.order_hint_in(level.direction);
        if let Some(cross_key) = ctx.items[super::item_index(ctx.graph, item)] {
            node.internal_order_hint = Some(OrderHint {
                rank: node.order_hint.map_or(0, |hint| hint.rank),
                cross_key,
            });
        }
        graph.nodes.push(node);
    }
    let (start, finish) = flow_sides(level.direction);
    let mut dummies = reserved(level.ports.len())?;
    for &border in &level.ports {
        work.visit(1)?;
        let side = ctx.sides[border];
        if side == start || side == finish {
            let index = graph.nodes.len() as u32;
            let mut node = NodeSpec::new(Size::default(), index);
            node.boundary = Some(if side == start {
                BoundaryRank::Start
            } else {
                BoundaryRank::End
            });
            node.internal_order_hint =
                ctx.ports[border].map(|cross_key| OrderHint { rank: 0, cross_key });
            graph.nodes.push(node);
            dummies.push(Some(NodeId(index)));
        } else {
            dummies.push(None);
        }
    }
    let mut pins = filled(level.items.len(), 0i64)?;
    let mut wires = reserved(level.legs.len())?;
    for (slot, leg) in level.legs.iter().enumerate() {
        work.visit(1)?;
        let mut spec = ctx.graph.edges[leg.edge];
        let path = &ctx.hierarchy.paths[leg.edge];
        let &(first_level, first_leg) = path.first().expect("edge has a source leg");
        let &(last_level, last_leg) = path.last().expect("edge has a target leg");
        let own = spec.label.is_some()
            && (matches!(
                ctx.hierarchy.levels[first_level].legs[first_leg].source,
                Terminal::Boundary(_)
            ) || matches!(
                ctx.hierarchy.levels[last_level].legs[last_leg].target,
                Terminal::Boundary(_)
            ));
        if (first_level, first_leg) != (id, slot) {
            spec.start_marker = Default::default();
            spec.start_label = None;
            spec.source_port = PortSideHint::Auto;
        }
        if (last_level, last_leg) != (id, slot) {
            spec.end_marker = Default::default();
            spec.end_label = None;
            spec.target_port = PortSideHint::Auto;
        }
        if !leg.main {
            spec.label = None;
            spec.min_rank_span = 1;
        }
        let ends = [
            end(
                leg.source,
                spec.source,
                spec.source_port,
                ctx,
                frames,
                &dummies,
            )?,
            end(
                leg.target,
                spec.target,
                spec.target_port,
                ctx,
                frames,
                &dummies,
            )?,
        ];
        let wire = Wire {
            ends,
            spec,
            own,
            band: own && leg.main,
            route: None,
            contacts: [None; 2],
        };
        if let [Some(source), Some(target)] = ends.map(End::node) {
            let mut local = spec;
            local.source = source.into();
            local.target = target.into();
            local.source_port = ends[0].port();
            local.target_port = ends[1].port();
            if wire.band {
                local.label = None;
            }
            if wire.manual_end_label(0) {
                local.start_label = None;
            }
            if wire.manual_end_label(1) {
                local.end_label = None;
            }
            graph.edges.push(local);
        } else if let Some((border, _, side)) = wire.border()
            && let End::Item { slot, .. } = ends[1 - border]
        {
            pins[slot] += if matches!(side, Side::Top | Side::Left) {
                -1
            } else {
                1
            };
        }
        wires.push(wire);
    }
    for (node, pin) in graph.nodes.iter_mut().zip(pins) {
        node.pin = pin.signum() as i8;
    }
    Ok((graph, wires))
}

pub(super) fn level(
    id: usize,
    ctx: &Pass<'_>,
    frames: &[Option<Frame>],
    expanded: &mut Expanded,
    work: &mut Work<'_>,
) -> LayoutResult<Frame> {
    let (graph, mut wires) = measure(id, ctx, frames, work)?;
    let drawing = flat::run(&graph, ctx.options, expanded, work)?;
    let mut edges = drawing.edges.into_iter();
    for wire in &mut wires {
        work.visit(1)?;
        if wire.flat() {
            wire.route = edges.next();
        }
    }
    let level = &ctx.hierarchy.levels[id];
    let mut nodes = drawing.nodes;
    let mut item_ranks = drawing.node_ranks;
    item_ranks.truncate(level.items.len());
    ports::items(
        &mut wires,
        &graph,
        &nodes[..level.items.len()],
        ctx.options,
        work,
    )?;
    let group = ctx.graph.groups.get(id);
    let mut fit = fit::Fit::new(group, drawing.bounds, &nodes, &wires, ctx.options, work)?;
    let border = |fit: &fit::Fit, work: &mut Work<'_>| -> LayoutResult<_> {
        let mut ports = ports::border(&wires, &nodes, fit, level.ports.len(), ctx.options, work)?;
        let title = fit.title(group, &wires, &ports, ctx.options, work)?;
        if let (Some(title), Some(group)) = (title, group) {
            ports::beside_title(
                &wires,
                &mut ports,
                title,
                fit::title_side(group.title_position),
                fit,
                ctx.options,
                work,
            )?;
        }
        Ok((ports, title))
    };
    let (mut ports, mut title) = border(&fit, work)?;
    if let (Some(rect), Some(group)) = (title, group) {
        let side = fit::title_side(group.title_position);
        let extra = routes::title_room(&wires, &ports, side, &fit, rect, ctx.options, work)?;
        if extra > 0.0 {
            fit.reserve_title_room(side, extra, work)?;
            (ports, title) = border(&fit, work)?;
        }
    }
    nodes.truncate(level.items.len());
    for rect in &mut nodes {
        work.visit(1)?;
        rect.x += fit.offset.x;
        rect.y += fit.offset.y;
    }
    let legs = routes::finish(&fit, title, &nodes, &mut wires, &ports, ctx.options, work)?;
    Ok(Frame {
        bounds: fit.size,
        items: nodes,
        item_ranks,
        title: title.map(Rect::center),
        legs,
        ports,
        stats: drawing.stats,
    })
}
