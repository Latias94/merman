//! Fold a contiguous set of real leaves into a box the ordinary router can avoid.

use super::{
    Component, Drawing, Expanded, draw_component, physical_side, prepare_component,
    to_canonical_port, validate,
};
use crate::work::{LayoutResult, Work, filled, push, reserved};
use crate::{
    Axis, Direction, EdgeGeometry, EdgeSpec, Graph, LayoutError, MarkerMetrics, NodeId, Options,
    PortSideHint, Rect, RoutePoint, RoutingStyle, Side, Size, WorkError,
};

const PRECISION: LayoutError = LayoutError::InvalidInput("folded fan exceeds coordinate precision");

#[derive(Clone, Copy, PartialEq)]
struct Hub {
    vertex: u32,
    incoming: bool,
    offset: Option<f64>,
}

#[derive(Clone, Copy)]
struct Leaf {
    hub: Hub,
    edge: u32,
    offset: f64,
    marker: MarkerMetrics,
    hub_marker: MarkerMetrics,
    span: u32,
}

impl Leaf {
    fn overhang(self, size: Size) -> (f64, f64) {
        let extra = (self.marker.width - size.width) * 0.5;
        (
            (extra - self.offset).max(0.0),
            (extra + self.offset).max(0.0),
        )
    }
}

#[derive(Clone, Copy, Default)]
enum Incidence {
    #[default]
    None,
    One(usize),
    Many,
}

impl Incidence {
    fn record(&mut self, link: usize) {
        *self = match self {
            Self::None => Self::One(link),
            _ => Self::Many,
        };
    }
}

fn facing_offset(direction: Direction, hint: PortSideHint, face: Side) -> Option<Option<f64>> {
    match to_canonical_port(direction, hint) {
        PortSideHint::Auto => Some(None),
        PortSideHint::Side(side) if side == face => Some(None),
        PortSideHint::FixedOffset { side, offset } if side == face => Some(Some(offset)),
        _ => None,
    }
}

fn leaves(
    g: &Expanded,
    c: &Component,
    graph: &Graph,
    work: &mut Work<'_>,
) -> LayoutResult<Vec<Option<Leaf>>> {
    work.charge(c.nodes.len())?;
    let mut degree = filled(c.nodes.len(), Incidence::None)?;
    for (index, link) in c.links.iter().enumerate() {
        work.visit(1)?;
        degree[link.from as usize].record(index);
        degree[link.to as usize].record(index);
    }
    for loop_edge in &c.loops {
        work.visit(1)?;
        degree[loop_edge.node as usize] = Incidence::Many;
    }
    let mut result = reserved(degree.len())?;
    for (vertex, count) in degree.into_iter().enumerate() {
        work.visit(1)?;
        let leaf = (|| {
            let Incidence::One(index) = count else {
                return None;
            };
            let link = c.links[index];
            let spec = &graph.edges[link.edge as usize];
            if spec.label.is_some() || spec.start_label.is_some() || spec.end_label.is_some() {
                return None;
            }
            let incoming = vertex == link.from as usize;
            let hub = if incoming { link.to } else { link.from };
            if (g.rank[vertex] < g.rank[hub as usize]) != incoming {
                return None;
            }
            let [leaf_hint, hub_hint] = if incoming {
                [spec.source_port, spec.target_port]
            } else {
                [spec.target_port, spec.source_port]
            };
            let [leaf_face, hub_face] = if incoming {
                [Side::Bottom, Side::Top]
            } else {
                [Side::Top, Side::Bottom]
            };
            let [marker, hub_marker] = if incoming {
                [spec.start_marker, spec.end_marker]
            } else {
                [spec.end_marker, spec.start_marker]
            };
            Some(Leaf {
                hub: Hub {
                    vertex: hub,
                    incoming,
                    offset: facing_offset(graph.direction, hub_hint, hub_face)?,
                },
                edge: link.edge,
                offset: facing_offset(graph.direction, leaf_hint, leaf_face)?.unwrap_or(0.0),
                marker,
                hub_marker,
                span: link.span,
            })
        })();
        result.push(leaf);
    }
    Ok(result)
}

struct Run<'a> {
    vertices: &'a [u32],
    open: f64,
    minimum: f64,
}

struct Seat {
    vertex: u32,
    leaf: Leaf,
    rect: Rect,
    shelf: f64,
}

struct Fan {
    members: Vec<Seat>,
    size: Size,
    lane: f64,
    exit_y: f64,
}

impl Fan {
    fn representative(&self) -> &Seat {
        &self.members[0]
    }

    fn new(
        run: &Run<'_>,
        leaves: &[Option<Leaf>],
        c: &Component,
        limit: f64,
        lane: f64,
        options: &Options,
        work: &mut Work<'_>,
    ) -> LayoutResult<Option<Self>> {
        work.charge(run.vertices.len())?;
        let mut members = reserved(run.vertices.len())?;
        let row_start = 3.0 * lane;
        let mut x = row_start;
        let mut y = 0.0_f64;
        let mut row_end = 0.0_f64;
        let mut lead = 0.0_f64;
        let mut first = 0;
        let mut width = 0.0_f64;
        let close_row = |row: &mut [Seat], y: f64, lead: f64, work: &mut Work<'_>| {
            for seat in row {
                work.visit(1)?;
                seat.rect.y = y - seat.rect.height;
                seat.shelf = y + lead + lane;
            }
            Ok::<_, LayoutError>(())
        };
        for &vertex in run.vertices {
            work.visit(1)?;
            let leaf = leaves[vertex as usize].expect("run of eligible leaves");
            let size = c.nodes[vertex as usize].size;
            let (before, after) = leaf.overhang(size);
            if first < members.len() && x + before + size.width + after + lane > limit {
                close_row(&mut members[first..], y + row_end, lead, work)?;
                y += row_end + options.spacing.rank.max(lead + 3.0 * lane);
                x = row_start;
                row_end = 0.0;
                lead = 0.0;
                first = members.len();
            }
            x += before;
            members.push(Seat {
                vertex,
                leaf,
                rect: Rect {
                    x,
                    y: 0.0,
                    width: size.width,
                    height: size.height,
                },
                shelf: 0.0,
            });
            x += size.width + after;
            width = width.max(x + lane);
            x += options.spacing.node.max(lane);
            row_end = row_end.max(size.height);
            lead = lead.max(leaf.marker.length);
        }
        if first == 0 || width >= run.open {
            return Ok(None);
        }
        close_row(&mut members[first..], y + row_end, lead, work)?;
        let exit_y = y + row_end + lead + 3.0 * lane;
        let height = exit_y + 2.0 * lane;
        if !width.is_finite() || !height.is_finite() || height <= exit_y {
            return Err(PRECISION);
        }
        Ok(Some(Self {
            members,
            size: Size { width, height },
            lane,
            exit_y,
        }))
    }

    fn guide(
        &self,
        spec: &mut EdgeSpec,
        direction: Direction,
        work: &mut Work<'_>,
    ) -> LayoutResult<()> {
        let incoming = self.representative().leaf.hub.incoming;
        let side = if incoming { Side::Bottom } else { Side::Top };
        let port = PortSideHint::FixedOffset {
            side: physical_side(direction, side),
            offset: 0.0,
        };
        let mut marker = MarkerMetrics::default();
        for seat in &self.members {
            work.visit(1)?;
            marker.width = marker.width.max(seat.leaf.hub_marker.width);
            marker.length = marker.length.max(seat.leaf.hub_marker.length);
            spec.min_rank_span = spec.min_rank_span.max(seat.leaf.span);
        }
        if incoming {
            spec.source_port = port;
            spec.start_marker = MarkerMetrics::default();
            spec.end_marker = marker;
        } else {
            spec.target_port = port;
            spec.end_marker = MarkerMetrics::default();
            spec.start_marker = marker;
        }
        Ok(())
    }

    fn expand_route(
        &self,
        proxy: Rect,
        guide: &EdgeGeometry,
        drawing: &mut Drawing,
        curved: bool,
        work: &mut Work<'_>,
    ) -> LayoutResult<()> {
        if guide.reversed_for_ranking || guide.ports != (Side::Bottom, Side::Top) {
            return Err(LayoutError::InvalidInput("folded fan has no facing guide"));
        }
        let incoming = self.representative().leaf.hub.incoming;
        let guide_x = if incoming {
            guide.points.first()
        } else {
            guide.points.last()
        }
        .ok_or(PRECISION)?
        .x;
        let draw_y = |y: f64| proxy.y + if incoming { y } else { proxy.height - y };
        let bus_x = proxy.x + self.lane;
        let capacity = guide
            .points
            .len()
            .checked_add(7)
            .ok_or(WorkError::ArithmeticOverflow)?;
        for seat in &self.members {
            work.charge(capacity)?;
            let mut rect = seat.rect;
            rect.x += proxy.x;
            rect.y = draw_y(rect.y + if incoming { 0.0 } else { rect.height });
            drawing.nodes[seat.vertex as usize] = rect;
            let tip_y = if incoming {
                rect.y + rect.height
            } else {
                rect.y
            };
            let tip_x = rect.center().x + seat.leaf.offset;
            let point = RoutePoint::new;
            let shoulder_x = (bus_x + self.lane).min(guide_x);
            let shelf_x = if curved { shoulder_x } else { bus_x };
            let release = if curved { self.lane } else { 0.0 };
            let mut branch = [
                point(tip_x, tip_y, Axis::Vertical),
                point(tip_x, draw_y(seat.shelf), Axis::Vertical),
                point(shelf_x, draw_y(seat.shelf), Axis::Horizontal),
                point(bus_x, draw_y(seat.shelf + self.lane), Axis::Vertical),
                point(bus_x, draw_y(self.exit_y), Axis::Vertical),
                point(
                    shoulder_x,
                    draw_y(self.exit_y + release),
                    if curved {
                        Axis::Vertical
                    } else {
                        Axis::Horizontal
                    },
                ),
                point(guide_x, draw_y(self.exit_y + release), Axis::Horizontal),
            ];
            let mut points = reserved(capacity)?;
            if incoming {
                points.extend(branch);
                points.extend_from_slice(&guide.points);
            } else {
                branch.reverse();
                for index in (1..branch.len()).rev() {
                    branch[index].axis = branch[index - 1].axis;
                }
                branch[0].axis = Axis::Vertical;
                points.extend_from_slice(&guide.points);
                points.extend(branch);
            }
            drawing.edges.push((
                seat.leaf.edge,
                EdgeGeometry {
                    points,
                    ports: guide.ports,
                    label_center: None,
                    start_label_center: None,
                    end_label_center: None,
                    shared_bus: false,
                    corridor_clearance: 0.0,
                    reversed_for_ranking: false,
                },
            ));
        }
        Ok(())
    }
}

fn runs<'a>(
    rank: &'a [u32],
    leaves: &[Option<Leaf>],
    c: &Component,
    gap: f64,
    lane: f64,
    out: &mut Vec<Run<'a>>,
    work: &mut Work<'_>,
) -> LayoutResult<()> {
    let leaf = |vertex: u32| leaves.get(vertex as usize).copied().flatten();
    let mut rest = rank;
    while let [first, ..] = rest {
        work.visit(1)?;
        let Some(first) = leaf(*first) else {
            rest = &rest[1..];
            continue;
        };
        let mut count = 0;
        let mut open = 0.0;
        let mut minimum = 0.0_f64;
        for &vertex in rest {
            work.visit(1)?;
            let Some(candidate) = leaf(vertex).filter(|candidate| candidate.hub == first.hub)
            else {
                break;
            };
            let size = c.nodes[vertex as usize].size;
            let (before, after) = candidate.overhang(size);
            open += size.width + if count == 0 { 0.0 } else { gap };
            minimum = minimum.max(size.width + before + after + 4.0 * lane);
            count += 1;
        }
        if count > 1 && minimum < open {
            push(
                out,
                Run {
                    vertices: &rest[..count],
                    open,
                    minimum,
                },
            )?;
        }
        rest = &rest[count..];
    }
    Ok(())
}

fn plans(
    g: &Expanded,
    c: &Component,
    graph: &Graph,
    options: &Options,
    limit: f64,
    work: &mut Work<'_>,
) -> LayoutResult<Vec<Fan>> {
    let leaves = leaves(g, c, graph, work)?;
    let gap = options.spacing.node;
    let lane = (options.spacing.edge * 0.5).max(1.0);
    let mut block = Vec::new();
    let mut eligible = Vec::new();
    for rank in &g.layers {
        work.visit(1)?;
        let mut width = gap * rank.len().saturating_sub(1) as f64;
        for &vertex in rank {
            work.visit(1)?;
            let id = vertex as usize;
            width += g.width[id];
            if id < c.nodes.len() {
                width += c.margin_left[id] + c.margin_right[id];
            }
        }
        if width <= limit {
            continue;
        }
        eligible.clear();
        runs(rank, &leaves, c, gap, lane, &mut eligible, work)?;
        let mut reducible = 0.0;
        for run in &eligible {
            work.visit(1)?;
            reducible += run.open - run.minimum;
        }
        let fraction = if reducible > 0.0 {
            ((width - limit) / reducible).min(1.0)
        } else {
            0.0
        };
        for run in &eligible {
            work.visit(1)?;
            let allotted = run.open - fraction * (run.open - run.minimum);
            if let Some(fan) = Fan::new(run, &leaves, c, allotted, lane, options, work)? {
                push(&mut block, fan)?;
            }
        }
    }
    Ok(block)
}

#[derive(Clone, Copy)]
enum EdgeOrigin {
    Ordinary(u32),
    Fan(usize),
}

pub(super) struct Folded {
    pub(super) drawing: Drawing,
    pub(super) dummies: usize,
    pub(super) fallbacks: usize,
    pub(super) reversed: usize,
}

pub(super) fn fold(
    g: &mut Expanded,
    c: &Component,
    graph: &Graph,
    options: &Options,
    limit: f64,
    work: &mut Work<'_>,
) -> LayoutResult<Option<Folded>> {
    if c.boundaries || c.pinned || c.group_proxies {
        return Ok(None);
    }
    let fans = plans(g, c, graph, options, limit, work)?;
    if fans.is_empty() {
        return Ok(None);
    }
    let edge_count = c.links.len() + c.loops.len();
    work.charge(c.nodes.len())?;
    work.charge(edge_count)?;
    let mut owner = filled(c.nodes.len(), None)?;
    for (id, fan) in fans.iter().enumerate() {
        for seat in &fan.members {
            work.visit(1)?;
            owner[seat.vertex as usize] = Some(id);
        }
    }
    let mut local = Graph {
        direction: graph.direction,
        nodes: reserved(c.nodes.len())?,
        edges: reserved(edge_count)?,
        ..Graph::default()
    };
    let mut remap = filled(c.nodes.len(), NodeId(0))?;
    let mut original = reserved(c.nodes.len())?;
    for (vertex, (node, parent)) in c.nodes.iter().zip(&owner).enumerate() {
        work.visit(1)?;
        let mut node = *node;
        if let Some(id) = *parent {
            if fans[id].representative().vertex as usize != vertex {
                continue;
            }
            node.size = fans[id].size;
            node.anchor_y = None;
            node.position_hint = None;
        }
        remap[vertex] = NodeId(local.nodes.len() as u32);
        original.push(vertex);
        local.nodes.push(node);
    }
    let mut edges = reserved(edge_count)?;
    for (from, to, edge) in c
        .links
        .iter()
        .map(|link| (link.from, link.to, link.edge))
        .chain(c.loops.iter().map(|link| (link.node, link.node, link.edge)))
    {
        work.visit(1)?;
        let mut spec = graph.edges[edge as usize];
        spec.source = remap[from as usize].into();
        spec.target = remap[to as usize].into();
        let origin = match owner[from as usize].or(owner[to as usize]) {
            Some(id) if fans[id].representative().leaf.edge == edge => {
                fans[id].guide(&mut spec, graph.direction, work)?;
                EdgeOrigin::Fan(id)
            }
            Some(_) => continue,
            None => EdgeOrigin::Ordinary(edge),
        };
        edges.push(origin);
        local.edges.push(spec);
    }
    let reduced = validate::collect(&local, options, true, work)?
        .pop()
        .ok_or(PRECISION)?;
    let (orientation, _) = prepare_component(&reduced, &local, options, work, g)?;
    let dummies = g.rank.len() - reduced.nodes.len();
    let mut fallbacks = 0;
    let mut reversed = 0;
    let guides = draw_component(g, &reduced, &orientation, &local, options, work)?;
    work.charge(c.nodes.len())?;
    work.charge(edge_count)?;
    let mut result = Drawing {
        nodes: filled(c.nodes.len(), Rect::default())?,
        edges: reserved(edge_count)?,
    };
    for (&vertex, &rect) in original.iter().zip(&guides.nodes) {
        work.visit(1)?;
        result.nodes[vertex] = rect;
    }
    for (edge, geometry) in guides.edges {
        work.visit(1)?;
        reversed += usize::from(geometry.reversed_for_ranking);
        let direct = g.direct.get(edge as usize).copied().unwrap_or(false);
        match edges[edge as usize] {
            EdgeOrigin::Ordinary(edge) => {
                fallbacks += usize::from(direct);
                result.edges.push((edge, geometry));
            }
            EdgeOrigin::Fan(id) => {
                let fan = &fans[id];
                if direct {
                    fallbacks += fan.members.len();
                }
                let proxy = remap[fan.representative().vertex as usize];
                fan.expand_route(
                    guides.nodes[proxy.0 as usize],
                    &geometry,
                    &mut result,
                    options.routing == RoutingStyle::Curved,
                    work,
                )?;
            }
        }
    }
    Ok(Some(Folded {
        drawing: result,
        dummies,
        fallbacks,
        reversed,
    }))
}
