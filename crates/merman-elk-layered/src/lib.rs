//! Source-backed Eclipse ELK layered port.
//!
//! This module mirrors the structure of Eclipse ELK instead of the old compatibility backend:
//! - `repo-ref/elk/plugins/org.eclipse.elk.alg.layered/src/org/eclipse/elk/alg/layered/LayeredPhases.java`
//! - `repo-ref/elk/plugins/org.eclipse.elk.alg.layered/src/org/eclipse/elk/alg/layered/GraphConfigurator.java`
//! - `repo-ref/elk/plugins/org.eclipse.elk.alg.layered/src/org/eclipse/elk/alg/layered/intermediate/IntermediateProcessorStrategy.java`
//! - `repo-ref/elk/plugins/org.eclipse.elk.alg.layered/src/org/eclipse/elk/alg/layered/graph/*.java`

pub mod configurator;
pub mod graph;
pub mod importer;
pub mod options;
pub mod pipeline;

pub use configurator::{LayeredSpacings, configure_graph_properties, configured_options};
pub use graph::{
    EdgeLabelPlacement, GraphProperties, LGraph, LLabel, LNode, LNodeKind, LPadding, LPoint, LPort,
    LSize, Layer, LayeredEdge, PortRef, PortSide, PortType,
};
pub use importer::{
    ElkInputEdge, ElkInputGraph, ElkInputLabel, ElkInputNode, ImportError, ImportResult,
    import_graph,
};
pub use options::{
    EdgeRouting, ElkDirection, HierarchyHandling, LayeredOptions, NodePlacementStrategy,
    SelfLoopDistributionStrategy, SpacingOptions,
};
pub use pipeline::{
    LayeredPhase, ProcessorKind, ProcessorSlot, assemble_processors, assemble_processors_for_graph,
};
