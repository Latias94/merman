use super::{MarkerOccupant, SceneOccupancy, TerminalClaim};
use crate::error::{AsciiError, Result};
use crate::graph::charset::GraphCharset;
use crate::graph::routing::layout_allocation_failed;
use crate::graph::routing::plan::{MarkerCandidate, MarkerEndpoint};
use crate::graph::routing::{PreparedRoute, sort_levels};
use crate::resource::{AsciiResourceLimitId, ResourceContext};

pub(in crate::graph::routing) fn allocate_marker_berths(
    routes: &mut [PreparedRoute],
    occupancy: &mut SceneOccupancy<'_>,
    charset: &GraphCharset,
    resources: &mut ResourceContext,
    diagram_type: &'static str,
) -> Result<()> {
    let marker_capacity = routes.len().checked_mul(2).ok_or_else(|| {
        resources
            .policy()
            .overflow(AsciiResourceLimitId::MaxLayoutWorkUnits)
    })?;
    let mut pending = Vec::new();
    pending
        .try_reserve(marker_capacity)
        .map_err(|_| layout_allocation_failed())?;
    for (route_index, route) in routes.iter().enumerate() {
        for endpoint in [MarkerEndpoint::Start, MarkerEndpoint::End] {
            let candidates = route
                .plan
                .marker_candidates(endpoint, diagram_type, resources)?;
            if !candidates.is_empty() {
                for candidate in candidates
                    .iter()
                    .filter(|candidate| !candidate.is_primary())
                {
                    resources.charge_layout_work(1)?;
                    occupancy.register_terminal_claim(
                        candidate.coord,
                        TerminalClaim {
                            route_index,
                            endpoint,
                            point_direction: candidate.point_direction,
                        },
                    )?;
                }
                pending.push(PendingMarker {
                    route_index,
                    endpoint,
                    candidates,
                });
            }
        }
    }

    let sort_work = resources.checked_work_mul(pending.len(), sort_levels(pending.len()))?;
    resources.charge_layout_work(sort_work)?;
    pending.sort_by(|left, right| {
        left.candidates
            .len()
            .cmp(&right.candidates.len())
            .then_with(|| {
                routes[left.route_index]
                    .owner
                    .canonical_edge_index
                    .cmp(&routes[right.route_index].owner.canonical_edge_index)
            })
            .then_with(|| {
                marker_endpoint_order(left.endpoint).cmp(&marker_endpoint_order(right.endpoint))
            })
    });

    for marker in pending {
        let mut selected = None;
        let mut predecessor = None;
        for candidate in marker.candidates.iter().copied() {
            resources.charge_layout_work(1)?;
            if !marker_candidate_continues_chain(predecessor, candidate) {
                break;
            }
            match occupancy.marker_candidate_disposition(
                routes,
                marker.route_index,
                marker.endpoint,
                candidate,
                resources,
            )? {
                MarkerCandidateDisposition::Available => {
                    selected = Some(candidate);
                    break;
                }
                MarkerCandidateDisposition::CompatiblePassThrough => {
                    predecessor = Some(candidate);
                }
                MarkerCandidateDisposition::Blocked => break,
            }
        }
        let Some(candidate) = selected else {
            return Err(AsciiError::UnsupportedFeature {
                diagram_type,
                feature: "independent endpoint marker berth exhausted",
            });
        };
        routes[marker.route_index].plan.materialize_marker_at(
            marker.endpoint,
            candidate,
            charset,
            diagram_type,
        )?;
        occupancy.suppress_route_terminal_tail(
            marker.route_index,
            candidate.terminal_tail(),
            &routes[marker.route_index].plan,
            resources,
            diagram_type,
        )?;
        let cell = routes[marker.route_index]
            .plan
            .materialized_marker_cell(marker.endpoint, diagram_type)?
            .ok_or(AsciiError::UnsupportedFeature {
                diagram_type,
                feature: "routes with unmaterialized endpoint markers",
            })?;
        occupancy.occupy_marker(
            cell.coord,
            MarkerOccupant {
                route_index: marker.route_index,
                endpoint: marker.endpoint,
            },
            resources,
            diagram_type,
        )?;
    }
    Ok(())
}

struct PendingMarker {
    route_index: usize,
    endpoint: MarkerEndpoint,
    candidates: Vec<MarkerCandidate>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(in crate::graph::routing) enum MarkerCandidateDisposition {
    Available,
    CompatiblePassThrough,
    Blocked,
}

pub(super) fn marker_candidate_continues_chain(
    predecessor: Option<MarkerCandidate>,
    candidate: MarkerCandidate,
) -> bool {
    match predecessor {
        Some(predecessor) => candidate.follows_terminal_predecessor(predecessor),
        None => candidate.is_primary(),
    }
}

const fn marker_endpoint_order(endpoint: MarkerEndpoint) -> u8 {
    match endpoint {
        MarkerEndpoint::Start => 0,
        MarkerEndpoint::End => 1,
    }
}
