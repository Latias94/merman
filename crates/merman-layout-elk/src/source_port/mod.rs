//! Source-backed Eclipse ELK layered port.
//!
//! This module mirrors the structure of Eclipse ELK instead of the old compatibility backend:
//! - `repo-ref/elk/plugins/org.eclipse.elk.alg.layered/src/org/eclipse/elk/alg/layered/LayeredPhases.java`
//! - `repo-ref/elk/plugins/org.eclipse.elk.alg.layered/src/org/eclipse/elk/alg/layered/GraphConfigurator.java`
//! - `repo-ref/elk/plugins/org.eclipse.elk.alg.layered/src/org/eclipse/elk/alg/layered/intermediate/IntermediateProcessorStrategy.java`
//! - `repo-ref/elk/plugins/org.eclipse.elk.alg.layered/src/org/eclipse/elk/alg/layered/graph/*.java`

pub mod graph;
pub mod options;
pub mod pipeline;

pub use graph::{LGraph, LNode, LNodeKind, LPort, Layer, LayeredEdge};
pub use options::{
    EdgeRouting, ElkDirection, HierarchyHandling, LayeredOptions, NodePlacementStrategy,
    SelfLoopDistributionStrategy,
};
pub use pipeline::{LayeredPhase, ProcessorKind, ProcessorSlot, assemble_processors};
