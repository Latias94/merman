//! Compound graph preprocessing support.
//!
//! Source references:
//! - https://github.com/eclipse-elk/elk/blob/62d5909f96fad541bc101ad52dabaece6b7eab7e/plugins/org.eclipse.elk.alg.layered/src/org/eclipse/elk/alg/layered/compound/CompoundGraphPreprocessor.java
//! - https://github.com/eclipse-elk/elk/blob/62d5909f96fad541bc101ad52dabaece6b7eab7e/plugins/org.eclipse.elk.alg.layered/src/org/eclipse/elk/alg/layered/compound/CrossHierarchyEdgeComparator.java

use crate::graph::{
    CompoundEdgeSegment, CrossHierarchyEdge, EdgeLabelPlacement, GraphNodeRef, GraphPortRef,
    HierarchyEdge, LGraph, LLabel, LNodeKind, LPort, LSize, LayeredEdge, PortRef, PortSide,
    PortType, create_external_port_dummy,
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
/// This is the current segment-building core of the Rust `CompoundGraphPreprocessor` port. The
/// full ELK algorithm introduces segments through `ExternalPort` records while walking the graph
/// recursively; this helper keeps segment ordering and label-placement semantics in the compound
/// boundary while that recursive port is filled in.
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
/// This keeps the ELK `ORIGIN`, `PORT_DUMMY`, and `INSIDE_CONNECTIONS` semantics represented in
/// the graph model for segment endpoints introduced by compound preprocessing.
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
    original_model_order: Option<usize>,
    edge: usize,
    segment: CompoundEdgeSegment,
) {
    graph.cross_hierarchy_edges.push(CrossHierarchyEdge {
        original_edge_id: original_edge_id.into(),
        original_model_order,
        graph_id: graph.id.clone(),
        edge,
        segment,
    });
}

/// Compatibility entry point for the source-backed compound preprocessor boundary.
///
/// ELK runs `CompoundGraphPreprocessor` after import and before recursive layout. The Rust port now
/// keeps hierarchy-crossing input edges as `HierarchyEdge` records during import and introduces
/// hierarchy-local layout segments here. The second pass still accepts already segmented edges as a
/// migration bridge for tests and later postprocessor work.
pub fn preprocess_source_ported_compound_graph(graph: &mut LGraph) {
    introduce_source_ported_hierarchy_edge_segments(graph);
    preprocess_source_ported_compound_graph_inner(graph);
}

fn introduce_source_ported_hierarchy_edge_segments(graph: &mut LGraph) {
    let hierarchy_edges = std::mem::take(&mut graph.hierarchy_edges);
    for edge in hierarchy_edges {
        introduce_source_ported_hierarchy_edge(graph, &edge);
    }

    for node in &mut graph.layerless_nodes {
        if let Some(nested_graph) = node.nested_graph.as_deref_mut() {
            introduce_source_ported_hierarchy_edge_segments(nested_graph);
        }
    }
}

fn introduce_source_ported_hierarchy_edge(graph: &mut LGraph, edge: &HierarchyEdge) {
    let source_path = edge
        .source_path
        .iter()
        .map(String::as_str)
        .collect::<Vec<_>>();
    let target_path = edge
        .target_path
        .iter()
        .map(String::as_str)
        .collect::<Vec<_>>();
    let segments = source_ported_cross_hierarchy_segments(
        edge.source_node_id.as_str(),
        edge.target_node_id.as_str(),
        &source_path,
        &target_path,
    );
    let label_segments = edge
        .labels
        .iter()
        .map(|label| compound_label_segment_index(&segments, label.placement))
        .collect::<Vec<_>>();

    for (segment_index, pending) in segments.into_iter().enumerate() {
        let labels = edge
            .labels
            .iter()
            .zip(label_segments.iter())
            .filter_map(|(label, label_segment)| {
                (*label_segment == segment_index).then(|| {
                    let mut label = label.clone();
                    label.original_label_edge = Some(edge.id.clone());
                    label
                })
            })
            .collect::<Vec<_>>();
        let graph = graph_for_parent_mut(graph, pending.graph_parent.as_deref());
        let edge_index = add_source_ported_hierarchy_edge_segment(graph, edge, &pending, labels);
        record_cross_hierarchy_edge_segment(
            graph,
            edge.id.clone(),
            edge.model_order,
            edge_index,
            pending.segment,
        );
    }
}

fn add_source_ported_hierarchy_edge_segment(
    graph: &mut LGraph,
    edge: &HierarchyEdge,
    pending: &PendingCompoundSegment,
    labels: Vec<LLabel>,
) -> usize {
    let source = ensure_port(graph, pending.source.as_str(), PortType::Output);
    let target = ensure_port(graph, pending.target.as_str(), PortType::Input);

    if source.node == target.node {
        graph.graph_properties.self_loops = true;
    }

    if has_parallel_port_edges(&graph.layerless_nodes[source.node].ports[source.port])
        || has_parallel_port_edges(&graph.layerless_nodes[target.node].ports[target.port])
    {
        graph.graph_properties.hyperedges = true;
    }

    for label in &labels {
        match label.placement {
            EdgeLabelPlacement::Center => graph.graph_properties.center_labels = true,
            EdgeLabelPlacement::Head | EdgeLabelPlacement::Tail => {
                graph.graph_properties.end_labels = true;
            }
        }
    }

    graph
        .add_edge(LayeredEdge {
            id: edge.id.clone(),
            source,
            target,
            source_node_id: edge.source_node_id.clone(),
            target_node_id: edge.target_node_id.clone(),
            labels,
            minlen: edge.minlen,
            reversed: false,
            bend_points: Vec::new(),
            model_order: edge.model_order,
            priority_direction: edge.priority_direction,
            priority_shortness: edge.priority_shortness,
            priority_straightness: edge.priority_straightness,
            thickness: 0.0,
            original_opposite_port: None,
            compound_segment: Some(pending.segment),
        })
        .expect("ports were created before adding hierarchy edge segment")
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
    record_cross_hierarchy_edge_segment(
        graph,
        edge_id,
        graph.edges[edge_index].model_order,
        edge_index,
        segment,
    );
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

fn ensure_port(graph: &mut LGraph, node_id: &str, port_type: PortType) -> PortRef {
    if let Some(node) = graph
        .layerless_nodes
        .iter()
        .position(|candidate| candidate.id == node_id)
    {
        let port = graph.layerless_nodes[node].ports.len();
        graph.layerless_nodes[node].ports.push(LPort::new(
            format!("{node_id}:{port:?}"),
            node,
            port_type,
        ));
        return PortRef { node, port };
    }

    graph.graph_properties.external_ports = true;
    let mut dummy = create_external_port_dummy(
        format!("external:{node_id}"),
        format!("external:{node_id}:0"),
        port_type,
        PortConstraints::Free,
        PortSide::Undefined,
        match port_type {
            PortType::Input => 1,
            PortType::Output => -1,
        },
        Default::default(),
        LSize::default(),
        LSize::default(),
        0.0,
        graph.options.direction,
    );
    let node = graph.layerless_nodes.len();
    dummy.ports[0].node = node;
    graph.layerless_nodes.push(dummy);
    PortRef { node, port: 0 }
}

fn has_parallel_port_edges(port: &LPort) -> bool {
    port.incoming_edges.len() + port.outgoing_edges.len() > 1
}

fn graph_for_parent_mut<'a>(graph: &'a mut LGraph, parent: Option<&str>) -> &'a mut LGraph {
    let Some(parent) = parent else {
        return graph;
    };

    let path = graph_path_for_parent(graph, parent);
    match path {
        Some(path) => graph_mut_at_path(graph, &path),
        None => graph,
    }
}

fn graph_path_for_parent(graph: &LGraph, parent: &str) -> Option<Vec<usize>> {
    for (index, node) in graph.layerless_nodes.iter().enumerate() {
        let Some(nested_graph) = node.nested_graph.as_deref() else {
            continue;
        };
        if node.id == parent || nested_graph.id == parent {
            return Some(vec![index]);
        }
        if let Some(mut path) = graph_path_for_parent(nested_graph, parent) {
            path.insert(0, index);
            return Some(path);
        }
    }
    None
}

fn graph_mut_at_path<'a>(mut graph: &'a mut LGraph, path: &[usize]) -> &'a mut LGraph {
    for index in path {
        graph = graph.layerless_nodes[*index]
            .nested_graph
            .as_deref_mut()
            .expect("graph path should only contain nested graph nodes");
    }
    graph
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
