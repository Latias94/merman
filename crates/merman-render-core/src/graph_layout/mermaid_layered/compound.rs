//! Compound levels, boundary ports and stitching from Mermaid's compound.ts.

use super::{DirectionExt, PointExt, RoutePointExt, SideExt, SizeExt, route_point};
use std::ops::Range;

use super::super::BorderStitch;
use super::crossings::route_crossings;
use super::work::{Work, add, check_slots, cmp, geometry, js_max, js_min};
use super::{
    Attachment, Axis, CompoundError, Direction, LevelEdge, LevelNode, LevelOptions, LevelPlan,
    LevelResult, Membership, Mode, NodeKind, OrderedLevel, PlacedLevel, Point, Result, RoutePoint,
    Side, Size, Solve,
};

const MAX_PORTS: usize = 40_000;
const MAX_ORDERED_PORTS: usize = 400;
const MAX_ORDERED_EDGES: usize = 500;
const FLAT_SLACK: f64 = 1.5;
const FLAT_SPARE: f64 = 4.0;
const JOIN_TOLERANCE: f64 = 0.01;
const IN_LINE: f64 = 0.5;

pub(crate) struct CNode {
    pub(crate) parent: Option<usize>,
    pub(crate) is_group: bool,
    pub(crate) direction: Option<Direction>,
    pub(crate) pad_x: f64,
    pub(crate) pad_top: f64,
    pub(crate) pad_bottom: f64,
    pub(crate) min_width: f64,
    pub(crate) span: f64,
    pub(crate) center: Point,
    pub(crate) size: Size,
    seq: usize,
}

impl CNode {
    pub(crate) fn new(seq: usize, parent: Option<usize>, size: Size) -> Self {
        Self {
            seq,
            parent,
            size,
            is_group: false,
            direction: None,
            pad_x: 0.0,
            pad_top: 0.0,
            pad_bottom: 0.0,
            min_width: 0.0,
            span: 0.0,
            center: Point::default(),
        }
    }
}

pub(crate) struct CEdge {
    pub(crate) ends: [usize; 2],
    pub(crate) label: Size,
    pub(crate) label_at: Point,
    pub(crate) route: Vec<RoutePoint>,
    pub(crate) reversed_for_ranking: bool,
    min_len: u32,
}

impl CEdge {
    pub(crate) fn new(ends: [usize; 2], min_len: u32, label: Size) -> Self {
        Self {
            ends,
            label,
            min_len,
            label_at: Point::default(),
            route: Vec::new(),
            reversed_for_ranking: false,
        }
    }
}

#[derive(Default)]
struct Level {
    /// Combined physical item slot. The root has no item.
    group: Option<usize>,
    direction: Direction,
    items: Vec<usize>,
    edges: Vec<usize>,
    ports: Vec<usize>,
    origin: Point,
}

struct Port {
    edge: usize,
    end: usize,
    inner: usize,
    child: Option<usize>,
    side: Side,
    hint: Option<f64>,
    contact: Point,
    route: Vec<RoutePoint>,
}

struct Projection {
    central: [usize; 2],
    // Inner to outer in each range. The last port meets the central edge.
    paths: [Range<usize>; 2],
    route: Vec<RoutePoint>,
}

impl Projection {
    fn outer_port(&self, end: usize) -> Option<usize> {
        let path = &self.paths[end];
        (!path.is_empty()).then(|| path.end - 1)
    }
}

struct Compound<'a> {
    nodes: &'a mut [CNode],
    edges: &'a mut [CEdge],
    node_count: usize,
    levels: Vec<Level>,
    order: Vec<usize>,
    local: Vec<usize>,
    projections: Vec<Option<Projection>>,
    ports: Vec<Port>,
    border_stitches: Vec<BorderStitch>,
    options: LevelOptions,
}

pub(crate) struct CompoundPlacement {
    pub(crate) size: Size,
    pub(crate) group_directions: Vec<Direction>,
    pub(crate) border_stitches: Vec<BorderStitch>,
}

pub(crate) fn place<E>(
    nodes: &mut [CNode],
    edges: &mut [CEdge],
    node_count: usize,
    root: Direction,
    options: LevelOptions,
    work: &mut Work<'_, E>,
    solve: &mut Solve<'_, E>,
) -> Result<CompoundPlacement, E> {
    let mut drawing = Compound::new(nodes, edges, node_count, root, options, work)?;
    let ordered = drawing.ports.len() > 1
        && drawing.ports.len() <= MAX_ORDERED_PORTS
        && drawing.edges.len() <= MAX_ORDERED_EDGES;
    let flat = ordered && drawing.order.len() > 1 && drawing.same_direction(root, work)?;
    let hints = if flat {
        Some(drawing.order_across_groups(root, work, solve)?)
    } else {
        None
    };
    let mut cache = if ordered {
        work.fill_with(drawing.levels.len(), Vec::new)?
    } else {
        Vec::new()
    };

    let mut size = drawing.place_levels(hints.as_ref(), &mut cache, work, solve)?;
    let mut keyed = flat;
    let mut crossings = None;
    if let Some(hints) = &hints {
        let initial = drawing.crossings(work)?;
        crossings = Some(initial);
        if initial as f64 > hints.crossings * FLAT_SLACK + FLAT_SPARE {
            let first = drawing.snapshot(size, work)?;
            size = drawing.place_levels(None, &mut cache, work, solve)?;
            let independent = drawing.crossings(work)?;
            if independent >= initial {
                size = drawing.restore(first, work)?;
            } else {
                keyed = false;
                crossings = Some(independent);
            }
        }
    }
    if keyed == flat && ordered && drawing.refine_hints(work)? {
        let before = match crossings {
            Some(value) => value,
            None => drawing.crossings(work)?,
        };
        if before > 0 {
            let first = drawing.snapshot(size, work)?;
            size = drawing.place_levels(hints.as_ref(), &mut cache, work, solve)?;
            if drawing.crossings(work)? >= before {
                size = drawing.restore(first, work)?;
            }
        }
    }
    drawing.draw_strays(work)?;

    let mut group_directions = work.reserve(drawing.levels.len() - 1)?;
    work.price(drawing.levels.len() - 1)?;
    for (i, level) in drawing.levels[1..].iter().enumerate() {
        work.poll(i)?;
        group_directions.push(level.direction);
    }
    Ok(CompoundPlacement {
        size,
        group_directions,
        border_stitches: drawing.border_stitches,
    })
}

fn level_of(group: Option<usize>, node_count: usize) -> usize {
    group.map_or(0, |slot| slot - node_count + 1)
}

struct Levels {
    levels: Vec<Level>,
    order: Vec<usize>,
    depth: Vec<usize>,
    local: Vec<usize>,
}

fn make_levels<E>(
    nodes: &mut [CNode],
    node_count: usize,
    root: Direction,
    work: &mut Work<'_, E>,
) -> Result<Levels, E> {
    let group_count = nodes.len() - node_count;
    let mut levels = work.fill_with(add(group_count, 1)?, Level::default)?;
    levels[0].direction = root;
    for (i, level) in levels[1..].iter_mut().enumerate() {
        work.price(1)?;
        level.group = Some(node_count + i);
    }
    let item_count = nodes.len();
    for (i, node) in nodes.iter_mut().enumerate() {
        work.price(1)?;
        if node
            .parent
            .is_some_and(|p| p < node_count || p >= item_count || p == i)
        {
            node.parent = None;
        }
        work.push(&mut levels[level_of(node.parent, node_count)].items, i)?;
    }
    let mut depth = work.fill(nodes.len(), 0)?;
    let mut placed = work.fill(nodes.len(), false)?;
    let mut order = work.reserve(levels.len())?;
    order.push(0);
    let mut cursor = 0;
    descend(
        nodes,
        &mut levels,
        &mut order,
        &mut cursor,
        &mut depth,
        &mut placed,
        node_count,
        work,
    )?;
    for i in 0..nodes.len() {
        work.price(1)?;
        if placed[i] {
            continue;
        }
        // Match the first unplaced slot, which can be a leaf of a parent cycle.
        nodes[i].parent = None;
        work.push(&mut levels[0].items, i)?;
        placed[i] = true;
        depth[i] = 0;
        if nodes[i].is_group {
            let child = level_of(Some(i), node_count);
            levels[child].direction = nodes[i].direction.unwrap_or(root);
            order.push(child);
            descend(
                nodes,
                &mut levels,
                &mut order,
                &mut cursor,
                &mut depth,
                &mut placed,
                node_count,
                work,
            )?;
        }
    }
    for &id in order[1..].iter().rev() {
        work.price(1)?;
        let level = &levels[id];
        let mut seq = None;
        work.price(level.items.len())?;
        for (i, &item) in level.items.iter().enumerate() {
            work.poll(i)?;
            seq = Some(seq.map_or(nodes[item].seq, |seq: usize| seq.min(nodes[item].seq)));
        }
        if let (Some(group), Some(seq)) = (level.group, seq) {
            nodes[group].seq = seq;
        }
    }
    let mut local = work.fill(nodes.len(), 0)?;
    for &id in &order {
        work.price(1)?;
        let items = &mut levels[id].items;
        work.sort_rows(items, |&a, &b| {
            nodes[a].seq.cmp(&nodes[b].seq).then(a.cmp(&b))
        })?;
        work.price(items.len())?;
        for (i, &item) in items.iter().enumerate() {
            work.poll(i)?;
            local[item] = i;
        }
    }
    Ok(Levels {
        levels,
        order,
        depth,
        local,
    })
}

#[allow(clippy::too_many_arguments)]
fn descend<E>(
    nodes: &[CNode],
    levels: &mut [Level],
    order: &mut Vec<usize>,
    cursor: &mut usize,
    depth: &mut [usize],
    placed: &mut [bool],
    node_count: usize,
    work: &mut Work<'_, E>,
) -> Result<(), E> {
    while *cursor < order.len() {
        work.price(1)?;
        let level_id = order[*cursor];
        *cursor += 1;
        let group = levels[level_id].group;
        let dir = levels[level_id].direction;
        let mut items = std::mem::take(&mut levels[level_id].items);
        let mut kept = 0;
        for i in 0..items.len() {
            work.price(1)?;
            let item = items[i];
            // Remove a lifted slot from its original child list as that list is visited.
            if nodes[item].parent != group {
                continue;
            }
            items[kept] = item;
            kept += 1;
            placed[item] = true;
            depth[item] = match group {
                Some(p) => add(depth[p], 1)?,
                None => 0,
            };
            if nodes[item].is_group {
                let child = level_of(Some(item), node_count);
                levels[child].direction = nodes[item].direction.unwrap_or(dir);
                order.push(child);
            }
        }
        items.truncate(kept);
        levels[level_id].items = items;
    }
    Ok(())
}

impl<'a> Compound<'a> {
    fn new<E>(
        nodes: &'a mut [CNode],
        edges: &'a mut [CEdge],
        node_count: usize,
        root: Direction,
        options: LevelOptions,
        work: &mut Work<'_, E>,
    ) -> Result<Self, E> {
        let Levels {
            levels,
            order,
            depth,
            local,
        } = make_levels(nodes, node_count, root, work)?;
        let mut drawing = Self {
            nodes,
            edges,
            node_count,
            levels,
            order,
            local,
            projections: Vec::new(),
            ports: Vec::new(),
            border_stitches: Vec::new(),
            options,
        };
        drawing.project(&depth, work)?;
        Ok(drawing)
    }

    fn project<E>(&mut self, depth: &[usize], work: &mut Work<'_, E>) -> Result<(), E> {
        self.projections = work.reserve(self.edges.len())?;
        for ei in 0..self.edges.len() {
            work.price(1)?;
            let ends = self.edges[ei].ends;
            let (central, crossed) = lift(self.nodes, depth, ends, work)?;
            if central[0] == central[1] {
                self.projections.push(None);
                continue;
            }
            let total = add(self.ports.len(), add(crossed[0], crossed[1])?)?;
            let mut paths = [0..0, 0..0];
            if total > MAX_PORTS {
                self.edges[ei].ends = central;
            } else {
                for end in 0..2 {
                    let mut item = ends[end];
                    let start = self.ports.len();
                    let mut child = None;
                    for _ in 0..crossed[end] {
                        let parent = self.nodes[item]
                            .parent
                            .ok_or(CompoundError::InconsistentLevel("missing crossed group"))?;
                        let level = level_of(Some(parent), self.node_count);
                        let port = self.ports.len();
                        work.push(
                            &mut self.ports,
                            Port {
                                edge: ei,
                                end,
                                inner: item,
                                child,
                                side: Side::Top,
                                hint: None,
                                contact: Point::default(),
                                route: Vec::new(),
                            },
                        )?;
                        work.push(&mut self.levels[level].ports, port)?;
                        child = Some(port);
                        item = parent;
                    }
                    paths[end] = start..self.ports.len();
                }
            }
            let level = level_of(self.nodes[central[0]].parent, self.node_count);
            work.push(&mut self.levels[level].edges, ei)?;
            self.projections.push(Some(Projection {
                central,
                paths,
                route: Vec::new(),
            }));
        }
        Ok(())
    }

    fn same_direction<E>(&self, root: Direction, work: &mut Work<'_, E>) -> Result<bool, E> {
        for level in &self.levels {
            work.price(1)?;
            if level.direction != root {
                return Ok(false);
            }
        }
        Ok(true)
    }

    fn order_across_groups<E>(
        &self,
        direction: Direction,
        work: &mut Work<'_, E>,
        solve: &mut Solve<'_, E>,
    ) -> Result<OrderHints, E> {
        let group_count = self.nodes.len() - self.node_count;
        let mut nodes = work.reserve(self.node_count)?;
        let mut of = work.reserve(self.node_count)?;
        for node in &self.nodes[..self.node_count] {
            work.price(1)?;
            nodes.push(LevelNode {
                size: node.size.in_axes(direction),
                ..LevelNode::default()
            });
            of.push(node.parent.map(|p| p - self.node_count));
        }
        let mut parent = work.reserve(group_count)?;
        for node in &self.nodes[self.node_count..] {
            work.price(1)?;
            parent.push(node.parent.map(|p| p - self.node_count));
        }
        let mut edges = Vec::new();
        let mut edge_slots = work.reserve(self.edges.len())?;
        for edge in self.edges.iter() {
            work.price(1)?;
            let [a, b] = edge.ends;
            let mut local = None;
            if a != b && a < self.node_count && b < self.node_count {
                local = Some(edges.len());
                let mut input = LevelEdge::new(a, b, edge.min_len);
                input.label = edge.label.in_axes(direction);
                work.push(&mut edges, input)?;
            }
            edge_slots.push(local);
        }
        let plan = LevelPlan {
            nodes: &nodes,
            edges: &edges,
            groups: Some(Membership {
                of: &of,
                parent: &parent,
            }),
            options: self.options,
        };
        let LevelResult::Ordered(ordered) = work.solve(solve, plan, Mode::OrderOnly)? else {
            return Err(CompoundError::InconsistentLevel("order-only result"));
        };
        check_order(&ordered, nodes.len(), edges.len(), work)?;
        let mut keys = work.fill(self.nodes.len(), None)?;
        let mut groups = work.fill(group_count, GroupOrder::default())?;
        for (i, node) in ordered.nodes.iter().enumerate() {
            work.price(1)?;
            keys[i] = Some(node.cross);
            let mut next = self.nodes[i].parent;
            while let Some(id) = next {
                work.price(1)?;
                let group = &mut groups[id - self.node_count];
                group.sum += node.cross;
                geometry(group.sum.is_finite())?;
                group.count += 1;
                group.last = Some(group.last.map_or(node.rank, |last| last.max(node.rank)));
                next = self.nodes[id].parent;
            }
        }
        for (i, group) in groups.iter().enumerate() {
            work.price(1)?;
            if group.count > 0 {
                keys[self.node_count + i] = Some(group.sum / group.count as f64);
            }
        }
        Ok(OrderHints {
            keys,
            groups,
            edge_slots,
            paths: ordered.edges,
            crossings: js_min(ordered.crossings, ordered.declared_crossings),
        })
    }

    fn build<E>(
        &self,
        id: usize,
        sized: bool,
        hints: Option<&OrderHints>,
        work: &mut Work<'_, E>,
    ) -> Result<OwnedPlan, E> {
        let level = &self.levels[id];
        let dir = level.direction;
        let start = dir.start();
        let node_cap = add(level.items.len(), level.ports.len())?;
        let edge_cap = add(level.edges.len(), level.ports.len())?;
        check_slots(node_cap)?;
        check_slots(edge_cap)?;
        let mut nodes = work.reserve(node_cap)?;
        let mut edges = work.reserve(edge_cap)?;
        for &item in &level.items {
            work.price(1)?;
            let node = &self.nodes[item];
            let size = if sized {
                node.size.in_axes(dir)
            } else {
                Size::default()
            };
            let span = if node.is_group {
                js_min(node.span, size.width - 2.0 * node.pad_x)
            } else {
                node.span
            };
            geometry(span.is_finite())?;
            nodes.push(LevelNode {
                size,
                span,
                key: hints.and_then(|h| h.keys[item]),
                ..LevelNode::default()
            });
        }
        for &ei in &level.edges {
            work.price(1)?;
            let projected = self.projection(ei)?;
            let [a, b] = projected.central;
            let edge = &self.edges[ei];
            let mut input = LevelEdge::new(self.local[a], self.local[b], edge.min_len);
            if sized {
                input.label = edge.label.in_axes(dir);
                input.tail_attachment = self.attachment(projected.outer_port(0), a, dir, true)?;
                input.head_attachment = self.attachment(projected.outer_port(1), b, dir, true)?;
            }
            edges.push(input);
        }
        for &pid in &level.ports {
            work.price(1)?;
            let port = &self.ports[pid];
            let inner = self.local[port.inner];
            if port.side.axis() == dir.axis() {
                let other = self.edges[port.edge].ends[1 - port.end];
                let kind = if port.side == start {
                    NodeKind::StartPort
                } else {
                    NodeKind::EndPort
                };
                let hint = match (port.hint, hints) {
                    (Some(value), _) => Some(value),
                    (None, Some(hints)) => self.far_hint(id, port, hints, work)?,
                    _ => None,
                };
                let dummy = nodes.len();
                nodes.push(LevelNode {
                    kind,
                    hint,
                    key: hints.and_then(|h| h.keys[other]),
                    ..LevelNode::default()
                });
                let attachment = self.attachment(port.child, port.inner, dir, sized)?;
                let edge = if port.end == 0 {
                    LevelEdge {
                        tail_attachment: attachment,
                        ..LevelEdge::new(inner, dummy, 1)
                    }
                } else {
                    LevelEdge {
                        head_attachment: attachment,
                        ..LevelEdge::new(dummy, inner, 1)
                    }
                };
                edges.push(edge);
            } else {
                nodes[inner].pin = if matches!(port.side, Side::Top | Side::Left) {
                    -1
                } else {
                    1
                };
            }
        }
        Ok(OwnedPlan { nodes, edges })
    }

    fn projection<E>(&self, edge: usize) -> Result<&Projection, E> {
        self.projections[edge]
            .as_ref()
            .ok_or(CompoundError::InconsistentLevel("missing edge projection"))
    }

    fn attachment<E>(
        &self,
        port: Option<usize>,
        item: usize,
        dir: Direction,
        sized: bool,
    ) -> Result<Attachment, E> {
        let Some(port) = port else {
            return Ok(Attachment::default());
        };
        let node = &self.nodes[item];
        let dx = if !sized {
            0.0
        } else if dir.vertical() {
            self.ports[port].contact.x - node.size.width / 2.0
        } else {
            self.ports[port].contact.y - node.size.height / 2.0
        };
        geometry(dx.is_finite())?;
        Ok(Attachment { dx, fixed: true })
    }

    fn far_hint<E>(
        &self,
        id: usize,
        port: &Port,
        hints: &OrderHints,
        work: &mut Work<'_, E>,
    ) -> Result<Option<f64>, E> {
        let level = &self.levels[id];
        if port.end != 0 || port.side != level.direction.start().opposite() {
            return Ok(None);
        }
        let (Some(group), Some(e)) = (level.group, hints.edge_slots[port.edge]) else {
            return Ok(None);
        };
        let last = hints.groups[group - self.node_count].last;
        let next = last.map(|last| add(last, 1)).transpose()?;
        let mut inside = hints.keys[self.edges[port.edge].ends[0]]
            .ok_or(CompoundError::InconsistentLevel("missing leaf key"))?;
        let mut outside = 0.0;
        work.price(hints.paths[e].len())?;
        for (i, knot) in hints.paths[e].iter().enumerate() {
            work.poll(i)?;
            if Some(knot.rank) == last {
                inside = knot.cross;
            }
            if Some(knot.rank) == next || (last.is_none() && knot.rank == 0) {
                outside = knot.cross;
            }
        }
        let hint = inside + outside * 1e-6;
        geometry(hint.is_finite())?;
        Ok(Some(hint))
    }
}

fn lift<E>(
    nodes: &[CNode],
    depth: &[usize],
    mut ends: [usize; 2],
    work: &mut Work<'_, E>,
) -> Result<([usize; 2], [usize; 2]), E> {
    let mut crossed = [0, 0];
    for end in 0..2 {
        while depth[ends[end]] > depth[ends[1 - end]] {
            work.price(1)?;
            ends[end] = nodes[ends[end]]
                .parent
                .ok_or(CompoundError::InconsistentLevel("missing ancestor"))?;
            crossed[end] += 1;
        }
    }
    while ends[0] != ends[1] && nodes[ends[0]].parent != nodes[ends[1]].parent {
        work.price(2)?;
        for end in 0..2 {
            ends[end] = nodes[ends[end]]
                .parent
                .ok_or(CompoundError::InconsistentLevel("missing common level"))?;
            crossed[end] += 1;
        }
    }
    Ok((ends, crossed))
}

#[derive(Clone, Copy, Default)]
struct GroupOrder {
    sum: f64,
    count: usize,
    last: Option<usize>,
}

struct OrderHints {
    keys: Vec<Option<f64>>,
    groups: Vec<GroupOrder>,
    edge_slots: Vec<Option<usize>>,
    paths: Vec<Vec<super::RankPoint>>,
    crossings: f64,
}

fn check_order<E>(
    ordered: &OrderedLevel,
    nodes: usize,
    edges: usize,
    work: &mut Work<'_, E>,
) -> Result<(), E> {
    if ordered.nodes.len() != nodes || ordered.edges.len() != edges {
        return Err(CompoundError::InconsistentLevel("order-only slot counts"));
    }
    geometry(
        ordered.crossings.is_finite()
            && ordered.crossings >= 0.0
            && ordered.declared_crossings.is_finite()
            && ordered.declared_crossings >= 0.0,
    )?;
    for n in &ordered.nodes {
        work.price(1)?;
        geometry(n.cross.is_finite())?;
        check_slots(n.rank)?;
    }
    for route in &ordered.edges {
        work.price(add(1, route.len())?)?;
        if route.len() < 2 {
            return Err(CompoundError::InconsistentLevel("short order-only route"));
        }
        let mut previous = None;
        for (i, n) in route.iter().enumerate() {
            work.poll(i)?;
            geometry(n.cross.is_finite())?;
            check_slots(n.rank)?;
            if previous.is_some_and(|rank| rank >= n.rank) {
                return Err(CompoundError::InconsistentLevel(
                    "order-only route rank order",
                ));
            }
            previous = Some(n.rank);
        }
    }
    Ok(())
}

struct OwnedPlan {
    nodes: Vec<LevelNode>,
    edges: Vec<LevelEdge>,
}

impl OwnedPlan {
    fn borrow(&self, options: LevelOptions) -> LevelPlan<'_> {
        LevelPlan {
            nodes: &self.nodes,
            edges: &self.edges,
            groups: None,
            options,
        }
    }

    fn same<E>(&self, other: &Self, work: &mut Work<'_, E>) -> Result<bool, E> {
        if self.nodes.len() != other.nodes.len() || self.edges.len() != other.edges.len() {
            return Ok(false);
        }
        let same = |a: f64, b: f64| a.to_bits() == b.to_bits();
        let size = |a: Size, b: Size| same(a.width, b.width) && same(a.height, b.height);
        let attachment = |a: Attachment, b: Attachment| a.fixed == b.fixed && same(a.dx, b.dx);
        for (a, b) in self.nodes.iter().zip(&other.nodes) {
            work.price(1)?;
            if !size(a.size, b.size)
                || a.kind != b.kind
                || !same(a.span, b.span)
                || a.pin != b.pin
                || a.key.map(f64::to_bits) != b.key.map(f64::to_bits)
                || a.hint.map(f64::to_bits) != b.hint.map(f64::to_bits)
            {
                return Ok(false);
            }
        }
        for (a, b) in self.edges.iter().zip(&other.edges) {
            work.price(1)?;
            if a.tail != b.tail
                || a.head != b.head
                || a.min_len != b.min_len
                || !size(a.label, b.label)
                || !attachment(a.tail_attachment, b.tail_attachment)
                || !attachment(a.head_attachment, b.head_attachment)
            {
                return Ok(false);
            }
        }
        Ok(true)
    }
}

struct CachedPlace {
    plan: OwnedPlan,
    layout: PlacedLevel,
}

impl Compound<'_> {
    fn place_levels<E>(
        &mut self,
        hints: Option<&OrderHints>,
        cache: &mut [Vec<CachedPlace>],
        work: &mut Work<'_, E>,
        solve: &mut Solve<'_, E>,
    ) -> Result<Size, E> {
        for i in 0..self.order.len() {
            work.price(1)?;
            self.orient_level(self.order[i], hints, work, solve)?;
        }
        let mut size = Size::default();
        for i in (0..self.order.len()).rev() {
            work.price(1)?;
            let id = self.order[i];
            let plan = self.build(id, true, hints, work)?;
            if let Some(past) = cache.get_mut(id) {
                let mut found = None;
                for item in past.iter() {
                    work.price(1)?;
                    if item.plan.same(&plan, work)? {
                        found = Some(&item.layout);
                        break;
                    }
                }
                size = if let Some(layout) = found {
                    self.project_level(id, layout, work)?
                } else {
                    let layout = solve_place(&plan, self.options, work, solve)?;
                    let size = self.project_level(id, &layout, work)?;
                    work.push(past, CachedPlace { plan, layout })?;
                    size
                };
            } else {
                let layout = solve_place(&plan, self.options, work, solve)?;
                size = self.project_level(id, &layout, work)?;
            }
        }
        self.translate_items(work)?;
        self.stitch(work)?;
        Ok(size)
    }

    fn orient_level<E>(
        &mut self,
        id: usize,
        hints: Option<&OrderHints>,
        work: &mut Work<'_, E>,
        solve: &mut Solve<'_, E>,
    ) -> Result<(), E> {
        let level = &self.levels[id];
        for &p in &level.ports {
            work.price(1)?;
            if let Some(child) = self.ports[p].child {
                self.ports[child].side = self.ports[p].side;
            }
        }
        let mut needed = false;
        for &e in &level.edges {
            work.price(1)?;
            let projection = self.projection(e)?;
            needed |= projection.paths.iter().any(|path| !path.is_empty());
        }
        if !needed {
            return Ok(());
        }
        let plan = self.build(id, false, hints, work)?;
        let LevelResult::Oriented(reversed) =
            work.solve(solve, plan.borrow(self.options), Mode::Orient)?
        else {
            return Err(CompoundError::InconsistentLevel("orientation result"));
        };
        if reversed.len() != plan.edges.len() {
            return Err(CompoundError::InconsistentLevel(
                "orientation edge slot count",
            ));
        }
        let start = level.direction.start();
        for (&ei, rev) in level.edges.iter().zip(reversed) {
            work.price(1)?;
            let projection = self.projection(ei)?;
            let ports = [projection.outer_port(0), projection.outer_port(1)];
            let tail = if rev { start } else { start.opposite() };
            for (port, side) in ports.into_iter().zip([tail, tail.opposite()]) {
                if let Some(port) = port {
                    self.ports[port].side = side;
                }
            }
        }
        Ok(())
    }

    fn project_level<E>(
        &mut self,
        id: usize,
        solved: &PlacedLevel,
        work: &mut Work<'_, E>,
    ) -> Result<Size, E> {
        let level = &self.levels[id];
        let group = level.group.map(|id| &self.nodes[id]);
        let transform = Transform::new(solved.size, level.direction, group)?;
        for (&item, &center) in level.items.iter().zip(&solved.centers) {
            work.price(1)?;
            self.nodes[item].center = transform.point(center)?;
        }
        for (&ei, solved_edge) in level.edges.iter().zip(&solved.edges) {
            work.price(1)?;
            let projection = self.projections[ei]
                .as_mut()
                .ok_or(CompoundError::InconsistentLevel("missing placed edge"))?;
            projection.route = transform.route(&solved_edge.points, 0, work)?;
            self.edges[ei].reversed_for_ranking = solved_edge.reversed_for_ranking;
            if self.edges[ei].label.width > 0.0 {
                self.edges[ei].label_at = transform.point(solved_edge.label)?;
            }
        }
        let mut slot = level.edges.len();
        for &pid in &level.ports {
            work.price(1)?;
            let port = &self.ports[pid];
            let out = port.end == 0;
            let side = port.side;
            let (contact, route) = if side.axis() == transform.dir.axis() {
                let path = solved
                    .edges
                    .get(slot)
                    .ok_or(CompoundError::InconsistentLevel("missing port route"))?;
                slot += 1;
                let mut route = transform.route(&path.points, 1, work)?;
                let point = if out { route.last() } else { route.first() };
                let point = point.ok_or(CompoundError::InconsistentLevel("empty port route"))?;
                let contact = transform.border(point.position(), side);
                let point = route_point(contact, side.axis());
                if out {
                    route.push(point);
                } else {
                    work.price(route.len())?;
                    route.insert(0, point);
                }
                (contact, route)
            } else {
                let item = &self.nodes[port.inner];
                let point = if let Some(child) = port.child {
                    let contact = self.ports[child].contact;
                    Point {
                        x: item.center.x + (contact.x - item.size.width / 2.0),
                        y: item.center.y + (contact.y - item.size.height / 2.0),
                    }
                } else {
                    face(item.center, item.size, side)
                };
                geometry(point.is_finite())?;
                let contact = transform.border(point, side);
                let knots = if out {
                    [point, contact]
                } else {
                    [contact, point]
                };
                let mut route = work.reserve(2)?;
                for point in knots {
                    route.push(route_point(point, side.axis()));
                }
                (contact, route)
            };
            self.ports[pid].contact = contact;
            self.ports[pid].route = route;
        }
        if slot != solved.edges.len() {
            return Err(CompoundError::InconsistentLevel("unused flat edge slot"));
        }
        if let Some(group) = level.group {
            self.nodes[group].size = transform.bounds;
        }
        Ok(transform.bounds)
    }

    fn translate_items<E>(&mut self, work: &mut Work<'_, E>) -> Result<(), E> {
        for &id in &self.order {
            work.price(1)?;
            let origin = self.levels[id].origin;
            for i in 0..self.levels[id].items.len() {
                work.price(1)?;
                let item = self.levels[id].items[i];
                let node = &mut self.nodes[item];
                node.center = node.center.translated(origin);
                geometry(node.center.is_finite())?;
                if node.is_group {
                    let child = level_of(Some(item), self.node_count);
                    self.levels[child].origin = Point {
                        x: node.center.x - node.size.width / 2.0,
                        y: node.center.y - node.size.height / 2.0,
                    };
                    geometry(self.levels[child].origin.is_finite())?;
                }
            }
        }
        Ok(())
    }

    fn stitch<E>(&mut self, work: &mut Work<'_, E>) -> Result<(), E> {
        let mut stitches = work.reserve(self.ports.len())?;
        for &id in &self.order {
            work.price(1)?;
            let level = &self.levels[id];
            for &ei in &level.edges {
                work.price(1)?;
                let projection = self.projection(ei)?;
                let mut route = Vec::new();
                for pid in projection.paths[0].clone() {
                    stitches.push(self.append_port(&mut route, pid, work)?);
                }
                append(&mut route, &projection.route, level.origin, None, work)?;
                for pid in projection.paths[1].clone().rev() {
                    stitches.push(self.append_port(&mut route, pid, work)?);
                }
                let edge = &mut self.edges[ei];
                edge.route = route;
                if edge.label.width > 0.0 {
                    edge.label_at = edge.label_at.translated(level.origin);
                    geometry(edge.label_at.is_finite())?;
                }
            }
        }
        work.sort_rows(&mut stitches, |a, b| {
            (a.edge, a.point).cmp(&(b.edge, b.point))
        })?;
        self.border_stitches = stitches;
        Ok(())
    }

    fn append_port<E>(
        &self,
        route: &mut Vec<RoutePoint>,
        port: usize,
        work: &mut Work<'_, E>,
    ) -> Result<BorderStitch, E> {
        let port = &self.ports[port];
        let level = level_of(self.nodes[port.inner].parent, self.node_count);
        let level = &self.levels[level];
        let group = level.group.ok_or(CompoundError::InconsistentLevel(
            "port attached to the root",
        ))?;
        let last = port
            .route
            .len()
            .checked_sub(1)
            .ok_or(CompoundError::InconsistentLevel("missing boundary route"))?;
        let contact = if port.end == 0 { last } else { 0 };
        let point = append(route, &port.route, level.origin, Some(contact), work)?
            .ok_or(CompoundError::InconsistentLevel("missing boundary contact"))?;
        Ok(BorderStitch {
            edge: port.edge,
            point,
            group: (group - self.node_count) as u32,
            side: port.side,
        })
    }

    fn crossings<E>(&self, work: &mut Work<'_, E>) -> Result<usize, E> {
        route_crossings(self.nodes, self.edges, self.options.routing, work)
    }

    fn refine_hints<E>(&mut self, work: &mut Work<'_, E>) -> Result<bool, E> {
        let mut again = false;
        let mut seen = Vec::new();
        for &id in &self.order[1..] {
            work.price(1)?;
            let level = &self.levels[id];
            let dir = level.direction;
            let end = dir.start().opposite();
            for &pid in &level.ports {
                work.price(1)?;
                let port = &mut self.ports[pid];
                if port.side != end {
                    continue;
                }
                let route = &self.edges[port.edge].route;
                let point = level.origin.translated(port.contact);
                geometry(point.is_finite())?;
                let hint = outside_hint(route, point, port.end == 0, dir, work)?;
                if let Some(value) = hint {
                    let hint = value - cross(level.origin, dir);
                    geometry(hint.is_finite())?;
                    port.hint = Some(hint);
                    let i = seen.len();
                    work.push(&mut seen, (cross(port.contact, dir), hint, i))?;
                }
            }
            work.sort_rows(&mut seen, |a, b| cmp(a.0, b.0).then(a.2.cmp(&b.2)))?;
            work.price(seen.len())?;
            for pair in seen.windows(2) {
                if pair[0].1 > pair[1].1 {
                    again = true;
                }
            }
            seen.clear();
        }
        Ok(again)
    }

    fn snapshot<E>(&mut self, size: Size, work: &mut Work<'_, E>) -> Result<Snapshot, E> {
        let mut nodes = work.reserve(self.nodes.len())?;
        let mut edges = work.reserve(self.edges.len())?;
        let mut origins = work.reserve(self.levels.len())?;
        let mut contacts = work.reserve(self.ports.len())?;
        for node in self.nodes.iter() {
            work.price(1)?;
            nodes.push((node.center, node.size));
        }
        for edge in self.edges.iter_mut() {
            work.price(1)?;
            edges.push((
                std::mem::take(&mut edge.route),
                edge.label_at,
                edge.reversed_for_ranking,
            ));
        }
        for level in &self.levels {
            work.price(1)?;
            origins.push(level.origin);
        }
        for port in &self.ports {
            work.price(1)?;
            contacts.push(port.contact);
        }
        Ok(Snapshot {
            nodes,
            edges,
            origins,
            contacts,
            size,
            border_stitches: std::mem::take(&mut self.border_stitches),
        })
    }

    fn restore<E>(&mut self, saved: Snapshot, work: &mut Work<'_, E>) -> Result<Size, E> {
        for (node, (center, size)) in self.nodes.iter_mut().zip(saved.nodes) {
            work.price(1)?;
            (node.center, node.size) = (center, size);
        }
        for (edge, (route, label, reversed)) in self.edges.iter_mut().zip(saved.edges) {
            work.price(1)?;
            (edge.route, edge.label_at) = (route, label);
            edge.reversed_for_ranking = reversed;
        }
        for (level, origin) in self.levels.iter_mut().zip(saved.origins) {
            work.price(1)?;
            level.origin = origin;
        }
        for (port, contact) in self.ports.iter_mut().zip(saved.contacts) {
            work.price(1)?;
            port.contact = contact;
        }
        self.border_stitches = saved.border_stitches;
        Ok(saved.size)
    }

    fn draw_strays<E>(&mut self, work: &mut Work<'_, E>) -> Result<(), E> {
        for (edge, projection) in self.edges.iter_mut().zip(&self.projections) {
            work.price(1)?;
            if projection.is_some() || edge.ends[0] == edge.ends[1] {
                continue;
            }
            let [a, b] = edge.ends.map(|id| self.nodes[id].center);
            edge.route = work.reserve(2)?;
            for point in [a, b] {
                edge.route.push(route_point(point, Axis::Vertical));
            }
            edge.label_at = Point {
                x: (a.x + b.x) / 2.0,
                y: (a.y + b.y) / 2.0,
            };
            geometry(edge.label_at.is_finite())?;
        }
        Ok(())
    }
}

fn solve_place<E>(
    plan: &OwnedPlan,
    options: LevelOptions,
    work: &mut Work<'_, E>,
    solve: &mut Solve<'_, E>,
) -> Result<PlacedLevel, E> {
    let LevelResult::Placed(solved) = work.solve(solve, plan.borrow(options), Mode::Place)? else {
        return Err(CompoundError::InconsistentLevel("place result"));
    };
    if solved.centers.len() != plan.nodes.len() || solved.edges.len() != plan.edges.len() {
        return Err(CompoundError::InconsistentLevel("place slot counts"));
    }
    geometry(solved.size.valid())?;
    Ok(solved)
}

struct Snapshot {
    nodes: Vec<(Point, Size)>,
    edges: Vec<(Vec<RoutePoint>, Point, bool)>,
    origins: Vec<Point>,
    contacts: Vec<Point>,
    size: Size,
    border_stitches: Vec<BorderStitch>,
}

struct Transform {
    dir: Direction,
    // Height in solver axes before translation and physical rotation.
    along: f64,
    offset: Point,
    bounds: Size,
}

impl Transform {
    fn new<E>(size: Size, dir: Direction, group: Option<&CNode>) -> Result<Self, E> {
        let content = size.in_axes(dir);
        let (pad_x, pad_top, pad_bottom, min_width) = group.map_or((0.0, 0.0, 0.0, 0.0), |g| {
            (g.pad_x, g.pad_top, g.pad_bottom, g.min_width)
        });
        let bounds = Size {
            width: js_max(content.width + 2.0 * pad_x, min_width),
            height: content.height + pad_top + pad_bottom,
        };
        geometry(bounds.valid())?;
        Ok(Self {
            dir,
            along: size.height,
            bounds,
            offset: Point {
                x: (bounds.width - content.width) / 2.0,
                y: pad_top,
            },
        })
    }

    fn point<E>(&self, p: Point) -> Result<Point, E> {
        let along = if self.dir.flipped() {
            self.along - p.y
        } else {
            p.y
        };
        let (x, y) = if self.dir.vertical() {
            (p.x, along)
        } else {
            (along, p.x)
        };
        let point = Point {
            x: self.offset.x + x,
            y: self.offset.y + y,
        };
        geometry(point.is_finite())?;
        Ok(point)
    }

    fn route<E>(
        &self,
        points: &[Point],
        extra: usize,
        work: &mut Work<'_, E>,
    ) -> Result<Vec<RoutePoint>, E> {
        if points.len() < 2 {
            return Err(CompoundError::InconsistentLevel("short flat route"));
        }
        let mut route = work.reserve(add(points.len(), extra)?)?;
        work.price(points.len())?;
        for (i, &p) in points.iter().enumerate() {
            work.poll(i)?;
            route.push(route_point(self.point(p)?, self.dir.axis()));
        }
        Ok(route)
    }

    fn border(&self, mut point: Point, side: Side) -> Point {
        match side {
            Side::Top => point.y = 0.0,
            Side::Right => point.x = self.bounds.width,
            Side::Bottom => point.y = self.bounds.height,
            Side::Left => point.x = 0.0,
        }
        point
    }
}

fn face(mut point: Point, size: Size, side: Side) -> Point {
    match side {
        Side::Top => point.y -= size.height / 2.0,
        Side::Right => point.x += size.width / 2.0,
        Side::Bottom => point.y += size.height / 2.0,
        Side::Left => point.x -= size.width / 2.0,
    }
    point
}

fn append<E>(
    route: &mut Vec<RoutePoint>,
    part: &[RoutePoint],
    origin: Point,
    contact: Option<usize>,
    work: &mut Work<'_, E>,
) -> Result<Option<usize>, E> {
    let mut contact_index = None;
    work.price(part.len())?;
    for (i, knot) in part.iter().enumerate() {
        work.poll(i)?;
        let point = knot.position().translated(origin);
        geometry(point.is_finite())?;
        if !route
            .last()
            .is_some_and(|last| matches_point(last.position(), point))
        {
            work.push(route, route_point(point, knot.axis))?;
        }
        if contact == Some(i) {
            contact_index = route.len().checked_sub(1);
        }
    }
    Ok(contact_index)
}

fn matches_point(a: Point, b: Point) -> bool {
    (a.x - b.x).abs() < JOIN_TOLERANCE && (a.y - b.y).abs() < JOIN_TOLERANCE
}

fn cross(point: Point, dir: Direction) -> f64 {
    if dir.vertical() { point.x } else { point.y }
}

fn outside_hint<E>(
    route: &[RoutePoint],
    at: Point,
    out: bool,
    dir: Direction,
    work: &mut Work<'_, E>,
) -> Result<Option<f64>, E> {
    let mut indices = 0..route.len();
    // The source selects the last touch outbound and the first touch inbound.
    while let Some(i) = if out {
        indices.next_back()
    } else {
        indices.next()
    } {
        work.price(1)?;
        if !matches_point(route[i].position(), at) {
            continue;
        }
        let at = cross(route[i].position(), dir);
        let mut tail = if out { i + 1..route.len() } else { 0..i };
        while let Some(j) = if out { tail.next() } else { tail.next_back() } {
            work.price(1)?;
            let next = cross(route[j].position(), dir);
            if (next - at).abs() >= IN_LINE {
                return Ok(Some(next));
            }
        }
        return Ok(None);
    }
    Ok(None)
}
