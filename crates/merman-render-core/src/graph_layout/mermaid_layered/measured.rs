//! The measured input contract and the physical projection from layered/index.ts.

use super::super::{Endpoint, MeasuredGraph, Rect, SolvedEdge, SolvedGraph, SolvedGroup};
use super::compound::{self, CEdge, CNode};
use super::inner::InnerEdge;
use super::work::{Work, add, check_slots, geometry, js_max};
use super::{
    AxisExt, Charge, CompoundError, Direction, DirectionExt, InputError, LayeredLayout,
    LevelOptions, Options, Owner, Point, PointExt, Result, RoutePoint, RoutePointExt, Side,
    SideExt, Size, SizeExt, Solve, Spacing, route_point,
};
use crate::LayoutLabel;

pub(super) const PORT_SPACING: f64 = 20.0;
pub(super) const CORNER: f64 = 12.0;
const LOOP_REACH: f64 = 26.0;
const LOOP_GAP: f64 = 6.0;
const TURN_BREAKPOINT: f64 = 640.0;
const MAX_MIN_LEN: u32 = 1000;

#[derive(Clone, Copy)]
pub(super) struct PlacedBox {
    pub(super) center: Point,
    pub(super) size: Size,
}

/// LayoutGraph is already measured. Family admission, contour clipping and rendering stay outside.
pub fn layout<E>(
    graph: &MeasuredGraph,
    options: Options,
    charge: &mut Charge<'_, E>,
    solve: &mut Solve<'_, E>,
) -> Result<LayeredLayout, E> {
    let mut work = Work::new(charge);
    work.price(0)?;
    let validated = Validated::new(graph, options, &mut work)?;
    let across = tighten(
        graph,
        options,
        &validated,
        graph.direction,
        &mut work,
        solve,
    )?;
    let best = match options.available_width {
        Some(width)
            if width > 0.0
                && across.graph.bounds.width > width
                && !graph.direction.vertical()
                && width < TURN_BREAKPOINT =>
        {
            let down = tighten(
                graph,
                options,
                &validated,
                Direction::Down,
                &mut work,
                solve,
            )?;
            if down.graph.bounds.width < across.graph.bounds.width {
                down
            } else {
                across
            }
        }
        _ => across,
    };
    work.price(0)?;
    Ok(best)
}

fn tighten<E>(
    graph: &MeasuredGraph,
    options: Options,
    validated: &Validated,
    direction: Direction,
    work: &mut Work<'_, E>,
    solve: &mut Solve<'_, E>,
) -> Result<LayeredLayout, E> {
    let loose = run(graph, options, validated, direction, false, work, solve)?;
    if options
        .available_width
        .is_some_and(|width| width > 0.0 && loose.graph.bounds.width > width)
    {
        let tight = run(graph, options, validated, direction, true, work, solve)?;
        if tight.graph.bounds.width < loose.graph.bounds.width {
            return Ok(tight);
        }
    }
    Ok(loose)
}

struct Validated {
    parents: Vec<Option<usize>>,
    ends: Vec<[usize; 2]>,
}

impl Validated {
    fn new<E>(graph: &MeasuredGraph, options: Options, work: &mut Work<'_, E>) -> Result<Self, E> {
        let node_count = graph.nodes.len();
        let group_count = graph.groups.len();
        let count = add(node_count, group_count)?;
        check_slots(count)?;
        check_slots(graph.edges.len())?;
        let mut parents = work.reserve(count)?;
        let parent = |slot: Option<u32>| {
            slot.map(|id| id as usize)
                .filter(|&id| id < group_count)
                .map(|id| node_count + id)
        };
        for (slot, node) in graph.nodes.iter().enumerate() {
            work.price(1)?;
            let owner = Owner::Node(slot);
            finite(node.size.width, owner, "width")?;
            finite(node.size.height, owner, "height")?;
            if let Some(span) = node.port_span {
                finite(span, owner, "span")?;
            }
            parents.push(parent(node.parent));
        }
        for (slot, group) in graph.groups.iter().enumerate() {
            work.price(1)?;
            let owner = Owner::Group(slot);
            for pad in [
                group.padding.y0,
                group.padding.x1,
                group.padding.y1,
                group.padding.x0,
            ] {
                finite(pad, owner, "padding")?;
            }
            if let Some(title) = group.title {
                finite(title.width, owner, "title width")?;
                finite(title.height, owner, "title height")?;
            }
            parents.push(parent(group.parent));
        }
        let endpoint = |end| match end {
            Endpoint::Node(id) if (id as usize) < node_count => Some(id as usize),
            Endpoint::Group(id) if (id as usize) < group_count => Some(node_count + id as usize),
            _ => None,
        };
        let mut ends = work.reserve(graph.edges.len())?;
        for (slot, edge) in graph.edges.iter().enumerate() {
            work.price(1)?;
            let source = endpoint(edge.source)
                .ok_or(CompoundError::InvalidInput(InputError::UnknownSource(slot)))?;
            let target = endpoint(edge.target)
                .ok_or(CompoundError::InvalidInput(InputError::UnknownTarget(slot)))?;
            if let Some(label) = edge.label {
                finite(label.width, Owner::Edge(slot), "label width")?;
                finite(label.height, Owner::Edge(slot), "label height")?;
            }
            ends.push([source, target]);
        }
        for (name, value) in [
            ("spacing.node", graph.node_spacing),
            ("spacing.rank", graph.rank_spacing),
            ("spacing.edge", options.edge_spacing),
            ("diagramPadding", options.diagram_padding),
            ("headRoom", options.head_room),
        ] {
            finite(value, Owner::Graph, name)?;
        }
        Ok(Self { parents, ends })
    }
}

fn finite<E>(value: f64, owner: Owner, field: &'static str) -> Result<(), E> {
    if value.is_finite() && value >= 0.0 {
        Ok(())
    } else {
        Err(CompoundError::InvalidInput(InputError::InvalidNumber {
            owner,
            field,
        }))
    }
}

struct Ancestors<'a> {
    parents: &'a [Option<usize>],
    visited: Vec<usize>,
    epoch: usize,
}

impl<'a> Ancestors<'a> {
    fn new<E>(parents: &'a [Option<usize>], work: &mut Work<'_, E>) -> Result<Self, E> {
        Ok(Self {
            parents,
            visited: work.fill(parents.len(), 0)?,
            epoch: 0,
        })
    }

    /// Matches first-visit order even for self-parent and mutually nested group inputs.
    fn find<E>(
        &mut self,
        start: Option<usize>,
        work: &mut Work<'_, E>,
        mut predicate: impl FnMut(usize, &mut Work<'_, E>) -> Result<bool, E>,
    ) -> Result<Option<usize>, E> {
        let mut next = start;
        self.epoch = add(self.epoch, 1)?;
        while let Some(slot) = next {
            work.price(1)?;
            if self.visited[slot] == self.epoch {
                break;
            }
            self.visited[slot] = self.epoch;
            if predicate(slot, work)? {
                return Ok(Some(slot));
            }
            next = self.parents[slot];
        }
        Ok(None)
    }

    fn inherited<E>(
        &mut self,
        nodes: &mut [CNode],
        ends: &[[usize; 2]],
        work: &mut Work<'_, E>,
    ) -> Result<(), E> {
        let mut from = Vec::new();
        let mut seen_from = work.fill(nodes.len(), false)?;
        for &[src, dst] in ends {
            work.price(1)?;
            let (a, b) = (self.parents[src], self.parents[dst]);
            if src == dst || a == b {
                continue;
            }
            self.find(a, work, |slot, work| {
                seen_from[slot] = true;
                work.push(&mut from, slot)?;
                Ok(false)
            })?;
            self.find(b, work, |slot, _| {
                if !std::mem::take(&mut seen_from[slot]) {
                    nodes[slot].direction = None;
                }
                Ok(false)
            })?;
            work.price(from.len())?;
            for (i, slot) in from.drain(..).enumerate() {
                work.poll(i)?;
                if std::mem::take(&mut seen_from[slot]) {
                    nodes[slot].direction = None;
                }
            }
        }
        Ok(())
    }

    fn holds<E>(
        &mut self,
        group: usize,
        member: usize,
        nodes: &[CNode],
        work: &mut Work<'_, E>,
    ) -> Result<bool, E> {
        Ok(group != member
            && nodes[group].is_group
            && self
                .find(self.parents[member], work, |slot, _| Ok(slot == group))?
                .is_some())
    }

    fn direction<E>(
        &mut self,
        start: Option<usize>,
        nodes: &[CNode],
        root: Direction,
        work: &mut Work<'_, E>,
    ) -> Result<Direction, E> {
        let group = self.find(start, work, |slot, _| Ok(nodes[slot].direction.is_some()))?;
        Ok(group.and_then(|slot| nodes[slot].direction).unwrap_or(root))
    }
}

#[derive(Clone, Copy, Default)]
struct LoopSlots {
    count: u16,
    reach: f64,
}

fn run<E>(
    graph: &MeasuredGraph,
    options: Options,
    validated: &Validated,
    direction: Direction,
    tight: bool,
    work: &mut Work<'_, E>,
    solve: &mut Solve<'_, E>,
) -> Result<LayeredLayout, E> {
    let node_count = graph.nodes.len();
    let scale = if tight { 0.6 } else { 1.0 };
    let mut nodes = work.reserve(validated.parents.len())?;
    for (i, node) in graph.nodes.iter().enumerate() {
        work.price(1)?;
        nodes.push(CNode::new(i, validated.parents[i], node.size));
    }
    for (j, group) in graph.groups.iter().enumerate() {
        work.price(1)?;
        let i = node_count + j;
        let mut node = CNode::new(i, validated.parents[i], Size::default());
        let title = group.title.unwrap_or_default();
        node.is_group = true;
        node.direction = group.direction;
        node.pad_x = js_max(group.padding.x0, group.padding.x1) * scale;
        node.pad_top = group.padding.y0 * scale + title.height;
        node.pad_bottom = group.padding.y1 * scale;
        node.min_width = title.width + 2.0 * node.pad_x;
        geometry(node.pad_top.is_finite() && node.min_width.is_finite())?;
        node.span = f64::INFINITY;
        nodes.push(node);
    }

    let mut ancestors = Ancestors::new(&validated.parents, work)?;
    ancestors.inherited(&mut nodes, &validated.ends, work)?;
    let head_room = options.head_room;
    let mut inner_edges = work.reserve(graph.edges.len())?;
    for (k, &[src, dst]) in validated.ends.iter().enumerate() {
        work.price(1)?;
        let out = ancestors.holds(src, dst, &nodes, work)?;
        let inner = if out || ancestors.holds(dst, src, &nodes, work)? {
            let (group, member) = if out { (src, dst) } else { (dst, src) };
            let dir = ancestors.direction(Some(group), &nodes, direction, work)?;
            Some(InnerEdge::new(
                [group, member],
                out,
                dir,
                &nodes[group],
                &graph.groups[group - node_count],
                graph.edges[k].label,
                scale,
            ))
        } else {
            None
        };
        inner_edges.push(inner);
    }
    for edge in &inner_edges {
        work.price(1)?;
        if let Some(edge) = edge {
            edge.reserve(&mut nodes[edge.group], head_room)?;
        }
    }

    let mut sideways = work.reserve(node_count)?;
    for (i, node) in graph.nodes.iter().enumerate() {
        work.price(1)?;
        let inherited = ancestors.direction(validated.parents[i], &nodes, direction, work)?;
        nodes[i].span = node
            .port_span
            .unwrap_or_else(|| js_max(0.0, node.size.in_axes(inherited).width - 2.0 * CORNER));
        sideways.push(!inherited.vertical());
    }
    let mut loop_count = work.fill(node_count, LoopSlots::default())?;
    let mut loop_slots = work.reserve(graph.edges.len())?;
    let mut edges = work.reserve(graph.edges.len())?;
    for (k, edge) in graph.edges.iter().enumerate() {
        work.price(1)?;
        let [src, dst] = validated.ends[k];
        let label = edge.label.unwrap_or_default();
        let mut slot = LoopSlots::default();
        if src == dst && src < node_count {
            let label_across = if sideways[src] {
                label.height
            } else {
                label.width
            };
            let grow = LOOP_REACH
                + if label_across > 0.0 {
                    label_across + LOOP_GAP
                } else {
                    0.0
                };
            let size = &mut nodes[src].size;
            let across = if sideways[src] {
                &mut size.height
            } else {
                &mut size.width
            };
            *across += 2.0 * grow;
            slot = loop_count[src];
            loop_count[src].count = slot.count.wrapping_add(1);
            loop_count[src].reach += grow;
            geometry(size.valid() && loop_count[src].reach.is_finite())?;
        }
        loop_slots.push(slot);
        edges.push(CEdge::new(
            [src, dst],
            edge.min_rank_span.clamp(1, MAX_MIN_LEN),
            label,
        ));
    }

    let level_options = LevelOptions {
        spacing: Spacing {
            node: graph.node_spacing * if tight { 0.5 } else { 1.0 },
            rank: graph.rank_spacing * if tight { 0.75 } else { 1.0 },
            edge: options.edge_spacing,
        },
        port_spacing: PORT_SPACING,
        head_room,
        routing: graph.routing,
    };
    let placed = compound::place(
        &mut nodes,
        &mut edges,
        node_count,
        direction,
        level_options,
        work,
        solve,
    )?;
    let pad = options.diagram_padding;
    let offset = Point { x: pad, y: pad };
    let size = Size {
        width: placed.size.width + 2.0 * pad,
        height: placed.size.height + 2.0 * pad,
    };
    geometry(size.valid())?;
    let mut boxes = work.reserve(nodes.len())?;
    let mut groups = work.reserve(graph.groups.len())?;
    for (i, node) in graph.nodes.iter().enumerate() {
        work.price(1)?;
        boxes.push(placed_box(&nodes[i], node.size, offset)?);
    }
    for (j, group) in graph.groups.iter().enumerate() {
        work.price(1)?;
        let c = &nodes[node_count + j];
        let bounds = placed_box(c, c.size, offset)?;
        let title = Point {
            x: bounds.center.x,
            y: bounds.center.y - c.size.height / 2.0
                + group.padding.y0 * scale / 2.0
                + group.title.unwrap_or_default().height / 2.0,
        };
        geometry(title.is_finite())?;
        groups.push(SolvedGroup {
            rect: rect(bounds)?,
            title: placed_label(group.title, Some(title)),
            parent: c.parent.map(|p| (p - node_count) as u32),
            title_struck: false,
            direction: placed.group_directions[j],
        });
        boxes.push(bounds);
    }

    let box_at = |i: usize| boxes[i];
    let mut output = work.fill_with(edges.len(), || None::<SolvedEdge>)?;
    let mut edge_order = work.reserve(edges.len())?;
    let mut deferred = Vec::new();
    for (i, mut edge) in edges.into_iter().enumerate() {
        work.price(1)?;
        let [src, dst] = validated.ends[i];
        let projected_ends = edge.ends.map(|e| endpoint(e, node_count));
        if let Some(inner) = inner_edges[i] {
            work.push(&mut deferred, (i, inner))?;
            continue;
        }
        let (points, label, ports) = if src == dst {
            let below = sideways.get(src).copied().unwrap_or(false);
            let count = loop_count.get(src).map_or(1, |slots| slots.count.max(1));
            self_loop(
                box_at(src),
                below,
                graph.edges[i].label,
                loop_slots[i],
                count,
                work,
            )?
        } else {
            work.price(edge.route.len())?;
            for (k, point) in edge.route.iter_mut().enumerate() {
                work.poll(k)?;
                let translated = point.position().translated(offset);
                geometry(translated.is_finite())?;
                *point = route_point(translated, point.axis);
            }
            let ports = [
                side_of(
                    box_at(src),
                    edge.route.first(),
                    direction.start().opposite(),
                ),
                side_of(box_at(dst), edge.route.last(), direction.start()),
            ];
            let label = (edge.label.width > 0.0 || edge.label.height > 0.0)
                .then(|| edge.label_at.translated(offset));
            (edge.route, label, ports)
        };
        geometry(label.is_none_or(|point| point.is_finite()))?;
        output[i] = Some(SolvedEdge {
            source: projected_ends[0],
            target: projected_ends[1],
            points,
            label: placed_label(graph.edges[i].label, label),
            ports: (ports[0], ports[1]),
            start_label: None,
            end_label: None,
            corridor_clearance: 0.0,
            reversed_for_ranking: edge.reversed_for_ranking,
        });
        edge_order.push(i);
    }
    for (i, inner) in deferred {
        work.price(1)?;
        let (points, label, ports) = inner.layout(
            box_at(inner.group),
            box_at(inner.member),
            graph.edges[i].label,
            &edge_order,
            |slot| output[slot].as_ref().map(|e| e.points.as_slice()),
            work,
        )?;
        output[i] = Some(SolvedEdge {
            source: graph.edges[i].source,
            target: graph.edges[i].target,
            points,
            label: placed_label(graph.edges[i].label, label),
            ports: (ports[0], ports[1]),
            start_label: None,
            end_label: None,
            corridor_clearance: 0.0,
            reversed_for_ranking: false,
        });
        edge_order.push(i);
    }
    let mut edges = work.reserve(output.len())?;
    work.price(output.len())?;
    for (i, edge) in output.into_iter().enumerate() {
        work.poll(i)?;
        edges.push(edge.ok_or(CompoundError::InconsistentLevel("missing original edge"))?);
    }
    let mut node_rects = work.reserve(node_count)?;
    work.price(node_count)?;
    for (i, &bounds) in boxes[..node_count].iter().enumerate() {
        work.poll(i)?;
        node_rects.push(rect(bounds)?);
    }
    Ok(LayeredLayout {
        graph: SolvedGraph {
            bounds: size,
            direction,
            nodes: node_rects,
            // Compound levels have local ranks in different axes, so there is no global rank table.
            node_ranks: Vec::new(),
            groups,
            edges,
            border_stitches: placed.border_stitches,
        },
        edge_order,
    })
}

fn endpoint(index: usize, node_count: usize) -> Endpoint {
    if index < node_count {
        Endpoint::Node(index as u32)
    } else {
        Endpoint::Group((index - node_count) as u32)
    }
}

fn rect<E>(bounds: PlacedBox) -> Result<Rect, E> {
    let Size { width, height } = bounds.size;
    let x = bounds.center.x - width / 2.0;
    let y = bounds.center.y - height / 2.0;
    geometry(x.is_finite() && y.is_finite())?;
    Ok(Rect {
        x,
        y,
        width,
        height,
    })
}

fn placed_label(size: Option<Size>, center: Option<Point>) -> Option<LayoutLabel> {
    let (size, center) = size.zip(center)?;
    Some(LayoutLabel {
        x: center.x,
        y: center.y,
        width: size.width,
        height: size.height,
    })
}

fn placed_box<E>(node: &CNode, size: Size, offset: Point) -> Result<PlacedBox, E> {
    let center = node.center.translated(offset);
    geometry(center.is_finite() && size.valid())?;
    Ok(PlacedBox { center, size })
}

fn side_of(bounds: PlacedBox, point: Option<&RoutePoint>, fallback: Side) -> Side {
    let Some(point) = point else {
        return fallback;
    };
    let dx = point.position().x - bounds.center.x;
    let dy = point.position().y - bounds.center.y;
    let rx = if bounds.size.width > 0.0 {
        dx.abs() / (bounds.size.width / 2.0)
    } else {
        dx.abs()
    };
    let ry = if bounds.size.height > 0.0 {
        dy.abs() / (bounds.size.height / 2.0)
    } else {
        dy.abs()
    };
    if rx < 1e-6 && ry < 1e-6 {
        fallback
    } else if ry >= rx {
        if dy < 0.0 { Side::Top } else { Side::Bottom }
    } else if dx < 0.0 {
        Side::Left
    } else {
        Side::Right
    }
}

pub(crate) type EdgePaint = (Vec<RoutePoint>, Option<Point>, [Side; 2]);

fn self_loop<E>(
    b: PlacedBox,
    below: bool,
    label: Option<Size>,
    slot: LoopSlots,
    count: u16,
    work: &mut Work<'_, E>,
) -> Result<EdgePaint, E> {
    let spread = (f64::from(slot.count) + 1.0) / (f64::from(count) + 1.0) / 2.0;
    let Point { x, y } = b.center;
    let Size { width, height } = b.size;
    let mut points = work.reserve(4)?;
    let (bracket, label, side) = if below {
        let face = y + height / 2.0;
        let out = face + slot.reach + LOOP_REACH;
        let x0 = x - width * spread;
        let x1 = x + width * spread;
        (
            [(x0, face), (x0, out), (x1, out), (x1, face)],
            label.map(|l| Point {
                x,
                y: out + LOOP_GAP + l.height / 2.0,
            }),
            Side::Bottom,
        )
    } else {
        let face = x + width / 2.0;
        let out = face + slot.reach + LOOP_REACH;
        let y0 = y - height * spread;
        let y1 = y + height * spread;
        (
            [(face, y0), (out, y0), (out, y1), (face, y1)],
            label.map(|l| Point {
                x: out + LOOP_GAP + l.width / 2.0,
                y,
            }),
            Side::Right,
        )
    };
    for (i, (x, y)) in bracket.into_iter().enumerate() {
        let point = Point { x, y };
        geometry(point.is_finite())?;
        let axis = if i == 2 {
            side.axis().perpendicular()
        } else {
            side.axis()
        };
        points.push(route_point(point, axis));
    }
    Ok((points, label, [side; 2]))
}
