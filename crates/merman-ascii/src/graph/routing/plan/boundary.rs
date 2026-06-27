use super::super::super::model::{AsciiGraph, AsciiGraphEdge, GraphDirection};
use super::super::super::topology::GraphGroupTopology;

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
    let topology = GraphGroupTopology::new(graph);
    let Some((group_index, relation)) = deepest_directional_boundary_group(graph, edge, &topology)
    else {
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

fn deepest_directional_boundary_group(
    graph: &AsciiGraph,
    edge: &AsciiGraphEdge,
    topology: &GraphGroupTopology<'_>,
) -> Option<(usize, BoundaryRelation)> {
    let mut best = None::<BoundaryCandidate>;

    for (group_index, group) in graph.groups.iter().enumerate() {
        let Some(_) = group.direction else {
            continue;
        };

        let from_inside = topology.group_contains_endpoint(group_index, edge.from.as_str());
        let to_inside = topology.group_contains_endpoint(group_index, edge.to.as_str());
        let relation = match (from_inside, to_inside) {
            (true, true) => BoundaryRelation::Internal,
            (false, true) => BoundaryRelation::Entering,
            (true, false) => BoundaryRelation::Leaving,
            (false, false) => continue,
        };
        let depth = topology.group_depth(group_index);
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
