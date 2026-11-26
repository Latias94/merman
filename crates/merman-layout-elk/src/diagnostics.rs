//! Guarded inspection of the source-backed layered pipeline.

use super::{
    Algorithm, ElkOperationSeed, Error, Graph, GraphExecution, HierarchySweepDebugTrace,
    LayeredPhase, ProcessorKind, Result, graph_to_source_input, source_port,
};
use source_port::{LGraph, LNodeKind};
use std::fmt::Write as _;

/// A guarded diagnostic session for inspecting the source-backed layered pipeline.
///
/// The underlying ELK graph remains private. Every method that executes processors enters a
/// fallible source-port pipeline boundary, so `randomSeed = 0` is either rejected or resolved
/// from the [`ElkOperationSeed`] supplied at construction.
///
/// The former raw re-export is intentionally unavailable:
///
/// ```compile_fail
/// use merman_layout_elk::source_port;
/// ```
pub struct SourcePhaseDiagnostics {
    graph: LGraph,
}

impl Drop for SourcePhaseDiagnostics {
    fn drop(&mut self) {
        detach_source_graph_hierarchy_for_drop(&mut self.graph);
    }
}

impl SourcePhaseDiagnostics {
    /// Builds a raw diagnostic session. Executing a graph that retains ELK's zero seed sentinel
    /// returns the same typed error as [`crate::layout`].
    pub fn from_graph(graph: &Graph) -> Result<Self> {
        Self::from_graph_with_optional_operation_seed(graph, None)
    }

    /// Builds a diagnostic session using an operation-owned seed for ELK's zero sentinel.
    pub fn from_graph_with_operation_seed(
        graph: &Graph,
        operation_seed: ElkOperationSeed,
    ) -> Result<Self> {
        Self::from_graph_with_optional_operation_seed(graph, Some(operation_seed))
    }

    fn from_graph_with_optional_operation_seed(
        graph: &Graph,
        operation_seed: Option<ElkOperationSeed>,
    ) -> Result<Self> {
        if graph.options.algorithm != Algorithm::Layered {
            return Err(Error::LayeredDiagnosticsRequired);
        }
        let input = graph_to_source_input(graph)?;
        let lgraph = match operation_seed {
            Some(operation_seed) => source_port::import_graph_with_operation_seed(
                &input,
                operation_seed.source_port_seed(),
            ),
            None => source_port::import_graph(&input),
        }
        .map_err(Error::SourceImport)?;
        Ok(Self { graph: lgraph })
    }

    /// Runs the source-backed pipeline until a phase completes.
    pub fn execute_until(&mut self, target: LayeredPhase) -> Result<Vec<ProcessorKind>> {
        source_port::execute_processors_until(&mut self.graph, target)
            .map_err(Error::SourcePipeline)
    }

    /// Runs the source-backed pipeline until a processor completes.
    pub fn execute_until_processor(&mut self, target: ProcessorKind) -> Result<Vec<ProcessorKind>> {
        source_port::execute_processors_until_processor(&mut self.graph, target)
            .map_err(Error::SourcePipeline)
    }

    /// Runs every currently ported processor for a flat graph.
    pub fn execute_all(&mut self) -> Result<Vec<ProcessorKind>> {
        source_port::execute_ported_processors(&mut self.graph).map_err(Error::SourcePipeline)
    }

    /// Runs the compound pipeline until a phase completes.
    pub fn execute_compound_until(&mut self, target: LayeredPhase) -> Result<Vec<GraphExecution>> {
        source_port::execute_ported_compound_processors_until(&mut self.graph, target)
            .map_err(Error::SourcePipeline)
    }

    /// Runs the compound pipeline until a processor completes.
    pub fn execute_compound_until_processor(
        &mut self,
        target: ProcessorKind,
    ) -> Result<Vec<GraphExecution>> {
        source_port::execute_ported_compound_processors_until_processor(&mut self.graph, target)
            .map_err(Error::SourcePipeline)
    }

    /// Runs every currently ported compound processor.
    pub fn execute_compound_all(&mut self) -> Result<Vec<GraphExecution>> {
        source_port::execute_ported_compound_processors(&mut self.graph)
            .map_err(Error::SourcePipeline)
    }

    /// Runs the guarded compound prefix used by the hierarchical crossing-sweep diagnostic.
    pub fn inspect_compound_crossings_after_processor(
        &mut self,
        target: ProcessorKind,
    ) -> Result<(Vec<GraphExecution>, Option<HierarchySweepDebugTrace>)> {
        source_port::inspect_compound_crossings_after_processor(&mut self.graph, target)
            .map_err(Error::SourcePipeline)
    }

    /// Formats the private source graph for human diagnostics without exposing executable phase
    /// APIs or the mutable `LGraph` itself.
    pub fn graph_dump(&self) -> String {
        let mut output = String::new();
        write_source_graph_dump(&mut output, &self.graph, 0)
            .expect("writing to String cannot fail");
        output
    }
}

fn write_source_graph_dump(output: &mut String, graph: &LGraph, depth: usize) -> std::fmt::Result {
    let mut stack = vec![(graph, depth)];
    while let Some((current, current_depth)) = stack.pop() {
        write_source_graph_dump_frame(output, current, current_depth)?;
        stack.extend(
            current
                .layerless_nodes
                .iter()
                .rev()
                .filter_map(|node| node.nested_graph.as_deref())
                .map(|nested| (nested, current_depth + 1)),
        );
    }
    Ok(())
}

/// Detaches nested graphs before their owners are dropped so deeply nested diagnostics do not
/// recurse through `Box<LGraph>` destruction on small worker stacks.
fn detach_source_graph_hierarchy_for_drop(graph: &mut LGraph) {
    let mut pending = graph
        .layerless_nodes
        .iter_mut()
        .filter_map(|node| node.nested_graph.take())
        .collect::<Vec<_>>();
    while let Some(mut nested) = pending.pop() {
        pending.extend(
            nested
                .layerless_nodes
                .iter_mut()
                .filter_map(|node| node.nested_graph.take()),
        );
    }
}

fn write_source_graph_dump_frame(
    output: &mut String,
    graph: &LGraph,
    depth: usize,
) -> std::fmt::Result {
    let indent = "  ".repeat(depth);
    writeln!(
        output,
        "{indent}graph {} parent={:?} size=({}, {}) offset=({}, {}) padding=({}, {}, {}, {})",
        graph.id,
        graph.parent_node_id,
        graph.size.width,
        graph.size.height,
        graph.offset.x,
        graph.offset.y,
        graph.padding.left,
        graph.padding.right,
        graph.padding.top,
        graph.padding.bottom
    )?;
    writeln!(
        output,
        "{indent}options direction={:?} port_constraints={:?} thoroughness={} hierarchical_sweepiness={} consider_model_order={:?} force_node_model_order={} port_model_order={}",
        graph.options.direction,
        graph.options.port_constraints,
        graph.options.thoroughness,
        graph.options.hierarchical_sweepiness,
        graph.options.consider_model_order_strategy,
        graph.options.force_node_model_order,
        graph.options.consider_model_order_port_model_order
    )?;
    writeln!(output, "{indent}layerless:")?;
    for (index, node) in graph.layerless_nodes.iter().enumerate() {
        writeln!(
            output,
            "{indent}- #{index} {} kind={:?} layer={:?} order={:?} pos=({}, {}) size=({}, {}) margin=({}, {}, {}, {}) port_constraints={:?} parent_graph={}",
            node.id,
            node.kind,
            node.layer_index,
            node.model_order,
            node.position.x,
            node.position.y,
            node.size.width,
            node.size.height,
            node.margin.left,
            node.margin.right,
            node.margin.top,
            node.margin.bottom,
            node.port_constraints,
            node.nested_graph.is_some()
        )?;
        if node.kind == LNodeKind::ExternalPort {
            writeln!(
                output,
                "{indent}  external side={:?} size=({}, {}) ratio_or_position={} replaced={:?}",
                node.external_port_side,
                node.external_port_size.width,
                node.external_port_size.height,
                node.port_ratio_or_position,
                node.replaced_external_port_dummy
            )?;
        }
        for (port_index, port) in node.ports.iter().enumerate() {
            let incoming = port
                .incoming_edges
                .iter()
                .map(|edge| graph.edges[*edge].id.as_str())
                .collect::<Vec<_>>();
            let outgoing = port
                .outgoing_edges
                .iter()
                .map(|edge| graph.edges[*edge].id.as_str())
                .collect::<Vec<_>>();
            writeln!(
                output,
                "{indent}  port #{port_index} {} type={:?} side={:?} order={:?} index={:?} pos=({}, {}) anchor=({}, {}) size=({}, {}) border={:?} inside={} dummy={:?} origin={:?} in=[{}] out=[{}]",
                port.id,
                port.port_type,
                port.side,
                port.model_order,
                port.port_index,
                port.position.x,
                port.position.y,
                port.anchor.x,
                port.anchor.y,
                port.size.width,
                port.size.height,
                port.border_offset,
                port.inside_connections,
                port.port_dummy,
                node.origin_port,
                incoming.join(","),
                outgoing.join(",")
            )?;
        }
    }
    if !graph.edges.is_empty() {
        writeln!(output, "{indent}edges:")?;
        for (edge_index, edge) in graph.edges.iter().enumerate() {
            let source_attached = graph.edge_source_attached(edge_index);
            let target_attached = graph.edge_target_attached(edge_index);
            writeln!(
                output,
                "{indent}- #{edge_index} {} {}:{} -> {}:{} segment={:?} reversed={} attached=({source_attached},{target_attached}) bends={:?}",
                edge.id,
                edge.source.node,
                edge.source.port,
                edge.target.node,
                edge.target.port,
                edge.compound_segment,
                edge.reversed,
                edge.bend_points
            )?;
        }
    }
    writeln!(output, "{indent}layers:")?;
    for (index, layer) in graph.layers.iter().enumerate() {
        let nodes = layer
            .nodes
            .iter()
            .map(|node| {
                let lnode = &graph.layerless_nodes[*node];
                format!(
                    "{}#{node}[{:?},order={:?},pos=({},{}),size=({},{})]",
                    lnode.id,
                    lnode.kind,
                    lnode.model_order,
                    lnode.position.x,
                    lnode.position.y,
                    lnode.size.width,
                    lnode.size.height
                )
            })
            .collect::<Vec<_>>();
        writeln!(
            output,
            "{indent}- layer {index} size=({}, {}) nodes={}",
            layer.size.width,
            layer.size.height,
            nodes.join(" -> ")
        )?;
    }
    writeln!(output)?;
    Ok(())
}
