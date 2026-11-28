//! ELK layered processor pipeline.
//!
//! Source references:
//! - https://github.com/eclipse-elk/elk/blob/62d5909f96fad541bc101ad52dabaece6b7eab7e/plugins/org.eclipse.elk.alg.layered/src/org/eclipse/elk/alg/layered/GraphConfigurator.java
//! - https://github.com/eclipse-elk/elk/blob/62d5909f96fad541bc101ad52dabaece6b7eab7e/plugins/org.eclipse.elk.alg.layered/src/org/eclipse/elk/alg/layered/ElkLayered.java
//! - https://github.com/eclipse-elk/elk/tree/62d5909f96fad541bc101ad52dabaece6b7eab7e/plugins/org.eclipse.elk.alg.layered/src/org/eclipse/elk/alg/layered/p1cycles
//! - https://github.com/eclipse-elk/elk/tree/62d5909f96fad541bc101ad52dabaece6b7eab7e/plugins/org.eclipse.elk.alg.layered/src/org/eclipse/elk/alg/layered/p2layers
//! - https://github.com/eclipse-elk/elk/tree/62d5909f96fad541bc101ad52dabaece6b7eab7e/plugins/org.eclipse.elk.alg.layered/src/org/eclipse/elk/alg/layered/p3order
//! - https://github.com/eclipse-elk/elk/tree/62d5909f96fad541bc101ad52dabaece6b7eab7e/plugins/org.eclipse.elk.alg.layered/src/org/eclipse/elk/alg/layered/p4nodes
//! - https://github.com/eclipse-elk/elk/tree/62d5909f96fad541bc101ad52dabaece6b7eab7e/plugins/org.eclipse.elk.alg.layered/src/org/eclipse/elk/alg/layered/p5edges
//! - https://github.com/eclipse-elk/elk/blob/62d5909f96fad541bc101ad52dabaece6b7eab7e/plugins/org.eclipse.elk.alg.layered/src/org/eclipse/elk/alg/layered/intermediate/HierarchicalNodeResizingProcessor.java

use super::options::{ContentAlignment, LayeringStrategy, PortConstraints};
use crate::RandomSeedError;
use crate::compound::{
    preprocess_source_ported_compound_graph, source_ported_cross_hierarchy_segment_count,
};
use crate::configurator::configure_graph_properties;
#[cfg(test)]
use crate::graph::ReorderNodePortsWorkContext;
use crate::graph::{LGraph, LNode, LNodeKind, LPoint, LSize, PortSide};
#[cfg(test)]
use crate::intermediate::edge_and_layer_constraint_reversal_may_mutate;
use crate::intermediate::{
    IntermediateError, calculate_layer_sizes_and_graph_height, insert_label_dummies,
    join_long_edges, merge_hyperedge_dummies, position_interactive_external_ports,
    postprocess_end_labels, postprocess_layer_constraints, preprocess_end_labels,
    preprocess_layer_constraints, process_hierarchical_port_constraints,
    process_hierarchical_port_dummy_sizes, process_hierarchical_port_orthogonal_edges,
    process_hierarchical_port_positions, process_inverted_ports, remove_label_dummies,
    restore_reversed_edges, reverse_edges_for_edge_and_layer_constraints, select_label_sides,
    sort_end_labels, split_long_edges, switch_label_dummies,
};
use crate::p1cycles::{
    break_cycles_depth_first, break_cycles_greedy, break_cycles_greedy_model_order,
    break_cycles_interactive, break_cycles_model_order,
};
use crate::p2layers::{LayeringError, layer_network_simplex, layer_with_strategy};
use crate::p3order::{
    process_port_sides, sort_by_input_model, sort_port_lists_with_work_control,
    sweep::{
        CrossMinType, HierarchySweepDebugTrace,
        debug_crossings_layer_sweep_hierarchical_with_work_control,
        minimize_crossings_layer_sweep_hierarchical_with_work_control,
        minimize_crossings_layer_sweep_with_type, minimize_crossings_layer_sweep_with_work_control,
    },
};
use crate::p4nodes::{
    calculate_innermost_node_margins, calculate_label_and_node_sizes, place_nodes_brandes_koepf,
    place_nodes_linear_segments, place_nodes_network_simplex, place_nodes_simple,
    process_in_layer_constraints,
};
use crate::p5edges::route_edges_orthogonal;
use crate::selfloops::{
    postprocess_self_loops, preprocess_self_loops, restore_self_loop_ports, route_self_loops,
};
use crate::transform::{GraphTransformMode, transform_graph_direction};
use crate::work::{NoopWorkControl, WorkControl, WorkError, checked_add, checked_mul, checked_sum};

mod plan;
mod processor_work;

pub use plan::{assemble_processors, assemble_processors_for_graph};
use plan::{validate_ported_graph_processors, validate_ported_processors};
#[cfg(test)]
use processor_work::{
    adapted_edge_reversal_structure_work_units, adapted_edge_reversal_work_units,
    collector_materialization_work_units, edge_and_layer_constraint_reversal_work_units,
    edge_reversal_work_units, end_label_sorter_work_units, graph_port_and_adjacency_work_units,
    graph_port_and_directional_adjacency_work_units, greedy_cycle_breaker_work_units,
    long_edge_joiner_work_units, long_edge_splitter_work_units, marked_edge_restoration_work_units,
    model_order_edge_reversal_work_units, port_list_sorter_work_units,
    reversed_edge_restorer_preparation_work_units, sort_by_input_model_work_units,
    split_edge_payload_work_units, unprepared_reorder_node_ports_work_units,
};
use processor_work::{
    edge_reversal_processor_work_units_with_base, local_graph_work_units, processor_work_units,
};

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum PipelineError {
    #[error(transparent)]
    Component(#[from] crate::components::ComponentError),
    #[error(transparent)]
    NodeSize(#[from] crate::options::NodeSizeError),
    #[error(transparent)]
    NetworkSimplex(#[from] crate::common::networksimplex::NetworkSimplexError),
    #[error(transparent)]
    RandomSeed(#[from] RandomSeedError),
    #[error("layered processor `{kind:?}` is not ported yet")]
    UnsupportedProcessor { kind: ProcessorKind },
    #[error("source-backed compound ELK layout does not support this graph yet: {reason}")]
    UnsupportedCompoundGraph { reason: &'static str },
    #[error(transparent)]
    Intermediate(#[from] IntermediateError),
    #[error(transparent)]
    Work(#[from] WorkError),
    #[error(transparent)]
    Layering(LayeringError),
}

impl From<LayeringError> for PipelineError {
    fn from(error: LayeringError) -> Self {
        match error {
            LayeringError::Work(error) => Self::Work(error),
            other => Self::Layering(other),
        }
    }
}

pub type PipelineResult<T> = Result<T, PipelineError>;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum LayeredPhase {
    P1CycleBreaking,
    P2Layering,
    P3NodeOrdering,
    P4NodePlacement,
    P5EdgeRouting,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ProcessorSlot {
    pub phase: Option<LayeredPhase>,
    pub kind: ProcessorKind,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[allow(clippy::enum_variant_names)]
pub enum ProcessorKind {
    DirectionPreprocessor,
    CommentPreprocessor,
    EdgeAndLayerConstraintEdgeReverser,
    InteractiveExternalPortPositioner,
    PartitionPreprocessor,
    GreedyCycleBreaker,
    DepthFirstCycleBreaker,
    InteractiveCycleBreaker,
    ModelOrderCycleBreaker,
    GreedyModelOrderCycleBreaker,
    LayerConstraintPreprocessor,
    NetworkSimplexLayerer,
    LongestPathLayerer,
    LongestPathSourceLayerer,
    CoffmanGrahamLayerer,
    InteractiveLayerer,
    StretchWidthLayerer,
    MinWidthLayerer,
    BreadthFirstModelOrderLayerer,
    DepthFirstModelOrderLayerer,
    LabelDummyInserter,
    SelfLoopPreProcessor,
    LayerConstraintPostprocessor,
    PartitionMidprocessor,
    LongEdgeSplitter,
    PortSideProcessor,
    InvertedPortProcessor,
    PortListSorter,
    SortByInputModelProcessor,
    NorthSouthPortPreprocessor,
    HighDegreeNodeLayerProcessor,
    NodePromotion,
    PartitionPostprocessor,
    HierarchicalPortConstraintProcessor,
    SemiInteractiveCrossMinProcessor,
    LayerSweepCrossingMinimizerBarycenter,
    LayerSweepCrossingMinimizerOneSidedGreedySwitch,
    LayerSweepCrossingMinimizerTwoSidedGreedySwitch,
    InteractiveCrossingMinimizer,
    NoCrossingMinimizer,
    InLayerConstraintProcessor,
    EndNodePortLabelManagementProcessor,
    LabelAndNodeSizeProcessor,
    InnermostNodeMarginCalculator,
    CommentNodeMarginCalculator,
    EndLabelPreprocessor,
    LabelSideSelector,
    HyperedgeDummyMerger,
    HierarchicalPortDummySizeProcessor,
    BKNodePlacer,
    SimpleNodePlacer,
    InteractiveNodePlacer,
    LinearSegmentsNodePlacer,
    NetworkSimplexPlacer,
    LayerSizeAndGraphHeightCalculator,
    HierarchicalPortPositionProcessor,
    OrthogonalEdgeRouter,
    PolylineEdgeRouter,
    SplineEdgeRouter,
    ConstraintsPostprocessor,
    CommentPostprocessor,
    LongEdgeJoiner,
    NorthSouthPortPostprocessor,
    HorizontalGraphCompactor,
    LabelDummyRemover,
    FinalSplineBendpointsCalculator,
    EndLabelSorter,
    ReversedEdgeRestorer,
    EndLabelPostprocessor,
    HierarchicalNodeResizer,
    DirectionPostprocessor,
    SelfLoopPortRestorer,
    SelfLoopRouter,
    SelfLoopPostProcessor,
    LabelDummySwitcher,
    CenterLabelManagementProcessor,
    HierarchicalPortOrthogonalEdgeRouter,
    HypernodesProcessor,
    BreakingPointInserter,
    BreakingPointProcessor,
    BreakingPointRemover,
    SingleEdgeGraphWrapper,
}

impl ProcessorKind {
    pub fn is_hierarchy_aware(self) -> bool {
        matches!(
            self,
            Self::LayerSweepCrossingMinimizerBarycenter
                | Self::LayerSweepCrossingMinimizerOneSidedGreedySwitch
                | Self::LayerSweepCrossingMinimizerTwoSidedGreedySwitch
        )
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GraphExecution {
    pub graph_id: String,
    pub parent_node_id: Option<String>,
    pub processors: Vec<ProcessorKind>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct GraphAlgorithm {
    next_processor: usize,
    processors: Vec<ProcessorSlot>,
    execution: GraphExecution,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct GraphArenaParent {
    graph: usize,
    node: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct GraphArenaEntry {
    parent: Option<GraphArenaParent>,
}

#[derive(Debug)]
struct CompoundGraphArena {
    entries: Vec<GraphArenaEntry>,
    algorithms: Vec<GraphAlgorithm>,
    graph_slots: Vec<Option<Box<LGraph>>>,
}

impl CompoundGraphArena {
    fn new(root: &LGraph) -> Self {
        let mut entries = vec![GraphArenaEntry { parent: None }];
        let mut algorithms = vec![graph_algorithm(root)];
        let mut search = vec![(0usize, root)];

        while let Some((graph_index, graph)) = search.pop() {
            for (node_index, node) in graph.layerless_nodes.iter().enumerate() {
                let Some(nested) = node.nested_graph.as_deref() else {
                    continue;
                };
                let child_index = entries.len();
                entries.push(GraphArenaEntry {
                    parent: Some(GraphArenaParent {
                        graph: graph_index,
                        node: node_index,
                    }),
                });
                algorithms.push(graph_algorithm(nested));
                // Match ELK's `ArrayDeque.push` discovery order. The last sibling is searched
                // first, while the collected graph list is later executed in reverse discovery
                // order.
                search.push((child_index, nested));
            }
        }

        let mut graph_slots = Vec::with_capacity(entries.len());
        graph_slots.resize_with(entries.len(), || None);

        Self {
            entries,
            algorithms,
            graph_slots,
        }
    }

    fn into_executions(self) -> Vec<GraphExecution> {
        // ELK's `collectAllGraphsBottomUp` returns the reverse of discovery order. This is not
        // interchangeable with ordinary DFS postorder: sibling subtrees are interleaved in a
        // public, error-observable order.
        self.algorithms
            .into_iter()
            .rev()
            .map(|algorithm| algorithm.execution)
            .collect()
    }
}

fn graph_algorithm(graph: &LGraph) -> GraphAlgorithm {
    GraphAlgorithm {
        next_processor: 0,
        processors: assemble_processors_for_graph(graph),
        execution: GraphExecution {
            graph_id: graph.id.clone(),
            parent_node_id: graph.parent_node_id.clone(),
            processors: Vec::new(),
        },
    }
}

/// Execute the source-backed layered pipeline until the requested phase completes.
///
/// This follows the processor sequence assembled from Eclipse ELK's `GraphConfigurator` and phase
/// dependency configuration. Processors that have not been ported fail explicitly instead of being
/// silently skipped, because skipping them would make later phase evidence misleading.
pub fn execute_processors_until(
    graph: &mut LGraph,
    target: LayeredPhase,
) -> PipelineResult<Vec<ProcessorKind>> {
    let mut work_control = NoopWorkControl;
    execute_processors_until_with_work_control(graph, target, &mut work_control)
}

pub fn execute_processors_until_with_work_control(
    graph: &mut LGraph,
    target: LayeredPhase,
    work_control: &mut dyn WorkControl,
) -> PipelineResult<Vec<ProcessorKind>> {
    let mut executed = Vec::new();
    let processors = prepare_single_graph_processors(graph, work_control)?;

    for slot in processors {
        execute_processor_with_work_control(graph, slot.kind, work_control)?;
        executed.push(slot.kind);

        if slot.phase == Some(target) {
            break;
        }
    }

    Ok(executed)
}

/// Execute the source-backed layered pipeline until the requested processor completes.
///
/// This is a diagnostic companion to [`execute_processors_until`]. It keeps the normal assembled
/// processor order and stops immediately after `target` executes.
pub fn execute_processors_until_processor(
    graph: &mut LGraph,
    target: ProcessorKind,
) -> PipelineResult<Vec<ProcessorKind>> {
    let mut work_control = NoopWorkControl;
    execute_processors_until_processor_with_work_control(graph, target, &mut work_control)
}

pub fn execute_processors_until_processor_with_work_control(
    graph: &mut LGraph,
    target: ProcessorKind,
    work_control: &mut dyn WorkControl,
) -> PipelineResult<Vec<ProcessorKind>> {
    let mut executed = Vec::new();
    let processors = prepare_single_graph_processors(graph, work_control)?;

    for slot in processors {
        execute_processor_with_work_control(graph, slot.kind, work_control)?;
        executed.push(slot.kind);

        if slot.kind == target {
            break;
        }
    }

    Ok(executed)
}

/// Execute all currently source-ported processors assembled for this graph.
///
/// This is the library equivalent of `ElkLayered.layout(...)`: it uses the same assembled
/// processor list as the phase-limited runner and rejects unsupported processors before mutating
/// the graph.
pub fn execute_ported_processors(graph: &mut LGraph) -> PipelineResult<Vec<ProcessorKind>> {
    let mut work_control = NoopWorkControl;
    execute_ported_processors_with_work_control(graph, &mut work_control)
}

pub fn execute_ported_processors_with_work_control(
    graph: &mut LGraph,
    work_control: &mut dyn WorkControl,
) -> PipelineResult<Vec<ProcessorKind>> {
    let mut executed = Vec::new();
    // The full runner promises to reject unsupported processors without mutating the caller's
    // graph. Account for the validation copy before materializing it, then run the normal charged
    // configuration path only after the copied graph has proved the processor list is supported.
    charge_hierarchy_work(graph, work_control)?;
    let mut validation_graph = graph.copy_preserving_reference_domains();
    configure_graph_properties(&mut validation_graph)?;
    validate_ported_processors(&assemble_processors_for_graph(&validation_graph))?;
    drop(validation_graph);
    let processors = prepare_single_graph_processors(graph, work_control)?;

    if let Some(mut components) = crate::components::split(graph, work_control)? {
        // ComponentsProcessor copies the configured properties shallowly. Its Java Random
        // is one shared object, so later components continue the previous component's stream.
        let mut random = graph.random.clone();
        for component in &mut components {
            component.graph.random = random;
            for slot in &processors {
                execute_processor_with_work_control(&mut component.graph, slot.kind, work_control)?;
            }
            random = component.graph.random.clone();
            charge_hierarchy_work(&component.graph, work_control)?;
        }
        crate::components::combine(graph, components, work_control)?;
        graph.random = random;
        executed.extend(processors.iter().map(|slot| slot.kind));
    } else {
        for slot in processors {
            execute_processor_with_work_control(graph, slot.kind, work_control)?;
            executed.push(slot.kind);
        }
    }

    // Compound layout resizes through its scheduled HierarchicalNodeResizer. Flat layout
    // applies ElkLayered.resizeGraph after direction postprocessing and component combination.
    if graph.options.hierarchy_handling != super::options::HierarchyHandling::IncludeChildren
        && graph.options.node_size_minimum.is_some()
    {
        work_control.charge(checked_add(graph.layerless_nodes.len(), 1)?)?;
        resize_graph_to_minimum(graph)?;
    }

    Ok(executed)
}

fn prepare_single_graph_processors(
    graph: &mut LGraph,
    work_control: &mut dyn WorkControl,
) -> PipelineResult<Vec<ProcessorSlot>> {
    // Processor execution below is root-local, but the source-backed GraphConfigurator walks every
    // nested graph to resolve options and random seeds. Charge that complete hierarchy atomically
    // before configuration mutates any graph; a root-only preflight would under-account this API.
    charge_hierarchy_work(graph, work_control)?;
    validate_node_size_minima(graph)?;
    configure_graph_properties(graph)?;
    Ok(assemble_processors_for_graph(graph))
}

fn validate_node_size_minima(graph: &LGraph) -> PipelineResult<()> {
    let mut pending = vec![graph];
    while let Some(graph) = pending.pop() {
        graph.options.validate_node_size_minimum()?;
        pending.extend(
            graph
                .layerless_nodes
                .iter()
                .filter_map(|node| node.nested_graph.as_deref()),
        );
    }
    Ok(())
}

/// Execute the source-backed layered pipeline for a compound graph hierarchy.
///
/// This follows Eclipse ELK's `ElkLayered#hierarchicalLayout(...)` execution shape: collect all
/// nested graphs in bottom-up order, keep a processor cursor for each graph, pause non-root graphs
/// at hierarchy-aware processors, execute those hierarchy-aware processors only at the root graph,
/// and then continue from the deepest graph again. Cross-hierarchy edges are represented as
/// hierarchical external port dummies by the importer and routed by the hierarchical port processors
/// in the same schedule.
pub fn execute_ported_compound_processors(
    graph: &mut LGraph,
) -> PipelineResult<Vec<GraphExecution>> {
    let mut work_control = NoopWorkControl;
    execute_ported_compound_processors_with_work_control(graph, &mut work_control)
}

pub fn execute_ported_compound_processors_with_work_control(
    graph: &mut LGraph,
    work_control: &mut dyn WorkControl,
) -> PipelineResult<Vec<GraphExecution>> {
    execute_ported_compound_processors_to(graph, None, work_control)
}

/// Execute the source-backed compound pipeline until the requested phase completes.
///
/// This is a diagnostic companion to [`execute_ported_compound_processors`]. It follows the same
/// hierarchical schedule, but stops after every graph algorithm has advanced past the requested
/// phase slot. It is useful for source-parity probes that need to inspect intermediate layer order.
pub fn execute_ported_compound_processors_until(
    graph: &mut LGraph,
    target: LayeredPhase,
) -> PipelineResult<Vec<GraphExecution>> {
    let mut work_control = NoopWorkControl;
    execute_ported_compound_processors_until_with_work_control(graph, target, &mut work_control)
}

pub fn execute_ported_compound_processors_until_with_work_control(
    graph: &mut LGraph,
    target: LayeredPhase,
    work_control: &mut dyn WorkControl,
) -> PipelineResult<Vec<GraphExecution>> {
    execute_ported_compound_processors_to(graph, Some(PipelineStop::Phase(target)), work_control)
}

/// Execute the source-backed compound pipeline until the requested processor completes.
///
/// For hierarchy-aware processors, only the root graph executes the processor in ELK's schedule;
/// child graph algorithms pause at that processor and resume after the root has run it.
pub fn execute_ported_compound_processors_until_processor(
    graph: &mut LGraph,
    target: ProcessorKind,
) -> PipelineResult<Vec<GraphExecution>> {
    let mut work_control = NoopWorkControl;
    execute_ported_compound_processors_until_processor_with_work_control(
        graph,
        target,
        &mut work_control,
    )
}

pub fn execute_ported_compound_processors_until_processor_with_work_control(
    graph: &mut LGraph,
    target: ProcessorKind,
    work_control: &mut dyn WorkControl,
) -> PipelineResult<Vec<GraphExecution>> {
    execute_ported_compound_processors_to(
        graph,
        Some(PipelineStop::Processor(target)),
        work_control,
    )
}

/// Execute a guarded compound prefix, then collect the source-port hierarchical crossing trace.
///
/// This is the only public diagnostics entry point that runs the randomized layer sweep outside
/// the normal full pipeline. It always enters through the same fallible configuration boundary as
/// every other public executor, so ELK's `randomSeed = 0` sentinel cannot observe the placeholder
/// `JavaRandom` installed on an unconfigured graph.
pub fn inspect_compound_crossings_after_processor(
    graph: &mut LGraph,
    target: ProcessorKind,
) -> PipelineResult<(Vec<GraphExecution>, Option<HierarchySweepDebugTrace>)> {
    let mut work_control = NoopWorkControl;
    inspect_compound_crossings_after_processor_with_work_control(graph, target, &mut work_control)
}

pub fn inspect_compound_crossings_after_processor_with_work_control(
    graph: &mut LGraph,
    target: ProcessorKind,
    work_control: &mut dyn WorkControl,
) -> PipelineResult<(Vec<GraphExecution>, Option<HierarchySweepDebugTrace>)> {
    let executions = execute_ported_compound_processors_until_processor_with_work_control(
        graph,
        target,
        work_control,
    )?;
    let diagnostic_work = hierarchy_processor_work_units_with_preflight(
        graph,
        ProcessorKind::LayerSweepCrossingMinimizerBarycenter,
        1,
        work_control,
    )?;
    // The diagnostic path additionally snapshots graph/run/layer state while executing the same
    // barycenter sweep. Fold that extra base copy into every preflight prefix so the sweep cannot
    // be admitted on its own; the pre-admitted scans consume one indivisible charge.
    work_control.charge(diagnostic_work)?;
    let trace = debug_crossings_layer_sweep_hierarchical_with_work_control(
        graph,
        CrossMinType::Barycenter,
        work_control,
    )?;
    Ok((executions, trace))
}

fn execute_ported_compound_processors_to(
    graph: &mut LGraph,
    stop: Option<PipelineStop>,
    work_control: &mut dyn WorkControl,
) -> PipelineResult<Vec<GraphExecution>> {
    charge_hierarchy_work(graph, work_control)?;
    if stop.is_none() {
        validate_ported_graph_processors(graph)?;
    }
    validate_node_size_minima(graph)?;
    charge_compound_preprocess_work(graph, work_control)?;
    preprocess_source_ported_compound_graph(graph);
    charge_hierarchy_work(graph, work_control)?;
    configure_graph_properties(graph)?;
    reject_unsupported_compound_graph(graph)?;
    review_and_correct_hierarchical_processors(graph)?;

    let mut arena = build_compound_graph_arena_with_work_control(graph, work_control)?;
    const ROOT_GRAPH: usize = 0;
    if stop.is_none() {
        // Configuration can change the assembled processor list. Validate in the same official
        // bottom-up order used for execution and public result emission.
        for algorithm in arena.algorithms.iter().rev() {
            validate_ported_processors(&algorithm.processors)?;
        }
    }

    while arena.algorithms[ROOT_GRAPH].next_processor
        < arena.algorithms[ROOT_GRAPH].processors.len()
        && stop
            .map(|stop| !compound_algorithms_reached_stop(&arena.algorithms, ROOT_GRAPH, stop))
            .unwrap_or(true)
    {
        charge_compound_graph_round_work(arena.entries.len(), work_control)?;
        execute_compound_graph_round(
            graph,
            &arena.entries,
            &mut arena.graph_slots,
            &mut arena.algorithms,
            stop,
            work_control,
        )?;
    }

    Ok(arena.into_executions())
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum PipelineStop {
    Phase(LayeredPhase),
    Processor(ProcessorKind),
}

fn compound_algorithms_reached_stop(
    algorithms: &[GraphAlgorithm],
    root_index: usize,
    stop: PipelineStop,
) -> bool {
    match stop {
        PipelineStop::Phase(target) => algorithms.iter().all(|algorithm| {
            !algorithm
                .processors
                .iter()
                .any(|slot| slot.phase == Some(target))
                || algorithm.processors[..algorithm.next_processor]
                    .iter()
                    .any(|slot| slot.phase == Some(target))
        }),
        PipelineStop::Processor(target) if target.is_hierarchy_aware() => algorithms[root_index]
            .execution
            .processors
            .contains(&target),
        PipelineStop::Processor(target) => algorithms.iter().all(|algorithm| {
            !algorithm.processors.iter().any(|slot| slot.kind == target)
                || algorithm.execution.processors.contains(&target)
        }),
    }
}

fn slot_matches_stop(slot: ProcessorSlot, stop: PipelineStop) -> bool {
    match stop {
        PipelineStop::Phase(target) => slot.phase == Some(target),
        PipelineStop::Processor(target) => slot.kind == target,
    }
}

fn execute_compound_algorithm_until_pause(
    graph: &mut LGraph,
    algorithm: &mut GraphAlgorithm,
    is_root: bool,
    mut parent: Option<ParentGraph<'_>>,
    stop: Option<PipelineStop>,
    work_control: &mut dyn WorkControl,
) -> PipelineResult<()> {
    while algorithm.next_processor < algorithm.processors.len() {
        let slot = algorithm.processors[algorithm.next_processor];
        let kind = slot.kind;
        algorithm.next_processor += 1;

        let hierarchy_aware = kind.is_hierarchy_aware();
        if hierarchy_aware && !is_root {
            break;
        }

        let size = if hierarchy_aware {
            if kind == ProcessorKind::LayerSweepCrossingMinimizerBarycenter
                && graph
                    .layerless_nodes
                    .iter()
                    .all(|node| node.nested_graph.is_none())
                && graph
                    .options
                    .consider_model_order_crossing_counter_node_influence
                    == 0.0
            {
                charge_hierarchy_work(graph, work_control)?;
                crate::p3order::sweep::minimize_single_graph_hierarchical_barycenter_with_work_control(
                    graph, work_control,
                )?;
            } else {
                charge_hierarchy_processor_work(graph, kind, work_control)?;
                execute_hierarchy_aware_processor(graph, kind, work_control)?;
            }
            actual_graph_size(graph)
        } else {
            execute_processor_with_work_control(graph, kind, work_control)?;
            actual_graph_size(graph)
        };
        if kind == ProcessorKind::HierarchicalNodeResizer
            && let Some(parent) = parent.as_mut()
        {
            transfer_nested_graph_layout_to_parent_node(graph, parent.graph, parent.node, size);
        }
        algorithm.execution.processors.push(kind);

        if hierarchy_aware || stop.is_some_and(|stop| slot_matches_stop(slot, stop)) {
            break;
        }
    }

    Ok(())
}

struct ParentGraph<'a> {
    graph: &'a mut LGraph,
    node: usize,
}

/// Owns one round's Rust-only detachment of the compound hierarchy.
///
/// Eclipse ELK keeps nested graph objects attached throughout execution. Rust needs disjoint
/// mutable owners to execute a child and update its parent at the same time, so the round stores
/// every nested graph in an arena slot. Completed children are immediately reattached before their
/// parent executes. Dropping the guard restores every still-detached graph in child-before-parent
/// order after an error or unwind.
struct DetachedCompoundGraphs<'a> {
    root: &'a mut LGraph,
    entries: &'a [GraphArenaEntry],
    slots: &'a mut [Option<Box<LGraph>>],
}

impl<'a> DetachedCompoundGraphs<'a> {
    fn new(
        root: &'a mut LGraph,
        entries: &'a [GraphArenaEntry],
        slots: &'a mut [Option<Box<LGraph>>],
    ) -> Self {
        assert_eq!(
            slots.len(),
            entries.len(),
            "the reusable graph slots must match the arena topology"
        );
        assert!(
            slots.iter().all(Option::is_none),
            "all compound graph slots must be empty between rounds"
        );

        Self {
            root,
            entries,
            slots,
        }
    }

    fn detach_all(&mut self) {
        // Arena discovery always assigns a parent before its children. Detach in that order so a
        // nested graph is still reachable from either the root or its parent's occupied slot.
        for graph_index in 1..self.entries.len() {
            let parent = self.entries[graph_index]
                .parent
                .expect("every non-root graph must have an arena parent");
            let graph = if parent.graph == 0 {
                self.root.layerless_nodes[parent.node].nested_graph.take()
            } else {
                self.slots[parent.graph]
                    .as_deref_mut()
                    .expect("a parent graph must remain detached until its children complete")
                    .layerless_nodes[parent.node]
                    .nested_graph
                    .take()
            }
            .expect("the compound graph topology must remain stable during layered execution");
            self.slots[graph_index] = Some(graph);
        }
    }

    fn execute_and_reattach(
        &mut self,
        graph_index: usize,
        algorithm: &mut GraphAlgorithm,
        stop: Option<PipelineStop>,
        work_control: &mut dyn WorkControl,
    ) -> PipelineResult<()> {
        let parent = self.entries[graph_index]
            .parent
            .expect("only nested graphs use detached execution");
        if parent.graph == 0 {
            let result = {
                let graph = self.slots[graph_index]
                    .as_deref_mut()
                    .expect("the nested graph must occupy its arena slot");
                execute_compound_algorithm_until_pause(
                    graph,
                    algorithm,
                    false,
                    Some(ParentGraph {
                        graph: self.root,
                        node: parent.node,
                    }),
                    stop,
                    work_control,
                )
            };
            let graph = self.slots[graph_index]
                .take()
                .expect("the completed graph must still occupy its arena slot");
            self.root.layerless_nodes[parent.node].nested_graph = Some(graph);
            return result;
        }

        debug_assert!(parent.graph < graph_index);
        let result = {
            let (parents, current_and_later) = self.slots.split_at_mut(graph_index);
            let parent_graph = parents[parent.graph]
                .as_deref_mut()
                .expect("the parent graph must remain detached until its turn");
            let graph = current_and_later[0]
                .as_deref_mut()
                .expect("the nested graph must occupy its arena slot");
            execute_compound_algorithm_until_pause(
                graph,
                algorithm,
                false,
                Some(ParentGraph {
                    graph: parent_graph,
                    node: parent.node,
                }),
                stop,
                work_control,
            )
        };

        let graph = self.slots[graph_index]
            .take()
            .expect("the completed graph must still occupy its arena slot");
        self.slots[parent.graph]
            .as_deref_mut()
            .expect("the parent graph must remain detached until its turn")
            .layerless_nodes[parent.node]
            .nested_graph = Some(graph);
        result
    }

    fn restore_slot(&mut self, graph_index: usize) {
        let Some(graph) = self.slots[graph_index].take() else {
            return;
        };
        let parent = self.entries[graph_index]
            .parent
            .expect("every non-root graph must have an arena parent");
        if parent.graph == 0 {
            self.root.layerless_nodes[parent.node].nested_graph = Some(graph);
        } else {
            self.slots[parent.graph]
                .as_deref_mut()
                .expect("a detached child must still have a detached parent during restoration")
                .layerless_nodes[parent.node]
                .nested_graph = Some(graph);
        }
    }
}

impl Drop for DetachedCompoundGraphs<'_> {
    fn drop(&mut self) {
        for graph_index in (1..self.entries.len()).rev() {
            self.restore_slot(graph_index);
        }
    }
}

fn execute_compound_graph_round(
    root: &mut LGraph,
    entries: &[GraphArenaEntry],
    graph_slots: &mut [Option<Box<LGraph>>],
    algorithms: &mut [GraphAlgorithm],
    stop: Option<PipelineStop>,
    work_control: &mut dyn WorkControl,
) -> PipelineResult<()> {
    let mut detached = DetachedCompoundGraphs::new(root, entries, graph_slots);
    detached.detach_all();
    for graph_index in (1..algorithms.len()).rev() {
        detached.execute_and_reattach(
            graph_index,
            &mut algorithms[graph_index],
            stop,
            work_control,
        )?;
    }
    execute_compound_algorithm_until_pause(
        detached.root,
        &mut algorithms[0],
        true,
        None,
        stop,
        work_control,
    )
}

fn execute_hierarchy_aware_processor(
    graph: &mut LGraph,
    kind: ProcessorKind,
    work_control: &mut dyn WorkControl,
) -> PipelineResult<()> {
    match kind {
        ProcessorKind::LayerSweepCrossingMinimizerBarycenter => {
            minimize_crossings_layer_sweep_hierarchical_with_work_control(
                graph,
                CrossMinType::Barycenter,
                work_control,
            )?;
        }
        ProcessorKind::LayerSweepCrossingMinimizerOneSidedGreedySwitch => {
            minimize_crossings_layer_sweep_hierarchical_with_work_control(
                graph,
                CrossMinType::OneSidedGreedySwitch,
                work_control,
            )?;
        }
        ProcessorKind::LayerSweepCrossingMinimizerTwoSidedGreedySwitch => {
            minimize_crossings_layer_sweep_hierarchical_with_work_control(
                graph,
                CrossMinType::TwoSidedGreedySwitch,
                work_control,
            )?;
        }
        _ => return Err(PipelineError::UnsupportedProcessor { kind }),
    }
    Ok(())
}

fn reject_unsupported_compound_graph(_graph: &LGraph) -> PipelineResult<()> {
    Ok(())
}

fn review_and_correct_hierarchical_processors(root: &mut LGraph) -> PipelineResult<()> {
    let root_crossing = root.options.crossing_minimization_strategy;
    let root_greedy_switch = root.options.greedy_switch_hierarchical_type;
    root.try_for_each_graph_mut(|graph| {
        if graph.options.crossing_minimization_strategy != root_crossing {
            return Err(PipelineError::UnsupportedCompoundGraph {
                reason: "child graphs must use the root hierarchy-aware crossing minimizer",
            });
        }
        graph.options.greedy_switch_hierarchical_type = root_greedy_switch;
        Ok(())
    })
}

fn build_compound_graph_arena_with_work_control(
    graph: &mut LGraph,
    work_control: &mut dyn WorkControl,
) -> PipelineResult<CompoundGraphArena> {
    charge_hierarchy_arena_plan_work(graph, work_control)?;
    Ok(CompoundGraphArena::new(graph))
}

fn transfer_nested_graph_layout_to_parent_node(
    nested_graph: &mut LGraph,
    parent: &mut LGraph,
    node_index: usize,
    size: LSize,
) {
    transfer_external_port_dummy_layout_to_parent_node(nested_graph, parent, node_index);
    let has_external_ports = nested_graph.graph_properties.external_ports;

    {
        let node = &mut parent.layerless_nodes[node_index];
        if has_external_ports {
            node.port_constraints = PortConstraints::FixedPos;
            resize_layered_node(node, size, false, true);
        } else {
            resize_layered_node(node, size, true, true);
        }
    }

    if has_external_ports {
        parent.graph_properties.non_free_ports = true;
    }
}

fn actual_graph_size(graph: &LGraph) -> LSize {
    LSize {
        width: graph.size.width + graph.padding.left + graph.padding.right,
        height: graph.size.height + graph.padding.top + graph.padding.bottom,
    }
}

fn transfer_external_port_dummy_layout_to_parent_node(
    nested_graph: &mut LGraph,
    parent: &mut LGraph,
    parent_node_index: usize,
) {
    for dummy_index in 0..nested_graph.layerless_nodes.len() {
        if nested_graph.layerless_nodes[dummy_index].kind != LNodeKind::ExternalPort {
            continue;
        }
        let Some(origin_port) = &nested_graph.layerless_nodes[dummy_index].origin_port else {
            continue;
        };
        if !origin_port.belongs_to(parent) || origin_port.port.node != parent_node_index {
            continue;
        }

        let parent_port_index = origin_port.port.port;
        let port_position = external_port_position(nested_graph, dummy_index);
        let external_side = nested_graph.layerless_nodes[dummy_index].external_port_side;
        if let Some(parent_port) = parent.layerless_nodes[parent_node_index]
            .ports
            .get_mut(parent_port_index)
        {
            parent_port.position = port_position;
            parent_port.set_side(external_side);
        }
    }
}

fn external_port_position(graph: &mut LGraph, dummy_index: usize) -> LPoint {
    let dummy_size = graph.layerless_nodes[dummy_index].size;
    let external_size = graph.layerless_nodes[dummy_index].external_port_size;
    let external_side = graph.layerless_nodes[dummy_index].external_port_side;
    let border_offset = graph.layerless_nodes[dummy_index]
        .ports
        .first()
        .and_then(|port| port.border_offset)
        .unwrap_or(0.0);

    let mut port_position = LPoint {
        x: graph.layerless_nodes[dummy_index].position.x + dummy_size.width / 2.0,
        y: graph.layerless_nodes[dummy_index].position.y + dummy_size.height / 2.0,
    };

    match external_side {
        PortSide::North => {
            port_position.x += graph.padding.left + graph.offset.x - external_size.width / 2.0;
            port_position.y = -external_size.height - border_offset;
            graph.layerless_nodes[dummy_index].position.y =
                -(graph.padding.top + border_offset + graph.offset.y);
        }
        PortSide::East => {
            port_position.x =
                graph.size.width + graph.padding.left + graph.padding.right + border_offset;
            port_position.y += graph.padding.top + graph.offset.y - external_size.height / 2.0;
            graph.layerless_nodes[dummy_index].position.x =
                graph.size.width + graph.padding.right + border_offset - graph.offset.x;
        }
        PortSide::South => {
            port_position.x += graph.padding.left + graph.offset.x - external_size.width / 2.0;
            port_position.y =
                graph.size.height + graph.padding.top + graph.padding.bottom + border_offset;
            graph.layerless_nodes[dummy_index].position.y =
                graph.size.height + graph.padding.bottom + border_offset - graph.offset.y;
        }
        PortSide::West => {
            port_position.x = -external_size.width - border_offset;
            port_position.y += graph.padding.top + graph.offset.y - external_size.height / 2.0;
            graph.layerless_nodes[dummy_index].position.x =
                -(graph.padding.left + border_offset + graph.offset.x);
        }
        PortSide::Undefined => {}
    }

    port_position
}

#[cfg(test)]
fn execute_processor(graph: &mut LGraph, kind: ProcessorKind) -> PipelineResult<()> {
    let mut work_control = NoopWorkControl;
    execute_processor_with_work_control(graph, kind, &mut work_control)
}

fn execute_processor_with_work_control(
    graph: &mut LGraph,
    kind: ProcessorKind,
    work_control: &mut dyn WorkControl,
) -> PipelineResult<()> {
    let work_units = if matches!(
        kind,
        ProcessorKind::ModelOrderCycleBreaker | ProcessorKind::EdgeAndLayerConstraintEdgeReverser
    ) {
        // Exact candidate accounting uses bounded O(V + E + P) shadow state. Reject budgets
        // smaller than the graph scan before reserving that state, then reuse the accepted floor
        // as the processor base. The final complete tranche remains transactional below.
        let base = local_graph_work_units(graph)?;
        work_control.check(base)?;
        edge_reversal_processor_work_units_with_base(graph, kind, base)?
    } else {
        processor_work_units(graph, kind)?
    };
    work_control.check(work_units)?;
    work_control.charge(work_units)?;
    match kind {
        ProcessorKind::DirectionPreprocessor => {
            transform_graph_direction(graph, GraphTransformMode::ToInputDirection);
        }
        ProcessorKind::DirectionPostprocessor => {
            transform_graph_direction(graph, GraphTransformMode::ToInternalLeftToRight);
        }
        ProcessorKind::EdgeAndLayerConstraintEdgeReverser => {
            reverse_edges_for_edge_and_layer_constraints(graph);
        }
        ProcessorKind::InteractiveExternalPortPositioner => {
            position_interactive_external_ports(graph);
        }
        ProcessorKind::SelfLoopPreProcessor => preprocess_self_loops(graph),
        ProcessorKind::GreedyCycleBreaker => break_cycles_greedy(graph),
        ProcessorKind::DepthFirstCycleBreaker => break_cycles_depth_first(graph),
        ProcessorKind::InteractiveCycleBreaker => break_cycles_interactive(graph),
        ProcessorKind::ModelOrderCycleBreaker => break_cycles_model_order(graph),
        ProcessorKind::GreedyModelOrderCycleBreaker => break_cycles_greedy_model_order(graph),
        ProcessorKind::LayerConstraintPreprocessor => preprocess_layer_constraints(graph)?,
        ProcessorKind::LabelDummyInserter => insert_label_dummies(graph),
        ProcessorKind::NetworkSimplexLayerer => layer_network_simplex(graph)?,
        ProcessorKind::LongestPathLayerer => {
            layer_with_strategy(graph, LayeringStrategy::LongestPath, work_control)?
        }
        ProcessorKind::LongestPathSourceLayerer => {
            layer_with_strategy(graph, LayeringStrategy::LongestPathSource, work_control)?
        }
        ProcessorKind::CoffmanGrahamLayerer => {
            layer_with_strategy(graph, LayeringStrategy::CoffmanGraham, work_control)?
        }
        ProcessorKind::InteractiveLayerer => {
            layer_with_strategy(graph, LayeringStrategy::Interactive, work_control)?
        }
        ProcessorKind::StretchWidthLayerer => {
            layer_with_strategy(graph, LayeringStrategy::StretchWidth, work_control)?
        }
        ProcessorKind::MinWidthLayerer => {
            layer_with_strategy(graph, LayeringStrategy::MinWidth, work_control)?
        }
        ProcessorKind::LayerConstraintPostprocessor => postprocess_layer_constraints(graph)?,
        ProcessorKind::HierarchicalPortConstraintProcessor => {
            process_hierarchical_port_constraints(graph);
        }
        ProcessorKind::LongEdgeSplitter => split_long_edges(graph),
        ProcessorKind::PortSideProcessor => process_port_sides(graph),
        ProcessorKind::InvertedPortProcessor => process_inverted_ports(graph),
        ProcessorKind::PortListSorter => sort_port_lists_with_work_control(graph, work_control)?,
        ProcessorKind::SortByInputModelProcessor => sort_by_input_model(graph, work_control)?,
        ProcessorKind::LayerSweepCrossingMinimizerBarycenter => {
            minimize_crossings_layer_sweep_with_work_control(graph, work_control)?;
        }
        ProcessorKind::LayerSweepCrossingMinimizerOneSidedGreedySwitch => {
            minimize_crossings_layer_sweep_with_type(graph, CrossMinType::OneSidedGreedySwitch);
        }
        ProcessorKind::LayerSweepCrossingMinimizerTwoSidedGreedySwitch => {
            minimize_crossings_layer_sweep_with_type(graph, CrossMinType::TwoSidedGreedySwitch);
        }
        ProcessorKind::SelfLoopPortRestorer => restore_self_loop_ports(graph),
        ProcessorKind::InLayerConstraintProcessor => process_in_layer_constraints(graph),
        ProcessorKind::LabelAndNodeSizeProcessor => calculate_label_and_node_sizes(graph),
        ProcessorKind::InnermostNodeMarginCalculator => calculate_innermost_node_margins(graph),
        ProcessorKind::SelfLoopRouter => route_self_loops(graph),
        ProcessorKind::LabelDummySwitcher => switch_label_dummies(graph),
        ProcessorKind::LabelSideSelector => select_label_sides(graph),
        ProcessorKind::HyperedgeDummyMerger => merge_hyperedge_dummies(graph),
        ProcessorKind::EndLabelPreprocessor => preprocess_end_labels(graph),
        ProcessorKind::BKNodePlacer => place_nodes_brandes_koepf(graph),
        ProcessorKind::SimpleNodePlacer => place_nodes_simple(graph),
        ProcessorKind::LinearSegmentsNodePlacer => place_nodes_linear_segments(graph),
        ProcessorKind::NetworkSimplexPlacer => place_nodes_network_simplex(graph)?,
        ProcessorKind::LayerSizeAndGraphHeightCalculator => {
            calculate_layer_sizes_and_graph_height(graph);
        }
        ProcessorKind::HierarchicalPortDummySizeProcessor => {
            process_hierarchical_port_dummy_sizes(graph);
        }
        ProcessorKind::HierarchicalPortPositionProcessor => {
            process_hierarchical_port_positions(graph);
        }
        ProcessorKind::OrthogonalEdgeRouter => route_edges_orthogonal(graph),
        ProcessorKind::HierarchicalPortOrthogonalEdgeRouter => {
            process_hierarchical_port_orthogonal_edges(graph);
        }
        ProcessorKind::LongEdgeJoiner => join_long_edges(graph),
        ProcessorKind::SelfLoopPostProcessor => postprocess_self_loops(graph),
        ProcessorKind::LabelDummyRemover => remove_label_dummies(graph),
        ProcessorKind::EndLabelSorter => sort_end_labels(graph),
        ProcessorKind::ReversedEdgeRestorer => restore_reversed_edges(graph),
        ProcessorKind::EndLabelPostprocessor => postprocess_end_labels(graph),
        ProcessorKind::HierarchicalNodeResizer => resize_hierarchical_node_graph(graph)?,
        ProcessorKind::NoCrossingMinimizer => {}
        _ => return Err(PipelineError::UnsupportedProcessor { kind }),
    }

    Ok(())
}

fn local_hierarchy_work_units(graph: &LGraph) -> Result<usize, WorkError> {
    Ok(checked_sum([
        local_graph_work_units(graph)?,
        graph.hierarchy_edges.len(),
        graph.cross_hierarchy_edges.len(),
    ])?
    .max(1))
}

/// Fold a hierarchy estimate while checking every admitted traversal-stack prefix.
///
/// Root-local work is computed without allocation so arithmetic overflow keeps its former
/// priority over a caller interruption. Child references are reserved as one unit each, checked
/// as a batch, and only then appended to the traversal stack. The final caller still charges one
/// complete tranche, so a failed preflight never advances the public budget.
fn preflight_hierarchy_fold_work_units<State>(
    graph: &LGraph,
    work_control: &mut dyn WorkControl,
    state: &mut State,
    mut discover_graph: impl FnMut(&LGraph, &mut State) -> Result<(), WorkError>,
    mut local_work_units: impl FnMut(&LGraph) -> Result<usize, WorkError>,
    mut admitted_work_units: impl FnMut(usize, &State) -> Result<usize, WorkError>,
) -> Result<usize, WorkError> {
    discover_graph(graph, state)?;
    let mut total = local_work_units(graph)?;
    work_control.check(admitted_work_units(total, state)?)?;

    let mut stack = Vec::new();
    reserve_hierarchy_children(
        graph,
        &mut stack,
        &mut total,
        work_control,
        state,
        &mut discover_graph,
        &mut admitted_work_units,
    )?;
    while let Some(current) = stack.pop() {
        let current_work = local_work_units(current)?;
        let remaining_work = current_work
            .checked_sub(1)
            .ok_or(WorkError::ArithmeticOverflow)?;
        total = checked_add(total, remaining_work)?;
        work_control.check(admitted_work_units(total, state)?)?;
        reserve_hierarchy_children(
            current,
            &mut stack,
            &mut total,
            work_control,
            state,
            &mut discover_graph,
            &mut admitted_work_units,
        )?;
    }
    admitted_work_units(total, state)
}

#[allow(clippy::too_many_arguments)]
fn reserve_hierarchy_children<'a, State>(
    graph: &'a LGraph,
    stack: &mut Vec<&'a LGraph>,
    total: &mut usize,
    work_control: &mut dyn WorkControl,
    state: &mut State,
    discover_graph: &mut impl FnMut(&LGraph, &mut State) -> Result<(), WorkError>,
    admitted_work_units: &mut impl FnMut(usize, &State) -> Result<usize, WorkError>,
) -> Result<(), WorkError> {
    let mut child_count = 0usize;
    for nested in graph
        .layerless_nodes
        .iter()
        .filter_map(|node| node.nested_graph.as_deref())
    {
        discover_graph(nested, state)?;
        child_count = checked_add(child_count, 1)?;
    }
    if child_count == 0 {
        return Ok(());
    }

    *total = checked_add(*total, child_count)?;
    work_control.check(admitted_work_units(*total, state)?)?;
    stack.reserve(child_count);
    stack.extend(
        graph
            .layerless_nodes
            .iter()
            .filter_map(|node| node.nested_graph.as_deref()),
    );
    Ok(())
}

fn preflight_hierarchy_work_units(
    graph: &LGraph,
    work_control: &mut dyn WorkControl,
    local_work_units: impl FnMut(&LGraph) -> Result<usize, WorkError>,
) -> Result<usize, WorkError> {
    preflight_hierarchy_fold_work_units(
        graph,
        work_control,
        &mut (),
        |_, ()| Ok(()),
        local_work_units,
        |total, ()| Ok(total),
    )
}

fn hierarchy_work_units_with_preflight(
    graph: &LGraph,
    work_control: &mut dyn WorkControl,
) -> Result<usize, WorkError> {
    preflight_hierarchy_work_units(graph, work_control, local_hierarchy_work_units)
}

#[cfg(test)]
fn hierarchy_work_units(graph: &LGraph) -> Result<usize, WorkError> {
    let mut work_control = NoopWorkControl;
    hierarchy_work_units_with_preflight(graph, &mut work_control)
}

fn local_hierarchy_arena_plan_work_units(graph: &LGraph) -> Result<usize, WorkError> {
    let nested_graph_count = graph
        .layerless_nodes
        .iter()
        .filter(|node| node.nested_graph.is_some())
        .count();

    // One discovery unit is reserved by `preflight_hierarchy_work_units`. The complete local
    // contribution covers the node scan, parent links, arena entry, algorithm binding, and
    // reusable graph slot without depending on hierarchy depth.
    checked_sum([4, graph.layerless_nodes.len(), nested_graph_count])
}

fn hierarchy_arena_plan_work_units_with_preflight(
    graph: &LGraph,
    work_control: &mut dyn WorkControl,
) -> Result<usize, WorkError> {
    preflight_hierarchy_work_units(graph, work_control, local_hierarchy_arena_plan_work_units)
}

#[cfg(test)]
fn hierarchy_arena_plan_work_units(graph: &LGraph) -> Result<usize, WorkError> {
    let mut work_control = NoopWorkControl;
    hierarchy_arena_plan_work_units_with_preflight(graph, &mut work_control)
}

fn compound_graph_round_work_units(graph_count: usize) -> Result<usize, WorkError> {
    let nested_graph_count = graph_count.saturating_sub(1);
    // Every round dispatches each graph once and moves every nested graph out of and back into its
    // parent exactly once. Reusable arena slots are allocated by the one-time plan tranche, so no
    // per-round slot initialization is charged here.
    checked_sum([graph_count, checked_mul(nested_graph_count, 2)?])
}

fn local_compound_preprocess_work_units(graph: &LGraph) -> Result<usize, WorkError> {
    let mut segment_count = 0usize;
    let mut label_count = 0usize;
    for edge in &graph.hierarchy_edges {
        segment_count = checked_add(
            segment_count,
            source_ported_cross_hierarchy_segment_count(
                edge.source_node_id.as_str(),
                edge.target_node_id.as_str(),
                &edge.source_path,
                &edge.target_path,
            )?,
        )?;
        label_count = checked_add(label_count, edge.labels.len())?;
    }

    checked_sum([
        checked_mul(local_hierarchy_work_units(graph)?, 4)?,
        segment_count,
        label_count,
    ])
}

fn compound_preprocess_work_units_with_preflight(
    graph: &LGraph,
    work_control: &mut dyn WorkControl,
) -> Result<usize, WorkError> {
    preflight_hierarchy_work_units(graph, work_control, local_compound_preprocess_work_units)
}

pub(crate) fn charge_hierarchy_work(
    graph: &LGraph,
    work_control: &mut dyn WorkControl,
) -> PipelineResult<()> {
    let work_units = hierarchy_work_units_with_preflight(graph, work_control)?;
    work_control.charge(work_units)?;
    Ok(())
}

fn charge_hierarchy_arena_plan_work(
    graph: &LGraph,
    work_control: &mut dyn WorkControl,
) -> PipelineResult<()> {
    let work_units = hierarchy_arena_plan_work_units_with_preflight(graph, work_control)?;
    work_control.charge(work_units)?;
    Ok(())
}

fn charge_compound_graph_round_work(
    graph_count: usize,
    work_control: &mut dyn WorkControl,
) -> PipelineResult<()> {
    let work_units = compound_graph_round_work_units(graph_count)?;
    work_control.check(work_units)?;
    work_control.charge(work_units)?;
    Ok(())
}

fn charge_hierarchy_processor_work(
    graph: &LGraph,
    kind: ProcessorKind,
    work_control: &mut dyn WorkControl,
) -> PipelineResult<()> {
    let work_units = hierarchy_processor_work_units_with_preflight(graph, kind, 0, work_control)?;
    work_control.charge(work_units)?;
    Ok(())
}

fn hierarchy_processor_work_units_with_preflight(
    graph: &LGraph,
    kind: ProcessorKind,
    extra_base_copies: usize,
    work_control: &mut dyn WorkControl,
) -> Result<usize, WorkError> {
    let hierarchical_sweep = matches!(
        kind,
        ProcessorKind::LayerSweepCrossingMinimizerBarycenter
            | ProcessorKind::LayerSweepCrossingMinimizerOneSidedGreedySwitch
            | ProcessorKind::LayerSweepCrossingMinimizerTwoSidedGreedySwitch
    );
    let mut prefix = (1usize, 0usize);
    preflight_hierarchy_fold_work_units(
        graph,
        work_control,
        &mut prefix,
        |current, (max_thoroughness, edge_count)| {
            if hierarchical_sweep {
                *max_thoroughness = (*max_thoroughness).max(current.options.thoroughness.max(1));
                *edge_count = checked_add(*edge_count, current.edges.len())?;
            }
            Ok(())
        },
        local_hierarchy_work_units,
        |base, (max_thoroughness, edge_count)| {
            let processor_multiplier = if hierarchical_sweep {
                checked_mul(*max_thoroughness, (*edge_count).max(1))?
            } else {
                1
            };
            checked_mul(base, checked_add(processor_multiplier, extra_base_copies)?)
        },
    )
}

#[cfg(test)]
fn hierarchy_processor_work_units(graph: &LGraph, kind: ProcessorKind) -> Result<usize, WorkError> {
    let mut work_control = NoopWorkControl;
    hierarchy_processor_work_units_with_preflight(graph, kind, 0, &mut work_control)
}

fn charge_compound_preprocess_work(
    graph: &LGraph,
    work_control: &mut dyn WorkControl,
) -> PipelineResult<()> {
    let work_units = compound_preprocess_work_units_with_preflight(graph, work_control)?;
    work_control.charge(work_units)?;
    Ok(())
}

fn resize_hierarchical_node_graph(graph: &mut LGraph) -> PipelineResult<()> {
    let layered_nodes = graph
        .layers
        .iter()
        .flat_map(|layer| layer.nodes.iter().copied())
        .collect::<Vec<_>>();
    for node in layered_nodes {
        graph.layerless_nodes[node].layer_index = None;
    }
    graph.layers.clear();
    resize_graph_to_minimum(graph)
}

fn resize_graph_to_minimum(graph: &mut LGraph) -> PipelineResult<()> {
    graph.options.validate_node_size_minimum()?;
    let old_size = actual_graph_size(graph);
    let mut new_size = old_size;
    if let Some(minimum) = graph.options.effective_node_size_minimum() {
        new_size.width = new_size.width.max(minimum.width);
        new_size.height = new_size.height.max(minimum.height);
    }
    resize_graph_no_really_i_mean_it(graph, old_size, new_size);
    Ok(())
}

fn resize_graph_no_really_i_mean_it(graph: &mut LGraph, old_size: LSize, new_size: LSize) {
    fn alignment_offset(alignment: ContentAlignment, old: f64, new: f64) -> f64 {
        if new <= old {
            return 0.0;
        }
        match alignment {
            ContentAlignment::Start => 0.0,
            ContentAlignment::Center => (new - old) / 2.0,
            ContentAlignment::End => new - old,
        }
    }
    graph.offset.x += alignment_offset(
        graph.options.horizontal_content_alignment,
        old_size.width,
        new_size.width,
    );
    graph.offset.y += alignment_offset(
        graph.options.vertical_content_alignment,
        old_size.height,
        new_size.height,
    );
    if graph.graph_properties.external_ports
        && (new_size.width > old_size.width || new_size.height > old_size.height)
    {
        for node in &mut graph.layerless_nodes {
            if node.kind != LNodeKind::ExternalPort {
                continue;
            }
            match node.external_port_side {
                PortSide::East => node.position.x += new_size.width - old_size.width,
                PortSide::South => node.position.y += new_size.height - old_size.height,
                PortSide::North | PortSide::West | PortSide::Undefined => {}
            }
        }
    }

    graph.size.width = new_size.width - graph.padding.left - graph.padding.right;
    graph.size.height = new_size.height - graph.padding.top - graph.padding.bottom;
}

fn resize_layered_node(node: &mut LNode, new_size: LSize, move_ports: bool, move_labels: bool) {
    let old_size = node.size;
    let width_ratio = ratio_or_one(new_size.width, old_size.width);
    let height_ratio = ratio_or_one(new_size.height, old_size.height);
    let width_diff = new_size.width - old_size.width;
    let height_diff = new_size.height - old_size.height;

    if move_ports {
        let fixed_ports = node.port_constraints == PortConstraints::FixedPos;
        for port in &mut node.ports {
            match port.side {
                PortSide::North => {
                    if !fixed_ports {
                        port.position.x *= width_ratio;
                    }
                }
                PortSide::East => {
                    port.position.x += width_diff;
                    if !fixed_ports {
                        port.position.y *= height_ratio;
                    }
                }
                PortSide::South => {
                    if !fixed_ports {
                        port.position.x *= width_ratio;
                    }
                    port.position.y += height_diff;
                }
                PortSide::West => {
                    if !fixed_ports {
                        port.position.y *= height_ratio;
                    }
                }
                PortSide::Undefined => {}
            }
        }
    }

    if move_labels {
        for label in &mut node.labels {
            let mid_x = label.position.x + label.size.width / 2.0;
            let mid_y = label.position.y + label.size.height / 2.0;
            let width_percent = ratio_or_zero(mid_x, old_size.width);
            let height_percent = ratio_or_zero(mid_y, old_size.height);

            if width_percent + height_percent >= 1.0 {
                if width_percent - height_percent > 0.0 && mid_y >= 0.0 {
                    label.position.x += width_diff;
                    label.position.y += height_diff * height_percent;
                } else if width_percent - height_percent < 0.0 && mid_x >= 0.0 {
                    label.position.x += width_diff * width_percent;
                    label.position.y += height_diff;
                }
            }
        }
    }

    node.size = new_size;
}

fn ratio_or_one(numerator: f64, denominator: f64) -> f64 {
    if denominator == 0.0 {
        1.0
    } else {
        numerator / denominator
    }
}

fn ratio_or_zero(numerator: f64, denominator: f64) -> f64 {
    if denominator == 0.0 {
        0.0
    } else {
        numerator / denominator
    }
}

#[cfg(test)]
mod tests;
