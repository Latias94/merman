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
use crate::graph::{
    LGraph, LNode, LNodeKind, LPoint, LSize, LayeredEdge, PortRef, PortSide, PortType,
    collector_port_id_suffix,
};
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
    visit_edge_and_layer_constraint_reversal_candidates,
};
use crate::p1cycles::{
    break_cycles_depth_first, break_cycles_greedy, break_cycles_greedy_model_order,
    break_cycles_interactive, break_cycles_model_order, depth_first_cycle_breaker_may_mutate,
    interactive_cycle_breaker_may_mutate, visit_model_order_feedback_edges,
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
use crate::work::{
    NoopWorkControl, WorkControl, WorkError, checked_add, checked_mul, checked_n_log_n, checked_sum,
};

mod plan;

pub use plan::{assemble_processors, assemble_processors_for_graph};
use plan::{validate_ported_graph_processors, validate_ported_processors};

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

fn local_graph_work_units(graph: &LGraph) -> Result<usize, WorkError> {
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

fn processor_work_units(graph: &LGraph, kind: ProcessorKind) -> Result<usize, WorkError> {
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

fn edge_reversal_processor_work_units_with_base(
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

fn end_label_sorter_work_units(graph: &LGraph) -> Result<usize, WorkError> {
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

fn long_edge_joiner_work_units(graph: &LGraph) -> Result<usize, WorkError> {
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

fn port_list_sorter_work_units(graph: &LGraph) -> Result<usize, WorkError> {
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

fn sort_by_input_model_work_units(graph: &LGraph) -> Result<usize, WorkError> {
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
fn unprepared_reorder_node_ports_work_units(
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

fn graph_port_and_adjacency_work_units(graph: &LGraph) -> Result<(usize, usize), WorkError> {
    let (ports, incoming, outgoing) = graph_port_and_directional_adjacency_work_units(graph)?;
    Ok((ports, checked_add(incoming, outgoing)?))
}

fn graph_port_and_directional_adjacency_work_units(
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

struct AdaptedEdgeReversalStructureWork {
    adjacency: usize,
    lookup_per_pass: usize,
    materialization: usize,
}

fn adapted_edge_reversal_structure_work_units(
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

fn adapted_edge_reversal_work_units(
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

fn model_order_edge_reversal_work_units(graph: &LGraph) -> Result<usize, WorkError> {
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

fn edge_and_layer_constraint_reversal_work_units(graph: &LGraph) -> Result<usize, WorkError> {
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

fn collector_materialization_work_units(
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

fn edge_reversal_work_units(graph: &LGraph) -> Result<usize, WorkError> {
    adapted_edge_reversal_work_units(graph, 1)
}

fn reversed_edge_restorer_preparation_work_units(graph: &LGraph) -> Result<usize, WorkError> {
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

fn marked_edge_restoration_work_units(graph: &LGraph) -> Result<usize, WorkError> {
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

fn split_edge_payload_work_units(
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

fn greedy_cycle_breaker_work_units(graph: &LGraph) -> Result<usize, WorkError> {
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

fn long_edge_splitter_work_units(graph: &LGraph) -> Result<usize, WorkError> {
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
