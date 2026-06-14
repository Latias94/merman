//! Phase 1 cycle breaking processors.
//!
//! Source references:
//! - https://github.com/eclipse-elk/elk/blob/62d5909f96fad541bc101ad52dabaece6b7eab7e/plugins/org.eclipse.elk.alg.layered/src/org/eclipse/elk/alg/layered/p1cycles/GreedyCycleBreaker.java
//! - https://github.com/eclipse-elk/elk/blob/62d5909f96fad541bc101ad52dabaece6b7eab7e/plugins/org.eclipse.elk.alg.layered/src/org/eclipse/elk/alg/layered/graph/LEdge.java

use std::collections::VecDeque;

use crate::graph::{LGraph, PortRef, reverse_edge};

pub fn break_cycles_greedy(graph: &mut LGraph) {
    let node_count = graph.layerless_nodes.len();
    let mut indeg = vec![0i32; node_count];
    let mut outdeg = vec![0i32; node_count];
    let mut mark = vec![0i32; node_count];
    let mut sources = VecDeque::new();
    let mut sinks = VecDeque::new();

    for node_index in 0..node_count {
        for port in &graph.layerless_nodes[node_index].ports {
            for edge_index in &port.incoming_edges {
                let edge = &graph.edges[*edge_index];
                if edge.source.node == node_index {
                    continue;
                }
                indeg[node_index] += positive_priority_weight(edge.priority_direction);
            }

            for edge_index in &port.outgoing_edges {
                let edge = &graph.edges[*edge_index];
                if edge.target.node == node_index {
                    continue;
                }
                outdeg[node_index] += positive_priority_weight(edge.priority_direction);
            }
        }

        if outdeg[node_index] == 0 {
            sinks.push_back(node_index);
        } else if indeg[node_index] == 0 {
            sources.push_back(node_index);
        }
    }

    let mut unprocessed_node_count = node_count;
    let mut next_right = -1;
    let mut next_left = 1;

    while unprocessed_node_count > 0 {
        while let Some(sink) = sinks.pop_front() {
            mark[sink] = next_right;
            next_right -= 1;
            update_neighbors(
                graph,
                sink,
                &mut indeg,
                &mut outdeg,
                &mark,
                &mut sources,
                &mut sinks,
            );
            unprocessed_node_count -= 1;
        }

        while let Some(source) = sources.pop_front() {
            mark[source] = next_left;
            next_left += 1;
            update_neighbors(
                graph,
                source,
                &mut indeg,
                &mut outdeg,
                &mark,
                &mut sources,
                &mut sinks,
            );
            unprocessed_node_count -= 1;
        }

        if unprocessed_node_count > 0 {
            let mut max_outflow = i32::MIN;
            let mut max_nodes = Vec::new();

            for node_index in 0..node_count {
                if mark[node_index] == 0 {
                    let outflow = outdeg[node_index] - indeg[node_index];
                    if outflow >= max_outflow {
                        if outflow > max_outflow {
                            max_nodes.clear();
                            max_outflow = outflow;
                        }
                        max_nodes.push(node_index);
                    }
                }
            }

            if let Some(max_node) = choose_node_with_max_outflow(graph, &max_nodes) {
                mark[max_node] = next_left;
                next_left += 1;
                update_neighbors(
                    graph,
                    max_node,
                    &mut indeg,
                    &mut outdeg,
                    &mark,
                    &mut sources,
                    &mut sinks,
                );
                unprocessed_node_count -= 1;
            } else {
                break;
            }
        }
    }

    let shift_base = node_count as i32 + 1;
    for node_mark in &mut mark {
        if *node_mark < 0 {
            *node_mark += shift_base;
        }
    }

    for node_index in 0..node_count {
        let port_count = graph.layerless_nodes[node_index].ports.len();
        for port_index in 0..port_count {
            let outgoing_edges = graph.layerless_nodes[node_index].ports[port_index]
                .outgoing_edges
                .clone();
            for edge_index in outgoing_edges {
                let target_index = graph.edges[edge_index].target.node;
                if mark[node_index] > mark[target_index] && reverse_edge(graph, edge_index, true) {
                    graph.cyclic = true;
                }
            }
        }
    }
}

fn choose_node_with_max_outflow(graph: &mut LGraph, nodes: &[usize]) -> Option<usize> {
    let random_index = graph.random.next_int(nodes.len())?;
    nodes.get(random_index).copied()
}

fn update_neighbors(
    graph: &LGraph,
    node_index: usize,
    indeg: &mut [i32],
    outdeg: &mut [i32],
    mark: &[i32],
    sources: &mut VecDeque<usize>,
    sinks: &mut VecDeque<usize>,
) {
    for (port_index, port) in graph.layerless_nodes[node_index].ports.iter().enumerate() {
        let port_ref = PortRef {
            node: node_index,
            port: port_index,
        };
        let connected_edges = port
            .incoming_edges
            .iter()
            .chain(port.outgoing_edges.iter())
            .copied()
            .collect::<Vec<_>>();

        for edge_index in connected_edges {
            let edge = &graph.edges[edge_index];
            let connected_port = if edge.source == port_ref {
                edge.target
            } else {
                edge.source
            };
            let endpoint_index = connected_port.node;

            if node_index == endpoint_index {
                continue;
            }

            if mark[endpoint_index] == 0 {
                let weight = non_negative_priority_weight(edge.priority_direction);
                if edge.target == connected_port {
                    indeg[endpoint_index] -= weight;
                    if indeg[endpoint_index] <= 0 && outdeg[endpoint_index] > 0 {
                        sources.push_back(endpoint_index);
                    }
                } else {
                    outdeg[endpoint_index] -= weight;
                    if outdeg[endpoint_index] <= 0 && indeg[endpoint_index] > 0 {
                        sinks.push_back(endpoint_index);
                    }
                }
            }
        }
    }
}

fn positive_priority_weight(priority: i32) -> i32 {
    if priority > 0 { priority + 1 } else { 1 }
}

fn non_negative_priority_weight(priority: i32) -> i32 {
    priority.max(0) + 1
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::graph::{EdgeLabelPlacement, LPoint, Layer};
    use crate::importer::{ElkInputEdge, ElkInputGraph, ElkInputLabel, ElkInputNode, import_graph};
    use crate::intermediate::restore_reversed_edges;
    use crate::options::{ElkDirection, LayeredOptions};

    fn node(id: &str) -> ElkInputNode {
        ElkInputNode {
            id: id.to_string(),
            width: 80.0,
            height: 40.0,
            parent: None,
            direction: None,
            hierarchy_handling: None,
            layer_constraint: None,
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
            priority_direction: 0,
            priority_shortness: 0,
            priority_straightness: 0,
        }
    }

    fn graph(nodes: Vec<ElkInputNode>, edges: Vec<ElkInputEdge>) -> LGraph {
        import_graph(&ElkInputGraph {
            id: "root".to_string(),
            options: LayeredOptions::mermaid_flowchart_defaults(ElkDirection::Down),
            nodes,
            edges,
        })
        .unwrap()
    }

    #[test]
    fn greedy_cycle_breaker_keeps_dag_edges_forward() {
        let mut graph = graph(
            vec![node("A"), node("B"), node("C")],
            vec![edge("A-B", "A", "B"), edge("B-C", "B", "C")],
        );

        break_cycles_greedy(&mut graph);

        assert!(!graph.cyclic);
        assert!(graph.edges.iter().all(|edge| !edge.reversed));
        assert_eq!(graph.edges[0].source_node_id, "A");
        assert_eq!(graph.edges[0].target_node_id, "B");
    }

    #[test]
    fn greedy_cycle_breaker_reverses_feedback_edges() {
        let mut graph = graph(
            vec![node("A"), node("B"), node("C")],
            vec![
                edge("A-B", "A", "B"),
                edge("B-C", "B", "C"),
                edge("C-A", "C", "A"),
            ],
        );

        break_cycles_greedy(&mut graph);

        assert!(graph.cyclic);
        assert!(graph.edges.iter().any(|edge| edge.reversed));
        assert!(!has_directed_cycle(&graph));
    }

    #[test]
    fn greedy_cycle_breaker_ignores_self_loops() {
        let mut graph = graph(vec![node("A")], vec![edge("A-A", "A", "A")]);

        break_cycles_greedy(&mut graph);

        assert!(!graph.cyclic);
        assert!(!graph.edges[0].reversed);
    }

    #[test]
    fn reversed_edge_restorer_restores_original_direction() {
        let mut graph = graph(
            vec![node("A"), node("B"), node("C")],
            vec![
                edge("A-B", "A", "B"),
                edge("B-C", "B", "C"),
                edge("C-A", "C", "A"),
            ],
        );
        let original_endpoints = graph
            .edges
            .iter()
            .map(|edge| (edge.source, edge.target))
            .collect::<Vec<_>>();

        break_cycles_greedy(&mut graph);
        graph.layers.push(Layer {
            nodes: (0..graph.layerless_nodes.len()).collect(),
        });
        restore_reversed_edges(&mut graph);

        assert!(graph.edges.iter().all(|edge| !edge.reversed));
        assert_eq!(
            graph
                .edges
                .iter()
                .map(|edge| (edge.source, edge.target))
                .collect::<Vec<_>>(),
            original_endpoints
        );
    }

    #[test]
    fn reverse_edge_swaps_endpoint_adjacency_labels_and_bendpoints() {
        let mut ab = edge("A-B", "A", "B");
        ab.label = Some(ElkInputLabel {
            text: "end".to_string(),
            width: 20.0,
            height: 12.0,
            placement: EdgeLabelPlacement::Head,
            inline: false,
        });
        let mut graph = graph(vec![node("A"), node("B")], vec![ab]);
        graph.edges[0].bend_points = vec![LPoint { x: 1.0, y: 2.0 }, LPoint { x: 3.0, y: 4.0 }];
        let old_source = graph.edges[0].source;
        let old_target = graph.edges[0].target;

        assert!(crate::graph::reverse_edge(&mut graph, 0, true));

        assert_eq!(graph.edges[0].source, old_target);
        assert_eq!(graph.edges[0].target, old_source);
        assert!(
            graph.layerless_nodes[old_source.node].ports[old_source.port]
                .outgoing_edges
                .is_empty()
        );
        assert_eq!(
            graph.layerless_nodes[old_source.node].ports[old_source.port].incoming_edges,
            vec![0]
        );
        assert_eq!(
            graph.layerless_nodes[old_target.node].ports[old_target.port].outgoing_edges,
            vec![0]
        );
        assert_eq!(graph.edges[0].labels[0].placement, EdgeLabelPlacement::Tail);
        assert_eq!(
            graph.edges[0].bend_points,
            vec![LPoint { x: 3.0, y: 4.0 }, LPoint { x: 1.0, y: 2.0 }]
        );
        assert!(graph.edges[0].reversed);
    }

    fn has_directed_cycle(graph: &LGraph) -> bool {
        let mut state = vec![VisitState::Unseen; graph.layerless_nodes.len()];
        (0..graph.layerless_nodes.len()).any(|node| visit(graph, node, &mut state))
    }

    fn visit(graph: &LGraph, node: usize, state: &mut [VisitState]) -> bool {
        match state[node] {
            VisitState::Active => return true,
            VisitState::Done => return false,
            VisitState::Unseen => {}
        }

        state[node] = VisitState::Active;
        for port in &graph.layerless_nodes[node].ports {
            for edge_index in &port.outgoing_edges {
                let edge = &graph.edges[*edge_index];
                if edge.source.node == edge.target.node {
                    continue;
                }
                if visit(graph, edge.target.node, state) {
                    return true;
                }
            }
        }
        state[node] = VisitState::Done;
        false
    }

    #[derive(Clone, Copy)]
    enum VisitState {
        Unseen,
        Active,
        Done,
    }
}
