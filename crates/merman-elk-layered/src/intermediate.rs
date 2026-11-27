//! Intermediate layered processors.
//!
//! Source references:
//! - https://github.com/eclipse-elk/elk/blob/62d5909f96fad541bc101ad52dabaece6b7eab7e/plugins/org.eclipse.elk.alg.layered/src/org/eclipse/elk/alg/layered/intermediate/ReversedEdgeRestorer.java
//! - https://github.com/eclipse-elk/elk/blob/62d5909f96fad541bc101ad52dabaece6b7eab7e/plugins/org.eclipse.elk.alg.layered/src/org/eclipse/elk/alg/layered/intermediate/EdgeAndLayerConstraintEdgeReverser.java
//! - https://github.com/eclipse-elk/elk/blob/62d5909f96fad541bc101ad52dabaece6b7eab7e/plugins/org.eclipse.elk.alg.layered/src/org/eclipse/elk/alg/layered/intermediate/LayerConstraintPreprocessor.java
//! - https://github.com/eclipse-elk/elk/blob/62d5909f96fad541bc101ad52dabaece6b7eab7e/plugins/org.eclipse.elk.alg.layered/src/org/eclipse/elk/alg/layered/intermediate/LayerConstraintPostprocessor.java
//! - https://github.com/eclipse-elk/elk/blob/62d5909f96fad541bc101ad52dabaece6b7eab7e/plugins/org.eclipse.elk.alg.layered/src/org/eclipse/elk/alg/layered/intermediate/LongEdgeSplitter.java
//! - https://github.com/eclipse-elk/elk/blob/62d5909f96fad541bc101ad52dabaece6b7eab7e/plugins/org.eclipse.elk.alg.layered/src/org/eclipse/elk/alg/layered/intermediate/LongEdgeJoiner.java
//! - https://github.com/eclipse-elk/elk/blob/62d5909f96fad541bc101ad52dabaece6b7eab7e/plugins/org.eclipse.elk.alg.layered/src/org/eclipse/elk/alg/layered/intermediate/LayerSizeAndGraphHeightCalculator.java
//! - https://github.com/eclipse-elk/elk/blob/62d5909f96fad541bc101ad52dabaece6b7eab7e/plugins/org.eclipse.elk.alg.layered/src/org/eclipse/elk/alg/layered/intermediate/InteractiveExternalPortPositioner.java
//! - https://github.com/eclipse-elk/elk/blob/62d5909f96fad541bc101ad52dabaece6b7eab7e/plugins/org.eclipse.elk.alg.layered/src/org/eclipse/elk/alg/layered/intermediate/InvertedPortProcessor.java

use std::collections::{HashMap, HashSet};

use crate::graph::{
    EdgeLabelPlacement, LGraph, LNode, LNodeKind, LPoint, LayeredEdge, PortRef, PortSide, PortType,
    reverse_edge,
};
use crate::options::{LayerConstraint, PortConstraints};

mod edge_labels;
mod hierarchical_ports;
pub use edge_labels::{
    insert_label_dummies, postprocess_end_labels, preprocess_end_labels, remove_label_dummies,
    select_label_sides, sort_end_labels, switch_label_dummies,
};
pub use hierarchical_ports::{
    process_hierarchical_port_constraints, process_hierarchical_port_dummy_sizes,
    process_hierarchical_port_orthogonal_edges, process_hierarchical_port_positions,
};

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum IntermediateError {
    #[error(
        "node `{node_id}` has layer constraint FIRST_SEPARATE but has incoming edge `{edge_id}`"
    )]
    FirstSeparateIncomingEdge { node_id: String, edge_id: String },
    #[error(
        "node `{node_id}` has layer constraint LAST_SEPARATE but has outgoing edge `{edge_id}`"
    )]
    LastSeparateOutgoingEdge { node_id: String, edge_id: String },
    #[error("node `{node_id}` has layer constraint FIRST but has incoming edge `{edge_id}`")]
    FirstIncomingEdge { node_id: String, edge_id: String },
    #[error("node `{node_id}` has layer constraint LAST but has outgoing edge `{edge_id}`")]
    LastOutgoingEdge { node_id: String, edge_id: String },
}

pub type IntermediateResult<T> = Result<T, IntermediateError>;

const INTERACTIVE_EXTERNAL_PORT_SPACING: f64 = 10.0;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum EdgeReversalPortType {
    Input,
    Output,
    All,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum HiddenNodeConnections {
    None,
    FirstSeparate,
    LastSeparate,
    Both,
}

pub fn position_interactive_external_ports(graph: &mut LGraph) {
    if !graph.graph_properties.external_ports {
        return;
    }

    let Some(bounds) = interactive_normal_node_bounds(graph) else {
        return;
    };

    for node_index in 0..graph.layerless_nodes.len() {
        if graph.layerless_nodes[node_index].kind != LNodeKind::ExternalPort {
            continue;
        }

        if graph.layerless_nodes[node_index].layer_constraint == LayerConstraint::FirstSeparate {
            graph.layerless_nodes[node_index].position.x =
                bounds.min_x - INTERACTIVE_EXTERNAL_PORT_SPACING;
            if let Some(y) = first_connected_other_node_center_y(graph, node_index, true) {
                graph.layerless_nodes[node_index].position.y = y;
            }
            continue;
        }

        if graph.layerless_nodes[node_index].layer_constraint == LayerConstraint::LastSeparate {
            graph.layerless_nodes[node_index].position.x =
                bounds.max_x + INTERACTIVE_EXTERNAL_PORT_SPACING;
            if let Some(y) = first_connected_other_node_center_y(graph, node_index, false) {
                graph.layerless_nodes[node_index].position.y = y;
            }
            continue;
        }

        match graph.layerless_nodes[node_index].in_layer_constraint {
            crate::graph::InLayerConstraint::Top => {
                if let Some(x) = north_south_external_port_x(graph, node_index) {
                    graph.layerless_nodes[node_index].position.x =
                        x + INTERACTIVE_EXTERNAL_PORT_SPACING;
                }
                graph.layerless_nodes[node_index].position.y =
                    bounds.min_y - INTERACTIVE_EXTERNAL_PORT_SPACING;
            }
            crate::graph::InLayerConstraint::Bottom => {
                if let Some(x) = north_south_external_port_x(graph, node_index) {
                    graph.layerless_nodes[node_index].position.x =
                        x + INTERACTIVE_EXTERNAL_PORT_SPACING;
                }
                graph.layerless_nodes[node_index].position.y =
                    bounds.max_y + INTERACTIVE_EXTERNAL_PORT_SPACING;
            }
            crate::graph::InLayerConstraint::None => {}
        }
    }
}

#[derive(Debug, Clone, Copy)]
struct InteractiveBounds {
    min_x: f64,
    max_x: f64,
    min_y: f64,
    max_y: f64,
}

fn interactive_normal_node_bounds(graph: &LGraph) -> Option<InteractiveBounds> {
    let mut bounds: Option<InteractiveBounds> = None;
    for node in graph
        .layerless_nodes
        .iter()
        .filter(|node| node.kind == LNodeKind::Normal)
    {
        let current = InteractiveBounds {
            min_x: node.position.x - node.margin.left,
            max_x: node.position.x + node.size.width + node.margin.right,
            min_y: node.position.y - node.margin.top,
            max_y: node.position.y + node.size.height + node.margin.bottom,
        };
        bounds = Some(match bounds {
            None => current,
            Some(previous) => InteractiveBounds {
                min_x: previous.min_x.min(current.min_x),
                max_x: previous.max_x.max(current.max_x),
                min_y: previous.min_y.min(current.min_y),
                max_y: previous.max_y.max(current.max_y),
            },
        });
    }
    bounds
}

fn first_connected_other_node_center_y(
    graph: &LGraph,
    dummy_index: usize,
    other_is_target: bool,
) -> Option<f64> {
    if let Some(edge_index) = graph.node_connected_edges(dummy_index).into_iter().next() {
        let edge = &graph.edges[edge_index];
        let other = if other_is_target {
            edge.target.node
        } else {
            edge.source.node
        };
        let node = graph.layerless_nodes.get(other)?;
        Some(node.position.y + node.size.height / 2.0)
    } else {
        None
    }
}

fn north_south_external_port_x(graph: &LGraph, dummy_index: usize) -> Option<f64> {
    let port = graph.layerless_nodes.get(dummy_index)?.ports.first()?;
    if !port.outgoing_edges.is_empty() && !port.incoming_edges.is_empty() {
        return None;
    }

    if !port.outgoing_edges.is_empty() {
        return port
            .outgoing_edges
            .iter()
            .filter_map(|edge_index| graph.edges.get(*edge_index))
            .filter_map(|edge| graph.layerless_nodes.get(edge.target.node))
            .map(|node| node.position.x - node.margin.left)
            .reduce(f64::min);
    }

    if !port.incoming_edges.is_empty() {
        return port
            .incoming_edges
            .iter()
            .filter_map(|edge_index| graph.edges.get(*edge_index))
            .filter_map(|edge| graph.layerless_nodes.get(edge.source.node))
            .map(|node| node.position.x + node.size.width + node.margin.right)
            .reduce(f64::max);
    }

    None
}

impl HiddenNodeConnections {
    fn combine(self, layer_constraint: LayerConstraint) -> Self {
        match self {
            Self::None => {
                if layer_constraint == LayerConstraint::FirstSeparate {
                    Self::FirstSeparate
                } else {
                    Self::LastSeparate
                }
            }
            Self::FirstSeparate => {
                if layer_constraint == LayerConstraint::FirstSeparate {
                    Self::FirstSeparate
                } else {
                    Self::Both
                }
            }
            Self::LastSeparate => {
                if layer_constraint == LayerConstraint::FirstSeparate {
                    Self::Both
                } else {
                    Self::LastSeparate
                }
            }
            Self::Both => Self::Both,
        }
    }
}

pub fn restore_reversed_edges(graph: &mut LGraph) {
    for layer_index in 0..graph.layers.len() {
        let nodes = graph.layers[layer_index].nodes.clone();
        for node_index in nodes {
            let Some(node) = graph.layerless_nodes.get(node_index) else {
                continue;
            };
            let port_count = node.ports.len();

            for port_index in 0..port_count {
                let outgoing_edges = graph.layerless_nodes[node_index].ports[port_index]
                    .outgoing_edges
                    .clone();
                for edge_index in outgoing_edges {
                    if graph.edges[edge_index].reversed {
                        reverse_edge(graph, edge_index, false);
                    }
                }
            }
        }
    }
}

pub fn reverse_edges_for_edge_and_layer_constraints(graph: &mut LGraph) {
    let mut remaining_nodes = Vec::new();

    for node in 0..graph.layerless_nodes.len() {
        let layer_constraint = graph.layerless_nodes[node].layer_constraint;
        if let Some(target_port_type) = target_port_type_for_layer_constraint(layer_constraint) {
            reverse_node_edges(graph, node, layer_constraint, target_port_type);
        } else {
            remaining_nodes.push(node);
        }
    }

    for node in remaining_nodes {
        if should_reverse_all_fixed_side_edges(graph, node) {
            let layer_constraint = graph.layerless_nodes[node].layer_constraint;
            reverse_node_edges(graph, node, layer_constraint, EdgeReversalPortType::All);
        }
    }
}

pub(crate) fn visit_edge_and_layer_constraint_reversal_candidates(
    graph: &LGraph,
    mut visit: impl FnMut(usize) -> bool,
) -> bool {
    ConstraintReversalShadow::new(graph).visit_candidates(&mut visit)
}

#[cfg(test)]
pub(crate) fn edge_and_layer_constraint_reversal_may_mutate(graph: &LGraph) -> bool {
    visit_edge_and_layer_constraint_reversal_candidates(graph, |_| true)
}

struct ConstraintReversalShadow<'a> {
    graph: &'a LGraph,
    reversed_by_processor: Vec<bool>,
    port_flows: Vec<i128>,
}

struct ShadowCollectorSlots {
    input: usize,
    output: usize,
    input_is_virtual: bool,
    output_is_virtual: bool,
    virtual_input_used: bool,
    virtual_output_used: bool,
}

impl<'a> ConstraintReversalShadow<'a> {
    fn new(graph: &'a LGraph) -> Self {
        Self {
            graph,
            reversed_by_processor: vec![false; graph.edges.len()],
            port_flows: Vec::new(),
        }
    }

    fn visit_candidates(&mut self, visit: &mut impl FnMut(usize) -> bool) -> bool {
        // The pinned processor completes every direct layer-constraint reversal before it walks
        // `remainingNodes` in model order. A fixed-side decision therefore observes all direct
        // mutations plus earlier fixed-side mutations; the shadow bitset preserves that order
        // without cloning labels, geometry, nested graphs, or adjacency vectors.
        for node in &self.graph.layerless_nodes {
            let Some(target_port_type) =
                target_port_type_for_layer_constraint(node.layer_constraint)
            else {
                continue;
            };
            for port in &node.ports {
                let edges = match target_port_type {
                    EdgeReversalPortType::Input => &port.outgoing_edges,
                    EdgeReversalPortType::Output => &port.incoming_edges,
                    EdgeReversalPortType::All => {
                        unreachable!("direct constraints select one edge side")
                    }
                };
                for &edge in edges {
                    let may_reverse = match target_port_type {
                        EdgeReversalPortType::Input => {
                            can_reverse_outgoing_edge(self.graph, node.layer_constraint, edge)
                        }
                        EdgeReversalPortType::Output => {
                            can_reverse_incoming_edge(self.graph, node.layer_constraint, edge)
                        }
                        EdgeReversalPortType::All => unreachable!(),
                    };
                    if may_reverse && self.mark_reversed(edge, visit) {
                        return true;
                    }
                }
            }
        }

        for node_index in 0..self.graph.layerless_nodes.len() {
            let node = &self.graph.layerless_nodes[node_index];
            if node.layer_constraint != LayerConstraint::None
                || !self.fixed_side_node_should_reverse(node_index)
            {
                continue;
            }

            for port in &node.ports {
                for &edge in &port.outgoing_edges {
                    if self.mark_reversed(edge, visit) {
                        return true;
                    }
                }
                for &edge in &port.incoming_edges {
                    if self.mark_reversed(edge, visit) {
                        return true;
                    }
                }
            }
        }

        false
    }

    fn mark_reversed(&mut self, edge_index: usize, visit: &mut impl FnMut(usize) -> bool) -> bool {
        let Some(edge) = self.graph.edges.get(edge_index) else {
            return false;
        };
        let Some(reversed_by_processor) = self.reversed_by_processor.get_mut(edge_index) else {
            return false;
        };
        if edge.reversed || *reversed_by_processor {
            return false;
        }

        *reversed_by_processor = true;
        visit(edge_index)
    }

    fn fixed_side_node_should_reverse(&mut self, node_index: usize) -> bool {
        let node = &self.graph.layerless_nodes[node_index];
        if !node.port_constraints.is_side_fixed() || node.ports.is_empty() {
            return false;
        }

        let original_port_count = node.ports.len();
        self.port_flows.clear();
        self.port_flows.extend(
            node.ports
                .iter()
                .map(|port| port.incoming_edges.len() as i128 - port.outgoing_edges.len() as i128),
        );
        self.port_flows.resize(original_port_count + 2, 0);

        let input_collector = node
            .ports
            .iter()
            .position(|port| port.collector_type == Some(PortType::Input));
        let output_collector = node
            .ports
            .iter()
            .position(|port| port.collector_type == Some(PortType::Output));
        let mut collectors = ShadowCollectorSlots {
            input: input_collector.unwrap_or(original_port_count),
            output: output_collector.unwrap_or(original_port_count + 1),
            input_is_virtual: input_collector.is_none(),
            output_is_virtual: output_collector.is_none(),
            virtual_input_used: false,
            virtual_output_used: false,
        };

        for port in &node.ports {
            for &edge in &port.outgoing_edges {
                self.apply_reversed_edge_flow(node_index, edge, &mut collectors);
            }
            for &edge in &port.incoming_edges {
                if self
                    .graph
                    .edges
                    .get(edge)
                    .is_some_and(|edge| edge.source.node == node_index)
                {
                    continue;
                }
                self.apply_reversed_edge_flow(node_index, edge, &mut collectors);
            }
        }

        if node
            .ports
            .iter()
            .enumerate()
            .any(|(port, data)| !port_flow_matches_side(data.side, self.port_flows[port]))
        {
            return false;
        }
        if collectors.virtual_input_used
            && !port_flow_matches_side(PortSide::West, self.port_flows[collectors.input])
        {
            return false;
        }
        if collectors.virtual_output_used
            && !port_flow_matches_side(PortSide::East, self.port_flows[collectors.output])
        {
            return false;
        }

        for port in &node.ports {
            for &edge in &port.outgoing_edges {
                if self.current_edge_violates_fixed_side_constraint(node_index, edge) {
                    return false;
                }
            }
            for &edge in &port.incoming_edges {
                if self
                    .graph
                    .edges
                    .get(edge)
                    .is_some_and(|edge| edge.source.node == node_index)
                {
                    continue;
                }
                if self.current_edge_violates_fixed_side_constraint(node_index, edge) {
                    return false;
                }
            }
        }

        true
    }

    fn apply_reversed_edge_flow(
        &mut self,
        node_index: usize,
        edge_index: usize,
        collectors: &mut ShadowCollectorSlots,
    ) {
        if !self
            .reversed_by_processor
            .get(edge_index)
            .copied()
            .unwrap_or(false)
        {
            return;
        }
        let Some(edge) = self.graph.edges.get(edge_index) else {
            return;
        };

        if edge.source.node == node_index {
            self.port_flows[edge.source.port] += 1;
            let source_port = &self.graph.layerless_nodes[node_index].ports[edge.source.port];
            let new_target = if source_port.collector_type == Some(PortType::Output) {
                if collectors.input_is_virtual {
                    collectors.virtual_input_used = true;
                }
                collectors.input
            } else {
                edge.source.port
            };
            self.port_flows[new_target] += 1;
        }

        if edge.target.node == node_index {
            self.port_flows[edge.target.port] -= 1;
            let target_port = &self.graph.layerless_nodes[node_index].ports[edge.target.port];
            let new_source = if target_port.collector_type == Some(PortType::Input) {
                if collectors.output_is_virtual {
                    collectors.virtual_output_used = true;
                }
                collectors.output
            } else {
                edge.target.port
            };
            self.port_flows[new_source] -= 1;
        }
    }

    fn current_edge_violates_fixed_side_constraint(
        &self,
        node_index: usize,
        edge_index: usize,
    ) -> bool {
        let Some(edge) = self.graph.edges.get(edge_index) else {
            return false;
        };
        let reversed = self
            .reversed_by_processor
            .get(edge_index)
            .copied()
            .unwrap_or(false);
        let (source, target) = if reversed {
            (edge.target.node, edge.source.node)
        } else {
            (edge.source.node, edge.target.node)
        };

        (source == node_index
            && matches!(
                self.graph.layerless_nodes[target].layer_constraint,
                LayerConstraint::Last | LayerConstraint::LastSeparate
            ))
            || (target == node_index
                && matches!(
                    self.graph.layerless_nodes[source].layer_constraint,
                    LayerConstraint::First | LayerConstraint::FirstSeparate
                ))
    }
}

fn port_flow_matches_side(side: PortSide, net_flow: i128) -> bool {
    match side {
        PortSide::East => net_flow > 0,
        PortSide::West => net_flow < 0,
        PortSide::North | PortSide::South | PortSide::Undefined => false,
    }
}

fn target_port_type_for_layer_constraint(
    layer_constraint: LayerConstraint,
) -> Option<EdgeReversalPortType> {
    match layer_constraint {
        LayerConstraint::First | LayerConstraint::FirstSeparate => {
            Some(EdgeReversalPortType::Output)
        }
        LayerConstraint::Last | LayerConstraint::LastSeparate => Some(EdgeReversalPortType::Input),
        LayerConstraint::None => None,
    }
}

fn should_reverse_all_fixed_side_edges(graph: &LGraph, node: usize) -> bool {
    let lnode = &graph.layerless_nodes[node];
    if !lnode.port_constraints.is_side_fixed() || lnode.ports.is_empty() {
        return false;
    }

    for port in &lnode.ports {
        let reversed_port = match port.side {
            PortSide::East => port.net_flow() > 0,
            PortSide::West => port.net_flow() < 0,
            PortSide::North | PortSide::South | PortSide::Undefined => false,
        };
        if !reversed_port {
            return false;
        }

        for edge in &port.outgoing_edges {
            let target_layer_constraint =
                graph.layerless_nodes[graph.edges[*edge].target.node].layer_constraint;
            if matches!(
                target_layer_constraint,
                LayerConstraint::Last | LayerConstraint::LastSeparate
            ) {
                return false;
            }
        }
        for edge in &port.incoming_edges {
            let source_layer_constraint =
                graph.layerless_nodes[graph.edges[*edge].source.node].layer_constraint;
            if matches!(
                source_layer_constraint,
                LayerConstraint::First | LayerConstraint::FirstSeparate
            ) {
                return false;
            }
        }
    }

    true
}

fn reverse_node_edges(
    graph: &mut LGraph,
    node: usize,
    node_layer_constraint: LayerConstraint,
    target_port_type: EdgeReversalPortType,
) {
    let port_count = graph.layerless_nodes[node].ports.len();
    for port in 0..port_count {
        if matches!(
            target_port_type,
            EdgeReversalPortType::Input | EdgeReversalPortType::All
        ) {
            let outgoing_edges = graph.layerless_nodes[node].ports[port]
                .outgoing_edges
                .clone();
            for edge in outgoing_edges {
                if can_reverse_outgoing_edge(graph, node_layer_constraint, edge) {
                    reverse_edge(graph, edge, true);
                }
            }
        }

        if matches!(
            target_port_type,
            EdgeReversalPortType::Output | EdgeReversalPortType::All
        ) {
            let incoming_edges = graph.layerless_nodes[node].ports[port]
                .incoming_edges
                .clone();
            for edge in incoming_edges {
                if can_reverse_incoming_edge(graph, node_layer_constraint, edge) {
                    reverse_edge(graph, edge, true);
                }
            }
        }
    }
}

fn can_reverse_outgoing_edge(
    graph: &LGraph,
    source_node_layer_constraint: LayerConstraint,
    edge: usize,
) -> bool {
    let Some(edge) = graph.edges.get(edge) else {
        return false;
    };
    if edge.reversed {
        return false;
    }

    let target_node = edge.target.node;
    if source_node_layer_constraint == LayerConstraint::Last
        && graph.layerless_nodes[target_node].kind == LNodeKind::Label
    {
        return false;
    }

    graph.layerless_nodes[target_node].layer_constraint != LayerConstraint::LastSeparate
}

fn can_reverse_incoming_edge(
    graph: &LGraph,
    target_node_layer_constraint: LayerConstraint,
    edge: usize,
) -> bool {
    let Some(edge) = graph.edges.get(edge) else {
        return false;
    };
    if edge.reversed {
        return false;
    }

    let source_node = edge.source.node;
    if target_node_layer_constraint == LayerConstraint::First
        && graph.layerless_nodes[source_node].kind == LNodeKind::Label
    {
        return false;
    }

    graph.layerless_nodes[source_node].layer_constraint != LayerConstraint::FirstSeparate
}

pub fn preprocess_layer_constraints(graph: &mut LGraph) -> IntermediateResult<()> {
    graph.hidden_nodes.clear();
    let mut hidden_connections = vec![HiddenNodeConnections::None; graph.layerless_nodes.len()];
    let nodes = (0..graph.layerless_nodes.len()).collect::<Vec<_>>();

    for node in nodes {
        if is_relevant_layer_constraint_node(graph, node) {
            hide_layer_constraint_node(graph, node, &mut hidden_connections)?;
            graph.layerless_nodes[node].hidden = true;
            graph.layerless_nodes[node].layer_index = None;
            graph.hidden_nodes.push(node);
        }
    }

    Ok(())
}

fn is_relevant_layer_constraint_node(graph: &LGraph, node: usize) -> bool {
    matches!(
        graph.layerless_nodes[node].layer_constraint,
        LayerConstraint::FirstSeparate | LayerConstraint::LastSeparate
    )
}

fn hide_layer_constraint_node(
    graph: &mut LGraph,
    node: usize,
    hidden_connections: &mut [HiddenNodeConnections],
) -> IntermediateResult<()> {
    ensure_no_unacceptable_separate_edges(graph, node)?;
    let connected_edges = graph.node_connected_edges(node);

    for edge in connected_edges {
        hide_layer_constraint_edge(graph, node, edge, hidden_connections);
    }

    Ok(())
}

fn hide_layer_constraint_edge(
    graph: &mut LGraph,
    node: usize,
    edge: usize,
    hidden_connections: &mut [HiddenNodeConnections],
) {
    let is_outgoing = graph.edges[edge].source.node == node;
    let opposite_port = if is_outgoing {
        graph.edges[edge].target
    } else {
        graph.edges[edge].source
    };

    if is_outgoing {
        graph.detach_edge_target(edge);
    } else {
        graph.detach_edge_source(edge);
    }
    graph.edges[edge].original_opposite_port = Some(opposite_port);

    update_opposite_node_layer_constraints(graph, node, opposite_port.node, hidden_connections);
}

fn update_opposite_node_layer_constraints(
    graph: &mut LGraph,
    hidden_node: usize,
    opposite_node: usize,
    hidden_connections: &mut [HiddenNodeConnections],
) {
    if graph.layerless_nodes[opposite_node].layer_constraint_explicit {
        return;
    }

    hidden_connections[opposite_node] = hidden_connections[opposite_node]
        .combine(graph.layerless_nodes[hidden_node].layer_constraint);

    if !graph.node_connected_edges(opposite_node).is_empty() {
        return;
    }

    match hidden_connections[opposite_node] {
        HiddenNodeConnections::FirstSeparate => {
            graph.layerless_nodes[opposite_node].layer_constraint = LayerConstraint::First;
            graph.layerless_nodes[opposite_node].layer_constraint_explicit = true;
        }
        HiddenNodeConnections::LastSeparate => {
            graph.layerless_nodes[opposite_node].layer_constraint = LayerConstraint::Last;
            graph.layerless_nodes[opposite_node].layer_constraint_explicit = true;
        }
        HiddenNodeConnections::None | HiddenNodeConnections::Both => {}
    }
}

fn ensure_no_unacceptable_separate_edges(graph: &LGraph, node: usize) -> IntermediateResult<()> {
    match graph.layerless_nodes[node].layer_constraint {
        LayerConstraint::FirstSeparate => {
            for edge in graph.node_incoming_edges(node) {
                if !is_acceptable_separate_incident_edge(graph, edge) {
                    return Err(IntermediateError::FirstSeparateIncomingEdge {
                        node_id: graph.layerless_nodes[node].id.clone(),
                        edge_id: graph.edges[edge].id.clone(),
                    });
                }
            }
        }
        LayerConstraint::LastSeparate => {
            for edge in graph.node_outgoing_edges(node) {
                if !is_acceptable_separate_incident_edge(graph, edge) {
                    return Err(IntermediateError::LastSeparateOutgoingEdge {
                        node_id: graph.layerless_nodes[node].id.clone(),
                        edge_id: graph.edges[edge].id.clone(),
                    });
                }
            }
        }
        _ => {}
    }

    Ok(())
}

fn is_acceptable_separate_incident_edge(graph: &LGraph, edge: usize) -> bool {
    graph.layerless_nodes[graph.edges[edge].source.node].kind == LNodeKind::ExternalPort
        && graph.layerless_nodes[graph.edges[edge].target.node].kind == LNodeKind::ExternalPort
}

pub fn postprocess_layer_constraints(graph: &mut LGraph) -> IntermediateResult<()> {
    if !graph.layers.is_empty() {
        move_first_and_last_nodes(graph)?;
    }

    if !graph.hidden_nodes.is_empty() {
        restore_hidden_layer_constraint_nodes(graph);
    }

    Ok(())
}

pub fn process_inverted_ports(graph: &mut LGraph) {
    let mut previous_layer = None;
    let mut unassigned_nodes = Vec::new();

    for layer_index in 0..graph.layers.len() {
        if let Some(target_layer) = previous_layer {
            assign_nodes_to_layer(graph, &mut unassigned_nodes, target_layer);
        }
        previous_layer = Some(layer_index);

        let layer_nodes = graph.layers[layer_index].nodes.clone();
        for node in layer_nodes {
            if graph.layerless_nodes[node].kind != LNodeKind::Normal
                || !graph.layerless_nodes[node].port_constraints.is_side_fixed()
            {
                continue;
            }

            let east_input_ports = ports_with_incidence(graph, node, PortSide::East, true);
            for port in east_input_ports {
                let incoming_edges = graph.layerless_nodes[node].ports[port]
                    .incoming_edges
                    .clone();
                for edge in incoming_edges {
                    create_east_port_side_dummy(
                        graph,
                        PortRef { node, port },
                        edge,
                        &mut unassigned_nodes,
                    );
                }
            }

            let west_output_ports = ports_with_incidence(graph, node, PortSide::West, false);
            for port in west_output_ports {
                let outgoing_edges = graph.layerless_nodes[node].ports[port]
                    .outgoing_edges
                    .clone();
                for edge in outgoing_edges {
                    create_west_port_side_dummy(
                        graph,
                        PortRef { node, port },
                        edge,
                        &mut unassigned_nodes,
                    );
                }
            }
        }
    }

    if let Some(target_layer) = previous_layer {
        assign_nodes_to_layer(graph, &mut unassigned_nodes, target_layer);
    }
}

fn ports_with_incidence(graph: &LGraph, node: usize, side: PortSide, incoming: bool) -> Vec<usize> {
    graph.layerless_nodes[node]
        .ports
        .iter()
        .enumerate()
        .filter_map(|(port, data)| {
            (data.side == side
                && if incoming {
                    !data.incoming_edges.is_empty()
                } else {
                    !data.outgoing_edges.is_empty()
                })
            .then_some(port)
        })
        .collect()
}

fn create_east_port_side_dummy(
    graph: &mut LGraph,
    eastward_port: PortRef,
    edge_index: usize,
    unassigned_nodes: &mut Vec<usize>,
) {
    if graph.edges[edge_index].source.node == eastward_port.node {
        return;
    }

    let original_target = graph.edges[edge_index].target;
    let dummy = create_inverted_port_dummy(graph, edge_index);
    unassigned_nodes.push(dummy);
    let dummy_input = add_inverted_dummy_port(graph, dummy, PortType::Input, PortSide::West);
    let dummy_output = add_inverted_dummy_port(graph, dummy, PortType::Output, PortSide::East);

    if !graph.set_edge_target(edge_index, dummy_input) {
        return;
    }

    let dummy_edge = clone_inverted_dummy_edge(graph, edge_index, dummy_output, original_target);
    let Some(dummy_edge_index) = graph.add_edge(dummy_edge) else {
        return;
    };

    set_dummy_node_properties(graph, dummy, edge_index, dummy_edge_index);
    move_head_labels(graph, edge_index, dummy_edge_index);
}

fn create_west_port_side_dummy(
    graph: &mut LGraph,
    westward_port: PortRef,
    edge_index: usize,
    unassigned_nodes: &mut Vec<usize>,
) {
    if graph.edges[edge_index].target.node == westward_port.node {
        return;
    }

    let original_target = graph.edges[edge_index].target;
    let dummy = create_inverted_port_dummy(graph, edge_index);
    unassigned_nodes.push(dummy);
    let dummy_input = add_inverted_dummy_port(graph, dummy, PortType::Input, PortSide::West);
    let dummy_output = add_inverted_dummy_port(graph, dummy, PortType::Output, PortSide::East);

    if !graph.set_edge_target(edge_index, dummy_input) {
        return;
    }

    let dummy_edge = clone_inverted_dummy_edge(graph, edge_index, dummy_output, original_target);
    let Some(dummy_edge_index) = graph.add_edge(dummy_edge) else {
        return;
    };

    set_dummy_node_properties(graph, dummy, edge_index, dummy_edge_index);
    move_head_labels(graph, edge_index, dummy_edge_index);
}

fn create_inverted_port_dummy(graph: &mut LGraph, edge_index: usize) -> usize {
    let dummy_index = graph.layerless_nodes.len();
    let mut dummy = LNode::new(
        format!("invertedPort:{edge_index}:{dummy_index}"),
        0.0,
        0.0,
        None,
    );
    dummy.kind = LNodeKind::LongEdge;
    dummy.origin_edge = Some(edge_index);
    dummy.port_constraints = PortConstraints::FixedPos;
    graph.layerless_nodes.push(dummy);
    dummy_index
}

fn add_inverted_dummy_port(
    graph: &mut LGraph,
    dummy: usize,
    port_type: PortType,
    side: PortSide,
) -> PortRef {
    graph
        .add_port(dummy, port_type, side, LPoint::default())
        .expect("inverted-port dummy was just inserted")
}

fn clone_inverted_dummy_edge(
    graph: &LGraph,
    edge_index: usize,
    source: PortRef,
    target: PortRef,
) -> LayeredEdge {
    let old_edge = &graph.edges[edge_index];
    LayeredEdge {
        id: old_edge.id.clone(),
        source,
        target,
        source_node_id: old_edge.source_node_id.clone(),
        target_node_id: old_edge.target_node_id.clone(),
        labels: Vec::new(),
        minlen: old_edge.minlen,
        reversed: old_edge.reversed,
        bend_points: Vec::new(),
        model_order: old_edge.model_order,
        priority_direction: old_edge.priority_direction,
        priority_shortness: old_edge.priority_shortness,
        priority_straightness: old_edge.priority_straightness,
        thickness: old_edge.thickness,
        original_opposite_port: old_edge.original_opposite_port,
        compound_segment: old_edge.compound_segment,
    }
}

fn assign_nodes_to_layer(graph: &mut LGraph, nodes: &mut Vec<usize>, layer_index: usize) {
    for node in nodes.drain(..) {
        graph.set_node_layer(node, layer_index);
    }
}

fn move_first_and_last_nodes(graph: &mut LGraph) -> IntermediateResult<()> {
    let first_layer = 0usize;
    let last_layer = graph.layers.len() - 1;
    let mut first_label_nodes = Vec::new();
    let mut last_label_nodes = Vec::new();

    let layered_nodes = graph
        .layers
        .iter()
        .flat_map(|layer| layer.nodes.iter().copied())
        .collect::<Vec<_>>();

    for node in layered_nodes {
        match graph.layerless_nodes[node].layer_constraint {
            LayerConstraint::First => {
                ensure_no_incoming_except_label_dummies(graph, node)?;
                graph.set_node_layer(node, first_layer);
                first_label_nodes.extend(adjacent_label_dummies(graph, node, true));
            }
            LayerConstraint::Last => {
                ensure_no_outgoing_except_label_dummies(graph, node)?;
                graph.set_node_layer(node, last_layer);
                last_label_nodes.extend(adjacent_label_dummies(graph, node, false));
            }
            _ => {}
        }
    }

    graph.compact_empty_layers();

    if !first_label_nodes.is_empty() {
        graph.insert_layer(0);
        for node in first_label_nodes {
            graph.set_node_layer(node, 0);
        }
    }

    if !last_label_nodes.is_empty() {
        let layer = graph.layers.len();
        graph.layers.push(crate::graph::Layer::new());
        for node in last_label_nodes {
            graph.set_node_layer(node, layer);
        }
    }

    Ok(())
}

fn adjacent_label_dummies(graph: &LGraph, node: usize, incoming: bool) -> Vec<usize> {
    let edges = if incoming {
        graph.node_incoming_edges(node)
    } else {
        graph.node_outgoing_edges(node)
    };

    edges
        .into_iter()
        .filter_map(|edge| {
            let candidate = if incoming {
                graph.edges[edge].source.node
            } else {
                graph.edges[edge].target.node
            };
            (graph.layerless_nodes[candidate].kind == LNodeKind::Label).then_some(candidate)
        })
        .collect()
}

fn restore_hidden_layer_constraint_nodes(graph: &mut LGraph) {
    let hidden_nodes = graph.hidden_nodes.clone();
    let mut first_separate_nodes = Vec::new();
    let mut last_separate_nodes = Vec::new();

    for node in hidden_nodes {
        graph.layerless_nodes[node].hidden = false;
        match graph.layerless_nodes[node].layer_constraint {
            LayerConstraint::FirstSeparate => first_separate_nodes.push(node),
            LayerConstraint::LastSeparate => last_separate_nodes.push(node),
            _ => {}
        }

        restore_hidden_node_edges(graph, node);
    }

    if !first_separate_nodes.is_empty() {
        graph.insert_layer(0);
        for node in first_separate_nodes {
            graph.set_node_layer(node, 0);
        }
    }

    if !last_separate_nodes.is_empty() {
        let layer = graph.layers.len();
        graph.layers.push(crate::graph::Layer::new());
        for node in last_separate_nodes {
            graph.set_node_layer(node, layer);
        }
    }

    graph.hidden_nodes.clear();
}

fn restore_hidden_node_edges(graph: &mut LGraph, node: usize) {
    for edge in graph.node_connected_edges(node) {
        if graph.edge_source_attached(edge) && graph.edge_target_attached(edge) {
            continue;
        }

        let Some(original_opposite_port) = graph.edges[edge].original_opposite_port else {
            continue;
        };

        if !graph.edge_target_attached(edge) {
            graph.set_edge_target(edge, original_opposite_port);
        } else if !graph.edge_source_attached(edge) {
            graph.set_edge_source(edge, original_opposite_port);
        }
    }
}

fn ensure_no_incoming_except_label_dummies(graph: &LGraph, node: usize) -> IntermediateResult<()> {
    for edge in graph.node_incoming_edges(node) {
        if graph.layerless_nodes[graph.edges[edge].source.node].kind != LNodeKind::Label {
            return Err(IntermediateError::FirstIncomingEdge {
                node_id: graph.layerless_nodes[node].id.clone(),
                edge_id: graph.edges[edge].id.clone(),
            });
        }
    }
    Ok(())
}

fn ensure_no_outgoing_except_label_dummies(graph: &LGraph, node: usize) -> IntermediateResult<()> {
    for edge in graph.node_outgoing_edges(node) {
        if graph.layerless_nodes[graph.edges[edge].target.node].kind != LNodeKind::Label {
            return Err(IntermediateError::LastOutgoingEdge {
                node_id: graph.layerless_nodes[node].id.clone(),
                edge_id: graph.edges[edge].id.clone(),
            });
        }
    }
    Ok(())
}

pub fn split_long_edges(graph: &mut LGraph) {
    if graph.layers.len() <= 2 {
        return;
    }

    let mut layer_index = 0usize;
    while layer_index + 1 < graph.layers.len() {
        let next_layer_index = layer_index + 1;
        let mut node_position = 0usize;

        while node_position < graph.layers[layer_index].nodes.len() {
            let node_index = graph.layers[layer_index].nodes[node_position];
            let outgoing_edges = graph.node_outgoing_edges(node_index);

            for edge_index in outgoing_edges {
                let target_node = graph.edges[edge_index].target.node;
                let Some(target_layer_index) = graph.layerless_nodes[target_node].layer_index
                else {
                    continue;
                };

                if target_layer_index != layer_index && target_layer_index != next_layer_index {
                    let dummy = create_long_edge_dummy_node(graph, next_layer_index, edge_index);
                    split_edge(graph, edge_index, dummy);
                }
            }

            node_position += 1;
        }

        layer_index += 1;
    }
}

pub fn merge_hyperedge_dummies(graph: &mut LGraph) {
    let hyperedge_ids = identify_hyperedges(graph);

    for layer_index in 0..graph.layers.len() {
        let mut node_position = 1usize;
        while node_position < graph.layers[layer_index].nodes.len() {
            let current = graph.layers[layer_index].nodes[node_position];
            let previous = graph.layers[layer_index].nodes[node_position - 1];

            if graph.layerless_nodes[current].kind == LNodeKind::LongEdge
                && graph.layerless_nodes[previous].kind == LNodeKind::LongEdge
            {
                let state = check_hyperedge_merge_allowed(graph, current, previous, &hyperedge_ids);
                if state.allow_merge {
                    merge_hyperedge_dummy_nodes(
                        graph,
                        current,
                        previous,
                        state.same_source,
                        state.same_target,
                    );
                    graph.remove_node_from_layer(current);
                    continue;
                }
            }

            node_position += 1;
        }
    }
}

#[derive(Clone, Copy)]
struct HyperedgeMergeState {
    allow_merge: bool,
    same_source: bool,
    same_target: bool,
}

fn check_hyperedge_merge_allowed(
    graph: &LGraph,
    current: usize,
    previous: usize,
    hyperedge_ids: &HashMap<PortRef, i32>,
) -> HyperedgeMergeState {
    let current_node = &graph.layerless_nodes[current];
    let previous_node = &graph.layerless_nodes[previous];
    let same_source = current_node.long_edge_source.is_some()
        && current_node.long_edge_source == previous_node.long_edge_source;
    let same_target = current_node.long_edge_target.is_some()
        && current_node.long_edge_target == previous_node.long_edge_target;

    if !current_node.long_edge_has_label_dummies && !previous_node.long_edge_has_label_dummies {
        let current_first_port = current_node.ports.first().map(|_| PortRef {
            node: current,
            port: 0,
        });
        let previous_first_port = previous_node.ports.first().map(|_| PortRef {
            node: previous,
            port: 0,
        });
        let allow_merge = match (current_first_port, previous_first_port) {
            (Some(current_port), Some(previous_port)) => {
                let current_id = hyperedge_ids.get(&current_port);
                let previous_id = hyperedge_ids.get(&previous_port);
                current_id.is_some() && current_id == previous_id
            }
            _ => false,
        };
        return HyperedgeMergeState {
            allow_merge,
            same_source,
            same_target,
        };
    }

    let current_before_label = is_before_label_dummy(graph, current);
    let previous_before_label = is_before_label_dummy(graph, previous);
    let eligible_for_source_merging = (!current_node.long_edge_has_label_dummies
        || current_before_label)
        && (!previous_node.long_edge_has_label_dummies || previous_before_label);
    let eligible_for_target_merging = (!current_node.long_edge_has_label_dummies
        || !current_before_label)
        && (!previous_node.long_edge_has_label_dummies || !previous_before_label);

    HyperedgeMergeState {
        allow_merge: (same_source && eligible_for_source_merging)
            || (same_target && eligible_for_target_merging),
        same_source,
        same_target,
    }
}

fn merge_hyperedge_dummy_nodes(
    graph: &mut LGraph,
    merge_source: usize,
    merge_target: usize,
    keep_source_port: bool,
    keep_target_port: bool,
) {
    let Some(merge_target_input_port) = find_port_by_side(graph, merge_target, PortSide::West)
    else {
        return;
    };
    let Some(merge_target_output_port) = find_port_by_side(graph, merge_target, PortSide::East)
    else {
        return;
    };

    let port_count = graph.layerless_nodes[merge_source].ports.len();
    for port_index in 0..port_count {
        while let Some(edge_index) = graph.layerless_nodes[merge_source].ports[port_index]
            .incoming_edges
            .first()
            .copied()
        {
            if !graph.set_edge_target(edge_index, merge_target_input_port) {
                break;
            }
        }

        while let Some(edge_index) = graph.layerless_nodes[merge_source].ports[port_index]
            .outgoing_edges
            .first()
            .copied()
        {
            if !graph.set_edge_source(edge_index, merge_target_output_port) {
                break;
            }
        }
    }

    if !keep_source_port {
        graph.layerless_nodes[merge_target].long_edge_source = None;
    }
    if !keep_target_port {
        graph.layerless_nodes[merge_target].long_edge_target = None;
    }
}

fn identify_hyperedges(graph: &LGraph) -> HashMap<PortRef, i32> {
    let ports = graph
        .layers
        .iter()
        .flat_map(|layer| layer.nodes.iter().copied())
        .flat_map(|node| {
            (0..graph.layerless_nodes[node].ports.len()).map(move |port| PortRef { node, port })
        })
        .collect::<Vec<_>>();
    let mut component_ids = HashMap::new();
    let mut next_id = 0i32;

    for port in ports {
        if component_ids.contains_key(&port) {
            continue;
        }
        mark_hyperedge_component(graph, port, next_id, &mut component_ids);
        next_id += 1;
    }

    component_ids
}

fn mark_hyperedge_component(
    graph: &LGraph,
    start: PortRef,
    component_id: i32,
    component_ids: &mut HashMap<PortRef, i32>,
) {
    let mut stack = vec![start];
    while let Some(port_ref) = stack.pop() {
        if !port_exists_in_graph(graph, port_ref) || component_ids.contains_key(&port_ref) {
            continue;
        }
        component_ids.insert(port_ref, component_id);

        for connected in connected_ports(graph, port_ref) {
            if !component_ids.contains_key(&connected) {
                stack.push(connected);
            }
        }

        if graph.layerless_nodes[port_ref.node].kind == LNodeKind::LongEdge {
            for sibling_port in 0..graph.layerless_nodes[port_ref.node].ports.len() {
                let sibling = PortRef {
                    node: port_ref.node,
                    port: sibling_port,
                };
                if sibling != port_ref && !component_ids.contains_key(&sibling) {
                    stack.push(sibling);
                }
            }
        }
    }
}

fn connected_ports(graph: &LGraph, port_ref: PortRef) -> Vec<PortRef> {
    let Some(port) = graph
        .layerless_nodes
        .get(port_ref.node)
        .and_then(|node| node.ports.get(port_ref.port))
    else {
        return Vec::new();
    };

    port.outgoing_edges
        .iter()
        .filter_map(|edge| graph.edges.get(*edge).map(|edge| edge.target))
        .chain(
            port.incoming_edges
                .iter()
                .filter_map(|edge| graph.edges.get(*edge).map(|edge| edge.source)),
        )
        .collect()
}

fn is_before_label_dummy(graph: &LGraph, node: usize) -> bool {
    let mut current = node;
    let mut visited = vec![false; graph.layerless_nodes.len()];

    loop {
        if current >= visited.len() || visited[current] {
            return false;
        }
        visited[current] = true;

        let Some(outgoing) = graph.node_outgoing_edges(current).into_iter().next() else {
            return false;
        };
        let target = graph.edges[outgoing].target.node;
        match graph.layerless_nodes[target].kind {
            LNodeKind::Label => return true,
            LNodeKind::LongEdge => current = target,
            _ => return false,
        }
    }
}

fn find_port_by_side(graph: &LGraph, node: usize, side: PortSide) -> Option<PortRef> {
    graph
        .layerless_nodes
        .get(node)?
        .ports
        .iter()
        .position(|port| port.side == side)
        .map(|port| PortRef { node, port })
}

fn port_exists_in_graph(graph: &LGraph, port_ref: PortRef) -> bool {
    graph
        .layerless_nodes
        .get(port_ref.node)
        .and_then(|node| node.ports.get(port_ref.port))
        .is_some()
}

pub fn join_long_edges(graph: &mut LGraph) {
    join_long_edges_observed(graph, &mut NoopLongEdgeJoinObserver);
}

trait LongEdgeJoinObserver {
    fn visit_preflight_layer_node(&mut self) {}

    fn visit_layer_node(&mut self) {}

    fn pair_edges(&mut self) {}

    fn visit_indexed_port(&mut self) {}

    fn visit_indexed_incoming_edge(&mut self) {}

    fn visit_fallback_incoming_edge(&mut self) {}

    fn replace_target_slot(&mut self) {}

    fn remove_dummy_input_edges(&mut self, _count: usize) {}

    fn remove_dummy_output_edges(&mut self, _count: usize) {}
}

struct NoopLongEdgeJoinObserver;

impl LongEdgeJoinObserver for NoopLongEdgeJoinObserver {}

#[cfg(test)]
#[derive(Debug, Default, PartialEq, Eq)]
struct LongEdgeJoinWork {
    preflight_layer_nodes_visited: usize,
    layer_nodes_visited: usize,
    edge_pairs: usize,
    indexed_ports_visited: usize,
    indexed_incoming_edges_visited: usize,
    fallback_incoming_edges_visited: usize,
    target_slots_replaced: usize,
    dummy_input_edges_removed: usize,
    dummy_output_edges_removed: usize,
}

#[cfg(test)]
impl LongEdgeJoinObserver for LongEdgeJoinWork {
    fn visit_preflight_layer_node(&mut self) {
        self.preflight_layer_nodes_visited += 1;
    }

    fn visit_layer_node(&mut self) {
        self.layer_nodes_visited += 1;
    }

    fn pair_edges(&mut self) {
        self.edge_pairs += 1;
    }

    fn visit_indexed_port(&mut self) {
        self.indexed_ports_visited += 1;
    }

    fn visit_indexed_incoming_edge(&mut self) {
        self.indexed_incoming_edges_visited += 1;
    }

    fn visit_fallback_incoming_edge(&mut self) {
        self.fallback_incoming_edges_visited += 1;
    }

    fn replace_target_slot(&mut self) {
        self.target_slots_replaced += 1;
    }

    fn remove_dummy_input_edges(&mut self, count: usize) {
        self.dummy_input_edges_removed += count;
    }

    fn remove_dummy_output_edges(&mut self, count: usize) {
        self.dummy_output_edges_removed += count;
    }
}

fn index_all_incoming_edges(
    graph: &LGraph,
    observer: &mut impl LongEdgeJoinObserver,
) -> HashMap<usize, (PortRef, usize)> {
    let mut positions = HashMap::with_capacity(graph.edges.len());
    for (node_index, node) in graph.layerless_nodes.iter().enumerate() {
        for (port_index, port) in node.ports.iter().enumerate() {
            observer.visit_indexed_port();
            let target = PortRef {
                node: node_index,
                port: port_index,
            };
            for (position, edge) in port.incoming_edges.iter().copied().enumerate() {
                observer.visit_indexed_incoming_edge();
                positions.insert(edge, (target, position));
            }
        }
    }
    positions
}

fn join_long_edges_observed(graph: &mut LGraph, observer: &mut impl LongEdgeJoinObserver) {
    let add_unnecessary_bendpoints = graph.options.unnecessary_bendpoints;
    let mut has_long_edge_dummy = false;
    'preflight: for layer in &graph.layers {
        for &node_index in &layer.nodes {
            observer.visit_preflight_layer_node();
            if graph.layerless_nodes[node_index].kind == LNodeKind::LongEdge {
                has_long_edge_dummy = true;
                break 'preflight;
            }
        }
    }
    if !has_long_edge_dummy {
        return;
    }

    let mut incoming_edge_positions = index_all_incoming_edges(graph, observer);

    for layer_index in 0..graph.layers.len() {
        let mut layer_nodes = std::mem::take(&mut graph.layers[layer_index].nodes);
        let mut retained_count = 0usize;

        for node_position in 0..layer_nodes.len() {
            let node_index = layer_nodes[node_position];
            observer.visit_layer_node();
            if graph.layerless_nodes[node_index].kind == LNodeKind::LongEdge {
                join_long_edge_at_observed(
                    graph,
                    node_index,
                    add_unnecessary_bendpoints,
                    Some(&mut incoming_edge_positions),
                    observer,
                );
            } else {
                layer_nodes[retained_count] = node_index;
                retained_count += 1;
            }
        }

        layer_nodes.truncate(retained_count);
        graph.layers[layer_index].nodes = layer_nodes;
    }
}

pub fn join_long_edge_at(
    graph: &mut LGraph,
    long_edge_dummy: usize,
    add_unnecessary_bendpoints: bool,
) {
    join_long_edge_at_observed(
        graph,
        long_edge_dummy,
        add_unnecessary_bendpoints,
        None,
        &mut NoopLongEdgeJoinObserver,
    );
}

#[derive(Debug, Clone, Copy)]
struct LongEdgeJoinPair {
    surviving_edge: usize,
    dropped_edge: usize,
    dropped_target: PortRef,
}

fn index_join_pair_targets(
    graph: &LGraph,
    pairs: &[LongEdgeJoinPair],
    observer: &mut impl LongEdgeJoinObserver,
) -> HashMap<usize, (PortRef, usize)> {
    let dropped_edges = pairs
        .iter()
        .map(|pair| pair.dropped_edge)
        .collect::<HashSet<_>>();
    let mut seen_target_ports = HashSet::with_capacity(pairs.len());
    let mut positions = HashMap::with_capacity(pairs.len());

    for target in pairs.iter().map(|pair| pair.dropped_target) {
        if !seen_target_ports.insert(target) {
            continue;
        }
        observer.visit_indexed_port();
        for (position, edge) in graph.layerless_nodes[target.node].ports[target.port]
            .incoming_edges
            .iter()
            .copied()
            .enumerate()
        {
            observer.visit_indexed_incoming_edge();
            if dropped_edges.contains(&edge) {
                positions.insert(edge, (target, position));
            }
        }
    }
    positions
}

fn join_long_edge_at_observed(
    graph: &mut LGraph,
    long_edge_dummy: usize,
    add_unnecessary_bendpoints: bool,
    incoming_edge_positions: Option<&mut HashMap<usize, (PortRef, usize)>>,
    observer: &mut impl LongEdgeJoinObserver,
) {
    let Some(input_port) = first_port_on_side(graph, long_edge_dummy, PortSide::West) else {
        return;
    };
    let Some(output_port) = first_port_on_side(graph, long_edge_dummy, PortSide::East) else {
        return;
    };

    let input_port_ref = PortRef {
        node: long_edge_dummy,
        port: input_port,
    };
    let output_port_ref = PortRef {
        node: long_edge_dummy,
        port: output_port,
    };
    let pair_count = graph.layerless_nodes[long_edge_dummy].ports[input_port]
        .incoming_edges
        .len()
        .min(
            graph.layerless_nodes[long_edge_dummy].ports[output_port]
                .outgoing_edges
                .len(),
        );
    if pair_count == 0 {
        return;
    }

    // ELK zips the existing input/output lists by index and keeps the input edge as the survivor.
    // Reordering either list, or choosing the output edge as survivor, changes labels and geometry.
    let mut pairs = Vec::with_capacity(pair_count);
    for (&surviving_edge, &dropped_edge) in graph.layerless_nodes[long_edge_dummy].ports[input_port]
        .incoming_edges
        .iter()
        .take(pair_count)
        .zip(
            graph.layerless_nodes[long_edge_dummy].ports[output_port]
                .outgoing_edges
                .iter()
                .take(pair_count),
        )
    {
        observer.pair_edges();
        pairs.push(LongEdgeJoinPair {
            surviving_edge,
            dropped_edge,
            dropped_target: graph.edges[dropped_edge].target,
        });
    }

    let mut local_incoming_edge_positions = None;
    let incoming_edge_positions = match incoming_edge_positions {
        Some(incoming_edge_positions) => incoming_edge_positions,
        None => {
            local_incoming_edge_positions.insert(index_join_pair_targets(graph, &pairs, observer))
        }
    };

    // Replacing the dropped edge in place preserves its exact incoming-edge slot without the
    // repeated shifts caused by insert-then-remove on the same target port.
    for pair in &pairs {
        let target_incoming = &mut graph.layerless_nodes[pair.dropped_target.node].ports
            [pair.dropped_target.port]
            .incoming_edges;
        let dropped_target_position = incoming_edge_positions
            .get(&pair.dropped_edge)
            .and_then(|indexed| {
                let (target, position) = *indexed;
                (target == pair.dropped_target
                    && target_incoming.get(position) == Some(&pair.dropped_edge))
                .then_some(position)
            })
            .or_else(|| {
                target_incoming.iter().position(|edge| {
                    observer.visit_fallback_incoming_edge();
                    *edge == pair.dropped_edge
                })
            })
            .expect("long-edge output must remain attached to its recorded target port");
        // ELK's ListIterator replacement keeps the surviving input edge in the exact slot of the
        // paired output edge. The linear fallback repairs a stale internal index without silently
        // appending and leaving the dropped edge attached.
        target_incoming[dropped_target_position] = pair.surviving_edge;
        let surviving_position = dropped_target_position;
        incoming_edge_positions.remove(&pair.dropped_edge);
        incoming_edge_positions.insert(
            pair.surviving_edge,
            (pair.dropped_target, surviving_position),
        );
        observer.replace_target_slot();
    }

    drop(
        graph.layerless_nodes[input_port_ref.node].ports[input_port_ref.port]
            .incoming_edges
            .drain(..pair_count),
    );
    observer.remove_dummy_input_edges(pair_count);
    for (position, edge) in graph.layerless_nodes[input_port_ref.node].ports[input_port_ref.port]
        .incoming_edges
        .iter()
        .copied()
        .enumerate()
    {
        observer.visit_indexed_incoming_edge();
        incoming_edge_positions.insert(edge, (input_port_ref, position));
    }
    drop(
        graph.layerless_nodes[output_port_ref.node].ports[output_port_ref.port]
            .outgoing_edges
            .drain(..pair_count),
    );
    observer.remove_dummy_output_edges(pair_count);

    let unnecessary_bendpoint =
        add_unnecessary_bendpoints.then(|| long_edge_dummy_anchor(graph, long_edge_dummy));
    for pair in pairs {
        join_long_edge_pair(
            graph,
            pair.surviving_edge,
            pair.dropped_edge,
            pair.dropped_target,
            unnecessary_bendpoint,
        );
    }
}

fn first_port_on_side(graph: &LGraph, node: usize, side: PortSide) -> Option<usize> {
    graph.layerless_nodes[node]
        .ports
        .iter()
        .position(|port| port.side == side)
}

fn join_long_edge_pair(
    graph: &mut LGraph,
    surviving_edge: usize,
    dropped_edge: usize,
    dropped_target: PortRef,
    unnecessary_bendpoint: Option<LPoint>,
) {
    let dropped_bend_points = graph.edges[dropped_edge].bend_points.clone();
    let dropped_labels = std::mem::take(&mut graph.edges[dropped_edge].labels);

    graph.edges[surviving_edge].target = dropped_target;

    if let Some(unnecessary_bendpoint) = unnecessary_bendpoint {
        graph.edges[surviving_edge]
            .bend_points
            .push(unnecessary_bendpoint);
    }
    graph.edges[surviving_edge]
        .bend_points
        .extend(dropped_bend_points);
    graph.edges[surviving_edge].labels.extend(dropped_labels);
}

fn long_edge_dummy_anchor(graph: &LGraph, node: usize) -> LPoint {
    let port = &graph.layerless_nodes[node].ports[0];
    LPoint {
        x: graph.layerless_nodes[node].position.x + port.position.x + port.anchor.x,
        y: graph.layerless_nodes[node].position.y + port.position.y + port.anchor.y,
    }
}

pub fn calculate_layer_sizes_and_graph_height(graph: &mut LGraph) {
    let mut min_y = f64::INFINITY;
    let mut max_y = f64::NEG_INFINITY;
    let mut found_nodes = false;
    let ports_surrounding = graph.options.spacing.ports_surrounding;

    for layer in &mut graph.layers {
        layer.size.width = 0.0;
        layer.size.height = 0.0;

        if layer.nodes.is_empty() {
            continue;
        }

        found_nodes = true;

        for node_index in &layer.nodes {
            let node = &graph.layerless_nodes[*node_index];
            layer.size.width = layer
                .size
                .width
                .max(node.size.width + node.margin.left + node.margin.right);
        }

        let first_node = &graph.layerless_nodes[layer.nodes[0]];
        let mut top = first_node.position.y - first_node.margin.top;
        if first_node.kind == LNodeKind::ExternalPort {
            top -= ports_surrounding.top;
        }

        let last_node = &graph.layerless_nodes[layer.nodes[layer.nodes.len() - 1]];
        let mut bottom = last_node.position.y + last_node.size.height + last_node.margin.bottom;
        if last_node.kind == LNodeKind::ExternalPort {
            bottom += ports_surrounding.bottom;
        }

        layer.size.height = bottom - top;
        min_y = min_y.min(top);
        max_y = max_y.max(bottom);
    }

    if !found_nodes {
        min_y = 0.0;
        max_y = 0.0;
    }

    graph.size.height = max_y - min_y;
    graph.offset.y -= min_y;
}

fn create_long_edge_dummy_node(
    graph: &mut LGraph,
    target_layer_index: usize,
    edge_to_split: usize,
) -> usize {
    let dummy_index = graph.layerless_nodes.len();
    let mut dummy = LNode::new(
        format!("longEdge:{edge_to_split}:{target_layer_index}"),
        0.0,
        0.0,
        None,
    );
    dummy.kind = LNodeKind::LongEdge;
    dummy.origin_edge = Some(edge_to_split);
    dummy.port_constraints = PortConstraints::FixedPos;
    graph.layerless_nodes.push(dummy);
    graph.set_node_layer(dummy_index, target_layer_index);
    dummy_index
}

pub fn split_edge(graph: &mut LGraph, edge_index: usize, dummy_node: usize) -> Option<usize> {
    let old_edge_target = graph.edges.get(edge_index)?.target;
    let thickness = graph.edges[edge_index].thickness.max(0.0);
    graph.edges[edge_index].thickness = thickness;
    graph.layerless_nodes[dummy_node].size.height = thickness;
    let port_position = LPoint {
        x: 0.0,
        y: (thickness / 2.0).floor(),
    };

    let dummy_input = graph.add_port(dummy_node, PortType::Input, PortSide::West, port_position)?;
    let dummy_output =
        graph.add_port(dummy_node, PortType::Output, PortSide::East, port_position)?;

    if !graph.set_edge_target(edge_index, dummy_input) {
        return None;
    }

    let old_segment = graph.edges[edge_index].clone();
    let dummy_edge = LayeredEdge {
        id: old_segment.id.clone(),
        source: dummy_output,
        target: old_edge_target,
        source_node_id: old_segment.source_node_id.clone(),
        target_node_id: old_segment.target_node_id.clone(),
        labels: Vec::new(),
        minlen: old_segment.minlen,
        reversed: old_segment.reversed,
        bend_points: Vec::new(),
        model_order: old_segment.model_order,
        priority_direction: old_segment.priority_direction,
        priority_shortness: old_segment.priority_shortness,
        priority_straightness: old_segment.priority_straightness,
        thickness,
        original_opposite_port: old_segment.original_opposite_port,
        compound_segment: old_segment.compound_segment,
    };
    let dummy_edge_index = graph.add_edge(dummy_edge)?;

    set_dummy_node_properties(graph, dummy_node, edge_index, dummy_edge_index);
    move_head_labels(graph, edge_index, dummy_edge_index);

    Some(dummy_edge_index)
}

fn set_dummy_node_properties(
    graph: &mut LGraph,
    dummy_node: usize,
    in_edge: usize,
    out_edge: usize,
) {
    let in_edge_source_node = graph.edges[in_edge].source.node;
    let out_edge_target_node = graph.edges[out_edge].target.node;

    if graph.layerless_nodes[in_edge_source_node].kind == LNodeKind::LongEdge {
        graph.layerless_nodes[dummy_node].long_edge_source =
            graph.layerless_nodes[in_edge_source_node].long_edge_source;
        graph.layerless_nodes[dummy_node].long_edge_target =
            graph.layerless_nodes[in_edge_source_node].long_edge_target;
        graph.layerless_nodes[dummy_node].long_edge_has_label_dummies =
            graph.layerless_nodes[in_edge_source_node].long_edge_has_label_dummies;
    } else if graph.layerless_nodes[in_edge_source_node].kind == LNodeKind::Label {
        graph.layerless_nodes[dummy_node].long_edge_source =
            graph.layerless_nodes[in_edge_source_node].long_edge_source;
        graph.layerless_nodes[dummy_node].long_edge_target =
            graph.layerless_nodes[in_edge_source_node].long_edge_target;
        graph.layerless_nodes[dummy_node].long_edge_has_label_dummies = true;
    } else if graph.layerless_nodes[out_edge_target_node].kind == LNodeKind::Label {
        graph.layerless_nodes[dummy_node].long_edge_source =
            graph.layerless_nodes[out_edge_target_node].long_edge_source;
        graph.layerless_nodes[dummy_node].long_edge_target =
            graph.layerless_nodes[out_edge_target_node].long_edge_target;
        graph.layerless_nodes[dummy_node].long_edge_has_label_dummies = true;
    } else {
        graph.layerless_nodes[dummy_node].long_edge_source = Some(graph.edges[in_edge].source);
        graph.layerless_nodes[dummy_node].long_edge_target = Some(graph.edges[out_edge].target);
    }
}

fn move_head_labels(graph: &mut LGraph, old_edge: usize, new_edge: usize) {
    let labels = std::mem::take(&mut graph.edges[old_edge].labels);
    let mut retained = Vec::with_capacity(labels.len());
    let mut moved = Vec::new();
    for mut label in labels {
        if label.placement == EdgeLabelPlacement::Head {
            if label.end_label_edge.is_none() {
                label.end_label_edge = Some(old_edge);
            }
            moved.push(label);
        } else {
            retained.push(label);
        }
    }
    graph.edges[old_edge].labels = retained;
    graph.edges[new_edge].labels.extend(moved);
}

#[cfg(test)]
mod tests;
