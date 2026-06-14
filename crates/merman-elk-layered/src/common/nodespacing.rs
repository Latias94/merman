//! Common node dimension calculation utilities.
//!
//! Source references:
//! - https://github.com/eclipse-elk/elk/blob/62d5909f96fad541bc101ad52dabaece6b7eab7e/plugins/org.eclipse.elk.alg.common/src/org/eclipse/elk/alg/common/nodespacing/NodeDimensionCalculation.java
//! - https://github.com/eclipse-elk/elk/blob/62d5909f96fad541bc101ad52dabaece6b7eab7e/plugins/org.eclipse.elk.alg.common/src/org/eclipse/elk/alg/common/nodespacing/NodeLabelAndSizeCalculator.java
//! - https://github.com/eclipse-elk/elk/blob/62d5909f96fad541bc101ad52dabaece6b7eab7e/plugins/org.eclipse.elk.alg.common/src/org/eclipse/elk/alg/common/nodespacing/NodeMarginCalculator.java

use crate::graph::{LGraph, LMargin, LPort, PortSide};
use crate::options::{PortAlignment, PortConstraints};

#[derive(Debug, Clone, Copy, Default, PartialEq)]
struct Rect {
    x: f64,
    y: f64,
    width: f64,
    height: f64,
}

impl Rect {
    fn union(&mut self, other: Self) {
        let min_x = self.x.min(other.x);
        let min_y = self.y.min(other.y);
        let max_x = (self.x + self.width).max(other.x + other.width);
        let max_y = (self.y + self.height).max(other.y + other.height);
        self.x = min_x;
        self.y = min_y;
        self.width = max_x - min_x;
        self.height = max_y - min_y;
    }
}

pub fn calculate_label_and_node_sizes(graph: &mut LGraph, nodes: impl IntoIterator<Item = usize>) {
    for node in nodes {
        place_ports(graph, node);
    }
}

pub fn calculate_node_margins(graph: &mut LGraph, nodes: impl IntoIterator<Item = usize>) {
    for node in nodes {
        process_node_margin(graph, node);
    }
}

fn place_ports(graph: &mut LGraph, node: usize) {
    match graph.layerless_nodes[node].port_constraints {
        PortConstraints::FixedPos => {
            place_fixed_pos_ports(graph, node, PortSide::North);
            place_fixed_pos_ports(graph, node, PortSide::South);
            place_fixed_pos_ports(graph, node, PortSide::East);
            place_fixed_pos_ports(graph, node, PortSide::West);
        }
        PortConstraints::FixedRatio => {
            place_fixed_ratio_ports(graph, node, PortSide::North);
            place_fixed_ratio_ports(graph, node, PortSide::South);
            place_fixed_ratio_ports(graph, node, PortSide::East);
            place_fixed_ratio_ports(graph, node, PortSide::West);
        }
        _ => {
            place_free_ports(graph, node, PortSide::North);
            place_free_ports(graph, node, PortSide::South);
            place_free_ports(graph, node, PortSide::East);
            place_free_ports(graph, node, PortSide::West);
        }
    }
}

fn place_fixed_pos_ports(graph: &mut LGraph, node: usize, side: PortSide) {
    let size = graph.layerless_nodes[node].size;
    let port_indices = ports_on_side(graph, node, side);
    for port in port_indices {
        match side {
            PortSide::North | PortSide::South => {
                graph.layerless_nodes[node].ports[port].position.y =
                    horizontal_port_y(&graph.layerless_nodes[node].ports[port]);
            }
            PortSide::East | PortSide::West => {
                graph.layerless_nodes[node].ports[port].position.x =
                    vertical_port_x(&graph.layerless_nodes[node].ports[port], size.width);
            }
            PortSide::Undefined => {}
        }
    }
}

fn place_fixed_ratio_ports(graph: &mut LGraph, node: usize, side: PortSide) {
    let size = graph.layerless_nodes[node].size;
    let port_indices = ports_on_side(graph, node, side);
    for port in port_indices {
        let ratio = graph.layerless_nodes[node].ports[port].ratio_or_position;
        match side {
            PortSide::North | PortSide::South => {
                graph.layerless_nodes[node].ports[port].position.x = size.width * ratio;
                graph.layerless_nodes[node].ports[port].position.y =
                    horizontal_port_y(&graph.layerless_nodes[node].ports[port]);
            }
            PortSide::East | PortSide::West => {
                graph.layerless_nodes[node].ports[port].position.x =
                    vertical_port_x(&graph.layerless_nodes[node].ports[port], size.width);
                graph.layerless_nodes[node].ports[port].position.y = size.height * ratio;
            }
            PortSide::Undefined => {}
        }
    }
}

fn place_free_ports(graph: &mut LGraph, node: usize, side: PortSide) {
    let port_indices = ports_on_side(graph, node, side);
    if port_indices.is_empty() {
        return;
    }

    let node_size = graph.layerless_nodes[node].size;
    let alignment = graph.layerless_nodes[node]
        .port_alignment
        .unwrap_or(graph.options.port_alignment_default);
    let spacing = graph.options.spacing.port_port;
    let surrounding = graph.options.spacing.ports_surrounding;

    match side {
        PortSide::North | PortSide::South => {
            let available = node_size.width - surrounding.left - surrounding.right;
            let port_span = port_indices
                .iter()
                .map(|port| graph.layerless_nodes[node].ports[*port].size.width)
                .sum::<f64>()
                + spacing * port_indices.len().saturating_sub(1) as f64;
            let (mut x, space_between_ports) = placement_start_and_spacing(
                available,
                port_span,
                spacing,
                port_indices.len(),
                alignment,
            );
            x += surrounding.left;

            for port in port_indices {
                graph.layerless_nodes[node].ports[port].position.x = x;
                graph.layerless_nodes[node].ports[port].position.y =
                    horizontal_port_y(&graph.layerless_nodes[node].ports[port]);
                x += graph.layerless_nodes[node].ports[port].size.width + space_between_ports;
            }
        }
        PortSide::East | PortSide::West => {
            let available = node_size.height - surrounding.top - surrounding.bottom;
            let port_span = port_indices
                .iter()
                .map(|port| graph.layerless_nodes[node].ports[*port].size.height)
                .sum::<f64>()
                + spacing * port_indices.len().saturating_sub(1) as f64;
            let (mut y, space_between_ports) = placement_start_and_spacing(
                available,
                port_span,
                spacing,
                port_indices.len(),
                alignment,
            );
            y += surrounding.top;

            for port in port_indices {
                graph.layerless_nodes[node].ports[port].position.x =
                    vertical_port_x(&graph.layerless_nodes[node].ports[port], node_size.width);
                graph.layerless_nodes[node].ports[port].position.y = y;
                y += graph.layerless_nodes[node].ports[port].size.height + space_between_ports;
            }
        }
        PortSide::Undefined => {}
    }
}

fn placement_start_and_spacing(
    available_space: f64,
    placement_span: f64,
    port_spacing: f64,
    port_count: usize,
    alignment: PortAlignment,
) -> (f64, f64) {
    let alignment = if matches!(
        alignment,
        PortAlignment::Distributed | PortAlignment::Justified
    ) && port_count == 1
    {
        PortAlignment::Center
    } else {
        alignment
    };

    match alignment {
        PortAlignment::Begin => (0.0, port_spacing),
        PortAlignment::Center => ((available_space - placement_span) / 2.0, port_spacing),
        PortAlignment::End => (available_space - placement_span, port_spacing),
        PortAlignment::Distributed => {
            let distributed_span = placement_span + 2.0 * port_spacing;
            let additional = (available_space - distributed_span) / (port_count + 1) as f64;
            let spacing = (port_spacing + additional.max(0.0)).max(0.0);
            (spacing, spacing)
        }
        PortAlignment::Justified => {
            let spacing = if port_count > 1 {
                port_spacing
                    + ((available_space - placement_span) / (port_count - 1) as f64).max(0.0)
            } else {
                port_spacing
            };
            (0.0, spacing)
        }
    }
}

fn horizontal_port_y(port: &LPort) -> f64 {
    let offset = port.border_offset.unwrap_or(0.0);
    match port.side {
        PortSide::North => -port.size.height - offset,
        PortSide::South => offset,
        _ => port.position.y,
    }
}

fn vertical_port_x(port: &LPort, node_width: f64) -> f64 {
    let offset = port.border_offset.unwrap_or(0.0);
    match port.side {
        PortSide::West => -port.size.width - offset,
        PortSide::East => node_width + offset,
        _ => port.position.x,
    }
}

fn ports_on_side(graph: &LGraph, node: usize, side: PortSide) -> Vec<usize> {
    graph.layerless_nodes[node]
        .ports
        .iter()
        .enumerate()
        .filter_map(|(port, port_data)| (port_data.side == side).then_some(port))
        .collect()
}

fn process_node_margin(graph: &mut LGraph, node: usize) {
    let position = graph.layerless_nodes[node].position;
    let size = graph.layerless_nodes[node].size;
    let mut bounds = Rect {
        x: position.x,
        y: position.y,
        width: size.width,
        height: size.height,
    };

    for label in &graph.layerless_nodes[node].labels {
        bounds.union(Rect {
            x: position.x + label.position.x,
            y: position.y + label.position.y,
            width: label.size.width,
            height: label.size.height,
        });
    }

    for port in &graph.layerless_nodes[node].ports {
        let port_x = position.x + port.position.x;
        let port_y = position.y + port.position.y;
        bounds.union(Rect {
            x: port_x,
            y: port_y,
            width: port.size.width,
            height: port.size.height,
        });

        for label in &port.labels {
            bounds.union(Rect {
                x: port_x + label.position.x,
                y: port_y + label.position.y,
                width: label.size.width,
                height: label.size.height,
            });
        }
    }

    graph.layerless_nodes[node].margin = LMargin {
        top: (position.y - bounds.y).max(0.0),
        bottom: (bounds.y + bounds.height - (position.y + size.height)).max(0.0),
        left: (position.x - bounds.x).max(0.0),
        right: (bounds.x + bounds.width - (position.x + size.width)).max(0.0),
    };
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::graph::{LNode, LSize, PortType};
    use crate::options::LayeredOptions;

    #[test]
    fn label_and_node_size_calculation_places_free_ports_on_node_border() {
        let mut graph = LGraph::new("root", LayeredOptions::default());
        graph
            .layerless_nodes
            .push(LNode::new("A", 80.0, 40.0, None));
        let node = 0;
        graph.set_node_layer(node, 0);
        let port = graph
            .add_port(
                node,
                PortType::Output,
                PortSide::East,
                crate::graph::LPoint::default(),
            )
            .unwrap();
        graph.layerless_nodes[node].ports[port.port].set_side(PortSide::East);

        calculate_label_and_node_sizes(&mut graph, [node]);

        let port = &graph.layerless_nodes[node].ports[port.port];
        assert_eq!(port.position.x, graph.layerless_nodes[node].size.width);
        assert!(port.position.y > 0.0);
        assert!(port.position.y < graph.layerless_nodes[node].size.height);
    }

    #[test]
    fn node_margin_calculation_includes_ports_and_port_labels() {
        let mut graph = LGraph::new("root", LayeredOptions::default());
        let node = graph.layerless_nodes.len();
        graph
            .layerless_nodes
            .push(LNode::new("A", 80.0, 40.0, None));
        graph.set_node_layer(node, 0);
        let port = graph
            .add_port(
                node,
                PortType::Output,
                PortSide::East,
                crate::graph::LPoint { x: 80.0, y: 10.0 },
            )
            .unwrap();
        graph.layerless_nodes[node].ports[port.port].size = LSize {
            width: 8.0,
            height: 8.0,
        };
        graph.layerless_nodes[node].ports[port.port]
            .labels
            .push(crate::graph::LLabel {
                text: "p".to_string(),
                size: LSize {
                    width: 12.0,
                    height: 6.0,
                },
                position: crate::graph::LPoint { x: 10.0, y: 0.0 },
                placement: crate::graph::EdgeLabelPlacement::Center,
                inline: false,
                end_label_edge: None,
            });

        calculate_node_margins(&mut graph, [node]);

        assert_eq!(graph.layerless_nodes[node].margin.left, 0.0);
        assert!(graph.layerless_nodes[node].margin.right >= 22.0);
    }
}
