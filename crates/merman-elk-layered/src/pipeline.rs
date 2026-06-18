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

use super::options::{
    CrossingMinimizationStrategy, CycleBreakingStrategy, EdgeRouting, ElkDirection,
    GreedySwitchType, LayeredOptions, LayeringStrategy, NodePlacementStrategy, OrderingStrategy,
    PortConstraints, WrappingStrategy,
};
use crate::compound::preprocess_source_ported_compound_graph;
use crate::configurator::{configure_graph_properties, configured_options};
use crate::graph::{LGraph, LNode, LNodeKind, LPoint, LSize, PortSide};
use crate::intermediate::{
    IntermediateError, calculate_layer_sizes_and_graph_height, insert_label_dummies,
    join_long_edges, postprocess_end_labels, postprocess_layer_constraints, preprocess_end_labels,
    preprocess_layer_constraints, process_hierarchical_port_constraints,
    process_hierarchical_port_dummy_sizes, process_hierarchical_port_orthogonal_edges,
    process_hierarchical_port_positions, process_inverted_ports, remove_label_dummies,
    restore_reversed_edges, reverse_edges_for_edge_and_layer_constraints, select_label_sides,
    sort_end_labels, split_long_edges, switch_label_dummies,
};
use crate::p1cycles::{break_cycles_greedy, break_cycles_greedy_model_order};
use crate::p2layers::layer_network_simplex;
use crate::p3order::{
    process_port_sides, sort_by_input_model, sort_port_lists,
    sweep::{
        CrossMinType, minimize_crossings_layer_sweep,
        minimize_crossings_layer_sweep_hierarchical_with_type,
        minimize_crossings_layer_sweep_with_type,
    },
};
use crate::p4nodes::{
    calculate_innermost_node_margins, calculate_label_and_node_sizes, place_nodes_brandes_koepf,
    process_in_layer_constraints,
};
use crate::p5edges::route_edges_orthogonal;
use crate::selfloops::{
    postprocess_self_loops, preprocess_self_loops, restore_self_loop_ports, route_self_loops,
};
use crate::transform::{GraphTransformMode, transform_graph_direction};

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum PipelineError {
    #[error("layered processor `{kind:?}` is not ported yet")]
    UnsupportedProcessor { kind: ProcessorKind },
    #[error("source-backed compound ELK layout does not support this graph yet: {reason}")]
    UnsupportedCompoundGraph { reason: &'static str },
    #[error(transparent)]
    Intermediate(#[from] IntermediateError),
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

impl LayeredPhase {
    const ALL: [Self; 5] = [
        Self::P1CycleBreaking,
        Self::P2Layering,
        Self::P3NodeOrdering,
        Self::P4NodePlacement,
        Self::P5EdgeRouting,
    ];

    fn ordinal(self) -> usize {
        match self {
            Self::P1CycleBreaking => 0,
            Self::P2Layering => 1,
            Self::P3NodeOrdering => 2,
            Self::P4NodePlacement => 3,
            Self::P5EdgeRouting => 4,
        }
    }
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
    path: Vec<usize>,
    next_processor: usize,
    processors: Vec<ProcessorSlot>,
    execution: GraphExecution,
}

#[derive(Debug)]
struct Config {
    slots: [Vec<ProcessorKind>; 6],
    phases: [Option<ProcessorKind>; 5],
}

impl Config {
    fn add_to_slot(&mut self, slot: usize, kind: ProcessorKind) {
        if !self.slots[slot].contains(&kind) {
            self.slots[slot].push(kind);
        }
    }

    fn add_before(&mut self, phase: LayeredPhase, kind: ProcessorKind) {
        self.add_to_slot(phase.ordinal(), kind);
    }

    fn add_after(&mut self, phase: LayeredPhase, kind: ProcessorKind) {
        self.add_to_slot(phase.ordinal() + 1, kind);
    }

    fn add_phase(&mut self, phase: LayeredPhase, kind: ProcessorKind) {
        self.phases[phase.ordinal()] = Some(kind);
    }

    fn merge(&mut self, other: Config) {
        for (slot, processors) in other.slots.into_iter().enumerate() {
            for kind in processors {
                self.add_to_slot(slot, kind);
            }
        }

        for (phase, kind) in other.phases.into_iter().enumerate() {
            if let Some(kind) = kind {
                self.phases[phase] = Some(kind);
            }
        }
    }

    fn into_slots(mut self) -> Vec<ProcessorSlot> {
        let mut out = Vec::new();

        for phase in LayeredPhase::ALL {
            let phase_index = phase.ordinal();
            push_processors(&mut out, &mut self.slots[phase_index]);

            if let Some(kind) = self.phases[phase_index] {
                out.push(ProcessorSlot {
                    phase: Some(phase),
                    kind,
                });
            }
        }

        push_processors(&mut out, &mut self.slots[LayeredPhase::ALL.len()]);
        out
    }
}

impl Default for Config {
    fn default() -> Self {
        Self {
            slots: std::array::from_fn(|_| Vec::new()),
            phases: [None; 5],
        }
    }
}

fn push_processors(out: &mut Vec<ProcessorSlot>, processors: &mut [ProcessorKind]) {
    processors.sort_by_key(|kind| intermediate_processor_order(*kind));
    out.extend(processors.iter().map(|kind| ProcessorSlot {
        phase: None,
        kind: *kind,
    }));
}

fn intermediate_processor_order(kind: ProcessorKind) -> usize {
    match kind {
        ProcessorKind::DirectionPreprocessor => 0,
        ProcessorKind::CommentPreprocessor => 1,
        ProcessorKind::EdgeAndLayerConstraintEdgeReverser => 2,
        ProcessorKind::InteractiveExternalPortPositioner => 3,
        ProcessorKind::PartitionPreprocessor => 4,
        ProcessorKind::LabelDummyInserter => 5,
        ProcessorKind::SelfLoopPreProcessor => 6,
        ProcessorKind::LayerConstraintPreprocessor => 7,
        ProcessorKind::PartitionMidprocessor => 8,
        ProcessorKind::HighDegreeNodeLayerProcessor => 9,
        ProcessorKind::NodePromotion => 10,
        ProcessorKind::LayerConstraintPostprocessor => 11,
        ProcessorKind::PartitionPostprocessor => 12,
        ProcessorKind::HierarchicalPortConstraintProcessor => 13,
        ProcessorKind::SemiInteractiveCrossMinProcessor => 14,
        ProcessorKind::BreakingPointInserter => 15,
        ProcessorKind::LongEdgeSplitter => 16,
        ProcessorKind::PortSideProcessor => 17,
        ProcessorKind::InvertedPortProcessor => 18,
        ProcessorKind::PortListSorter => 19,
        ProcessorKind::SortByInputModelProcessor => 20,
        ProcessorKind::NorthSouthPortPreprocessor => 21,
        ProcessorKind::BreakingPointProcessor => 22,
        ProcessorKind::LayerSweepCrossingMinimizerOneSidedGreedySwitch => 23,
        ProcessorKind::LayerSweepCrossingMinimizerTwoSidedGreedySwitch => 24,
        ProcessorKind::SelfLoopPortRestorer => 25,
        ProcessorKind::SingleEdgeGraphWrapper => 26,
        ProcessorKind::InLayerConstraintProcessor => 27,
        ProcessorKind::EndNodePortLabelManagementProcessor => 28,
        ProcessorKind::LabelAndNodeSizeProcessor => 29,
        ProcessorKind::InnermostNodeMarginCalculator => 30,
        ProcessorKind::SelfLoopRouter => 31,
        ProcessorKind::CommentNodeMarginCalculator => 32,
        ProcessorKind::EndLabelPreprocessor => 33,
        ProcessorKind::LabelDummySwitcher => 34,
        ProcessorKind::CenterLabelManagementProcessor => 35,
        ProcessorKind::LabelSideSelector => 36,
        ProcessorKind::HyperedgeDummyMerger => 37,
        ProcessorKind::HierarchicalPortDummySizeProcessor => 38,
        ProcessorKind::LayerSizeAndGraphHeightCalculator => 39,
        ProcessorKind::HierarchicalPortPositionProcessor => 40,
        ProcessorKind::ConstraintsPostprocessor => 41,
        ProcessorKind::CommentPostprocessor => 42,
        ProcessorKind::HypernodesProcessor => 43,
        ProcessorKind::HierarchicalPortOrthogonalEdgeRouter => 44,
        ProcessorKind::LongEdgeJoiner => 45,
        ProcessorKind::SelfLoopPostProcessor => 46,
        ProcessorKind::BreakingPointRemover => 47,
        ProcessorKind::NorthSouthPortPostprocessor => 48,
        ProcessorKind::HorizontalGraphCompactor => 49,
        ProcessorKind::LabelDummyRemover => 50,
        ProcessorKind::FinalSplineBendpointsCalculator => 51,
        ProcessorKind::EndLabelSorter => 52,
        ProcessorKind::ReversedEdgeRestorer => 53,
        ProcessorKind::EndLabelPostprocessor => 54,
        ProcessorKind::HierarchicalNodeResizer => 55,
        ProcessorKind::DirectionPostprocessor => 56,
        ProcessorKind::GreedyCycleBreaker
        | ProcessorKind::DepthFirstCycleBreaker
        | ProcessorKind::InteractiveCycleBreaker
        | ProcessorKind::ModelOrderCycleBreaker
        | ProcessorKind::GreedyModelOrderCycleBreaker
        | ProcessorKind::NetworkSimplexLayerer
        | ProcessorKind::LongestPathLayerer
        | ProcessorKind::LongestPathSourceLayerer
        | ProcessorKind::CoffmanGrahamLayerer
        | ProcessorKind::InteractiveLayerer
        | ProcessorKind::StretchWidthLayerer
        | ProcessorKind::MinWidthLayerer
        | ProcessorKind::BreadthFirstModelOrderLayerer
        | ProcessorKind::DepthFirstModelOrderLayerer
        | ProcessorKind::LayerSweepCrossingMinimizerBarycenter
        | ProcessorKind::InteractiveCrossingMinimizer
        | ProcessorKind::NoCrossingMinimizer
        | ProcessorKind::BKNodePlacer
        | ProcessorKind::SimpleNodePlacer
        | ProcessorKind::InteractiveNodePlacer
        | ProcessorKind::LinearSegmentsNodePlacer
        | ProcessorKind::NetworkSimplexPlacer
        | ProcessorKind::OrthogonalEdgeRouter
        | ProcessorKind::PolylineEdgeRouter
        | ProcessorKind::SplineEdgeRouter => usize::MAX,
    }
}

/// Assemble the layered processor list for a graph.
///
/// This follows `GraphConfigurator.prepareGraphForLayout(...)` and the selected phases'
/// `getLayoutProcessorConfiguration(...)` methods. It intentionally returns processor kinds rather
/// than executing them so each Java phase can be ported independently.
pub fn assemble_processors(options: &LayeredOptions) -> Vec<ProcessorSlot> {
    assemble_processors_with_graph_size(options, 0, true)
}

pub fn assemble_processors_for_graph(graph: &LGraph) -> Vec<ProcessorSlot> {
    let options = configured_options(graph);
    assemble_processors_with_graph_size(
        &options,
        graph.layerless_nodes.len(),
        graph.parent_node_id.is_none(),
    )
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
    let mut executed = Vec::new();
    configure_graph_properties(graph);
    let processors = assemble_processors_for_graph(graph);

    for slot in processors {
        execute_processor(graph, slot.kind)?;
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
    let mut executed = Vec::new();
    configure_graph_properties(graph);
    let processors = assemble_processors_for_graph(graph);

    for slot in processors {
        execute_processor(graph, slot.kind)?;
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
/// processor list as the phase-limited runner, fails at the first unsupported processor, and leaves
/// the graph in the post-processor state produced by the ported pipeline.
pub fn execute_ported_processors(graph: &mut LGraph) -> PipelineResult<Vec<ProcessorKind>> {
    let mut executed = Vec::new();
    configure_graph_properties(graph);
    let processors = assemble_processors_for_graph(graph);

    for slot in processors {
        execute_processor(graph, slot.kind)?;
        executed.push(slot.kind);
    }

    Ok(executed)
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
    execute_ported_compound_processors_to(graph, None)
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
    execute_ported_compound_processors_to(graph, Some(PipelineStop::Phase(target)))
}

/// Execute the source-backed compound pipeline until the requested processor completes.
///
/// For hierarchy-aware processors, only the root graph executes the processor in ELK's schedule;
/// child graph algorithms pause at that processor and resume after the root has run it.
pub fn execute_ported_compound_processors_until_processor(
    graph: &mut LGraph,
    target: ProcessorKind,
) -> PipelineResult<Vec<GraphExecution>> {
    execute_ported_compound_processors_to(graph, Some(PipelineStop::Processor(target)))
}

fn execute_ported_compound_processors_to(
    graph: &mut LGraph,
    stop: Option<PipelineStop>,
) -> PipelineResult<Vec<GraphExecution>> {
    preprocess_source_ported_compound_graph(graph);
    configure_graph_properties(graph);
    reject_unsupported_compound_graph(graph)?;
    review_and_correct_hierarchical_processors(graph)?;

    let paths = collect_graph_paths_bottom_up(graph);
    let root_index = paths.len().saturating_sub(1);
    let mut algorithms = paths
        .into_iter()
        .map(|path| {
            let current = graph_at_path(graph, &path);
            GraphAlgorithm {
                path,
                next_processor: 0,
                processors: assemble_processors_for_graph(current),
                execution: GraphExecution {
                    graph_id: current.id.clone(),
                    parent_node_id: current.parent_node_id.clone(),
                    processors: Vec::new(),
                },
            }
        })
        .collect::<Vec<_>>();

    while algorithms[root_index].next_processor < algorithms[root_index].processors.len()
        && stop
            .map(|stop| !compound_algorithms_reached_stop(&algorithms, root_index, stop))
            .unwrap_or(true)
    {
        for index in 0..algorithms.len() {
            execute_compound_algorithm_until_pause(
                graph,
                &mut algorithms[index],
                index == root_index,
                stop,
            )?;
        }
    }

    Ok(algorithms
        .into_iter()
        .map(|algorithm| algorithm.execution)
        .collect())
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
    stop: Option<PipelineStop>,
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
            execute_hierarchy_aware_processor(graph, kind)?;
            actual_graph_size(graph_at_path(graph, &algorithm.path))
        } else {
            let current = graph_mut_at_path(graph, &algorithm.path);
            execute_processor(current, kind)?;
            actual_graph_size(current)
        };
        if kind == ProcessorKind::HierarchicalNodeResizer {
            transfer_nested_graph_layout_to_parent_node(graph, &algorithm.path, size);
        }
        algorithm.execution.processors.push(kind);

        if hierarchy_aware || stop.is_some_and(|stop| slot_matches_stop(slot, stop)) {
            break;
        }
    }

    Ok(())
}

fn execute_hierarchy_aware_processor(
    graph: &mut LGraph,
    kind: ProcessorKind,
) -> PipelineResult<()> {
    match kind {
        ProcessorKind::LayerSweepCrossingMinimizerBarycenter => {
            minimize_crossings_layer_sweep_hierarchical_with_type(graph, CrossMinType::Barycenter);
        }
        ProcessorKind::LayerSweepCrossingMinimizerOneSidedGreedySwitch => {
            minimize_crossings_layer_sweep_hierarchical_with_type(
                graph,
                CrossMinType::OneSidedGreedySwitch,
            );
        }
        ProcessorKind::LayerSweepCrossingMinimizerTwoSidedGreedySwitch => {
            minimize_crossings_layer_sweep_hierarchical_with_type(
                graph,
                CrossMinType::TwoSidedGreedySwitch,
            );
        }
        _ => return Err(PipelineError::UnsupportedProcessor { kind }),
    }
    Ok(())
}

fn reject_unsupported_compound_graph(graph: &LGraph) -> PipelineResult<()> {
    for node in &graph.layerless_nodes {
        if let Some(nested_graph) = node.nested_graph.as_deref() {
            reject_unsupported_compound_graph(nested_graph)?;
        }
    }

    Ok(())
}

fn review_and_correct_hierarchical_processors(root: &mut LGraph) -> PipelineResult<()> {
    let root_crossing = root.options.crossing_minimization_strategy;
    let root_greedy_switch = root.options.greedy_switch_hierarchical_type;
    review_nested_hierarchical_processors(root, root_crossing, root_greedy_switch)
}

fn review_nested_hierarchical_processors(
    graph: &mut LGraph,
    root_crossing: CrossingMinimizationStrategy,
    root_greedy_switch: GreedySwitchType,
) -> PipelineResult<()> {
    if graph.options.crossing_minimization_strategy != root_crossing {
        return Err(PipelineError::UnsupportedCompoundGraph {
            reason: "child graphs must use the root hierarchy-aware crossing minimizer",
        });
    }
    graph.options.greedy_switch_hierarchical_type = root_greedy_switch;

    for node in &mut graph.layerless_nodes {
        if let Some(nested_graph) = node.nested_graph.as_deref_mut() {
            review_nested_hierarchical_processors(nested_graph, root_crossing, root_greedy_switch)?;
        }
    }

    Ok(())
}

fn collect_graph_paths_bottom_up(graph: &LGraph) -> Vec<Vec<usize>> {
    let mut paths = vec![Vec::new()];
    let mut search = vec![Vec::new()];

    while let Some(path) = search.pop() {
        let current = graph_at_path(graph, &path);
        for (index, node) in current.layerless_nodes.iter().enumerate() {
            if node.nested_graph.is_some() {
                let mut child_path = path.clone();
                child_path.push(index);
                paths.insert(0, child_path.clone());
                search.push(child_path);
            }
        }
    }

    paths
}

fn graph_at_path<'a>(mut graph: &'a LGraph, path: &[usize]) -> &'a LGraph {
    for index in path {
        graph = graph.layerless_nodes[*index]
            .nested_graph
            .as_deref()
            .expect("graph path should only contain nested graph nodes");
    }
    graph
}

fn graph_mut_at_path<'a>(mut graph: &'a mut LGraph, path: &[usize]) -> &'a mut LGraph {
    for index in path {
        graph = graph.layerless_nodes[*index]
            .nested_graph
            .as_deref_mut()
            .expect("graph path should only contain nested graph nodes");
    }
    graph
}

fn transfer_nested_graph_layout_to_parent_node(graph: &mut LGraph, path: &[usize], size: LSize) {
    let Some((node_index, parent_path)) = path.split_last() else {
        return;
    };
    let parent = graph_mut_at_path(graph, parent_path);
    let has_external_ports = {
        let node = &mut parent.layerless_nodes[*node_index];
        let Some(nested_graph) = node.nested_graph.as_mut() else {
            return;
        };
        transfer_external_port_dummy_layout_to_parent_node(
            nested_graph,
            *node_index,
            &mut node.ports,
        );
        nested_graph.graph_properties.external_ports
    };

    {
        let node = &mut parent.layerless_nodes[*node_index];
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
    parent_node_index: usize,
    parent_ports: &mut [crate::graph::LPort],
) {
    for dummy_index in 0..nested_graph.layerless_nodes.len() {
        if nested_graph.layerless_nodes[dummy_index].kind != LNodeKind::ExternalPort {
            continue;
        }
        let Some(origin_port) = nested_graph.layerless_nodes[dummy_index]
            .origin_port
            .clone()
        else {
            continue;
        };
        if origin_port.port.node != parent_node_index {
            continue;
        }

        let port_position = external_port_position(nested_graph, dummy_index);
        let external_side = nested_graph.layerless_nodes[dummy_index].external_port_side;
        if let Some(parent_port) = parent_ports.get_mut(origin_port.port.port) {
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

fn execute_processor(graph: &mut LGraph, kind: ProcessorKind) -> PipelineResult<()> {
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
        ProcessorKind::SelfLoopPreProcessor => preprocess_self_loops(graph),
        ProcessorKind::GreedyCycleBreaker => break_cycles_greedy(graph),
        ProcessorKind::GreedyModelOrderCycleBreaker => break_cycles_greedy_model_order(graph),
        ProcessorKind::LayerConstraintPreprocessor => preprocess_layer_constraints(graph)?,
        ProcessorKind::LabelDummyInserter => insert_label_dummies(graph),
        ProcessorKind::NetworkSimplexLayerer => layer_network_simplex(graph),
        ProcessorKind::LayerConstraintPostprocessor => postprocess_layer_constraints(graph)?,
        ProcessorKind::HierarchicalPortConstraintProcessor => {
            process_hierarchical_port_constraints(graph);
        }
        ProcessorKind::LongEdgeSplitter => split_long_edges(graph),
        ProcessorKind::PortSideProcessor => process_port_sides(graph),
        ProcessorKind::InvertedPortProcessor => process_inverted_ports(graph),
        ProcessorKind::PortListSorter => sort_port_lists(graph),
        ProcessorKind::SortByInputModelProcessor => sort_by_input_model(graph),
        ProcessorKind::LayerSweepCrossingMinimizerBarycenter => {
            minimize_crossings_layer_sweep(graph);
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
        ProcessorKind::EndLabelPreprocessor => preprocess_end_labels(graph),
        ProcessorKind::BKNodePlacer => place_nodes_brandes_koepf(graph),
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
        ProcessorKind::HierarchicalNodeResizer => resize_hierarchical_node_graph(graph),
        ProcessorKind::NoCrossingMinimizer => {}
        _ => return Err(PipelineError::UnsupportedProcessor { kind }),
    }

    Ok(())
}

fn resize_hierarchical_node_graph(graph: &mut LGraph) {
    let layered_nodes = graph
        .layers
        .iter()
        .flat_map(|layer| layer.nodes.iter().copied())
        .collect::<Vec<_>>();
    for node in layered_nodes {
        graph.layerless_nodes[node].layer_index = None;
    }
    graph.layers.clear();
    let old_size = actual_graph_size(graph);
    let new_size = LSize {
        width: old_size.width.max(0.0),
        height: old_size.height.max(0.0),
    };
    resize_graph_no_really_i_mean_it(graph, old_size, new_size);
}

fn resize_graph_no_really_i_mean_it(graph: &mut LGraph, old_size: LSize, new_size: LSize) {
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

fn assemble_processors_with_graph_size(
    options: &LayeredOptions,
    graph_size: usize,
    is_root_graph: bool,
) -> Vec<ProcessorSlot> {
    let mut config = Config::default();

    add_baseline_processors(&mut config);
    add_graph_configurator_processors(&mut config, options, graph_size, is_root_graph);

    let cycle = cycle_breaking_processor(options.cycle_breaking_strategy);
    config.merge(cycle_breaking_dependencies(cycle));
    config.add_phase(LayeredPhase::P1CycleBreaking, cycle);

    let layerer = layering_processor(options.layering_strategy);
    config.merge(layering_dependencies(layerer));
    config.add_phase(LayeredPhase::P2Layering, layerer);

    let crossing = crossing_minimization_processor(options.crossing_minimization_strategy);
    config.merge(crossing_minimization_dependencies(crossing));
    config.add_phase(LayeredPhase::P3NodeOrdering, crossing);

    let node_placer = node_placement_processor(options.node_placement_strategy);
    config.merge(node_placement_dependencies(node_placer));
    config.add_phase(LayeredPhase::P4NodePlacement, node_placer);

    let edge_router = edge_routing_processor(options.edge_routing);
    config.merge(edge_routing_dependencies(options, edge_router));
    config.add_phase(LayeredPhase::P5EdgeRouting, edge_router);

    config.into_slots()
}

fn add_baseline_processors(config: &mut Config) {
    config.add_before(
        LayeredPhase::P4NodePlacement,
        ProcessorKind::InnermostNodeMarginCalculator,
    );
    config.add_before(
        LayeredPhase::P4NodePlacement,
        ProcessorKind::LabelAndNodeSizeProcessor,
    );
    config.add_before(
        LayeredPhase::P5EdgeRouting,
        ProcessorKind::LayerSizeAndGraphHeightCalculator,
    );
    config.add_after(LayeredPhase::P5EdgeRouting, ProcessorKind::EndLabelSorter);
}

fn add_graph_configurator_processors(
    config: &mut Config,
    options: &LayeredOptions,
    graph_size: usize,
    is_root_graph: bool,
) {
    if options.hierarchy_handling == super::options::HierarchyHandling::IncludeChildren {
        config.add_after(
            LayeredPhase::P5EdgeRouting,
            ProcessorKind::HierarchicalNodeResizer,
        );
    }

    let port_side_phase = if options.feedback_edges {
        LayeredPhase::P1CycleBreaking
    } else {
        LayeredPhase::P3NodeOrdering
    };
    config.add_before(port_side_phase, ProcessorKind::PortSideProcessor);

    match options.direction {
        ElkDirection::Left | ElkDirection::Down | ElkDirection::Up => {
            config.add_before(
                LayeredPhase::P1CycleBreaking,
                ProcessorKind::DirectionPreprocessor,
            );
            config.add_after(
                LayeredPhase::P5EdgeRouting,
                ProcessorKind::DirectionPostprocessor,
            );
        }
        ElkDirection::Right | ElkDirection::Undefined => {}
    }

    if activate_greedy_switch_for(options, graph_size, is_root_graph) {
        let kind = if options.is_hierarchical_layout() {
            match options.greedy_switch_hierarchical_type {
                GreedySwitchType::OneSided => {
                    ProcessorKind::LayerSweepCrossingMinimizerOneSidedGreedySwitch
                }
                GreedySwitchType::TwoSided => {
                    ProcessorKind::LayerSweepCrossingMinimizerTwoSidedGreedySwitch
                }
                GreedySwitchType::Off => unreachable!("checked by activate_greedy_switch_for"),
            }
        } else {
            match options.greedy_switch_type {
                GreedySwitchType::OneSided => {
                    ProcessorKind::LayerSweepCrossingMinimizerOneSidedGreedySwitch
                }
                GreedySwitchType::TwoSided => {
                    ProcessorKind::LayerSweepCrossingMinimizerTwoSidedGreedySwitch
                }
                GreedySwitchType::Off => unreachable!("checked by activate_greedy_switch_for"),
            }
        };
        config.add_before(LayeredPhase::P4NodePlacement, kind);
    }

    match options.wrapping_strategy {
        WrappingStrategy::SingleEdge => {
            config.add_before(
                LayeredPhase::P4NodePlacement,
                ProcessorKind::SingleEdgeGraphWrapper,
            );
        }
        WrappingStrategy::MultiEdge => {
            config.add_before(
                LayeredPhase::P3NodeOrdering,
                ProcessorKind::BreakingPointInserter,
            );
            config.add_before(
                LayeredPhase::P4NodePlacement,
                ProcessorKind::BreakingPointProcessor,
            );
            config.add_after(
                LayeredPhase::P5EdgeRouting,
                ProcessorKind::BreakingPointRemover,
            );
        }
        WrappingStrategy::Off => {}
    }

    if options.consider_model_order_strategy != OrderingStrategy::None {
        config.add_before(
            LayeredPhase::P3NodeOrdering,
            ProcessorKind::SortByInputModelProcessor,
        );
    }
}

fn activate_greedy_switch_for(
    options: &LayeredOptions,
    graph_size: usize,
    is_root_graph: bool,
) -> bool {
    if options.is_hierarchical_layout() {
        return is_root_graph && options.greedy_switch_hierarchical_type != GreedySwitchType::Off;
    }

    let interactive_cross_min =
        options.crossing_minimization_strategy == CrossingMinimizationStrategy::Interactive;
    !interactive_cross_min
        && options.greedy_switch_type != GreedySwitchType::Off
        && (options.greedy_switch_activation_threshold == 0
            || options.greedy_switch_activation_threshold > graph_size)
}

fn cycle_breaking_processor(strategy: CycleBreakingStrategy) -> ProcessorKind {
    match strategy {
        CycleBreakingStrategy::Greedy => ProcessorKind::GreedyCycleBreaker,
        CycleBreakingStrategy::DepthFirst => ProcessorKind::DepthFirstCycleBreaker,
        CycleBreakingStrategy::Interactive => ProcessorKind::InteractiveCycleBreaker,
        CycleBreakingStrategy::ModelOrder => ProcessorKind::ModelOrderCycleBreaker,
        CycleBreakingStrategy::GreedyModelOrder => ProcessorKind::GreedyModelOrderCycleBreaker,
    }
}

fn cycle_breaking_dependencies(_processor: ProcessorKind) -> Config {
    let mut config = Config::default();
    config.add_after(
        LayeredPhase::P5EdgeRouting,
        ProcessorKind::ReversedEdgeRestorer,
    );
    config
}

fn layering_processor(strategy: LayeringStrategy) -> ProcessorKind {
    match strategy {
        LayeringStrategy::NetworkSimplex => ProcessorKind::NetworkSimplexLayerer,
        LayeringStrategy::LongestPath => ProcessorKind::LongestPathLayerer,
        LayeringStrategy::LongestPathSource => ProcessorKind::LongestPathSourceLayerer,
        LayeringStrategy::CoffmanGraham => ProcessorKind::CoffmanGrahamLayerer,
        LayeringStrategy::Interactive => ProcessorKind::InteractiveLayerer,
        LayeringStrategy::StretchWidth => ProcessorKind::StretchWidthLayerer,
        LayeringStrategy::MinWidth => ProcessorKind::MinWidthLayerer,
        LayeringStrategy::BreadthFirstModelOrder => ProcessorKind::BreadthFirstModelOrderLayerer,
        LayeringStrategy::DepthFirstModelOrder => ProcessorKind::DepthFirstModelOrderLayerer,
    }
}

fn layering_dependencies(_processor: ProcessorKind) -> Config {
    let mut config = Config::default();
    config.add_before(
        LayeredPhase::P1CycleBreaking,
        ProcessorKind::EdgeAndLayerConstraintEdgeReverser,
    );
    config.add_before(
        LayeredPhase::P2Layering,
        ProcessorKind::LayerConstraintPreprocessor,
    );
    config.add_before(
        LayeredPhase::P3NodeOrdering,
        ProcessorKind::LayerConstraintPostprocessor,
    );
    config
}

fn crossing_minimization_processor(strategy: CrossingMinimizationStrategy) -> ProcessorKind {
    match strategy {
        CrossingMinimizationStrategy::LayerSweep => {
            ProcessorKind::LayerSweepCrossingMinimizerBarycenter
        }
        CrossingMinimizationStrategy::Interactive => ProcessorKind::InteractiveCrossingMinimizer,
        CrossingMinimizationStrategy::None => ProcessorKind::NoCrossingMinimizer,
    }
}

fn crossing_minimization_dependencies(_processor: ProcessorKind) -> Config {
    let mut config = Config::default();
    config.add_before(
        LayeredPhase::P3NodeOrdering,
        ProcessorKind::LongEdgeSplitter,
    );
    config.add_before(LayeredPhase::P3NodeOrdering, ProcessorKind::PortListSorter);
    config.add_before(
        LayeredPhase::P4NodePlacement,
        ProcessorKind::InLayerConstraintProcessor,
    );
    config.add_after(LayeredPhase::P5EdgeRouting, ProcessorKind::LongEdgeJoiner);
    config
}

fn node_placement_processor(strategy: NodePlacementStrategy) -> ProcessorKind {
    match strategy {
        NodePlacementStrategy::Simple => ProcessorKind::SimpleNodePlacer,
        NodePlacementStrategy::Interactive => ProcessorKind::InteractiveNodePlacer,
        NodePlacementStrategy::LinearSegments => ProcessorKind::LinearSegmentsNodePlacer,
        NodePlacementStrategy::BrandesKoepf => ProcessorKind::BKNodePlacer,
        NodePlacementStrategy::NetworkSimplex => ProcessorKind::NetworkSimplexPlacer,
    }
}

fn node_placement_dependencies(_processor: ProcessorKind) -> Config {
    Config::default()
}

fn edge_routing_processor(strategy: EdgeRouting) -> ProcessorKind {
    match strategy {
        EdgeRouting::Polyline => ProcessorKind::PolylineEdgeRouter,
        EdgeRouting::Orthogonal => ProcessorKind::OrthogonalEdgeRouter,
        EdgeRouting::Splines => ProcessorKind::SplineEdgeRouter,
    }
}

fn edge_routing_dependencies(options: &LayeredOptions, _processor: ProcessorKind) -> Config {
    let mut config = Config::default();
    if options.graph_has_hyperedges {
        config.add_before(
            LayeredPhase::P4NodePlacement,
            ProcessorKind::HyperedgeDummyMerger,
        );
        config.add_before(
            LayeredPhase::P3NodeOrdering,
            ProcessorKind::InvertedPortProcessor,
        );
    }
    if options.graph_has_non_free_ports || options.feedback_edges {
        config.add_before(
            LayeredPhase::P3NodeOrdering,
            ProcessorKind::InvertedPortProcessor,
        );
        if options.graph_has_north_south_ports {
            config.add_before(
                LayeredPhase::P3NodeOrdering,
                ProcessorKind::NorthSouthPortPreprocessor,
            );
            config.add_after(
                LayeredPhase::P5EdgeRouting,
                ProcessorKind::NorthSouthPortPostprocessor,
            );
        }
    }
    if options.graph_has_external_ports {
        config.add_before(
            LayeredPhase::P3NodeOrdering,
            ProcessorKind::HierarchicalPortConstraintProcessor,
        );
        config.add_before(
            LayeredPhase::P4NodePlacement,
            ProcessorKind::HierarchicalPortDummySizeProcessor,
        );
        config.add_before(
            LayeredPhase::P5EdgeRouting,
            ProcessorKind::HierarchicalPortPositionProcessor,
        );
        config.add_after(
            LayeredPhase::P5EdgeRouting,
            ProcessorKind::HierarchicalPortOrthogonalEdgeRouter,
        );
    }
    if options.graph_has_self_loops {
        config.add_before(
            LayeredPhase::P1CycleBreaking,
            ProcessorKind::SelfLoopPreProcessor,
        );
        config.add_before(
            LayeredPhase::P4NodePlacement,
            ProcessorKind::SelfLoopPortRestorer,
        );
        config.add_before(LayeredPhase::P4NodePlacement, ProcessorKind::SelfLoopRouter);
        config.add_after(
            LayeredPhase::P5EdgeRouting,
            ProcessorKind::SelfLoopPostProcessor,
        );
    }
    if options.graph_has_hypernodes {
        config.add_after(
            LayeredPhase::P5EdgeRouting,
            ProcessorKind::HypernodesProcessor,
        );
    }
    if options.graph_has_center_labels {
        config.add_before(LayeredPhase::P2Layering, ProcessorKind::LabelDummyInserter);
        config.add_before(
            LayeredPhase::P4NodePlacement,
            ProcessorKind::LabelDummySwitcher,
        );
        config.add_before(
            LayeredPhase::P4NodePlacement,
            ProcessorKind::LabelSideSelector,
        );
        config.add_after(
            LayeredPhase::P5EdgeRouting,
            ProcessorKind::LabelDummyRemover,
        );
    }
    if options.graph_has_end_labels {
        config.add_before(
            LayeredPhase::P4NodePlacement,
            ProcessorKind::LabelSideSelector,
        );
        config.add_before(
            LayeredPhase::P4NodePlacement,
            ProcessorKind::EndLabelPreprocessor,
        );
        config.add_after(
            LayeredPhase::P5EdgeRouting,
            ProcessorKind::EndLabelPostprocessor,
        );
    }
    config
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::graph::{LNode, LNodeKind, PortSide, PortType};
    use crate::importer::{ElkInputEdge, ElkInputGraph, ElkInputLabel, ElkInputNode, import_graph};
    use crate::options::{ElkDirection, GreedySwitchType, LayeredOptions, PortConstraints};
    use crate::p3order::{counting::CrossingsCounter, process_port_sides, sort_port_lists};

    fn kinds(options: &LayeredOptions) -> Vec<ProcessorKind> {
        assemble_processors(options)
            .into_iter()
            .map(|slot| slot.kind)
            .collect()
    }

    fn graph_kinds(graph: &LGraph) -> Vec<ProcessorKind> {
        assemble_processors_for_graph(graph)
            .into_iter()
            .map(|slot| slot.kind)
            .collect()
    }

    fn node(id: &str) -> ElkInputNode {
        ElkInputNode {
            id: id.to_string(),
            width: 80.0,
            height: 40.0,
            parent: None,
            direction: None,
            hierarchy_handling: None,
            layer_constraint: None,
            port_constraints: None,
            node_label_placement: crate::options::NodeLabelPlacement::Fixed,
            nested_spacing_base: None,
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
            inside_self_loops_yo: false,
            priority_direction: 0,
            priority_shortness: 0,
            priority_straightness: 0,
        }
    }

    fn p3_options() -> LayeredOptions {
        LayeredOptions {
            direction: ElkDirection::Right,
            greedy_switch_type: GreedySwitchType::Off,
            ..LayeredOptions::default()
        }
    }

    #[test]
    fn layered_baseline_processor_sequence_matches_elkjs_0_9_3_logging() {
        assert_eq!(
            kinds(&LayeredOptions::default()),
            vec![
                ProcessorKind::EdgeAndLayerConstraintEdgeReverser,
                ProcessorKind::GreedyCycleBreaker,
                ProcessorKind::LayerConstraintPreprocessor,
                ProcessorKind::NetworkSimplexLayerer,
                ProcessorKind::LayerConstraintPostprocessor,
                ProcessorKind::LongEdgeSplitter,
                ProcessorKind::PortSideProcessor,
                ProcessorKind::PortListSorter,
                ProcessorKind::LayerSweepCrossingMinimizerBarycenter,
                ProcessorKind::LayerSweepCrossingMinimizerTwoSidedGreedySwitch,
                ProcessorKind::InLayerConstraintProcessor,
                ProcessorKind::LabelAndNodeSizeProcessor,
                ProcessorKind::InnermostNodeMarginCalculator,
                ProcessorKind::BKNodePlacer,
                ProcessorKind::LayerSizeAndGraphHeightCalculator,
                ProcessorKind::OrthogonalEdgeRouter,
                ProcessorKind::LongEdgeJoiner,
                ProcessorKind::EndLabelSorter,
                ProcessorKind::ReversedEdgeRestorer,
            ]
        );
    }

    #[test]
    fn mermaid_flowchart_defaults_insert_direction_model_order_and_hierarchy_processors() {
        let options = LayeredOptions::mermaid_flowchart_defaults(ElkDirection::Down);
        assert_eq!(
            kinds(&options),
            vec![
                ProcessorKind::DirectionPreprocessor,
                ProcessorKind::EdgeAndLayerConstraintEdgeReverser,
                ProcessorKind::GreedyCycleBreaker,
                ProcessorKind::LayerConstraintPreprocessor,
                ProcessorKind::NetworkSimplexLayerer,
                ProcessorKind::LayerConstraintPostprocessor,
                ProcessorKind::LongEdgeSplitter,
                ProcessorKind::PortSideProcessor,
                ProcessorKind::PortListSorter,
                ProcessorKind::SortByInputModelProcessor,
                ProcessorKind::LayerSweepCrossingMinimizerBarycenter,
                ProcessorKind::InLayerConstraintProcessor,
                ProcessorKind::LabelAndNodeSizeProcessor,
                ProcessorKind::InnermostNodeMarginCalculator,
                ProcessorKind::BKNodePlacer,
                ProcessorKind::LayerSizeAndGraphHeightCalculator,
                ProcessorKind::OrthogonalEdgeRouter,
                ProcessorKind::LongEdgeJoiner,
                ProcessorKind::EndLabelSorter,
                ProcessorKind::ReversedEdgeRestorer,
                ProcessorKind::HierarchicalNodeResizer,
                ProcessorKind::DirectionPostprocessor,
            ]
        );
    }

    #[test]
    fn mermaid_wrapping_flags_do_not_enable_wrapping_processors_without_strategy() {
        let options = LayeredOptions::mermaid_flowchart_defaults(ElkDirection::Down);
        let processors = kinds(&options);
        assert!(!processors.contains(&ProcessorKind::BreakingPointInserter));
        assert!(!processors.contains(&ProcessorKind::BreakingPointProcessor));
        assert!(!processors.contains(&ProcessorKind::BreakingPointRemover));
    }

    #[test]
    fn greedy_model_order_cycle_breaking_strategy_assembles_processor() {
        let options = LayeredOptions {
            cycle_breaking_strategy: CycleBreakingStrategy::GreedyModelOrder,
            ..LayeredOptions::mermaid_flowchart_defaults(ElkDirection::Down)
        };
        assert!(kinds(&options).contains(&ProcessorKind::GreedyModelOrderCycleBreaker));
    }

    #[test]
    fn graph_properties_insert_label_self_loop_and_external_port_processors() {
        let mut graph = import_graph(&ElkInputGraph {
            id: "root".to_string(),
            options: LayeredOptions::mermaid_flowchart_defaults(ElkDirection::Down),
            nodes: vec![
                ElkInputNode {
                    id: "cluster".to_string(),
                    width: 0.0,
                    height: 0.0,
                    parent: None,
                    direction: None,
                    hierarchy_handling: Some(crate::options::HierarchyHandling::IncludeChildren),
                    layer_constraint: None,
                    port_constraints: None,
                    node_label_placement: crate::options::NodeLabelPlacement::Fixed,
                    nested_spacing_base: None,
                    label: None,
                },
                ElkInputNode {
                    id: "A".to_string(),
                    width: 80.0,
                    height: 40.0,
                    parent: Some("cluster".to_string()),
                    direction: None,
                    hierarchy_handling: None,
                    layer_constraint: None,
                    port_constraints: None,
                    node_label_placement: crate::options::NodeLabelPlacement::Fixed,
                    nested_spacing_base: None,
                    label: None,
                },
            ],
            edges: vec![
                ElkInputEdge {
                    id: "cluster-A".to_string(),
                    source: "cluster".to_string(),
                    target: "A".to_string(),
                    label: Some(ElkInputLabel::center("inside", 24.0, 12.0)),
                    minlen: 1,
                    inside_self_loops_yo: false,
                    priority_direction: 0,
                    priority_shortness: 0,
                    priority_straightness: 0,
                },
                ElkInputEdge {
                    id: "A-A".to_string(),
                    source: "A".to_string(),
                    target: "A".to_string(),
                    label: None,
                    minlen: 1,
                    inside_self_loops_yo: false,
                    priority_direction: 0,
                    priority_shortness: 0,
                    priority_straightness: 0,
                },
            ],
        })
        .unwrap();
        preprocess_source_ported_compound_graph(&mut graph);

        let nested = graph.layerless_nodes[0].nested_graph.as_ref().unwrap();
        let processors = graph_kinds(nested);
        assert!(processors.contains(&ProcessorKind::HierarchicalPortConstraintProcessor));
        assert!(processors.contains(&ProcessorKind::HierarchicalPortDummySizeProcessor));
        assert!(processors.contains(&ProcessorKind::HierarchicalPortPositionProcessor));
        assert!(processors.contains(&ProcessorKind::HierarchicalPortOrthogonalEdgeRouter));
        assert!(processors.contains(&ProcessorKind::LabelDummyInserter));
        assert!(processors.contains(&ProcessorKind::LabelDummySwitcher));
        assert!(processors.contains(&ProcessorKind::LabelDummyRemover));
        assert!(processors.contains(&ProcessorKind::SelfLoopPreProcessor));
        assert!(processors.contains(&ProcessorKind::SelfLoopRouter));
        assert!(processors.contains(&ProcessorKind::SelfLoopPostProcessor));
    }

    #[test]
    fn hierarchical_port_orthogonal_router_runs_for_east_west_external_ports() {
        let mut graph = LGraph::new(
            "root",
            LayeredOptions {
                port_constraints: PortConstraints::FixedPos,
                ..LayeredOptions::default()
            },
        );
        let port = push_test_external_dummy(&mut graph, "west", PortSide::West);
        graph.layerless_nodes[port].port_ratio_or_position = 20.0;
        graph.set_node_layer(port, 0);

        execute_processor(
            &mut graph,
            ProcessorKind::HierarchicalPortOrthogonalEdgeRouter,
        )
        .unwrap();

        assert_eq!(graph.layerless_nodes[port].position.y, 20.0);
    }

    #[test]
    fn hierarchical_port_orthogonal_router_runs_for_north_south_external_ports() {
        let mut graph = LGraph::new("root", LayeredOptions::default());
        let port = push_test_external_dummy(&mut graph, "north", PortSide::North);
        graph.set_node_layer(port, 0);

        execute_processor(
            &mut graph,
            ProcessorKind::HierarchicalPortOrthogonalEdgeRouter,
        )
        .unwrap();

        assert_eq!(graph.layerless_nodes[port].position.y, 0.0);
    }

    #[test]
    fn execute_processors_until_p3_runs_source_ported_processor_sequence() {
        let mut graph = import_graph(&ElkInputGraph {
            id: "root".to_string(),
            options: p3_options(),
            nodes: vec![node("A"), node("B"), node("C")],
            edges: vec![edge("A-B", "A", "B"), edge("B-C", "B", "C")],
        })
        .unwrap();

        let executed = execute_processors_until(&mut graph, LayeredPhase::P3NodeOrdering).unwrap();

        assert_eq!(
            executed,
            vec![
                ProcessorKind::EdgeAndLayerConstraintEdgeReverser,
                ProcessorKind::GreedyCycleBreaker,
                ProcessorKind::LayerConstraintPreprocessor,
                ProcessorKind::NetworkSimplexLayerer,
                ProcessorKind::LayerConstraintPostprocessor,
                ProcessorKind::LongEdgeSplitter,
                ProcessorKind::PortSideProcessor,
                ProcessorKind::PortListSorter,
                ProcessorKind::LayerSweepCrossingMinimizerBarycenter,
            ]
        );
        assert_eq!(graph.layers.len(), 3);
        assert_eq!(
            graph
                .layerless_nodes
                .iter()
                .filter(|node| node.hidden)
                .count(),
            0
        );
    }

    #[test]
    fn execute_processors_until_p3_minimizes_crossings() {
        let input = ElkInputGraph {
            id: "root".to_string(),
            options: p3_options(),
            nodes: vec![node("Top"), node("Bottom"), node("Left"), node("Right")],
            edges: vec![
                edge("Top-Right", "Top", "Right"),
                edge("Bottom-Left", "Bottom", "Left"),
            ],
        };
        let mut before_graph = import_graph(&input).unwrap();

        layer_network_simplex(&mut before_graph);
        process_port_sides(&mut before_graph);
        sort_port_lists(&mut before_graph);
        let top = before_graph
            .layerless_nodes
            .iter()
            .position(|node| node.id == "Top")
            .unwrap();
        let bottom = before_graph
            .layerless_nodes
            .iter()
            .position(|node| node.id == "Bottom")
            .unwrap();
        let left = before_graph
            .layerless_nodes
            .iter()
            .position(|node| node.id == "Left")
            .unwrap();
        let right = before_graph
            .layerless_nodes
            .iter()
            .position(|node| node.id == "Right")
            .unwrap();
        before_graph.layers[0].nodes = vec![top, bottom];
        before_graph.layers[1].nodes = vec![left, right];
        let before = CrossingsCounter::new().count_all_crossings(&before_graph);

        let mut graph = import_graph(&input).unwrap();
        execute_processors_until(&mut graph, LayeredPhase::P3NodeOrdering).unwrap();

        let after = CrossingsCounter::new().count_all_crossings(&graph);
        assert_eq!(before, 1);
        assert_eq!(after, 0);
    }

    #[test]
    fn execute_processors_until_p4_runs_two_sided_greedy_switch_processor() {
        let mut graph = import_graph(&ElkInputGraph {
            id: "root".to_string(),
            options: LayeredOptions {
                direction: ElkDirection::Right,
                greedy_switch_activation_threshold: 0,
                ..LayeredOptions::default()
            },
            nodes: vec![node("Top"), node("Bottom"), node("Left"), node("Right")],
            edges: vec![
                edge("Top-Right", "Top", "Right"),
                edge("Bottom-Left", "Bottom", "Left"),
            ],
        })
        .unwrap();

        let executed = execute_processors_until(&mut graph, LayeredPhase::P4NodePlacement).unwrap();

        assert!(executed.contains(&ProcessorKind::LayerSweepCrossingMinimizerBarycenter));
        assert!(executed.contains(&ProcessorKind::LayerSweepCrossingMinimizerTwoSidedGreedySwitch));
        assert_eq!(executed.last(), Some(&ProcessorKind::BKNodePlacer));
    }

    #[test]
    fn execute_processors_until_p4_runs_one_sided_greedy_switch_processor() {
        let mut graph = import_graph(&ElkInputGraph {
            id: "root".to_string(),
            options: LayeredOptions {
                direction: ElkDirection::Right,
                greedy_switch_type: GreedySwitchType::OneSided,
                greedy_switch_activation_threshold: 0,
                ..LayeredOptions::default()
            },
            nodes: vec![node("A"), node("B"), node("C")],
            edges: vec![edge("A-B", "A", "B"), edge("B-C", "B", "C")],
        })
        .unwrap();

        let executed = execute_processors_until(&mut graph, LayeredPhase::P4NodePlacement).unwrap();

        assert!(executed.contains(&ProcessorKind::LayerSweepCrossingMinimizerOneSidedGreedySwitch));
        assert_eq!(executed.last(), Some(&ProcessorKind::BKNodePlacer));
    }

    #[test]
    fn execute_processors_until_p3_runs_mermaid_down_direction_preprocessor() {
        let mut graph = import_graph(&ElkInputGraph {
            id: "root".to_string(),
            options: LayeredOptions::mermaid_flowchart_defaults(ElkDirection::Down),
            nodes: vec![node("A"), node("B"), node("C")],
            edges: vec![edge("A-B", "A", "B"), edge("B-C", "B", "C")],
        })
        .unwrap();

        let executed = execute_processors_until(&mut graph, LayeredPhase::P3NodeOrdering).unwrap();

        assert_eq!(executed[0], ProcessorKind::DirectionPreprocessor);
        assert!(executed.contains(&ProcessorKind::SortByInputModelProcessor));
        assert_eq!(
            executed.last(),
            Some(&ProcessorKind::LayerSweepCrossingMinimizerBarycenter)
        );
        assert_eq!(graph.layers.len(), 3);
    }

    #[test]
    fn execute_processors_until_p4_runs_bk_node_placer_after_prerequisites() {
        let mut graph = import_graph(&ElkInputGraph {
            id: "root".to_string(),
            options: LayeredOptions {
                direction: ElkDirection::Right,
                greedy_switch_type: GreedySwitchType::Off,
                ..LayeredOptions::default()
            },
            nodes: vec![node("A"), node("B"), node("C")],
            edges: vec![edge("A-B", "A", "B"), edge("B-C", "B", "C")],
        })
        .unwrap();

        let executed = execute_processors_until(&mut graph, LayeredPhase::P4NodePlacement).unwrap();

        assert!(executed.contains(&ProcessorKind::LabelAndNodeSizeProcessor));
        assert!(executed.contains(&ProcessorKind::InnermostNodeMarginCalculator));
        assert_eq!(executed.last(), Some(&ProcessorKind::BKNodePlacer));
        for node in &graph.layerless_nodes {
            for port in &node.ports {
                if port.side == crate::graph::PortSide::East {
                    assert_eq!(port.position.x, node.size.width);
                }
                if port.side == crate::graph::PortSide::West {
                    assert_eq!(port.position.x, 0.0);
                }
            }
        }
        assert!(graph.layerless_nodes.iter().any(|node| {
            node.ports
                .iter()
                .any(|port| port.side != crate::graph::PortSide::Undefined)
        }));
        for layer in &graph.layers {
            let mut bottom = f64::NEG_INFINITY;
            for node in &layer.nodes {
                let lnode = &graph.layerless_nodes[*node];
                assert!(lnode.position.y - lnode.margin.top > bottom);
                bottom = lnode.position.y + lnode.size.height + lnode.margin.bottom;
            }
        }
    }

    #[test]
    fn execute_processors_until_p5_runs_orthogonal_router() {
        let mut graph = import_graph(&ElkInputGraph {
            id: "root".to_string(),
            options: LayeredOptions {
                direction: ElkDirection::Right,
                greedy_switch_type: GreedySwitchType::Off,
                ..LayeredOptions::default()
            },
            nodes: vec![node("A"), node("B")],
            edges: vec![edge("A-B", "A", "B")],
        })
        .unwrap();

        let executed = execute_processors_until(&mut graph, LayeredPhase::P5EdgeRouting).unwrap();

        assert_eq!(executed.last(), Some(&ProcessorKind::OrthogonalEdgeRouter));
        assert!(
            graph
                .layerless_nodes
                .iter()
                .all(|node| node.position.y.is_finite())
        );
        assert!(graph.size.height > 0.0);
        assert!(graph.size.width > 0.0);
        assert!(graph.layers.iter().all(|layer| layer.size.width > 0.0));
        assert!(graph.layers.iter().all(|layer| layer.size.height > 0.0));
        assert!(graph.edges.iter().all(|edge| {
            edge.bend_points
                .iter()
                .all(|point| point.x.is_finite() && point.y.is_finite())
        }));
    }

    #[test]
    fn source_ported_self_loop_runs_through_self_loop_lifecycle() {
        let mut graph = import_graph(&ElkInputGraph {
            id: "root".to_string(),
            options: LayeredOptions::mermaid_flowchart_defaults(ElkDirection::Down),
            nodes: vec![node("A")],
            edges: vec![edge("A-A", "A", "A")],
        })
        .unwrap();

        let executed = execute_ported_processors(&mut graph).unwrap();

        assert!(executed.contains(&ProcessorKind::SelfLoopPreProcessor));
        assert!(executed.contains(&ProcessorKind::SelfLoopPortRestorer));
        assert!(executed.contains(&ProcessorKind::SelfLoopRouter));
        assert!(executed.contains(&ProcessorKind::SelfLoopPostProcessor));
        assert!(graph.edge_source_attached(0));
        assert!(graph.edge_target_attached(0));
        assert_eq!(graph.edges[0].source.node, graph.edges[0].target.node);
        assert!(graph.edges[0].bend_points.len() >= 2);
        assert!(
            graph.edges[0]
                .bend_points
                .iter()
                .all(|point| point.x.is_finite() && point.y.is_finite())
        );
        assert!(graph.layerless_nodes[0].margin.top > 0.0);
    }

    #[test]
    fn assembled_processors_keep_long_edge_joiner_after_p5_router() {
        let processors = kinds(&p3_options());
        let router = processors
            .iter()
            .position(|kind| *kind == ProcessorKind::OrthogonalEdgeRouter)
            .unwrap();
        let joiner = processors
            .iter()
            .position(|kind| *kind == ProcessorKind::LongEdgeJoiner)
            .unwrap();

        assert!(joiner > router);
    }

    #[test]
    fn long_edge_joiner_processor_removes_split_dummies_after_p5() {
        let mut graph = import_graph(&ElkInputGraph {
            id: "root".to_string(),
            options: LayeredOptions {
                direction: ElkDirection::Right,
                greedy_switch_type: GreedySwitchType::Off,
                ..LayeredOptions::default()
            },
            nodes: vec![node("A"), node("B"), node("C")],
            edges: vec![
                edge("A-B", "A", "B"),
                edge("B-C", "B", "C"),
                edge("A-C", "A", "C"),
            ],
        })
        .unwrap();

        execute_processors_until(&mut graph, LayeredPhase::P5EdgeRouting).unwrap();
        assert!(
            graph
                .layerless_nodes
                .iter()
                .any(|node| node.kind == crate::graph::LNodeKind::LongEdge)
        );

        execute_processor(&mut graph, ProcessorKind::LongEdgeJoiner).unwrap();

        assert!(!graph.layers.iter().any(|layer| {
            layer
                .nodes
                .iter()
                .any(|node| graph.layerless_nodes[*node].kind == crate::graph::LNodeKind::LongEdge)
        }));
    }

    #[test]
    fn source_ported_plain_flowchart_runs_through_reversed_edge_restorer() {
        let mut graph = import_graph(&ElkInputGraph {
            id: "root".to_string(),
            options: LayeredOptions {
                direction: ElkDirection::Right,
                greedy_switch_type: GreedySwitchType::Off,
                ..LayeredOptions::default()
            },
            nodes: vec![node("A"), node("B"), node("C")],
            edges: vec![edge("A-B", "A", "B"), edge("B-C", "B", "C")],
        })
        .unwrap();

        let executed = execute_ported_processors(&mut graph).unwrap();

        assert!(executed.contains(&ProcessorKind::EndLabelSorter));
        assert!(executed.contains(&ProcessorKind::ReversedEdgeRestorer));
        assert_eq!(executed.last(), Some(&ProcessorKind::ReversedEdgeRestorer));
        assert!(graph.edges.iter().all(|edge| !edge.reversed));
    }

    #[test]
    fn source_ported_default_elk_runs_through_two_sided_greedy_switch() {
        let mut graph = import_graph(&ElkInputGraph {
            id: "root".to_string(),
            options: LayeredOptions::default(),
            nodes: vec![node("A"), node("B"), node("C")],
            edges: vec![edge("A-B", "A", "B"), edge("B-C", "B", "C")],
        })
        .unwrap();

        let executed = execute_ported_processors(&mut graph).unwrap();

        assert!(executed.contains(&ProcessorKind::LayerSweepCrossingMinimizerTwoSidedGreedySwitch));
        assert_eq!(executed.last(), Some(&ProcessorKind::ReversedEdgeRestorer));
        assert!(graph.size.height > 0.0);
        assert!(graph.size.width > 0.0);
    }

    #[test]
    fn source_ported_center_label_flowchart_runs_through_label_dummy_lifecycle() {
        let mut labelled = edge("A-C", "A", "C");
        labelled.label = Some(ElkInputLabel::center("choice", 48.0, 12.0));
        let mut graph = import_graph(&ElkInputGraph {
            id: "root".to_string(),
            options: LayeredOptions::mermaid_flowchart_defaults(ElkDirection::Down),
            nodes: vec![node("A"), node("B"), node("C")],
            edges: vec![edge("A-B", "A", "B"), edge("B-C", "B", "C"), labelled],
        })
        .unwrap();

        let executed = execute_ported_processors(&mut graph).unwrap();

        assert!(executed.contains(&ProcessorKind::LabelDummyInserter));
        assert!(executed.contains(&ProcessorKind::LabelDummySwitcher));
        assert!(executed.contains(&ProcessorKind::LabelDummyRemover));
        assert!(
            !graph
                .layers
                .iter()
                .flat_map(|layer| layer.nodes.iter().copied())
                .any(|node| graph.layerless_nodes[node].kind == crate::graph::LNodeKind::Label)
        );
        let restored = graph
            .edges
            .iter()
            .find(|edge| edge.id == "A-C" && !edge.labels.is_empty())
            .expect("center label should be restored to an A-C segment");
        assert_eq!(restored.labels[0].text, "choice");
    }

    #[test]
    fn source_ported_mermaid_defaults_run_through_hierarchical_resizer_for_flat_graph() {
        let mut graph = import_graph(&ElkInputGraph {
            id: "root".to_string(),
            options: LayeredOptions::mermaid_flowchart_defaults(ElkDirection::Down),
            nodes: vec![node("A"), node("B"), node("C")],
            edges: vec![edge("A-B", "A", "B"), edge("B-C", "B", "C")],
        })
        .unwrap();

        let executed = execute_ported_processors(&mut graph).unwrap();

        assert!(executed.contains(&ProcessorKind::HierarchicalNodeResizer));
        assert_eq!(
            executed.last(),
            Some(&ProcessorKind::DirectionPostprocessor)
        );
        assert!(graph.layers.is_empty());
        assert!(
            graph
                .layerless_nodes
                .iter()
                .all(|node| node.layer_index.is_none())
        );
        assert!(graph.size.height > 0.0);
        assert!(graph.size.width > 0.0);
    }

    #[test]
    fn source_ported_compound_runner_executes_bottom_up_and_resizes_parent_node() {
        let mut cluster = node("cluster");
        cluster.width = 1.0;
        cluster.height = 1.0;
        cluster.hierarchy_handling = Some(crate::options::HierarchyHandling::IncludeChildren);
        let mut child_a = node("A");
        child_a.parent = Some("cluster".to_string());
        let mut child_b = node("B");
        child_b.parent = Some("cluster".to_string());
        let mut graph = import_graph(&ElkInputGraph {
            id: "root".to_string(),
            options: LayeredOptions::mermaid_flowchart_defaults(ElkDirection::Down),
            nodes: vec![cluster, child_a, child_b, node("C")],
            edges: vec![edge("A-B", "A", "B")],
        })
        .unwrap();

        let executed = execute_ported_compound_processors(&mut graph).unwrap();

        assert_eq!(executed.len(), 2);
        assert_eq!(executed[0].graph_id, "cluster");
        assert_eq!(executed[1].graph_id, "root");
        assert!(
            executed[0]
                .processors
                .contains(&ProcessorKind::HierarchicalNodeResizer)
        );
        assert!(
            !executed[0]
                .processors
                .iter()
                .any(|kind| kind.is_hierarchy_aware())
        );
        assert!(
            executed[1]
                .processors
                .iter()
                .any(|kind| kind.is_hierarchy_aware())
        );
        let cluster = graph
            .layerless_nodes
            .iter()
            .find(|node| node.id == "cluster")
            .unwrap();
        let nested = cluster.nested_graph.as_ref().unwrap();
        assert!(cluster.size.width >= nested.size.width);
        assert!(cluster.size.height >= nested.size.height);
        assert!(nested.layers.is_empty());
    }

    #[test]
    fn compound_runner_finishes_child_tail_between_root_hierarchy_aware_processors() {
        let mut cluster = node("cluster");
        cluster.width = 1.0;
        cluster.height = 1.0;
        cluster.hierarchy_handling = Some(crate::options::HierarchyHandling::IncludeChildren);
        let mut child_a = node("A");
        child_a.parent = Some("cluster".to_string());
        let mut child_b = node("B");
        child_b.parent = Some("cluster".to_string());
        let mut options = LayeredOptions::mermaid_flowchart_defaults(ElkDirection::Down);
        options.greedy_switch_hierarchical_type = GreedySwitchType::TwoSided;
        let mut graph = import_graph(&ElkInputGraph {
            id: "root".to_string(),
            options,
            nodes: vec![cluster, child_a, child_b, node("C")],
            edges: vec![edge("A-B", "A", "B"), edge("cluster-C", "cluster", "C")],
        })
        .unwrap();

        let executed = execute_ported_compound_processors(&mut graph).unwrap();

        let child_execution = &executed[0].processors;
        let root_execution = &executed[1].processors;
        assert!(
            root_execution
                .iter()
                .filter(|kind| kind.is_hierarchy_aware())
                .count()
                > 1
        );
        assert!(child_execution.contains(&ProcessorKind::HierarchicalNodeResizer));
        assert_eq!(
            child_execution.last(),
            Some(&ProcessorKind::DirectionPostprocessor)
        );
        let cluster = graph
            .layerless_nodes
            .iter()
            .find(|node| node.id == "cluster")
            .unwrap();
        assert!(cluster.size.width > 1.0);
        assert!(cluster.size.height > 1.0);
        assert!(cluster.nested_graph.as_ref().unwrap().layers.is_empty());
    }

    #[test]
    fn source_ported_compound_runner_routes_cross_hierarchy_edges() {
        let mut cluster = node("cluster");
        cluster.hierarchy_handling = Some(crate::options::HierarchyHandling::IncludeChildren);
        let mut child = node("A");
        child.parent = Some("cluster".to_string());
        let mut graph = import_graph(&ElkInputGraph {
            id: "root".to_string(),
            options: LayeredOptions::mermaid_flowchart_defaults(ElkDirection::Down),
            nodes: vec![cluster, child],
            edges: vec![edge("cluster-A", "cluster", "A")],
        })
        .unwrap();

        let executed = execute_ported_compound_processors(&mut graph).unwrap();

        assert_eq!(executed.len(), 2);
        assert!(
            graph
                .layerless_nodes
                .iter()
                .filter(|node| node.kind == crate::graph::LNodeKind::ExternalPort)
                .all(|node| node.layer_index.is_none())
        );
        assert!(graph.size.width > 0.0);
        assert!(graph.size.height > 0.0);
    }

    #[test]
    fn hierarchical_resizer_moves_east_and_south_external_ports_when_graph_grows() {
        let mut graph = LGraph::new("root", LayeredOptions::default());
        graph.graph_properties.external_ports = true;
        graph.padding.left = 5.0;
        graph.padding.right = 5.0;
        graph.padding.top = 7.0;
        graph.padding.bottom = 7.0;
        graph.size = LSize {
            width: 10.0,
            height: 20.0,
        };

        let mut east_node = LNode::new("east", 0.0, 0.0, None);
        east_node.kind = LNodeKind::ExternalPort;
        east_node.external_port_side = PortSide::East;
        east_node.position.x = 12.0;
        let east = graph.layerless_nodes.len();
        graph.layerless_nodes.push(east_node);

        let mut south_node = LNode::new("south", 0.0, 0.0, None);
        south_node.kind = LNodeKind::ExternalPort;
        south_node.external_port_side = PortSide::South;
        south_node.position.y = 25.0;
        let south = graph.layerless_nodes.len();
        graph.layerless_nodes.push(south_node);

        resize_graph_no_really_i_mean_it(
            &mut graph,
            LSize {
                width: 20.0,
                height: 34.0,
            },
            LSize {
                width: 50.0,
                height: 74.0,
            },
        );

        assert_eq!(graph.layerless_nodes[east].position.x, 42.0);
        assert_eq!(graph.layerless_nodes[south].position.y, 65.0);
        assert_eq!(graph.size.width, 40.0);
        assert_eq!(graph.size.height, 60.0);
    }

    #[test]
    fn layered_node_resize_moves_ports_when_compound_has_no_external_ports() {
        let mut node = LNode::new("cluster", 100.0, 50.0, None);
        node.port_constraints = PortConstraints::Free;
        node.ports.push(crate::graph::LPort::new(
            "east".to_string(),
            0,
            crate::graph::PortType::Output,
        ));
        node.ports[0].set_side(PortSide::East);
        node.ports[0].position = LPoint { x: 100.0, y: 25.0 };
        node.ports.push(crate::graph::LPort::new(
            "south".to_string(),
            0,
            crate::graph::PortType::Output,
        ));
        node.ports[1].set_side(PortSide::South);
        node.ports[1].position = LPoint { x: 50.0, y: 50.0 };

        resize_layered_node(
            &mut node,
            LSize {
                width: 150.0,
                height: 100.0,
            },
            true,
            true,
        );

        assert_eq!(node.size.width, 150.0);
        assert_eq!(node.size.height, 100.0);
        assert_eq!(node.ports[0].position, LPoint { x: 150.0, y: 50.0 });
        assert_eq!(node.ports[1].position, LPoint { x: 75.0, y: 100.0 });
    }

    #[test]
    fn layered_node_resize_keeps_external_port_positions_fixed() {
        let mut node = LNode::new("cluster", 100.0, 50.0, None);
        node.port_constraints = PortConstraints::FixedPos;
        node.ports.push(crate::graph::LPort::new(
            "external".to_string(),
            0,
            crate::graph::PortType::Input,
        ));
        node.ports[0].set_side(PortSide::West);
        node.ports[0].position = LPoint { x: -2.0, y: 20.0 };

        resize_layered_node(
            &mut node,
            LSize {
                width: 150.0,
                height: 100.0,
            },
            false,
            true,
        );

        assert_eq!(node.size.width, 150.0);
        assert_eq!(node.size.height, 100.0);
        assert_eq!(node.ports[0].position, LPoint { x: -2.0, y: 20.0 });
    }

    #[test]
    fn reversed_edge_restorer_processor_restores_cycle_breaking_edges() {
        let mut graph = import_graph(&ElkInputGraph {
            id: "root".to_string(),
            options: LayeredOptions {
                direction: ElkDirection::Right,
                greedy_switch_type: GreedySwitchType::Off,
                ..LayeredOptions::default()
            },
            nodes: vec![node("A"), node("B")],
            edges: vec![edge("A-B", "A", "B"), edge("B-A", "B", "A")],
        })
        .unwrap();

        execute_processors_until(&mut graph, LayeredPhase::P5EdgeRouting).unwrap();
        assert!(graph.edges.iter().any(|edge| edge.reversed));

        execute_processor(&mut graph, ProcessorKind::ReversedEdgeRestorer).unwrap();

        assert!(graph.edges.iter().all(|edge| !edge.reversed));
    }

    #[test]
    fn source_ported_end_label_flowchart_runs_through_end_label_lifecycle() {
        let mut head = ElkInputLabel::center("head", 20.0, 10.0);
        head.placement = crate::graph::EdgeLabelPlacement::Head;
        let mut labelled_edge = edge("A-B", "A", "B");
        labelled_edge.label = Some(head);
        let mut graph = import_graph(&ElkInputGraph {
            id: "root".to_string(),
            options: LayeredOptions {
                direction: ElkDirection::Right,
                greedy_switch_type: GreedySwitchType::Off,
                ..LayeredOptions::default()
            },
            nodes: vec![node("A"), node("B")],
            edges: vec![labelled_edge],
        })
        .unwrap();

        let executed = execute_ported_processors(&mut graph).unwrap();

        assert!(executed.contains(&ProcessorKind::LabelSideSelector));
        assert!(executed.contains(&ProcessorKind::EndLabelPreprocessor));
        assert!(executed.contains(&ProcessorKind::EndLabelSorter));
        assert!(executed.contains(&ProcessorKind::EndLabelPostprocessor));
        let edge = graph.edges.iter().find(|edge| edge.id == "A-B").unwrap();
        let label = edge
            .labels
            .iter()
            .find(|label| label.placement == crate::graph::EdgeLabelPlacement::Head)
            .expect("head label should be restored to its original edge");
        assert_eq!(label.text, "head");
        assert_eq!(label.size.width, 20.0);
        assert_eq!(label.size.height, 10.0);
        assert!(label.position.x.is_finite());
        assert!(label.position.y.is_finite());
        assert_eq!(label.end_label_edge, Some(0));
        assert!(
            graph
                .layerless_nodes
                .iter()
                .flat_map(|node| node.ports.iter())
                .all(|port| port.labels.is_empty() && port.end_label_cell.is_none())
        );
    }

    #[test]
    fn graph_aware_greedy_switch_matches_graph_configurator_activation_rules() {
        let mut options = LayeredOptions {
            hierarchy_handling: crate::options::HierarchyHandling::SeparateChildren,
            direction: ElkDirection::Right,
            greedy_switch_activation_threshold: 1,
            ..LayeredOptions::default()
        };
        let mut graph = LGraph::new("root", options.clone());
        graph
            .layerless_nodes
            .push(crate::graph::LNode::new("A", 10.0, 10.0, Some(0)));
        graph
            .layerless_nodes
            .push(crate::graph::LNode::new("B", 10.0, 10.0, Some(1)));
        assert!(
            !graph_kinds(&graph)
                .contains(&ProcessorKind::LayerSweepCrossingMinimizerTwoSidedGreedySwitch)
        );

        options.greedy_switch_activation_threshold = 0;
        graph.options = options;
        assert!(
            graph_kinds(&graph)
                .contains(&ProcessorKind::LayerSweepCrossingMinimizerTwoSidedGreedySwitch)
        );

        let mut nested = LGraph::new(
            "cluster",
            LayeredOptions {
                hierarchy_handling: crate::options::HierarchyHandling::IncludeChildren,
                greedy_switch_hierarchical_type: crate::options::GreedySwitchType::TwoSided,
                ..LayeredOptions::default()
            },
        );
        nested.parent_node_id = Some("cluster".to_string());
        assert!(
            !graph_kinds(&nested)
                .contains(&ProcessorKind::LayerSweepCrossingMinimizerTwoSidedGreedySwitch)
        );
    }

    fn push_test_external_dummy(graph: &mut LGraph, id: &str, side: PortSide) -> usize {
        let node = graph.layerless_nodes.len();
        let mut dummy = LNode::new(id, 0.0, 0.0, None);
        dummy.kind = LNodeKind::ExternalPort;
        dummy.external_port_side = side;
        graph.layerless_nodes.push(dummy);
        graph
            .add_port(node, PortType::Input, side.opposed(), Default::default())
            .unwrap();
        node
    }
}
