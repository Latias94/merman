//! Internal layered graph model.
//!
//! Source references:
//! - https://github.com/eclipse-elk/elk/blob/62d5909f96fad541bc101ad52dabaece6b7eab7e/plugins/org.eclipse.elk.alg.layered/src/org/eclipse/elk/alg/layered/graph/LGraph.java
//! - https://github.com/eclipse-elk/elk/blob/62d5909f96fad541bc101ad52dabaece6b7eab7e/plugins/org.eclipse.elk.alg.layered/src/org/eclipse/elk/alg/layered/graph/LNode.java
//! - https://github.com/eclipse-elk/elk/blob/62d5909f96fad541bc101ad52dabaece6b7eab7e/plugins/org.eclipse.elk.alg.layered/src/org/eclipse/elk/alg/layered/graph/LEdge.java
//! - https://github.com/eclipse-elk/elk/blob/62d5909f96fad541bc101ad52dabaece6b7eab7e/plugins/org.eclipse.elk.alg.layered/src/org/eclipse/elk/alg/layered/graph/LPort.java

use super::options::{LayeredOptions, PortConstraints};
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
    pub hidden_nodes: Vec<usize>,
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
            hidden_nodes: Vec::new(),
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
    pub layer_index: Option<usize>,
    pub layer_constraint: super::options::LayerConstraint,
    pub port_constraints: PortConstraints,
    pub origin_edge: Option<usize>,
    pub long_edge_source: Option<PortRef>,
    pub long_edge_target: Option<PortRef>,
    pub long_edge_has_label_dummies: bool,
    pub layer_constraint_explicit: bool,
    pub hidden: bool,
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
            layer_index: None,
            layer_constraint: super::options::LayerConstraint::None,
            port_constraints: PortConstraints::Undefined,
            origin_edge: None,
            long_edge_source: None,
            long_edge_target: None,
            long_edge_has_label_dummies: false,
            layer_constraint_explicit: false,
            hidden: false,
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
    pub priority_shortness: i32,
    pub thickness: f64,
    pub original_opposite_port: Option<PortRef>,
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
    pub end_label_edge: Option<usize>,
}

impl LLabel {
    pub fn new(text: impl Into<String>, width: f64, height: f64) -> Self {
        Self {
            text: text.into(),
            size: LSize { width, height },
            placement: EdgeLabelPlacement::Center,
            inline: false,
            end_label_edge: None,
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

impl LGraph {
    pub fn clear_layers(&mut self) {
        self.layers.clear();
        for node in &mut self.layerless_nodes {
            node.layer_index = None;
        }
    }

    pub fn set_node_layer(&mut self, node_index: usize, layer_index: usize) {
        while self.layers.len() <= layer_index {
            self.layers.push(Layer { nodes: Vec::new() });
        }

        if let Some(old_layer) = self.layerless_nodes[node_index].layer_index {
            if let Some(layer) = self.layers.get_mut(old_layer) {
                remove_node(&mut layer.nodes, node_index);
            }
        }

        self.layerless_nodes[node_index].layer_index = Some(layer_index);
        self.layers[layer_index].nodes.push(node_index);
    }

    pub fn insert_layer(&mut self, layer_index: usize) {
        self.layers.insert(layer_index, Layer { nodes: Vec::new() });
        for node in &mut self.layerless_nodes {
            if let Some(current) = node.layer_index {
                if current >= layer_index {
                    node.layer_index = Some(current + 1);
                }
            }
        }
    }

    pub fn compact_empty_layers(&mut self) {
        let mut layer_map = vec![None; self.layers.len()];
        let mut compacted = Vec::new();

        for (old_index, layer) in self.layers.iter().enumerate() {
            if !layer.nodes.is_empty() {
                layer_map[old_index] = Some(compacted.len());
                compacted.push(layer.clone());
            }
        }

        self.layers = compacted;
        for node in &mut self.layerless_nodes {
            if let Some(old_index) = node.layer_index {
                node.layer_index = layer_map.get(old_index).copied().flatten();
            }
        }
    }

    pub fn add_port(
        &mut self,
        node_index: usize,
        port_type: PortType,
        side: PortSide,
        position: LPoint,
    ) -> Option<PortRef> {
        let node = self.layerless_nodes.get_mut(node_index)?;
        let port_index = node.ports.len();
        let mut port = LPort::new(format!("{}:{port_index}", node.id), node_index, port_type);
        port.side = side;
        port.position = position;
        node.ports.push(port);
        Some(PortRef {
            node: node_index,
            port: port_index,
        })
    }

    pub fn set_edge_source(&mut self, edge_index: usize, source: PortRef) -> bool {
        if self.edges.get(edge_index).is_none() || !port_exists(self, source) {
            return false;
        }

        let old_source = self.edges[edge_index].source;
        if port_exists(self, old_source) {
            remove_edge(
                &mut self.layerless_nodes[old_source.node].ports[old_source.port].outgoing_edges,
                edge_index,
            );
        }

        self.layerless_nodes[source.node].ports[source.port]
            .outgoing_edges
            .push(edge_index);
        self.edges[edge_index].source = source;
        true
    }

    pub fn set_edge_target(&mut self, edge_index: usize, target: PortRef) -> bool {
        if self.edges.get(edge_index).is_none() || !port_exists(self, target) {
            return false;
        }

        let old_target = self.edges[edge_index].target;
        if port_exists(self, old_target) {
            remove_edge(
                &mut self.layerless_nodes[old_target.node].ports[old_target.port].incoming_edges,
                edge_index,
            );
        }

        self.layerless_nodes[target.node].ports[target.port]
            .incoming_edges
            .push(edge_index);
        self.edges[edge_index].target = target;
        true
    }

    pub fn detach_edge_source(&mut self, edge_index: usize) -> Option<PortRef> {
        let source = self.edges.get(edge_index)?.source;
        if port_exists(self, source) {
            remove_edge(
                &mut self.layerless_nodes[source.node].ports[source.port].outgoing_edges,
                edge_index,
            );
        }
        Some(source)
    }

    pub fn detach_edge_target(&mut self, edge_index: usize) -> Option<PortRef> {
        let target = self.edges.get(edge_index)?.target;
        if port_exists(self, target) {
            remove_edge(
                &mut self.layerless_nodes[target.node].ports[target.port].incoming_edges,
                edge_index,
            );
        }
        Some(target)
    }

    pub fn edge_source_attached(&self, edge_index: usize) -> bool {
        let Some(edge) = self.edges.get(edge_index) else {
            return false;
        };
        self.layerless_nodes
            .get(edge.source.node)
            .and_then(|node| node.ports.get(edge.source.port))
            .map(|port| port.outgoing_edges.contains(&edge_index))
            .unwrap_or(false)
    }

    pub fn edge_target_attached(&self, edge_index: usize) -> bool {
        let Some(edge) = self.edges.get(edge_index) else {
            return false;
        };
        self.layerless_nodes
            .get(edge.target.node)
            .and_then(|node| node.ports.get(edge.target.port))
            .map(|port| port.incoming_edges.contains(&edge_index))
            .unwrap_or(false)
    }

    pub fn add_edge(&mut self, edge: LayeredEdge) -> Option<usize> {
        if !port_exists(self, edge.source) || !port_exists(self, edge.target) {
            return None;
        }

        let edge_index = self.edges.len();
        let source = edge.source;
        let target = edge.target;
        self.edges.push(edge);
        self.layerless_nodes[source.node].ports[source.port]
            .outgoing_edges
            .push(edge_index);
        self.layerless_nodes[target.node].ports[target.port]
            .incoming_edges
            .push(edge_index);
        Some(edge_index)
    }

    pub fn node_incoming_edges(&self, node_index: usize) -> Vec<usize> {
        self.layerless_nodes[node_index]
            .ports
            .iter()
            .flat_map(|port| port.incoming_edges.iter().copied())
            .collect()
    }

    pub fn node_outgoing_edges(&self, node_index: usize) -> Vec<usize> {
        self.layerless_nodes[node_index]
            .ports
            .iter()
            .flat_map(|port| port.outgoing_edges.iter().copied())
            .collect()
    }

    pub fn node_connected_edges(&self, node_index: usize) -> Vec<usize> {
        self.layerless_nodes[node_index]
            .ports
            .iter()
            .flat_map(|port| {
                port.incoming_edges
                    .iter()
                    .chain(port.outgoing_edges.iter())
                    .copied()
            })
            .collect()
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

fn remove_node(nodes: &mut Vec<usize>, node_index: usize) {
    if let Some(position) = nodes.iter().position(|candidate| *candidate == node_index) {
        nodes.remove(position);
    }
}
