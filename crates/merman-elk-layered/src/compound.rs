//! Compound graph preprocessing support.
//!
//! Source references:
//! - https://github.com/eclipse-elk/elk/blob/62d5909f96fad541bc101ad52dabaece6b7eab7e/plugins/org.eclipse.elk.alg.layered/src/org/eclipse/elk/alg/layered/compound/CompoundGraphPreprocessor.java
//! - https://github.com/eclipse-elk/elk/blob/62d5909f96fad541bc101ad52dabaece6b7eab7e/plugins/org.eclipse.elk.alg.layered/src/org/eclipse/elk/alg/layered/compound/CrossHierarchyEdgeComparator.java

use std::collections::HashMap;

use crate::graph::{
    CompoundEdgeSegment, CrossHierarchyEdge, EdgeLabelPlacement, GraphNodeRef, GraphPortRef,
    HierarchyEdge, LGraph, LLabel, LNodeKind, LPort, LSize, LayeredEdge, PortRef, PortSide,
    PortType, create_external_port_dummy,
};
use crate::options::{ElkDirection, PortConstraints};

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct PendingCompoundSegment {
    pub(crate) graph_parent: Option<String>,
    pub(crate) source: PendingSegmentEndpoint,
    pub(crate) target: PendingSegmentEndpoint,
    pub(crate) segment: CompoundEdgeSegment,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum PendingSegmentEndpoint {
    LocalNode {
        node_id: String,
        port_key: String,
    },
    ParentBoundary {
        node_id: String,
        port_key: String,
        port_type: PortType,
        parent_port_type: PortType,
        connects_parent_node: bool,
    },
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
    source_port_key: &str,
    target_port_key: &str,
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
        let connects_parent_node = target == source_path[depth - 1];
        segments.push(PendingCompoundSegment {
            graph_parent: Some(source_path[depth - 1].to_string()),
            source: PendingSegmentEndpoint::LocalNode {
                node_id: segment_source,
                port_key: source_port_key.to_string(),
            },
            target: PendingSegmentEndpoint::ParentBoundary {
                node_id: source_path[depth - 1].to_string(),
                port_key: if connects_parent_node {
                    target_port_key.to_string()
                } else {
                    source_port_key.to_string()
                },
                port_type: PortType::Output,
                parent_port_type: if connects_parent_node {
                    PortType::Input
                } else {
                    PortType::Output
                },
                connects_parent_node,
            },
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
            source: PendingSegmentEndpoint::LocalNode {
                node_id: segment_source,
                port_key: source_port_key.to_string(),
            },
            target: PendingSegmentEndpoint::LocalNode {
                node_id: segment_target,
                port_key: target_port_key.to_string(),
            },
            segment,
        });
    }

    for depth in common_depth + 1..=target_path.len() {
        let segment_target = if depth == target_path.len() {
            target.to_string()
        } else {
            target_path[depth].to_string()
        };
        let connects_parent_node = source == target_path[depth - 1];
        segments.push(PendingCompoundSegment {
            graph_parent: Some(target_path[depth - 1].to_string()),
            source: PendingSegmentEndpoint::ParentBoundary {
                node_id: target_path[depth - 1].to_string(),
                port_key: if connects_parent_node {
                    source_port_key.to_string()
                } else {
                    target_port_key.to_string()
                },
                port_type: PortType::Input,
                parent_port_type: if connects_parent_node {
                    PortType::Output
                } else {
                    PortType::Input
                },
                connects_parent_node,
            },
            target: PendingSegmentEndpoint::LocalNode {
                node_id: segment_target,
                port_key: target_port_key.to_string(),
            },
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
    dummy_side: PortSide,
    dummy_graph_id: impl Into<String>,
    dummy_node: usize,
    dummy_border_offset: Option<f64>,
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
    parent_port_data.set_side(dummy_side);
    if let Some(border_offset) = dummy_border_offset {
        parent_port_data.border_offset = Some(border_offset);
    }
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
    link_compound_external_dummy_metadata(graph);
    ensure_nested_external_dummies_for_parent_ports(graph);
}

fn introduce_source_ported_hierarchy_edge_segments(graph: &mut LGraph) {
    let hierarchy_edges = std::mem::take(&mut graph.hierarchy_edges);
    let mut external_ports = HashMap::new();
    for edge in hierarchy_edges {
        introduce_source_ported_hierarchy_edge(graph, &edge, &mut external_ports);
    }

    for node in &mut graph.layerless_nodes {
        if let Some(nested_graph) = node.nested_graph.as_deref_mut() {
            introduce_source_ported_hierarchy_edge_segments(nested_graph);
        }
    }
}

fn introduce_source_ported_hierarchy_edge(
    graph: &mut LGraph,
    edge: &HierarchyEdge,
    external_ports: &mut HashMap<ExternalPortKey, ExternalPort>,
) {
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
        edge.source_port_key.as_str(),
        edge.target_port_key.as_str(),
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
        let edge_index =
            introduce_hierarchical_edge_segment(graph, edge, &pending, labels, external_ports);
        record_cross_hierarchy_edge_segment(
            graph,
            edge.id.clone(),
            edge.model_order,
            edge_index,
            pending.segment,
        );
    }
}

#[derive(Debug, Clone)]
struct ExternalPort {
    original_edges: Vec<String>,
    new_edge: usize,
    dummy_node: usize,
    dummy_port: PortRef,
    port_type: PortType,
    exported: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
struct ExternalPortKey {
    graph_id: String,
    opposite_port_key: String,
    port_type: PortType,
}

/// Source-backed equivalent of ELK's `introduceHierarchicalEdgeSegment(...)`.
fn introduce_hierarchical_edge_segment(
    graph: &mut LGraph,
    edge: &HierarchyEdge,
    pending: &PendingCompoundSegment,
    labels: Vec<LLabel>,
    external_ports: &mut HashMap<ExternalPortKey, ExternalPort>,
) -> usize {
    let parent_boundary = match (&pending.source, &pending.target) {
        (
            PendingSegmentEndpoint::LocalNode { port_key, .. },
            PendingSegmentEndpoint::ParentBoundary {
                port_type,
                connects_parent_node,
                ..
            },
        ) => Some((*port_type, port_key.as_str(), *connects_parent_node)),
        (
            PendingSegmentEndpoint::ParentBoundary {
                port_type,
                connects_parent_node,
                ..
            },
            PendingSegmentEndpoint::LocalNode { port_key, .. },
        ) => Some((*port_type, port_key.as_str(), *connects_parent_node)),
        _ => None,
    };

    if let Some((port_type, opposite_port_key, connects_parent_node)) = parent_boundary
        && graph.options.merge_hierarchy_edges
        && !connects_parent_node
    {
        let key = ExternalPortKey {
            graph_id: graph.id.clone(),
            opposite_port_key: opposite_port_key.to_string(),
            port_type,
        };
        if let Some(external_port) = external_ports.get_mut(&key) {
            debug_assert_eq!(external_port.port_type, port_type);
            debug_assert!(external_port.exported);
            debug_assert_eq!(external_port.dummy_port.node, external_port.dummy_node);
            external_port.original_edges.push(edge.id.clone());
            debug_assert_eq!(external_port.original_edges.last(), Some(&edge.id));
            apply_label_graph_properties(graph, &labels);
            graph.edges[external_port.new_edge].labels.extend(labels);
            return external_port.new_edge;
        }
    }

    let source = ensure_segment_endpoint_port(graph, &pending.source, PortType::Output);
    let target = ensure_segment_endpoint_port(graph, &pending.target, PortType::Input);

    if source.node == target.node {
        graph.graph_properties.self_loops = true;
    }

    if has_incident_edges(&graph.layerless_nodes[source.node].ports[source.port])
        || has_incident_edges(&graph.layerless_nodes[target.node].ports[target.port])
    {
        graph.graph_properties.hyperedges = true;
    }

    apply_label_graph_properties(graph, &labels);

    let edge_index = graph
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
        .expect("ports were created before adding hierarchy edge segment");

    if let Some((port_type, opposite_port_key, connects_parent_node)) = parent_boundary {
        let dummy_port = match port_type {
            PortType::Output => target,
            PortType::Input => source,
        };
        if graph
            .layerless_nodes
            .get(dummy_port.node)
            .is_some_and(|node| node.kind == LNodeKind::ExternalPort)
        {
            let exported = !connects_parent_node;
            let external_port = ExternalPort {
                original_edges: vec![edge.id.clone()],
                new_edge: edge_index,
                dummy_node: dummy_port.node,
                dummy_port,
                port_type,
                exported,
            };
            if exported {
                external_ports.insert(
                    ExternalPortKey {
                        graph_id: graph.id.clone(),
                        opposite_port_key: opposite_port_key.to_string(),
                        port_type,
                    },
                    external_port,
                );
            }
        }
    }

    edge_index
}

fn apply_label_graph_properties(graph: &mut LGraph, labels: &[LLabel]) {
    for label in labels {
        match label.placement {
            EdgeLabelPlacement::Center => graph.graph_properties.center_labels = true,
            EdgeLabelPlacement::Head | EdgeLabelPlacement::Tail => {
                graph.graph_properties.end_labels = true;
            }
        }
    }
}

fn ensure_segment_endpoint_port(
    graph: &mut LGraph,
    endpoint: &PendingSegmentEndpoint,
    port_type: PortType,
) -> PortRef {
    match endpoint {
        PendingSegmentEndpoint::LocalNode { node_id, port_key } => {
            ensure_local_node_port(graph, node_id.as_str(), port_key.as_str(), port_type)
                .expect("compound segment local endpoint should exist in the current graph")
        }
        PendingSegmentEndpoint::ParentBoundary { node_id, .. } => create_parent_boundary_port(
            graph,
            node_id.as_str(),
            endpoint_port_type(endpoint, port_type),
            endpoint_parent_port_type(endpoint, port_type),
            endpoint_port_key(endpoint).unwrap_or_default(),
            endpoint_connects_parent_node(endpoint),
        ),
    }
}

fn endpoint_port_type(endpoint: &PendingSegmentEndpoint, fallback: PortType) -> PortType {
    match endpoint {
        PendingSegmentEndpoint::ParentBoundary { port_type, .. } => *port_type,
        PendingSegmentEndpoint::LocalNode { .. } => fallback,
    }
}

fn endpoint_parent_port_type(endpoint: &PendingSegmentEndpoint, fallback: PortType) -> PortType {
    match endpoint {
        PendingSegmentEndpoint::ParentBoundary {
            parent_port_type, ..
        } => *parent_port_type,
        PendingSegmentEndpoint::LocalNode { .. } => fallback,
    }
}

fn endpoint_port_key(endpoint: &PendingSegmentEndpoint) -> Option<&str> {
    match endpoint {
        PendingSegmentEndpoint::ParentBoundary { port_key, .. }
        | PendingSegmentEndpoint::LocalNode { port_key, .. } => Some(port_key.as_str()),
    }
}

fn endpoint_connects_parent_node(endpoint: &PendingSegmentEndpoint) -> bool {
    match endpoint {
        PendingSegmentEndpoint::ParentBoundary {
            connects_parent_node,
            ..
        } => *connects_parent_node,
        PendingSegmentEndpoint::LocalNode { .. } => false,
    }
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

fn ensure_nested_external_dummies_for_parent_ports(graph: &mut LGraph) {
    let graph_id = graph.id.clone();
    let node_count = graph.layerless_nodes.len();

    for node_index in 0..node_count {
        if let Some(nested_graph) = graph.layerless_nodes[node_index]
            .nested_graph
            .as_deref_mut()
        {
            ensure_nested_external_dummies_for_parent_ports(nested_graph);
        }

        let Some(nested_graph) = graph.layerless_nodes[node_index].nested_graph.as_ref() else {
            continue;
        };
        if !nested_graph.graph_properties.external_ports {
            continue;
        }

        let parent_node_id = graph.layerless_nodes[node_index].id.clone();
        let parent_constraints = graph.layerless_nodes[node_index].port_constraints;
        let parent_size = graph.layerless_nodes[node_index].size;
        let nested_graph_id = nested_graph.id.clone();
        let nested_direction = nested_graph.options.direction;
        let port_count = graph.layerless_nodes[node_index].ports.len();

        for port_index in 0..port_count {
            if graph.layerless_nodes[node_index].ports[port_index]
                .port_dummy
                .is_some()
            {
                continue;
            }

            let parent_port = PortRef {
                node: node_index,
                port: port_index,
            };
            let port = &graph.layerless_nodes[node_index].ports[port_index];
            let port_constraints = parent_external_dummy_constraints(parent_constraints, port.side);
            let mut dummy = create_external_port_dummy(
                format!("external:{parent_node_id}"),
                format!("external:{parent_node_id}:0"),
                port.port_type,
                port_constraints,
                port.side,
                -port.net_flow(),
                port.position,
                port.size,
                parent_size,
                port.border_offset.unwrap_or(0.0),
                nested_direction,
            );
            let dummy_side = dummy.external_port_side;
            let dummy_border_offset = dummy.ports.first().and_then(|port| port.border_offset);
            let dummy_node = {
                let nested_graph = graph.layerless_nodes[node_index]
                    .nested_graph
                    .as_deref_mut()
                    .expect("nested graph existence checked above");
                let dummy_node = nested_graph.layerless_nodes.len();
                dummy.ports[0].node = dummy_node;
                nested_graph.layerless_nodes.push(dummy);
                dummy_node
            };

            link_external_port_dummy(
                graph,
                parent_port,
                dummy_side,
                nested_graph_id.clone(),
                dummy_node,
                dummy_border_offset,
            );
            if let Some(nested_graph) = graph.layerless_nodes[node_index]
                .nested_graph
                .as_deref_mut()
            {
                set_external_dummy_origin(nested_graph, dummy_node, graph_id.clone(), parent_port);
            }
        }
    }
}

fn parent_external_dummy_constraints(
    parent_constraints: PortConstraints,
    port_side: PortSide,
) -> PortConstraints {
    if parent_constraints.is_side_fixed() && port_side == PortSide::Undefined {
        PortConstraints::Free
    } else {
        parent_constraints
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
    let nested_dummy = node.nested_graph.as_deref().and_then(|nested| {
        external_dummy_for_compound_edge(nested, edge_id.as_str(), node.id.as_str()).map(|dummy| {
            (
                nested.id.clone(),
                dummy,
                nested.layerless_nodes[dummy].external_port_side,
                nested.layerless_nodes[dummy]
                    .ports
                    .first()
                    .and_then(|port| port.border_offset),
            )
        })
    });

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

    if let Some((dummy_graph_id, dummy_node, dummy_side, dummy_border_offset)) = nested_dummy {
        let origin_graph_id = graph.id.clone();
        link_external_port_dummy(
            graph,
            endpoint,
            dummy_side,
            dummy_graph_id,
            dummy_node,
            dummy_border_offset,
        );
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

#[derive(Debug, Clone)]
struct ExternalDummyInfo {
    dummy_node: usize,
    incident_edge_ids: Vec<String>,
    port_type: PortType,
    parent_port_key: String,
    parent_port_type: PortType,
    external_port_side: PortSide,
    border_offset: Option<f64>,
    origin_port: Option<GraphPortRef>,
}

fn link_compound_external_dummy_metadata(graph: &mut LGraph) {
    let graph_id = graph.id.clone();
    let node_count = graph.layerless_nodes.len();

    for node_index in 0..node_count {
        let Some(nested_graph) = graph.layerless_nodes[node_index].nested_graph.as_deref() else {
            continue;
        };
        let parent_node_id = graph.layerless_nodes[node_index].id.clone();
        let nested_graph_id = nested_graph.id.clone();
        let external_dummies =
            external_dummies_for_parent_node(nested_graph, parent_node_id.as_str());

        for external_dummy in external_dummies {
            let parent_port = parent_port_for_external_dummy(graph, node_index, &external_dummy)
                .unwrap_or_else(|| {
                    create_parent_external_port(
                        graph,
                        node_index,
                        external_dummy.port_type,
                        external_dummy.external_port_side,
                        Some(external_dummy.parent_port_key.as_str()),
                    )
                });
            link_external_port_dummy(
                graph,
                parent_port,
                external_dummy.external_port_side,
                nested_graph_id.clone(),
                external_dummy.dummy_node,
                external_dummy.border_offset,
            );

            if let Some(nested_graph) = graph.layerless_nodes[node_index]
                .nested_graph
                .as_deref_mut()
            {
                set_external_dummy_origin(
                    nested_graph,
                    external_dummy.dummy_node,
                    graph_id.clone(),
                    parent_port,
                );
            }
        }

        if let Some(nested_graph) = graph.layerless_nodes[node_index]
            .nested_graph
            .as_deref_mut()
        {
            link_compound_external_dummy_metadata(nested_graph);
        }
    }
}

fn external_dummies_for_parent_node(
    nested_graph: &LGraph,
    parent_node_id: &str,
) -> Vec<ExternalDummyInfo> {
    let dummy_id = format!("external:{parent_node_id}");
    nested_graph
        .layerless_nodes
        .iter()
        .enumerate()
        .filter(|(_, node)| node.kind == LNodeKind::ExternalPort && node.id == dummy_id)
        .filter_map(|(dummy_node, node)| {
            let port = node.ports.first()?;
            let incident_edge_ids = port
                .incoming_edges
                .iter()
                .chain(port.outgoing_edges.iter())
                .map(|edge| nested_graph.edges[*edge].id.clone())
                .collect::<Vec<_>>();
            Some(ExternalDummyInfo {
                dummy_node,
                incident_edge_ids,
                port_type: port.port_type,
                parent_port_key: node
                    .parent_port_key
                    .clone()
                    .unwrap_or_else(|| port.id.clone()),
                parent_port_type: node.parent_port_type.unwrap_or(port.port_type),
                external_port_side: node.external_port_side,
                border_offset: port.border_offset,
                origin_port: node.origin_port.clone(),
            })
        })
        .collect()
}

fn parent_port_for_external_dummy(
    graph: &LGraph,
    parent_node: usize,
    external_dummy: &ExternalDummyInfo,
) -> Option<PortRef> {
    if let Some(origin_port) = external_dummy.origin_port.as_ref()
        && origin_port.graph_id == graph.id
        && origin_port.port.node == parent_node
        && graph
            .layerless_nodes
            .get(parent_node)
            .and_then(|node| node.ports.get(origin_port.port.port))
            .is_some()
    {
        return Some(origin_port.port);
    }

    if !external_dummy.parent_port_key.is_empty()
        && let Some(port) = graph
            .layerless_nodes
            .get(parent_node)?
            .ports
            .iter()
            .position(|port| {
                port.id == external_dummy.parent_port_key
                    && port.port_type == external_dummy.parent_port_type
            })
    {
        return Some(PortRef {
            node: parent_node,
            port,
        });
    }

    graph
        .layerless_nodes
        .get(parent_node)?
        .ports
        .iter()
        .enumerate()
        .find_map(|(port_index, port)| {
            let matches_edge = port
                .incoming_edges
                .iter()
                .chain(port.outgoing_edges.iter())
                .any(|edge| {
                    external_dummy
                        .incident_edge_ids
                        .contains(&graph.edges[*edge].id)
                });
            matches_edge.then_some(PortRef {
                node: parent_node,
                port: port_index,
            })
        })
}

fn create_parent_external_port(
    graph: &mut LGraph,
    parent_node: usize,
    port_type: PortType,
    port_side: PortSide,
    port_key: Option<&str>,
) -> PortRef {
    let port_side = if port_side == PortSide::Undefined {
        match port_type {
            PortType::Output => port_side_from_direction(graph.options.direction),
            PortType::Input => port_side_from_direction(graph.options.direction).opposed(),
        }
    } else {
        port_side
    };
    graph
        .add_port(parent_node, port_type, port_side, Default::default())
        .map(|port| {
            if let Some(port_key) = port_key
                && !port_key.is_empty()
                && let Some(port_data) = graph.layerless_nodes[parent_node].ports.get_mut(port.port)
            {
                port_data.id = port_key.to_string();
            }
            port
        })
        .expect("parent compound node should exist when linking external dummy")
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

fn ensure_local_node_port(
    graph: &mut LGraph,
    node_id: &str,
    port_key: &str,
    port_type: PortType,
) -> Option<PortRef> {
    if let Some(node) = graph
        .layerless_nodes
        .iter()
        .position(|candidate| candidate.id == node_id)
    {
        if let Some(port) = graph.layerless_nodes[node]
            .ports
            .iter()
            .position(|candidate| candidate.id == port_key && candidate.port_type == port_type)
        {
            return Some(PortRef { node, port });
        }

        let port = graph.layerless_nodes[node].ports.len();
        graph.layerless_nodes[node]
            .ports
            .push(LPort::new(port_key.to_string(), node, port_type));
        return Some(PortRef { node, port });
    }
    None
}

fn create_parent_boundary_port(
    graph: &mut LGraph,
    parent_node_id: &str,
    dummy_port_type: PortType,
    parent_port_type: PortType,
    parent_port_key: &str,
    _connects_parent_node: bool,
) -> PortRef {
    graph.graph_properties.external_ports = true;
    graph.graph_properties.non_free_ports = true;
    graph.options.port_constraints = if graph.options.port_constraints.is_side_fixed() {
        PortConstraints::FixedSide
    } else {
        PortConstraints::Free
    };
    let border_offset = graph.options.spacing.edge_edge / 2.0;
    let mut dummy = create_external_port_dummy(
        format!("external:{parent_node_id}"),
        if parent_port_key.is_empty() {
            format!("external:{parent_node_id}:0")
        } else {
            parent_port_key.to_string()
        },
        dummy_port_type,
        graph.options.port_constraints,
        PortSide::Undefined,
        match dummy_port_type {
            PortType::Input => -1,
            PortType::Output => 1,
        },
        Default::default(),
        LSize::default(),
        LSize::default(),
        border_offset,
        graph.options.direction,
    );
    let node = graph.layerless_nodes.len();
    dummy.parent_port_key = (!parent_port_key.is_empty()).then(|| parent_port_key.to_string());
    dummy.parent_port_type = Some(parent_port_type);
    dummy.ports[0].node = node;
    graph.layerless_nodes.push(dummy);
    PortRef { node, port: 0 }
}

fn has_incident_edges(port: &LPort) -> bool {
    port.incoming_edges.len() + port.outgoing_edges.len() > 0
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
        let segments = source_ported_cross_hierarchy_segments(
            "A",
            "B",
            "A:source",
            "B:target",
            &["outer", "inner"],
            &["sibling"],
        );

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
    fn source_ported_segments_mark_parent_boundary_endpoints() {
        let segments = source_ported_cross_hierarchy_segments(
            "A",
            "B",
            "A:source",
            "B:target",
            &["outer", "inner"],
            &["sibling"],
        );

        assert!(matches!(
            &segments[0].target,
            PendingSegmentEndpoint::ParentBoundary {
                node_id,
                port_key,
                connects_parent_node: false,
                ..
            } if node_id == "inner" && port_key == "A:source"
        ));
        assert!(matches!(
            &segments[2].target,
            PendingSegmentEndpoint::LocalNode { node_id, port_key }
                if node_id == "sibling" && port_key == "B:target"
        ));
        assert!(matches!(
            &segments[3].source,
            PendingSegmentEndpoint::ParentBoundary {
                node_id,
                port_key,
                connects_parent_node: false,
                ..
            } if node_id == "sibling" && port_key == "B:target"
        ));
    }

    #[test]
    fn center_label_uses_shallowest_segment() {
        let segments = source_ported_cross_hierarchy_segments(
            "A",
            "B",
            "A:source",
            "B:target",
            &["outer", "inner"],
            &["sibling"],
        );

        let label_index = compound_label_segment_index(&segments, EdgeLabelPlacement::Center);

        assert_eq!(
            segments[label_index].segment,
            CompoundEdgeSegment::Output { depth: 0 }
        );
    }
}
