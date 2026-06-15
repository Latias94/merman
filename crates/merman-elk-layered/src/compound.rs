//! Compound graph preprocessing support.
//!
//! Source references:
//! - https://github.com/eclipse-elk/elk/blob/62d5909f96fad541bc101ad52dabaece6b7eab7e/plugins/org.eclipse.elk.alg.layered/src/org/eclipse/elk/alg/layered/compound/CompoundGraphPreprocessor.java
//! - https://github.com/eclipse-elk/elk/blob/62d5909f96fad541bc101ad52dabaece6b7eab7e/plugins/org.eclipse.elk.alg.layered/src/org/eclipse/elk/alg/layered/compound/CrossHierarchyEdgeComparator.java

use crate::graph::{
    CompoundEdgeSegment, CrossHierarchyEdge, EdgeLabelPlacement, GraphNodeRef, GraphPortRef,
    LGraph, LNodeKind, PortRef, PortSide,
};
use crate::options::{ElkDirection, PortConstraints};

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

/// Compatibility entry point for the source-backed compound preprocessor boundary.
///
/// ELK runs `CompoundGraphPreprocessor` after import and before recursive layout. The current
/// Rust importer still creates hierarchy-local edge segments directly; this hook attaches the
/// source-backed cross-hierarchy contract to those imported segments at the correct pipeline stage.
pub fn preprocess_source_ported_compound_graph(graph: &mut LGraph) {
    preprocess_source_ported_compound_graph_inner(graph);
}

fn preprocess_source_ported_compound_graph_inner(graph: &mut LGraph) {
    let edge_count = graph.edges.len();
    for edge_index in 0..edge_count {
        let Some(segment) = graph.edges[edge_index].compound_segment else {
            continue;
        };
        ensure_cross_hierarchy_edge_record(graph, edge_index, segment);
        apply_source_ported_compound_endpoint_metadata(graph, edge_index, segment);
    }

    for node in &mut graph.layerless_nodes {
        if let Some(nested_graph) = node.nested_graph.as_deref_mut() {
            preprocess_source_ported_compound_graph_inner(nested_graph);
        }
    }
}

fn ensure_cross_hierarchy_edge_record(
    graph: &mut LGraph,
    edge_index: usize,
    segment: CompoundEdgeSegment,
) {
    let edge_id = graph.edges[edge_index].id.clone();
    if graph
        .cross_hierarchy_edges
        .iter()
        .any(|candidate| candidate.edge == edge_index)
    {
        return;
    }
    record_cross_hierarchy_edge_segment(graph, edge_id, edge_index, segment);
}

fn apply_source_ported_compound_endpoint_metadata(
    graph: &mut LGraph,
    edge_index: usize,
    segment: CompoundEdgeSegment,
) {
    let Some(edge) = graph.edges.get(edge_index) else {
        return;
    };
    let edge_id = edge.id.clone();
    let endpoint = match segment {
        CompoundEdgeSegment::Output { .. } => edge.source,
        CompoundEdgeSegment::Input { .. } => edge.target,
    };
    let Some(node) = graph
        .layerless_nodes
        .get(endpoint.node)
        .filter(|node| node.compound)
    else {
        return;
    };
    let nested_dummy = node
        .nested_graph
        .as_deref()
        .and_then(|nested| {
            external_dummy_for_compound_edge(nested, edge_id.as_str(), node.id.as_str())
        })
        .map(|dummy| (node.nested_graph.as_ref().unwrap().id.clone(), dummy));

    let Some(node) = graph.layerless_nodes.get_mut(endpoint.node) else {
        return;
    };
    node.port_constraints = PortConstraints::FixedSide;
    let port_side = match segment {
        CompoundEdgeSegment::Output { .. } => port_side_from_direction(graph.options.direction),
        CompoundEdgeSegment::Input { .. } => {
            port_side_from_direction(graph.options.direction).opposed()
        }
    };

    if let Some(port) = node.ports.get_mut(endpoint.port) {
        port.set_side(port_side);
    }

    if let Some((dummy_graph_id, dummy_node)) = nested_dummy {
        let origin_graph_id = graph.id.clone();
        link_external_port_dummy(graph, endpoint, dummy_graph_id, dummy_node);
        if let Some(nested_graph) = graph
            .layerless_nodes
            .get_mut(endpoint.node)
            .and_then(|node| node.nested_graph.as_deref_mut())
        {
            set_external_dummy_origin(nested_graph, dummy_node, origin_graph_id, endpoint);
        }
    }

    if graph.options.port_constraints.is_side_fixed() {
        graph.options.port_constraints = PortConstraints::FixedSide;
    } else {
        graph.options.port_constraints = PortConstraints::Free;
    }
    graph.graph_properties.non_free_ports = true;
}

fn external_dummy_for_compound_edge(
    nested_graph: &LGraph,
    edge_id: &str,
    compound_node_id: &str,
) -> Option<usize> {
    let dummy_id = format!("external:{compound_node_id}");
    nested_graph
        .layerless_nodes
        .iter()
        .enumerate()
        .find_map(|(node_index, node)| {
            (node.kind == LNodeKind::ExternalPort
                && node.id == dummy_id
                && node.ports.iter().any(|port| {
                    port.incoming_edges
                        .iter()
                        .chain(port.outgoing_edges.iter())
                        .any(|edge| nested_graph.edges[*edge].id == edge_id)
                }))
            .then_some(node_index)
        })
}

fn port_side_from_direction(direction: ElkDirection) -> PortSide {
    match direction {
        ElkDirection::Right | ElkDirection::Undefined => PortSide::East,
        ElkDirection::Left => PortSide::West,
        ElkDirection::Down => PortSide::South,
        ElkDirection::Up => PortSide::North,
    }
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
