//! Plan, validate and materialize externally scoped hierarchy edge segments.

use super::{
    ElkInputEdge, ElkInputEdgeSegment, ElkInputEdgeSegmentEndpoint, ImportError, ImportResult,
    InputIndex, hierarchy_port_key, label_to_lgraph,
};
use crate::compound::{
    PendingCompoundSegment, PendingSegmentEndpoint, ScopedHierarchySegment,
    compound_label_segment_index, source_ported_cross_hierarchy_segments,
};
use crate::graph::{CompoundEdgeSegment, HierarchyEdge, PortType};
use crate::work::{WorkError, checked_mul, checked_sum};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct ScopedSegmentWorkPlan {
    pub(super) materialization_path_steps: usize,
    pub(super) materialized_segments: usize,
    pub(super) label_selection_steps: usize,
    #[cfg(test)]
    pub(super) hierarchy_query_steps: usize,
}

pub(super) fn plan_scoped_edge_segments(
    index: &InputIndex<'_>,
    segments: &[ElkInputEdgeSegment],
) -> ImportResult<ScopedSegmentWorkPlan> {
    let mut materialization_path_steps = 0usize;
    let mut materialized_segments = 0usize;
    let mut label_selection_steps = 0usize;
    #[cfg(test)]
    let mut hierarchy_query_steps = 0usize;

    for scoped in segments {
        validate_scoped_segment_endpoint(index, &scoped.edge, &scoped.source)?;
        validate_scoped_segment_endpoint(index, &scoped.edge, &scoped.target)?;
        let source_placeholder = scoped_segment_endpoint_id(&scoped.source);
        let target_placeholder = scoped_segment_endpoint_id(&scoped.target);
        let source_depth = index.scoped_endpoint_depth(&scoped.edge, &scoped.source)?;
        let target_depth = index.scoped_endpoint_depth(&scoped.edge, &scoped.target)?;
        let shape = index.scoped_segment_shape(
            source_placeholder,
            target_placeholder,
            &scoped.source,
            &scoped.target,
            source_depth,
            target_depth,
        )?;
        #[cfg(test)]
        {
            hierarchy_query_steps = checked_sum([hierarchy_query_steps, shape.probe_steps])?;
        }
        let segment_count = checked_sum([
            source_depth
                .checked_sub(shape.common_depth)
                .ok_or(WorkError::ArithmeticOverflow)?,
            target_depth
                .checked_sub(shape.common_depth)
                .ok_or(WorkError::ArithmeticOverflow)?,
            usize::from(!shape.source_is_target_ancestor && !shape.target_is_source_ancestor),
        ])?
        .max(1);
        checked_sum([
            scoped_segment_depth(scoped.segment),
            source_depth.max(target_depth),
        ])?;
        let common_path_probe = usize::from(shape.common_depth < source_depth.min(target_depth));
        let materialization_steps = checked_sum([
            source_depth,
            target_depth,
            shape.common_depth,
            common_path_probe,
        ])?;
        materialization_path_steps =
            checked_sum([materialization_path_steps, materialization_steps])?;
        materialized_segments = checked_sum([materialized_segments, segment_count])?;
        label_selection_steps = checked_sum([
            label_selection_steps,
            checked_mul(segment_count, scoped.edge.labels.len())?,
        ])?;
    }

    Ok(ScopedSegmentWorkPlan {
        materialization_path_steps,
        materialized_segments,
        label_selection_steps,
        #[cfg(test)]
        hierarchy_query_steps,
    })
}

pub(super) fn materialize_scoped_edge_segments(
    index: &InputIndex<'_>,
    segments: &[ElkInputEdgeSegment],
    merge_edges: bool,
    work: ScopedSegmentWorkPlan,
) -> PreparedScopedSegments {
    let mut pending_segments = Vec::with_capacity(work.materialized_segments);

    for scoped in segments {
        let edge_order = scoped.edge_order;
        let source_port_key = hierarchy_port_key(
            scoped.edge.source.as_str(),
            edge_order,
            "source",
            merge_edges,
            PortType::Output,
        );
        let target_port_key = hierarchy_port_key(
            scoped.edge.target.as_str(),
            edge_order,
            "target",
            merge_edges,
            PortType::Input,
        );
        let source_placeholder = scoped_segment_endpoint_id(&scoped.source);
        let target_placeholder = scoped_segment_endpoint_id(&scoped.target);
        let source_path = match &scoped.source {
            ElkInputEdgeSegmentEndpoint::Node { id } => index.materialized_graph_path(id).expect(
                "scoped endpoint reachability was validated before materialization was charged",
            ),
            ElkInputEdgeSegmentEndpoint::ParentBoundary { .. } => Vec::new(),
        };
        let target_path = match &scoped.target {
            ElkInputEdgeSegmentEndpoint::Node { id } => index.materialized_graph_path(id).expect(
                "scoped endpoint reachability was validated before materialization was charged",
            ),
            ElkInputEdgeSegmentEndpoint::ParentBoundary { .. } => Vec::new(),
        };
        let mut local = source_ported_cross_hierarchy_segments(
            source_placeholder,
            target_placeholder,
            source_port_key.as_str(),
            target_port_key.as_str(),
            &source_path,
            &target_path,
        );
        if local.is_empty() {
            local.push(PendingCompoundSegment {
                graph_parent: None,
                source: PendingSegmentEndpoint::LocalNode {
                    node_id: source_placeholder.to_string(),
                    port_key: source_port_key.clone(),
                },
                target: PendingSegmentEndpoint::LocalNode {
                    node_id: target_placeholder.to_string(),
                    port_key: target_port_key.clone(),
                },
                segment: scoped.segment,
            });
        }

        for pending in &mut local {
            replace_scoped_boundary_endpoint(
                &mut pending.source,
                &scoped.source,
                true,
                source_port_key.as_str(),
                target_port_key.as_str(),
            );
            replace_scoped_boundary_endpoint(
                &mut pending.target,
                &scoped.target,
                false,
                source_port_key.as_str(),
                target_port_key.as_str(),
            );
            pending.segment = rebase_scoped_segment(pending.segment, scoped.segment);
        }

        let label_segments: Vec<_> = scoped
            .edge
            .labels
            .iter()
            .map(|label| compound_label_segment_index(&local, label.placement))
            .collect();
        for (segment_index, pending) in local.into_iter().enumerate() {
            let labels = scoped
                .edge
                .labels
                .iter()
                .zip(&label_segments)
                .filter(|(_, owner)| **owner == segment_index)
                .filter_map(|(label, _)| {
                    let mut label = label_to_lgraph(label)?;
                    label.original_label_edge = Some(scoped.edge.id.clone());
                    Some(label)
                })
                .collect();
            pending_segments.push(ScopedHierarchySegment {
                edge: HierarchyEdge {
                    id: scoped.edge.id.clone(),
                    source_node_id: scoped.edge.source.clone(),
                    target_node_id: scoped.edge.target.clone(),
                    source_port_key: source_port_key.clone(),
                    target_port_key: target_port_key.clone(),
                    source_path: Vec::new(),
                    target_path: Vec::new(),
                    labels: Vec::new(),
                    minlen: scoped.edge.minlen.max(1),
                    model_order: scoped.edge.model_order,
                    priority_direction: scoped.edge.priority_direction,
                    priority_shortness: scoped.edge.priority_shortness,
                    priority_straightness: scoped.edge.priority_straightness,
                },
                pending,
                labels,
            });
        }
    }

    debug_assert_eq!(pending_segments.len(), work.materialized_segments);
    PreparedScopedSegments {
        segments: pending_segments,
    }
}

fn scoped_segment_depth(segment: CompoundEdgeSegment) -> usize {
    match segment {
        CompoundEdgeSegment::Output { depth } | CompoundEdgeSegment::Input { depth } => depth,
    }
}

fn rebase_scoped_segment(
    local: CompoundEdgeSegment,
    scoped: CompoundEdgeSegment,
) -> CompoundEdgeSegment {
    // `local` is relative to this import scope, but `scoped` already carries the caller's absolute
    // compound side and depth. Preserve that side while rebasing every generated local piece.
    let depth = scoped_segment_depth(scoped)
        .checked_add(scoped_segment_depth(local))
        .expect("scoped segment depth was checked before materialization was charged");
    match scoped {
        CompoundEdgeSegment::Output { .. } => CompoundEdgeSegment::Output { depth },
        CompoundEdgeSegment::Input { .. } => CompoundEdgeSegment::Input { depth },
    }
}

pub(super) struct PreparedScopedSegments {
    pub(super) segments: Vec<ScopedHierarchySegment>,
}

fn validate_scoped_segment_endpoint(
    index: &InputIndex<'_>,
    edge: &ElkInputEdge,
    endpoint: &ElkInputEdgeSegmentEndpoint,
) -> ImportResult<()> {
    let ElkInputEdgeSegmentEndpoint::Node { id } = endpoint else {
        return Ok(());
    };
    if index.nodes.contains_key(id.as_str()) {
        return Ok(());
    }
    Err(ImportError::MissingEndpoint {
        edge_id: edge.id.clone(),
        node_id: id.clone(),
    })
}

fn scoped_segment_endpoint_id(endpoint: &ElkInputEdgeSegmentEndpoint) -> &str {
    match endpoint {
        ElkInputEdgeSegmentEndpoint::Node { id }
        | ElkInputEdgeSegmentEndpoint::ParentBoundary { id, .. } => id.as_str(),
    }
}

fn replace_scoped_boundary_endpoint(
    pending: &mut PendingSegmentEndpoint,
    endpoint: &ElkInputEdgeSegmentEndpoint,
    source: bool,
    source_port_key: &str,
    target_port_key: &str,
) {
    let ElkInputEdgeSegmentEndpoint::ParentBoundary { id, connects_node } = endpoint else {
        return;
    };
    let PendingSegmentEndpoint::LocalNode { node_id, .. } = pending else {
        return;
    };
    if node_id != id {
        return;
    }

    let (port_type, parent_port_type, port_key) = if source {
        (
            PortType::Input,
            if *connects_node {
                PortType::Output
            } else {
                PortType::Input
            },
            if *connects_node {
                source_port_key
            } else {
                target_port_key
            },
        )
    } else {
        (
            PortType::Output,
            if *connects_node {
                PortType::Input
            } else {
                PortType::Output
            },
            if *connects_node {
                target_port_key
            } else {
                source_port_key
            },
        )
    };
    *pending = PendingSegmentEndpoint::ParentBoundary {
        node_id: id.clone(),
        port_key: port_key.to_string(),
        port_type,
        parent_port_type,
        connects_parent_node: *connects_node,
    };
}

impl<'a> InputIndex<'a> {
    fn scoped_endpoint_depth(
        &self,
        edge: &ElkInputEdge,
        endpoint: &ElkInputEdgeSegmentEndpoint,
    ) -> ImportResult<usize> {
        let ElkInputEdgeSegmentEndpoint::Node { id } = endpoint else {
            return Ok(0);
        };
        self.container_depth(id).ok_or_else(|| {
            let graph_parent = self
                .first_unmaterialized_graph_parent(id)
                .unwrap_or(id.as_str());
            ImportError::UnavailableSegmentScope {
                edge_id: edge.id.clone(),
                graph_parent: graph_parent.to_string(),
            }
        })
    }

    fn scoped_segment_shape(
        &self,
        source: &str,
        target: &str,
        source_endpoint: &ElkInputEdgeSegmentEndpoint,
        target_endpoint: &ElkInputEdgeSegmentEndpoint,
        source_depth: usize,
        target_depth: usize,
    ) -> ImportResult<ScopedSegmentShape> {
        let source_owner = self.scoped_endpoint_owner_index(source_endpoint);
        let target_owner = self.scoped_endpoint_owner_index(target_endpoint);
        let (common_owner, common_probe_steps) = match (source_owner, target_owner) {
            (Some(source_owner), Some(target_owner)) => {
                self.hierarchy_queries.lca(source_owner, target_owner)?
            }
            _ => (None, 0),
        };
        let common_depth = match common_owner {
            Some(owner) => checked_sum([self.hierarchy_queries.depth(owner), 1])?,
            None => 0,
        };
        let (source_branch, source_probe_steps) =
            self.scoped_branch_owner(source_endpoint, source_depth, common_depth)?;
        let (target_branch, target_probe_steps) =
            self.scoped_branch_owner(target_endpoint, target_depth, common_depth)?;
        #[cfg(test)]
        let probe_steps =
            checked_sum([common_probe_steps, source_probe_steps, target_probe_steps])?;
        #[cfg(not(test))]
        let _ = (common_probe_steps, source_probe_steps, target_probe_steps);

        Ok(ScopedSegmentShape {
            common_depth,
            source_is_target_ancestor: target_depth > common_depth && target_branch == Some(source),
            target_is_source_ancestor: source_depth > common_depth && source_branch == Some(target),
            #[cfg(test)]
            probe_steps,
        })
    }

    fn scoped_endpoint_owner_index(&self, endpoint: &ElkInputEdgeSegmentEndpoint) -> Option<usize> {
        match endpoint {
            ElkInputEdgeSegmentEndpoint::Node { id } => self
                .node_parent(id)
                .and_then(|parent| self.node_index(parent)),
            ElkInputEdgeSegmentEndpoint::ParentBoundary { .. } => None,
        }
    }

    fn scoped_branch_owner(
        &self,
        endpoint: &ElkInputEdgeSegmentEndpoint,
        depth: usize,
        common_depth: usize,
    ) -> ImportResult<(Option<&'a str>, usize)> {
        if depth <= common_depth {
            return Ok((None, 0));
        }
        let owner = self
            .scoped_endpoint_owner_index(endpoint)
            .expect("positive scoped depth has an indexed graph owner");
        let steps = depth
            .checked_sub(common_depth)
            .and_then(|steps| steps.checked_sub(1))
            .ok_or(WorkError::ArithmeticOverflow)?;
        let (owner, probes) = self.hierarchy_queries.lift(owner, steps)?;
        Ok((owner.map(|owner| self.node_id(owner)), probes))
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct ScopedSegmentShape {
    common_depth: usize,
    source_is_target_ancestor: bool,
    target_is_source_ancestor: bool,
    #[cfg(test)]
    probe_steps: usize,
}
