//! Source-backed Eclipse ELK layered port.
//!
//! This module mirrors the structure of Eclipse ELK instead of the old compatibility backend:
//! - https://github.com/eclipse-elk/elk/blob/62d5909f96fad541bc101ad52dabaece6b7eab7e/plugins/org.eclipse.elk.alg.layered/src/org/eclipse/elk/alg/layered/LayeredPhases.java
//! - https://github.com/eclipse-elk/elk/blob/62d5909f96fad541bc101ad52dabaece6b7eab7e/plugins/org.eclipse.elk.alg.layered/src/org/eclipse/elk/alg/layered/GraphConfigurator.java
//! - https://github.com/eclipse-elk/elk/blob/62d5909f96fad541bc101ad52dabaece6b7eab7e/plugins/org.eclipse.elk.alg.layered/src/org/eclipse/elk/alg/layered/intermediate/IntermediateProcessorStrategy.java
//! - https://github.com/eclipse-elk/elk/tree/62d5909f96fad541bc101ad52dabaece6b7eab7e/plugins/org.eclipse.elk.alg.layered/src/org/eclipse/elk/alg/layered/graph

mod common;
mod compound;
mod configurator;
mod graph;
mod importer;
mod intermediate;
mod options;
mod p1cycles;
mod p2layers;
mod p3order;
mod p4nodes;
mod p5edges;
mod pipeline;
mod random;
mod selfloops;
mod transform;
mod work;

// The source-port graph and phase implementations are deliberately private. A raw `LGraph`
// may carry ELK's `randomSeed = 0` sentinel, so only public pipeline entry points are allowed to
// execute it: they configure the graph first and either fail closed or resolve it from an
// operation-owned seed. Keep phase helpers crate-private even when they are useful for parity
// work; diagnostics must go through the guarded pipeline APIs below.
pub use configurator::{LayeredSpacings, configure_graph_properties, configured_options};
pub use graph::{
    CompoundEdgeSegment, CrossHierarchyEdge, EdgeLabelPlacement, GraphProperties, LGraph, LLabel,
    LMargin, LNode, LNodeKind, LPadding, LPoint, LPort, LSize, LabelSide, Layer, LayeredEdge,
    PortRef, PortSide, PortType, SelfHyperLoop, SelfHyperLoopLabels, SelfLoopEdge, SelfLoopHolder,
    SelfLoopLabelAlignment, SelfLoopLabelRef, SelfLoopPort, SelfLoopType,
};
pub use importer::{
    ElkInputEdge, ElkInputEdgeSegment, ElkInputEdgeSegmentEndpoint, ElkInputGraph, ElkInputLabel,
    ElkInputNode, ImportError, ImportResult, import_graph,
    import_graph_at_scope_and_segments_with_work_control,
    import_graph_at_seed_scope_and_segments_with_work_control, import_graph_with_operation_seed,
    import_graph_with_operation_seed_and_work_control, import_graph_with_operation_seed_at_scope,
    import_graph_with_operation_seed_at_scope_and_segments_with_work_control,
    import_graph_with_operation_seed_at_seed_scope_and_segments_with_work_control,
    import_graph_with_work_control,
};
pub use intermediate::IntermediateError;
pub use options::{
    Alignment, CycleBreakingStrategy, DirectionCongruency, EdgeLabelSideSelection, EdgeRouting,
    ElkDirection, ElkPadding, FixedAlignment, GreedySwitchType, HierarchyHandling, LayerConstraint,
    LayeredOptions, LongEdgeOrderingStrategy, NodeLabelPlacement, NodePlacementStrategy,
    OrderingStrategy, PortConstraints, PortSortingStrategy, SelfLoopDistributionStrategy,
    SelfLoopOrderingStrategy, SpacingOptions,
};
pub use p3order::sweep::{HierarchySweepDebugTrace, HierarchySweepNodeDebug};
pub use pipeline::{
    GraphExecution, LayeredPhase, PipelineError, PipelineResult, ProcessorKind, ProcessorSlot,
    assemble_processors, assemble_processors_for_graph, execute_ported_compound_processors,
    execute_ported_compound_processors_until, execute_ported_compound_processors_until_processor,
    execute_ported_compound_processors_until_processor_with_work_control,
    execute_ported_compound_processors_until_with_work_control,
    execute_ported_compound_processors_with_work_control, execute_ported_processors,
    execute_ported_processors_with_work_control, execute_processors_until,
    execute_processors_until_processor, execute_processors_until_processor_with_work_control,
    execute_processors_until_with_work_control, inspect_compound_crossings_after_processor,
    inspect_compound_crossings_after_processor_with_work_control,
};
pub use random::{GraphSeedScope, OperationSeed, RandomSeedError};
pub use work::{NoopWorkControl, WorkControl, WorkError};

/// Phase implementations are intentionally not public. In particular, this must fail to compile:
/// a raw graph has not crossed the fallible configuration boundary that resolves or rejects
/// ELK's `randomSeed = 0` sentinel.
///
/// ```compile_fail
/// use merman_elk_layered::{LGraph, LayeredOptions};
///
/// let mut graph = LGraph::new(
///     "root",
///     LayeredOptions {
///         random_seed: 0,
///         ..Default::default()
///     },
/// );
/// merman_elk_layered::p1cycles::break_cycles_greedy(&mut graph);
/// ```
const _RAW_PHASES_ARE_NOT_PUBLIC: () = ();
