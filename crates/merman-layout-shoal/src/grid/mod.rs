//! Four-sided grid placement and orthogonal routing for measured Architecture graphs.

mod place;
pub(crate) mod route;
#[cfg(test)]
mod tests;

use crate::flat;
use crate::work::{LayoutResult, Work, filled, push, reserved};
use crate::{
    Axis, Direction, Endpoint, Graph, GroupId, LayoutError, LayoutGeometry, LayoutOutput, NodeId,
    Options, Point, PortSideHint, Side, Size, WorkControl, WorkError,
};

/// Two nodes in a declared row (horizontal) or column (vertical), in authored order.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Alignment {
    pub first: NodeId,
    pub next: NodeId,
    pub axis: Axis,
}

/// Lays out measured nodes and nested groups on an integer grid and routes their physical ports.
/// Declared alignments take precedence over edge placement preferences. An occupied cell or a
/// conflicting preference may turn an edge, while both of its declared contact sides stay fixed.
/// Geometry is returned in input order, using drawing units. Captured ranks are local grid indices.
pub fn layout_with_control(
    graph: &Graph,
    alignments: &[Alignment],
    options: &Options,
    control: &mut impl WorkControl,
) -> Result<LayoutGeometry, LayoutError> {
    let mut work = Work::new(control);
    work.check()?;
    flat::validate_options(options)?;
    if !graph.lane_order.is_empty() {
        return Err(LayoutError::Unsupported(
            "swimlane placement on an Architecture grid",
        ));
    }
    let mut hierarchy = Hierarchy::new(graph, &mut work)?;
    let mut metrics = Metrics::new(options);
    let projections = hierarchy.edges(graph, &mut metrics, &mut work)?;
    let mut edges = reserved(graph.edges.len())?;
    work.charge(alignments.len())?;
    for &Alignment { first, next, axis } in alignments {
        work.visit(1)?;
        let ends = [Endpoint::Node(first), Endpoint::Node(next)];
        let projection = hierarchy.project(ends, &mut work)?;
        let sides = match axis {
            Axis::Horizontal => [Side::Right, Side::Left],
            Axis::Vertical => [Side::Bottom, Side::Top],
        };
        hierarchy.link(projection, sides, true)?;
    }
    let [outward, inward] = match graph.direction {
        Direction::Down => [Side::Bottom, Side::Top],
        Direction::Up => [Side::Top, Side::Bottom],
        Direction::Right => [Side::Right, Side::Left],
        Direction::Left => [Side::Left, Side::Right],
    };
    for (edge, projection) in graph.edges.iter().zip(projections) {
        work.visit(1)?;
        let sides = [
            port_side(edge.source_port, outward),
            port_side(edge.target_port, inward),
        ];
        edges.push((projection, sides));
    }
    // Same-side edges carry less placement information than facing or perpendicular ports.
    for alike in [false, true] {
        for &(projection, sides) in &edges {
            work.visit(1)?;
            if alike == (sides[0] == sides[1]) {
                hierarchy.link(projection, sides, false)?;
            }
        }
    }
    let (mut drawing, tracks) = place::place(graph, &hierarchy, metrics, options, &mut work)?;
    route::route_with_fallback(
        graph,
        &hierarchy,
        &edges,
        metrics,
        options,
        &mut drawing,
        |router, _, ends, bounds, work| {
            route::gutter_route(router, ends, bounds, [&tracks[0], &tracks[1]], work)
        },
        &mut work,
    )?;
    work.finish()?;
    Ok(drawing.into())
}

const BAD_ENDPOINT: LayoutError =
    LayoutError::InvalidInput("grid endpoint is outside its input array");
const PRECISION: LayoutError =
    LayoutError::InvalidInput("grid coordinates exceed finite precision");

#[derive(Clone, Copy)]
pub(crate) struct Metrics {
    separation: f64,
    marker: f64,
    label_clearance: f64,
    loop_gap: f64,
}

impl Metrics {
    pub(crate) fn new(options: &Options) -> Self {
        Self {
            separation: options.spacing.edge.max(1.0),
            marker: 0.0,
            label_clearance: options.spacing.end_label_across,
            loop_gap: options.spacing.loop_gap,
        }
    }

    pub(crate) fn margin(self) -> f64 {
        ((self.marker + self.separation) * 2.0).max(self.loop_gap)
    }
}

#[derive(Clone, Copy)]
pub(crate) struct Projection {
    pub(crate) scope: usize,
    pub(crate) ends: [usize; 2],
    pub(crate) items: [Option<usize>; 2],
}

#[derive(Clone, Copy)]
struct Link {
    ends: [usize; 2],
    items: [usize; 2],
    sides: [Side; 2],
    declared: bool,
}

#[derive(Default)]
pub(crate) struct Scope {
    pub(crate) items: Vec<usize>,
    links: Vec<Link>,
    depth: usize,
    pub(crate) caption: Size,
}

pub(crate) struct Hierarchy {
    pub(crate) scopes: Vec<Scope>,
    pub(crate) parent: Vec<usize>,
    pub(crate) order: Vec<usize>,
    pub(crate) node_count: usize,
}

impl Hierarchy {
    pub(crate) fn new(graph: &Graph, work: &mut Work<'_>) -> LayoutResult<Self> {
        let root = graph.groups.len();
        let node_count = graph.nodes.len();
        for count in [root, node_count, graph.edges.len()] {
            u32::try_from(count).map_err(|_| WorkError::ArithmeticOverflow)?;
        }
        let scopes_len = root.checked_add(1).ok_or(WorkError::ArithmeticOverflow)?;
        let item_count = node_count
            .checked_add(root)
            .ok_or(WorkError::ArithmeticOverflow)?;
        work.charge(
            item_count
                .checked_add(scopes_len)
                .ok_or(WorkError::ArithmeticOverflow)?,
        )?;
        let mut scopes = reserved(scopes_len)?;
        scopes.resize_with(scopes_len, Scope::default);
        let mut parent = reserved(item_count)?;
        let mut add = |owner: Option<GroupId>| -> LayoutResult<()> {
            let owner = match owner {
                None => root,
                Some(id) if (id.0 as usize) < root => id.0 as usize,
                _ => {
                    return Err(LayoutError::InvalidInput(
                        "invalid Architecture grid parent",
                    ));
                }
            };
            push(&mut scopes[owner].items, parent.len())?;
            parent.push(owner);
            Ok(())
        };
        for node in &graph.nodes {
            work.visit(1)?;
            flat::validate_node(node)?;
            add(node.parent)?;
        }
        for group in &graph.groups {
            work.visit(1)?;
            let pad = group.padding;
            if ![pad.top, pad.right, pad.bottom, pad.left]
                .into_iter()
                .all(flat::length)
                || !flat::measured(group.min_size)
                || group.title.is_some_and(|size| !flat::measured(size))
            {
                return Err(LayoutError::InvalidInput(
                    "invalid Architecture grid group measurements",
                ));
            }
            add(group.parent)?;
        }
        let mut order = reserved(scopes_len)?;
        order.push(root);
        let mut cursor = 0;
        while let Some(&scope) = order.get(cursor) {
            work.visit(scopes[scope].items.len() + 1)?;
            let first_child = order.len();
            for &item in &scopes[scope].items {
                if item >= node_count {
                    order.push(item - node_count);
                }
            }
            let depth = scopes[scope].depth + 1;
            for &child in &order[first_child..] {
                scopes[child].depth = depth;
            }
            cursor += 1;
        }
        if order.len() != scopes_len {
            return Err(LayoutError::InvalidInput("cyclic Architecture grid groups"));
        }
        let mut seq = reserved(item_count)?;
        seq.extend(graph.nodes.iter().map(|node| node.seq));
        seq.extend(graph.groups.iter().map(|group| group.seq));
        for &scope in order.iter().rev() {
            let items = &mut scopes[scope].items;
            charge_sort(items.len(), work)?;
            items.sort_unstable_by_key(|&item| (seq[item], item));
            if scope != root
                && let Some(&first) = items.first()
            {
                seq[node_count + scope] = seq[first];
            }
        }
        Ok(Self {
            scopes,
            parent,
            order,
            node_count,
        })
    }

    pub(crate) fn root(&self) -> usize {
        self.scopes.len() - 1
    }

    fn endpoint(&self, item: usize) -> Endpoint {
        if item < self.node_count {
            NodeId(item as u32).into()
        } else {
            GroupId((item - self.node_count) as u32).into()
        }
    }

    pub(crate) fn project(
        &self,
        [first, next]: [Endpoint; 2],
        work: &mut Work<'_>,
    ) -> LayoutResult<Projection> {
        let item = |end| match end {
            Endpoint::Node(id) if (id.0 as usize) < self.node_count => Ok(id.0 as usize),
            Endpoint::Group(id) if (id.0 as usize) < self.root() => {
                Ok(self.node_count + id.0 as usize)
            }
            _ => Err(BAD_ENDPOINT),
        };
        let ends = [item(first)?, item(next)?];
        let container = |item: usize| {
            item.checked_sub(self.node_count)
                .unwrap_or(self.parent[item])
        };
        let [mut left, mut right] = ends.map(container);
        if first == next {
            left = self.parent[ends[0]];
            right = left;
        }
        while left != right {
            work.visit(1)?;
            if self.scopes[left].depth >= self.scopes[right].depth {
                left = self.parent[self.node_count + left];
            } else {
                right = self.parent[self.node_count + right];
            }
        }
        let mut items = [None; 2];
        for (slot, mut item) in items.iter_mut().zip(ends) {
            if item == self.node_count + left {
                continue;
            }
            while self.parent[item] != left {
                work.visit(1)?;
                item = self.node_count + self.parent[item];
            }
            *slot = Some(item);
        }
        Ok(Projection {
            scope: left,
            ends,
            items,
        })
    }

    pub(crate) fn edges(
        &mut self,
        graph: &Graph,
        metrics: &mut Metrics,
        work: &mut Work<'_>,
    ) -> LayoutResult<Vec<Projection>> {
        let mut edges = reserved(graph.edges.len())?;
        work.charge(graph.edges.len())?;
        for edge in &graph.edges {
            work.visit(1)?;
            flat::validate_edge(edge)?;
            let ends = [edge.source, edge.target];
            for (end, hint) in ends.into_iter().zip([edge.source_port, edge.target_port]) {
                if let Endpoint::Node(id) = end {
                    flat::validate_port(
                        graph.nodes.get(id.0 as usize).ok_or(BAD_ENDPOINT)?.size,
                        hint,
                    )?;
                } else if matches!(hint, PortSideHint::FixedOffset { offset, .. } if !offset.is_finite())
                {
                    return Err(LayoutError::InvalidInput(
                        "non-finite grid group port offset",
                    ));
                }
            }
            let projection = self.project(ends, work)?;
            let caption = &mut self.scopes[projection.scope].caption;
            let clearance = metrics.label_clearance * 2.0;
            for size in [edge.label, edge.start_label, edge.end_label]
                .into_iter()
                .flatten()
            {
                caption.width = caption.width.max(size.width + clearance);
                caption.height = caption.height.max(size.height + clearance);
            }
            metrics.marker = metrics
                .marker
                .max(edge.start_marker.length)
                .max(edge.end_marker.length);
            edges.push(projection);
        }
        Ok(edges)
    }

    fn link(
        &mut self,
        projection: Projection,
        sides: [Side; 2],
        declared: bool,
    ) -> LayoutResult<()> {
        if let [Some(first), Some(next)] = projection.items
            && first != next
        {
            push(
                &mut self.scopes[projection.scope].links,
                Link {
                    ends: projection.ends,
                    items: [first, next],
                    sides,
                    declared,
                },
            )?;
        }
        Ok(())
    }
}

fn charge_sort(len: usize, work: &mut Work<'_>) -> LayoutResult<()> {
    if len > 1 {
        work.charge(
            len.checked_mul(len.ilog2() as usize + 1)
                .ok_or(WorkError::ArithmeticOverflow)?,
        )?;
    }
    Ok(())
}

fn port_side(hint: PortSideHint, fallback: Side) -> Side {
    match hint {
        PortSideHint::Auto => fallback,
        PortSideHint::Side(side) | PortSideHint::FixedOffset { side, .. } => side,
    }
}

// Axis index and sign in drawing coordinates.
fn normal(side: Side) -> (usize, i64) {
    match side {
        Side::Top => (1, -1),
        Side::Right => (0, 1),
        Side::Bottom => (1, 1),
        Side::Left => (0, -1),
    }
}

fn point([x, y]: [f64; 2]) -> Point {
    Point { x, y }
}

pub(crate) fn rect(drawing: &LayoutOutput, item: usize) -> crate::Rect {
    if let Some(&node) = drawing.nodes.get(item) {
        node
    } else {
        drawing.groups[item - drawing.nodes.len()].rect
    }
}
