//! Component-local topology and geometry in the canonical top-to-bottom coordinate space.

mod acyclic;
mod boundary;
mod bus;
mod component_cache;
mod crossings;
mod expand;
mod extras;
mod fan;
mod hub;
pub(crate) mod lanes;
pub(crate) use extras::BoundaryRoutes;
mod label_place;
mod order;
mod pack;
mod position;
mod rank;
mod refine;
mod routing;
mod validate;

use crate::work::{LayoutResult, Work, filled};
use crate::{Direction, Graph, LayoutError, NodeSpec, Options, Size, WorkError};

pub(crate) use extras::route_corridor as corridor;
pub(crate) use validate::{
    length, measured, validate_edge, validate_node, validate_options, validate_port,
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum BoundaryRank {
    Start,
    End,
}

/// Orientation is independent of the edge's semantic direction and input occurrence.
#[derive(Clone, Copy, Debug)]
struct Link {
    from: u32,
    to: u32,
    edge: u32,
    span: u32,
    weight: f32,
}

impl Link {
    fn flipped(self) -> Self {
        Self {
            from: self.to,
            to: self.from,
            ..self
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Kind {
    Node,
    Dummy,
    Label,
    StartPort,
    EndPort,
}

impl Kind {
    fn interior(self) -> bool {
        matches!(self, Self::Dummy | Self::Label)
    }
}

#[derive(Debug, Default)]
struct Csr {
    start: Vec<u32>,
    entries: Vec<u32>,
}

impl Csr {
    fn new(
        n: usize,
        endpoints: impl Iterator<Item = u32> + Clone,
        work: &mut Work<'_>,
    ) -> LayoutResult<Self> {
        let mut result = Self::default();
        result.rebuild(n, endpoints, work)?;
        Ok(result)
    }

    fn rebuild(
        &mut self,
        n: usize,
        endpoints: impl Iterator<Item = u32> + Clone,
        work: &mut Work<'_>,
    ) -> LayoutResult<()> {
        let count = n.checked_add(1).ok_or(WorkError::ArithmeticOverflow)?;
        work.charge(count)?;
        self.start.clear();
        self.start
            .try_reserve_exact(count)
            .map_err(|_| LayoutError::AllocationLimit)?;
        self.start.resize(count, 0);
        for endpoint in endpoints.clone() {
            work.visit(1)?;
            let endpoint = endpoint as usize;
            if endpoint >= n {
                return Err(LayoutError::InvalidInput("invalid adjacency endpoint"));
            }
            let bin = &mut self.start[endpoint + 1];
            *bin = bin.checked_add(1).ok_or(WorkError::ArithmeticOverflow)?;
        }
        for i in 1..count {
            self.start[i] = self.start[i]
                .checked_add(self.start[i - 1])
                .ok_or(WorkError::ArithmeticOverflow)?;
        }
        let len = self.start[n] as usize;
        work.charge(len)?;
        self.entries.clear();
        self.entries
            .try_reserve_exact(len)
            .map_err(|_| LayoutError::AllocationLimit)?;
        self.entries.resize(len, 0);
        // The starts double as fill cursors; restore the prefix array in place afterwards.
        for (index, endpoint) in endpoints.enumerate() {
            let cursor = &mut self.start[endpoint as usize];
            self.entries[*cursor as usize] = index as u32;
            *cursor += 1;
        }
        self.start.copy_within(..n, 1);
        self.start[0] = 0;
        Ok(())
    }

    fn at(&self, node: u32) -> &[u32] {
        let i = node as usize;
        &self.entries[self.start[i] as usize..self.start[i + 1] as usize]
    }
}

#[derive(Debug, Default)]
pub(crate) struct Expanded {
    rank: Vec<u32>,
    logical_rank: Vec<u64>,
    /// Tiers inserted between previously adjacent logical ranks.
    inserted_ranks: Vec<u32>,
    layers: Vec<Vec<u32>>,
    width: Vec<f64>,
    height: Vec<f64>,
    kind: Vec<Kind>,
    keep: Vec<bool>,
    from: Vec<u32>,
    to: Vec<u32>,
    weight: Vec<f32>,
    source_offset: Vec<f64>,
    target_offset: Vec<f64>,
    spine_ports: position::SpinePorts,
    chains: Vec<Vec<u32>>,
    label_vertex: Vec<Option<u32>>,
    inline_labels: Vec<label_place::InlineLabel>,
    direct: Vec<bool>,
    exterior_lanes: Vec<order::ExteriorLane>,
    separated_proxies: Vec<(u32, u32)>,
    pred: Csr,
    succ: Csr,
    order_scratch: order::OrderScratch,
    position_scratch: position::PositionScratch,
    pub(crate) component_cache: component_cache::ComponentCache,
}

struct Loop {
    edge: u32,
    node: u32,
    right: bool,
    /// Outer edge of this loop's reserved band, measured from the true node face.
    reach: f64,
}

#[derive(Default)]
struct Component {
    ids: Vec<u32>,
    nodes: Vec<NodeSpec>,
    links: Vec<Link>,
    loops: Vec<Loop>,
    margin_left: Vec<f64>,
    margin_right: Vec<f64>,
    boundaries: bool,
    pinned: bool,
    group_proxies: bool,
}

fn canonical_size(dir: Direction, size: Size) -> Size {
    match dir {
        Direction::Down | Direction::Up => size,
        Direction::Right | Direction::Left => Size {
            width: size.height,
            height: size.width,
        },
    }
}

#[cfg(test)]
mod tests;

/// Footprint of owned allocations, including the individual paths and ranks.
impl Expanded {
    fn separate_ports(&self, ends: (u32, u32), work: &mut Work<'_>) -> LayoutResult<bool> {
        if self.separated_proxies.is_empty() {
            return Ok(false);
        }
        work.visit(self.separated_proxies.len().ilog2() as usize + 1)?;
        Ok(self.separated_proxies.binary_search(&ends).is_ok())
    }

    pub(crate) fn order_ranks(&self) -> &[u32] {
        &self.rank
    }

    pub(crate) fn order_chains(&self) -> &[Vec<u32>] {
        &self.chains
    }

    pub(super) fn retained_bytes(&self) -> usize {
        fn bytes<T>(v: &Vec<T>) -> usize {
            v.capacity() * size_of::<T>()
        }
        let nested = |v: &Vec<Vec<u32>>| bytes(v) + v.iter().map(bytes).sum::<usize>();
        let csr = |c: &Csr| bytes(&c.start) + bytes(&c.entries);
        bytes(&self.rank)
            + bytes(&self.logical_rank)
            + bytes(&self.inserted_ranks)
            + nested(&self.layers)
            + bytes(&self.width)
            + bytes(&self.height)
            + bytes(&self.kind)
            + bytes(&self.keep)
            + bytes(&self.from)
            + bytes(&self.to)
            + bytes(&self.weight)
            + bytes(&self.source_offset)
            + bytes(&self.target_offset)
            + self.spine_ports.retained_bytes()
            + nested(&self.chains)
            + bytes(&self.label_vertex)
            + bytes(&self.inline_labels)
            + bytes(&self.direct)
            + bytes(&self.exterior_lanes)
            + bytes(&self.separated_proxies)
            + csr(&self.pred)
            + csr(&self.succ)
            + self.order_scratch.retained_bytes()
            + self.position_scratch.retained_bytes()
    }
}

fn expand_component(
    c: &Component,
    graph: &Graph,
    options: &Options,
    work: &mut Work<'_>,
    expanded: &mut Expanded,
) -> LayoutResult<acyclic::Orientation> {
    let orientation = boundary::orient(&c.nodes, &c.links, c.boundaries, options, work)?;
    expand::build(c, &orientation, graph, options, &[], work, expanded)?;
    Ok(orientation)
}

fn prepare_component(
    c: &Component,
    graph: &Graph,
    options: &Options,
    work: &mut Work<'_>,
    expanded: &mut Expanded,
) -> LayoutResult<(acyclic::Orientation, u64)> {
    let orientation = expand_component(c, graph, options, work, expanded)?;
    let crossings = order_component(expanded, c, &orientation, options, work)?;
    Ok((orientation, crossings))
}

fn order_component(
    expanded: &mut Expanded,
    c: &Component,
    orientation: &acyclic::Orientation,
    options: &Options,
    work: &mut Work<'_>,
) -> LayoutResult<u64> {
    expanded.exterior_lanes.clear();
    let mut crossings = if c.boundaries || c.pinned {
        order::constrained(expanded, c, None, options, work)?
    } else {
        order::reduce(expanded, &c.nodes, options, work)?
    };
    if options.cycle_policy == crate::CyclePolicy::HybridAdaptive
        && !c.boundaries
        && !c.pinned
        && !c.group_proxies
    {
        crossings = order::reserve_exterior_lanes(expanded, c, orientation, crossings, work)?;
    }
    Ok(crossings)
}

pub(crate) fn compound_order(
    graph: &Graph,
    groups: &[Option<crate::GroupId>],
    parents: &[Option<crate::GroupId>],
    options: &Options,
    expanded: &mut Expanded,
    work: &mut Work<'_>,
) -> LayoutResult<(Vec<f64>, u64)> {
    if groups.len() != graph.nodes.len() {
        return Err(LayoutError::InvalidInput(
            "incomplete order group membership",
        ));
    }
    let c = validate::collect(graph, options, true, work)?
        .pop()
        .unwrap_or_default();
    if !c.loops.is_empty() {
        return Err(LayoutError::InvalidInput(
            "order proxy requires loop-free edges",
        ));
    }
    let mut topology_options = *options;
    topology_options.inline_labels = false;
    expand_component(&c, graph, &topology_options, work, expanded)?;
    let crossings = order::constrained(expanded, &c, Some((groups, parents)), options, work)?;
    let x = position::assign(expanded, &c, None, graph, options, work)?;
    Ok((x, crossings))
}

pub(crate) fn orientations(
    nodes: &[NodeSpec],
    edges: &[(usize, usize)],
    options: &Options,
    work: &mut Work<'_>,
) -> LayoutResult<Vec<bool>> {
    u32::try_from(nodes.len()).map_err(|_| WorkError::ArithmeticOverflow)?;
    u32::try_from(edges.len()).map_err(|_| WorkError::ArithmeticOverflow)?;
    work.charge(nodes.len())?;
    let boundaries = nodes.iter().any(|n| n.boundary.is_some());
    work.charge(edges.len())?;
    let mut links = crate::work::reserved(edges.len())?;
    for (edge, &(from, to)) in edges.iter().enumerate() {
        work.visit(1)?;
        if from >= nodes.len() || to >= nodes.len() {
            return Err(LayoutError::InvalidInput("invalid orientation endpoint"));
        }
        if from != to {
            links.push(Link {
                from: from as u32,
                to: to as u32,
                edge: edge as u32,
                span: 1,
                weight: 1.,
            });
        }
    }
    let direction = boundary::orient(nodes, &links, boundaries, options, work)?;
    let arcs = expand::oriented(&links, &direction.reversed, work)?;
    let ranks = rank::assign(
        nodes.len(),
        &arcs,
        &direction.topo,
        options.rank_policy,
        boundaries.then_some(nodes),
        work,
    )?;
    work.charge(edges.len())?;
    let mut flipped = filled(edges.len(), false)?;
    for link in &links {
        work.visit(1)?;
        flipped[link.edge as usize] = ranks[link.from as usize] > ranks[link.to as usize];
    }
    Ok(flipped)
}

use crate::{EdgeGeometry, LayoutOutput, LayoutStats, PortSideHint, Rect, Side};

#[derive(Default)]
struct Drawing {
    nodes: Vec<Rect>,
    edges: Vec<(u32, EdgeGeometry)>,
}

fn physical_side(dir: Direction, side: Side) -> Side {
    use Side::*;
    match dir {
        Direction::Down => side,
        Direction::Up => match side {
            Top => Bottom,
            Bottom => Top,
            other => other,
        },
        Direction::Right => match side {
            Top => Left,
            Bottom => Right,
            Left => Top,
            Right => Bottom,
        },
        Direction::Left => match side {
            Top => Right,
            Bottom => Left,
            Left => Top,
            Right => Bottom,
        },
    }
}

fn to_canonical_port(dir: Direction, hint: PortSideHint) -> PortSideHint {
    let (side, offset) = match hint {
        PortSideHint::Auto => return hint,
        PortSideHint::Side(s) => (s, None),
        PortSideHint::FixedOffset { side, offset } => (side, Some(offset)),
    };
    let canonical = match dir {
        Direction::Left => match side {
            Side::Right => Side::Top,
            Side::Left => Side::Bottom,
            Side::Top => Side::Left,
            Side::Bottom => Side::Right,
        },
        _ => physical_side(dir, side),
    };
    if let Some(offset) = offset {
        // An offset grows along the physical face. Only a reversed longitudinal axis flips it.
        let reverse = matches!(canonical, Side::Left | Side::Right)
            && matches!(dir, Direction::Up | Direction::Left);
        PortSideHint::FixedOffset {
            side: canonical,
            offset: if reverse { -offset } else { offset },
        }
    } else {
        PortSideHint::Side(canonical)
    }
}

pub(crate) fn run(
    graph: &Graph,
    options: &Options,
    g: &mut Expanded,
    work: &mut Work<'_>,
) -> LayoutResult<LayoutOutput> {
    run_packed(graph, options, g, None, work).map(|(drawing, _)| drawing)
}

pub(crate) fn fold(
    graph: &Graph,
    options: &Options,
    g: &mut Expanded,
    max_width: f64,
    work: &mut Work<'_>,
) -> LayoutResult<Option<LayoutOutput>> {
    if !graph.groups.is_empty() || !matches!(graph.direction, Direction::Down | Direction::Up) {
        return Ok(None);
    }
    run_packed(graph, options, g, Some(max_width), work)
        .map(|(drawing, folded)| folded.then_some(drawing))
}

fn draw_component(
    g: &mut Expanded,
    c: &Component,
    orientation: &acyclic::Orientation,
    graph: &Graph,
    options: &Options,
    work: &mut Work<'_>,
) -> LayoutResult<Drawing> {
    draw_with_label_gaps(g, c, orientation, graph, options, work).map(|(drawing, _)| drawing)
}

fn draw_with_label_gaps(
    g: &mut Expanded,
    c: &Component,
    orientation: &acyclic::Orientation,
    graph: &Graph,
    options: &Options,
    work: &mut Work<'_>,
) -> LayoutResult<(Drawing, Vec<[f64; 2]>)> {
    let mut x = position::assign(g, c, Some(orientation), graph, options, work)?;
    refine::refine(g, c, orientation, graph, options, &mut x, work)?;
    position::straighten_exterior_lanes(g, c, options, &mut x, work)?;
    let (mut drawing, label_gaps) =
        routing::route_with_label_gaps(g, c, orientation, graph, options, &x, work)?;
    extras::finish(&mut drawing, g, c, graph, options, work)?;
    Ok((drawing, label_gaps))
}

fn run_packed(
    graph: &Graph,
    options: &Options,
    g: &mut Expanded,
    fold_limit: Option<f64>,
    work: &mut Work<'_>,
) -> LayoutResult<(LayoutOutput, bool)> {
    let components = validate::components(graph, options, work)?;
    let mut stats = LayoutStats {
        components: components.len(),
        ..Default::default()
    };
    let mut tiles = crate::work::reserved(components.len())?;
    let mut folded = false;
    for c in components {
        let mut local = *options;
        local.caps.max_dummies -= stats.dummy_nodes;
        local.inline_labels &= fold_limit.is_none();
        let cache = fold_limit.is_none() && g.component_cache.active_for(&c);
        let previous_choice = work.editor_choice;
        let cached = if cache {
            work.editor_choice = false;
            g.component_cache.reuse(&c, graph, local, work)?
        } else {
            None
        };
        let (solution, folded_component) = match cached {
            Some(solution) => (solution, false),
            None => solve_component(g, &c, graph, options, local, fold_limit, work)?,
        };
        if cache {
            g.component_cache
                .remember(&c, graph, local, &solution, work)?;
        }
        work.editor_choice = previous_choice || solution.editor_choice;
        folded |= folded_component;
        let component_cache::LocalLayout {
            drawing,
            ranks,
            stats: component,
            ..
        } = solution;
        stats.scc_cyclic_count += component.scc_cyclic_count;
        stats.dummy_nodes += component.dummy_nodes;
        stats.direct_fallback_edges += component.direct_fallback_edges;
        stats.reversed_edges += component.reversed_edges;
        stats.topological_crossings = stats
            .topological_crossings
            .checked_add(component.topological_crossings)
            .ok_or(WorkError::ArithmeticOverflow)?;
        stats.routed_crossings = stats
            .routed_crossings
            .checked_add(component.routed_crossings)
            .ok_or(WorkError::ArithmeticOverflow)?;
        tiles.push(pack::Tile::new(
            c.ids,
            ranks,
            drawing,
            graph,
            options.width_policy,
            work,
        )?);
    }
    Ok((pack::assemble(tiles, graph, options, stats, work)?, folded))
}

fn solve_component(
    g: &mut Expanded,
    c: &Component,
    graph: &Graph,
    options: &Options,
    mut local: Options,
    fold_limit: Option<f64>,
    work: &mut Work<'_>,
) -> LayoutResult<(component_cache::LocalLayout, bool)> {
    let editor_choice = work.editor_choice;
    let mut restored_labels = Vec::new();
    let orientation = boundary::orient(&c.nodes, &c.links, c.boundaries, &local, work)?;
    let reversed = orientation
        .reversed
        .iter()
        .filter(|&&flipped| flipped)
        .count();
    loop {
        expand::build(c, &orientation, graph, &local, &restored_labels, work, g)?;
        let crossings = order_component(g, c, &orientation, &local, work)?;
        let dummies = g.rank.len() - c.nodes.len();
        let fallbacks = g.direct.iter().filter(|&&direct| direct).count();
        let ranks = if options.capture_node_ranks {
            work.charge(c.nodes.len())?;
            let mut ranks = crate::work::reserved(c.nodes.len())?;
            ranks.extend_from_slice(&g.rank[..c.nodes.len()]);
            ranks
        } else {
            Vec::new()
        };
        let candidate = match fold_limit {
            Some(max_width) => fan::fold(g, c, graph, &local, max_width, work)?,
            None => None,
        };
        let folded_component = candidate.is_some();
        let (mut drawing, label_gaps, dummies, fallbacks, reversed) = match candidate {
            Some(candidate) => (
                candidate.drawing,
                Vec::new(),
                candidate.dummies,
                candidate.fallbacks,
                candidate.reversed,
            ),
            None => {
                let (drawing, label_gaps) =
                    draw_with_label_gaps(g, c, &orientation, graph, options, work)?;
                (drawing, label_gaps, dummies, fallbacks, reversed)
            }
        };
        if !folded_component {
            hub::spread(&mut drawing, c, graph, options, work)?;
        }
        let compress_shared =
            folded_component || bus::bundle(&mut drawing, c, graph, options, work)?;
        if !label_gaps.is_empty() {
            let placement = label_place::place(
                &mut drawing,
                &g.inline_labels,
                &label_gaps,
                graph,
                options,
                work,
            )?;
            match placement {
                label_place::Placement::Placed => {}
                label_place::Placement::Restore(gaps) if restored_labels.is_empty() => {
                    restored_labels = gaps;
                    work.editor_choice = editor_choice;
                    continue;
                }
                // A second failed placement restores every label obstacle and its routes.
                label_place::Placement::Restore(_) | label_place::Placement::Legacy => {
                    local.inline_labels = false;
                    work.editor_choice = editor_choice;
                    continue;
                }
            }
        }
        if options.compact_ranks && !folded_component {
            routing::compact_ranks(&mut drawing, c, g, graph, options, work)?;
        }
        let stats = LayoutStats {
            scc_cyclic_count: orientation.cyclic_sccs,
            dummy_nodes: dummies,
            direct_fallback_edges: fallbacks,
            reversed_edges: reversed,
            topological_crossings: crossings,
            routed_crossings: crossings::pairs(&drawing.edges, compress_shared, work)?,
            ..LayoutStats::default()
        };
        return Ok((
            component_cache::LocalLayout {
                drawing,
                ranks,
                stats,
                editor_choice: work.editor_choice,
            },
            folded_component,
        ));
    }
}
