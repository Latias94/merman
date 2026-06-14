//! Internal layered graph model.
//!
//! Source references:
//! - https://github.com/eclipse-elk/elk/blob/62d5909f96fad541bc101ad52dabaece6b7eab7e/plugins/org.eclipse.elk.alg.layered/src/org/eclipse/elk/alg/layered/graph/LGraph.java
//! - https://github.com/eclipse-elk/elk/blob/62d5909f96fad541bc101ad52dabaece6b7eab7e/plugins/org.eclipse.elk.alg.layered/src/org/eclipse/elk/alg/layered/graph/LNode.java
//! - https://github.com/eclipse-elk/elk/blob/62d5909f96fad541bc101ad52dabaece6b7eab7e/plugins/org.eclipse.elk.alg.layered/src/org/eclipse/elk/alg/layered/graph/LEdge.java
//! - https://github.com/eclipse-elk/elk/blob/62d5909f96fad541bc101ad52dabaece6b7eab7e/plugins/org.eclipse.elk.alg.layered/src/org/eclipse/elk/alg/layered/graph/LPort.java

use super::options::LayeredOptions;
use crate::random::JavaRandom;

#[derive(Debug, Clone, PartialEq)]
pub struct LGraph {
    pub id: String,
    pub options: LayeredOptions,
    pub size: LSize,
    pub padding: LPadding,
    pub offset: LPoint,
    pub layerless_nodes: Vec<LNode>,
    pub layers: Vec<Layer>,
    pub edges: Vec<LayeredEdge>,
    pub graph_properties: GraphProperties,
    pub cyclic: bool,
    pub random: JavaRandom,
    pub parent_node_id: Option<String>,
}

impl LGraph {
    pub fn new(id: impl Into<String>, options: LayeredOptions) -> Self {
        let random = JavaRandom::from_layout_seed(options.random_seed);
        Self {
            id: id.into(),
            options,
            size: LSize::default(),
            padding: LPadding::default(),
            offset: LPoint::default(),
            layerless_nodes: Vec::new(),
            layers: Vec::new(),
            edges: Vec::new(),
            graph_properties: GraphProperties::default(),
            cyclic: false,
            random,
            parent_node_id: None,
        }
    }

    pub fn sync_graph_properties_to_options(&mut self) {
        self.graph_properties.apply_to_options(&mut self.options);
        for node in &mut self.layerless_nodes {
            if let Some(nested_graph) = node.nested_graph.as_mut() {
                nested_graph.sync_graph_properties_to_options();
            }
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Layer {
    pub nodes: Vec<usize>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct LNode {
    pub id: String,
    pub kind: LNodeKind,
    pub size: LSize,
    pub position: LPoint,
    pub labels: Vec<LLabel>,
    pub ports: Vec<LPort>,
    pub nested_graph: Option<Box<LGraph>>,
    pub model_order: Option<usize>,
    pub compound: bool,
}

impl LNode {
    pub fn new(id: impl Into<String>, width: f64, height: f64, model_order: Option<usize>) -> Self {
        Self {
            id: id.into(),
            kind: LNodeKind::Normal,
            size: LSize { width, height },
            position: LPoint::default(),
            labels: Vec::new(),
            ports: Vec::new(),
            nested_graph: None,
            model_order,
            compound: false,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum LNodeKind {
    #[default]
    Normal,
    LongEdge,
    ExternalPort,
    Label,
    NorthSouthPort,
    BreakingPoint,
}

#[derive(Debug, Clone, PartialEq)]
pub struct LPort {
    pub id: String,
    pub node: usize,
    pub port_type: PortType,
    pub side: PortSide,
    pub position: LPoint,
    pub anchor: LPoint,
    pub incoming_edges: Vec<usize>,
    pub outgoing_edges: Vec<usize>,
}

impl LPort {
    pub fn new(id: impl Into<String>, node: usize, port_type: PortType) -> Self {
        Self {
            id: id.into(),
            node,
            port_type,
            side: PortSide::Undefined,
            position: LPoint::default(),
            anchor: LPoint::default(),
            incoming_edges: Vec::new(),
            outgoing_edges: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct LayeredEdge {
    pub id: String,
    pub source: PortRef,
    pub target: PortRef,
    pub source_node_id: String,
    pub target_node_id: String,
    pub labels: Vec<LLabel>,
    pub minlen: usize,
    pub reversed: bool,
    pub bend_points: Vec<LPoint>,
    pub model_order: Option<usize>,
    pub priority_direction: i32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PortRef {
    pub node: usize,
    pub port: usize,
}

#[derive(Debug, Clone, PartialEq)]
pub struct LLabel {
    pub text: String,
    pub size: LSize,
    pub placement: EdgeLabelPlacement,
    pub inline: bool,
}

impl LLabel {
    pub fn new(text: impl Into<String>, width: f64, height: f64) -> Self {
        Self {
            text: text.into(),
            size: LSize { width, height },
            placement: EdgeLabelPlacement::Center,
            inline: false,
        }
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct LPoint {
    pub x: f64,
    pub y: f64,
}

#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct LSize {
    pub width: f64,
    pub height: f64,
}

#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct LPadding {
    pub top: f64,
    pub right: f64,
    pub bottom: f64,
    pub left: f64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum PortType {
    #[default]
    Input,
    Output,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum PortSide {
    #[default]
    Undefined,
    North,
    East,
    South,
    West,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum EdgeLabelPlacement {
    Head,
    Tail,
    #[default]
    Center,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct GraphProperties {
    pub self_loops: bool,
    pub center_labels: bool,
    pub end_labels: bool,
    pub non_free_ports: bool,
    pub north_south_ports: bool,
    pub hyperedges: bool,
    pub external_ports: bool,
    pub hypernodes: bool,
}

impl GraphProperties {
    pub fn apply_to_options(&self, options: &mut LayeredOptions) {
        options.graph_has_self_loops = self.self_loops;
        options.graph_has_center_labels = self.center_labels;
        options.graph_has_end_labels = self.end_labels;
        options.graph_has_non_free_ports = self.non_free_ports;
        options.graph_has_north_south_ports = self.north_south_ports;
        options.graph_has_hyperedges = self.hyperedges;
        options.graph_has_external_ports = self.external_ports;
        options.graph_has_hypernodes = self.hypernodes;
    }
}

/// Reverse an edge according to `LEdge.reverse(...)`.
///
/// Source:
/// https://github.com/eclipse-elk/elk/blob/62d5909f96fad541bc101ad52dabaece6b7eab7e/plugins/org.eclipse.elk.alg.layered/src/org/eclipse/elk/alg/layered/graph/LEdge.java
///
/// `adapt_ports` is accepted to keep the call sites aligned with ELK. The collector-port branch
/// depends on `InternalProperties.INPUT_COLLECT` / `OUTPUT_COLLECT`, which are not represented by
/// the current importer yet, so this currently performs the non-collector endpoint swap.
pub fn reverse_edge(graph: &mut LGraph, edge_index: usize, _adapt_ports: bool) -> bool {
    let Some(edge) = graph.edges.get(edge_index) else {
        return false;
    };
    let old_source = edge.source;
    let old_target = edge.target;

    if !port_exists(graph, old_source) || !port_exists(graph, old_target) {
        return false;
    }

    remove_edge(
        &mut graph.layerless_nodes[old_source.node].ports[old_source.port].outgoing_edges,
        edge_index,
    );
    remove_edge(
        &mut graph.layerless_nodes[old_target.node].ports[old_target.port].incoming_edges,
        edge_index,
    );

    graph.layerless_nodes[old_target.node].ports[old_target.port]
        .outgoing_edges
        .push(edge_index);
    graph.layerless_nodes[old_source.node].ports[old_source.port]
        .incoming_edges
        .push(edge_index);

    let edge = &mut graph.edges[edge_index];
    edge.source = old_target;
    edge.target = old_source;

    for label in &mut edge.labels {
        label.placement = match label.placement {
            EdgeLabelPlacement::Head => EdgeLabelPlacement::Tail,
            EdgeLabelPlacement::Tail => EdgeLabelPlacement::Head,
            EdgeLabelPlacement::Center => EdgeLabelPlacement::Center,
        };
    }

    edge.reversed = !edge.reversed;
    edge.bend_points.reverse();
    true
}

fn port_exists(graph: &LGraph, port_ref: PortRef) -> bool {
    graph
        .layerless_nodes
        .get(port_ref.node)
        .and_then(|node| node.ports.get(port_ref.port))
        .is_some()
}

fn remove_edge(edges: &mut Vec<usize>, edge_index: usize) {
    if let Some(position) = edges.iter().position(|candidate| *candidate == edge_index) {
        edges.remove(position);
    }
}
