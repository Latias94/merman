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

use super::options::{
    CrossingMinimizationStrategy, CycleBreakingStrategy, EdgeRouting, ElkDirection,
    GreedySwitchType, LayeredOptions, LayeringStrategy, NodePlacementStrategy, OrderingStrategy,
    WrappingStrategy,
};
use crate::configurator::configured_options;
use crate::graph::LGraph;

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
    use crate::importer::{ElkInputEdge, ElkInputGraph, ElkInputLabel, ElkInputNode, import_graph};
    use crate::options::{ElkDirection, LayeredOptions};

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
    fn graph_properties_insert_label_self_loop_and_external_port_processors() {
        let graph = import_graph(&ElkInputGraph {
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
                    label: None,
                },
                ElkInputNode {
                    id: "A".to_string(),
                    width: 80.0,
                    height: 40.0,
                    parent: Some("cluster".to_string()),
                    direction: None,
                    hierarchy_handling: None,
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
                    priority_direction: 0,
                },
                ElkInputEdge {
                    id: "A-A".to_string(),
                    source: "A".to_string(),
                    target: "A".to_string(),
                    label: None,
                    minlen: 1,
                    priority_direction: 0,
                },
            ],
        })
        .unwrap();

        let nested = graph.layerless_nodes[0].nested_graph.as_ref().unwrap();
        let processors = graph_kinds(nested);
        assert!(processors.contains(&ProcessorKind::HierarchicalPortConstraintProcessor));
        assert!(processors.contains(&ProcessorKind::HierarchicalPortDummySizeProcessor));
        assert!(processors.contains(&ProcessorKind::HierarchicalPortOrthogonalEdgeRouter));
        assert!(processors.contains(&ProcessorKind::LabelDummyInserter));
        assert!(processors.contains(&ProcessorKind::LabelDummySwitcher));
        assert!(processors.contains(&ProcessorKind::LabelDummyRemover));
        assert!(processors.contains(&ProcessorKind::SelfLoopPreProcessor));
        assert!(processors.contains(&ProcessorKind::SelfLoopRouter));
        assert!(processors.contains(&ProcessorKind::SelfLoopPostProcessor));
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
}
