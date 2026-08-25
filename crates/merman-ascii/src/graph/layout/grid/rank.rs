use super::{
    charge_sort_work, index_order, layout_allocation_failed, try_reserve_hash_map,
    try_reserve_hash_set, try_reserve_vec,
};
use crate::error::{AsciiError, Result};
use crate::graph::model::{AsciiGraph, AsciiGraphEdge, GraphDirection, GraphNodeSide};
use crate::graph::topology::{GraphEndpointIndex, GraphGroupTopology};
use crate::operation::AsciiExecution;
use crate::resource::{AsciiResourceLimitId, ResourceContext};
use dugong::graphlib::{Graph, GraphOptions, is_javascript_array_index};
use dugong::{
    EdgeLabel, GraphLabel as DagreGraphLabel, NodeLabel, RankDir, WorkControl, WorkError,
};
use std::collections::{HashMap, HashSet};

const GRID_UNITS_PER_RANK: usize = 4;

type NodeIndexById<'a> = HashMap<&'a str, usize>;

pub(super) struct RankLevels {
    pub(super) nodes: Vec<usize>,
    pub(super) leaf_groups: Vec<Option<usize>>,
    pub(super) parent_indices: Vec<Vec<usize>>,
    pub(super) side_constraints: Vec<Option<ResolvedNodeSideConstraint>>,
}

#[derive(Debug, Clone, Copy)]
pub(super) struct ResolvedNodeSideConstraint {
    pub(super) anchor_node_index: usize,
    pub(super) side: GraphNodeSide,
}

struct DagreRankLevels {
    nodes: Vec<usize>,
    leaf_groups: Vec<Option<usize>>,
}

struct DagreRankGraph {
    // Keep the graphlib owner off callers' fixed stack frames. Rank planning invokes several
    // independently large debug-build phases, so carrying this aggregate by value through the
    // whole call chain can exhaust otherwise reasonable small worker stacks.
    graph: Box<Graph<NodeLabel, EdgeLabel, DagreGraphLabel>>,
    node_ids: Vec<String>,
    group_ids: Vec<String>,
    group_anchors: Vec<GroupRankAnchor>,
}

struct PlannedDagreRanks {
    plan: dugong::rank::RankPlan,
    node_ids: Vec<String>,
    group_ids: Vec<String>,
    group_anchors: Vec<GroupRankAnchor>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
enum GroupRankAnchor {
    Node(usize),
    Group(usize),
}

pub(super) fn rank_levels(
    graph: &AsciiGraph,
    topology: Option<&GraphGroupTopology<'_>>,
    direction: GraphDirection,
    resources: &mut ResourceContext,
    execution: AsciiExecution<'_>,
) -> Result<RankLevels> {
    // Preserve the existing admission order: resolve authored side constraints first, then run
    // Dagre, then project the adjusted levels and incoming-parent lanes.
    let side_constraints = resolve_node_side_constraints(graph, topology, resources)?;
    let mut levels = dagre_rank_levels(graph, topology, direction, resources, execution)?;
    apply_side_constraint_levels(
        graph,
        direction,
        &side_constraints,
        &mut levels.nodes,
        resources,
    )?;
    let parent_indices = rank_parent_indices(
        graph,
        &levels.nodes,
        direction,
        &side_constraints,
        resources,
    )?;

    Ok(RankLevels {
        nodes: levels.nodes,
        leaf_groups: levels.leaf_groups,
        parent_indices,
        side_constraints,
    })
}

pub(in crate::graph::layout) fn rank_leaf_group_levels(
    graph: &AsciiGraph,
    topology: &GraphGroupTopology<'_>,
    resources: &mut ResourceContext,
    execution: AsciiExecution<'_>,
) -> Result<Vec<Option<usize>>> {
    Ok(dagre_rank_levels(
        graph,
        Some(topology),
        graph.direction.canonical(),
        resources,
        execution,
    )?
    .leaf_groups)
}

fn resolve_node_side_constraints(
    graph: &AsciiGraph,
    topology: Option<&GraphGroupTopology<'_>>,
    resources: &ResourceContext,
) -> Result<Vec<Option<ResolvedNodeSideConstraint>>> {
    resources.charge_layout_work(graph.nodes.len())?;
    let node_index_by_id = node_indices_by_id(graph)?;
    let has_group_anchor = graph.nodes.iter().any(|node| {
        node.semantics
            .side_constraint
            .as_ref()
            .is_some_and(|constraint| !node_index_by_id.contains_key(constraint.anchor_id.as_str()))
    });
    let group_anchors = if has_group_anchor {
        Some(group_rank_anchors(
            graph,
            topology.ok_or(AsciiError::UnsupportedFeature {
                diagram_type: graph.diagram_type(),
                feature: "node side constraints with missing anchors",
            })?,
            resources,
        )?)
    } else {
        None
    };

    let mut resolved = Vec::new();
    try_reserve_vec(&mut resolved, graph.nodes.len())?;
    for node in &graph.nodes {
        let Some(constraint) = node.semantics.side_constraint.as_ref() else {
            resolved.push(None);
            continue;
        };
        let anchor_node_index =
            if let Some(index) = node_index_by_id.get(constraint.anchor_id.as_str()).copied() {
                index
            } else {
                let topology = topology.ok_or(AsciiError::UnsupportedFeature {
                    diagram_type: graph.diagram_type(),
                    feature: "node side constraints with missing anchors",
                })?;
                let Some(GraphEndpointIndex::Group(group_index)) =
                    topology.endpoint_index(&constraint.anchor_id)
                else {
                    return Err(AsciiError::UnsupportedFeature {
                        diagram_type: graph.diagram_type(),
                        feature: "node side constraints with missing anchors",
                    });
                };
                let Some(GroupRankAnchor::Node(index)) = group_anchors
                    .as_deref()
                    .and_then(|anchors| anchors.get(group_index))
                    .copied()
                else {
                    return Err(AsciiError::UnsupportedFeature {
                        diagram_type: graph.diagram_type(),
                        feature: "node side constraints on empty groups",
                    });
                };
                index
            };
        resolved.push(Some(ResolvedNodeSideConstraint {
            anchor_node_index,
            side: constraint.side,
        }));
    }
    Ok(resolved)
}

fn apply_side_constraint_levels(
    graph: &AsciiGraph,
    direction: GraphDirection,
    resolved_side_constraints: &[Option<ResolvedNodeSideConstraint>],
    rank_levels: &mut [usize],
    resources: &ResourceContext,
) -> Result<()> {
    if direction.canonical() != GraphDirection::TopDown {
        return Ok(());
    }
    resources.charge_layout_work(graph.nodes.len())?;
    for (node_index, constraint) in resolved_side_constraints.iter().copied().enumerate() {
        let Some(constraint) = constraint else {
            continue;
        };
        let Some(anchor_level) = rank_levels.get(constraint.anchor_node_index).copied() else {
            return Err(AsciiError::UnsupportedFeature {
                diagram_type: graph.diagram_type(),
                feature: "node side constraints with missing anchor ranks",
            });
        };
        let Some(level) = rank_levels.get_mut(node_index) else {
            return Err(AsciiError::UnsupportedFeature {
                diagram_type: graph.diagram_type(),
                feature: "node side constraints with missing node ranks",
            });
        };
        *level = anchor_level;
    }
    Ok(())
}

fn rank_parent_indices(
    graph: &AsciiGraph,
    rank_levels: &[usize],
    direction: GraphDirection,
    resolved_side_constraints: &[Option<ResolvedNodeSideConstraint>],
    resources: &ResourceContext,
) -> Result<Vec<Vec<usize>>> {
    resources
        .charge_layout_work(resources.checked_work_add(graph.nodes.len(), graph.edges.len())?)?;
    let index_by_id = node_indices_by_id(graph)?;
    let mut parents = Vec::new();
    try_reserve_vec(&mut parents, graph.nodes.len())?;
    parents.resize_with(graph.nodes.len(), Vec::new);
    for edge in &graph.edges {
        let (from_index, to_index) =
            if let Some((node_index, _, _)) = side_constraint_for_edge(graph, &index_by_id, edge) {
                let Some(constraint) = resolved_side_constraints.get(node_index).copied().flatten()
                else {
                    continue;
                };
                if direction.canonical() == GraphDirection::TopDown {
                    continue;
                }
                canonical_side_constraint_endpoints(
                    graph.direction,
                    node_index,
                    constraint.anchor_node_index,
                    constraint.side,
                )
            } else {
                let (Some(from_index), Some(to_index)) = (
                    index_by_id.get(edge.from.as_str()).copied(),
                    index_by_id.get(edge.to.as_str()).copied(),
                ) else {
                    continue;
                };
                (from_index, to_index)
            };
        if from_index == to_index || rank_levels[from_index] >= rank_levels[to_index] {
            continue;
        }
        let incoming = &mut parents[to_index];
        incoming
            .try_reserve(1)
            .map_err(|_| layout_allocation_failed())?;
        incoming.push(from_index);
    }
    Ok(parents)
}

fn dagre_rank_levels(
    graph: &AsciiGraph,
    topology: Option<&GraphGroupTopology<'_>>,
    direction: GraphDirection,
    resources: &mut ResourceContext,
    execution: AsciiExecution<'_>,
) -> Result<DagreRankLevels> {
    let planned = plan_dagre_ranks(graph, topology, direction, resources, execution)?;
    project_dagre_rank_levels(graph, planned, resources, execution)
}

fn plan_dagre_ranks(
    graph: &AsciiGraph,
    topology: Option<&GraphGroupTopology<'_>>,
    direction: GraphDirection,
    resources: &mut ResourceContext,
    execution: AsciiExecution<'_>,
) -> Result<PlannedDagreRanks> {
    let DagreRankGraph {
        graph: rank_graph,
        node_ids,
        group_ids,
        group_anchors,
    } = build_dagre_rank_graph(graph, topology, direction, resources, execution)?;
    let plan = {
        let mut work_control = AsciiDagreWorkControl::new(resources, execution);
        match dugong::rank::plan_controlled(&rank_graph, &mut work_control) {
            Ok(plan) => plan,
            Err(error) => return Err(work_control.into_ascii_error(error, graph.diagram_type())),
        }
    };

    Ok(PlannedDagreRanks {
        plan,
        node_ids,
        group_ids,
        group_anchors,
    })
}

fn project_dagre_rank_levels(
    graph: &AsciiGraph,
    planned: PlannedDagreRanks,
    resources: &mut ResourceContext,
    execution: AsciiExecution<'_>,
) -> Result<DagreRankLevels> {
    let PlannedDagreRanks {
        plan,
        node_ids,
        group_ids,
        group_anchors,
    } = planned;

    let endpoint_count = resources.checked_work_add(node_ids.len(), group_ids.len())?;
    let projection_work = resources.checked_work_add(
        plan.nodes.len(),
        resources.checked_work_mul(endpoint_count, 4)?,
    )?;
    resources.charge_layout_work(projection_work)?;
    let mut rank_by_id = HashMap::new();
    try_reserve_hash_map(&mut rank_by_id, plan.nodes.len())?;
    for (index, node) in plan.nodes.into_iter().enumerate() {
        checkpoint_layout(execution, index)?;
        if let Some(rank) = node.rank {
            rank_by_id.insert(node.id, rank);
        }
    }

    let mut node_ranks = Vec::new();
    try_reserve_vec(&mut node_ranks, node_ids.len())?;
    for (index, id) in node_ids.iter().enumerate() {
        checkpoint_layout(execution, index)?;
        let Some(rank) = rank_by_id.get(id.as_str()).copied() else {
            return Err(AsciiError::UnsupportedFeature {
                diagram_type: graph.diagram_type(),
                feature: "Dagre-compatible rank planning for an unranked graph node",
            });
        };
        node_ranks.push(rank);
    }

    let mut leaf_group_ranks = Vec::new();
    try_reserve_vec(&mut leaf_group_ranks, group_ids.len())?;
    leaf_group_ranks.resize(group_ids.len(), None);
    for (group_index, anchor) in group_anchors.iter().copied().enumerate() {
        checkpoint_layout(execution, group_index)?;
        if anchor != GroupRankAnchor::Group(group_index) {
            continue;
        }
        let Some(group_id) = group_ids.get(group_index) else {
            continue;
        };
        let Some(rank) = rank_by_id.get(group_id.as_str()).copied() else {
            return Err(AsciiError::UnsupportedFeature {
                diagram_type: graph.diagram_type(),
                feature: "Dagre-compatible rank planning for an empty compound group",
            });
        };
        leaf_group_ranks[group_index] = Some(rank);
    }

    // Dagre rank values can contain intentionally empty numeric ranks (especially around compound
    // borders). Terminal geometry needs the ordering and same-rank equivalence, not zero-width
    // canvas cells for those absent layers. Empty groups are real compound leaves, so retain their
    // occupied ranks alongside caller nodes without projecting them as phantom node layouts.
    let mut occupied_ranks = Vec::new();
    try_reserve_vec(&mut occupied_ranks, endpoint_count)?;
    occupied_ranks.extend(node_ranks.iter().copied());
    occupied_ranks.extend(leaf_group_ranks.iter().flatten().copied());
    charge_sort_work(occupied_ranks.len(), resources)?;
    occupied_ranks.sort_unstable();
    occupied_ranks.dedup();
    let mut dense_rank = HashMap::new();
    try_reserve_hash_map(&mut dense_rank, occupied_ranks.len())?;
    for (index, rank) in occupied_ranks.into_iter().enumerate() {
        checkpoint_layout(execution, index)?;
        dense_rank.insert(
            rank,
            resources.checked_grid_mul(index, GRID_UNITS_PER_RANK)?,
        );
    }

    let mut node_levels = Vec::new();
    try_reserve_vec(&mut node_levels, node_ranks.len())?;
    for (index, rank) in node_ranks.into_iter().enumerate() {
        checkpoint_layout(execution, index)?;
        let Some(level) = dense_rank.get(&rank).copied() else {
            return Err(AsciiError::UnsupportedFeature {
                diagram_type: graph.diagram_type(),
                feature: "Dagre-compatible dense rank projection",
            });
        };
        node_levels.push(level);
    }

    let mut leaf_group_levels = Vec::new();
    try_reserve_vec(&mut leaf_group_levels, leaf_group_ranks.len())?;
    for (index, rank) in leaf_group_ranks.into_iter().enumerate() {
        checkpoint_layout(execution, index)?;
        let level = rank
            .map(|rank| {
                dense_rank
                    .get(&rank)
                    .copied()
                    .ok_or_else(|| AsciiError::UnsupportedFeature {
                        diagram_type: graph.diagram_type(),
                        feature: "Dagre-compatible dense empty-group rank projection",
                    })
            })
            .transpose()?;
        leaf_group_levels.push(level);
    }

    Ok(DagreRankLevels {
        nodes: node_levels,
        leaf_groups: leaf_group_levels,
    })
}

fn build_dagre_rank_graph(
    graph: &AsciiGraph,
    topology: Option<&GraphGroupTopology<'_>>,
    direction: GraphDirection,
    resources: &ResourceContext,
    execution: AsciiExecution<'_>,
) -> Result<DagreRankGraph> {
    let mut rank_graph = Graph::new(GraphOptions {
        directed: true,
        multigraph: true,
        compound: true,
    });
    rank_graph.set_graph(DagreGraphLabel {
        rankdir: dagre_rank_direction(direction),
        ..DagreGraphLabel::default()
    });
    rank_graph.set_default_node_label(NodeLabel::default);
    rank_graph.set_default_edge_label(|| EdgeLabel {
        minlen: 1,
        weight: 1.0,
        ..EdgeLabel::default()
    });

    let node_order = graphlib_node_order(graph, resources)?;
    let mut node_ids = Vec::new();
    try_reserve_vec(&mut node_ids, graph.nodes.len())?;
    node_ids.resize_with(graph.nodes.len(), String::new);
    for (ordinal, node_index) in node_order.into_iter().enumerate() {
        checkpoint_layout(execution, ordinal)?;
        let internal_id = format!("node:{ordinal}");
        rank_graph.set_node(internal_id.clone(), NodeLabel::default());
        node_ids[node_index] = internal_id;
    }

    let group_order = graphlib_group_order(graph, resources)?;
    let mut group_ids = Vec::new();
    try_reserve_vec(&mut group_ids, graph.groups.len())?;
    group_ids.resize_with(graph.groups.len(), String::new);
    for (ordinal, group_index) in group_order.into_iter().enumerate() {
        checkpoint_layout(execution, ordinal)?;
        let internal_id = format!("group:{ordinal}");
        rank_graph.set_node(internal_id.clone(), NodeLabel::default());
        group_ids[group_index] = internal_id;
    }

    if let Some(topology) = topology {
        let parent_capacity = resources.checked_work_add(graph.nodes.len(), graph.groups.len())?;
        // Admit the two assignment scans plus the bounded union/validation and replay owned by
        // Graphlib's atomic parent batch before materializing the compound forest.
        resources.charge_layout_work(resources.checked_work_mul(parent_capacity, 8)?)?;
        let mut parent_assignments = Vec::new();
        try_reserve_vec(&mut parent_assignments, parent_capacity)?;
        for (node_index, node_id) in node_ids.iter().enumerate() {
            checkpoint_layout(execution, node_index)?;
            let Some(group_index) = topology.direct_node_group_index(&graph.nodes[node_index].id)
            else {
                continue;
            };
            if let Some(group_id) = group_ids.get(group_index) {
                let (Some(node_ix), Some(group_ix)) =
                    (rank_graph.node_ix(node_id), rank_graph.node_ix(group_id))
                else {
                    continue;
                };
                parent_assignments.push((node_ix, group_ix));
            }
        }
        for (group_index, group_id) in group_ids.iter().enumerate() {
            checkpoint_layout(execution, group_index)?;
            let Some(parent_index) = topology.parent_group_index(group_index) else {
                continue;
            };
            if let Some(parent_id) = group_ids.get(parent_index) {
                let (Some(group_ix), Some(parent_ix)) =
                    (rank_graph.node_ix(group_id), rank_graph.node_ix(parent_id))
                else {
                    continue;
                };
                parent_assignments.push((group_ix, parent_ix));
            }
        }
        rank_graph
            .try_set_unparented_parents_ix(&parent_assignments)
            .map_err(|_| AsciiError::UnsupportedFeature {
                diagram_type: graph.diagram_type(),
                feature: "cyclic or multiply-owned compound graph membership",
            })?;
    }

    let index_by_id = node_indices_by_id(graph)?;
    let group_anchors = topology
        .map(|topology| group_rank_anchors(graph, topology, resources))
        .transpose()?
        .unwrap_or_default();
    let edge_order = graphlib_edge_order(graph)?;
    for (ordinal, edge_index) in edge_order.into_iter().enumerate() {
        checkpoint_layout(execution, ordinal)?;
        let edge = &graph.edges[edge_index];
        let constrained_endpoints = side_constraint_for_edge(graph, &index_by_id, edge);
        let (from_id, to_id) = if let Some((node_index, anchor_id, side)) = constrained_endpoints {
            if direction.canonical() == GraphDirection::TopDown {
                continue;
            }
            let Some(node_id) = node_ids.get(node_index).map(String::as_str) else {
                continue;
            };
            let Some(anchor_id) = rank_endpoint_id(
                anchor_id,
                &index_by_id,
                topology,
                Some(group_anchors.as_slice()),
                &node_ids,
                &group_ids,
            ) else {
                continue;
            };
            match canonical_side(graph.direction, side) {
                GraphNodeSide::Left => (node_id, anchor_id),
                GraphNodeSide::Right => (anchor_id, node_id),
            }
        } else {
            let (Some(from_id), Some(to_id)) = (
                rank_endpoint_id(
                    &edge.from,
                    &index_by_id,
                    topology,
                    Some(group_anchors.as_slice()),
                    &node_ids,
                    &group_ids,
                ),
                rank_endpoint_id(
                    &edge.to,
                    &index_by_id,
                    topology,
                    Some(group_anchors.as_slice()),
                    &node_ids,
                    &group_ids,
                ),
            ) else {
                continue;
            };
            (from_id, to_id)
        };
        rank_graph.set_edge_named(
            from_id.to_string(),
            to_id.to_string(),
            Some(format!("edge:{ordinal}")),
            Some(EdgeLabel {
                minlen: edge.length,
                weight: 1.0,
                ..EdgeLabel::default()
            }),
        );
    }

    Ok(DagreRankGraph {
        graph: Box::new(rank_graph),
        node_ids,
        group_ids,
        group_anchors,
    })
}

fn side_constraint_for_edge<'a>(
    graph: &AsciiGraph,
    index_by_id: &NodeIndexById<'_>,
    edge: &'a AsciiGraphEdge,
) -> Option<(usize, &'a str, GraphNodeSide)> {
    for (node_endpoint, anchor_endpoint) in [
        (edge.from.as_str(), edge.to.as_str()),
        (edge.to.as_str(), edge.from.as_str()),
    ] {
        let Some(node_index) = index_by_id.get(node_endpoint).copied() else {
            continue;
        };
        let Some(constraint) = graph
            .nodes
            .get(node_index)
            .and_then(|node| node.semantics.side_constraint.as_ref())
        else {
            continue;
        };
        if anchor_endpoint == constraint.anchor_id {
            return Some((node_index, anchor_endpoint, constraint.side));
        }
    }
    None
}

fn canonical_side(graph_direction: GraphDirection, side: GraphNodeSide) -> GraphNodeSide {
    if graph_direction == GraphDirection::RightLeft {
        side.reversed()
    } else {
        side
    }
}

fn canonical_side_constraint_endpoints(
    graph_direction: GraphDirection,
    node_index: usize,
    anchor_index: usize,
    side: GraphNodeSide,
) -> (usize, usize) {
    match canonical_side(graph_direction, side) {
        GraphNodeSide::Left => (node_index, anchor_index),
        GraphNodeSide::Right => (anchor_index, node_index),
    }
}

fn graphlib_node_order(graph: &AsciiGraph, resources: &ResourceContext) -> Result<Vec<usize>> {
    let mut order = index_order(graph.nodes.len())?;
    charge_sort_work(order.len(), resources)?;
    order.sort_unstable_by(|left, right| {
        graphlib_creation_order_cmp(
            &graph.nodes[*left].id,
            *left,
            &graph.nodes[*right].id,
            *right,
        )
    });
    Ok(order)
}

fn graphlib_group_order(graph: &AsciiGraph, resources: &ResourceContext) -> Result<Vec<usize>> {
    let mut order = index_order(graph.groups.len())?;
    charge_sort_work(order.len(), resources)?;
    order.sort_unstable_by(|left, right| {
        graphlib_creation_order_cmp(
            &graph.groups[*left].id,
            *left,
            &graph.groups[*right].id,
            *right,
        )
    });
    Ok(order)
}

fn graphlib_creation_order_cmp(
    left: &str,
    left_creation_index: usize,
    right: &str,
    right_creation_index: usize,
) -> std::cmp::Ordering {
    // Graphlib stores node and child ids as JavaScript object properties. Array-index keys are
    // enumerated numerically; every other string retains its first-property-creation order.
    match (
        is_javascript_array_index(left),
        is_javascript_array_index(right),
    ) {
        (true, true) => match (left.parse::<u32>(), right.parse::<u32>()) {
            (Ok(left), Ok(right)) => left
                .cmp(&right)
                .then_with(|| left_creation_index.cmp(&right_creation_index)),
            _ => left
                .cmp(right)
                .then_with(|| left_creation_index.cmp(&right_creation_index)),
        },
        (true, false) => std::cmp::Ordering::Less,
        (false, true) => std::cmp::Ordering::Greater,
        (false, false) => left_creation_index.cmp(&right_creation_index),
    }
}

fn graphlib_edge_order(graph: &AsciiGraph) -> Result<Vec<usize>> {
    // Graphlib stores edge objects under delimiter-containing property keys, so `edges()` observes
    // their first `setEdge` creation order. Re-sorting by endpoint or label metadata changes DFS
    // cycle breaking and Network Simplex tie-breaking compared with pinned Mermaid.
    index_order(graph.edges.len())
}

fn group_rank_anchors(
    graph: &AsciiGraph,
    topology: &GraphGroupTopology<'_>,
    resources: &ResourceContext,
) -> Result<Vec<GroupRankAnchor>> {
    let mut anchors = Vec::new();
    try_reserve_vec(&mut anchors, graph.groups.len())?;
    for group_index in 0..graph.groups.len() {
        resources.charge_layout_work(1)?;
        anchors.push(find_non_cluster_child_anchor(
            graph,
            topology,
            group_index,
            resources,
        )?);
    }
    Ok(anchors)
}

fn find_non_cluster_child_anchor(
    graph: &AsciiGraph,
    topology: &GraphGroupTopology<'_>,
    root_group_index: usize,
    resources: &ResourceContext,
) -> Result<GroupRankAnchor> {
    let root_children =
        ordered_direct_group_children(graph, topology, root_group_index, resources)?;
    if root_children.is_empty() {
        return Ok(GroupRankAnchor::Group(root_group_index));
    }

    let mut stack = Vec::new();
    try_reserve_vec(&mut stack, root_children.len())?;
    stack.extend(root_children.into_iter().rev());
    let mut visited_groups = HashSet::new();
    try_reserve_hash_set(&mut visited_groups, graph.groups.len())?;
    let mut reserve = None;

    while let Some(endpoint) = stack.pop() {
        resources.charge_layout_work(1)?;
        let candidate = match endpoint {
            GraphEndpointIndex::Node(node_index) => GroupRankAnchor::Node(node_index),
            GraphEndpointIndex::Group(group_index) => {
                if !visited_groups.insert(group_index) {
                    continue;
                }
                let children =
                    ordered_direct_group_children(graph, topology, group_index, resources)?;
                if children.is_empty() {
                    GroupRankAnchor::Group(group_index)
                } else {
                    try_reserve_vec(&mut stack, children.len())?;
                    stack.extend(children.into_iter().rev());
                    continue;
                }
            }
        };

        if group_anchor_has_common_edge(graph, root_group_index, candidate, resources)? {
            reserve = Some(candidate);
        } else {
            return Ok(candidate);
        }
    }

    Ok(reserve.unwrap_or(GroupRankAnchor::Group(root_group_index)))
}

fn ordered_direct_group_children(
    graph: &AsciiGraph,
    topology: &GraphGroupTopology<'_>,
    group_index: usize,
    resources: &ResourceContext,
) -> Result<Vec<GraphEndpointIndex>> {
    let Some(group) = graph.groups.get(group_index) else {
        return Ok(Vec::new());
    };
    resources.charge_layout_work(group.nodes.len())?;
    let mut children = Vec::new();
    try_reserve_vec(&mut children, group.nodes.len())?;
    let mut seen = HashSet::new();
    try_reserve_hash_set(&mut seen, group.nodes.len())?;
    for (creation_index, member_id) in group.nodes.iter().enumerate() {
        let endpoint = match topology.endpoint_index(member_id) {
            Some(GraphEndpointIndex::Node(node_index))
                if topology.direct_node_group_index(member_id) == Some(group_index) =>
            {
                GraphEndpointIndex::Node(node_index)
            }
            Some(GraphEndpointIndex::Group(child_group_index))
                if child_group_index != group_index
                    && topology.parent_group_index(child_group_index) == Some(group_index) =>
            {
                GraphEndpointIndex::Group(child_group_index)
            }
            _ => continue,
        };
        if seen.insert(endpoint) {
            children.push((creation_index, member_id.as_str(), endpoint));
        }
    }
    charge_sort_work(children.len(), resources)?;
    children.sort_unstable_by(|left, right| {
        graphlib_creation_order_cmp(left.1, left.0, right.1, right.0)
    });
    let mut ordered = Vec::new();
    try_reserve_vec(&mut ordered, children.len())?;
    ordered.extend(children.into_iter().map(|(_, _, endpoint)| endpoint));
    Ok(ordered)
}

fn group_anchor_has_common_edge(
    graph: &AsciiGraph,
    group_index: usize,
    candidate: GroupRankAnchor,
    resources: &ResourceContext,
) -> Result<bool> {
    let Some(group_id) = graph.groups.get(group_index).map(|group| group.id.as_str()) else {
        return Ok(false);
    };
    let candidate_id = match candidate {
        GroupRankAnchor::Node(node_index) => {
            graph.nodes.get(node_index).map(|node| node.id.as_str())
        }
        GroupRankAnchor::Group(candidate_group_index) => graph
            .groups
            .get(candidate_group_index)
            .map(|group| group.id.as_str()),
    };
    let Some(candidate_id) = candidate_id else {
        return Ok(false);
    };

    // Pinned Mermaid snapshots both incident edge lists and compares the endpoint pairs after its
    // intentionally asymmetric `w` rewrite. A hash set preserves that behavior in linear work.
    resources.charge_layout_work(resources.checked_work_mul(graph.edges.len(), 2)?)?;
    let mut candidate_edges = HashSet::new();
    try_reserve_hash_set(&mut candidate_edges, graph.edges.len())?;
    for edge in &graph.edges {
        if edge.from == candidate_id || edge.to == candidate_id {
            candidate_edges.insert((edge.from.as_str(), edge.to.as_str()));
        }
    }
    for edge in &graph.edges {
        if edge.from != group_id && edge.to != group_id {
            continue;
        }
        let rewritten = (
            if edge.from == group_id {
                candidate_id
            } else {
                edge.from.as_str()
            },
            if edge.to == group_id {
                group_id
            } else {
                edge.to.as_str()
            },
        );
        if candidate_edges.contains(&rewritten) {
            return Ok(true);
        }
    }
    Ok(false)
}

fn rank_endpoint_id<'a>(
    endpoint: &str,
    node_index_by_id: &NodeIndexById<'_>,
    topology: Option<&GraphGroupTopology<'_>>,
    group_anchors: Option<&[GroupRankAnchor]>,
    node_ids: &'a [String],
    group_ids: &'a [String],
) -> Option<&'a str> {
    let endpoint_index = match topology {
        Some(topology) => topology.endpoint_index(endpoint)?,
        None => GraphEndpointIndex::Node(node_index_by_id.get(endpoint).copied()?),
    };

    match endpoint_index {
        GraphEndpointIndex::Node(node_index) => node_ids.get(node_index).map(String::as_str),
        GraphEndpointIndex::Group(group_index) => {
            // Mermaid's Dagre wrapper does not leave an externally connected edge attached to a
            // non-empty compound node: it either extracts the cluster as an outer leaf or rewrites
            // the endpoint through `findNonClusterChild`. A childless group is already a real leaf
            // and keeps its own rank id; routing still targets the visible group boundary.
            match group_anchors?.get(group_index).copied()? {
                GroupRankAnchor::Node(node_index) => node_ids.get(node_index).map(String::as_str),
                GroupRankAnchor::Group(anchor_group_index) => {
                    group_ids.get(anchor_group_index).map(String::as_str)
                }
            }
        }
    }
}

fn node_indices_by_id(graph: &AsciiGraph) -> Result<NodeIndexById<'_>> {
    let mut index_by_id = HashMap::new();
    try_reserve_hash_map(&mut index_by_id, graph.nodes.len())?;
    for (index, node) in graph.nodes.iter().enumerate() {
        index_by_id.insert(node.id.as_str(), index);
    }
    Ok(index_by_id)
}

const fn dagre_rank_direction(direction: GraphDirection) -> RankDir {
    match direction {
        GraphDirection::TopDown => RankDir::TB,
        GraphDirection::BottomTop => RankDir::BT,
        GraphDirection::LeftRight => RankDir::LR,
        GraphDirection::RightLeft => RankDir::RL,
    }
}

fn checkpoint_layout(execution: AsciiExecution<'_>, iteration: usize) -> Result<()> {
    execution.checkpoint_loop(merman_core::OperationPhase::Layout, iteration)
}

struct AsciiDagreWorkControl<'resources, 'execution> {
    resources: &'resources ResourceContext,
    execution: AsciiExecution<'execution>,
    ascii_error: Option<AsciiError>,
}

impl<'resources, 'execution> AsciiDagreWorkControl<'resources, 'execution> {
    fn new(resources: &'resources ResourceContext, execution: AsciiExecution<'execution>) -> Self {
        Self {
            resources,
            execution,
            ascii_error: None,
        }
    }

    fn into_ascii_error(
        mut self,
        error: dugong::LayoutError,
        diagram_type: &'static str,
    ) -> AsciiError {
        if let Some(error) = self.ascii_error.take() {
            return error;
        }
        match error {
            dugong::LayoutError::Work(WorkError::ArithmeticOverflow) => self
                .resources
                .overflow(AsciiResourceLimitId::MaxLayoutWorkUnits),
            _ => AsciiError::UnsupportedFeature {
                diagram_type,
                feature: "Dagre-compatible rank planning for this graph topology",
            },
        }
    }
}

impl WorkControl for AsciiDagreWorkControl<'_, '_> {
    fn charge(&mut self, units: usize) -> std::result::Result<(), WorkError> {
        if self.ascii_error.is_some() {
            return Err(WorkError::Interrupted);
        }
        if let Err(error) = self
            .execution
            .checkpoint(merman_core::OperationPhase::Layout)
        {
            self.ascii_error = Some(error);
            return Err(WorkError::Interrupted);
        }
        if let Err(error) = self.resources.charge_layout_work(units) {
            self.ascii_error = Some(error);
            return Err(WorkError::Interrupted);
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::graph::model::GraphGroupStyle;
    use crate::resource::AsciiResourcePolicy;
    use merman_core::resources::ResourceProfile;
    use merman_core::{OperationControl, OperationPhase};

    fn unbounded_resources() -> ResourceContext {
        ResourceContext::new(AsciiResourcePolicy::for_profile(
            ResourceProfile::UnboundedForTrustedInput,
        ))
    }

    #[test]
    fn graphlib_node_and_group_order_preserves_ordinary_creation_order() {
        let mut graph = AsciiGraph::new(GraphDirection::TopDown);
        for id in ["z", "10", "a", "2", "01", "4294967295"] {
            graph.add_node(id, id);
            graph.add_group_with_style(
                format!("group-{id}"),
                id,
                None,
                Vec::new(),
                GraphGroupStyle::default(),
            );
        }
        graph.add_group_with_style(
            "10",
            "numeric ten",
            None,
            Vec::new(),
            GraphGroupStyle::default(),
        );
        graph.add_group_with_style(
            "2",
            "numeric two",
            None,
            Vec::new(),
            GraphGroupStyle::default(),
        );

        let resources = unbounded_resources();
        let node_order = graphlib_node_order(&graph, &resources).unwrap();
        let node_ids = node_order
            .into_iter()
            .map(|index| graph.nodes[index].id.as_str())
            .collect::<Vec<_>>();
        assert_eq!(node_ids, ["2", "10", "z", "a", "01", "4294967295"]);

        let group_order = graphlib_group_order(&graph, &resources).unwrap();
        let group_ids = group_order
            .into_iter()
            .map(|index| graph.groups[index].id.as_str())
            .collect::<Vec<_>>();
        assert_eq!(
            group_ids,
            [
                "2",
                "10",
                "group-z",
                "group-10",
                "group-a",
                "group-2",
                "group-01",
                "group-4294967295",
            ]
        );
    }

    #[test]
    fn dagre_rank_graph_preserves_authored_edge_creation_order() {
        let mut graph = AsciiGraph::new(GraphDirection::TopDown);
        for id in ["z", "a", "m"] {
            graph.add_node(id, id);
        }
        graph.add_edge("m", "z");
        graph.add_edge("z", "a");
        graph.add_edge("a", "m");

        let resources = unbounded_resources();
        let policy = resources.policy();
        let DagreRankGraph {
            graph: rank_graph,
            node_ids,
            ..
        } = build_dagre_rank_graph(
            &graph,
            None,
            GraphDirection::TopDown,
            &resources,
            AsciiExecution::for_test(&policy),
        )
        .unwrap();
        let edges = rank_graph.edges().collect::<Vec<_>>();

        assert_eq!(edges.len(), 3);
        assert_eq!(
            (&edges[0].v, &edges[0].w, edges[0].name.as_deref()),
            (&node_ids[2], &node_ids[0], Some("edge:0"))
        );
        assert_eq!(
            (&edges[1].v, &edges[1].w, edges[1].name.as_deref()),
            (&node_ids[0], &node_ids[1], Some("edge:1"))
        );
        assert_eq!(
            (&edges[2].v, &edges[2].w, edges[2].name.as_deref()),
            (&node_ids[1], &node_ids[2], Some("edge:2"))
        );
    }

    #[test]
    fn state_composite_group_endpoints_enter_the_dagre_rank_graph() {
        let mut graph = AsciiGraph::new_for_diagram("state", GraphDirection::TopDown);
        graph.add_node("before", "Before");
        graph.add_node("child", "Child");
        graph.add_node("after", "After");
        graph.add_group_with_style(
            "Inner",
            "Inner",
            None,
            vec!["child".to_string()],
            GraphGroupStyle::default(),
        );
        graph.add_group_with_style(
            "Parent",
            "Parent",
            None,
            vec!["Inner".to_string()],
            GraphGroupStyle::default(),
        );
        graph.add_edge("before", "Parent");
        graph.add_edge("Parent", "after");

        let mut resources = unbounded_resources();
        let policy = resources.policy();
        let topology = GraphGroupTopology::try_new(&graph, &mut resources).unwrap();
        let DagreRankGraph {
            graph: rank_graph,
            node_ids,
            ..
        } = build_dagre_rank_graph(
            &graph,
            Some(&topology),
            GraphDirection::TopDown,
            &resources,
            AsciiExecution::for_test(&policy),
        )
        .unwrap();
        let edges = rank_graph.edges().collect::<Vec<_>>();

        assert_eq!(edges.len(), 2);
        assert_eq!((&edges[0].v, &edges[0].w), (&node_ids[0], &node_ids[1]));
        assert_eq!((&edges[1].v, &edges[1].w), (&node_ids[1], &node_ids[2]));

        let levels = dagre_rank_levels(
            &graph,
            Some(&topology),
            GraphDirection::TopDown,
            &mut resources,
            AsciiExecution::for_test(&policy),
        )
        .unwrap();
        assert!(levels.nodes[0] < levels.nodes[1]);
        assert!(levels.nodes[1] < levels.nodes[2]);
    }

    #[test]
    fn pinned_group_to_first_child_keeps_the_outgoing_asymmetry() {
        let mut graph = AsciiGraph::new(GraphDirection::TopDown);
        graph.add_node("first", "First");
        graph.add_node("second", "Second");
        graph.add_group_with_style(
            "group",
            "Group",
            None,
            vec!["first".to_string(), "second".to_string()],
            GraphGroupStyle::default(),
        );
        graph.add_edge("group", "first");

        let mut resources = unbounded_resources();
        let topology = GraphGroupTopology::try_new(&graph, &mut resources).unwrap();
        let anchors = group_rank_anchors(&graph, &topology, &resources).unwrap();

        // Mermaid's pinned `findCommonEdges` rewrites the `w` side asymmetrically, so the
        // group-to-first-child case intentionally remains anchored to that first child.
        assert_eq!(anchors, [GroupRankAnchor::Node(0)]);
    }

    #[test]
    fn child_to_group_avoids_the_common_first_child_edge() {
        let mut graph = AsciiGraph::new(GraphDirection::TopDown);
        graph.add_node("first", "First");
        graph.add_node("second", "Second");
        graph.add_group_with_style(
            "group",
            "Group",
            None,
            vec!["first".to_string(), "second".to_string()],
            GraphGroupStyle::default(),
        );
        graph.add_edge("first", "group");

        let mut resources = unbounded_resources();
        let topology = GraphGroupTopology::try_new(&graph, &mut resources).unwrap();
        let anchors = group_rank_anchors(&graph, &topology, &resources).unwrap();

        assert_eq!(anchors, [GroupRankAnchor::Node(1)]);
    }

    #[test]
    fn nested_group_anchor_rechecks_descendants_against_the_root_group() {
        let mut graph = AsciiGraph::new(GraphDirection::TopDown);
        graph.add_node("first", "First");
        graph.add_node("second", "Second");
        graph.add_group_with_style(
            "inner",
            "Inner",
            None,
            vec!["first".to_string(), "second".to_string()],
            GraphGroupStyle::default(),
        );
        graph.add_group_with_style(
            "outer",
            "Outer",
            None,
            vec!["inner".to_string()],
            GraphGroupStyle::default(),
        );
        graph.add_edge("first", "outer");

        let mut resources = unbounded_resources();
        let topology = GraphGroupTopology::try_new(&graph, &mut resources).unwrap();
        let anchors = group_rank_anchors(&graph, &topology, &resources).unwrap();

        assert_eq!(anchors[1], GroupRankAnchor::Node(1));
    }

    #[test]
    fn outgoing_parallel_common_edge_uses_the_next_descendant() {
        let mut graph = AsciiGraph::new(GraphDirection::TopDown);
        graph.add_node("first", "First");
        graph.add_node("second", "Second");
        graph.add_node("sink", "Sink");
        graph.add_group_with_style(
            "group",
            "Group",
            None,
            vec!["first".to_string(), "second".to_string()],
            GraphGroupStyle::default(),
        );
        graph.add_edge("group", "sink");
        graph.add_edge("first", "sink");

        let mut resources = unbounded_resources();
        let topology = GraphGroupTopology::try_new(&graph, &mut resources).unwrap();
        let anchors = group_rank_anchors(&graph, &topology, &resources).unwrap();

        assert_eq!(anchors, [GroupRankAnchor::Node(1)]);
    }

    #[test]
    fn compound_anchor_scans_are_checked_against_the_layout_work_budget() {
        let mut graph = AsciiGraph::new(GraphDirection::TopDown);
        graph.add_node("first", "First");
        graph.add_node("second", "Second");
        graph.add_group_with_style(
            "group",
            "Group",
            None,
            vec!["first".to_string(), "second".to_string()],
            GraphGroupStyle::default(),
        );
        graph.add_edge("first", "group");

        let unbounded = AsciiResourcePolicy::for_profile(ResourceProfile::UnboundedForTrustedInput);
        let mut topology_resources = ResourceContext::new(unbounded);
        let topology = GraphGroupTopology::try_new(&graph, &mut topology_resources).unwrap();

        let measured_resources = ResourceContext::new(unbounded);
        group_rank_anchors(&graph, &topology, &measured_resources).unwrap();
        let exact_work = measured_resources.layout_work_used();
        assert!(exact_work > 0);

        let exact_policy = unbounded
            .with_limit(AsciiResourceLimitId::MaxLayoutWorkUnits, exact_work)
            .unwrap();
        let exact_resources = ResourceContext::new(exact_policy);
        group_rank_anchors(&graph, &topology, &exact_resources)
            .expect("exact compound-anchor work budget should pass");
        assert_eq!(exact_resources.layout_work_used(), exact_work);

        let below_policy = unbounded
            .with_limit(AsciiResourceLimitId::MaxLayoutWorkUnits, exact_work - 1)
            .unwrap();
        let below_resources = ResourceContext::new(below_policy);
        let error = group_rank_anchors(&graph, &topology, &below_resources)
            .expect_err("max-minus-one compound-anchor work budget should fail");
        let AsciiError::ResourceLimitExceeded(details) = error else {
            panic!("expected a layout-work resource error, got {error:?}");
        };
        assert_eq!(details.limit, AsciiResourceLimitId::MaxLayoutWorkUnits);
        assert_eq!(details.actual, exact_work);
        assert_eq!(details.max, exact_work - 1);
    }

    #[test]
    fn empty_group_uses_its_real_compound_leaf_rank() {
        let mut graph = AsciiGraph::new(GraphDirection::TopDown);
        graph.add_node("after", "After");
        graph.add_group_with_style(
            "empty",
            "Empty",
            None,
            Vec::new(),
            GraphGroupStyle::default(),
        );
        graph.add_edge("empty", "after");

        let mut resources = unbounded_resources();
        let policy = resources.policy();
        let topology = GraphGroupTopology::try_new(&graph, &mut resources).unwrap();
        let DagreRankGraph {
            graph: rank_graph,
            node_ids,
            group_ids,
            group_anchors: anchors,
        } = build_dagre_rank_graph(
            &graph,
            Some(&topology),
            GraphDirection::TopDown,
            &resources,
            AsciiExecution::for_test(&policy),
        )
        .unwrap();
        let edges = rank_graph.edges().collect::<Vec<_>>();

        assert_eq!(anchors, [GroupRankAnchor::Group(0)]);
        assert_eq!((&edges[0].v, &edges[0].w), (&group_ids[0], &node_ids[0]));

        let levels = dagre_rank_levels(
            &graph,
            Some(&topology),
            GraphDirection::TopDown,
            &mut resources,
            AsciiExecution::for_test(&policy),
        )
        .unwrap();
        assert!(levels.leaf_groups[0].unwrap() < levels.nodes[0]);
    }

    #[test]
    fn dagre_rank_work_observes_operation_cancellation() {
        let mut graph = AsciiGraph::new(GraphDirection::TopDown);
        graph.add_node("a", "A");
        graph.add_node("b", "B");
        graph.add_edge("a", "b");
        let policy = AsciiResourcePolicy::default();
        let mut resources = ResourceContext::new(policy);
        let control = OperationControl::new();
        // Two graph-construction checkpoints complete before cancellation is observed through
        // Dugong's existing work-charge seam.
        control.cancel_after_checkpoints(2);

        let error = match dagre_rank_levels(
            &graph,
            None,
            GraphDirection::TopDown,
            &mut resources,
            AsciiExecution::new(&control, &policy),
        ) {
            Ok(_) => panic!("Dagre rank work should observe scheduled cancellation"),
            Err(error) => error,
        };

        assert!(matches!(
            error,
            AsciiError::Cancelled(cancelled)
                if cancelled.phase == OperationPhase::Layout
                    && cancelled.reason == merman_core::CancelReason::Requested
        ));
    }
}
