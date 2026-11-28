//! Work estimates for processors that run within a single graph.

use super::ProcessorKind;
use crate::graph::{
    LGraph, LNode, LNodeKind, LayeredEdge, PortRef, PortSide, PortType, collector_port_id_suffix,
};
use crate::intermediate::visit_edge_and_layer_constraint_reversal_candidates;
use crate::options::PortConstraints;
use crate::p1cycles::{
    depth_first_cycle_breaker_may_mutate, interactive_cycle_breaker_may_mutate,
    visit_model_order_feedback_edges,
};
use crate::work::{WorkError, checked_add, checked_mul, checked_n_log_n, checked_sum};

pub(super) fn local_graph_work_units(graph: &LGraph) -> Result<usize, WorkError> {
    let mut ports = 0usize;
    let mut labels = 0usize;
    for node in &graph.layerless_nodes {
        ports = checked_add(ports, node.ports.len())?;
        labels = checked_add(labels, node.labels.len())?;
    }
    for edge in &graph.edges {
        labels = checked_add(labels, edge.labels.len())?;
    }
    Ok(checked_sum([
        graph.layerless_nodes.len(),
        graph.edges.len(),
        graph.layers.len(),
        ports,
        labels,
    ])?
    .max(1))
}

pub(super) fn processor_work_units(
    graph: &LGraph,
    kind: ProcessorKind,
) -> Result<usize, WorkError> {
    match kind {
        ProcessorKind::GreedyCycleBreaker | ProcessorKind::GreedyModelOrderCycleBreaker => {
            return greedy_cycle_breaker_work_units(graph);
        }
        ProcessorKind::DepthFirstCycleBreaker
        | ProcessorKind::InteractiveCycleBreaker
        | ProcessorKind::ModelOrderCycleBreaker
        | ProcessorKind::EdgeAndLayerConstraintEdgeReverser
        | ProcessorKind::ReversedEdgeRestorer => {
            return edge_reversal_processor_work_units(graph, kind);
        }
        ProcessorKind::LongEdgeSplitter => return long_edge_splitter_work_units(graph),
        ProcessorKind::LongEdgeJoiner => return long_edge_joiner_work_units(graph),
        ProcessorKind::EndLabelSorter => return end_label_sorter_work_units(graph),
        ProcessorKind::PortListSorter => return port_list_sorter_work_units(graph),
        ProcessorKind::SortByInputModelProcessor => {
            return sort_by_input_model_work_units(graph);
        }
        _ => {}
    }

    let base = local_graph_work_units(graph)?;
    let multiplier = match kind {
        ProcessorKind::NetworkSimplexLayerer => {
            let component_scale = (graph.layerless_nodes.len() as f64).sqrt() as usize;
            checked_mul(
                checked_mul(graph.options.thoroughness, 4)?,
                component_scale.max(1),
            )?
        }
        ProcessorKind::NetworkSimplexPlacer => {
            // Flexible nodes have two corners and one auxiliary vertex per port.
            let ports = checked_sum(graph.layerless_nodes.iter().map(|node| node.ports.len()))?;
            let auxiliary_nodes = checked_sum([
                checked_mul(graph.layerless_nodes.len(), 2)?,
                ports,
                graph.edges.len(),
                1,
            ])?;
            checked_mul(graph.options.thoroughness, auxiliary_nodes.max(1))?
        }
        ProcessorKind::LayerSweepCrossingMinimizerOneSidedGreedySwitch
        | ProcessorKind::LayerSweepCrossingMinimizerTwoSidedGreedySwitch => {
            checked_mul(graph.options.thoroughness.max(1), graph.edges.len().max(1))?
        }
        _ => 1,
    };
    checked_mul(base, multiplier.max(1))
}

fn edge_reversal_processor_work_units(
    graph: &LGraph,
    kind: ProcessorKind,
) -> Result<usize, WorkError> {
    let base = local_graph_work_units(graph)?;
    edge_reversal_processor_work_units_with_base(graph, kind, base)
}

pub(super) fn edge_reversal_processor_work_units_with_base(
    graph: &LGraph,
    kind: ProcessorKind,
    base: usize,
) -> Result<usize, WorkError> {
    let nodes = graph.layerless_nodes.len();

    let (preparation, mutation) = match kind {
        // `DepthFirstCycleBreaker`: visited, active, sources, candidate edges, recursive stack,
        // incoming source discovery, and outgoing DFS materialization.
        ProcessorKind::DepthFirstCycleBreaker => {
            let (ports, incoming, outgoing) =
                graph_port_and_directional_adjacency_work_units(graph)?;
            let preparation = checked_sum([
                checked_mul(nodes, 5)?,
                checked_mul(ports, 2)?,
                incoming,
                checked_mul(outgoing, 3)?,
            ])?;
            let mutation = if depth_first_cycle_breaker_may_mutate(graph) {
                adapted_edge_reversal_work_units(graph, 1)?
            } else {
                0
            };
            (preparation, mutation)
        }
        // `InteractiveCycleBreaker` performs two ordered batches. The first batch mutates the
        // graph before the DFS batch, so their candidate vectors and traversals cannot be fused.
        ProcessorKind::InteractiveCycleBreaker => {
            let (ports, outgoing) = graph_port_and_outgoing_adjacency_work_units(graph)?;
            let preparation = checked_sum([
                checked_mul(nodes, 4)?,
                checked_mul(ports, 2)?,
                checked_mul(outgoing, 6)?,
            ])?;
            let mutation = if interactive_cycle_breaker_may_mutate(graph) {
                adapted_edge_reversal_work_units(graph, 2)?
            } else {
                0
            };
            (preparation, mutation)
        }
        // ELK 0.12.0 resets the FIRST/LAST_SEPARATE calculator per source and recomputes
        // target order in edge traversal order; a node-score cache changes the result.
        ProcessorKind::ModelOrderCycleBreaker => {
            let (ports, outgoing) = graph_port_and_outgoing_adjacency_work_units(graph)?;
            let preparation =
                checked_sum([checked_mul(nodes, 2)?, ports, checked_mul(outgoing, 4)?])?;
            let mutation = model_order_edge_reversal_work_units(graph)?;
            (preparation, mutation)
        }
        ProcessorKind::EdgeAndLayerConstraintEdgeReverser => {
            let (ports, incoming, outgoing) =
                graph_port_and_directional_adjacency_work_units(graph)?;
            let preparation = checked_sum([
                checked_mul(nodes, 3)?,
                checked_mul(ports, 2)?,
                checked_mul(checked_add(incoming, outgoing)?, 4)?,
            ])?;
            let mutation = edge_and_layer_constraint_reversal_work_units(graph)?;
            (preparation, mutation)
        }
        ProcessorKind::ReversedEdgeRestorer => (
            reversed_edge_restorer_preparation_work_units(graph)?,
            marked_edge_restoration_work_units(graph)?,
        ),
        _ => unreachable!("only edge-reversal processors reach reversal accounting"),
    };
    checked_sum([base, preparation, mutation])
}

fn graph_port_and_outgoing_adjacency_work_units(
    graph: &LGraph,
) -> Result<(usize, usize), WorkError> {
    let mut ports = 0usize;
    let mut outgoing = 0usize;
    for node in &graph.layerless_nodes {
        ports = checked_add(ports, node.ports.len())?;
        for port in &node.ports {
            outgoing = checked_add(outgoing, port.outgoing_edges.len())?;
        }
    }
    Ok((ports, outgoing))
}

pub(super) fn end_label_sorter_work_units(graph: &LGraph) -> Result<usize, WorkError> {
    let mut port_labels = 0usize;
    let mut local_sort_work = 0usize;
    for node in &graph.layerless_nodes {
        for port in &node.ports {
            port_labels = checked_add(port_labels, port.labels.len())?;
            local_sort_work = checked_add(local_sort_work, checked_n_log_n(port.labels.len())?)?;
        }
    }
    checked_sum([
        local_graph_work_units(graph)?,
        graph.edges.len(),
        port_labels,
        local_sort_work,
    ])
}

pub(super) fn long_edge_joiner_work_units(graph: &LGraph) -> Result<usize, WorkError> {
    let layer_memberships = graph
        .layers
        .iter()
        .try_fold(0usize, |total, layer| checked_add(total, layer.nodes.len()))?;
    let has_long_edge_dummy = graph.layers.iter().any(|layer| {
        layer
            .nodes
            .iter()
            .any(|&node| graph.layerless_nodes[node].kind == LNodeKind::LongEdge)
    });
    let base = local_graph_work_units(graph)?;
    if !has_long_edge_dummy {
        return checked_add(base, layer_memberships);
    }

    let mut indexed_ports = 0usize;
    let mut indexed_incoming_edges = 0usize;
    for node in &graph.layerless_nodes {
        indexed_ports = checked_add(indexed_ports, node.ports.len())?;
        for port in &node.ports {
            indexed_incoming_edges =
                checked_add(indexed_incoming_edges, port.incoming_edges.len())?;
        }
    }

    let mut join_work = checked_sum([
        checked_mul(layer_memberships, 2)?,
        indexed_ports,
        indexed_incoming_edges,
    ])?;

    for layer in &graph.layers {
        for &node_index in &layer.nodes {
            let node = &graph.layerless_nodes[node_index];
            if node.kind != LNodeKind::LongEdge {
                continue;
            }
            join_work = checked_add(join_work, checked_mul(node.ports.len(), 2)?)?;
            let Some(input_port) = node
                .ports
                .iter()
                .position(|port| port.side == PortSide::West)
            else {
                continue;
            };
            let Some(output_port) = node
                .ports
                .iter()
                .position(|port| port.side == PortSide::East)
            else {
                continue;
            };
            let input_edges = &node.ports[input_port].incoming_edges;
            let output_edges = &node.ports[output_port].outgoing_edges;
            let pair_count = input_edges.len().min(output_edges.len());
            join_work = checked_sum([
                join_work,
                input_edges.len(),
                output_edges.len(),
                checked_mul(pair_count, 8)?,
            ])?;

            for dropped_edge in output_edges.iter().copied().take(pair_count) {
                let edge = &graph.edges[dropped_edge];
                join_work = checked_sum([
                    join_work,
                    checked_mul(edge.bend_points.len(), 2)?,
                    edge.labels.len(),
                ])?;
            }
        }
    }

    // The implementation builds one incoming-edge position index for the whole graph. Shared
    // collector ports therefore contribute their adjacency once, not once per long-edge dummy.
    checked_add(base, join_work)
}

pub(super) fn port_list_sorter_work_units(graph: &LGraph) -> Result<usize, WorkError> {
    let mut node_visits = 0usize;
    let mut port_scans = 0usize;
    let mut adjacency_scans = 0usize;
    let mut sort_work = 0usize;
    let mut local_permutations = 0usize;

    for layer in &graph.layers {
        for &node_index in &layer.nodes {
            node_visits = checked_add(node_visits, 1)?;
            let node = &graph.layerless_nodes[node_index];
            let port_count = node.ports.len();
            let constraints = node.port_constraints;
            if constraints.is_order_fixed() {
                let reorder_work = checked_mul(port_count, 4)?;
                port_scans = checked_add(port_scans, port_count)?;
                sort_work = checked_add(sort_work, checked_n_log_n(port_count)?)?;
                local_permutations = checked_add(local_permutations, reorder_work)?;
            } else if constraints.is_side_fixed() {
                let reorder_work = checked_mul(port_count, 4)?;
                // One side-key pass plus the two official South/West range probes.
                port_scans = checked_add(port_scans, checked_mul(port_count, 5)?)?;
                sort_work = checked_add(sort_work, checked_n_log_n(port_count)?)?;
                local_permutations = checked_add(local_permutations, reorder_work)?;
                for _ in 0..2 {
                    if port_count > 3 {
                        port_scans = checked_add(port_scans, port_count)?;
                        local_permutations = checked_add(local_permutations, reorder_work)?;
                    }
                }
                if graph.options.port_sorting_strategy
                    == crate::options::PortSortingStrategy::PortDegree
                {
                    port_scans = checked_add(port_scans, port_count)?;
                    sort_work = checked_add(sort_work, checked_n_log_n(port_count)?)?;
                    local_permutations = checked_add(local_permutations, reorder_work)?;
                    for port in &node.ports {
                        adjacency_scans = checked_add(
                            adjacency_scans,
                            checked_add(port.incoming_edges.len(), port.outgoing_edges.len())?,
                        )?;
                    }
                }
            }
        }
    }

    checked_sum([
        local_graph_work_units(graph)?,
        node_visits,
        port_scans,
        adjacency_scans,
        sort_work,
        local_permutations,
    ])
}

pub(super) fn sort_by_input_model_work_units(graph: &LGraph) -> Result<usize, WorkError> {
    let mut layer_copy_work = 0usize;
    let mut preprocessing_work = 0usize;
    let mut local_permutations = 0usize;

    for (layer_index, layer) in graph.layers.iter().enumerate() {
        let previous_layer = if layer_index == 0 {
            layer
        } else {
            &graph.layers[layer_index - 1]
        };
        layer_copy_work = checked_sum([
            layer_copy_work,
            previous_layer.nodes.len(),
            layer.nodes.len(),
            layer.nodes.len(),
            layer.nodes.len(),
            if layer_index == 0 {
                layer.nodes.len()
            } else {
                0
            },
        ])?;

        for &node_index in &layer.nodes {
            let node = &graph.layerless_nodes[node_index];
            if matches!(
                node.port_constraints,
                PortConstraints::FixedOrder | PortConstraints::FixedPos
            ) {
                continue;
            }

            let port_count = node.ports.len();
            preprocessing_work = checked_add(preprocessing_work, checked_mul(port_count, 2)?)?;
            local_permutations = checked_add(local_permutations, checked_mul(port_count, 4)?)?;
        }
    }

    checked_sum([
        local_graph_work_units(graph)?,
        layer_copy_work,
        preprocessing_work,
        local_permutations,
    ])
}

#[cfg(test)]
pub(super) fn unprepared_reorder_node_ports_work_units(
    graph: &LGraph,
    node_index: usize,
) -> Result<usize, WorkError> {
    let port_count = graph.layerless_nodes[node_index].ports.len();
    let mut self_loop_refs = graph.self_loop_holders.len();
    for holder in &graph.self_loop_holders {
        if holder.node != node_index {
            continue;
        }
        for hyper_loop in &holder.hyper_loops {
            self_loop_refs = checked_sum([
                self_loop_refs,
                hyper_loop.ports.len(),
                checked_mul(hyper_loop.edges.len(), 2)?,
            ])?;
        }
    }

    let mut descendant_nodes = 0usize;
    let mut stack = graph
        .layerless_nodes
        .iter()
        .filter_map(|node| node.nested_graph.as_deref())
        .collect::<Vec<_>>();
    while let Some(current) = stack.pop() {
        descendant_nodes = checked_add(descendant_nodes, current.layerless_nodes.len())?;
        stack.extend(
            current
                .layerless_nodes
                .iter()
                .filter_map(|node| node.nested_graph.as_deref()),
        );
    }

    // Validation, permutation storage, old-to-new mapping, and moving each port are four local
    // passes. Edges expose three port references, local nodes expose three optional references,
    // while descendants are visited only for their `origin_port` reference.
    checked_sum([
        checked_mul(port_count, 4)?,
        checked_mul(graph.edges.len(), 3)?,
        self_loop_refs,
        checked_mul(graph.layerless_nodes.len(), 3)?,
        descendant_nodes,
        graph.id.len(),
    ])
}

pub(super) fn graph_port_and_adjacency_work_units(
    graph: &LGraph,
) -> Result<(usize, usize), WorkError> {
    let (ports, incoming, outgoing) = graph_port_and_directional_adjacency_work_units(graph)?;
    Ok((ports, checked_add(incoming, outgoing)?))
}

pub(super) fn graph_port_and_directional_adjacency_work_units(
    graph: &LGraph,
) -> Result<(usize, usize, usize), WorkError> {
    let mut ports = 0usize;
    let mut incoming = 0usize;
    let mut outgoing = 0usize;
    for node in &graph.layerless_nodes {
        ports = checked_add(ports, node.ports.len())?;
        for port in &node.ports {
            incoming = checked_add(incoming, port.incoming_edges.len())?;
            outgoing = checked_add(outgoing, port.outgoing_edges.len())?;
        }
    }
    Ok((ports, incoming, outgoing))
}

pub(super) struct AdaptedEdgeReversalStructureWork {
    pub(super) adjacency: usize,
    lookup_per_pass: usize,
    materialization: usize,
}

pub(super) fn adapted_edge_reversal_structure_work_units(
    graph: &LGraph,
) -> Result<AdaptedEdgeReversalStructureWork, WorkError> {
    // Accumulate owner-local scalars only. This estimator runs before the processor budget check;
    // allocating incidence caches here would let rejected input consume uncharged memory.
    let mut adjacency_base = 0usize;
    let mut lookup_per_pass = 0usize;
    let mut materialization = 0usize;
    for node in &graph.layerless_nodes {
        let mut collector_incidence = 0usize;
        let mut dedicated_adjacency = 0usize;
        let mut has_input_collector = false;
        let mut has_output_collector = false;
        let mut input_target_count = 0usize;
        let mut output_source_count = 0usize;
        for port in &node.ports {
            let incidence = checked_add(port.incoming_edges.len(), port.outgoing_edges.len())?;
            match port.collector_type {
                Some(PortType::Input) => {
                    has_input_collector = true;
                    collector_incidence = checked_add(collector_incidence, incidence)?;
                    input_target_count =
                        checked_add(input_target_count, port.incoming_edges.len())?;
                }
                Some(PortType::Output) => {
                    has_output_collector = true;
                    collector_incidence = checked_add(collector_incidence, incidence)?;
                    output_source_count =
                        checked_add(output_source_count, port.outgoing_edges.len())?;
                }
                None => {
                    dedicated_adjacency =
                        checked_add(dedicated_adjacency, checked_mul(incidence, incidence)?)?;
                }
            }
        }

        adjacency_base = checked_sum([
            adjacency_base,
            checked_mul(collector_incidence, collector_incidence)?,
            dedicated_adjacency,
        ])?;

        let materializes_input = output_source_count > 0 && !has_input_collector;
        let materializes_output = input_target_count > 0 && !has_output_collector;
        let materialized_ports = usize::from(materializes_input) + usize::from(materializes_output);
        let lookup_count = checked_add(input_target_count, output_source_count)?;
        let lookup_width = checked_sum([node.ports.len(), materialized_ports, 1])?;
        lookup_per_pass = checked_add(lookup_per_pass, checked_mul(lookup_count, lookup_width)?)?;

        if materializes_input {
            materialization = checked_add(
                materialization,
                collector_materialization_work_units(node, PortType::Input)?,
            )?;
        }
        if materializes_output {
            materialization = checked_add(
                materialization,
                collector_materialization_work_units(node, PortType::Output)?,
            )?;
        }
    }

    Ok(AdaptedEdgeReversalStructureWork {
        // The factor of two covers stable removal plus append/growth without depending on the
        // current Vec capacity.
        adjacency: checked_mul(adjacency_base, 2)?,
        lookup_per_pass,
        materialization,
    })
}

pub(super) fn adapted_edge_reversal_work_units(
    graph: &LGraph,
    reversal_passes: usize,
) -> Result<usize, WorkError> {
    if reversal_passes == 0 {
        return Ok(0);
    }

    let structure = adapted_edge_reversal_structure_work_units(graph)?;

    let mut payload_per_pass = 0usize;
    for edge in &graph.edges {
        payload_per_pass = checked_add(payload_per_pass, edge_reversal_payload_work_units(edge)?)?;
    }

    let repeated = checked_sum([
        structure.adjacency,
        structure.lookup_per_pass,
        payload_per_pass,
    ])?;
    checked_add(
        checked_mul(repeated, reversal_passes)?,
        structure.materialization,
    )
}

pub(super) fn model_order_edge_reversal_work_units(graph: &LGraph) -> Result<usize, WorkError> {
    let context = EdgeReversalCandidateWorkContext::new(graph)?;
    let mut work = Ok(0usize);
    let mut has_candidate = false;
    visit_model_order_feedback_edges(graph, |edge| {
        has_candidate = true;
        work = work.and_then(|current| {
            checked_add(
                current,
                adapted_edge_reversal_candidate_work_units(graph, &context, edge)?,
            )
        });
        work.is_err()
    });

    let work = work?;
    if has_candidate {
        checked_add(work, context.possible_collector_materialization)
    } else {
        Ok(0)
    }
}

pub(super) fn edge_and_layer_constraint_reversal_work_units(
    graph: &LGraph,
) -> Result<usize, WorkError> {
    let context = EdgeReversalCandidateWorkContext::new(graph)?;
    let mut work = Ok(0usize);
    let mut has_candidate = false;
    visit_edge_and_layer_constraint_reversal_candidates(graph, |edge| {
        has_candidate = true;
        work = work.and_then(|current| {
            checked_add(
                current,
                adapted_edge_reversal_candidate_work_units(graph, &context, edge)?,
            )
        });
        work.is_err()
    });

    let work = work?;
    if has_candidate {
        checked_add(work, context.possible_collector_materialization)
    } else {
        Ok(0)
    }
}

struct EdgeReversalCandidateWorkContext {
    collector_adjacency_by_node: Vec<usize>,
    possible_collector_materialization: usize,
}

impl EdgeReversalCandidateWorkContext {
    fn new(graph: &LGraph) -> Result<Self, WorkError> {
        let mut collector_adjacency_by_node = Vec::with_capacity(graph.layerless_nodes.len());
        let mut possible_collector_materialization = 0usize;

        for node in &graph.layerless_nodes {
            let mut collector_adjacency = 0usize;
            let mut has_input_collector = false;
            let mut has_output_collector = false;
            let mut has_input_target = false;
            let mut has_output_source = false;
            for port in &node.ports {
                if port.collector_type.is_some() {
                    collector_adjacency = checked_sum([
                        collector_adjacency,
                        port.incoming_edges.len(),
                        port.outgoing_edges.len(),
                    ])?;
                }
                match port.collector_type {
                    Some(PortType::Input) => {
                        has_input_collector = true;
                        has_input_target |= !port.incoming_edges.is_empty();
                    }
                    Some(PortType::Output) => {
                        has_output_collector = true;
                        has_output_source |= !port.outgoing_edges.is_empty();
                    }
                    None => {}
                }
            }
            collector_adjacency_by_node.push(collector_adjacency);

            if has_output_source && !has_input_collector {
                possible_collector_materialization = checked_add(
                    possible_collector_materialization,
                    collector_materialization_work_units(node, PortType::Input)?,
                )?;
            }
            if has_input_target && !has_output_collector {
                possible_collector_materialization = checked_add(
                    possible_collector_materialization,
                    collector_materialization_work_units(node, PortType::Output)?,
                )?;
            }
        }

        Ok(Self {
            collector_adjacency_by_node,
            possible_collector_materialization,
        })
    }
}

fn adapted_edge_reversal_candidate_work_units(
    graph: &LGraph,
    context: &EdgeReversalCandidateWorkContext,
    edge_index: usize,
) -> Result<usize, WorkError> {
    let Some(edge) = graph.edges.get(edge_index) else {
        return Ok(0);
    };
    if graph
        .layerless_nodes
        .get(edge.source.node)
        .and_then(|node| node.ports.get(edge.source.port))
        .is_none()
        || graph
            .layerless_nodes
            .get(edge.target.node)
            .and_then(|node| node.ports.get(edge.target.port))
            .is_none()
    {
        return Ok(0);
    }

    let (source_adjacency, source_lookup) =
        edge_reversal_endpoint_work_units(graph, context, edge.source, PortType::Output)?;
    let (target_adjacency, target_lookup) =
        edge_reversal_endpoint_work_units(graph, context, edge.target, PortType::Input)?;
    checked_sum([
        checked_mul(checked_add(source_adjacency, target_adjacency)?, 2)?,
        source_lookup,
        target_lookup,
        edge_reversal_payload_work_units(edge)?,
    ])
}

fn edge_reversal_endpoint_work_units(
    graph: &LGraph,
    context: &EdgeReversalCandidateWorkContext,
    endpoint: PortRef,
    lookup_collector_type: PortType,
) -> Result<(usize, usize), WorkError> {
    let node = &graph.layerless_nodes[endpoint.node];
    let port = &node.ports[endpoint.port];
    let adjacency = if port.collector_type.is_some() {
        context.collector_adjacency_by_node[endpoint.node]
    } else {
        checked_add(port.incoming_edges.len(), port.outgoing_edges.len())?
    };
    let lookup = if port.collector_type == Some(lookup_collector_type) {
        // At most two opposite collectors can be materialized while this processor runs.
        checked_sum([node.ports.len(), 3])?
    } else {
        0
    };
    Ok((adjacency, lookup))
}

pub(super) fn collector_materialization_work_units(
    node: &LNode,
    port_type: PortType,
) -> Result<usize, WorkError> {
    // Copy the complete collector ID, construct the port, append it, and conservatively cover a
    // capacity-independent relocation of every existing port. The estimate deliberately ignores
    // `Vec::capacity()` so logically identical graphs receive the same public budget result.
    checked_sum([
        node.ports.len(),
        node.id.len(),
        collector_port_id_suffix(port_type).len(),
        2,
    ])
}

fn edge_reversal_payload_work_units(edge: &LayeredEdge) -> Result<usize, WorkError> {
    // Endpoint replacement, reversed-state update, label placement swaps, and bend reversal.
    checked_sum([3, edge.labels.len(), edge.bend_points.len()])
}

pub(super) fn edge_reversal_work_units(graph: &LGraph) -> Result<usize, WorkError> {
    adapted_edge_reversal_work_units(graph, 1)
}

pub(super) fn reversed_edge_restorer_preparation_work_units(
    graph: &LGraph,
) -> Result<usize, WorkError> {
    let mut layer_memberships = 0usize;
    let mut member_ports = 0usize;
    let mut outgoing_snapshots = 0usize;
    for layer in &graph.layers {
        layer_memberships = checked_add(layer_memberships, layer.nodes.len())?;
        for &node_index in &layer.nodes {
            let Some(node) = graph.layerless_nodes.get(node_index) else {
                continue;
            };
            member_ports = checked_add(member_ports, node.ports.len())?;
            for port in &node.ports {
                outgoing_snapshots = checked_add(outgoing_snapshots, port.outgoing_edges.len())?;
            }
        }
    }
    checked_sum([
        graph.layers.len(),
        checked_mul(layer_memberships, 2)?,
        member_ports,
        checked_mul(outgoing_snapshots, 2)?,
    ])
}

pub(super) fn marked_edge_restoration_work_units(graph: &LGraph) -> Result<usize, WorkError> {
    let mut adjacency = 0usize;
    for node in &graph.layerless_nodes {
        for port in &node.ports {
            let marked_incoming = port
                .incoming_edges
                .iter()
                .filter(|&&edge| graph.edges.get(edge).is_some_and(|edge| edge.reversed))
                .count();
            let marked_outgoing = port
                .outgoing_edges
                .iter()
                .filter(|&&edge| graph.edges.get(edge).is_some_and(|edge| edge.reversed))
                .count();
            adjacency = checked_add(
                adjacency,
                checked_mul(
                    checked_sum([
                        checked_mul(marked_incoming, port.incoming_edges.len())?,
                        checked_mul(marked_outgoing, port.outgoing_edges.len())?,
                    ])?,
                    2,
                )?,
            )?;
        }
    }

    let mut payload = 0usize;
    for edge in graph.edges.iter().filter(|edge| edge.reversed) {
        payload = checked_add(payload, edge_reversal_payload_work_units(edge)?)?;
    }
    checked_add(adjacency, payload)
}

fn checked_triangular(value: usize) -> Result<usize, WorkError> {
    if value <= 1 {
        return Ok(0);
    }
    let predecessor = value - 1;
    if value.is_multiple_of(2) {
        checked_mul(value / 2, predecessor)
    } else {
        checked_mul(value, predecessor / 2)
    }
}

pub(super) fn split_edge_payload_work_units(
    edge: &crate::graph::LayeredEdge,
    split_count: usize,
) -> Result<usize, WorkError> {
    if split_count == 0 {
        return Ok(0);
    }

    let label_count = edge.labels.len();
    let mut head_label_count = 0usize;
    let mut first_removal_shifts = 0usize;
    for (index, label) in edge.labels.iter().enumerate() {
        if label.placement == crate::graph::EdgeLabelPlacement::Head {
            head_label_count = checked_add(head_label_count, 1)?;
            first_removal_shifts = checked_add(first_removal_shifts, label_count - index - 1)?;
        }
    }

    // The first split clones every label and bend point, scans the old label vector, shifts the
    // tail for each removed head label, then moves each head label through the temporary vector.
    let first_split = checked_sum([
        checked_mul(label_count, 2)?,
        first_removal_shifts,
        checked_mul(head_label_count, 2)?,
        edge.bend_points.len(),
    ])?;
    if split_count == 1 {
        return Ok(first_split);
    }

    // Later segments contain only head labels. Removing them from the front produces the exact
    // triangular shift count on every downstream split, in addition to clone/scan/move passes.
    let later_split = checked_add(
        checked_mul(head_label_count, 4)?,
        checked_triangular(head_label_count)?,
    )?;
    checked_add(first_split, checked_mul(later_split, split_count - 1)?)
}

pub(super) fn greedy_cycle_breaker_work_units(graph: &LGraph) -> Result<usize, WorkError> {
    let node_count = graph.layerless_nodes.len();
    let (ports, adjacency) = graph_port_and_adjacency_work_units(graph)?;

    // A source/sink removal visits each port and adjacency reference a bounded number of times.
    // When a cyclic remainder has neither, ELK's greedy selector scans all nodes and then scans its
    // maximal candidate set; at most one such pair of scans removes each node. Three V^2 tranches
    // cover the node scan, candidate collection, and model-order/random choice without pretending
    // sparse E alone bounds the processor.
    let quadratic = checked_mul(checked_mul(node_count, node_count)?, 3)?;
    let linear = checked_mul(
        checked_sum([node_count, ports, adjacency, graph.edges.len()])?,
        4,
    )?;
    Ok(checked_sum([quadratic, linear, edge_reversal_work_units(graph)?])?.max(1))
}

pub(super) fn long_edge_splitter_work_units(graph: &LGraph) -> Result<usize, WorkError> {
    let base = local_graph_work_units(graph)?;
    let layer_count = graph.layers.len();
    if layer_count <= 2 {
        return Ok(base);
    }

    let mut split_count = 0usize;
    let mut target_adjacency_rewrite_work = 0usize;
    let mut split_payload_work = 0usize;
    for edge in &graph.edges {
        let source_layer = graph
            .layerless_nodes
            .get(edge.source.node)
            .and_then(|node| node.layer_index);
        let target_layer = graph
            .layerless_nodes
            .get(edge.target.node)
            .and_then(|node| node.layer_index);
        let Some(source_layer) = source_layer else {
            continue;
        };
        let Some(next_source_layer) = source_layer.checked_add(1) else {
            continue;
        };
        if next_source_layer >= layer_count {
            continue;
        }

        let remaining_layers = layer_count - next_source_layer;
        let edge_splits = match target_layer {
            None => 0,
            Some(target_layer)
                if target_layer == source_layer || target_layer == next_source_layer =>
            {
                0
            }
            Some(target_layer) if target_layer > next_source_layer => {
                (target_layer - next_source_layer).min(remaining_layers)
            }
            // A malformed/backward layered edge can still be visited once in every later source
            // layer by the mutation loop. Charge that reachable upper bound rather than assuming
            // the normal forward-edge invariant.
            Some(_) => remaining_layers,
        };
        split_count = checked_add(split_count, edge_splits)?;
        if edge_splits > 0 {
            let target_degree = graph
                .layerless_nodes
                .get(edge.target.node)
                .and_then(|node| node.ports.get(edge.target.port))
                .map(|port| port.incoming_edges.len())
                .unwrap_or(0);
            // Every downstream split removes its segment from the original target port and appends
            // the replacement. On collector ports the list length stays constant, so account for
            // both the linear search and shifting removal at that full degree on every split.
            target_adjacency_rewrite_work = checked_add(
                target_adjacency_rewrite_work,
                checked_mul(checked_mul(edge_splits, target_degree)?, 2)?,
            )?;
            split_payload_work = checked_add(
                split_payload_work,
                split_edge_payload_work_units(edge, edge_splits)?,
            )?;
        }
    }

    // Each split inserts one node, two ports, one edge, and their adjacency/layer memberships.
    // The cloned segment also copies every label on the first split and the head labels that move
    // to the downstream segment on every later split.
    checked_sum([
        base,
        checked_mul(split_count, 8)?,
        target_adjacency_rewrite_work,
        split_payload_work,
    ])
}
