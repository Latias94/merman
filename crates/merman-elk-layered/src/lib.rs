//! Source-backed Eclipse ELK layered port.
//!
//! This module mirrors the structure of Eclipse ELK instead of the old compatibility backend:
//! - https://github.com/eclipse-elk/elk/blob/62d5909f96fad541bc101ad52dabaece6b7eab7e/plugins/org.eclipse.elk.alg.layered/src/org/eclipse/elk/alg/layered/LayeredPhases.java
//! - https://github.com/eclipse-elk/elk/blob/62d5909f96fad541bc101ad52dabaece6b7eab7e/plugins/org.eclipse.elk.alg.layered/src/org/eclipse/elk/alg/layered/GraphConfigurator.java
//! - https://github.com/eclipse-elk/elk/blob/62d5909f96fad541bc101ad52dabaece6b7eab7e/plugins/org.eclipse.elk.alg.layered/src/org/eclipse/elk/alg/layered/intermediate/IntermediateProcessorStrategy.java
//! - https://github.com/eclipse-elk/elk/tree/62d5909f96fad541bc101ad52dabaece6b7eab7e/plugins/org.eclipse.elk.alg.layered/src/org/eclipse/elk/alg/layered/graph

pub mod common;
pub mod compound;
pub mod configurator;
pub mod graph;
pub mod importer;
pub mod intermediate;
pub mod options;
pub mod p1cycles;
pub mod p2layers;
pub mod p3order;
pub mod p4nodes;
pub mod p5edges;
pub mod pipeline;
pub mod random;
pub mod transform;

pub use compound::compare_compound_segments;
pub use configurator::{LayeredSpacings, configure_graph_properties, configured_options};
pub use graph::{
    CompoundEdgeSegment, CrossHierarchyEdge, EdgeLabelPlacement, GraphProperties, LGraph, LLabel,
    LMargin, LNode, LNodeKind, LPadding, LPoint, LPort, LSize, LabelSide, Layer, LayeredEdge,
    PortRef, PortSide, PortType, create_external_port_dummy, reverse_edge,
};
pub use importer::{
    ElkInputEdge, ElkInputGraph, ElkInputLabel, ElkInputNode, ImportError, ImportResult,
    import_graph,
};
pub use intermediate::{
    IntermediateError, IntermediateResult, calculate_layer_sizes_and_graph_height,
    insert_label_dummies, postprocess_layer_constraints, preprocess_layer_constraints,
    process_hierarchical_port_constraints, process_hierarchical_port_dummy_sizes,
    process_hierarchical_port_orthogonal_edges, process_hierarchical_port_positions,
    process_inverted_ports, remove_label_dummies, restore_reversed_edges,
    reverse_edges_for_edge_and_layer_constraints, select_label_sides, split_edge, split_long_edges,
    switch_label_dummies,
};
pub use options::{
    Alignment, CycleBreakingStrategy, DirectionCongruency, EdgeLabelSideSelection, EdgeRouting,
    ElkDirection, ElkPadding, GreedySwitchType, HierarchyHandling, LayerConstraint, LayeredOptions,
    LongEdgeOrderingStrategy, NodeLabelPlacement, NodePlacementStrategy, OrderingStrategy,
    PortConstraints, PortSortingStrategy, SelfLoopDistributionStrategy, SpacingOptions,
};
pub use p2layers::layer_network_simplex;
pub use p3order::counting::CrossingsCounter;
pub use p3order::sweep::{
    CrossMinType, HierarchySweepDebugTrace, HierarchySweepNodeDebug,
    debug_crossings_layer_sweep_hierarchical_with_type, minimize_crossings_layer_sweep,
    minimize_crossings_layer_sweep_with_type,
};
pub use p3order::{
    long_edge_target_node_preprocessing, process_port_sides, set_port_side, sort_by_input_model,
    sort_port_lists, target_node,
};
pub use p4nodes::{
    calculate_innermost_node_margins, calculate_label_and_node_sizes, process_in_layer_constraints,
};
pub use pipeline::{
    GraphExecution, LayeredPhase, PipelineError, PipelineResult, ProcessorKind, ProcessorSlot,
    assemble_processors, assemble_processors_for_graph, execute_ported_compound_processors,
    execute_ported_compound_processors_until, execute_ported_compound_processors_until_processor,
    execute_ported_processors, execute_processors_until, execute_processors_until_processor,
};
pub use random::JavaRandom;
pub use transform::{GraphTransformMode, transform_graph_direction};
