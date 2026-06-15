//! Internal layered graph model.
//!
//! Source references:
//! - https://github.com/eclipse-elk/elk/blob/62d5909f96fad541bc101ad52dabaece6b7eab7e/plugins/org.eclipse.elk.alg.layered/src/org/eclipse/elk/alg/layered/graph/LGraph.java
//! - https://github.com/eclipse-elk/elk/blob/62d5909f96fad541bc101ad52dabaece6b7eab7e/plugins/org.eclipse.elk.alg.layered/src/org/eclipse/elk/alg/layered/graph/LNode.java
//! - https://github.com/eclipse-elk/elk/blob/62d5909f96fad541bc101ad52dabaece6b7eab7e/plugins/org.eclipse.elk.alg.layered/src/org/eclipse/elk/alg/layered/graph/LEdge.java
//! - https://github.com/eclipse-elk/elk/blob/62d5909f96fad541bc101ad52dabaece6b7eab7e/plugins/org.eclipse.elk.alg.layered/src/org/eclipse/elk/alg/layered/graph/LPort.java

use super::options::{
    Alignment, ElkDirection, LayerConstraint, LayeredOptions, PortAlignment, PortConstraints,
};
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
    pub replaced_external_port_dummies: Vec<usize>,
    pub cross_hierarchy_edges: Vec<CrossHierarchyEdge>,
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
            replaced_external_port_dummies: Vec::new(),
            cross_hierarchy_edges: Vec::new(),
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
    pub size: LSize,
}

impl Layer {
    pub fn new() -> Self {
        Self {
            nodes: Vec::new(),
            size: LSize::default(),
        }
    }
}

impl Default for Layer {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct LNode {
    pub id: String,
    pub kind: LNodeKind,
    pub size: LSize,
    pub position: LPoint,
    pub margin: LMargin,
    pub padding: LPadding,
    pub labels: Vec<LLabel>,
    pub ports: Vec<LPort>,
    pub nested_graph: Option<Box<LGraph>>,
    pub model_order: Option<usize>,
    pub layer_index: Option<usize>,
    pub layer_constraint: super::options::LayerConstraint,
    pub node_alignment: Alignment,
    pub port_constraints: PortConstraints,
    pub port_alignment: Option<PortAlignment>,
    pub external_port_side: PortSide,
    pub external_port_size: LSize,
    pub port_ratio_or_position: f64,
    pub replaced_external_port_dummy: Option<usize>,
    pub in_layer_successor_constraints: Vec<usize>,
    pub origin_port: Option<GraphPortRef>,
    pub origin_edge: Option<usize>,
    pub long_edge_source: Option<PortRef>,
    pub long_edge_target: Option<PortRef>,
    pub long_edge_has_label_dummies: bool,
    pub label_side: LabelSide,
    pub in_layer_constraint: InLayerConstraint,
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
            margin: LMargin::default(),
            padding: LPadding::default(),
            labels: Vec::new(),
            ports: Vec::new(),
            nested_graph: None,
            model_order,
            layer_index: None,
            layer_constraint: super::options::LayerConstraint::None,
            node_alignment: Alignment::Automatic,
            port_constraints: PortConstraints::Undefined,
            port_alignment: None,
            external_port_side: PortSide::Undefined,
            external_port_size: LSize::default(),
            port_ratio_or_position: 0.0,
            replaced_external_port_dummy: None,
            in_layer_successor_constraints: Vec::new(),
            origin_port: None,
            origin_edge: None,
            long_edge_source: None,
            long_edge_target: None,
            long_edge_has_label_dummies: false,
            label_side: LabelSide::Below,
            in_layer_constraint: InLayerConstraint::None,
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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum InLayerConstraint {
    #[default]
    None,
    Top,
    Bottom,
}

#[derive(Debug, Clone, PartialEq)]
pub struct LPort {
    pub id: String,
    pub node: usize,
    pub port_type: PortType,
    pub side: PortSide,
    pub size: LSize,
    pub position: LPoint,
    pub anchor: LPoint,
    pub margin: LMargin,
    pub labels: Vec<LLabel>,
    pub model_order: Option<usize>,
    pub port_index: Option<isize>,
    pub border_offset: Option<f64>,
    pub ratio_or_position: f64,
    pub connected_to_external_nodes: bool,
    pub port_dummy: Option<GraphNodeRef>,
    pub inside_connections: bool,
    pub end_label_cell: Option<LabelCellLayout>,
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
            size: LSize::default(),
            position: LPoint::default(),
            anchor: LPoint::default(),
            margin: LMargin::default(),
            labels: Vec::new(),
            model_order: None,
            port_index: None,
            border_offset: None,
            ratio_or_position: 0.0,
            connected_to_external_nodes: true,
            port_dummy: None,
            inside_connections: false,
            end_label_cell: None,
            incoming_edges: Vec::new(),
            outgoing_edges: Vec::new(),
        }
    }

    pub fn set_side(&mut self, side: PortSide) {
        self.side = side;
        self.anchor = match side {
            PortSide::North => LPoint {
                x: self.size.width / 2.0,
                y: 0.0,
            },
            PortSide::East => LPoint {
                x: self.size.width,
                y: self.size.height / 2.0,
            },
            PortSide::South => LPoint {
                x: self.size.width / 2.0,
                y: self.size.height,
            },
            PortSide::West => LPoint {
                x: 0.0,
                y: self.size.height / 2.0,
            },
            PortSide::Undefined => self.anchor,
        };
    }

    pub fn net_flow(&self) -> isize {
        self.incoming_edges.len() as isize - self.outgoing_edges.len() as isize
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
    pub priority_straightness: i32,
    pub thickness: f64,
    pub original_opposite_port: Option<PortRef>,
    pub compound_segment: Option<CompoundEdgeSegment>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct PortRef {
    pub node: usize,
    pub port: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GraphNodeRef {
    pub graph_id: String,
    pub node: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GraphPortRef {
    pub graph_id: String,
    pub port: PortRef,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CompoundEdgeSegment {
    Output { depth: usize },
    Input { depth: usize },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CrossHierarchyEdge {
    pub original_edge_id: String,
    pub graph_id: String,
    pub edge: usize,
    pub segment: CompoundEdgeSegment,
}

#[derive(Debug, Clone, PartialEq)]
pub struct LLabel {
    pub text: String,
    pub size: LSize,
    pub position: LPoint,
    pub placement: EdgeLabelPlacement,
    pub inline: bool,
    pub label_side: Option<LabelSide>,
    pub end_label_edge: Option<usize>,
}

impl LLabel {
    pub fn new(text: impl Into<String>, width: f64, height: f64) -> Self {
        Self {
            text: text.into(),
            size: LSize { width, height },
            position: LPoint::default(),
            placement: EdgeLabelPlacement::Center,
            inline: false,
            label_side: None,
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

#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct LMargin {
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

impl PortSide {
    pub fn opposed(self) -> Self {
        match self {
            Self::North => Self::South,
            Self::East => Self::West,
            Self::South => Self::North,
            Self::West => Self::East,
            Self::Undefined => Self::Undefined,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct LabelCellLayout {
    pub position: LPoint,
    pub size: LSize,
    pub horizontal_layout: bool,
    pub horizontal_alignment: HorizontalLabelAlignment,
    pub vertical_alignment: VerticalLabelAlignment,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum HorizontalLabelAlignment {
    Left,
    #[default]
    Center,
    Right,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum VerticalLabelAlignment {
    Top,
    #[default]
    Center,
    Bottom,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum EdgeLabelPlacement {
    Head,
    Tail,
    #[default]
    Center,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum LabelSide {
    Above,
    #[default]
    Below,
    Inline,
}

impl LabelSide {
    pub fn opposite(self) -> Self {
        match self {
            Self::Above => Self::Below,
            Self::Below => Self::Above,
            Self::Inline => Self::Inline,
        }
    }
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
            self.layers.push(Layer::new());
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
        self.layers.insert(layer_index, Layer::new());
        for node in &mut self.layerless_nodes {
            if let Some(current) = node.layer_index {
                if current >= layer_index {
                    node.layer_index = Some(current + 1);
                }
            }
        }
    }

    pub fn remove_node_from_layer(&mut self, node_index: usize) {
        let Some(layer_index) = self.layerless_nodes[node_index].layer_index.take() else {
            return;
        };
        if let Some(layer) = self.layers.get_mut(layer_index) {
            remove_node(&mut layer.nodes, node_index);
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
        port.set_side(side);
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
        self.set_edge_target_at(edge_index, target, None)
    }

    pub fn set_edge_target_at(
        &mut self,
        edge_index: usize,
        target: PortRef,
        insert_at: Option<usize>,
    ) -> bool {
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

        let incoming_edges =
            &mut self.layerless_nodes[target.node].ports[target.port].incoming_edges;
        if let Some(index) = insert_at {
            incoming_edges.insert(index.min(incoming_edges.len()), edge_index);
        } else {
            incoming_edges.push(edge_index);
        }
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

    pub fn detach_edge(&mut self, edge_index: usize) -> Option<(PortRef, PortRef)> {
        let source = self.detach_edge_source(edge_index)?;
        let target = self.detach_edge_target(edge_index)?;
        Some((source, target))
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

    /// ELK's `LNode#getPorts(PortType.OUTPUT)` is based on actual outgoing edge incidence.
    pub fn port_has_output_type(&self, port_ref: PortRef) -> bool {
        self.layerless_nodes
            .get(port_ref.node)
            .and_then(|node| node.ports.get(port_ref.port))
            .map(|port| !port.outgoing_edges.is_empty())
            .unwrap_or(false)
    }

    /// ELK's `LNode#getPorts(PortType.INPUT)` is based on actual incoming edge incidence.
    pub fn port_has_input_type(&self, port_ref: PortRef) -> bool {
        self.layerless_nodes
            .get(port_ref.node)
            .and_then(|node| node.ports.get(port_ref.port))
            .map(|port| !port.incoming_edges.is_empty())
            .unwrap_or(false)
    }

    pub fn reorder_node_ports(
        &mut self,
        node_index: usize,
        new_order: impl IntoIterator<Item = usize>,
    ) -> bool {
        let Some(node) = self.layerless_nodes.get(node_index) else {
            return false;
        };
        let port_count = node.ports.len();
        let new_order = new_order.into_iter().collect::<Vec<_>>();
        if new_order.len() != port_count {
            return false;
        }

        let mut seen = vec![false; port_count];
        for old_index in &new_order {
            if *old_index >= port_count || seen[*old_index] {
                return false;
            }
            seen[*old_index] = true;
        }

        let old_ports = std::mem::take(&mut self.layerless_nodes[node_index].ports);
        let mut old_to_new = vec![0usize; old_ports.len()];
        let mut reordered = Vec::with_capacity(old_ports.len());

        for (new_index, old_index) in new_order.into_iter().enumerate() {
            old_to_new[old_index] = new_index;
            let mut port = old_ports[old_index].clone();
            port.node = node_index;
            reordered.push(port);
        }

        self.layerless_nodes[node_index].ports = reordered;

        for edge in &mut self.edges {
            if edge.source.node == node_index {
                edge.source.port = old_to_new[edge.source.port];
            }
            if edge.target.node == node_index {
                edge.target.port = old_to_new[edge.target.port];
            }
            if let Some(port) = edge.original_opposite_port.as_mut()
                && port.node == node_index
            {
                port.port = old_to_new[port.port];
            }
        }

        let graph_id = self.id.clone();
        for node in &mut self.layerless_nodes {
            update_graph_port_ref_after_reorder(
                &mut node.origin_port,
                graph_id.as_str(),
                node_index,
                &old_to_new,
            );
            if let Some(port) = node.long_edge_source.as_mut()
                && port.node == node_index
            {
                port.port = old_to_new[port.port];
            }
            if let Some(port) = node.long_edge_target.as_mut()
                && port.node == node_index
            {
                port.port = old_to_new[port.port];
            }
        }

        true
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

fn update_graph_port_ref_after_reorder(
    graph_ref: &mut Option<GraphPortRef>,
    graph_id: &str,
    node_index: usize,
    old_to_new: &[usize],
) {
    if let Some(port_ref) = graph_ref.as_mut()
        && port_ref.graph_id == graph_id
        && port_ref.port.node == node_index
    {
        port_ref.port.port = old_to_new[port_ref.port.port];
    }
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

/// Create an external port dummy node following ELK's `LGraphUtil.createExternalPortDummy(...)`.
///
/// Source:
/// https://github.com/eclipse-elk/elk/blob/62d5909f96fad541bc101ad52dabaece6b7eab7e/plugins/org.eclipse.elk.alg.layered/src/org/eclipse/elk/alg/layered/graph/LGraphUtil.java
pub fn create_external_port_dummy(
    id: impl Into<String>,
    port_id: impl Into<String>,
    port_type: PortType,
    port_constraints: PortConstraints,
    port_side: PortSide,
    net_flow: isize,
    port_position: LPoint,
    port_size: LSize,
    port_node_size: LSize,
    border_offset: f64,
    layout_direction: ElkDirection,
) -> LNode {
    let external_side = if port_constraints.is_side_fixed() {
        port_side
    } else if net_flow >= 0 {
        port_side_from_direction(layout_direction)
    } else {
        port_side_from_direction(layout_direction).opposed()
    };

    let mut dummy = LNode::new(id, 0.0, 0.0, None);
    dummy.kind = LNodeKind::ExternalPort;
    dummy.port_constraints = PortConstraints::FixedPos;
    dummy.external_port_side = external_side;
    dummy.external_port_size = port_size;

    let mut anchor = LPoint {
        x: port_size.width / 2.0,
        y: port_size.height / 2.0,
    };
    let mut dummy_port_side = external_side.opposed();

    match external_side {
        PortSide::West => {
            dummy.layer_constraint = LayerConstraint::FirstSeparate;
            dummy.layer_constraint_explicit = true;
            dummy.size.height = port_size.height;
            if border_offset < 0.0 {
                dummy.size.width = -border_offset;
            }
            anchor.x = 0.0;
        }
        PortSide::East => {
            dummy.layer_constraint = LayerConstraint::LastSeparate;
            dummy.layer_constraint_explicit = true;
            dummy.size.height = port_size.height;
            if border_offset < 0.0 {
                dummy.size.width = -border_offset;
            }
            anchor.x = 0.0;
        }
        PortSide::North => {
            dummy.in_layer_constraint = InLayerConstraint::Top;
            dummy.size.width = port_size.width;
            if border_offset < 0.0 {
                dummy.size.height = -border_offset;
            }
            anchor.y = 0.0;
        }
        PortSide::South => {
            dummy.in_layer_constraint = InLayerConstraint::Bottom;
            dummy.size.width = port_size.width;
            if border_offset < 0.0 {
                dummy.size.height = -border_offset;
            }
            anchor.y = 0.0;
        }
        PortSide::Undefined => {
            dummy_port_side = PortSide::Undefined;
        }
    }

    let mut port = LPort::new(port_id, 0, port_type);
    port.side = dummy_port_side;
    port.position = anchor;
    port.anchor = LPoint::default();
    port.size = port_size;
    port.border_offset = Some(border_offset);
    if port_constraints.is_order_fixed() {
        dummy.port_ratio_or_position = port_ratio_or_position(
            external_side,
            port_position,
            port_node_size,
            port_constraints.is_ratio_fixed(),
        );
    }
    dummy.ports.push(port);
    dummy
}

fn port_side_from_direction(direction: ElkDirection) -> PortSide {
    match direction {
        ElkDirection::Right | ElkDirection::Undefined => PortSide::East,
        ElkDirection::Left => PortSide::West,
        ElkDirection::Down => PortSide::South,
        ElkDirection::Up => PortSide::North,
    }
}

fn port_ratio_or_position(
    side: PortSide,
    port_position: LPoint,
    port_node_size: LSize,
    ratio_fixed: bool,
) -> f64 {
    match side {
        PortSide::West | PortSide::East => {
            if ratio_fixed && port_node_size.height > 0.0 {
                port_position.y / port_node_size.height
            } else {
                port_position.y
            }
        }
        PortSide::North | PortSide::South => {
            if ratio_fixed && port_node_size.width > 0.0 {
                port_position.x / port_node_size.width
            } else {
                port_position.x
            }
        }
        PortSide::Undefined => 0.0,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn external_port_dummy_uses_direction_and_net_flow_for_free_ports() {
        let dummy = create_external_port_dummy(
            "external:A",
            "external:A:0",
            PortType::Input,
            PortConstraints::Free,
            PortSide::Undefined,
            1,
            LPoint::default(),
            LSize {
                width: 4.0,
                height: 6.0,
            },
            LSize::default(),
            0.0,
            ElkDirection::Down,
        );

        assert_eq!(dummy.kind, LNodeKind::ExternalPort);
        assert_eq!(dummy.in_layer_constraint, InLayerConstraint::Bottom);
        assert_eq!(dummy.port_constraints, PortConstraints::FixedPos);
        assert_eq!(dummy.ports[0].side, PortSide::North);
        assert_eq!(dummy.ports[0].position, LPoint { x: 2.0, y: 0.0 });
    }

    #[test]
    fn external_port_dummy_keeps_fixed_side_and_order_metadata() {
        let dummy = create_external_port_dummy(
            "external:A",
            "external:A:0",
            PortType::Output,
            PortConstraints::FixedRatio,
            PortSide::West,
            -1,
            LPoint { x: 0.0, y: 40.0 },
            LSize {
                width: 4.0,
                height: 6.0,
            },
            LSize {
                width: 20.0,
                height: 80.0,
            },
            -3.0,
            ElkDirection::Right,
        );

        assert_eq!(dummy.layer_constraint, LayerConstraint::FirstSeparate);
        assert_eq!(dummy.size.width, 3.0);
        assert_eq!(dummy.size.height, 6.0);
        assert_eq!(dummy.ports[0].side, PortSide::East);
        assert_eq!(dummy.ports[0].position, LPoint { x: 0.0, y: 3.0 });
        assert_eq!(dummy.ports[0].border_offset, Some(-3.0));
        assert_eq!(dummy.port_ratio_or_position, 0.5);
    }
}
