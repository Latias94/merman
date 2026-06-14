//! Source-backed Eclipse ELK layered port.
//!
//! This module mirrors the structure of Eclipse ELK instead of the old compatibility backend:
//! - https://github.com/eclipse-elk/elk/blob/62d5909f96fad541bc101ad52dabaece6b7eab7e/plugins/org.eclipse.elk.alg.layered/src/org/eclipse/elk/alg/layered/LayeredPhases.java
//! - https://github.com/eclipse-elk/elk/blob/62d5909f96fad541bc101ad52dabaece6b7eab7e/plugins/org.eclipse.elk.alg.layered/src/org/eclipse/elk/alg/layered/GraphConfigurator.java
//! - https://github.com/eclipse-elk/elk/blob/62d5909f96fad541bc101ad52dabaece6b7eab7e/plugins/org.eclipse.elk.alg.layered/src/org/eclipse/elk/alg/layered/intermediate/IntermediateProcessorStrategy.java
//! - https://github.com/eclipse-elk/elk/tree/62d5909f96fad541bc101ad52dabaece6b7eab7e/plugins/org.eclipse.elk.alg.layered/src/org/eclipse/elk/alg/layered/graph

pub mod common;
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

pub use configurator::{LayeredSpacings, configure_graph_properties, configured_options};
pub use graph::{
    EdgeLabelPlacement, GraphProperties, LGraph, LLabel, LMargin, LNode, LNodeKind, LPadding,
    LPoint, LPort, LSize, Layer, LayeredEdge, PortRef, PortSide, PortType, reverse_edge,
};
pub use importer::{
    ElkInputEdge, ElkInputGraph, ElkInputLabel, ElkInputNode, ImportError, ImportResult,
    import_graph,
};
pub use intermediate::{
    IntermediateError, IntermediateResult, calculate_layer_sizes_and_graph_height,
    postprocess_layer_constraints, preprocess_layer_constraints, restore_reversed_edges,
    reverse_edges_for_edge_and_layer_constraints, split_edge, split_long_edges,
};
pub use options::{
    CycleBreakingStrategy, DirectionCongruency, EdgeRouting, ElkDirection, GreedySwitchType,
    HierarchyHandling, LayerConstraint, LayeredOptions, LongEdgeOrderingStrategy,
    NodePlacementStrategy, OrderingStrategy, PortConstraints, PortSortingStrategy,
    SelfLoopDistributionStrategy, SpacingOptions,
};
pub use p2layers::layer_network_simplex;
pub use p3order::counting::CrossingsCounter;
pub use p3order::sweep::{
    CrossMinType, minimize_crossings_layer_sweep, minimize_crossings_layer_sweep_with_type,
};
pub use p3order::{
    long_edge_target_node_preprocessing, process_port_sides, set_port_side, sort_by_input_model,
    sort_port_lists, target_node,
};
pub use p4nodes::{
    calculate_innermost_node_margins, calculate_label_and_node_sizes, process_in_layer_constraints,
};
pub use pipeline::{
    LayeredPhase, PipelineError, PipelineResult, ProcessorKind, ProcessorSlot, assemble_processors,
    assemble_processors_for_graph, execute_ported_processors, execute_processors_until,
};
pub use random::JavaRandom;
pub use transform::{GraphTransformMode, transform_graph_direction};
