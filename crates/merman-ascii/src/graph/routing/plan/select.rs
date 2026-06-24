use std::collections::{HashMap, HashSet};

use super::super::super::charset::GraphCharset;
use super::super::super::layout::{GraphLayout, NodeLayout};
use super::super::super::model::{AsciiGraph, AsciiGraphEdge, GraphDirection};
use super::super::path::Port;
use super::PlannedRouteSegment;
use super::RoutePlan;
use super::grid::{
    GridRouteOptions, plan_left_right_grid_path_route,
    plan_left_right_grid_path_route_with_options, plan_left_right_grid_path_route_with_ports,
};
use super::left_right::{
    left_right_back_edge_bottom_y, plan_left_right_bottom_lane_route, plan_left_right_direct_route,
    plan_left_right_down_route, plan_left_right_down_then_right_route,
    plan_left_right_reverse_over_self_loop_route, plan_left_right_right_then_up_route,
    plan_left_right_self_loop_route, self_loop_bottom_y_for_edges, self_loop_right_x,
};
use super::top_down::{
    plan_top_down_back_route, plan_top_down_bent_route, plan_top_down_direct_route,
    plan_top_down_side_entry_route, top_down_back_edge_lane_x,
};
use crate::text::display_width;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(in crate::graph::routing) enum EdgeBoundaryContext<'a> {
    External {
        direction: GraphDirection,
    },
    Internal {
        group_id: &'a str,
        direction: GraphDirection,
    },
    Entering {
        group_id: &'a str,
        root_direction: GraphDirection,
        local_direction: GraphDirection,
    },
    Leaving {
        group_id: &'a str,
        root_direction: GraphDirection,
        local_direction: GraphDirection,
    },
}

#[derive(Debug, Clone, Copy)]
pub(in crate::graph::routing) struct EdgeRouteRequest<'a> {
    pub(in crate::graph::routing) graph: &'a AsciiGraph,
    pub(in crate::graph::routing) graph_layout: &'a GraphLayout,
    pub(in crate::graph::routing) edges: &'a [AsciiGraphEdge],
    pub(in crate::graph::routing) from: &'a NodeLayout,
    pub(in crate::graph::routing) to: &'a NodeLayout,
    pub(in crate::graph::routing) edge_index: usize,
    pub(in crate::graph::routing) edge: &'a AsciiGraphEdge,
    pub(in crate::graph::routing) charset: &'a GraphCharset,
}

pub(in crate::graph::routing) fn plan_edge_route(
    request: EdgeRouteRequest<'_>,
) -> Option<RoutePlan> {
    let boundary = edge_boundary_context(request.graph, request.edge);
    if let Some(plan) = plan_boundary_route(boundary, request) {
        return Some(plan);
    }

    match boundary.direction().canonical() {
        GraphDirection::LeftRight => plan_left_right_route(request),
        GraphDirection::TopDown => plan_top_down_route(request),
        GraphDirection::RightLeft | GraphDirection::BottomTop => unreachable!(),
    }
}

fn plan_left_right_route(request: EdgeRouteRequest<'_>) -> Option<RoutePlan> {
    let graph_layout = request.graph_layout;
    let from = request.from;
    let to = request.to;
    let edge = request.edge;
    let charset = request.charset;

    if from.id == to.id {
        return plan_left_right_self_loop_route(
            &graph_layout.nodes,
            request.edges,
            from,
            edge,
            charset,
        );
    }

    let parallel_index = parallel_edge_index(request.edges, request.edge_index);
    if from.center_y() == to.center_y() && from.x < to.x && parallel_index > 0 {
        return plan_left_right_bottom_lane_route(from, to, edge, charset);
    }

    if from.center_y() == to.center_y() && from.x > to.x {
        if has_self_loop(request.edges, &to.id) {
            return plan_left_right_reverse_over_self_loop_route(
                &graph_layout.nodes,
                from,
                to,
                edge,
                charset,
            );
        }
        return plan_left_right_bottom_lane_route(from, to, edge, charset);
    }

    if from.center_y() == to.center_y()
        && from.x < to.x
        && let Some(plan) =
            plan_left_right_direct_route(&graph_layout.nodes, from, to, edge, charset)
    {
        return Some(plan);
    }

    if let Some(plan) = plan_left_right_grid_path_route(graph_layout, from, to, edge, charset) {
        return Some(plan);
    }

    if from.center_y() < to.center_y() && to.x > from.x {
        return plan_left_right_down_then_right_route(
            &graph_layout.nodes,
            request.edges,
            from,
            to,
            edge,
            charset,
        );
    }

    if from.center_y() < to.center_y() && to.x == from.x {
        return plan_left_right_down_route(from, to, edge, charset);
    }

    if from.center_y() > to.center_y() && to.x > from.x {
        return plan_left_right_right_then_up_route(
            &graph_layout.nodes,
            request.edges,
            from,
            to,
            edge,
            charset,
        );
    }

    None
}

fn plan_top_down_route(request: EdgeRouteRequest<'_>) -> Option<RoutePlan> {
    let from = request.from;
    let to = request.to;
    let edge = request.edge;
    let charset = request.charset;

    if from.center_y() > to.center_y() {
        return plan_top_down_back_route(from, to, edge, charset);
    }

    if from.center_y() == to.center_y()
        && let Some(plan) =
            plan_left_right_direct_route(&request.graph_layout.nodes, from, to, edge, charset)
    {
        return Some(plan);
    }

    if from.center_x() != to.center_x() {
        return plan_top_down_bent_route(from, to, edge, charset);
    }

    plan_top_down_direct_route(from, to, edge, charset)
}

fn plan_boundary_route(
    boundary: EdgeBoundaryContext<'_>,
    request: EdgeRouteRequest<'_>,
) -> Option<RoutePlan> {
    match boundary {
        EdgeBoundaryContext::Entering {
            root_direction: GraphDirection::TopDown,
            local_direction: GraphDirection::LeftRight,
            ..
        } => plan_left_right_grid_path_route_with_ports(
            request.graph_layout,
            request.from,
            request.to,
            request.edge,
            request.charset,
            Some(Port::Right),
            Some(Port::Left),
        ),
        EdgeBoundaryContext::Leaving {
            root_direction: GraphDirection::TopDown,
            local_direction: GraphDirection::LeftRight,
            ..
        } => plan_left_right_grid_path_route_with_options(
            request.graph_layout,
            request.from,
            request.to,
            request.edge,
            request.charset,
            GridRouteOptions::with_ports(Some(Port::Right), Some(Port::Right))
                .with_segment(PlannedRouteSegment::Boundary),
        ),
        EdgeBoundaryContext::Entering {
            group_id,
            root_direction: GraphDirection::TopDown,
            local_direction: GraphDirection::TopDown,
        } if request.edge.to == group_id => {
            plan_top_down_side_entry_route(request.from, request.to, request.edge, request.charset)
        }
        EdgeBoundaryContext::External { .. } | EdgeBoundaryContext::Internal { .. } => None,
        EdgeBoundaryContext::Entering { .. } | EdgeBoundaryContext::Leaving { .. } => None,
    }
}

pub(in crate::graph::routing) fn route_canvas_extent(
    graph: &AsciiGraph,
    layouts: &[NodeLayout],
    edges: &[AsciiGraphEdge],
    _direction: GraphDirection,
) -> (usize, usize) {
    let mut width = 0;
    let mut height = 0;

    for edge in edges.iter().filter(|edge| edge.from == edge.to) {
        let Some(layout) = layouts.iter().find(|layout| layout.id == edge.from) else {
            continue;
        };
        width = width.max(self_loop_right_x(layouts, layout) + 1);
        height = height.max(self_loop_bottom_y_for_edges(layouts, edges, layout) + 1);
    }
    for (edge_index, edge) in edges
        .iter()
        .enumerate()
        .filter(|(_, edge)| edge.from != edge.to)
    {
        let Some(from) = layouts.iter().find(|layout| layout.id == edge.from) else {
            continue;
        };
        let Some(to) = layouts.iter().find(|layout| layout.id == edge.to) else {
            continue;
        };
        match edge_boundary_context(graph, edge).direction().canonical() {
            GraphDirection::LeftRight => {
                if from.center_y() == to.center_y()
                    && (from.x > to.x || parallel_edge_index(edges, edge_index) > 0)
                {
                    width = width.max(from.center_x().max(to.center_x()) + 3);
                    height = height.max(left_right_back_edge_bottom_y(from) + 1);
                }
            }
            GraphDirection::TopDown => {
                if from.center_y() > to.center_y() {
                    let lane_x = top_down_back_edge_lane_x(from, to);
                    width = width.max(lane_x + 3);
                    if let Some(label) = edge.label.as_deref() {
                        let label_start = lane_x.saturating_sub(display_width(label) / 2);
                        width = width.max(label_start + display_width(label) + 1);
                    }
                }
            }
            GraphDirection::RightLeft | GraphDirection::BottomTop => unreachable!(),
        }
    }

    (width, height)
}

pub(in crate::graph::routing) fn edge_boundary_context<'a>(
    graph: &'a AsciiGraph,
    edge: &AsciiGraphEdge,
) -> EdgeBoundaryContext<'a> {
    let Some((group_index, relation)) = narrowest_boundary_group(graph, edge) else {
        return EdgeBoundaryContext::External {
            direction: graph.direction,
        };
    };
    let Some(group) = graph.groups.get(group_index) else {
        return EdgeBoundaryContext::External {
            direction: graph.direction,
        };
    };
    let Some(local_direction) = group.direction else {
        return EdgeBoundaryContext::External {
            direction: graph.direction,
        };
    };

    match relation {
        BoundaryRelation::Internal => EdgeBoundaryContext::Internal {
            group_id: group.id.as_str(),
            direction: local_direction,
        },
        BoundaryRelation::Entering => EdgeBoundaryContext::Entering {
            group_id: group.id.as_str(),
            root_direction: graph.direction,
            local_direction,
        },
        BoundaryRelation::Leaving => EdgeBoundaryContext::Leaving {
            group_id: group.id.as_str(),
            root_direction: graph.direction,
            local_direction,
        },
    }
}

impl EdgeBoundaryContext<'_> {
    fn direction(self) -> GraphDirection {
        match self {
            Self::External { direction } | Self::Internal { direction, .. } => direction,
            Self::Entering {
                root_direction: _,
                local_direction,
                ..
            }
            | Self::Leaving {
                root_direction: _,
                local_direction,
                ..
            } => local_direction,
        }
    }
}

fn has_self_loop(edges: &[AsciiGraphEdge], node_id: &str) -> bool {
    edges
        .iter()
        .any(|edge| edge.from == node_id && edge.to == node_id)
}

fn parallel_edge_index(edges: &[AsciiGraphEdge], edge_index: usize) -> usize {
    let Some(edge) = edges.get(edge_index) else {
        return 0;
    };
    edges[..edge_index]
        .iter()
        .filter(|previous| same_edge_pair(previous, edge))
        .count()
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum BoundaryRelation {
    Internal,
    Entering,
    Leaving,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct BoundaryCandidate {
    group_index: usize,
    depth: usize,
    relation: BoundaryRelation,
}

fn narrowest_boundary_group(
    graph: &AsciiGraph,
    edge: &AsciiGraphEdge,
) -> Option<(usize, BoundaryRelation)> {
    let group_index_by_id = graph
        .groups
        .iter()
        .enumerate()
        .map(|(index, group)| (group.id.as_str(), index))
        .collect::<HashMap<_, _>>();
    let parent_indices = graph
        .groups
        .iter()
        .enumerate()
        .flat_map(|(parent_index, group)| {
            group
                .nodes
                .iter()
                .filter_map(|member| {
                    group_index_by_id
                        .get(member.as_str())
                        .copied()
                        .map(|child_index| (child_index, parent_index))
                })
                .collect::<Vec<_>>()
        })
        .collect::<HashMap<_, _>>();
    let mut depth_cache = HashMap::<usize, usize>::new();
    let mut best = None::<BoundaryCandidate>;

    for (group_index, group) in graph.groups.iter().enumerate() {
        let Some(_) = group.direction else {
            continue;
        };

        let from_inside =
            group_contains_endpoint(graph, group_index, edge.from.as_str(), &group_index_by_id);
        let to_inside =
            group_contains_endpoint(graph, group_index, edge.to.as_str(), &group_index_by_id);
        let relation = match (from_inside, to_inside) {
            (true, true) => BoundaryRelation::Internal,
            (false, true) => BoundaryRelation::Entering,
            (true, false) => BoundaryRelation::Leaving,
            (false, false) => continue,
        };
        let depth = group_depth(group_index, &parent_indices, &mut depth_cache);
        let candidate = BoundaryCandidate {
            group_index,
            depth,
            relation,
        };
        if best.is_none_or(|current| candidate.depth > current.depth) {
            best = Some(candidate);
        }
    }

    best.map(|candidate| (candidate.group_index, candidate.relation))
}

fn group_contains_endpoint(
    graph: &AsciiGraph,
    group_index: usize,
    endpoint: &str,
    group_index_by_id: &HashMap<&str, usize>,
) -> bool {
    let mut visited_groups = HashSet::new();
    let mut stack = vec![group_index];

    while let Some(index) = stack.pop() {
        if !visited_groups.insert(index) {
            continue;
        }
        let Some(group) = graph.groups.get(index) else {
            continue;
        };
        if group.id == endpoint {
            return true;
        }

        for member in &group.nodes {
            if member == endpoint {
                return true;
            }
            if let Some(child_group_index) = group_index_by_id.get(member.as_str()).copied() {
                stack.push(child_group_index);
            }
        }
    }

    false
}

fn group_depth(
    group_index: usize,
    parent_indices: &HashMap<usize, usize>,
    depth_cache: &mut HashMap<usize, usize>,
) -> usize {
    if let Some(depth) = depth_cache.get(&group_index).copied() {
        return depth;
    }

    let mut visiting = HashSet::new();
    let depth = group_depth_inner(group_index, parent_indices, depth_cache, &mut visiting);
    depth_cache.insert(group_index, depth);
    depth
}

fn group_depth_inner(
    group_index: usize,
    parent_indices: &HashMap<usize, usize>,
    depth_cache: &mut HashMap<usize, usize>,
    visiting: &mut HashSet<usize>,
) -> usize {
    if let Some(depth) = depth_cache.get(&group_index).copied() {
        return depth;
    }
    if !visiting.insert(group_index) {
        return 0;
    }

    let depth = parent_indices
        .get(&group_index)
        .copied()
        .map(|parent_index| {
            1 + group_depth_inner(parent_index, parent_indices, depth_cache, visiting)
        })
        .unwrap_or(0);
    visiting.remove(&group_index);
    depth_cache.insert(group_index, depth);
    depth
}

fn same_edge_pair(left: &AsciiGraphEdge, right: &AsciiGraphEdge) -> bool {
    (left.from == right.from && left.to == right.to)
        || (left.from == right.to && left.to == right.from)
}
