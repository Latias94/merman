use std::collections::{HashMap, HashSet};

use super::super::super::model::{AsciiGraph, AsciiGraphEdge, GraphDirection};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum EdgeBoundaryContext<'a> {
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

pub(super) fn edge_boundary_context<'a>(
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
    pub(super) fn direction(self) -> GraphDirection {
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
