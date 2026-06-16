//! Layered graph coordinate transformations.
//!
//! Source references:
//! - https://github.com/eclipse-elk/elk/blob/62d5909f96fad541bc101ad52dabaece6b7eab7e/plugins/org.eclipse.elk.alg.layered/src/org/eclipse/elk/alg/layered/intermediate/GraphTransformer.java
//! - https://github.com/eclipse-elk/elk/blob/62d5909f96fad541bc101ad52dabaece6b7eab7e/plugins/org.eclipse.elk.alg.layered/src/org/eclipse/elk/alg/layered/options/DirectionCongruency.java

use crate::graph::{
    InLayerConstraint, LGraph, LLabel, LNode, LNodeKind, LPadding, LPoint, LPort, LSize, PortSide,
};
use crate::options::{DirectionCongruency, ElkDirection, LayerConstraint};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GraphTransformMode {
    ToInternalLeftToRight,
    ToInputDirection,
}

pub fn transform_graph_direction(graph: &mut LGraph, mode: GraphTransformMode) {
    match graph.options.direction {
        ElkDirection::Left | ElkDirection::Down | ElkDirection::Up => {}
        ElkDirection::Right | ElkDirection::Undefined => return,
    }

    match graph.options.direction_congruency {
        DirectionCongruency::ReadingDirection => match graph.options.direction {
            ElkDirection::Left => mirror_all_x(graph),
            ElkDirection::Down => transpose_all(graph),
            ElkDirection::Up => match mode {
                GraphTransformMode::ToInternalLeftToRight => {
                    transpose_all(graph);
                    mirror_all_y(graph);
                }
                GraphTransformMode::ToInputDirection => {
                    mirror_all_y(graph);
                    transpose_all(graph);
                }
            },
            ElkDirection::Right | ElkDirection::Undefined => {}
        },
        DirectionCongruency::Rotation => match mode {
            GraphTransformMode::ToInternalLeftToRight => match graph.options.direction {
                ElkDirection::Left => {
                    mirror_all_x(graph);
                    mirror_all_y(graph);
                }
                ElkDirection::Down => rotate90_clockwise(graph),
                ElkDirection::Up => rotate90_counter_clockwise(graph),
                ElkDirection::Right | ElkDirection::Undefined => {}
            },
            GraphTransformMode::ToInputDirection => match graph.options.direction {
                ElkDirection::Left => {
                    mirror_all_x(graph);
                    mirror_all_y(graph);
                }
                ElkDirection::Down => rotate90_counter_clockwise(graph),
                ElkDirection::Up => rotate90_clockwise(graph),
                ElkDirection::Right | ElkDirection::Undefined => {}
            },
        },
    }
}

fn rotate90_clockwise(graph: &mut LGraph) {
    transpose_all(graph);
    mirror_all_x(graph);
}

fn rotate90_counter_clockwise(graph: &mut LGraph) {
    mirror_all_x(graph);
    transpose_all(graph);
}

fn mirror_all_x(graph: &mut LGraph) {
    let offset = graph_x_offset(graph);
    graph.padding.swap_left_right();

    for node in &mut graph.layerless_nodes {
        mirror_node_x(node, offset);
    }
    for edge in &mut graph.edges {
        for point in &mut edge.bend_points {
            point.mirror_x(offset);
        }
        for label in &mut edge.labels {
            mirror_label_x(label, offset);
        }
    }
}

fn mirror_node_x(node: &mut LNode, offset: f64) {
    node.position.mirror_x(offset - node.size.width);
    node.padding.swap_left_right();
    for port in &mut node.ports {
        mirror_port_x(port, node.size.width);
    }
    if node.kind == LNodeKind::ExternalPort {
        node.layer_constraint = mirror_layer_constraint_x(node.layer_constraint);
        node.external_port_side = mirror_port_side_x(node.external_port_side);
    }
    for label in &mut node.labels {
        mirror_label_x_within_node(label, node.size.width);
    }
}

fn mirror_port_x(port: &mut LPort, node_width: f64) {
    port.position.mirror_x(node_width - port.size.width);
    port.anchor.mirror_x(port.size.width);
    port.side = mirror_port_side_x(port.side);
    reverse_port_index(port);
    for label in &mut port.labels {
        mirror_label_x_within_node(label, port.size.width);
    }
}

fn mirror_all_y(graph: &mut LGraph) {
    let offset = graph_y_offset(graph);
    graph.padding.swap_top_bottom();

    for node in &mut graph.layerless_nodes {
        mirror_node_y(node, offset);
    }
    for edge in &mut graph.edges {
        for point in &mut edge.bend_points {
            point.mirror_y(offset);
        }
        for label in &mut edge.labels {
            mirror_label_y(label, offset);
        }
    }
}

fn mirror_node_y(node: &mut LNode, offset: f64) {
    node.position.mirror_y(offset - node.size.height);
    node.padding.swap_top_bottom();
    node.in_layer_constraint = match node.in_layer_constraint {
        InLayerConstraint::Top => InLayerConstraint::Bottom,
        InLayerConstraint::Bottom => InLayerConstraint::Top,
        InLayerConstraint::None => InLayerConstraint::None,
    };
    if node.kind == LNodeKind::ExternalPort {
        node.external_port_side = mirror_port_side_y(node.external_port_side);
    }
    for port in &mut node.ports {
        mirror_port_y(port, node.size.height);
    }
    for label in &mut node.labels {
        mirror_label_y_within_node(label, node.size.height);
    }
}

fn mirror_port_y(port: &mut LPort, node_height: f64) {
    port.position.mirror_y(node_height - port.size.height);
    port.anchor.mirror_y(port.size.height);
    port.side = mirror_port_side_y(port.side);
    reverse_port_index(port);
    for label in &mut port.labels {
        mirror_label_y_within_node(label, port.size.height);
    }
}

fn transpose_all(graph: &mut LGraph) {
    graph.offset.transpose();
    graph.size.transpose();
    graph.padding.transpose();
    graph.options.edge_label_side_selection = graph.options.edge_label_side_selection.transpose();

    for node in &mut graph.layerless_nodes {
        transpose_node(node);
    }
    for edge in &mut graph.edges {
        for point in &mut edge.bend_points {
            point.transpose();
        }
        for label in &mut edge.labels {
            transpose_label(label);
        }
    }
}

fn transpose_node(node: &mut LNode) {
    node.position.transpose();
    node.size.transpose();
    node.padding.transpose();
    for port in &mut node.ports {
        transpose_port(port);
    }
    if node.kind == LNodeKind::ExternalPort {
        transpose_layer_constraint_for_external_port(node);
        node.external_port_side = transpose_port_side(node.external_port_side);
    }
    for label in &mut node.labels {
        transpose_label(label);
    }
}

fn transpose_port(port: &mut LPort) {
    port.position.transpose();
    port.anchor.transpose();
    port.size.transpose();
    port.side = transpose_port_side(port.side);
    reverse_port_index(port);
    for label in &mut port.labels {
        transpose_label(label);
    }
}

fn graph_x_offset(graph: &LGraph) -> f64 {
    let offset = if graph.size.width == 0.0 {
        graph
            .layerless_nodes
            .iter()
            .map(|node| node.position.x + node.size.width + node.margin.right)
            .fold(0.0, f64::max)
    } else {
        graph.size.width - graph.offset.x
    };
    offset - graph.offset.x
}

fn graph_y_offset(graph: &LGraph) -> f64 {
    let offset = if graph.size.height == 0.0 {
        graph
            .layerless_nodes
            .iter()
            .map(|node| node.position.y + node.size.height + node.margin.bottom)
            .fold(0.0, f64::max)
    } else {
        graph.size.height - graph.offset.y
    };
    offset - graph.offset.y
}

fn mirror_label_x(label: &mut LLabel, offset: f64) {
    label.position.mirror_x(offset - label.size.width);
}

fn mirror_label_y(label: &mut LLabel, offset: f64) {
    label.position.mirror_y(offset - label.size.height);
}

fn mirror_label_x_within_node(label: &mut LLabel, node_width: f64) {
    label.position.mirror_x(node_width - label.size.width);
}

fn mirror_label_y_within_node(label: &mut LLabel, node_height: f64) {
    label.position.mirror_y(node_height - label.size.height);
}

fn transpose_label(label: &mut LLabel) {
    label.position.transpose();
    label.size.transpose();
}

fn mirror_port_side_x(side: PortSide) -> PortSide {
    match side {
        PortSide::East => PortSide::West,
        PortSide::West => PortSide::East,
        side => side,
    }
}

fn mirror_port_side_y(side: PortSide) -> PortSide {
    match side {
        PortSide::North => PortSide::South,
        PortSide::South => PortSide::North,
        side => side,
    }
}

fn transpose_port_side(side: PortSide) -> PortSide {
    match side {
        PortSide::North => PortSide::West,
        PortSide::West => PortSide::North,
        PortSide::South => PortSide::East,
        PortSide::East => PortSide::South,
        PortSide::Undefined => PortSide::Undefined,
    }
}

fn mirror_layer_constraint_x(layer_constraint: LayerConstraint) -> LayerConstraint {
    match layer_constraint {
        LayerConstraint::First => LayerConstraint::Last,
        LayerConstraint::FirstSeparate => LayerConstraint::LastSeparate,
        LayerConstraint::Last => LayerConstraint::First,
        LayerConstraint::LastSeparate => LayerConstraint::FirstSeparate,
        LayerConstraint::None => LayerConstraint::None,
    }
}

fn transpose_layer_constraint_for_external_port(node: &mut LNode) {
    if node.layer_constraint == LayerConstraint::FirstSeparate {
        node.layer_constraint = LayerConstraint::None;
        node.in_layer_constraint = InLayerConstraint::Top;
    } else if node.layer_constraint == LayerConstraint::LastSeparate {
        node.layer_constraint = LayerConstraint::None;
        node.in_layer_constraint = InLayerConstraint::Bottom;
    } else if node.in_layer_constraint == InLayerConstraint::Top {
        node.layer_constraint = LayerConstraint::FirstSeparate;
        node.in_layer_constraint = InLayerConstraint::None;
    } else if node.in_layer_constraint == InLayerConstraint::Bottom {
        node.layer_constraint = LayerConstraint::LastSeparate;
        node.in_layer_constraint = InLayerConstraint::None;
    }
}

fn reverse_port_index(port: &mut LPort) {
    if let Some(index) = port.port_index.as_mut() {
        *index = -*index;
    }
}

trait PointTransform {
    fn mirror_x(&mut self, offset: f64);
    fn mirror_y(&mut self, offset: f64);
    fn transpose(&mut self);
}

impl PointTransform for LPoint {
    fn mirror_x(&mut self, offset: f64) {
        self.x = offset - self.x;
    }

    fn mirror_y(&mut self, offset: f64) {
        self.y = offset - self.y;
    }

    fn transpose(&mut self) {
        std::mem::swap(&mut self.x, &mut self.y);
    }
}

trait SizeTransform {
    fn transpose(&mut self);
}

impl SizeTransform for LSize {
    fn transpose(&mut self) {
        std::mem::swap(&mut self.width, &mut self.height);
    }
}

trait PaddingTransform {
    fn swap_left_right(&mut self);
    fn swap_top_bottom(&mut self);
    fn transpose(&mut self);
}

impl PaddingTransform for LPadding {
    fn swap_left_right(&mut self) {
        std::mem::swap(&mut self.left, &mut self.right);
    }

    fn swap_top_bottom(&mut self) {
        std::mem::swap(&mut self.top, &mut self.bottom);
    }

    fn transpose(&mut self) {
        let old_top = self.top;
        let old_bottom = self.bottom;
        let old_left = self.left;
        let old_right = self.right;

        self.top = old_left;
        self.bottom = old_right;
        self.left = old_top;
        self.right = old_bottom;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::graph::{EdgeLabelPlacement, LayeredEdge, PortType};
    use crate::options::{LayeredOptions, PortConstraints};

    fn graph(direction: ElkDirection) -> LGraph {
        LGraph::new(
            "root",
            LayeredOptions {
                direction,
                ..LayeredOptions::default()
            },
        )
    }

    #[test]
    fn reading_direction_down_transposes_graph() {
        let mut graph = graph(ElkDirection::Down);
        let mut node = LNode::new("A", 20.0, 10.0, None);
        node.position = LPoint { x: 3.0, y: 7.0 };
        let port = graph.add_node_for_test(node);
        graph.layerless_nodes[port].ports[0].position = LPoint { x: 1.0, y: 2.0 };
        graph.layerless_nodes[port].ports[0].anchor = LPoint { x: 3.0, y: 4.0 };
        graph.layerless_nodes[port].ports[0].size = LSize {
            width: 5.0,
            height: 6.0,
        };
        graph.layerless_nodes[port].ports[0].side = PortSide::North;
        graph.size = LSize {
            width: 100.0,
            height: 80.0,
        };
        graph.padding = LPadding {
            top: 1.0,
            right: 2.0,
            bottom: 3.0,
            left: 4.0,
        };

        transform_graph_direction(&mut graph, GraphTransformMode::ToInputDirection);

        let node = &graph.layerless_nodes[port];
        assert_eq!(node.position, LPoint { x: 7.0, y: 3.0 });
        assert_eq!(
            node.size,
            LSize {
                width: 10.0,
                height: 20.0
            }
        );
        assert_eq!(node.ports[0].position, LPoint { x: 2.0, y: 1.0 });
        assert_eq!(node.ports[0].anchor, LPoint { x: 4.0, y: 3.0 });
        assert_eq!(node.ports[0].side, PortSide::West);
        assert_eq!(
            graph.size,
            LSize {
                width: 80.0,
                height: 100.0
            }
        );
        assert_eq!(
            graph.padding,
            LPadding {
                top: 4.0,
                right: 3.0,
                bottom: 2.0,
                left: 1.0
            }
        );
    }

    #[test]
    fn rotation_down_to_input_direction_rotates_counter_clockwise() {
        let mut graph = graph(ElkDirection::Down);
        graph.options.direction_congruency = DirectionCongruency::Rotation;
        let mut node = LNode::new("A", 20.0, 10.0, None);
        node.position = LPoint { x: 3.0, y: 7.0 };
        let node = graph.add_node_for_test(node);

        transform_graph_direction(&mut graph, GraphTransformMode::ToInputDirection);

        assert_eq!(
            graph.layerless_nodes[node].size,
            LSize {
                width: 10.0,
                height: 20.0
            }
        );
        assert_eq!(
            graph.layerless_nodes[node].position,
            LPoint { x: 7.0, y: 0.0 }
        );
    }

    #[test]
    fn left_transform_mirrors_external_port_layer_constraint() {
        let mut graph = graph(ElkDirection::Left);
        let mut node = LNode::new("external", 0.0, 0.0, None);
        node.kind = LNodeKind::ExternalPort;
        node.layer_constraint = LayerConstraint::FirstSeparate;
        node.external_port_side = PortSide::West;
        let node = graph.add_node_for_test(node);

        transform_graph_direction(&mut graph, GraphTransformMode::ToInputDirection);

        assert_eq!(
            graph.layerless_nodes[node].layer_constraint,
            LayerConstraint::LastSeparate
        );
        assert_eq!(
            graph.layerless_nodes[node].external_port_side,
            PortSide::East
        );
    }

    #[test]
    fn transpose_preserves_external_port_layer_constraint_through_in_layer_constraint() {
        let mut graph = graph(ElkDirection::Down);
        let mut node = LNode::new("external", 0.0, 0.0, None);
        node.kind = LNodeKind::ExternalPort;
        node.layer_constraint = LayerConstraint::FirstSeparate;
        node.external_port_side = PortSide::South;
        let node = graph.add_node_for_test(node);

        transform_graph_direction(&mut graph, GraphTransformMode::ToInputDirection);

        assert_eq!(
            graph.layerless_nodes[node].layer_constraint,
            LayerConstraint::None
        );
        assert_eq!(
            graph.layerless_nodes[node].in_layer_constraint,
            InLayerConstraint::Top
        );
        assert_eq!(
            graph.layerless_nodes[node].external_port_side,
            PortSide::East
        );

        transform_graph_direction(&mut graph, GraphTransformMode::ToInternalLeftToRight);

        assert_eq!(
            graph.layerless_nodes[node].layer_constraint,
            LayerConstraint::FirstSeparate
        );
        assert_eq!(
            graph.layerless_nodes[node].in_layer_constraint,
            InLayerConstraint::None
        );
        assert_eq!(
            graph.layerless_nodes[node].external_port_side,
            PortSide::South
        );
    }

    #[test]
    fn edge_bend_points_and_labels_are_transformed() {
        let mut graph = graph(ElkDirection::Down);
        let source = graph.add_node_for_test(LNode::new("A", 10.0, 10.0, None));
        let target = graph.add_node_for_test(LNode::new("B", 10.0, 10.0, None));
        let source_port = graph.add_port(
            source,
            PortType::Output,
            PortSide::East,
            LPoint { x: 10.0, y: 5.0 },
        );
        let target_port = graph.add_port(
            target,
            PortType::Input,
            PortSide::West,
            LPoint { x: 0.0, y: 5.0 },
        );
        let edge = LayeredEdge {
            id: "A-B".to_string(),
            source: source_port.unwrap(),
            target: target_port.unwrap(),
            source_node_id: "A".to_string(),
            target_node_id: "B".to_string(),
            labels: vec![LLabel {
                text: "label".to_string(),
                size: LSize {
                    width: 12.0,
                    height: 4.0,
                },
                position: LPoint { x: 30.0, y: 40.0 },
                placement: EdgeLabelPlacement::Center,
                inline: false,
                label_side: None,
                end_label_edge: None,
                original_label_edge: None,
            }],
            minlen: 1,
            reversed: false,
            bend_points: vec![LPoint { x: 1.0, y: 2.0 }],
            model_order: None,
            priority_direction: 0,
            priority_shortness: 0,
            priority_straightness: 0,
            thickness: 0.0,
            original_opposite_port: None,
            compound_segment: None,
        };
        graph.add_edge(edge);

        transform_graph_direction(&mut graph, GraphTransformMode::ToInputDirection);

        assert_eq!(graph.edges[0].bend_points[0], LPoint { x: 2.0, y: 1.0 });
        assert_eq!(
            graph.edges[0].labels[0].size,
            LSize {
                width: 4.0,
                height: 12.0
            }
        );
        assert_eq!(
            graph.edges[0].labels[0].position,
            LPoint { x: 40.0, y: 30.0 }
        );
    }

    trait TestGraphExt {
        fn add_node_for_test(&mut self, node: LNode) -> usize;
    }

    impl TestGraphExt for LGraph {
        fn add_node_for_test(&mut self, mut node: LNode) -> usize {
            let index = self.layerless_nodes.len();
            node.port_constraints = PortConstraints::FixedSide;
            node.ports.push(LPort::new(
                format!("{}:0", node.id),
                index,
                PortType::Output,
            ));
            self.layerless_nodes.push(node);
            index
        }
    }
}
