use super::Pass;
use super::hierarchy::Terminal;
use super::solve::Frame;
use crate::work::{LayoutResult, Work, filled, reserved};
use crate::{
    BorderStitch, EdgeGeometry, EdgeId, Endpoint, GroupGeometry, GroupId, LayoutError,
    LayoutOutput, LayoutStats, Point, Rect, RoutePoint, Side, TruncatedRoute, WorkError,
};

const PRECISION: LayoutError =
    LayoutError::InvalidInput("compound coordinates exceed finite precision");
const SEAM: LayoutError = LayoutError::InvalidInput("discontinuous compound border route");

fn translate(point: Point, origin: Point) -> LayoutResult<Point> {
    let moved = Point {
        x: point.x + origin.x,
        y: point.y + origin.y,
    };
    if moved.x.is_finite() && moved.y.is_finite() {
        Ok(moved)
    } else {
        Err(PRECISION)
    }
}

fn place(rect: Rect, origin: Point) -> LayoutResult<Rect> {
    let top_left = translate(
        Point {
            x: rect.x,
            y: rect.y,
        },
        origin,
    )?;
    let right = top_left.x + rect.width;
    let bottom = top_left.y + rect.height;
    if !right.is_finite()
        || !bottom.is_finite()
        || (rect.width > 0. && top_left.x == right)
        || (rect.height > 0. && top_left.y == bottom)
    {
        return Err(PRECISION);
    }
    Ok(Rect {
        x: top_left.x,
        y: top_left.y,
        ..rect
    })
}

fn near(left: f64, right: f64) -> bool {
    (left - right).abs() <= 16. * f64::EPSILON * left.abs().max(right.abs()).max(1.)
}

fn same(left: Point, right: Point) -> bool {
    near(left.x, right.x) && near(left.y, right.y)
}

fn on_face(point: Point, rect: Rect, side: Side) -> LayoutResult<Point> {
    let right = rect.x + rect.width;
    let bottom = rect.y + rect.height;
    let contact = match side {
        Side::Top => Point {
            x: point.x.clamp(rect.x, right),
            y: rect.y,
        },
        Side::Bottom => Point {
            x: point.x.clamp(rect.x, right),
            y: bottom,
        },
        Side::Left => Point {
            x: rect.x,
            y: point.y.clamp(rect.y, bottom),
        },
        Side::Right => Point {
            x: right,
            y: point.y.clamp(rect.y, bottom),
        },
    };
    same(contact, point).then_some(contact).ok_or(SEAM)
}

struct Coordinates {
    nodes: Vec<Rect>,
    node_ranks: Vec<u32>,
    groups: Vec<GroupGeometry>,
    origins: Vec<Point>,
    borders: Vec<Point>,
}

impl Coordinates {
    fn new(ctx: &Pass<'_>, frames: &[Option<Frame>], work: &mut Work<'_>) -> LayoutResult<Self> {
        let hierarchy = ctx.hierarchy;
        let mut groups = reserved(ctx.graph.groups.len())?;
        groups.extend(
            hierarchy.levels[..ctx.graph.groups.len()]
                .iter()
                .map(|level| GroupGeometry {
                    effective_direction: level.direction,
                    parent: None,
                    rect: Rect::default(),
                    title_center: None,
                }),
        );
        let mut result = Self {
            nodes: filled(ctx.graph.nodes.len(), Rect::default())?,
            node_ranks: if ctx.options.capture_node_ranks {
                work.charge(ctx.graph.nodes.len())?;
                filled(ctx.graph.nodes.len(), 0)?
            } else {
                Vec::new()
            },
            groups,
            origins: filled(frames.len(), Point::default())?,
            borders: filled(hierarchy.borders.len(), Point::default())?,
        };
        for &id in &hierarchy.order {
            let level = &hierarchy.levels[id];
            let frame = frames[id].as_ref().expect("solved level");
            let origin = result.origins[id];
            work.visit(level.items.len() + level.ports.len() + 1)?;
            if level.items.len() != frame.items.len()
                || (ctx.options.capture_node_ranks && level.items.len() != frame.item_ranks.len())
                || level.ports.len() != frame.ports.len()
                || level.legs.len() != frame.legs.len()
            {
                return Err(LayoutError::InvalidInput("incomplete compound level"));
            }
            for (slot, (&item, &rect)) in level.items.iter().zip(&frame.items).enumerate() {
                let rect = place(rect, origin)?;
                match item {
                    Endpoint::Node(node) => {
                        let index = node.0 as usize;
                        result.nodes[index] = rect;
                        if let Some(rank) = result.node_ranks.get_mut(index) {
                            *rank = frame.item_ranks[slot];
                        }
                    }
                    Endpoint::Group(group) => {
                        let child = group.0 as usize;
                        result.groups[child].rect = rect;
                        result.groups[child].parent =
                            (id < ctx.graph.groups.len()).then_some(GroupId(id as u32));
                        result.origins[child] = Point {
                            x: rect.x,
                            y: rect.y,
                        };
                    }
                }
            }
            if let Some(group) = result.groups.get_mut(id) {
                group.title_center = frame
                    .title
                    .map(|point| translate(point, origin))
                    .transpose()?;
                for (&border, &point) in level.ports.iter().zip(&frame.ports) {
                    result.borders[border] =
                        on_face(translate(point, origin)?, group.rect, ctx.sides[border])?;
                }
            }
        }
        Ok(result)
    }

    fn contact(&self, terminal: Terminal, side: Side, point: Point) -> LayoutResult<Point> {
        match terminal {
            Terminal::Boundary(border) | Terminal::Item(_, Some(border)) => {
                let contact = self.borders[border];
                same(contact, point).then_some(contact).ok_or(SEAM)
            }
            Terminal::Item(Endpoint::Node(node), None) => {
                on_face(point, self.nodes[node.0 as usize], side)
            }
            Terminal::Item(Endpoint::Group(group), None) => {
                on_face(point, self.groups[group.0 as usize].rect, side)
            }
        }
    }
}

fn append_label(label: &mut Option<Point>, from: Option<Point>, origin: Point) -> LayoutResult<()> {
    if let Some(point) = from {
        if label.is_some() {
            return Err(LayoutError::InvalidInput(
                "duplicate compound label occurrence",
            ));
        }
        *label = Some(translate(point, origin)?);
    }
    Ok(())
}

fn stitch(
    ctx: &Pass<'_>,
    path: &[(usize, usize)],
    coordinates: &Coordinates,
    frames: &[Option<Frame>],
    stitches: &mut Vec<BorderStitch>,
    work: &mut Work<'_>,
) -> LayoutResult<EdgeGeometry> {
    let &(first_level, first_leg) = path
        .first()
        .ok_or(LayoutError::InvalidInput("missing compound route"))?;
    let first = &frames[first_level].as_ref().expect("solved level").legs[first_leg];
    let mut output = EdgeGeometry {
        points: Vec::new(),
        ports: first.ports,
        label_center: None,
        start_label_center: None,
        end_label_center: None,
        reversed_for_ranking: false,
    };
    for &(level, slot) in path {
        let leg = &ctx.hierarchy.levels[level].legs[slot];
        let part = &frames[level].as_ref().expect("solved level").legs[slot];
        let origin = coordinates.origins[level];
        if part.points.len() < 2 {
            return Err(LayoutError::InvalidInput("incomplete compound route"));
        }
        work.charge(part.points.len())?;
        output
            .points
            .try_reserve(part.points.len())
            .map_err(|_| LayoutError::AllocationLimit)?;
        for (index, &point) in part.points.iter().enumerate() {
            let moved = translate(
                Point {
                    x: point.x,
                    y: point.y,
                },
                origin,
            )?;
            let moved = if index == 0 {
                coordinates.contact(leg.source, part.ports.0, moved)?
            } else if index + 1 == part.points.len() {
                coordinates.contact(leg.target, part.ports.1, moved)?
            } else {
                moved
            };
            if index == 0
                && let Some(previous) = output.points.last()
            {
                if previous.x != moved.x || previous.y != moved.y {
                    return Err(SEAM);
                }
                let (Terminal::Boundary(border) | Terminal::Item(_, Some(border))) = leg.source
                else {
                    return Err(SEAM);
                };
                crate::work::push(
                    stitches,
                    BorderStitch {
                        edge: EdgeId(leg.edge as u32),
                        point: output.points.len() - 1,
                        group: GroupId(ctx.hierarchy.borders[border].level as u32),
                        side: ctx.sides[border],
                    },
                )?;
                continue;
            }
            output.points.push(RoutePoint {
                x: moved.x,
                y: moved.y,
                ..point
            });
        }
        output.ports.1 = part.ports.1;
        if leg.main {
            output.reversed_for_ranking = part.reversed_for_ranking;
        }
        append_label(&mut output.label_center, part.label_center, origin)?;
        append_label(
            &mut output.start_label_center,
            part.start_label_center,
            origin,
        )?;
        append_label(&mut output.end_label_center, part.end_label_center, origin)?;
    }
    Ok(output)
}

pub(super) fn drawing(
    ctx: &Pass<'_>,
    frames: &[Option<Frame>],
    work: &mut Work<'_>,
) -> LayoutResult<LayoutOutput> {
    let root = frames[ctx.graph.groups.len()]
        .as_ref()
        .expect("solved root");
    let coordinates = Coordinates::new(ctx, frames, work)?;
    let mut edges = reserved(ctx.graph.edges.len())?;
    let mut truncated_routes = Vec::new();
    let mut border_stitches = reserved(ctx.hierarchy.borders.len())?;
    let mut stats = LayoutStats {
        components: root.stats.components,
        truncated_border_ports: ctx.hierarchy.truncated,
        ..Default::default()
    };
    for frame in frames.iter().flatten() {
        work.visit(1)?;
        stats.scc_cyclic_count = stats
            .scc_cyclic_count
            .checked_add(frame.stats.scc_cyclic_count)
            .ok_or(WorkError::ArithmeticOverflow)?;
        stats.dummy_nodes = stats
            .dummy_nodes
            .checked_add(frame.stats.dummy_nodes)
            .ok_or(WorkError::ArithmeticOverflow)?;
        stats.direct_fallback_edges = stats
            .direct_fallback_edges
            .checked_add(frame.stats.direct_fallback_edges)
            .ok_or(WorkError::ArithmeticOverflow)?;
        stats.topological_crossings = stats
            .topological_crossings
            .checked_add(frame.stats.topological_crossings)
            .ok_or(WorkError::ArithmeticOverflow)?;
    }
    for (index, path) in ctx.hierarchy.paths.iter().enumerate() {
        let edge = stitch(ctx, path, &coordinates, frames, &mut border_stitches, work)?;
        if ctx.hierarchy.truncated != 0 {
            let owner = |terminal| match terminal {
                Terminal::Item(endpoint, _) => endpoint,
                Terminal::Boundary(port) => {
                    Endpoint::Group(GroupId(ctx.hierarchy.borders[port].level as u32))
                }
            };
            let (first, last) = (path[0], path[path.len() - 1]);
            let source = owner(ctx.hierarchy.levels[first.0].legs[first.1].source);
            let target = owner(ctx.hierarchy.levels[last.0].legs[last.1].target);
            let spec = &ctx.graph.edges[index];
            if (spec.source, spec.target) != (source, target) {
                crate::work::push(
                    &mut truncated_routes,
                    TruncatedRoute {
                        edge: EdgeId(index as u32),
                        source,
                        target,
                    },
                )?;
            }
        }
        stats.reversed_edges += usize::from(edge.reversed_for_ranking);
        edges.push(edge);
    }
    Ok(LayoutOutput {
        nodes: coordinates.nodes,
        node_ranks: coordinates.node_ranks,
        groups: coordinates.groups,
        edges,
        truncated_routes,
        border_stitches,
        bounds: root.bounds,
        direction: ctx.graph.direction,
        stats,
    })
}
