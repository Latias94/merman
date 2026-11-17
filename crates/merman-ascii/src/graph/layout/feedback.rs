//! Measured, semantic capacity requests for a fresh layout generation.

use super::super::label::try_measure_edge_label;
use super::super::model::{AsciiGraph, GraphDirection, GraphEdgeMarker, GraphNodeShape};
use super::super::shape::GraphNodeShapeSize;
use super::super::topology::{GraphEndpointIndex, GraphGroupTopology};
use super::{GraphLayout, GridCoord, LayoutInput, charge_sort_work};
use crate::error::{AsciiError, Result};
use crate::options::GraphLayoutPolicy;
use crate::resource::ResourceContext;
use rustc_hash::FxHashMap as HashMap;

// A private marked terminal needs its contact column/row and one reading cell on either side.
const PRIVATE_CONTACT_PITCH: usize = 3;
const CONTACT_SHOULDERS: usize = 2;

pub(in crate::graph) struct MeasuredLayout {
    pub(in crate::graph) layout: GraphLayout,
    pub(in crate::graph::layout) reversed_edge_indices: Vec<usize>,
    pub(in crate::graph::layout) group_directions: Vec<Option<GraphDirection>>,
}

#[derive(Default)]
pub(in crate::graph) struct LayoutFeedback {
    endpoints: Vec<(usize, GraphNodeShapeSize)>,
    corridors: Vec<CorridorDemand>,
    returns: Vec<ScopedReturnDemand>,
}

struct CorridorDemand {
    source_edge_index: usize,
    width: usize,
    height: usize,
}

struct ScopedReturnDemand {
    scope: Option<usize>,
    cross_x: bool,
    cells: usize,
}

impl LayoutFeedback {
    /// Measures cyclic compound scopes before routing. Root returns and real self loops
    /// already own exterior envelopes; only recovery may request extra space for those.
    pub(in crate::graph) fn record_initial_returns_with_input(
        &mut self,
        input: &LayoutInput<'_>,
        measured: &MeasuredLayout,
        resources: &ResourceContext,
    ) -> Result<bool> {
        resources.checkpoint()?;
        if input.graph.groups.is_empty() {
            return Ok(false);
        }
        let requests = scoped_return_demands_with_input(input, measured, None, resources)?;
        let changed = !requests.is_empty();
        self.returns = requests;
        Ok(changed)
    }

    /// Records the first unmet endpoint/corridor request. The caller owns the finite rebuild
    /// schedule; a repeated request does not justify another generation.
    pub(in crate::graph) fn record_failure_with_input(
        &mut self,
        input: &LayoutInput<'_>,
        measured: &MeasuredLayout,
        miss: &AsciiError,
        resources: &ResourceContext,
    ) -> Result<bool> {
        let graph = input.graph;
        let demand = &input.demand;
        resources.checkpoint()?;
        match miss {
            AsciiError::PortCapacityExceeded {
                endpoint,
                required,
                available,
                ..
            } if required > available => {
                let Some(index) = node_index(graph, endpoint, resources)? else {
                    return Ok(false);
                };
                if fixed_terminal(graph.nodes[index].shape) {
                    return Ok(false);
                }
                let horizontal = match graph.nodes[index].shape {
                    GraphNodeShape::ForkJoinHorizontal => false,
                    GraphNodeShape::ForkJoinVertical => true,
                    _ => graph.direction.canonical() == GraphDirection::LeftRight,
                };
                let contacts = demand
                    .contacts_for_node(endpoint, resources)?
                    .max(*required);
                let mut changed =
                    self.reserve_endpoint(index, horizontal, contacts, measured, resources)?;
                // The opposite open ends still need independent contacts. Enlarging only the
                // certified endpoint would leave the same parallel scene geometrically blocked.
                for edge in &graph.edges {
                    resources.checkpoint()?;
                    resources.charge_layout_work(1)?;
                    if !edge.stroke.is_visible() {
                        continue;
                    }
                    let other = if edge.from == *endpoint {
                        &edge.to
                    } else if edge.to == *endpoint {
                        &edge.from
                    } else {
                        continue;
                    };
                    if let Some(index) = node_index(graph, other, resources)? {
                        changed |= self.reserve_endpoint(
                            index,
                            horizontal,
                            demand.contacts_for_node(other, resources)?,
                            measured,
                            resources,
                        )?;
                    }
                }
                Ok(changed)
            }
            AsciiError::RoutingSearchExhausted { edge_index, .. } => {
                self.reserve_corridor(input, measured, *edge_index, resources)
            }
            _ => Ok(false),
        }
    }

    /// Adds simultaneous return-lane demand in the failed edge's actual compound scope.
    /// SCC analysis is iterative, metered, and confined to scope-projected semantic edges.
    pub(in crate::graph) fn record_return_failure_with_input(
        &mut self,
        input: &LayoutInput<'_>,
        measured: &MeasuredLayout,
        miss: &AsciiError,
        resources: &ResourceContext,
    ) -> Result<bool> {
        let changed = self.record_failure_with_input(input, measured, miss, resources)?;
        let AsciiError::RoutingSearchExhausted { edge_index, .. } = miss else {
            return Ok(changed);
        };
        let Some(request) =
            scoped_return_demand_with_input(input, measured, *edge_index, resources)?
        else {
            return Ok(changed);
        };
        resources.charge_layout_work(self.returns.len())?;
        for current in &mut self.returns {
            resources.checkpoint()?;
            if current.scope == request.scope && current.cross_x == request.cross_x {
                let grew = current.cells < request.cells;
                current.cells = current.cells.max(request.cells);
                return Ok(changed || grew);
            }
        }
        reserve(&mut self.returns, 1)?;
        self.returns.push(request);
        Ok(true)
    }

    fn reserve_endpoint(
        &mut self,
        index: usize,
        horizontal: bool,
        contacts: usize,
        measured: &MeasuredLayout,
        resources: &ResourceContext,
    ) -> Result<bool> {
        let Some(node) = measured.layout.nodes.get(index) else {
            return Ok(false);
        };
        if fixed_terminal(node.shape) || contacts == 0 {
            return Ok(false);
        }
        let span = resources.checked_grid_add(
            resources.checked_grid_mul(contacts, PRIVATE_CONTACT_PITCH)?,
            CONTACT_SHOULDERS,
        )?;
        let mut size = GraphNodeShapeSize {
            width: node.width,
            height: node.height,
        };
        match node.shape {
            GraphNodeShape::ForkJoinHorizontal => size.width = size.width.max(span),
            GraphNodeShape::ForkJoinVertical => size.height = size.height.max(span),
            _ if horizontal => size.height = size.height.max(span),
            _ => size.width = size.width.max(span),
        }
        if matches!(
            node.shape,
            GraphNodeShape::LeanLeft | GraphNodeShape::LeanRight
        ) {
            size.width = size
                .width
                .max(resources.checked_grid_add(node.width, size.height - node.height)?);
        }
        resources.grid_extent(size.width, size.height)?;
        if size.width == node.width && size.height == node.height {
            return Ok(false);
        }
        resources.charge_layout_work(self.endpoints.len())?;
        for (current_index, current) in &mut self.endpoints {
            resources.checkpoint()?;
            if *current_index == index {
                let grew = size.width > current.width || size.height > current.height;
                current.width = current.width.max(size.width);
                current.height = current.height.max(size.height);
                return Ok(grew);
            }
        }
        reserve(&mut self.endpoints, 1)?;
        self.endpoints.push((index, size));
        Ok(true)
    }

    fn reserve_corridor(
        &mut self,
        input: &LayoutInput<'_>,
        measured: &MeasuredLayout,
        source_edge_index: usize,
        resources: &ResourceContext,
    ) -> Result<bool> {
        let graph = input.graph;
        let policy = &input.policy;
        let Some(edge) = graph
            .edges
            .get(source_edge_index)
            .filter(|edge| edge.stroke.is_visible())
        else {
            return Ok(false);
        };
        let Some(from) = endpoint_bounds(&measured.layout, &edge.from, resources)? else {
            return Ok(false);
        };
        let Some(to) = endpoint_bounds(&measured.layout, &edge.to, resources)? else {
            return Ok(false);
        };
        let demand = &input.demand;
        let contacts = demand
            .contacts_for_node(&edge.from, resources)?
            .max(demand.contacts_for_node(&edge.to, resources)?);
        let independent_span = resources.checked_grid_add(
            resources.checked_grid_mul(contacts, PRIVATE_CONTACT_PITCH)?,
            CONTACT_SHOULDERS,
        )?;
        let label = edge
            .label
            .as_deref()
            .map(|raw| {
                try_measure_edge_label(
                    raw,
                    policy.terminal_width_profile,
                    resources,
                    edge.label_break_policy,
                )
            })
            .transpose()?
            .flatten();
        let marker_cells = [edge.start_marker, edge.end_marker]
            .into_iter()
            .filter(|marker| *marker != GraphEdgeMarker::Open)
            .count();
        let main_clearance = resources.checked_grid_add(
            resources.checked_grid_mul(marker_cells, PRIVATE_CONTACT_PITCH)?,
            2,
        )?;
        let direction = edge_direction(
            input,
            measured,
            edge.from.as_str(),
            edge.to.as_str(),
            resources,
        )?;
        let horizontal = direction.canonical() == GraphDirection::LeftRight;
        let rank_gap = if horizontal {
            from.gap_x(to)
        } else {
            from.gap_y(to)
        };
        let cross_gap = if horizontal {
            from.gap_y(to)
        } else {
            from.gap_x(to)
        };
        let label_main = label.map_or(0, |label| {
            if horizontal {
                label.max_width
            } else {
                label.line_count
            }
        });
        let label_cross = label.map_or(0, |label| {
            if horizontal {
                label.line_count
            } else {
                label.max_width
            }
        });
        let required_main = resources
            .checked_grid_add(label_main, main_clearance)?
            .max(resources.checked_grid_mul(contacts, 2)?);
        let required_cross = independent_span.max(resources.checked_grid_add(label_cross, 2)?);
        let main = if required_main > rank_gap {
            required_main
        } else {
            0
        };
        // A shared cross-axis center is useful for straight routes. Demand additional
        // transverse space only when distinct endpoint bands already separate on that axis.
        let cross = if cross_gap > 0 && required_cross > cross_gap {
            required_cross
        } else {
            0
        };
        if main == 0 && cross == 0 {
            return Ok(false);
        }
        let (width, height) = if horizontal {
            (main, cross)
        } else {
            (cross, main)
        };
        resources.grid_extent(width.max(1), height.max(1))?;
        resources.charge_layout_work(self.corridors.len())?;
        for current in &mut self.corridors {
            resources.checkpoint()?;
            if current.source_edge_index == source_edge_index {
                let grew = width > current.width || height > current.height;
                current.width = current.width.max(width);
                current.height = current.height.max(height);
                return Ok(grew);
            }
        }
        reserve(&mut self.corridors, 1)?;
        self.corridors.push(CorridorDemand {
            source_edge_index,
            width,
            height,
        });
        Ok(true)
    }

    pub(in crate::graph::layout) fn size_for_node(
        &self,
        index: usize,
        mut size: GraphNodeShapeSize,
        resources: &ResourceContext,
    ) -> Result<GraphNodeShapeSize> {
        resources.charge_layout_work(self.endpoints.len())?;
        for (endpoint, required) in &self.endpoints {
            resources.checkpoint()?;
            if *endpoint == index {
                size.width = size.width.max(required.width);
                size.height = size.height.max(required.height);
            }
        }
        resources.grid_extent(size.width, size.height)?;
        Ok(size)
    }

    pub(in crate::graph::layout) fn reserve_axes(
        &self,
        input: &LayoutInput<'_>,
        placements: &[GridCoord],
        columns: &mut super::grid::AxisSizes,
        rows: &mut super::grid::AxisSizes,
        resources: &ResourceContext,
    ) -> Result<()> {
        if self.corridors.is_empty() {
            return Ok(());
        }
        let graph = input.graph;
        let topology = input.topology.as_ref();
        for request in &self.corridors {
            resources.checkpoint()?;
            resources.charge_layout_work(1)?;
            let edge = &graph.edges[request.source_edge_index];
            let (Some(from), Some(to)) = (
                endpoint_grid_bounds(graph, placements, topology, &edge.from, resources)?,
                endpoint_grid_bounds(graph, placements, topology, &edge.to, resources)?,
            ) else {
                continue;
            };
            for (axis, from_start, from_end, to_start, to_end, required) in [
                (
                    &mut *columns,
                    from.0.x,
                    from.1.x,
                    to.0.x,
                    to.1.x,
                    request.width,
                ),
                (
                    &mut *rows,
                    from.0.y,
                    from.1.y,
                    to.0.y,
                    to.1.y,
                    request.height,
                ),
            ] {
                let separator = if from_end < to_start {
                    Some(to_start - 1)
                } else if to_end < from_start {
                    Some(from_start - 1)
                } else {
                    None
                };
                if let Some(separator) = separator.filter(|_| required > 0) {
                    super::grid::set_axis_size(axis, separator, required);
                }
            }
        }
        Ok(())
    }

    pub(in crate::graph::layout) fn cross_gap(
        &self,
        graph: &AsciiGraph,
        node_index: usize,
        horizontal: bool,
        base: usize,
        resources: &ResourceContext,
    ) -> Result<usize> {
        let mut gap = base;
        resources.charge_layout_work(self.corridors.len())?;
        for request in &self.corridors {
            resources.checkpoint()?;
            let edge = &graph.edges[request.source_edge_index];
            if edge.from == graph.nodes[node_index].id || edge.to == graph.nodes[node_index].id {
                gap = gap.max(if horizontal {
                    request.height
                } else {
                    request.width
                });
            }
        }
        Ok(gap)
    }

    pub(in crate::graph::layout) fn apply_group_policies(
        &self,
        policies: &mut [GraphLayoutPolicy],
        resources: &ResourceContext,
    ) -> Result<()> {
        resources.charge_layout_work(self.returns.len())?;
        for request in &self.returns {
            resources.checkpoint()?;
            if let Some(policy) = request.scope.and_then(|index| policies.get_mut(index)) {
                let padding = resources.checked_grid_add(request.cells, 1)?;
                if request.cross_x {
                    policy.group_padding_x = policy.group_padding_x.max(padding);
                } else {
                    policy.group_padding_y = policy.group_padding_y.max(padding);
                    policy.group_title_clearance = policy.group_title_clearance.max(request.cells);
                }
            }
        }
        Ok(())
    }

    pub(in crate::graph::layout) fn reserve_root_margin(
        &self,
        layout: &mut GraphLayout,
        resources: &ResourceContext,
    ) -> Result<()> {
        let mut dx = 0;
        let mut dy = 0;
        for request in &self.returns {
            resources.checkpoint()?;
            resources.charge_layout_work(1)?;
            if request.scope.is_none() {
                if request.cross_x {
                    dx = dx.max(request.cells);
                } else {
                    dy = dy.max(request.cells);
                }
            }
        }
        if dx == 0 && dy == 0 {
            return Ok(());
        }
        for node in &mut layout.nodes {
            resources.checkpoint()?;
            resources.charge_layout_work(1)?;
            node.x = resources.checked_grid_add(node.x, dx)?;
            node.y = resources.checked_grid_add(node.y, dy)?;
        }
        for group in &mut layout.groups {
            resources.checkpoint()?;
            resources.charge_layout_work(1)?;
            group.x = resources.checked_grid_add(group.x, dx)?;
            group.y = resources.checked_grid_add(group.y, dy)?;
            if let Some(span) = &mut group.divider_span {
                span.x_start = resources.checked_grid_add(span.x_start, dx)?;
                span.x_end = resources.checked_grid_add(span.x_end, dx)?;
            }
        }
        Ok(())
    }
}

#[cfg(test)]
impl LayoutFeedback {
    fn record_failure(
        &mut self,
        graph: &AsciiGraph,
        policy: &GraphLayoutPolicy,
        measured: &MeasuredLayout,
        miss: &AsciiError,
        resources: &ResourceContext,
    ) -> Result<bool> {
        let input = LayoutInput::prepare(graph, policy, &mut resources.clone())?;
        self.record_failure_with_input(&input, measured, miss, resources)
    }

    fn record_return_failure(
        &mut self,
        graph: &AsciiGraph,
        policy: &GraphLayoutPolicy,
        measured: &MeasuredLayout,
        miss: &AsciiError,
        resources: &ResourceContext,
    ) -> Result<bool> {
        let input = LayoutInput::prepare(graph, policy, &mut resources.clone())?;
        self.record_return_failure_with_input(&input, measured, miss, resources)
    }
}

fn fixed_terminal(shape: GraphNodeShape) -> bool {
    matches!(
        shape,
        GraphNodeShape::Text
            | GraphNodeShape::StateStart
            | GraphNodeShape::StateEnd
            | GraphNodeShape::Choice
    )
}

fn node_index(graph: &AsciiGraph, id: &str, resources: &ResourceContext) -> Result<Option<usize>> {
    for group in &graph.groups {
        resources.checkpoint()?;
        resources.charge_layout_work(1)?;
        if group.id == id {
            return Ok(None);
        }
    }
    for (index, node) in graph.nodes.iter().enumerate() {
        resources.checkpoint()?;
        resources.charge_layout_work(1)?;
        if node.id == id {
            return Ok(Some(index));
        }
    }
    Ok(None)
}

fn endpoint_grid_bounds(
    graph: &AsciiGraph,
    placements: &[GridCoord],
    topology: Option<&GraphGroupTopology<'_>>,
    id: &str,
    resources: &ResourceContext,
) -> Result<Option<(GridCoord, GridCoord)>> {
    let node_bounds = |index: usize| -> Result<(GridCoord, GridCoord)> {
        let first = placements[index];
        Ok((
            first,
            GridCoord {
                x: resources.checked_grid_add(first.x, 2)?,
                y: resources.checked_grid_add(first.y, 2)?,
            },
        ))
    };
    let Some(topology) = topology else {
        return node_index(graph, id, resources)?
            .map(node_bounds)
            .transpose();
    };
    match topology.endpoint_index(id) {
        Some(GraphEndpointIndex::Node(index)) => Ok(Some(node_bounds(index)?)),
        Some(GraphEndpointIndex::Group(index)) => {
            let mut membership_resources = resources.clone();
            let members = topology.group_member_node_indices(index, &mut membership_resources)?;
            let mut bounds = None::<(GridCoord, GridCoord)>;
            for index in members {
                resources.checkpoint()?;
                resources.charge_layout_work(1)?;
                let (first, last) = node_bounds(index)?;
                if let Some((start, end)) = &mut bounds {
                    start.x = start.x.min(first.x);
                    start.y = start.y.min(first.y);
                    end.x = end.x.max(last.x);
                    end.y = end.y.max(last.y);
                } else {
                    bounds = Some((first, last));
                }
            }
            Ok(bounds)
        }
        None => Ok(None),
    }
}

#[derive(Clone, Copy)]
struct EndpointBounds {
    x: usize,
    y: usize,
    right: usize,
    bottom: usize,
}

impl EndpointBounds {
    fn gap_x(self, other: Self) -> usize {
        self.x
            .max(other.x)
            .saturating_sub(self.right.min(other.right))
            .saturating_sub(1)
    }

    fn gap_y(self, other: Self) -> usize {
        self.y
            .max(other.y)
            .saturating_sub(self.bottom.min(other.bottom))
            .saturating_sub(1)
    }

    fn center(self, horizontal: bool) -> usize {
        if horizontal {
            self.x + (self.right - self.x) / 2
        } else {
            self.y + (self.bottom - self.y) / 2
        }
    }
}

fn endpoint_bounds(
    layout: &GraphLayout,
    id: &str,
    resources: &ResourceContext,
) -> Result<Option<EndpointBounds>> {
    for group in &layout.groups {
        resources.checkpoint()?;
        resources.charge_layout_work(1)?;
        if group.id == id {
            return Ok(Some(EndpointBounds {
                x: group.x,
                y: group.y,
                right: group.right(),
                bottom: group.bottom(),
            }));
        }
    }
    for node in &layout.nodes {
        resources.checkpoint()?;
        resources.charge_layout_work(1)?;
        if node.id == id {
            return Ok(Some(EndpointBounds {
                x: node.x,
                y: node.y,
                right: node.right(),
                bottom: node.bottom(),
            }));
        }
    }
    Ok(None)
}

fn edge_direction(
    input: &LayoutInput<'_>,
    measured: &MeasuredLayout,
    from: &str,
    to: &str,
    resources: &ResourceContext,
) -> Result<GraphDirection> {
    let graph = input.graph;
    let Some(topology) = input.topology.as_ref() else {
        return Ok(graph.direction.canonical());
    };
    let mut topology_resources = resources.clone();
    let from_groups = topology.groups_containing_endpoint(from, &mut topology_resources)?;
    let to_groups = topology.groups_containing_endpoint(to, &mut topology_resources)?;
    let mut nearest = None;
    for &index in &from_groups {
        resources.checkpoint()?;
        resources.charge_layout_work(1)?;
        if to_groups.contains(&index) {
            let depth = topology.group_depth(index, &mut topology_resources)?;
            if nearest.is_none_or(|(current, _)| depth > current) {
                nearest = Some((depth, index));
            }
        }
    }
    Ok(nearest
        .and_then(|(_, index)| measured.group_directions.get(index).copied().flatten())
        .map_or(graph.direction.canonical(), |direction| {
            direction.before_root_output_transform(graph.direction)
        }))
}

struct ScopedEdge {
    source_edge_index: usize,
    from: usize,
    to: usize,
    scope: Option<usize>,
}

struct ScopeGraph<'a> {
    ids: Vec<&'a str>,
    parents: Vec<Option<usize>>,
    edges: Vec<ScopedEdge>,
    outgoing: Vec<Vec<usize>>,
    incoming: Vec<Vec<usize>>,
}

impl<'a> ScopeGraph<'a> {
    #[cfg(test)]
    fn try_new(graph: &'a AsciiGraph, resources: &ResourceContext) -> Result<Self> {
        let topology = if graph.groups.is_empty() {
            None
        } else {
            Some(GraphGroupTopology::try_new(graph, &mut resources.clone())?)
        };
        Self::try_with_topology(graph, topology.as_ref(), resources)
    }

    fn try_with_topology(
        graph: &'a AsciiGraph,
        topology: Option<&GraphGroupTopology<'_>>,
        resources: &ResourceContext,
    ) -> Result<Self> {
        let count = resources.checked_work_add(graph.nodes.len(), graph.groups.len())?;
        resources.charge_layout_work_product(count, 5)?;
        let mut ids = Vec::new();
        reserve(&mut ids, count)?;
        ids.extend(graph.nodes.iter().map(|node| node.id.as_str()));
        ids.extend(graph.groups.iter().map(|group| group.id.as_str()));
        let mut indices = HashMap::default();
        indices
            .try_reserve(count)
            .map_err(|_| AsciiError::allocation_failed("ascii_layout_feedback"))?;
        for (index, &id) in ids.iter().enumerate() {
            resources.checkpoint()?;
            resources.charge_layout_work(id.len())?;
            if index < graph.nodes.len() {
                indices.entry(id).or_insert(index);
            } else {
                indices.insert(id, index);
            }
        }
        let mut parents = Vec::new();
        reserve(&mut parents, count)?;
        parents.resize(count, None);
        if let Some(topology) = topology {
            for (index, parent) in parents.iter_mut().enumerate() {
                resources.checkpoint()?;
                let group = if index < graph.nodes.len() {
                    topology.direct_node_group_index(ids[index])
                } else {
                    topology.parent_group_index(index - graph.nodes.len())
                };
                *parent = group.map(|group| group + graph.nodes.len());
            }
        }
        let mut depths = Vec::new();
        reserve(&mut depths, count)?;
        for index in 0..count {
            let mut current = index;
            let mut depth = 0;
            while let Some(parent) = parents[current] {
                resources.checkpoint()?;
                resources.charge_layout_work(1)?;
                depth = resources.checked_work_add(depth, 1)?;
                resources.check_nesting_depth(depth)?;
                if depth > graph.groups.len() {
                    return Err(AsciiError::UnsupportedFeature {
                        diagram_type: graph.diagram_type(),
                        feature: "cyclic compound feedback scopes",
                    });
                }
                current = parent;
            }
            depths.push(depth);
        }
        let mut edges = Vec::new();
        reserve(&mut edges, graph.edges.len())?;
        let mut outgoing = Vec::new();
        let mut incoming = Vec::new();
        reserve(&mut outgoing, count)?;
        reserve(&mut incoming, count)?;
        outgoing.resize_with(count, Vec::new);
        incoming.resize_with(count, Vec::new);
        for (source_edge_index, edge) in graph.edges.iter().enumerate() {
            resources.checkpoint()?;
            resources.charge_layout_work(1)?;
            if !edge.stroke.is_visible() {
                continue;
            }
            let (Some(&original_from), Some(&original_to)) = (
                indices.get(edge.from.as_str()),
                indices.get(edge.to.as_str()),
            ) else {
                continue;
            };
            let (from, to) = lift_endpoints_to_common_scope(
                original_from,
                original_to,
                &parents,
                &depths,
                resources,
            )?;
            if from == to && original_from != original_to {
                continue;
            }
            let scope = parents[from].map(|parent| parent - graph.nodes.len());
            reserve(&mut outgoing[from], 1)?;
            reserve(&mut incoming[to], 1)?;
            outgoing[from].push(to);
            incoming[to].push(from);
            edges.push(ScopedEdge {
                source_edge_index,
                from,
                to,
                scope,
            });
        }
        Ok(Self {
            ids,
            parents,
            edges,
            outgoing,
            incoming,
        })
    }

    fn endpoint_bounds(
        &self,
        layout: &GraphLayout,
        resources: &ResourceContext,
    ) -> Result<Vec<Option<EndpointBounds>>> {
        let count = resources.checked_work_add(layout.nodes.len(), layout.groups.len())?;
        resources.charge_layout_work(count)?;
        let mut by_id = HashMap::default();
        by_id
            .try_reserve(count)
            .map_err(|_| AsciiError::allocation_failed("ascii_layout_feedback"))?;
        // Resolve against this generation, retaining the first group before any node with
        // the same ID, just as the scalar endpoint lookup does.
        for (id, bounds) in layout
            .groups
            .iter()
            .map(|group| {
                (
                    group.id.as_str(),
                    EndpointBounds {
                        x: group.x,
                        y: group.y,
                        right: group.right(),
                        bottom: group.bottom(),
                    },
                )
            })
            .chain(layout.nodes.iter().map(|node| {
                (
                    node.id.as_str(),
                    EndpointBounds {
                        x: node.x,
                        y: node.y,
                        right: node.right(),
                        bottom: node.bottom(),
                    },
                )
            }))
        {
            resources.checkpoint()?;
            resources.charge_layout_work(id.len())?;
            by_id.entry(id).or_insert(bounds);
        }
        resources.charge_layout_work(self.ids.len())?;
        let mut bounds = Vec::new();
        reserve(&mut bounds, self.ids.len())?;
        for &id in &self.ids {
            resources.checkpoint()?;
            resources.charge_layout_work(id.len())?;
            bounds.push(by_id.get(id).copied());
        }
        Ok(bounds)
    }

    fn components(&self, resources: &ResourceContext) -> Result<(Vec<usize>, Vec<usize>)> {
        let count = self.ids.len();
        resources.charge_layout_work_product(count, 4)?;
        let mut visited = Vec::new();
        reserve(&mut visited, count)?;
        visited.resize(count, false);
        let mut order = Vec::new();
        let mut stack = Vec::new();
        reserve(&mut order, count)?;
        reserve(&mut stack, count)?;
        for start in 0..count {
            resources.checkpoint()?;
            if visited[start] {
                continue;
            }
            visited[start] = true;
            stack.push((start, 0));
            while let Some((node, next)) = stack.last_mut() {
                resources.checkpoint()?;
                resources.charge_layout_work(1)?;
                if *next == self.outgoing[*node].len() {
                    order.push(*node);
                    stack.pop();
                } else {
                    let target = self.outgoing[*node][*next];
                    *next += 1;
                    if !visited[target] {
                        visited[target] = true;
                        stack.push((target, 0));
                    }
                }
            }
        }
        let mut components = Vec::new();
        let mut sizes = Vec::new();
        let mut pending = Vec::new();
        reserve(&mut components, count)?;
        reserve(&mut sizes, count)?;
        reserve(&mut pending, count)?;
        components.resize(count, usize::MAX);
        for start in order.into_iter().rev() {
            resources.checkpoint()?;
            if components[start] != usize::MAX {
                continue;
            }
            let component = sizes.len();
            let mut size = 0;
            components[start] = component;
            pending.push(start);
            while let Some(node) = pending.pop() {
                resources.checkpoint()?;
                resources.charge_layout_work(1)?;
                size += 1;
                for &source in &self.incoming[node] {
                    resources.checkpoint()?;
                    resources.charge_layout_work(1)?;
                    if components[source] == usize::MAX {
                        components[source] = component;
                        pending.push(source);
                    }
                }
            }
            sizes.push(size);
        }
        Ok((components, sizes))
    }
}

fn lift_endpoints_to_common_scope(
    mut from: usize,
    mut to: usize,
    parents: &[Option<usize>],
    depths: &[usize],
    resources: &ResourceContext,
) -> Result<(usize, usize)> {
    // Lift endpoints to immediate children of their least common enclosing scope.
    // A compound child participates in its parent's SCC; its members participate
    // independently in the child's SCC, with no cross-scope adjacency.
    while parents[from] != parents[to] {
        resources.checkpoint()?;
        resources.charge_layout_work(1)?;
        if depths[from] >= depths[to] {
            if let Some(parent) = parents[from] {
                from = parent;
            }
        } else if let Some(parent) = parents[to] {
            to = parent;
        }
    }
    Ok((from, to))
}

#[derive(PartialEq, Eq, PartialOrd, Ord)]
enum LaneEventKind {
    Start,
    End,
}

fn scoped_return_demand_with_input(
    input: &LayoutInput<'_>,
    measured: &MeasuredLayout,
    failed_edge_index: usize,
    resources: &ResourceContext,
) -> Result<Option<ScopedReturnDemand>> {
    Ok(
        scoped_return_demands_with_input(input, measured, Some(failed_edge_index), resources)?
            .pop(),
    )
}

struct ReturnLanes {
    scope: Option<usize>,
    horizontal: bool,
    pitch: usize,
    active: usize,
    peak: usize,
}

fn scoped_return_demands_with_input(
    input: &LayoutInput<'_>,
    measured: &MeasuredLayout,
    failed_edge_index: Option<usize>,
    resources: &ResourceContext,
) -> Result<Vec<ScopedReturnDemand>> {
    let graph = input.graph;
    let policy = &input.policy;
    let scoped = ScopeGraph::try_with_topology(graph, input.topology.as_ref(), resources)?;
    let (components, sizes) = scoped.components(resources)?;
    let selected_component = if let Some(failed_edge_index) = failed_edge_index {
        resources.charge_layout_work(scoped.edges.len())?;
        let mut selected = None;
        for edge in &scoped.edges {
            resources.checkpoint()?;
            if edge.source_edge_index == failed_edge_index {
                if components[edge.from] == components[edge.to] {
                    selected = Some(components[edge.from]);
                }
                break;
            }
        }
        let Some(component) = selected else {
            return Ok(Vec::new());
        };
        Some(component)
    } else {
        None
    };
    let mut events = Vec::new();
    reserve(
        &mut events,
        resources.checked_work_mul(scoped.edges.len(), 2)?,
    )?;
    resources.charge_layout_work(sizes.len())?;
    let mut lanes = Vec::new();
    reserve(&mut lanes, sizes.len())?;
    lanes.resize_with(sizes.len(), || None::<ReturnLanes>);
    let bounds = scoped.endpoint_bounds(&measured.layout, resources)?;
    for edge in &scoped.edges {
        resources.checkpoint()?;
        resources.charge_layout_work(1)?;
        if failed_edge_index.is_none() && (edge.scope.is_none() || edge.from == edge.to) {
            continue;
        }
        let component = components[edge.from];
        if selected_component.is_some_and(|selected| selected != component)
            || components[edge.to] != component
            || (sizes[component] == 1 && edge.from != edge.to)
        {
            continue;
        }
        let direction = edge
            .scope
            .and_then(|index| measured.group_directions.get(index).copied().flatten())
            .map_or(graph.direction.canonical(), |direction| {
                direction.before_root_output_transform(graph.direction)
            });
        let horizontal = direction.canonical() == GraphDirection::LeftRight;
        let reversed_direction = matches!(
            direction,
            GraphDirection::RightLeft | GraphDirection::BottomTop
        );
        resources.charge_layout_work(2)?;
        let (Some(from), Some(to)) = (bounds[edge.from], bounds[edge.to]) else {
            continue;
        };
        let start = from.center(horizontal);
        let end = to.center(horizontal);
        let physical_return = if reversed_direction {
            start <= end
        } else {
            start >= end
        };
        let rank_return = edge.scope.is_none()
            && measured
                .reversed_edge_indices
                .binary_search(&edge.source_edge_index)
                .is_ok();
        if !physical_return && !rank_return {
            continue;
        }
        events.push((component, start.min(end), LaneEventKind::Start));
        events.push((component, start.max(end), LaneEventKind::End));
        let demand = lanes[component].get_or_insert(ReturnLanes {
            scope: edge.scope,
            horizontal,
            pitch: PRIVATE_CONTACT_PITCH,
            active: 0,
            peak: 0,
        });
        let original = &graph.edges[edge.source_edge_index];
        if let Some(raw) = &original.label
            && let Some(label) = try_measure_edge_label(
                raw,
                policy.terminal_width_profile,
                resources,
                original.label_break_policy,
            )?
        {
            let cross = if horizontal {
                label.line_count
            } else {
                label.max_width
            };
            demand.pitch = demand.pitch.max(resources.checked_grid_add(cross, 2)?);
        }
    }
    if events.is_empty() {
        return Ok(Vec::new());
    }
    charge_sort_work(events.len(), resources)?;
    // Starts sort before ends: returns sharing a turn rank still need independent lanes.
    events.sort_unstable();
    for (component, _, kind) in events {
        resources.checkpoint()?;
        resources.charge_layout_work(1)?;
        let Some(demand) = &mut lanes[component] else {
            continue;
        };
        match kind {
            LaneEventKind::Start => {
                demand.active = resources.checked_work_add(demand.active, 1)?;
                demand.peak = demand.peak.max(demand.active);
            }
            LaneEventKind::End => demand.active -= 1,
        }
    }
    // Build both transverse margins once for each scope, rather than rescanning all
    // endpoints for every cyclic component in the initial whole-scene request.
    let scope_count = resources.checked_work_add(graph.groups.len(), 1)?;
    resources.charge_layout_work_product(scope_count, 2)?;
    let mut available = Vec::new();
    let mut requested = Vec::new();
    reserve(&mut available, scope_count)?;
    reserve(&mut requested, scope_count)?;
    available.resize(scope_count, (usize::MAX, usize::MAX));
    requested.resize(scope_count, (0usize, 0usize));
    for (index, &parent) in scoped.parents.iter().enumerate() {
        resources.checkpoint()?;
        resources.charge_layout_work(1)?;
        let slot = parent.map_or(graph.groups.len(), |parent| parent - graph.nodes.len());
        let scope_bounds = parent.and_then(|parent| bounds[parent]);
        if let Some(endpoint) = bounds[index] {
            let (x, y) = match scope_bounds {
                Some(scope) => (
                    endpoint
                        .x
                        .saturating_sub(scope.x + 1)
                        .min(scope.right.saturating_sub(endpoint.right + 1)),
                    endpoint
                        .y
                        .saturating_sub(scope.y + 1)
                        .min(scope.bottom.saturating_sub(endpoint.bottom + 1)),
                ),
                None => (endpoint.x, endpoint.y),
            };
            available[slot].0 = available[slot].0.min(x);
            available[slot].1 = available[slot].1.min(y);
        }
    }
    for demand in lanes.into_iter().flatten() {
        resources.checkpoint()?;
        resources.charge_layout_work(1)?;
        let cells = resources
            .checked_grid_add(resources.checked_grid_mul(demand.peak, demand.pitch)?, 2)?;
        resources.grid_extent(cells, 1)?;
        let slot = demand.scope.unwrap_or(graph.groups.len());
        let (available, requested) = if demand.horizontal {
            (available[slot].1, &mut requested[slot].1)
        } else {
            (available[slot].0, &mut requested[slot].0)
        };
        if cells > available {
            *requested = (*requested).max(cells);
        }
    }
    let mut requests = Vec::new();
    reserve(&mut requests, resources.checked_work_mul(scope_count, 2)?)?;
    for (slot, (x, y)) in requested.into_iter().enumerate() {
        resources.checkpoint()?;
        resources.charge_layout_work(1)?;
        for (cross_x, cells) in [(true, x), (false, y)] {
            if cells > 0 {
                requests.push(ScopedReturnDemand {
                    scope: (slot < graph.groups.len()).then_some(slot),
                    cross_x,
                    cells,
                });
            }
        }
    }
    Ok(requests)
}

#[cfg(test)]
fn scoped_return_demand(
    graph: &AsciiGraph,
    policy: &GraphLayoutPolicy,
    measured: &MeasuredLayout,
    failed_edge_index: usize,
    resources: &ResourceContext,
) -> Result<Option<ScopedReturnDemand>> {
    let input = LayoutInput::prepare(graph, policy, &mut resources.clone())?;
    scoped_return_demand_with_input(&input, measured, failed_edge_index, resources)
}

fn reserve<T>(values: &mut Vec<T>, additional: usize) -> Result<()> {
    values
        .try_reserve(additional)
        .map_err(|_| AsciiError::allocation_failed("ascii_layout_feedback"))
}

#[cfg(test)]
mod tests;
