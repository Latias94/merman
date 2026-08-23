use super::super::super::model::{AsciiGraph, AsciiGraphEdge, GraphDirection};
use super::super::super::topology::GraphGroupTopology;
use crate::error::Result;
use crate::resource::ResourceContext;

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

pub(super) fn edge_boundary_context_with_resources<'a>(
    graph: &'a AsciiGraph,
    edge: &AsciiGraphEdge,
    topology: Option<&GraphGroupTopology<'a>>,
    resources: &mut ResourceContext,
) -> Result<EdgeBoundaryContext<'a>> {
    let Some(topology) = topology else {
        return Ok(EdgeBoundaryContext::External {
            direction: graph.direction,
        });
    };
    let Some((group_index, relation)) =
        deepest_directional_boundary_group(graph, edge, topology, resources)?
    else {
        return Ok(EdgeBoundaryContext::External {
            direction: graph.direction,
        });
    };
    let Some(group) = graph.groups.get(group_index) else {
        return Ok(EdgeBoundaryContext::External {
            direction: graph.direction,
        });
    };
    let Some(local_direction) = group.direction else {
        return Ok(EdgeBoundaryContext::External {
            direction: graph.direction,
        });
    };

    Ok(match relation {
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
    })
}

#[cfg(test)]
pub(super) fn edge_boundary_context<'a>(
    graph: &'a AsciiGraph,
    edge: &AsciiGraphEdge,
) -> EdgeBoundaryContext<'a> {
    let mut resources = ResourceContext::new(crate::resource::AsciiResourcePolicy::for_profile(
        merman_core::resources::ResourceProfile::UnboundedForTrustedInput,
    ));
    let topology = if graph.groups.is_empty() {
        None
    } else {
        Some(
            GraphGroupTopology::try_new(graph, &mut resources)
                .expect("test topology construction must remain representable"),
        )
    };
    edge_boundary_context_with_resources(graph, edge, topology.as_ref(), &mut resources)
        .expect("test boundary traversal must remain representable")
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
    resources: &mut ResourceContext,
) -> Result<Option<(usize, BoundaryRelation)>> {
    let mut best = None::<BoundaryCandidate>;
    let from_groups = topology.groups_containing_endpoint(edge.from.as_str(), resources)?;
    let to_groups = topology.groups_containing_endpoint(edge.to.as_str(), resources)?;

    for (group_index, group) in graph.groups.iter().enumerate() {
        resources.charge_layout_work(1)?;
        let Some(_) = group.direction else {
            continue;
        };

        let from_inside = from_groups.contains(&group_index);
        let to_inside = to_groups.contains(&group_index);
        let relation = match (from_inside, to_inside) {
            (true, true) => BoundaryRelation::Internal,
            (false, true) => BoundaryRelation::Entering,
            (true, false) => BoundaryRelation::Leaving,
            (false, false) => continue,
        };
        let depth = topology.group_depth(group_index, resources)?;
        let candidate = BoundaryCandidate {
            group_index,
            depth,
            relation,
        };
        if best.is_none_or(|current| candidate.depth > current.depth) {
            best = Some(candidate);
        }
    }

    Ok(best.map(|candidate| (candidate.group_index, candidate.relation)))
}
