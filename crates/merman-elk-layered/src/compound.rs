//! Compound graph preprocessing support.
//!
//! Source references:
//! - https://github.com/eclipse-elk/elk/blob/62d5909f96fad541bc101ad52dabaece6b7eab7e/plugins/org.eclipse.elk.alg.layered/src/org/eclipse/elk/alg/layered/compound/CompoundGraphPreprocessor.java
//! - https://github.com/eclipse-elk/elk/blob/62d5909f96fad541bc101ad52dabaece6b7eab7e/plugins/org.eclipse.elk.alg.layered/src/org/eclipse/elk/alg/layered/compound/CrossHierarchyEdgeComparator.java

use crate::graph::{
    CompoundEdgeSegment, CrossHierarchyEdge, EdgeLabelPlacement, GraphNodeRef, GraphPortRef,
    LGraph, PortRef,
};
use crate::options::PortConstraints;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct PendingCompoundSegment {
    pub(crate) graph_parent: Option<String>,
    pub(crate) source: String,
    pub(crate) target: String,
    pub(crate) segment: CompoundEdgeSegment,
}

/// Build the graph-local segments for a hierarchy-crossing edge.
///
/// This is the current compatibility bridge toward ELK's `CompoundGraphPreprocessor`: the importer
/// still calls it directly, but the segment ordering and label-placement semantics live in this
/// compound boundary instead of being spread through `ElkGraphImporter` code.
pub(crate) fn source_ported_cross_hierarchy_segments(
    source: &str,
    target: &str,
    source_path: &[&str],
    target_path: &[&str],
) -> Vec<PendingCompoundSegment> {
    let common_depth = common_graph_depth(source_path, target_path);
    let source_is_target_ancestor =
        target_path.len() > common_depth && source == target_path[common_depth];
    let target_is_source_ancestor =
        source_path.len() > common_depth && target == source_path[common_depth];

    let mut segments = Vec::new();

    for depth in (common_depth + 1..=source_path.len()).rev() {
        let segment_source = if depth == source_path.len() {
            source.to_string()
        } else {
            source_path[depth].to_string()
        };
        segments.push(PendingCompoundSegment {
            graph_parent: Some(source_path[depth - 1].to_string()),
            source: segment_source,
            target: source_path[depth - 1].to_string(),
            segment: CompoundEdgeSegment::Output { depth },
        });
    }

    if !source_is_target_ancestor && !target_is_source_ancestor {
        let segment_source = if source_path.len() > common_depth {
            source_path[common_depth].to_string()
        } else {
            source.to_string()
        };
        let segment_target = if target_path.len() > common_depth {
            target_path[common_depth].to_string()
        } else {
            target.to_string()
        };
        let segment = if source_path.len() > common_depth {
            CompoundEdgeSegment::Output {
                depth: common_depth,
            }
        } else {
            CompoundEdgeSegment::Input {
                depth: common_depth,
            }
        };
        segments.push(PendingCompoundSegment {
            graph_parent: common_depth
                .checked_sub(1)
                .map(|parent_depth| source_path[parent_depth].to_string()),
            source: segment_source,
            target: segment_target,
            segment,
        });
    }

    for depth in common_depth + 1..=target_path.len() {
        let segment_target = if depth == target_path.len() {
            target.to_string()
        } else {
            target_path[depth].to_string()
        };
        segments.push(PendingCompoundSegment {
            graph_parent: Some(target_path[depth - 1].to_string()),
            source: target_path[depth - 1].to_string(),
            target: segment_target,
            segment: CompoundEdgeSegment::Input { depth },
        });
    }

    segments
}

pub(crate) fn compound_label_segment_index(
    segments: &[PendingCompoundSegment],
    placement: EdgeLabelPlacement,
) -> usize {
    let mut sorted = segments
        .iter()
        .enumerate()
        .map(|(index, segment)| (index, segment.segment))
        .collect::<Vec<_>>();
    sorted.sort_by(|(_, left), (_, right)| compare_compound_segments(*left, *right));

    match placement {
        EdgeLabelPlacement::Tail => sorted.first().map(|(index, _)| *index).unwrap_or(0),
        EdgeLabelPlacement::Head => sorted.last().map(|(index, _)| *index).unwrap_or(0),
        EdgeLabelPlacement::Center => sorted
            .iter()
            .position(|(_, segment)| matches!(segment, CompoundEdgeSegment::Input { .. }))
            .map(|index| index.saturating_sub(1))
            .or_else(|| sorted.len().checked_sub(1))
            .and_then(|index| sorted.get(index).map(|(segment_index, _)| *segment_index))
            .unwrap_or(0),
    }
}

/// Mirror `CompoundGraphPreprocessor#setSidesOfPortsToSidesOfDummyNodes` for an exported
/// compound port / external-port dummy pair.
///
/// The current importer still creates compatibility segments directly, but this keeps the ELK
/// `ORIGIN`, `PORT_DUMMY`, and `INSIDE_CONNECTIONS` semantics represented in the graph model.
pub(crate) fn link_external_port_dummy(
    parent_graph: &mut LGraph,
    parent_port: PortRef,
    dummy_graph_id: impl Into<String>,
    dummy_node: usize,
) {
    let dummy_graph_id = dummy_graph_id.into();
    let Some(parent_node) = parent_graph.layerless_nodes.get_mut(parent_port.node) else {
        return;
    };
    let Some(parent_port_data) = parent_node.ports.get_mut(parent_port.port) else {
        return;
    };

    parent_port_data.port_dummy = Some(GraphNodeRef {
        graph_id: dummy_graph_id,
        node: dummy_node,
    });
    parent_port_data.inside_connections = true;
    parent_node.port_constraints = PortConstraints::FixedSide;
    parent_graph.graph_properties.non_free_ports = true;
}

pub(crate) fn set_external_dummy_origin(
    dummy_graph: &mut LGraph,
    dummy_node: usize,
    origin_graph_id: impl Into<String>,
    origin_port: PortRef,
) {
    let Some(dummy) = dummy_graph.layerless_nodes.get_mut(dummy_node) else {
        return;
    };
    dummy.origin_port = Some(GraphPortRef {
        graph_id: origin_graph_id.into(),
        port: origin_port,
    });
}

pub(crate) fn record_cross_hierarchy_edge_segment(
    graph: &mut LGraph,
    original_edge_id: impl Into<String>,
    edge: usize,
    segment: CompoundEdgeSegment,
) {
    graph.cross_hierarchy_edges.push(CrossHierarchyEdge {
        original_edge_id: original_edge_id.into(),
        graph_id: graph.id.clone(),
        edge,
        segment,
    });
}

pub fn compare_compound_segments(
    left: CompoundEdgeSegment,
    right: CompoundEdgeSegment,
) -> std::cmp::Ordering {
    match (left, right) {
        (CompoundEdgeSegment::Output { .. }, CompoundEdgeSegment::Input { .. }) => {
            std::cmp::Ordering::Less
        }
        (CompoundEdgeSegment::Input { .. }, CompoundEdgeSegment::Output { .. }) => {
            std::cmp::Ordering::Greater
        }
        (
            CompoundEdgeSegment::Output { depth: left },
            CompoundEdgeSegment::Output { depth: right },
        ) => right.cmp(&left),
        (
            CompoundEdgeSegment::Input { depth: left },
            CompoundEdgeSegment::Input { depth: right },
        ) => left.cmp(&right),
    }
}

fn common_graph_depth(left: &[&str], right: &[&str]) -> usize {
    left.iter()
        .zip(right.iter())
        .take_while(|(left, right)| left == right)
        .count()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn source_ported_segments_sort_like_cross_hierarchy_edge_comparator() {
        let segments =
            source_ported_cross_hierarchy_segments("A", "B", &["outer", "inner"], &["sibling"]);

        let mut sorted = segments
            .iter()
            .map(|segment| segment.segment)
            .collect::<Vec<_>>();
        sorted.sort_by(|left, right| compare_compound_segments(*left, *right));

        assert_eq!(
            sorted,
            vec![
                CompoundEdgeSegment::Output { depth: 2 },
                CompoundEdgeSegment::Output { depth: 1 },
                CompoundEdgeSegment::Output { depth: 0 },
                CompoundEdgeSegment::Input { depth: 1 },
            ]
        );
    }

    #[test]
    fn center_label_uses_shallowest_segment() {
        let segments =
            source_ported_cross_hierarchy_segments("A", "B", &["outer", "inner"], &["sibling"]);

        let label_index = compound_label_segment_index(&segments, EdgeLabelPlacement::Center);

        assert_eq!(
            segments[label_index].segment,
            CompoundEdgeSegment::Output { depth: 0 }
        );
    }
}
