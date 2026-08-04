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
use crate::RandomSeedError;
use crate::compound::{
    preprocess_source_ported_compound_graph, source_ported_cross_hierarchy_segment_count,
};
use crate::configurator::{configure_graph_properties, configured_options};
use crate::graph::{LGraph, LNode, LNodeKind, LPoint, LSize, PortSide};
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
use crate::p2layers::layer_network_simplex;
use crate::p3order::{
    process_port_sides, sort_by_input_model, sort_port_lists,
    sweep::{
        CrossMinType, HierarchySweepDebugTrace, debug_crossings_layer_sweep_hierarchical_with_type,
        minimize_crossings_layer_sweep, minimize_crossings_layer_sweep_hierarchical_with_type,
        minimize_crossings_layer_sweep_with_type,
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

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum PipelineError {
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
struct GraphArenaChild {
    graph: usize,
    node: usize,
}

#[derive(Debug, PartialEq, Eq)]
struct GraphArenaEntry {
    parent: Option<GraphArenaParent>,
    children: Vec<GraphArenaChild>,
}

#[derive(Debug)]
struct CompoundGraphArena {
    entries: Vec<GraphArenaEntry>,
    postorder: Vec<usize>,
    algorithms: Vec<GraphAlgorithm>,
}

impl CompoundGraphArena {
    fn new(root: &LGraph) -> Self {
        let mut entries = vec![GraphArenaEntry {
            parent: None,
            children: Vec::new(),
        }];
        let mut algorithms = vec![graph_algorithm(root)];
        let mut search = vec![(0usize, root)];

        while let Some((graph_index, graph)) = search.pop() {
            let mut children = Vec::new();
            for (node_index, node) in graph.layerless_nodes.iter().enumerate() {
                let Some(nested) = node.nested_graph.as_deref() else {
                    continue;
                };
                let child_index = entries.len();
                let parent = GraphArenaParent {
                    graph: graph_index,
                    node: node_index,
                };
                let child = GraphArenaChild {
                    graph: child_index,
                    node: node_index,
                };
                entries.push(GraphArenaEntry {
                    parent: Some(parent),
                    children: Vec::new(),
                });
                algorithms.push(graph_algorithm(nested));
                entries[graph_index].children.push(child);
                children.push((child_index, nested));
            }
            search.extend(children.into_iter().rev());
        }

        // ELK advances every nested algorithm before its parent. The explicit postorder keeps that
        // source order without retaining or rebuilding complete root-to-graph paths.
        let mut postorder = Vec::with_capacity(entries.len());
        let mut stack = vec![(0usize, false)];
        while let Some((graph_index, expanded)) = stack.pop() {
            if expanded {
                postorder.push(graph_index);
                continue;
            }
            stack.push((graph_index, true));
            stack.extend(
                entries[graph_index]
                    .children
                    .iter()
                    .rev()
                    .map(|child| (child.graph, false)),
            );
        }

        Self {
            entries,
            postorder,
            algorithms,
        }
    }

    fn into_executions(self) -> Vec<GraphExecution> {
        let mut executions = self
            .algorithms
            .into_iter()
            .map(|algorithm| Some(algorithm.execution))
            .collect::<Vec<_>>();
        self.postorder
            .into_iter()
            .map(|graph_index| {
                executions[graph_index]
                    .take()
                    .expect("each graph execution should be emitted exactly once")
            })
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

fn is_source_ported_processor(kind: ProcessorKind) -> bool {
    matches!(
        kind,
        ProcessorKind::DirectionPreprocessor
            | ProcessorKind::EdgeAndLayerConstraintEdgeReverser
            | ProcessorKind::InteractiveExternalPortPositioner
            | ProcessorKind::GreedyCycleBreaker
            | ProcessorKind::DepthFirstCycleBreaker
            | ProcessorKind::InteractiveCycleBreaker
            | ProcessorKind::ModelOrderCycleBreaker
            | ProcessorKind::GreedyModelOrderCycleBreaker
            | ProcessorKind::LayerConstraintPreprocessor
            | ProcessorKind::NetworkSimplexLayerer
            | ProcessorKind::LabelDummyInserter
            | ProcessorKind::SelfLoopPreProcessor
            | ProcessorKind::LayerConstraintPostprocessor
            | ProcessorKind::LongEdgeSplitter
            | ProcessorKind::PortSideProcessor
            | ProcessorKind::InvertedPortProcessor
            | ProcessorKind::PortListSorter
            | ProcessorKind::SortByInputModelProcessor
            | ProcessorKind::HierarchicalPortConstraintProcessor
            | ProcessorKind::LayerSweepCrossingMinimizerBarycenter
            | ProcessorKind::LayerSweepCrossingMinimizerOneSidedGreedySwitch
            | ProcessorKind::LayerSweepCrossingMinimizerTwoSidedGreedySwitch
            | ProcessorKind::NoCrossingMinimizer
            | ProcessorKind::InLayerConstraintProcessor
            | ProcessorKind::LabelAndNodeSizeProcessor
            | ProcessorKind::InnermostNodeMarginCalculator
            | ProcessorKind::EndLabelPreprocessor
            | ProcessorKind::LabelSideSelector
            | ProcessorKind::HyperedgeDummyMerger
            | ProcessorKind::HierarchicalPortDummySizeProcessor
            | ProcessorKind::BKNodePlacer
            | ProcessorKind::SimpleNodePlacer
            | ProcessorKind::LinearSegmentsNodePlacer
            | ProcessorKind::NetworkSimplexPlacer
            | ProcessorKind::LayerSizeAndGraphHeightCalculator
            | ProcessorKind::HierarchicalPortPositionProcessor
            | ProcessorKind::OrthogonalEdgeRouter
            | ProcessorKind::LongEdgeJoiner
            | ProcessorKind::LabelDummyRemover
            | ProcessorKind::EndLabelSorter
            | ProcessorKind::ReversedEdgeRestorer
            | ProcessorKind::EndLabelPostprocessor
            | ProcessorKind::HierarchicalNodeResizer
            | ProcessorKind::DirectionPostprocessor
            | ProcessorKind::SelfLoopPortRestorer
            | ProcessorKind::SelfLoopRouter
            | ProcessorKind::SelfLoopPostProcessor
            | ProcessorKind::LabelDummySwitcher
            | ProcessorKind::HierarchicalPortOrthogonalEdgeRouter
    )
}

fn validate_ported_processors(processors: &[ProcessorSlot]) -> PipelineResult<()> {
    if let Some(slot) = processors
        .iter()
        .find(|slot| !is_source_ported_processor(slot.kind))
    {
        return Err(PipelineError::UnsupportedProcessor { kind: slot.kind });
    }
    Ok(())
}

fn validate_ported_graph_processors(graph: &LGraph) -> PipelineResult<()> {
    let mut stack = vec![graph];
    while let Some(current) = stack.pop() {
        validate_ported_processors(&assemble_processors_for_graph(current))?;
        stack.extend(
            current
                .layerless_nodes
                .iter()
                .rev()
                .filter_map(|node| node.nested_graph.as_deref()),
        );
    }
    Ok(())
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
    let mut validation_graph = graph.clone();
    configure_graph_properties(&mut validation_graph)?;
    validate_ported_processors(&assemble_processors_for_graph(&validation_graph))?;
    let processors = prepare_single_graph_processors(graph, work_control)?;

    for slot in processors {
        execute_processor_with_work_control(graph, slot.kind, work_control)?;
        executed.push(slot.kind);
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
    configure_graph_properties(graph)?;
    Ok(assemble_processors_for_graph(graph))
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
    let sweep_work = hierarchy_processor_work_units(
        graph,
        ProcessorKind::LayerSweepCrossingMinimizerBarycenter,
    )?;
    // The diagnostic path additionally snapshots graph/run/layer state while executing the same
    // barycenter sweep. They are one indivisible call, so reject the combined tranche before
    // either the sweep or its trace materialization begins.
    let diagnostic_work = checked_add(sweep_work, hierarchy_work_units(graph)?)?;
    work_control.check(diagnostic_work)?;
    work_control.charge(diagnostic_work)?;
    let trace = debug_crossings_layer_sweep_hierarchical_with_type(graph, CrossMinType::Barycenter);
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
    charge_compound_preprocess_work(graph, work_control)?;
    preprocess_source_ported_compound_graph(graph);
    charge_hierarchy_work(graph, work_control)?;
    configure_graph_properties(graph)?;
    reject_unsupported_compound_graph(graph)?;
    review_and_correct_hierarchical_processors(graph)?;

    let mut arena = build_compound_graph_arena_with_work_control(graph, work_control)?;
    const ROOT_GRAPH: usize = 0;
    if stop.is_none() {
        // Configuration can change the assembled processor list. Preserve the previous bottom-up
        // validation order while using the linear arena instead of rebuilding root-to-graph paths.
        for &graph_index in &arena.postorder {
            validate_ported_processors(&arena.algorithms[graph_index].processors)?;
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
            charge_hierarchy_processor_work(graph, kind, work_control)?;
            execute_hierarchy_aware_processor(graph, kind)?;
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

struct CompoundTraversalFrame {
    graph_index: usize,
    parent_node: usize,
    graph: Box<LGraph>,
    next_child: usize,
}

/// Owns the Rust-only detachment used while one compound subtree is processed.
///
/// Eclipse ELK keeps every nested graph attached to its parent object throughout execution. The
/// source port detaches graphs only to obtain disjoint mutable Rust owners, so an error or unwind
/// must not make that implementation detail observable to the caller. Dropping this guard restores
/// the complete child-to-parent chain in bottom-up order.
struct DetachedCompoundSubtree<'a> {
    root_parent: &'a mut LGraph,
    stack: Vec<CompoundTraversalFrame>,
}

impl<'a> DetachedCompoundSubtree<'a> {
    fn new(root_parent: &'a mut LGraph, root_child: GraphArenaChild) -> Self {
        let graph = root_parent.layerless_nodes[root_child.node]
            .nested_graph
            .take()
            .expect("the compound graph topology must remain stable during layered execution");
        Self {
            root_parent,
            stack: vec![CompoundTraversalFrame {
                graph_index: root_child.graph,
                parent_node: root_child.node,
                graph,
                next_child: 0,
            }],
        }
    }

    fn reattach_completed_frame(&mut self) {
        let frame = self
            .stack
            .pop()
            .expect("a completed compound graph should have an active frame");
        reattach_compound_frame(frame, &mut self.stack, self.root_parent);
    }
}

impl Drop for DetachedCompoundSubtree<'_> {
    fn drop(&mut self) {
        while let Some(frame) = self.stack.pop() {
            reattach_compound_frame(frame, &mut self.stack, self.root_parent);
        }
    }
}

fn execute_compound_graph_round(
    root: &mut LGraph,
    entries: &[GraphArenaEntry],
    algorithms: &mut [GraphAlgorithm],
    stop: Option<PipelineStop>,
    work_control: &mut dyn WorkControl,
) -> PipelineResult<()> {
    for child in entries[0].children.iter().copied() {
        debug_assert_eq!(
            entries[child.graph].parent,
            Some(GraphArenaParent {
                graph: 0,
                node: child.node,
            })
        );
        execute_compound_subtree_round(root, child, entries, algorithms, stop, work_control)?;
    }
    execute_compound_algorithm_until_pause(root, &mut algorithms[0], true, None, stop, work_control)
}

fn execute_compound_subtree_round(
    root_parent: &mut LGraph,
    root_child: GraphArenaChild,
    entries: &[GraphArenaEntry],
    algorithms: &mut [GraphAlgorithm],
    stop: Option<PipelineStop>,
    work_control: &mut dyn WorkControl,
) -> PipelineResult<()> {
    let mut traversal = DetachedCompoundSubtree::new(root_parent, root_child);

    loop {
        let child = {
            let frame = traversal
                .stack
                .last_mut()
                .expect("a detached compound subtree always has an active frame");
            let child = entries[frame.graph_index]
                .children
                .get(frame.next_child)
                .copied();
            frame.next_child += usize::from(child.is_some());
            child
        };
        if let Some(child) = child {
            debug_assert_eq!(
                entries[child.graph].parent,
                Some(GraphArenaParent {
                    graph: traversal
                        .stack
                        .last()
                        .expect("the parent frame should still be active")
                        .graph_index,
                    node: child.node,
                })
            );
            let graph = traversal
                .stack
                .last_mut()
                .expect("the parent frame should still be active")
                .graph
                .layerless_nodes[child.node]
                .nested_graph
                .take()
                .expect("the compound graph topology must remain stable during layered execution");
            traversal.stack.push(CompoundTraversalFrame {
                graph_index: child.graph,
                parent_node: child.node,
                graph,
                next_child: 0,
            });
            continue;
        }

        let frame_count = traversal.stack.len();
        let result = {
            let (root_parent, stack) = (&mut *traversal.root_parent, &mut traversal.stack);
            if frame_count > 1 {
                let (parents, current) = stack.split_at_mut(frame_count - 1);
                let parent = parents
                    .last_mut()
                    .expect("a nested compound frame should have a parent frame");
                let frame = &mut current[0];
                execute_compound_algorithm_until_pause(
                    &mut frame.graph,
                    &mut algorithms[frame.graph_index],
                    false,
                    Some(ParentGraph {
                        graph: &mut parent.graph,
                        node: frame.parent_node,
                    }),
                    stop,
                    work_control,
                )
            } else {
                let frame = stack
                    .last_mut()
                    .expect("a detached compound subtree always has an active frame");
                execute_compound_algorithm_until_pause(
                    &mut frame.graph,
                    &mut algorithms[frame.graph_index],
                    false,
                    Some(ParentGraph {
                        graph: root_parent,
                        node: frame.parent_node,
                    }),
                    stop,
                    work_control,
                )
            }
        };

        traversal.reattach_completed_frame();
        result?;
        if traversal.stack.is_empty() {
            return Ok(());
        }
    }
}

fn reattach_compound_frame(
    frame: CompoundTraversalFrame,
    stack: &mut [CompoundTraversalFrame],
    root_parent: &mut LGraph,
) {
    if let Some(parent) = stack.last_mut() {
        parent.graph.layerless_nodes[frame.parent_node].nested_graph = Some(frame.graph);
    } else {
        root_parent.layerless_nodes[frame.parent_node].nested_graph = Some(frame.graph);
    }
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
    let has_external_ports = {
        let node = &mut parent.layerless_nodes[node_index];
        transfer_external_port_dummy_layout_to_parent_node(
            nested_graph,
            node_index,
            &mut node.ports,
        );
        nested_graph.graph_properties.external_ports
    };

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
    let work_units = processor_work_units(graph, kind)?;
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
        ProcessorKind::HyperedgeDummyMerger => merge_hyperedge_dummies(graph),
        ProcessorKind::EndLabelPreprocessor => preprocess_end_labels(graph),
        ProcessorKind::BKNodePlacer => place_nodes_brandes_koepf(graph),
        ProcessorKind::SimpleNodePlacer => place_nodes_simple(graph),
        ProcessorKind::LinearSegmentsNodePlacer => place_nodes_linear_segments(graph),
        ProcessorKind::NetworkSimplexPlacer => place_nodes_network_simplex(graph),
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
    if matches!(
        kind,
        ProcessorKind::GreedyCycleBreaker | ProcessorKind::GreedyModelOrderCycleBreaker
    ) {
        return greedy_cycle_breaker_work_units(graph);
    }
    if kind == ProcessorKind::LongEdgeSplitter {
        return long_edge_splitter_work_units(graph);
    }
    if kind == ProcessorKind::LongEdgeJoiner {
        return long_edge_joiner_work_units(graph);
    }
    if kind == ProcessorKind::EndLabelSorter {
        return end_label_sorter_work_units(graph);
    }
    if kind == ProcessorKind::PortListSorter {
        return port_list_sorter_work_units(graph);
    }
    if kind == ProcessorKind::SortByInputModelProcessor {
        return sort_by_input_model_work_units(graph);
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
            let auxiliary_nodes = checked_sum([graph.layerless_nodes.len(), graph.edges.len(), 1])?;
            checked_mul(graph.options.thoroughness, auxiliary_nodes.max(1))?
        }
        ProcessorKind::LayerSweepCrossingMinimizerBarycenter
        | ProcessorKind::LayerSweepCrossingMinimizerOneSidedGreedySwitch
        | ProcessorKind::LayerSweepCrossingMinimizerTwoSidedGreedySwitch => {
            checked_mul(graph.options.thoroughness.max(1), graph.edges.len().max(1))?
        }
        _ => 1,
    };
    checked_mul(base, multiplier.max(1))
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
    let mut reference_rewrites = 0usize;
    let mut reorder_context = None;

    for layer in &graph.layers {
        for &node_index in &layer.nodes {
            node_visits = checked_add(node_visits, 1)?;
            let node = &graph.layerless_nodes[node_index];
            let port_count = node.ports.len();
            let constraints = node.port_constraints;
            if constraints.is_order_fixed() {
                let reorder_work = prepared_reorder_node_ports_work_units(
                    graph,
                    node_index,
                    &mut reorder_context,
                )?;
                port_scans = checked_add(port_scans, port_count)?;
                sort_work = checked_add(sort_work, checked_n_log_n(port_count)?)?;
                reference_rewrites = checked_add(reference_rewrites, reorder_work)?;
            } else if constraints.is_side_fixed() {
                let reorder_work = prepared_reorder_node_ports_work_units(
                    graph,
                    node_index,
                    &mut reorder_context,
                )?;
                // One side-key pass plus the two official South/West range probes.
                port_scans = checked_add(port_scans, checked_mul(port_count, 5)?)?;
                sort_work = checked_add(sort_work, checked_n_log_n(port_count)?)?;
                reference_rewrites = checked_add(reference_rewrites, reorder_work)?;
                for side in [crate::graph::PortSide::South, crate::graph::PortSide::West] {
                    if node.ports.iter().filter(|port| port.side == side).count() > 2 {
                        port_scans = checked_add(port_scans, port_count)?;
                        reference_rewrites = checked_add(reference_rewrites, reorder_work)?;
                    }
                }
                if graph.options.port_sorting_strategy
                    == crate::options::PortSortingStrategy::PortDegree
                {
                    port_scans = checked_add(port_scans, port_count)?;
                    sort_work = checked_add(sort_work, checked_n_log_n(port_count)?)?;
                    reference_rewrites = checked_add(reference_rewrites, reorder_work)?;
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

    // Mermaid enables this processor by default. Charge each official owner-local permutation and
    // the exact reference domains it scans; unrelated descendant edges and labels are not work of
    // the parent owner and must not be folded into a hierarchy-wide square.
    checked_sum([
        local_graph_work_units(graph)?,
        node_visits,
        port_scans,
        adjacency_scans,
        sort_work,
        reference_rewrites,
    ])
}

fn sort_by_input_model_work_units(graph: &LGraph) -> Result<usize, WorkError> {
    let max_ports = graph
        .layerless_nodes
        .iter()
        .map(|node| node.ports.len())
        .max()
        .unwrap_or(0);
    let chain_bound = checked_mul(
        checked_add(graph.edges.len(), 1)?,
        checked_add(max_ports, 2)?,
    )?;
    let mut layer_copy_work = 0usize;
    let mut preprocessing_work = 0usize;
    let mut relation_sort_work = 0usize;
    let mut reference_rewrites = 0usize;
    let mut reorder_context = None;

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
        ])?;

        let node_context =
            checked_sum([previous_layer.nodes.len(), checked_mul(max_ports, 4)?, 1])?;
        relation_sort_work = checked_add(
            relation_sort_work,
            stateful_relation_sort_work_units(layer.nodes.len(), node_context)?,
        )?;

        for &node_index in &layer.nodes {
            let node = &graph.layerless_nodes[node_index];
            if matches!(
                node.port_constraints,
                PortConstraints::FixedOrder | PortConstraints::FixedPos
            ) {
                continue;
            }

            let port_count = node.ports.len();
            let outgoing_ports = node
                .ports
                .iter()
                .filter(|port| !port.outgoing_edges.is_empty())
                .count();
            preprocessing_work = checked_sum([
                preprocessing_work,
                checked_mul(port_count, 2)?,
                checked_mul(outgoing_ports, chain_bound)?,
            ])?;
            relation_sort_work = checked_add(
                relation_sort_work,
                stateful_relation_sort_work_units(
                    port_count,
                    checked_add(previous_layer.nodes.len(), 1)?,
                )?,
            )?;
            reference_rewrites = checked_add(
                reference_rewrites,
                prepared_reorder_node_ports_work_units(graph, node_index, &mut reorder_context)?,
            )?;
        }
    }

    // The stateful ELK comparator must keep its stable-sort call order. Model its owner-local layer
    // and port widths plus the references each resulting permutation actually rewrites; taking the
    // fourth power of total hierarchy payload rejected ordinary official Mermaid fixtures.
    checked_sum([
        local_graph_work_units(graph)?,
        layer_copy_work,
        preprocessing_work,
        relation_sort_work,
        reference_rewrites,
    ])
}

fn stateful_relation_sort_work_units(
    item_count: usize,
    contextual_scan: usize,
) -> Result<usize, WorkError> {
    let comparisons = checked_n_log_n(item_count)?;
    let squared = checked_mul(item_count, item_count)?;
    // The official stateful comparator clones two relation sets and can clone/extend another set
    // inside each side of the transitive-closure update. Keep this local to the sorted owner set.
    let relation_closure =
        checked_sum([checked_mul(squared, 4)?, checked_mul(item_count, 8)?, 16])?;
    checked_mul(comparisons, checked_add(relation_closure, contextual_scan)?)
}

struct ReorderNodePortsWorkContext {
    shared_reference_work: usize,
    self_loop_holder_scan_work: usize,
    self_loop_payload_by_node: Vec<usize>,
}

impl ReorderNodePortsWorkContext {
    fn new(graph: &LGraph) -> Result<Self, WorkError> {
        let mut self_loop_payload_by_node = vec![0usize; graph.layerless_nodes.len()];
        for holder in &graph.self_loop_holders {
            let Some(payload) = self_loop_payload_by_node.get_mut(holder.node) else {
                continue;
            };
            for hyper_loop in &holder.hyper_loops {
                *payload = checked_sum([
                    *payload,
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

        Ok(Self {
            shared_reference_work: checked_sum([
                checked_mul(graph.edges.len(), 3)?,
                checked_mul(graph.layerless_nodes.len(), 3)?,
                descendant_nodes,
                graph.id.len(),
            ])?,
            self_loop_holder_scan_work: graph.self_loop_holders.len(),
            self_loop_payload_by_node,
        })
    }

    fn node_work(&self, graph: &LGraph, node_index: usize) -> Result<usize, WorkError> {
        let port_count = graph.layerless_nodes[node_index].ports.len();
        checked_sum([
            checked_mul(port_count, 4)?,
            self.shared_reference_work,
            self.self_loop_holder_scan_work,
            self.self_loop_payload_by_node
                .get(node_index)
                .copied()
                .unwrap_or(0),
        ])
    }
}

fn prepared_reorder_node_ports_work_units(
    graph: &LGraph,
    node_index: usize,
    context: &mut Option<ReorderNodePortsWorkContext>,
) -> Result<usize, WorkError> {
    if context.is_none() {
        // ELK reorders each owner independently, so every permutation still charges its global
        // reference rewrite. Only the immutable reference-domain census is shared across owners;
        // rebuilding that census per node made the resource estimator itself quadratic.
        *context = Some(ReorderNodePortsWorkContext::new(graph)?);
    }
    context
        .as_ref()
        .expect("reorder work context was initialized above")
        .node_work(graph, node_index)
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
    let mut ports = 0usize;
    let mut adjacency = 0usize;
    for node in &graph.layerless_nodes {
        ports = checked_add(ports, node.ports.len())?;
        for port in &node.ports {
            adjacency = checked_add(adjacency, port.incoming_edges.len())?;
            adjacency = checked_add(adjacency, port.outgoing_edges.len())?;
        }
    }
    Ok((ports, adjacency))
}

fn edge_reversal_adjacency_work_units(graph: &LGraph) -> Result<usize, WorkError> {
    let mut port_incidence = Vec::with_capacity(graph.layerless_nodes.len());
    let mut collector_incidence = Vec::with_capacity(graph.layerless_nodes.len());
    for node in &graph.layerless_nodes {
        let mut node_port_incidence = Vec::with_capacity(node.ports.len());
        let mut node_collector_incidence = 0usize;
        for port in &node.ports {
            let incidence = checked_add(port.incoming_edges.len(), port.outgoing_edges.len())?;
            node_port_incidence.push(incidence);
            if port.collector_type.is_some() {
                node_collector_incidence = checked_add(node_collector_incidence, incidence)?;
            }
        }
        port_incidence.push(node_port_incidence);
        collector_incidence.push(node_collector_incidence);
    }

    let mut total = 0usize;
    for edge in &graph.edges {
        let source_port = &graph.layerless_nodes[edge.source.node].ports[edge.source.port];
        let target_port = &graph.layerless_nodes[edge.target.node].ports[edge.target.port];
        let source_degree = if source_port.collector_type.is_some() {
            collector_incidence[edge.source.node]
        } else {
            port_incidence[edge.source.node][edge.source.port]
        };
        let target_degree = if target_port.collector_type.is_some() {
            collector_incidence[edge.target.node]
        } else {
            port_incidence[edge.target.node][edge.target.port]
        };
        // `reverse_edge` performs a linear search and then a shifting `Vec::remove` at both
        // endpoints. A normal port can only exchange incoming/outgoing incidence with itself;
        // collector adaptation can exchange incidence across the node's collector pair. Charge
        // those stable local bounds without turning unrelated per-edge ports into a false E^2.
        total = checked_add(
            total,
            checked_mul(checked_add(source_degree, target_degree)?, 2)?,
        )?;
    }
    Ok(total)
}

fn edge_reversal_work_units(graph: &LGraph) -> Result<usize, WorkError> {
    let mut total = edge_reversal_adjacency_work_units(graph)?;
    for edge in &graph.edges {
        let source_node = &graph.layerless_nodes[edge.source.node];
        let target_node = &graph.layerless_nodes[edge.target.node];
        // Collector adaptation scans the endpoint port lists and may append one opposite
        // collector. Labels and bend points are then traversed in place.
        total = checked_add(
            total,
            checked_sum([
                source_node.ports.len(),
                target_node.ports.len(),
                2,
                edge.labels.len(),
                edge.bend_points.len(),
                1,
            ])?,
        )?;
    }
    Ok(total)
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

fn hierarchy_work_units(graph: &LGraph) -> Result<usize, WorkError> {
    let mut total = 0usize;
    let mut stack = vec![graph];
    while let Some(current) = stack.pop() {
        total = checked_add(total, local_graph_work_units(current)?)?;
        total = checked_add(total, current.hierarchy_edges.len())?;
        total = checked_add(total, current.cross_hierarchy_edges.len())?;
        for node in &current.layerless_nodes {
            if let Some(nested) = node.nested_graph.as_deref() {
                stack.push(nested);
            }
        }
    }
    Ok(total.max(1))
}

fn hierarchy_arena_plan_work_units(graph: &LGraph) -> Result<usize, WorkError> {
    let mut graph_count = 0usize;
    let mut node_count = 0usize;
    let mut nested_graph_count = 0usize;
    let mut stack = vec![graph];
    while let Some(current) = stack.pop() {
        graph_count = checked_add(graph_count, 1)?;
        node_count = checked_add(node_count, current.layerless_nodes.len())?;
        for node in &current.layerless_nodes {
            if let Some(nested) = node.nested_graph.as_deref() {
                nested_graph_count = checked_add(nested_graph_count, 1)?;
                stack.push(nested);
            }
        }
    }

    // Charge discovery, the full node scan, both stored ends of every parent/child link, both
    // postorder frames, and one algorithm binding per graph. None of these terms depends on depth.
    checked_sum([
        graph_count,
        node_count,
        checked_mul(nested_graph_count, 2)?,
        checked_mul(graph_count, 2)?,
        graph_count,
    ])
}

fn compound_graph_round_work_units(graph_count: usize) -> Result<usize, WorkError> {
    let nested_graph_count = graph_count.saturating_sub(1);
    // Every round dispatches each graph once and moves every nested graph out of and back into its
    // parent exactly once. The iterative frames keep the live storage linear in hierarchy depth.
    checked_sum([graph_count, checked_mul(nested_graph_count, 2)?])
}

fn compound_preprocess_work_units(graph: &LGraph) -> Result<usize, WorkError> {
    let mut total = checked_mul(hierarchy_work_units(graph)?, 4)?;
    let mut stack = vec![graph];
    while let Some(current) = stack.pop() {
        for edge in &current.hierarchy_edges {
            total = checked_add(
                total,
                source_ported_cross_hierarchy_segment_count(
                    edge.source_node_id.as_str(),
                    edge.target_node_id.as_str(),
                    &edge.source_path,
                    &edge.target_path,
                )?,
            )?;
        }
        for node in &current.layerless_nodes {
            if let Some(nested) = node.nested_graph.as_deref() {
                stack.push(nested);
            }
        }
    }
    Ok(total)
}

fn charge_hierarchy_work(graph: &LGraph, work_control: &mut dyn WorkControl) -> PipelineResult<()> {
    let work_units = hierarchy_work_units(graph)?;
    work_control.check(work_units)?;
    work_control.charge(work_units)?;
    Ok(())
}

fn charge_hierarchy_arena_plan_work(
    graph: &LGraph,
    work_control: &mut dyn WorkControl,
) -> PipelineResult<()> {
    let work_units = hierarchy_arena_plan_work_units(graph)?;
    work_control.check(work_units)?;
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
    let work_units = hierarchy_processor_work_units(graph, kind)?;
    work_control.check(work_units)?;
    work_control.charge(work_units)?;
    Ok(())
}

fn hierarchy_processor_work_units(graph: &LGraph, kind: ProcessorKind) -> Result<usize, WorkError> {
    let base = hierarchy_work_units(graph)?;
    let mut multiplier = 1usize;
    if matches!(
        kind,
        ProcessorKind::LayerSweepCrossingMinimizerBarycenter
            | ProcessorKind::LayerSweepCrossingMinimizerOneSidedGreedySwitch
            | ProcessorKind::LayerSweepCrossingMinimizerTwoSidedGreedySwitch
    ) {
        let mut stack = vec![graph];
        let mut edge_count = 0usize;
        while let Some(current) = stack.pop() {
            multiplier = multiplier.max(current.options.thoroughness.max(1));
            edge_count = checked_add(edge_count, current.edges.len())?;
            stack.extend(
                current
                    .layerless_nodes
                    .iter()
                    .filter_map(|node| node.nested_graph.as_deref()),
            );
        }
        multiplier = checked_mul(multiplier, edge_count.max(1))?;
    }
    checked_mul(base, multiplier)
}

fn charge_compound_preprocess_work(
    graph: &LGraph,
    work_control: &mut dyn WorkControl,
) -> PipelineResult<()> {
    let work_units = compound_preprocess_work_units(graph)?;
    work_control.check(work_units)?;
    work_control.charge(work_units)?;
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

fn cycle_breaking_dependencies(processor: ProcessorKind) -> Config {
    let mut config = Config::default();
    if processor == ProcessorKind::InteractiveCycleBreaker {
        config.add_before(
            LayeredPhase::P1CycleBreaking,
            ProcessorKind::InteractiveExternalPortPositioner,
        );
    }
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
    use crate::graph::{LLabel, LNode, LNodeKind, LPort, Layer, PortSide, PortType};
    use crate::importer::{ElkInputEdge, ElkInputGraph, ElkInputLabel, ElkInputNode, import_graph};
    use crate::options::{
        CrossingMinimizationStrategy, CycleBreakingStrategy, EdgeRouting, ElkDirection,
        FixedAlignment, GreedySwitchType, LayeredOptions, LayeringStrategy, NodePlacementStrategy,
        OrderingStrategy, PortConstraints, WrappingStrategy,
    };
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

    fn assert_processors_are_source_ported(case: &str, processors: Vec<ProcessorKind>) {
        let unsupported = processors
            .into_iter()
            .filter(|kind| !is_source_ported_processor(*kind))
            .collect::<Vec<_>>();

        assert!(
            unsupported.is_empty(),
            "{case} reached unported ELK processors: {unsupported:?}"
        );
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
            model_order: None,
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

    #[derive(Debug, Clone, Copy)]
    struct BudgetWorkControl {
        remaining: usize,
        charged: usize,
    }

    impl BudgetWorkControl {
        fn new(remaining: usize) -> Self {
            Self {
                remaining,
                charged: 0,
            }
        }
    }

    impl WorkControl for BudgetWorkControl {
        fn check(&mut self, units: usize) -> Result<(), WorkError> {
            if units <= self.remaining {
                Ok(())
            } else {
                Err(WorkError::Interrupted)
            }
        }

        fn charge(&mut self, units: usize) -> Result<(), WorkError> {
            self.check(units)?;
            self.remaining -= units;
            self.charged += units;
            Ok(())
        }
    }

    fn bidirected_cycle_graph(node_count: usize) -> LGraph {
        let nodes = (0..node_count)
            .map(|index| node(&format!("n{index}")))
            .collect::<Vec<_>>();
        let mut edges = Vec::with_capacity(node_count * 2);
        for index in 0..node_count {
            let next = (index + 1) % node_count;
            edges.push(edge(
                &format!("forward-{index}"),
                &format!("n{index}"),
                &format!("n{next}"),
            ));
            edges.push(edge(
                &format!("backward-{index}"),
                &format!("n{next}"),
                &format!("n{index}"),
            ));
        }

        import_graph(&ElkInputGraph {
            id: "root".to_string(),
            options: LayeredOptions {
                consider_model_order_strategy: OrderingStrategy::NodesAndEdges,
                ..LayeredOptions::default()
            },
            nodes,
            edges,
        })
        .unwrap()
    }

    fn parallel_long_edge_graph(edge_count: usize) -> LGraph {
        disjoint_long_edge_graph(edge_count, edge_count + 1)
    }

    fn disjoint_long_edge_graph(edge_count: usize, target_layer: usize) -> LGraph {
        assert!(target_layer > 0);
        let mut nodes = Vec::with_capacity(edge_count * 2);
        let mut edges = Vec::with_capacity(edge_count);
        for index in 0..edge_count {
            nodes.push(node(&format!("source-{index}")));
            nodes.push(node(&format!("target-{index}")));
            edges.push(edge(
                &format!("edge-{index}"),
                &format!("source-{index}"),
                &format!("target-{index}"),
            ));
        }

        let mut graph = import_graph(&ElkInputGraph {
            id: "root".to_string(),
            options: LayeredOptions::default(),
            nodes,
            edges,
        })
        .unwrap();
        graph.clear_layers();
        for index in 0..edge_count {
            graph.set_node_layer(index * 2, 0);
            graph.set_node_layer(index * 2 + 1, target_layer);
        }
        graph
    }

    fn shared_target_long_edge_graph_with_span(edge_count: usize, target_layer: usize) -> LGraph {
        assert!(target_layer > 0);
        let mut nodes = (0..edge_count)
            .map(|index| node(&format!("source-{index}")))
            .collect::<Vec<_>>();
        nodes.push(node("target"));
        let edges = (0..edge_count)
            .map(|index| {
                edge(
                    &format!("edge-{index}"),
                    &format!("source-{index}"),
                    "target",
                )
            })
            .collect::<Vec<_>>();

        let mut graph = import_graph(&ElkInputGraph {
            id: "root".to_string(),
            options: LayeredOptions {
                merge_edges: true,
                ..LayeredOptions::default()
            },
            nodes,
            edges,
        })
        .unwrap();
        graph.clear_layers();
        for source in 0..edge_count {
            graph.set_node_layer(source, 0);
        }
        graph.set_node_layer(edge_count, target_layer);
        graph
    }

    fn shared_port_cycle_graph(parallel_edge_count: usize) -> LGraph {
        let mut edges = Vec::with_capacity(parallel_edge_count * 2);
        for index in 0..parallel_edge_count {
            edges.push(edge(&format!("A-B-{index}"), "A", "B"));
            edges.push(edge(&format!("B-A-{index}"), "B", "A"));
        }
        import_graph(&ElkInputGraph {
            id: "root".to_string(),
            options: LayeredOptions {
                merge_edges: true,
                consider_model_order_strategy: OrderingStrategy::NodesAndEdges,
                ..LayeredOptions::default()
            },
            nodes: vec![node("A"), node("B")],
            edges,
        })
        .unwrap()
    }

    fn deep_compound_chain(graph_count: usize) -> LGraph {
        assert!(graph_count > 0);
        let options = LayeredOptions {
            hierarchy_handling: crate::options::HierarchyHandling::IncludeChildren,
            greedy_switch_type: GreedySwitchType::Off,
            greedy_switch_hierarchical_type: GreedySwitchType::Off,
            ..LayeredOptions::default()
        };
        let mut nested = None;
        for depth in (0..graph_count).rev() {
            let mut graph = LGraph::new(format!("graph-{depth}"), options.clone());
            if depth > 0 {
                graph.parent_node_id = Some(format!("node-{}", depth - 1));
            }
            if let Some(child) = nested.take() {
                let mut holder = LNode::new(format!("node-{depth}"), 1.0, 1.0, None);
                holder.compound = true;
                holder.nested_graph = Some(child);
                graph.layerless_nodes.push(holder);
            }
            nested = Some(Box::new(graph));
        }
        *nested.expect("a positive graph count creates the root graph")
    }

    fn high_thoroughness_diagnostic_graph() -> LGraph {
        let mut options = LayeredOptions::mermaid_flowchart_defaults(ElkDirection::Down);
        options.thoroughness = 64;
        import_graph(&ElkInputGraph {
            id: "root".to_string(),
            options,
            nodes: vec![
                node("top-a"),
                node("top-b"),
                node("bottom-a"),
                node("bottom-b"),
            ],
            // K2,2 has an unavoidable crossing, so the debug barycenter sweep cannot terminate
            // early at zero and must execute every configured thoroughness run.
            edges: vec![
                edge("aa", "top-a", "bottom-a"),
                edge("ab", "top-a", "bottom-b"),
                edge("ba", "top-b", "bottom-a"),
                edge("bb", "top-b", "bottom-b"),
            ],
        })
        .unwrap()
    }

    fn assert_processor_budget_boundaries(graph: &LGraph, kind: ProcessorKind) {
        let required = processor_work_units(graph, kind).unwrap();
        assert!(required > 0);

        let mut below_graph = graph.clone();
        let mut below = BudgetWorkControl::new(required - 1);
        assert_eq!(
            execute_processor_with_work_control(&mut below_graph, kind, &mut below),
            Err(PipelineError::Work(WorkError::Interrupted))
        );
        assert_eq!(below_graph, *graph, "rejection must precede mutation");
        assert_eq!(below.charged, 0);
        assert_eq!(below.remaining, required - 1);

        let mut exact_graph = graph.clone();
        let mut exact = BudgetWorkControl::new(required);
        execute_processor_with_work_control(&mut exact_graph, kind, &mut exact).unwrap();
        assert_ne!(
            exact_graph, *graph,
            "{kind:?} boundary fixture must expose mutation"
        );
        assert_eq!(exact.charged, required);
        assert_eq!(exact.remaining, 0);

        let mut above_graph = graph.clone();
        let mut above = BudgetWorkControl::new(required + 1);
        execute_processor_with_work_control(&mut above_graph, kind, &mut above).unwrap();
        assert_eq!(
            above_graph, exact_graph,
            "budget headroom changed semantics"
        );
        assert_eq!(above.charged, required);
        assert_eq!(above.remaining, 1);
    }

    #[test]
    fn superlinear_processors_enforce_transactional_budget_boundaries() {
        let cycle_graph = bidirected_cycle_graph(4);
        assert_processor_budget_boundaries(&cycle_graph, ProcessorKind::GreedyCycleBreaker);
        assert_processor_budget_boundaries(
            &cycle_graph,
            ProcessorKind::GreedyModelOrderCycleBreaker,
        );

        let long_edge_graph = parallel_long_edge_graph(3);
        assert_processor_budget_boundaries(&long_edge_graph, ProcessorKind::LongEdgeSplitter);

        let mut joined_graph = disjoint_long_edge_graph(3, 2);
        split_long_edges(&mut joined_graph);
        assert_processor_budget_boundaries(&joined_graph, ProcessorKind::LongEdgeJoiner);

        let mut shared_target_graph = shared_target_long_edge_graph_with_span(32, 2);
        split_long_edges(&mut shared_target_graph);
        assert_processor_budget_boundaries(&shared_target_graph, ProcessorKind::LongEdgeJoiner);
    }

    #[test]
    fn long_edge_joiner_work_is_linear_in_disjoint_segment_count() {
        for edge_count in [1usize, 8, 32, 128] {
            let mut graph = disjoint_long_edge_graph(edge_count, 2);
            split_long_edges(&mut graph);
            let base = local_graph_work_units(&graph).unwrap();

            // Each edge creates one two-port dummy and one pair. The preflight and layer rebuild
            // visit three nodes per edge, while the global port/adjacency index is built once.
            assert_eq!(
                long_edge_joiner_work_units(&graph),
                Ok(base + 26 * edge_count)
            );
            assert_eq!(
                processor_work_units(&graph, ProcessorKind::LongEdgeJoiner),
                long_edge_joiner_work_units(&graph)
            );
        }
    }

    #[test]
    fn long_edge_joiner_work_is_linear_for_shared_target_fanin() {
        for edge_count in [1usize, 8, 32, 128, 512] {
            let mut graph = shared_target_long_edge_graph_with_span(edge_count, 2);
            split_long_edges(&mut graph);
            let base = local_graph_work_units(&graph).unwrap();

            // The collector's incoming adjacency is indexed once for the whole graph. It must not
            // be multiplied by the number of long-edge dummies that share that target port.
            assert_eq!(
                long_edge_joiner_work_units(&graph),
                Ok(base + 23 * edge_count + 3)
            );
        }
    }

    #[test]
    fn long_edge_joiner_work_skips_index_cost_without_dummies() {
        let graph = disjoint_long_edge_graph(32, 1);
        let base = local_graph_work_units(&graph).unwrap();
        let layer_memberships = graph
            .layers
            .iter()
            .map(|layer| layer.nodes.len())
            .sum::<usize>();

        assert_eq!(
            long_edge_joiner_work_units(&graph),
            Ok(base + layer_memberships)
        );
    }

    #[test]
    fn end_label_sorter_work_sums_owner_local_label_sorts() {
        let expected_sort_work = [(1usize, 0usize), (8, 24), (16, 64), (32, 160)];
        for (label_count, sort_work) in expected_sort_work {
            let mut graph = LGraph::new("root", LayeredOptions::default());
            let mut node = LNode::new("node", 10.0, 10.0, Some(0));
            let mut port = LPort::new("port", 0, PortType::Output);
            port.labels = (0..label_count)
                .map(|index| LLabel::new(format!("label-{index}"), 1.0, 1.0))
                .collect();
            node.ports.push(port);
            graph.layerless_nodes.push(node);

            let base = local_graph_work_units(&graph).unwrap();
            assert_eq!(
                end_label_sorter_work_units(&graph),
                Ok(base + label_count + sort_work)
            );
            assert_eq!(
                processor_work_units(&graph, ProcessorKind::EndLabelSorter),
                end_label_sorter_work_units(&graph)
            );
        }

        let mut graph = LGraph::new("root", LayeredOptions::default());
        let mut node = LNode::new("node", 10.0, 10.0, Some(0));
        for port_index in 0..2 {
            let mut port = LPort::new(format!("port-{port_index}"), 0, PortType::Output);
            port.labels = (0..8)
                .map(|label_index| {
                    LLabel::new(format!("label-{port_index}-{label_index}"), 1.0, 1.0)
                })
                .collect();
            node.ports.push(port);
        }
        graph.layerless_nodes.push(node);
        let base = local_graph_work_units(&graph).unwrap();
        // Two local 8-item sorts cost 2*24, not one global 16-item sort (64).
        assert_eq!(end_label_sorter_work_units(&graph), Ok(base + 16 + 48));
    }

    #[test]
    fn port_model_order_processors_charge_only_owner_local_amplification() {
        let mut graph = LGraph::new("root", LayeredOptions::default());
        let mut node = LNode::new("node", 10.0, 10.0, Some(0));
        node.ports.push(LPort::new("port-0", 0, PortType::Output));
        node.ports.push(LPort::new("port-1", 0, PortType::Output));
        graph.layerless_nodes.push(node);
        graph.layers.push(Layer {
            nodes: vec![0],
            ..Layer::default()
        });

        // One node, two ports, and one layer give an independently countable base of four.
        assert_eq!(local_graph_work_units(&graph), Ok(4));
        assert_eq!(hierarchy_work_units(&graph), Ok(4));

        // Free ports make PortListSorter visit the owner node without sorting or rewriting it.
        assert_eq!(port_list_sorter_work_units(&graph), Ok(4 + 1));
        // SortByInputModel copies the one-node layer three times, scans two ports twice, charges
        // one two-item stateful relation sort, and performs one exact owner reference rewrite.
        assert_eq!(
            sort_by_input_model_work_units(&graph),
            Ok(4 + 3 + 4 + 100 + 15)
        );
        assert_eq!(
            processor_work_units(&graph, ProcessorKind::PortListSorter),
            Ok(5)
        );
        assert_eq!(
            processor_work_units(&graph, ProcessorKind::SortByInputModelProcessor),
            Ok(126)
        );

        graph.layerless_nodes[0].port_constraints = PortConstraints::FixedOrder;
        // Fixed-order ports add one scan, one two-item sort, and one hierarchy-wide reference
        // rewrite. SortByInputModel now skips the node after the layer-copy work.
        assert_eq!(port_list_sorter_work_units(&graph), Ok(4 + 1 + 2 + 2 + 15));
        assert_eq!(sort_by_input_model_work_units(&graph), Ok(4 + 3));
    }

    #[test]
    fn prepared_port_reorder_work_matches_the_per_owner_reference() {
        let mut graph = LGraph::new("root", LayeredOptions::default());
        for node_index in 0..16 {
            let mut node = LNode::new(format!("node-{node_index}"), 10.0, 10.0, Some(node_index));
            node.port_constraints = PortConstraints::FixedOrder;
            node.ports.push(LPort::new(
                format!("port-{node_index}-0"),
                node_index,
                PortType::Output,
            ));
            node.ports.push(LPort::new(
                format!("port-{node_index}-1"),
                node_index,
                PortType::Output,
            ));
            graph.layerless_nodes.push(node);
        }
        graph.layers.push(Layer {
            nodes: (0..graph.layerless_nodes.len()).collect(),
            ..Layer::default()
        });
        graph.layerless_nodes[0].nested_graph = Some(Box::new(deep_compound_chain(64)));

        let context = ReorderNodePortsWorkContext::new(&graph).unwrap();
        for node_index in 0..graph.layerless_nodes.len() {
            assert_eq!(
                context.node_work(&graph, node_index),
                unprepared_reorder_node_ports_work_units(&graph, node_index)
            );
        }
    }

    #[test]
    fn single_graph_configuration_preflights_the_complete_nested_hierarchy() {
        let graph = deep_compound_chain(32);
        let required = hierarchy_work_units(&graph).unwrap();

        for runner in 0..3 {
            let mut below_graph = graph.clone();
            let before = below_graph.clone();
            let mut below = BudgetWorkControl::new(required - 1);
            let result = match runner {
                0 => execute_processors_until_with_work_control(
                    &mut below_graph,
                    LayeredPhase::P1CycleBreaking,
                    &mut below,
                )
                .map(|_| ()),
                1 => execute_processors_until_processor_with_work_control(
                    &mut below_graph,
                    ProcessorKind::PortListSorter,
                    &mut below,
                )
                .map(|_| ()),
                _ => execute_ported_processors_with_work_control(&mut below_graph, &mut below)
                    .map(|_| ()),
            };
            assert_eq!(result, Err(PipelineError::Work(WorkError::Interrupted)));
            assert_eq!(below_graph, before);
            assert_eq!(below.charged, 0);
            assert_eq!(below.remaining, required - 1);
        }

        let mut exact_graph = graph.clone();
        let mut exact = BudgetWorkControl::new(required);
        let exact_processors =
            prepare_single_graph_processors(&mut exact_graph, &mut exact).unwrap();
        assert_eq!(exact.charged, required);
        assert_eq!(exact.remaining, 0);

        let mut above_graph = graph;
        let mut above = BudgetWorkControl::new(required + 1);
        let above_processors =
            prepare_single_graph_processors(&mut above_graph, &mut above).unwrap();
        assert_eq!(above_graph, exact_graph);
        assert_eq!(above_processors, exact_processors);
        assert_eq!(above.charged, required);
        assert_eq!(above.remaining, 1);
    }

    #[test]
    fn greedy_cycle_breaker_work_curve_covers_quadratic_candidate_scans() {
        let mut previous_quadratic = None;
        for node_count in [8usize, 16, 32, 64] {
            let graph = bidirected_cycle_graph(node_count);
            let (ports, adjacency) = graph_port_and_adjacency_work_units(&graph).unwrap();
            let linear = checked_mul(
                checked_sum([node_count, ports, adjacency, graph.edges.len()]).unwrap(),
                4,
            )
            .unwrap();
            let work = greedy_cycle_breaker_work_units(&graph).unwrap();
            let quadratic = work - linear - edge_reversal_work_units(&graph).unwrap();

            assert_eq!(quadratic, 3 * node_count * node_count);
            for kind in [
                ProcessorKind::GreedyCycleBreaker,
                ProcessorKind::GreedyModelOrderCycleBreaker,
            ] {
                assert_eq!(
                    processor_work_units(&graph, kind),
                    greedy_cycle_breaker_work_units(&graph)
                );
            }
            if let Some(previous) = previous_quadratic {
                assert_eq!(quadratic, previous * 4);
            }
            previous_quadratic = Some(quadratic);
        }
    }

    #[test]
    fn greedy_cycle_breaker_work_curve_tracks_shared_port_removal_amplification() {
        let mut previous = None;
        for parallel_edge_count in [8usize, 16, 32, 64] {
            let graph = shared_port_cycle_graph(parallel_edge_count);
            let adjacency_work = edge_reversal_adjacency_work_units(&graph).unwrap();

            assert_eq!(
                adjacency_work,
                16 * parallel_edge_count * parallel_edge_count
            );
            if let Some(previous) = previous {
                assert_eq!(adjacency_work, previous * 4);
            }
            previous = Some(adjacency_work);
        }
    }

    #[test]
    fn long_edge_splitter_work_curve_tracks_total_rank_span() {
        let edge_count = 4usize;
        for target_layer in [2usize, 4, 8, 16, 32] {
            let graph = disjoint_long_edge_graph(edge_count, target_layer);
            let base = local_graph_work_units(&graph).unwrap();
            let work = long_edge_splitter_work_units(&graph).unwrap();
            let expected_splits = edge_count * (target_layer - 1);

            assert_eq!(work - base, expected_splits * 10);
            assert_eq!(
                processor_work_units(&graph, ProcessorKind::LongEdgeSplitter),
                Ok(work)
            );
        }
    }

    #[test]
    fn long_edge_splitter_work_curve_is_linear_in_edge_count_at_fixed_span() {
        let target_layer = 9usize;
        let mut previous = None;
        for edge_count in [4usize, 8, 16, 32] {
            let graph = disjoint_long_edge_graph(edge_count, target_layer);
            let base = local_graph_work_units(&graph).unwrap();
            let incremental = long_edge_splitter_work_units(&graph).unwrap() - base;

            assert_eq!(incremental, edge_count * (target_layer - 1) * 10);
            if let Some(previous) = previous {
                assert_eq!(incremental, previous * 2);
            }
            previous = Some(incremental);
        }
    }

    #[test]
    fn long_edge_splitter_work_curve_tracks_shared_target_removal_amplification() {
        let target_layer = 9usize;
        for edge_count in [4usize, 8, 16, 32] {
            let graph = shared_target_long_edge_graph_with_span(edge_count, target_layer);
            let base = local_graph_work_units(&graph).unwrap();
            let work = long_edge_splitter_work_units(&graph).unwrap();
            let split_count = edge_count * (target_layer - 1);
            let target_rewrite_work = 2 * split_count * edge_count;

            assert_eq!(work - base, split_count * 8 + target_rewrite_work);
        }
    }

    #[test]
    fn long_edge_splitter_work_ignores_same_layer_and_unlayered_targets() {
        let mut graph = import_graph(&ElkInputGraph {
            id: "root".to_string(),
            options: LayeredOptions::default(),
            nodes: vec![node("A"), node("B"), node("C"), node("layer-anchor")],
            edges: vec![edge("same", "A", "B"), edge("unlayered", "A", "C")],
        })
        .unwrap();
        graph.clear_layers();
        graph.set_node_layer(0, 0);
        graph.set_node_layer(1, 0);
        graph.set_node_layer(3, 2);

        assert_eq!(
            long_edge_splitter_work_units(&graph),
            local_graph_work_units(&graph)
        );
    }

    #[test]
    fn long_edge_splitter_work_accounts_for_repeated_head_label_moves() {
        let mut head = ElkInputLabel::center("head", 20.0, 10.0);
        head.placement = crate::graph::EdgeLabelPlacement::Head;
        let mut labelled = edge("long", "A", "D");
        labelled.label = Some(head);
        let mut graph = import_graph(&ElkInputGraph {
            id: "root".to_string(),
            options: LayeredOptions::default(),
            nodes: vec![node("A"), node("B"), node("C"), node("D")],
            edges: vec![labelled],
        })
        .unwrap();
        graph.clear_layers();
        for node_index in 0..4 {
            graph.set_node_layer(node_index, node_index);
        }

        let base = local_graph_work_units(&graph).unwrap();
        // Two splits: 16 structural units, four target-list rewrite units, and two complete
        // clone/scan/move passes over the downstream head label.
        assert_eq!(long_edge_splitter_work_units(&graph).unwrap() - base, 28);
    }

    #[test]
    fn long_edge_splitter_work_accounts_for_quadratic_head_label_removals() {
        let mut head = ElkInputLabel::center("head-0", 20.0, 10.0);
        head.placement = crate::graph::EdgeLabelPlacement::Head;
        let mut labelled = edge("long", "A", "E");
        labelled.label = Some(head);
        let mut graph = import_graph(&ElkInputGraph {
            id: "root".to_string(),
            options: LayeredOptions::default(),
            nodes: vec![node("A"), node("B"), node("C"), node("D"), node("E")],
            edges: vec![labelled],
        })
        .unwrap();
        for index in 1..4 {
            let mut label = crate::graph::LLabel::new(format!("head-{index}"), 20.0, 10.0);
            label.placement = crate::graph::EdgeLabelPlacement::Head;
            graph.edges[0].labels.push(label);
        }
        graph.clear_layers();
        for node_index in 0..5 {
            graph.set_node_layer(node_index, node_index);
        }

        assert_eq!(split_edge_payload_work_units(&graph.edges[0], 3), Ok(66));
        let base = local_graph_work_units(&graph).unwrap();
        // Three splits add 24 structural units and six target-list rewrite units. Four head labels
        // add 22 units on the first split and on each later split: clone + scan + moves + 3+2+1
        // shifted tail elements from repeated `Vec::remove(0)`.
        assert_eq!(long_edge_splitter_work_units(&graph).unwrap() - base, 96);
    }

    #[test]
    fn compound_arena_plan_work_curve_is_linear_in_depth() {
        for graph_count in [8usize, 16, 32, 64, 128] {
            let graph = deep_compound_chain(graph_count);
            let node_count = graph_count - 1;
            let nested_graph_count = graph_count - 1;
            assert_eq!(
                hierarchy_arena_plan_work_units(&graph).unwrap(),
                graph_count * 4 + node_count + nested_graph_count * 2
            );
        }
    }

    #[test]
    fn compound_arena_stores_one_parent_and_child_link_per_nested_graph() {
        for graph_count in [8usize, 16, 32, 64, 128] {
            let graph = deep_compound_chain(graph_count);
            let arena = CompoundGraphArena::new(&graph);
            let parent_links = arena
                .entries
                .iter()
                .filter(|entry| entry.parent.is_some())
                .count();
            let child_links = arena
                .entries
                .iter()
                .map(|entry| entry.children.len())
                .sum::<usize>();

            assert_eq!(arena.entries.len(), graph_count);
            assert_eq!(arena.algorithms.len(), graph_count);
            assert_eq!(arena.postorder.len(), graph_count);
            assert_eq!(parent_links, graph_count - 1);
            assert_eq!(child_links, graph_count - 1);
            assert_eq!(arena.postorder, (0..graph_count).rev().collect::<Vec<_>>());
        }
    }

    #[test]
    fn compound_arena_preserves_node_ordered_depth_first_postorder() {
        let options = LayeredOptions::default();
        let grandchild = LGraph::new("grandchild", options.clone());
        let mut first_child = LGraph::new("first-child", options.clone());
        let mut grandchild_holder = LNode::new("grandchild-holder", 1.0, 1.0, None);
        grandchild_holder.nested_graph = Some(Box::new(grandchild));
        first_child.layerless_nodes.push(grandchild_holder);
        let second_child = LGraph::new("second-child", options.clone());
        let mut root = LGraph::new("root", options);
        let mut first_holder = LNode::new("first-holder", 1.0, 1.0, None);
        first_holder.nested_graph = Some(Box::new(first_child));
        let mut second_holder = LNode::new("second-holder", 1.0, 1.0, None);
        second_holder.nested_graph = Some(Box::new(second_child));
        root.layerless_nodes.extend([first_holder, second_holder]);

        let arena = CompoundGraphArena::new(&root);
        let graph_ids = arena
            .postorder
            .iter()
            .map(|graph_index| arena.algorithms[*graph_index].execution.graph_id.as_str())
            .collect::<Vec<_>>();

        assert_eq!(
            graph_ids,
            ["grandchild", "first-child", "second-child", "root"]
        );
    }

    #[test]
    fn compound_round_reattaches_the_hierarchy_when_a_child_processor_fails() {
        let options = LayeredOptions::default();
        let child = LGraph::new("child", options.clone());
        let mut holder = LNode::new("holder", 1.0, 1.0, None);
        holder.nested_graph = Some(Box::new(child));
        let mut root = LGraph::new("root", options);
        root.layerless_nodes.push(holder);
        let mut arena = CompoundGraphArena::new(&root);
        let child_index = arena.entries[0].children[0].graph;
        arena.algorithms[child_index].processors = vec![ProcessorSlot {
            phase: None,
            kind: ProcessorKind::CommentPreprocessor,
        }];
        let mut work_control = NoopWorkControl;

        assert_eq!(
            execute_compound_graph_round(
                &mut root,
                &arena.entries,
                &mut arena.algorithms,
                None,
                &mut work_control,
            ),
            Err(PipelineError::UnsupportedProcessor {
                kind: ProcessorKind::CommentPreprocessor,
            })
        );
        assert_eq!(
            root.layerless_nodes[0]
                .nested_graph
                .as_deref()
                .map(|graph| graph.id.as_str()),
            Some("child")
        );
    }

    #[test]
    fn compound_round_reattaches_the_complete_hierarchy_when_work_control_panics() {
        struct PanicOnCharge;

        impl WorkControl for PanicOnCharge {
            fn check(&mut self, _units: usize) -> Result<(), WorkError> {
                Ok(())
            }

            fn charge(&mut self, _units: usize) -> Result<(), WorkError> {
                panic!("injected work-control panic")
            }
        }

        let mut root = deep_compound_chain(3);
        let mut arena = CompoundGraphArena::new(&root);
        for algorithm in &mut arena.algorithms {
            algorithm.processors.clear();
        }
        let child_index = arena.entries[0].children[0].graph;
        let grandchild_index = arena.entries[child_index].children[0].graph;
        arena.algorithms[grandchild_index].processors = vec![ProcessorSlot {
            phase: None,
            kind: ProcessorKind::DirectionPreprocessor,
        }];
        let mut work_control = PanicOnCharge;

        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let _ = execute_compound_graph_round(
                &mut root,
                &arena.entries,
                &mut arena.algorithms,
                None,
                &mut work_control,
            );
        }));

        assert!(result.is_err());
        let child = root.layerless_nodes[0]
            .nested_graph
            .as_deref()
            .expect("the first detached graph must be restored during unwind");
        assert_eq!(child.id, "graph-1");
        let grandchild = child.layerless_nodes[0]
            .nested_graph
            .as_deref()
            .expect("the deepest detached graph must be restored during unwind");
        assert_eq!(grandchild.id, "graph-2");
    }

    #[test]
    fn compound_arena_plan_work_curve_accounts_for_flat_node_scans() {
        for node_count in [8usize, 16, 32, 64, 128] {
            let mut graph = LGraph::new("root", LayeredOptions::default());
            for index in 0..node_count {
                graph
                    .layerless_nodes
                    .push(LNode::new(format!("node-{index}"), 1.0, 1.0, None));
            }

            assert_eq!(
                hierarchy_arena_plan_work_units(&graph).unwrap(),
                node_count + 4
            );
        }
    }

    #[test]
    fn compound_arena_plan_rejects_before_materializing_the_index() {
        let mut graph = deep_compound_chain(32);
        let required = hierarchy_arena_plan_work_units(&graph).unwrap();

        let mut below = BudgetWorkControl::new(required - 1);
        assert!(matches!(
            build_compound_graph_arena_with_work_control(&mut graph, &mut below),
            Err(PipelineError::Work(WorkError::Interrupted))
        ));
        assert_eq!(below.charged, 0);
        assert_eq!(below.remaining, required - 1);

        let mut exact = BudgetWorkControl::new(required);
        let exact_arena =
            build_compound_graph_arena_with_work_control(&mut graph, &mut exact).unwrap();
        assert_eq!(exact_arena.entries.len(), 32);
        assert_eq!(exact_arena.postorder, (0..32).rev().collect::<Vec<_>>());
        assert_eq!(exact.charged, required);
        assert_eq!(exact.remaining, 0);

        let mut above = BudgetWorkControl::new(required + 1);
        let above_arena =
            build_compound_graph_arena_with_work_control(&mut graph, &mut above).unwrap();
        assert_eq!(above_arena.entries, exact_arena.entries);
        assert_eq!(above_arena.postorder, exact_arena.postorder);
        assert_eq!(above.charged, required);
        assert_eq!(above.remaining, 1);
    }

    #[test]
    fn diagnostic_crossing_sweep_and_trace_share_one_budget_tranche() {
        const HIERARCHY_WORK: usize = 18;
        const EDGE_COUNT: usize = 4;
        const THOROUGHNESS: usize = 64;
        const DIAGNOSTIC_WORK: usize = HIERARCHY_WORK * (THOROUGHNESS * EDGE_COUNT + 1);

        let target = ProcessorKind::PortListSorter;
        let graph = high_thoroughness_diagnostic_graph();

        let mut prepared = graph.clone();
        let mut prefix = BudgetWorkControl::new(usize::MAX);
        execute_ported_compound_processors_until_processor_with_work_control(
            &mut prepared,
            target,
            &mut prefix,
        )
        .unwrap();
        assert_eq!(hierarchy_work_units(&prepared), Ok(HIERARCHY_WORK));
        assert_eq!(
            hierarchy_processor_work_units(
                &prepared,
                ProcessorKind::LayerSweepCrossingMinimizerBarycenter,
            ),
            Ok(HIERARCHY_WORK * THOROUGHNESS * EDGE_COUNT)
        );
        let required = checked_add(prefix.charged, DIAGNOSTIC_WORK).unwrap();

        let mut below_graph = graph.clone();
        let mut below = BudgetWorkControl::new(required - 1);
        assert_eq!(
            inspect_compound_crossings_after_processor_with_work_control(
                &mut below_graph,
                target,
                &mut below,
            ),
            Err(PipelineError::Work(WorkError::Interrupted))
        );
        assert_eq!(below_graph, prepared);
        assert_eq!(below.charged, prefix.charged);
        assert_eq!(below.remaining, DIAGNOSTIC_WORK - 1);

        let mut exact_graph = graph.clone();
        let mut exact = BudgetWorkControl::new(required);
        let (_, exact_trace) = inspect_compound_crossings_after_processor_with_work_control(
            &mut exact_graph,
            target,
            &mut exact,
        )
        .unwrap();
        assert_eq!(
            exact_trace.as_ref().map(|trace| trace.runs.len()),
            Some(THOROUGHNESS)
        );
        assert_eq!(exact.charged, required);
        assert_eq!(exact.remaining, 0);

        let mut above_graph = graph;
        let mut above = BudgetWorkControl::new(required + 1);
        let above_result = inspect_compound_crossings_after_processor_with_work_control(
            &mut above_graph,
            target,
            &mut above,
        )
        .unwrap();
        assert_eq!(above_graph, exact_graph);
        assert_eq!(above_result.1, exact_trace);
        assert_eq!(above.charged, required);
        assert_eq!(above.remaining, 1);
    }

    #[test]
    fn processor_work_rejects_configured_iteration_overflow_before_execution() {
        let mut graph = LGraph::new(
            "root",
            LayeredOptions {
                thoroughness: usize::MAX,
                ..LayeredOptions::default()
            },
        );
        graph
            .layerless_nodes
            .push(LNode::new("A", 80.0, 40.0, None));

        assert_eq!(
            processor_work_units(&graph, ProcessorKind::NetworkSimplexLayerer),
            Err(WorkError::ArithmeticOverflow)
        );
    }

    #[test]
    fn full_runner_rejects_unported_strategies_before_mutating_graph() {
        let cases = [
            (
                LayeredOptions {
                    layering_strategy: LayeringStrategy::LongestPath,
                    ..LayeredOptions::default()
                },
                ProcessorKind::LongestPathLayerer,
            ),
            (
                LayeredOptions {
                    crossing_minimization_strategy: CrossingMinimizationStrategy::Interactive,
                    ..LayeredOptions::default()
                },
                ProcessorKind::InteractiveCrossingMinimizer,
            ),
            (
                LayeredOptions {
                    node_placement_strategy: NodePlacementStrategy::Interactive,
                    ..LayeredOptions::default()
                },
                ProcessorKind::InteractiveNodePlacer,
            ),
            (
                LayeredOptions {
                    edge_routing: EdgeRouting::Polyline,
                    ..LayeredOptions::default()
                },
                ProcessorKind::PolylineEdgeRouter,
            ),
            (
                LayeredOptions {
                    wrapping_strategy: WrappingStrategy::SingleEdge,
                    ..LayeredOptions::default()
                },
                ProcessorKind::SingleEdgeGraphWrapper,
            ),
        ];

        for (options, expected) in cases {
            let mut graph = import_graph(&ElkInputGraph {
                id: "root".to_string(),
                options,
                nodes: vec![node("A"), node("B")],
                edges: vec![edge("A-B", "A", "B")],
            })
            .unwrap();
            let before = graph.clone();

            let result = execute_ported_processors(&mut graph);

            assert_eq!(
                result,
                Err(PipelineError::UnsupportedProcessor { kind: expected })
            );
            assert_eq!(
                graph, before,
                "{expected:?} mutated the graph before failing"
            );
        }
    }

    #[test]
    fn full_runner_charges_validation_before_rejecting_an_unported_processor() {
        let graph = import_graph(&ElkInputGraph {
            id: "root".to_string(),
            options: LayeredOptions {
                layering_strategy: LayeringStrategy::LongestPath,
                ..LayeredOptions::default()
            },
            nodes: vec![node("A"), node("B")],
            edges: vec![edge("A-B", "A", "B")],
        })
        .unwrap();
        let required = hierarchy_work_units(&graph).unwrap();

        let mut below_graph = graph.clone();
        let mut below = BudgetWorkControl::new(required - 1);
        assert_eq!(
            execute_ported_processors_with_work_control(&mut below_graph, &mut below),
            Err(PipelineError::Work(WorkError::Interrupted))
        );
        assert_eq!(below_graph, graph);
        assert_eq!(below.charged, 0);

        let mut exact_graph = graph.clone();
        let mut exact = BudgetWorkControl::new(required);
        assert_eq!(
            execute_ported_processors_with_work_control(&mut exact_graph, &mut exact),
            Err(PipelineError::UnsupportedProcessor {
                kind: ProcessorKind::LongestPathLayerer,
            })
        );
        assert_eq!(exact_graph, graph);
        assert_eq!(exact.charged, required);
        assert_eq!(exact.remaining, 0);
    }

    #[test]
    fn compound_full_runner_rejects_child_unported_processor_before_mutation() {
        let mut graph = deep_compound_chain(2);
        graph.layerless_nodes[0]
            .nested_graph
            .as_deref_mut()
            .expect("the fixture should contain one child graph")
            .options
            .layering_strategy = LayeringStrategy::LongestPath;
        let before = graph.clone();

        assert_eq!(
            execute_ported_compound_processors(&mut graph),
            Err(PipelineError::UnsupportedProcessor {
                kind: ProcessorKind::LongestPathLayerer,
            })
        );
        assert_eq!(graph, before);
    }

    #[test]
    fn full_runner_reports_first_unported_processor_in_pipeline_order() {
        let mut graph = import_graph(&ElkInputGraph {
            id: "root".to_string(),
            options: LayeredOptions {
                layering_strategy: LayeringStrategy::LongestPath,
                node_placement_strategy: NodePlacementStrategy::Interactive,
                edge_routing: EdgeRouting::Splines,
                wrapping_strategy: WrappingStrategy::MultiEdge,
                ..LayeredOptions::default()
            },
            nodes: vec![node("A"), node("B")],
            edges: vec![edge("A-B", "A", "B")],
        })
        .unwrap();

        assert_eq!(
            execute_ported_processors(&mut graph),
            Err(PipelineError::UnsupportedProcessor {
                kind: ProcessorKind::LongestPathLayerer,
            })
        );
    }

    #[test]
    fn compound_ported_processor_preflight_is_small_stack_safe() {
        const GRAPH_COUNT: usize = 4_096;

        std::thread::Builder::new()
            .name("elk-ported-processor-preflight-small-stack".to_string())
            .stack_size(64 * 1024)
            .spawn(|| {
                let mut graph = deep_compound_chain(GRAPH_COUNT);
                let mut leaf = &mut graph;
                for _ in 1..GRAPH_COUNT {
                    leaf = leaf.layerless_nodes[0]
                        .nested_graph
                        .as_deref_mut()
                        .expect("the deep compound fixture should retain every child graph");
                }
                leaf.options.layering_strategy = LayeringStrategy::LongestPath;

                let graph = std::mem::ManuallyDrop::new(graph);
                assert_eq!(
                    validate_ported_graph_processors(&graph),
                    Err(PipelineError::UnsupportedProcessor {
                        kind: ProcessorKind::LongestPathLayerer,
                    })
                );
            })
            .expect("the small-stack preflight thread should start")
            .join()
            .expect("ported processor validation must not recurse with hierarchy depth");
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
    fn mermaid_reachable_flowchart_elk_processors_are_source_ported() {
        for direction in [
            ElkDirection::Right,
            ElkDirection::Left,
            ElkDirection::Down,
            ElkDirection::Up,
        ] {
            assert_processors_are_source_ported(
                &format!("Mermaid flowchart direction {direction:?}"),
                kinds(&LayeredOptions::mermaid_flowchart_defaults(direction)),
            );
        }

        for cycle_breaking_strategy in [
            CycleBreakingStrategy::Greedy,
            CycleBreakingStrategy::DepthFirst,
            CycleBreakingStrategy::Interactive,
            CycleBreakingStrategy::ModelOrder,
            CycleBreakingStrategy::GreedyModelOrder,
        ] {
            assert_processors_are_source_ported(
                &format!("public cycleBreakingStrategy {cycle_breaking_strategy:?}"),
                kinds(&LayeredOptions {
                    cycle_breaking_strategy,
                    ..LayeredOptions::mermaid_flowchart_defaults(ElkDirection::Down)
                }),
            );
        }

        for node_placement_strategy in [
            NodePlacementStrategy::BrandesKoepf,
            NodePlacementStrategy::Simple,
            NodePlacementStrategy::LinearSegments,
            NodePlacementStrategy::NetworkSimplex,
        ] {
            assert_processors_are_source_ported(
                &format!("public nodePlacementStrategy {node_placement_strategy:?}"),
                kinds(&LayeredOptions {
                    node_placement_strategy,
                    ..LayeredOptions::mermaid_flowchart_defaults(ElkDirection::Down)
                }),
            );
        }

        for node_placement_bk_fixed_alignment in [
            FixedAlignment::None,
            FixedAlignment::LeftUp,
            FixedAlignment::RightUp,
            FixedAlignment::LeftDown,
            FixedAlignment::RightDown,
            FixedAlignment::Balanced,
        ] {
            assert_processors_are_source_ported(
                &format!("public nodePlacementAlignment {node_placement_bk_fixed_alignment:?}"),
                kinds(&LayeredOptions {
                    node_placement_bk_fixed_alignment,
                    ..LayeredOptions::mermaid_flowchart_defaults(ElkDirection::Down)
                }),
            );
        }

        for consider_model_order_strategy in [
            OrderingStrategy::None,
            OrderingStrategy::NodesAndEdges,
            OrderingStrategy::PreferEdges,
            OrderingStrategy::PreferNodes,
        ] {
            assert_processors_are_source_ported(
                &format!("public considerModelOrder {consider_model_order_strategy:?}"),
                kinds(&LayeredOptions {
                    consider_model_order_strategy,
                    ..LayeredOptions::mermaid_flowchart_defaults(ElkDirection::Down)
                }),
            );
        }

        assert_processors_are_source_ported(
            "public forceNodeModelOrder",
            kinds(&LayeredOptions {
                force_node_model_order: true,
                ..LayeredOptions::mermaid_flowchart_defaults(ElkDirection::Down)
            }),
        );

        let merge_edges_graph = import_graph(&ElkInputGraph {
            id: "root".to_string(),
            options: LayeredOptions {
                merge_edges: true,
                ..LayeredOptions::mermaid_flowchart_defaults(ElkDirection::Down)
            },
            nodes: vec![node("A"), node("B"), node("C")],
            edges: vec![
                edge("A-C-1", "A", "C"),
                edge("A-C-2", "A", "C"),
                edge("B-C", "B", "C"),
            ],
        })
        .unwrap();
        assert_processors_are_source_ported(
            "public mergeEdges hyperedge graph",
            graph_kinds(&merge_edges_graph),
        );

        let mut constrained_entry = node("A");
        constrained_entry.layer_constraint = Some(crate::options::LayerConstraint::First);
        let keep_entry_graph = import_graph(&ElkInputGraph {
            id: "root".to_string(),
            options: LayeredOptions::mermaid_flowchart_defaults(ElkDirection::Down),
            nodes: vec![constrained_entry, node("B"), node("C")],
            edges: vec![edge("A-B", "A", "B"), edge("B-C", "B", "C")],
        })
        .unwrap();
        assert_processors_are_source_ported(
            "public keepEntryNodeOnTop layer constraint",
            graph_kinds(&keep_entry_graph),
        );

        let mut head = ElkInputLabel::center("head", 20.0, 10.0);
        head.placement = crate::graph::EdgeLabelPlacement::Head;
        let mut end_label_edge = edge("A-B", "A", "B");
        end_label_edge.label = Some(head);
        let label_and_self_loop_graph = import_graph(&ElkInputGraph {
            id: "root".to_string(),
            options: LayeredOptions::mermaid_flowchart_defaults(ElkDirection::Down),
            nodes: vec![node("A"), node("B")],
            edges: vec![
                ElkInputEdge {
                    label: Some(ElkInputLabel::center("center", 28.0, 12.0)),
                    ..edge("A-A", "A", "A")
                },
                end_label_edge,
            ],
        })
        .unwrap();
        assert_processors_are_source_ported(
            "Mermaid flowchart labels and self loops",
            graph_kinds(&label_and_self_loop_graph),
        );

        let mut cluster = node("cluster");
        cluster.hierarchy_handling = Some(crate::options::HierarchyHandling::IncludeChildren);
        let mut child = node("A");
        child.parent = Some("cluster".to_string());
        let mut compound_graph = import_graph(&ElkInputGraph {
            id: "root".to_string(),
            options: LayeredOptions::mermaid_flowchart_defaults(ElkDirection::Down),
            nodes: vec![cluster, child, node("B")],
            edges: vec![edge("A-B", "A", "B")],
        })
        .unwrap();
        preprocess_source_ported_compound_graph(&mut compound_graph);
        assert_processors_are_source_ported(
            "Mermaid flowchart compound root graph",
            graph_kinds(&compound_graph),
        );
        let nested_graph = compound_graph.layerless_nodes[0]
            .nested_graph
            .as_ref()
            .expect("compound fixture should create a nested graph");
        assert_processors_are_source_ported(
            "Mermaid flowchart compound child graph",
            graph_kinds(nested_graph),
        );
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
    fn documented_node_placement_strategies_assemble_public_processors() {
        let cases = [
            (
                NodePlacementStrategy::BrandesKoepf,
                ProcessorKind::BKNodePlacer,
            ),
            (
                NodePlacementStrategy::Simple,
                ProcessorKind::SimpleNodePlacer,
            ),
            (
                NodePlacementStrategy::LinearSegments,
                ProcessorKind::LinearSegmentsNodePlacer,
            ),
            (
                NodePlacementStrategy::NetworkSimplex,
                ProcessorKind::NetworkSimplexPlacer,
            ),
        ];

        for (strategy, expected_processor) in cases {
            let options = LayeredOptions {
                node_placement_strategy: strategy,
                ..LayeredOptions::mermaid_flowchart_defaults(ElkDirection::Down)
            };

            assert!(
                kinds(&options).contains(&expected_processor),
                "{strategy:?} should assemble {expected_processor:?}"
            );
        }
    }

    #[test]
    fn merge_edges_hyperedge_graph_assembles_dummy_merger() {
        let graph = import_graph(&ElkInputGraph {
            id: "root".to_string(),
            options: LayeredOptions {
                merge_edges: true,
                ..LayeredOptions::mermaid_flowchart_defaults(ElkDirection::Down)
            },
            nodes: vec![node("A"), node("B"), node("C")],
            edges: vec![
                edge("A-C-1", "A", "C"),
                edge("A-C-2", "A", "C"),
                edge("B-C", "B", "C"),
            ],
        })
        .unwrap();

        assert!(graph.options.graph_has_hyperedges);
        assert!(
            graph_kinds(&graph).contains(&ProcessorKind::HyperedgeDummyMerger),
            "mergeEdges hyperedge graph should assemble HyperedgeDummyMerger"
        );
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
                    model_order: None,
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
                    model_order: None,
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
    fn inside_self_loops_enable_self_loop_processors_on_nested_graphs() {
        let mut input = ElkInputGraph {
            id: "root".to_string(),
            options: LayeredOptions::mermaid_flowchart_defaults(ElkDirection::Down),
            nodes: vec![ElkInputNode {
                id: "A".to_string(),
                width: 80.0,
                height: 40.0,
                parent: None,
                direction: None,
                hierarchy_handling: Some(crate::options::HierarchyHandling::IncludeChildren),
                layer_constraint: None,
                port_constraints: None,
                node_label_placement: crate::options::NodeLabelPlacement::Fixed,
                nested_spacing_base: None,
                label: None,
            }],
            edges: vec![ElkInputEdge {
                id: "A-A".to_string(),
                source: "A".to_string(),
                target: "A".to_string(),
                label: None,
                minlen: 1,
                inside_self_loops_yo: true,
                model_order: None,
                priority_direction: 0,
                priority_shortness: 0,
                priority_straightness: 0,
            }],
        };
        input.options.inside_self_loops_activate = true;

        let mut graph = import_graph(&input).unwrap();

        preprocess_source_ported_compound_graph(&mut graph);

        let nested = graph.layerless_nodes[0]
            .nested_graph
            .as_ref()
            .expect("inside self-loop should create a nested graph");
        let processors = graph_kinds(nested);

        assert!(processors.contains(&ProcessorKind::SelfLoopPreProcessor));
        assert!(processors.contains(&ProcessorKind::SelfLoopPortRestorer));
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
