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
use crate::work::{WorkError, checked_sum};

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

#[derive(Debug, Clone)]
pub(crate) struct ScopedHierarchySegment {
    pub(crate) edge: HierarchyEdge,
    pub(crate) pending: PendingCompoundSegment,
    pub(crate) labels: Vec<LLabel>,
}

/// Build the graph-local segments for a hierarchy-crossing edge.
///
/// This is the current segment-building core of the Rust `CompoundGraphPreprocessor` port. The
/// full ELK algorithm introduces segments through `ExternalPort` records while walking the graph
/// recursively; this helper keeps segment ordering and label-placement semantics in the compound
/// boundary while that recursive port is filled in.
pub(crate) fn source_ported_cross_hierarchy_segments<SourcePath, TargetPath>(
    source: &str,
    target: &str,
    source_port_key: &str,
    target_port_key: &str,
    source_path: &[SourcePath],
    target_path: &[TargetPath],
) -> Vec<PendingCompoundSegment>
where
    SourcePath: AsRef<str>,
    TargetPath: AsRef<str>,
{
    let shape = cross_hierarchy_shape(source, target, source_path, target_path);
    let common_depth = shape.common_depth;

    let mut segments = Vec::with_capacity(
        cross_hierarchy_segment_count(source_path.len(), target_path.len(), shape)
            .expect("in-memory hierarchy paths cannot overflow their segment count"),
    );

    for depth in (common_depth + 1..=source_path.len()).rev() {
        let segment_source = if depth == source_path.len() {
            source.to_string()
        } else {
            source_path[depth].as_ref().to_string()
        };
        let connects_parent_node = target == source_path[depth - 1].as_ref();
        segments.push(PendingCompoundSegment {
            graph_parent: Some(source_path[depth - 1].as_ref().to_string()),
            source: PendingSegmentEndpoint::LocalNode {
                node_id: segment_source,
                port_key: source_port_key.to_string(),
            },
            target: PendingSegmentEndpoint::ParentBoundary {
                node_id: source_path[depth - 1].as_ref().to_string(),
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

    if !shape.source_is_target_ancestor && !shape.target_is_source_ancestor {
        let segment_source = if source_path.len() > common_depth {
            source_path[common_depth].as_ref().to_string()
        } else {
            source.to_string()
        };
        let segment_target = if target_path.len() > common_depth {
            target_path[common_depth].as_ref().to_string()
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
                .map(|parent_depth| source_path[parent_depth].as_ref().to_string()),
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
            target_path[depth].as_ref().to_string()
        };
        let connects_parent_node = source == target_path[depth - 1].as_ref();
        segments.push(PendingCompoundSegment {
            graph_parent: Some(target_path[depth - 1].as_ref().to_string()),
            source: PendingSegmentEndpoint::ParentBoundary {
                node_id: target_path[depth - 1].as_ref().to_string(),
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

pub(crate) fn source_ported_cross_hierarchy_segment_count<SourcePath, TargetPath>(
    source: &str,
    target: &str,
    source_path: &[SourcePath],
    target_path: &[TargetPath],
) -> Result<usize, WorkError>
where
    SourcePath: AsRef<str>,
    TargetPath: AsRef<str>,
{
    let shape = cross_hierarchy_shape(source, target, source_path, target_path);
    cross_hierarchy_segment_count(source_path.len(), target_path.len(), shape)
}

fn cross_hierarchy_segment_count(
    source_depth: usize,
    target_depth: usize,
    shape: CrossHierarchyShape,
) -> Result<usize, WorkError> {
    checked_sum([
        source_depth
            .checked_sub(shape.common_depth)
            .ok_or(WorkError::ArithmeticOverflow)?,
        target_depth
            .checked_sub(shape.common_depth)
            .ok_or(WorkError::ArithmeticOverflow)?,
        usize::from(!shape.source_is_target_ancestor && !shape.target_is_source_ancestor),
    ])
}

#[derive(Debug, Clone, Copy)]
struct CrossHierarchyShape {
    common_depth: usize,
    source_is_target_ancestor: bool,
    target_is_source_ancestor: bool,
}

fn cross_hierarchy_shape<SourcePath, TargetPath>(
    source: &str,
    target: &str,
    source_path: &[SourcePath],
    target_path: &[TargetPath],
) -> CrossHierarchyShape
where
    SourcePath: AsRef<str>,
    TargetPath: AsRef<str>,
{
    let common_depth = common_graph_depth(source_path, target_path);
    CrossHierarchyShape {
        common_depth,
        source_is_target_ancestor: target_path.len() > common_depth
            && source == target_path[common_depth].as_ref(),
        target_is_source_ancestor: source_path.len() > common_depth
            && target == source_path[common_depth].as_ref(),
    }
}

pub(crate) fn compound_label_segment_index(
    segments: &[PendingCompoundSegment],
    placement: EdgeLabelPlacement,
) -> usize {
    match placement {
        EdgeLabelPlacement::Tail => stable_extreme_segment_index(segments, false),
        EdgeLabelPlacement::Head => stable_extreme_segment_index(segments, true),
        EdgeLabelPlacement::Center => center_segment_index(segments),
    }
}

fn stable_extreme_segment_index(segments: &[PendingCompoundSegment], maximum: bool) -> usize {
    let mut selected = None;
    for (index, candidate) in segments.iter().enumerate() {
        let Some((_, current)) = selected else {
            selected = Some((index, candidate.segment));
            continue;
        };
        let ordering = compare_compound_segments(candidate.segment, current);
        let replace = if maximum {
            ordering != std::cmp::Ordering::Less
        } else {
            ordering == std::cmp::Ordering::Less
        };
        if replace {
            selected = Some((index, candidate.segment));
        }
    }
    selected.map(|(index, _)| index).unwrap_or(0)
}

fn center_segment_index(segments: &[PendingCompoundSegment]) -> usize {
    let mut shallowest_output = None;
    let mut shallowest_input = None;
    for (index, segment) in segments.iter().enumerate() {
        match segment.segment {
            CompoundEdgeSegment::Output { depth } => {
                if shallowest_output.is_none_or(|(_, current)| depth <= current) {
                    shallowest_output = Some((index, depth));
                }
            }
            CompoundEdgeSegment::Input { depth } => {
                if shallowest_input.is_none_or(|(_, current)| depth < current) {
                    shallowest_input = Some((index, depth));
                }
            }
        }
    }
    shallowest_output
        .or(shallowest_input)
        .map(|(index, _)| index)
        .unwrap_or(0)
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
    if parent_port_data.border_offset.is_none()
        && let Some(border_offset) = dummy_border_offset
    {
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
    preprocess_source_ported_compound_graph_observed(graph, &mut NoopCompoundPreprocessObserver);
}

trait CompoundPreprocessObserver {
    fn visit_graph_edge_for_endpoint_plan(&mut self) {}

    fn visit_existing_cross_hierarchy_edge(&mut self) {}

    fn visit_endpoint_nested_node(&mut self) {}

    fn visit_endpoint_incident_edge(&mut self) {}

    fn process_segment_edge(&mut self) {}

    fn build_parent_port_index(&mut self) {}
}

struct NoopCompoundPreprocessObserver;

impl CompoundPreprocessObserver for NoopCompoundPreprocessObserver {}

#[cfg(test)]
#[derive(Debug, Default, PartialEq, Eq)]
struct CompoundPreprocessWork {
    graph_edges_planned: usize,
    existing_cross_hierarchy_edges_indexed: usize,
    endpoint_nested_nodes_indexed: usize,
    endpoint_incident_edges_indexed: usize,
    segment_edges_processed: usize,
    parent_port_indexes_built: usize,
}

#[cfg(test)]
impl CompoundPreprocessObserver for CompoundPreprocessWork {
    fn visit_graph_edge_for_endpoint_plan(&mut self) {
        self.graph_edges_planned += 1;
    }

    fn visit_existing_cross_hierarchy_edge(&mut self) {
        self.existing_cross_hierarchy_edges_indexed += 1;
    }

    fn visit_endpoint_nested_node(&mut self) {
        self.endpoint_nested_nodes_indexed += 1;
    }

    fn visit_endpoint_incident_edge(&mut self) {
        self.endpoint_incident_edges_indexed += 1;
    }

    fn process_segment_edge(&mut self) {
        self.segment_edges_processed += 1;
    }

    fn build_parent_port_index(&mut self) {
        self.parent_port_indexes_built += 1;
    }
}

fn preprocess_source_ported_compound_graph_observed(
    graph: &mut LGraph,
    observer: &mut impl CompoundPreprocessObserver,
) {
    introduce_source_ported_hierarchy_edge_segments(graph);
    if !hierarchy_has_external_ports(graph) {
        let result = graph.try_for_each_graph_mut(|graph| {
            prepare_compound_endpoint_metadata(graph, observer);
            Ok::<(), std::convert::Infallible>(())
        });
        result.expect("compound endpoint preprocessing is infallible");
        return;
    }

    let mut detached = detach_nested_graph_hierarchy(graph);
    let root_endpoint_plan = prepare_compound_endpoint_metadata(graph, observer);
    let mut detached_endpoint_plans = Vec::with_capacity(detached.len());
    for entry in &mut detached {
        detached_endpoint_plans.push(prepare_compound_endpoint_metadata(
            &mut entry.graph,
            observer,
        ));
    }
    link_compound_external_dummy_metadata(
        graph,
        &root_endpoint_plan,
        &mut detached,
        &detached_endpoint_plans,
        observer,
    );
    ensure_nested_external_dummies_for_parent_ports(graph, detached);
}

fn introduce_source_ported_hierarchy_edge_segments(graph: &mut LGraph) {
    if !hierarchy_has_pending_edges(graph) {
        return;
    }
    struct DetachedGraph {
        graph: Box<LGraph>,
        parent: Option<usize>,
        parent_node: usize,
    }

    fn process_local_edges(graph: &mut LGraph) {
        // A hierarchy edge owns a traversal of its relative graph scope. Skipping empty owners is
        // essential on deep compound chains: otherwise every detached descendant would rescan its
        // remaining subtree even though the official preprocessor has no segments to introduce.
        if graph.hierarchy_edges.is_empty() {
            return;
        }
        let hierarchy_edges = std::mem::take(&mut graph.hierarchy_edges);
        let mut segments = Vec::new();
        for edge in hierarchy_edges {
            segments.extend(materialize_hierarchy_edge_segments(&edge));
        }
        introduce_source_ported_scoped_edge_segments(graph, segments);
    }

    fn take_nested_graphs(graph: &mut LGraph) -> Vec<(usize, Box<LGraph>)> {
        graph
            .layerless_nodes
            .iter_mut()
            .enumerate()
            .filter_map(|(node, data)| data.nested_graph.take().map(|graph| (node, graph)))
            .collect()
    }

    process_local_edges(graph);
    let mut detached = take_nested_graphs(graph)
        .into_iter()
        .map(|(parent_node, graph)| DetachedGraph {
            graph,
            parent: None,
            parent_node,
        })
        .collect::<Vec<_>>();
    let mut cursor = 0usize;
    while cursor < detached.len() {
        process_local_edges(&mut detached[cursor].graph);
        let children = take_nested_graphs(&mut detached[cursor].graph);
        detached.extend(
            children
                .into_iter()
                .map(|(parent_node, graph)| DetachedGraph {
                    graph,
                    parent: Some(cursor),
                    parent_node,
                }),
        );
        cursor += 1;
    }
    while let Some(detached_graph) = detached.pop() {
        let parent_node = detached_graph.parent_node;
        if let Some(parent) = detached_graph.parent {
            detached[parent].graph.layerless_nodes[parent_node].nested_graph =
                Some(detached_graph.graph);
        } else {
            graph.layerless_nodes[parent_node].nested_graph = Some(detached_graph.graph);
        }
    }
}

fn hierarchy_has_pending_edges(graph: &LGraph) -> bool {
    let mut stack = vec![graph];
    while let Some(current) = stack.pop() {
        if !current.hierarchy_edges.is_empty() {
            return true;
        }
        stack.extend(
            current
                .layerless_nodes
                .iter()
                .filter_map(|node| node.nested_graph.as_deref()),
        );
    }
    false
}

fn hierarchy_has_external_ports(graph: &LGraph) -> bool {
    let mut stack = vec![graph];
    while let Some(current) = stack.pop() {
        if current.graph_properties.external_ports {
            return true;
        }
        stack.extend(
            current
                .layerless_nodes
                .iter()
                .filter_map(|node| node.nested_graph.as_deref()),
        );
    }
    false
}

fn materialize_hierarchy_edge_segments(edge: &HierarchyEdge) -> Vec<ScopedHierarchySegment> {
    let segments = source_ported_cross_hierarchy_segments(
        edge.source_node_id.as_str(),
        edge.target_node_id.as_str(),
        edge.source_port_key.as_str(),
        edge.target_port_key.as_str(),
        &edge.source_path,
        &edge.target_path,
    );
    let mut tail_segment = None;
    let mut center_segment = None;
    let mut head_segment = None;
    let label_segments = edge
        .labels
        .iter()
        .map(|label| match label.placement {
            EdgeLabelPlacement::Tail => *tail_segment.get_or_insert_with(|| {
                compound_label_segment_index(&segments, EdgeLabelPlacement::Tail)
            }),
            EdgeLabelPlacement::Center => *center_segment.get_or_insert_with(|| {
                compound_label_segment_index(&segments, EdgeLabelPlacement::Center)
            }),
            EdgeLabelPlacement::Head => *head_segment.get_or_insert_with(|| {
                compound_label_segment_index(&segments, EdgeLabelPlacement::Head)
            }),
        })
        .collect::<Vec<_>>();

    segments
        .into_iter()
        .enumerate()
        .map(|(segment_index, pending)| {
            let labels = edge
                .labels
                .iter()
                .zip(label_segments.iter())
                .filter(|(_, label_segment)| **label_segment == segment_index)
                .map(|(label, _)| {
                    let mut label = label.clone();
                    label.original_label_edge = Some(edge.id.clone());
                    label
                })
                .collect::<Vec<_>>();
            ScopedHierarchySegment {
                edge: HierarchyEdge {
                    id: edge.id.clone(),
                    source_node_id: edge.source_node_id.clone(),
                    target_node_id: edge.target_node_id.clone(),
                    source_port_key: edge.source_port_key.clone(),
                    target_port_key: edge.target_port_key.clone(),
                    source_path: Vec::new(),
                    target_path: Vec::new(),
                    labels: Vec::new(),
                    minlen: edge.minlen,
                    model_order: edge.model_order,
                    priority_direction: edge.priority_direction,
                    priority_shortness: edge.priority_shortness,
                    priority_straightness: edge.priority_straightness,
                },
                pending,
                labels,
            }
        })
        .collect()
}

pub(crate) fn introduce_source_ported_scoped_edge_segments(
    graph: &mut LGraph,
    segments: Vec<ScopedHierarchySegment>,
) {
    let mut segments_by_parent: HashMap<Option<String>, Vec<ScopedHierarchySegment>> =
        HashMap::new();
    for scoped in segments {
        segments_by_parent
            .entry(scoped.pending.graph_parent.clone())
            .or_default()
            .push(scoped);
    }
    let mut first_graph = true;
    let result = graph.try_for_each_graph_mut(|graph| {
        let segments = if first_graph {
            first_graph = false;
            segments_by_parent.remove(&None)
        } else {
            segments_by_parent.remove(&graph.parent_node_id)
        };
        if let Some(segments) = segments {
            let mut external_ports = HashMap::new();
            let mut local_ports = GraphPortIndex::new(graph);
            for scoped in segments {
                let edge_index = introduce_hierarchical_edge_segment(
                    graph,
                    &scoped.edge,
                    &scoped.pending,
                    scoped.labels,
                    &mut external_ports,
                    &mut local_ports,
                );
                let model_order = scoped.edge.model_order;
                record_cross_hierarchy_edge_segment(
                    graph,
                    scoped.edge.id,
                    model_order,
                    edge_index,
                    scoped.pending.segment,
                );
            }
        }
        Ok::<(), std::convert::Infallible>(())
    });
    result.expect("scoped hierarchy segment introduction is infallible");
    assert!(
        segments_by_parent.is_empty(),
        "scoped hierarchy segment owners were validated before graph mutation"
    );
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
    opposite_port_key: String,
    port_type: PortType,
}

struct GraphPortIndex {
    nodes: HashMap<String, usize>,
    ports: HashMap<(usize, PortType), HashMap<String, usize>>,
}

impl GraphPortIndex {
    fn new(graph: &LGraph) -> Self {
        let mut nodes = HashMap::with_capacity(graph.layerless_nodes.len());
        let mut ports = HashMap::new();
        for (node, data) in graph.layerless_nodes.iter().enumerate() {
            nodes.entry(data.id.clone()).or_insert(node);
            for (port, data) in data.ports.iter().enumerate() {
                ports
                    .entry((node, data.port_type))
                    .or_insert_with(HashMap::new)
                    .entry(data.id.clone())
                    .or_insert(port);
            }
        }
        Self { nodes, ports }
    }

    fn port(&self, node_id: &str, port_key: &str, port_type: PortType) -> Option<PortRef> {
        let node = *self.nodes.get(node_id)?;
        let port = *self.ports.get(&(node, port_type))?.get(port_key)?;
        Some(PortRef { node, port })
    }

    fn ensure_local_node_port(
        &mut self,
        graph: &mut LGraph,
        node_id: &str,
        port_key: &str,
        port_type: PortType,
    ) -> Option<PortRef> {
        if let Some(port) = self.port(node_id, port_key, port_type) {
            return Some(port);
        }
        let node = *self.nodes.get(node_id)?;
        let port = graph.layerless_nodes[node].ports.len();
        graph.layerless_nodes[node]
            .ports
            .push(LPort::new(port_key.to_string(), node, port_type));
        self.ports
            .entry((node, port_type))
            .or_default()
            .insert(port_key.to_string(), port);
        Some(PortRef { node, port })
    }
}

struct NodePortIndex {
    owner: usize,
    ports: HashMap<PortType, HashMap<String, usize>>,
}

impl NodePortIndex {
    fn new(graph: &LGraph, owner: usize) -> Self {
        let mut ports = HashMap::new();
        let node = graph
            .layerless_nodes
            .get(owner)
            .expect("compound parent node must exist while indexing its ports");
        for (port_index, port) in node.ports.iter().enumerate() {
            ports
                .entry(port.port_type)
                .or_insert_with(HashMap::new)
                .entry(port.id.clone())
                .or_insert(port_index);
        }
        Self { owner, ports }
    }

    fn owner(&self) -> usize {
        self.owner
    }

    fn port(&self, port_key: &str, port_type: PortType) -> Option<PortRef> {
        let port = *self.ports.get(&port_type)?.get(port_key)?;
        Some(PortRef {
            node: self.owner,
            port,
        })
    }

    fn record_port(&mut self, graph: &LGraph, port: usize) {
        let port_data = graph
            .layerless_nodes
            .get(self.owner)
            .and_then(|node| node.ports.get(port))
            .expect("new compound parent port must belong to the indexed owner");
        self.ports
            .entry(port_data.port_type)
            .or_default()
            .entry(port_data.id.clone())
            .or_insert(port);
    }
}

#[derive(Debug, Clone, Copy)]
struct ExternalDummyMetadata {
    dummy_node: usize,
    external_port_side: PortSide,
    border_offset: Option<f64>,
}

struct NestedExternalDummyIndex {
    graph_id: String,
    by_edge_id: HashMap<String, ExternalDummyMetadata>,
}

#[derive(Debug)]
struct CompoundEndpoint {
    port: PortRef,
    edge: usize,
}

struct CompoundEndpointPlan {
    by_node: Vec<Vec<CompoundEndpoint>>,
}

impl CompoundEndpointPlan {
    fn new(graph: &LGraph, observer: &mut impl CompoundPreprocessObserver) -> Self {
        let mut by_node = (0..graph.layerless_nodes.len())
            .map(|_| Vec::new())
            .collect::<Vec<_>>();
        for (edge_index, edge) in graph.edges.iter().enumerate() {
            observer.visit_graph_edge_for_endpoint_plan();
            let Some(segment) = edge.compound_segment else {
                continue;
            };
            let port = match segment {
                CompoundEdgeSegment::Output { .. } => edge.source,
                CompoundEdgeSegment::Input { .. } => edge.target,
            };
            if graph
                .layerless_nodes
                .get(port.node)
                .is_some_and(|node| node.compound)
            {
                by_node[port.node].push(CompoundEndpoint {
                    port,
                    edge: edge_index,
                });
            }
        }
        Self { by_node }
    }

    fn for_node(&self, node_index: usize) -> &[CompoundEndpoint] {
        self.by_node
            .get(node_index)
            .map(Vec::as_slice)
            .expect("compound endpoint plan must match the graph node arena")
    }

    fn has_endpoints(&self) -> bool {
        self.by_node.iter().any(|endpoints| !endpoints.is_empty())
    }
}

impl NestedExternalDummyIndex {
    fn new(
        graph: &LGraph,
        parent_node_id: &str,
        observer: &mut impl CompoundPreprocessObserver,
    ) -> Self {
        let dummy_id = format!("external:{parent_node_id}");
        let mut by_edge_id = HashMap::new();
        for (dummy_node, node) in graph.layerless_nodes.iter().enumerate() {
            observer.visit_endpoint_nested_node();
            if node.kind != LNodeKind::ExternalPort || node.id != dummy_id {
                continue;
            }
            let Some(port) = node.ports.first() else {
                continue;
            };
            let metadata = ExternalDummyMetadata {
                dummy_node,
                external_port_side: node.external_port_side,
                border_offset: port.border_offset,
            };
            for port in &node.ports {
                for edge_index in port
                    .incoming_edges
                    .iter()
                    .chain(port.outgoing_edges.iter())
                    .copied()
                {
                    observer.visit_endpoint_incident_edge();
                    // Keep the first match in nested node/port/list order, exactly like the
                    // source-backed external_dummy_for_compound_edge scan.
                    let edge_id = graph.edges[edge_index].id.clone();
                    by_edge_id.entry(edge_id).or_insert(metadata);
                }
            }
        }
        Self {
            graph_id: graph.id.clone(),
            by_edge_id,
        }
    }

    fn external_dummy(&self, edge_id: &str) -> Option<(&str, ExternalDummyMetadata)> {
        let metadata = *self.by_edge_id.get(edge_id)?;
        Some((self.graph_id.as_str(), metadata))
    }
}

/// Source-backed equivalent of ELK's `introduceHierarchicalEdgeSegment(...)`.
fn introduce_hierarchical_edge_segment(
    graph: &mut LGraph,
    edge: &HierarchyEdge,
    pending: &PendingCompoundSegment,
    labels: Vec<LLabel>,
    external_ports: &mut HashMap<ExternalPortKey, ExternalPort>,
    local_ports: &mut GraphPortIndex,
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

    let source =
        ensure_segment_endpoint_port(graph, local_ports, &pending.source, PortType::Output);
    let target = ensure_segment_endpoint_port(graph, local_ports, &pending.target, PortType::Input);

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
    local_ports: &mut GraphPortIndex,
    endpoint: &PendingSegmentEndpoint,
    port_type: PortType,
) -> PortRef {
    match endpoint {
        PendingSegmentEndpoint::LocalNode { node_id, port_key } => local_ports
            .ensure_local_node_port(graph, node_id.as_str(), port_key.as_str(), port_type)
            .expect("compound segment local endpoint should exist in the current graph"),
        PendingSegmentEndpoint::ParentBoundary { node_id, .. } => create_parent_boundary_port(
            graph,
            local_ports,
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

fn prepare_compound_endpoint_metadata(
    graph: &mut LGraph,
    observer: &mut impl CompoundPreprocessObserver,
) -> CompoundEndpointPlan {
    let endpoint_plan = CompoundEndpointPlan::new(graph, observer);
    let mut recorded_edges = vec![false; graph.edges.len()];
    for record in &graph.cross_hierarchy_edges {
        observer.visit_existing_cross_hierarchy_edge();
        if let Some(recorded) = recorded_edges.get_mut(record.edge) {
            *recorded = true;
        }
    }

    for (edge_index, recorded_edge) in recorded_edges.iter_mut().enumerate() {
        let Some(segment) = graph.edges[edge_index].compound_segment else {
            continue;
        };
        observer.process_segment_edge();
        if !*recorded_edge {
            let edge_id = graph.edges[edge_index].id.clone();
            let model_order = graph.edges[edge_index].model_order;
            record_cross_hierarchy_edge_segment(graph, edge_id, model_order, edge_index, segment);
            *recorded_edge = true;
        }
    }

    if endpoint_plan.has_endpoints() {
        graph.options.port_constraints = if graph.options.port_constraints.is_side_fixed() {
            PortConstraints::FixedSide
        } else {
            PortConstraints::Free
        };
        graph.graph_properties.non_free_ports = true;
    }

    endpoint_plan
}

struct DetachedNestedGraph {
    graph: Box<LGraph>,
    parent: Option<usize>,
    parent_node: usize,
}

fn detach_nested_graph_hierarchy(graph: &mut LGraph) -> Vec<DetachedNestedGraph> {
    fn take_nested_graphs(graph: &mut LGraph) -> Vec<(usize, Box<LGraph>)> {
        graph
            .layerless_nodes
            .iter_mut()
            .enumerate()
            .filter_map(|(node, data)| data.nested_graph.take().map(|graph| (node, graph)))
            .collect()
    }

    let mut detached = take_nested_graphs(graph)
        .into_iter()
        .map(|(parent_node, graph)| DetachedNestedGraph {
            graph,
            parent: None,
            parent_node,
        })
        .collect::<Vec<_>>();
    let mut cursor = 0usize;
    while cursor < detached.len() {
        let children = take_nested_graphs(&mut detached[cursor].graph);
        detached.extend(
            children
                .into_iter()
                .map(|(parent_node, graph)| DetachedNestedGraph {
                    graph,
                    parent: Some(cursor),
                    parent_node,
                }),
        );
        cursor += 1;
    }
    detached
}

fn detached_nested_graph_preorder(detached: &[DetachedNestedGraph]) -> Vec<usize> {
    let mut root_children = Vec::new();
    let mut children_by_parent = vec![Vec::new(); detached.len()];
    for (index, entry) in detached.iter().enumerate() {
        if let Some(parent) = entry.parent {
            children_by_parent[parent].push(index);
        } else {
            root_children.push(index);
        }
    }

    let mut preorder = Vec::with_capacity(detached.len());
    let mut stack = root_children.into_iter().rev().collect::<Vec<_>>();
    while let Some(index) = stack.pop() {
        preorder.push(index);
        stack.extend(children_by_parent[index].iter().rev().copied());
    }
    preorder
}

fn ensure_nested_external_dummies_for_parent_ports(
    graph: &mut LGraph,
    mut detached: Vec<DetachedNestedGraph>,
) {
    while let Some(mut entry) = detached.pop() {
        ensure_direct_nested_external_dummies_for_parent_ports(&mut entry.graph);
        if let Some(parent) = entry.parent {
            detached[parent].graph.layerless_nodes[entry.parent_node].nested_graph =
                Some(entry.graph);
        } else {
            graph.layerless_nodes[entry.parent_node].nested_graph = Some(entry.graph);
        }
    }
    ensure_direct_nested_external_dummies_for_parent_ports(graph);
}

fn ensure_direct_nested_external_dummies_for_parent_ports(graph: &mut LGraph) {
    let graph_id = graph.id.clone();
    let node_count = graph.layerless_nodes.len();

    for node_index in 0..node_count {
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
            let (port_constraints, port_side) = parent_dummy_port_side(
                port,
                parent_constraints,
                parent_size,
                graph.options.direction,
            );
            let mut dummy = create_external_port_dummy(
                format!("external:{parent_node_id}"),
                format!("external:{parent_node_id}:0"),
                port.port_type,
                port_constraints,
                port_side,
                -port.net_flow(),
                port.position,
                port.size,
                parent_size,
                port.border_offset.unwrap_or(0.0),
                nested_direction,
            );
            dummy.parent_port_key = Some(port.id.clone());
            dummy.parent_port_type = Some(port.port_type);
            let dummy_side = dummy.external_port_side;
            let dummy_border_offset = port.border_offset;
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

fn parent_dummy_port_side(
    port: &LPort,
    parent_constraints: PortConstraints,
    parent_size: LSize,
    direction: ElkDirection,
) -> (PortConstraints, PortSide) {
    if !parent_constraints.is_side_fixed() || port.side != PortSide::Undefined {
        return (parent_constraints, port.side);
    }

    let side = calc_port_side(port, parent_size, direction);
    if side != PortSide::Undefined {
        return (parent_constraints, side);
    }

    // ELK creates missing child dummies before `setSidesOfPortsToSidesOfDummyNodes` fixes the
    // parent node constraints. If a sibling dummy already fixed this node in the Rust model, keep
    // this still-undefined port on the original free-constraints path so net flow can choose a side.
    (PortConstraints::Free, PortSide::Undefined)
}

#[derive(Debug, Clone)]
struct ExternalDummyInfo {
    dummy_node: usize,
    port_type: PortType,
    parent_port_key: String,
    parent_port_type: PortType,
    external_port_side: PortSide,
    border_offset: Option<f64>,
    origin_port: Option<GraphPortRef>,
}

fn link_compound_external_dummy_metadata(
    graph: &mut LGraph,
    root_endpoint_plan: &CompoundEndpointPlan,
    detached: &mut [DetachedNestedGraph],
    detached_endpoint_plans: &[CompoundEndpointPlan],
    observer: &mut impl CompoundPreprocessObserver,
) {
    debug_assert_eq!(detached.len(), detached_endpoint_plans.len());
    let preorder = detached_nested_graph_preorder(detached);
    for child in preorder {
        let parent = detached[child].parent;
        let parent_node = detached[child].parent_node;
        if let Some(parent) = parent {
            debug_assert!(parent < child);
            let (parents, children) = detached.split_at_mut(child);
            link_direct_compound_external_dummy_metadata(
                &mut parents[parent].graph,
                parent_node,
                &mut children[0].graph,
                detached_endpoint_plans[parent].for_node(parent_node),
                observer,
            );
        } else {
            link_direct_compound_external_dummy_metadata(
                graph,
                parent_node,
                &mut detached[child].graph,
                root_endpoint_plan.for_node(parent_node),
                observer,
            );
        }
    }
}

fn link_direct_compound_external_dummy_metadata(
    graph: &mut LGraph,
    node_index: usize,
    nested_graph: &mut LGraph,
    endpoints: &[CompoundEndpoint],
    observer: &mut impl CompoundPreprocessObserver,
) {
    let graph_id = graph.id.clone();
    let parent_node_id = graph.layerless_nodes[node_index].id.clone();
    let nested_graph_id = nested_graph.id.clone();
    if !endpoints.is_empty() {
        let endpoint_index =
            NestedExternalDummyIndex::new(nested_graph, parent_node_id.as_str(), observer);
        for endpoint in endpoints {
            // The endpoint plan is consumed before edge slots can be reordered or removed, so the
            // stable edge index avoids cloning IDs while retaining official edge-list order.
            let Some((dummy_graph_id, dummy)) =
                endpoint_index.external_dummy(graph.edges[endpoint.edge].id.as_str())
            else {
                continue;
            };
            // The edge endpoint is the authoritative port identity. The later key-based pass is
            // only a compatibility fallback and must not collapse duplicate port IDs to the first
            // matching port.
            link_external_port_dummy(
                graph,
                endpoint.port,
                dummy.external_port_side,
                dummy_graph_id.to_string(),
                dummy.dummy_node,
                dummy.border_offset,
            );
            set_external_dummy_origin(
                nested_graph,
                dummy.dummy_node,
                graph_id.clone(),
                endpoint.port,
            );
        }
    }

    let external_dummies = external_dummies_for_parent_node(nested_graph, parent_node_id.as_str());
    if external_dummies.is_empty() {
        return;
    }

    observer.build_parent_port_index();
    // Every nested graph belongs to exactly one compound parent node. Index only that owner's
    // ports, and append newly created ports in the existing dummy traversal order. This preserves
    // ELK's list-order semantics without rescanning unrelated siblings in the parent graph.
    let mut parent_ports = NodePortIndex::new(graph, node_index);

    for external_dummy in external_dummies {
        let parent_port = if let Some(port) =
            parent_port_for_external_dummy(graph, &parent_ports, &external_dummy)
        {
            port
        } else {
            let port = create_parent_external_port(
                graph,
                node_index,
                external_dummy.port_type,
                external_dummy.external_port_side,
                Some(external_dummy.parent_port_key.as_str()),
            );
            parent_ports.record_port(graph, port.port);
            port
        };
        link_external_port_dummy(
            graph,
            parent_port,
            external_dummy.external_port_side,
            nested_graph_id.clone(),
            external_dummy.dummy_node,
            external_dummy.border_offset,
        );
        set_external_dummy_origin(
            nested_graph,
            external_dummy.dummy_node,
            graph_id.clone(),
            parent_port,
        );
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
            Some(ExternalDummyInfo {
                dummy_node,
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
    parent_ports: &NodePortIndex,
    external_dummy: &ExternalDummyInfo,
) -> Option<PortRef> {
    let parent_node = parent_ports.owner();
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
        && let Some(port) = parent_ports.port(
            external_dummy.parent_port_key.as_str(),
            external_dummy.parent_port_type,
        )
    {
        return Some(port);
    }

    None
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
    let port = graph
        .add_port(parent_node, port_type, port_side, Default::default())
        .expect("parent compound node should exist when linking external dummy");
    if let Some(port_key) = port_key
        && !port_key.is_empty()
        && let Some(port_data) = graph.layerless_nodes[parent_node].ports.get_mut(port.port)
    {
        port_data.id = port_key.to_string();
    }
    port
}

fn create_parent_boundary_port(
    graph: &mut LGraph,
    local_ports: &GraphPortIndex,
    parent_node_id: &str,
    dummy_port_type: PortType,
    parent_port_type: PortType,
    parent_port_key: &str,
    connects_parent_node: bool,
) -> PortRef {
    graph.graph_properties.external_ports = true;
    graph.graph_properties.non_free_ports = true;
    graph.options.port_constraints = if graph.options.port_constraints.is_side_fixed() {
        PortConstraints::FixedSide
    } else {
        PortConstraints::Free
    };
    let parent_port_side = if connects_parent_node {
        local_ports
            .port(parent_node_id, parent_port_key, parent_port_type)
            .and_then(|port| graph.layerless_nodes[port.node].ports.get(port.port))
            .map(|port| port.side)
    } else {
        None
    };
    let port_side = if let Some(port_side) = parent_port_side {
        port_side
    } else if graph.options.port_constraints.is_side_fixed() {
        match parent_port_type {
            PortType::Input => port_side_from_direction(graph.options.direction).opposed(),
            PortType::Output => port_side_from_direction(graph.options.direction),
        }
    } else {
        PortSide::Undefined
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
        port_side,
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
    dummy.parent_port_type = if connects_parent_node {
        Some(parent_port_type)
    } else {
        Some(dummy_port_type)
    };
    dummy.ports[0].node = node;
    graph.layerless_nodes.push(dummy);
    PortRef { node, port: 0 }
}

fn has_incident_edges(port: &LPort) -> bool {
    port.incoming_edges.len() + port.outgoing_edges.len() > 0
}

fn port_side_from_direction(direction: ElkDirection) -> PortSide {
    match direction {
        ElkDirection::Right | ElkDirection::Undefined => PortSide::East,
        ElkDirection::Left => PortSide::West,
        ElkDirection::Down => PortSide::South,
        ElkDirection::Up => PortSide::North,
    }
}

fn calc_port_side(port: &LPort, node_size: LSize, direction: ElkDirection) -> PortSide {
    let node_width = node_size.width;
    let node_height = node_size.height;
    if node_width <= 0.0 && node_height <= 0.0 {
        return PortSide::Undefined;
    }

    let xpos = port.position.x;
    let ypos = port.position.y;
    let width = port.size.width;
    let height = port.size.height;

    match direction {
        ElkDirection::Left | ElkDirection::Right => {
            if xpos < 0.0 {
                PortSide::West
            } else if xpos + width > node_width {
                PortSide::East
            } else {
                calc_port_side_from_percentages(node_width, node_height, xpos, ypos, width, height)
            }
        }
        ElkDirection::Up | ElkDirection::Down => {
            if ypos < 0.0 {
                PortSide::North
            } else if ypos + height > node_height {
                PortSide::South
            } else {
                calc_port_side_from_percentages(node_width, node_height, xpos, ypos, width, height)
            }
        }
        ElkDirection::Undefined => {
            calc_port_side_from_percentages(node_width, node_height, xpos, ypos, width, height)
        }
    }
}

fn calc_port_side_from_percentages(
    node_width: f64,
    node_height: f64,
    xpos: f64,
    ypos: f64,
    width: f64,
    height: f64,
) -> PortSide {
    let width_percent = (xpos + width / 2.0) / node_width;
    let height_percent = (ypos + height / 2.0) / node_height;

    if width_percent + height_percent <= 1.0 && width_percent - height_percent <= 0.0 {
        PortSide::West
    } else if width_percent + height_percent >= 1.0 && width_percent - height_percent >= 0.0 {
        PortSide::East
    } else if height_percent < 0.5 {
        PortSide::North
    } else {
        PortSide::South
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

fn common_graph_depth<Left, Right>(left: &[Left], right: &[Right]) -> usize
where
    Left: AsRef<str>,
    Right: AsRef<str>,
{
    left.iter()
        .zip(right.iter())
        .take_while(|(left, right)| left.as_ref() == right.as_ref())
        .count()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::graph::LPoint;
    use crate::importer::{ElkInputEdge, ElkInputGraph, ElkInputNode, import_graph};
    use crate::options::{HierarchyHandling, LayeredOptions, NodeLabelPlacement};

    fn node(id: &str) -> ElkInputNode {
        ElkInputNode {
            id: id.to_string(),
            width: 80.0,
            height: 40.0,
            parent: None,
            direction: None,
            hierarchy_handling: None,
            layer_constraint: None,
            port_constraints: None,
            node_label_placement: NodeLabelPlacement::Fixed,
            nested_spacing_base: None,
            label: None,
        }
    }

    fn edge(id: &str, source: &str, target: &str) -> ElkInputEdge {
        ElkInputEdge {
            id: id.to_string(),
            source: source.to_string(),
            target: target.to_string(),
            label: None,
            minlen: 1,
            inside_self_loops_yo: false,
            model_order: None,
            priority_direction: 0,
            priority_shortness: 0,
            priority_straightness: 0,
        }
    }

    fn graph(nodes: Vec<ElkInputNode>, edges: Vec<ElkInputEdge>) -> ElkInputGraph {
        ElkInputGraph {
            id: "root".to_string(),
            options: LayeredOptions::mermaid_flowchart_defaults(ElkDirection::Down),
            nodes,
            edges,
        }
    }

    fn compound_fanout_graph(edge_count: usize) -> ElkInputGraph {
        let mut group = node("group");
        group.hierarchy_handling = Some(HierarchyHandling::IncludeChildren);
        let mut nodes = vec![group, node("outer")];
        let mut edges = Vec::with_capacity(edge_count);
        for index in 0..edge_count {
            let leaf_id = format!("leaf-{index}");
            let mut leaf = node(&leaf_id);
            leaf.parent = Some("group".to_string());
            nodes.push(leaf);
            edges.push(edge(&format!("edge-{index}"), &leaf_id, "outer"));
        }
        let mut input = graph(nodes, edges);
        input.options.hierarchy_handling = HierarchyHandling::IncludeChildren;
        input.options.merge_hierarchy_edges = false;
        input
    }

    fn wide_compound_sibling_graph(sibling_count: usize) -> ElkInputGraph {
        let mut nodes = Vec::with_capacity(sibling_count * 2 + 1);
        for index in 0..sibling_count {
            let group_id = format!("group-{index}");
            let leaf_id = format!("leaf-{index}");
            let mut group = node(&group_id);
            group.hierarchy_handling = Some(HierarchyHandling::IncludeChildren);
            let mut leaf = node(&leaf_id);
            leaf.parent = Some(group_id);
            nodes.push(group);
            nodes.push(leaf);
        }
        nodes.push(node("outer"));
        let mut input = graph(nodes, vec![edge("crossing", "leaf-0", "outer")]);
        input.options.hierarchy_handling = HierarchyHandling::IncludeChildren;
        input.options.merge_hierarchy_edges = false;
        input
    }

    fn compound_observed_work(input: &ElkInputGraph) -> CompoundPreprocessWork {
        let mut graph = import_graph(input).expect("compound fixture import");
        let mut work = CompoundPreprocessWork::default();
        preprocess_source_ported_compound_graph_observed(&mut graph, &mut work);
        work
    }

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

    #[test]
    fn allocation_free_label_selection_matches_stable_sorted_identity() {
        let segments = source_ported_cross_hierarchy_segments(
            "A",
            "B",
            "A:source",
            "B:target",
            &["outer", "inner"],
            &["sibling"],
        );
        let reference = |placement| {
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
        };

        for placement in [
            EdgeLabelPlacement::Tail,
            EdgeLabelPlacement::Center,
            EdgeLabelPlacement::Head,
        ] {
            assert_eq!(
                compound_label_segment_index(&segments, placement),
                reference(placement)
            );
        }
    }

    #[test]
    fn compound_segment_registration_and_endpoint_indexing_are_linear() {
        let mut previous_total = None;
        for edge_count in [1usize, 8, 64, 256] {
            let work = compound_observed_work(&compound_fanout_graph(edge_count));
            assert_eq!(
                work.existing_cross_hierarchy_edges_indexed,
                work.segment_edges_processed
            );
            assert!(work.endpoint_nested_nodes_indexed > 0);
            assert!(work.endpoint_incident_edges_indexed > 0);

            let total = work.graph_edges_planned
                + work.existing_cross_hierarchy_edges_indexed
                + work.endpoint_nested_nodes_indexed
                + work.endpoint_incident_edges_indexed
                + work.segment_edges_processed;
            if let Some((previous_edges, previous_total)) = previous_total {
                assert!(
                    total * previous_edges <= previous_total * edge_count + 8 * edge_count,
                    "compound preprocessing work grew faster than its owned edge/node payload"
                );
            }
            previous_total = Some((edge_count, total));
        }
    }

    #[test]
    fn compound_endpoint_link_uses_exact_port_identity_when_ids_repeat() {
        let mut graph = import_graph(&compound_fanout_graph(1)).expect("compound fixture import");
        introduce_source_ported_hierarchy_edge_segments(&mut graph);
        let group = graph
            .layerless_nodes
            .iter()
            .position(|node| node.id == "group")
            .expect("compound parent node");
        let edge_index = graph
            .edges
            .iter()
            .position(|edge| {
                edge.compound_segment.is_some()
                    && (edge.source.node == group || edge.target.node == group)
            })
            .expect("root compound segment");
        let original_port = if graph.edges[edge_index].source.node == group {
            graph.edges[edge_index].source
        } else {
            graph.edges[edge_index].target
        };
        let mut duplicate = graph.layerless_nodes[group].ports[original_port.port].clone();
        duplicate.incoming_edges.clear();
        duplicate.outgoing_edges.clear();
        duplicate.port_dummy = None;
        let duplicate_port = PortRef {
            node: group,
            port: graph.layerless_nodes[group].ports.len(),
        };
        graph.layerless_nodes[group].ports.push(duplicate);
        if graph.edges[edge_index].source == original_port {
            assert!(graph.set_edge_source(edge_index, duplicate_port));
        } else {
            assert!(graph.set_edge_target(edge_index, duplicate_port));
        }

        preprocess_source_ported_compound_graph(&mut graph);

        let nested = graph.layerless_nodes[group]
            .nested_graph
            .as_deref()
            .expect("compound child graph");
        let incident_dummy = nested
            .layerless_nodes
            .iter()
            .enumerate()
            .find(|(_, node)| {
                node.kind == LNodeKind::ExternalPort
                    && node.ports.iter().any(|port| {
                        port.incoming_edges
                            .iter()
                            .chain(port.outgoing_edges.iter())
                            .any(|edge| nested.edges[*edge].id == "edge-0")
                    })
            })
            .map(|(index, _)| index)
            .expect("incident external dummy");
        assert_eq!(
            nested.layerless_nodes[incident_dummy]
                .origin_port
                .as_ref()
                .map(|origin| origin.port),
            Some(duplicate_port)
        );
        assert_eq!(
            graph.layerless_nodes[group].ports[duplicate_port.port]
                .port_dummy
                .as_ref()
                .map(|dummy| dummy.node),
            Some(incident_dummy)
        );
    }

    #[test]
    fn parent_port_index_is_owner_local_in_wide_hierarchy() {
        for sibling_count in [1usize, 8, 64, 256] {
            let work = compound_observed_work(&wide_compound_sibling_graph(sibling_count));

            // Only group-0 owns an external dummy. Unrelated compound siblings must not trigger
            // whole-parent port-index rebuilds.
            assert_eq!(work.parent_port_indexes_built, 1);
        }
    }

    #[test]
    fn parent_end_external_dummy_uses_fixed_parent_port_side() {
        let mut cluster = node("cluster");
        cluster.hierarchy_handling = Some(HierarchyHandling::IncludeChildren);
        cluster.port_constraints = Some(PortConstraints::FixedSide);
        let mut child = node("A");
        child.parent = Some("cluster".to_string());

        let mut lgraph = import_graph(&graph(
            vec![cluster, child],
            vec![edge("cluster-A", "cluster", "A")],
        ))
        .unwrap();
        let cluster_index = lgraph
            .layerless_nodes
            .iter()
            .position(|node| node.id == "cluster")
            .unwrap();
        {
            let parent_port = &mut lgraph.layerless_nodes[cluster_index].ports[0];
            parent_port.set_side(PortSide::East);
            parent_port.position = LPoint { x: 80.0, y: 20.0 };
            parent_port.size = LSize {
                width: 6.0,
                height: 8.0,
            };
            parent_port.border_offset = Some(-4.0);
        }

        preprocess_source_ported_compound_graph(&mut lgraph);

        let cluster = &lgraph.layerless_nodes[cluster_index];
        let parent_port = &cluster.ports[0];
        let port_dummy = parent_port
            .port_dummy
            .as_ref()
            .expect("parent port should link to the nested external dummy");
        let nested = cluster.nested_graph.as_ref().unwrap();
        let external = &nested.layerless_nodes[port_dummy.node];

        assert_eq!(parent_port.side, PortSide::South);
        assert_eq!(external.external_port_side, PortSide::South);
        assert_eq!(
            external.parent_port_key.as_deref(),
            Some("cluster:0:source")
        );
        assert_eq!(external.parent_port_type, Some(PortType::Output));
        assert_eq!(external.ports[0].side, PortSide::North);
    }

    #[test]
    fn parent_end_external_dummy_uses_parent_port_net_flow() {
        let mut cluster = node("cluster");
        cluster.hierarchy_handling = Some(HierarchyHandling::IncludeChildren);
        let mut child = node("A");
        child.parent = Some("cluster".to_string());

        let mut lgraph = import_graph(&graph(
            vec![cluster, child],
            vec![edge("cluster-A", "cluster", "A")],
        ))
        .unwrap();
        let cluster_index = lgraph
            .layerless_nodes
            .iter()
            .position(|node| node.id == "cluster")
            .unwrap();

        preprocess_source_ported_compound_graph(&mut lgraph);

        let cluster = &lgraph.layerless_nodes[cluster_index];
        let parent_port = &cluster.ports[0];
        assert_eq!(parent_port.net_flow(), 0);
        let port_dummy = parent_port
            .port_dummy
            .as_ref()
            .expect("parent port should link to the nested external dummy");
        let nested = cluster.nested_graph.as_ref().unwrap();
        let external = &nested.layerless_nodes[port_dummy.node];

        assert_eq!(external.parent_port_type, Some(PortType::Output));
        assert_eq!(external.external_port_side, PortSide::North);
        assert_eq!(external.ports[0].side, PortSide::South);
    }

    #[test]
    fn unlinked_parent_port_uses_net_flow_after_sibling_external_port_fixing() {
        let mut s1 = node("S1");
        s1.hierarchy_handling = Some(HierarchyHandling::IncludeChildren);
        let mut sub1 = node("sub1");
        sub1.parent = Some("S1".to_string());
        let mut s2 = node("S2");
        s2.hierarchy_handling = Some(HierarchyHandling::IncludeChildren);
        let mut sub4 = node("sub4");
        sub4.parent = Some("S2".to_string());

        let mut lgraph = import_graph(&graph(
            vec![s1, sub1, s2, sub4],
            vec![edge("S1-S2", "S1", "S2"), edge("sub1-sub4", "sub1", "sub4")],
        ))
        .unwrap();
        let s2_index = lgraph
            .layerless_nodes
            .iter()
            .position(|node| node.id == "S2")
            .unwrap();

        preprocess_source_ported_compound_graph(&mut lgraph);

        let s2 = &lgraph.layerless_nodes[s2_index];
        let parent_port = s2
            .ports
            .iter()
            .find(|port| port.id == "S2:0" && port.port_type == PortType::Input)
            .expect("top-level edge target port should exist on S2");
        let port_dummy = parent_port
            .port_dummy
            .as_ref()
            .expect("S2 top-level target port should link to nested external dummy");
        let nested = s2.nested_graph.as_ref().unwrap();
        let external = &nested.layerless_nodes[port_dummy.node];

        assert_eq!(parent_port.net_flow(), 1);
        assert_eq!(parent_port.side, PortSide::West);
        assert_eq!(external.external_port_side, PortSide::West);
    }

    #[test]
    fn nested_graph_hierarchy_edges_resolve_their_relative_root_scope() {
        let mut group = node("group");
        group.hierarchy_handling = Some(HierarchyHandling::IncludeChildren);
        let mut inner = node("inner");
        inner.parent = Some("group".to_string());
        inner.hierarchy_handling = Some(HierarchyHandling::IncludeChildren);
        let mut leaf = node("leaf");
        leaf.parent = Some("inner".to_string());
        let mut sibling = node("sibling");
        sibling.parent = Some("group".to_string());
        let mut input = graph(vec![group, inner, leaf, sibling], Vec::new());
        input.options.hierarchy_handling = HierarchyHandling::IncludeChildren;
        let mut lgraph = import_graph(&input).unwrap();
        let group_graph = lgraph.layerless_nodes[0]
            .nested_graph
            .as_deref_mut()
            .expect("group graph is materialized");
        group_graph.hierarchy_edges.push(HierarchyEdge {
            id: "leaf-sibling".to_string(),
            source_node_id: "leaf".to_string(),
            target_node_id: "sibling".to_string(),
            source_port_key: "leaf:source".to_string(),
            target_port_key: "sibling:target".to_string(),
            source_path: vec!["inner".to_string()],
            target_path: Vec::new(),
            labels: Vec::new(),
            minlen: 1,
            model_order: Some(3),
            priority_direction: 0,
            priority_shortness: 0,
            priority_straightness: 0,
        });

        preprocess_source_ported_compound_graph(&mut lgraph);

        let group_graph = lgraph.layerless_nodes[0]
            .nested_graph
            .as_deref()
            .expect("group graph remains materialized");
        assert!(group_graph.hierarchy_edges.is_empty());
        assert_eq!(group_graph.cross_hierarchy_edges.len(), 1);
        assert_eq!(
            group_graph.cross_hierarchy_edges[0].original_model_order,
            Some(3)
        );
        let inner_graph = group_graph.layerless_nodes[0]
            .nested_graph
            .as_deref()
            .expect("inner graph remains materialized");
        assert_eq!(inner_graph.cross_hierarchy_edges.len(), 1);
        assert_eq!(
            inner_graph.cross_hierarchy_edges[0].original_model_order,
            Some(3)
        );
    }

    #[test]
    fn compound_external_ports_are_small_stack_safe_for_deep_mixed_hierarchy() {
        const DEPTH: usize = 512;

        std::thread::Builder::new()
            .name("elk-compound-external-port-small-stack".to_string())
            .stack_size(128 * 1024)
            .spawn(|| {
                let mut nodes = Vec::with_capacity(DEPTH + 2);
                for depth in 0..DEPTH {
                    let group_id = format!("group-{depth}");
                    let mut group = node(&group_id);
                    group.parent = depth.checked_sub(1).map(|parent| format!("group-{parent}"));
                    group.hierarchy_handling = Some(HierarchyHandling::IncludeChildren);
                    nodes.push(group);
                }
                let mut leaf = node("leaf");
                leaf.parent = Some(format!("group-{}", DEPTH - 1));
                nodes.push(leaf);
                nodes.push(node("outer"));
                let mut input = graph(nodes, vec![edge("leaf-outer", "leaf", "outer")]);
                input.options.hierarchy_handling = HierarchyHandling::IncludeChildren;

                let mut lgraph = std::mem::ManuallyDrop::new(
                    import_graph(&input).expect("deep hierarchy import"),
                );
                // The recursive-layout owner materializes SeparateChildren scopes before they
                // reach this pass. Alternate the retained scope policy after import so this one
                // materialized chain covers the mixed-policy ancestry seen by preprocessing.
                let mut graph_order = 0usize;
                lgraph
                    .try_for_each_graph_mut(|graph| {
                        graph.options.hierarchy_handling = if graph_order.is_multiple_of(2) {
                            HierarchyHandling::IncludeChildren
                        } else {
                            HierarchyHandling::SeparateChildren
                        };
                        graph_order += 1;
                        Ok::<(), std::convert::Infallible>(())
                    })
                    .expect("hierarchy option update is infallible");

                preprocess_source_ported_compound_graph(&mut lgraph);

                let mut current: &LGraph = &lgraph;
                for depth in 0..DEPTH {
                    let group_id = format!("group-{depth}");
                    let group = current
                        .layerless_nodes
                        .iter()
                        .find(|node| node.id == group_id)
                        .expect("every group remains in its parent graph");
                    assert!(
                        group.ports.iter().any(|port| port.port_dummy.is_some()),
                        "group {group_id} must link to its nested external dummy"
                    );
                    current = group
                        .nested_graph
                        .as_deref()
                        .expect("every group retains its materialized child graph");
                }
            })
            .expect("small-stack worker should start")
            .join()
            .expect("compound preprocessing must not overflow the worker stack");
    }
}
