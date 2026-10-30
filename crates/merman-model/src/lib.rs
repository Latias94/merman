#![forbid(unsafe_code)]
//! Canonical Mermaid data and shared controlled execution contracts.
//!
//! This package does not ingest Mermaid source or select an output backend. System adapters are
//! opt-in; default operation runtime values are deterministic.

pub mod baseline;
pub mod config;
pub mod families;
pub mod family;
pub mod json;
pub mod operation;
#[cfg(feature = "presentation-data")]
pub mod presentation;
pub mod resources;
pub mod runtime;
pub mod semantic;
pub mod source;
pub mod text;
pub mod time;
pub mod warnings;

pub use config::MermaidConfig;
pub use json::ManagedJson;
pub use operation::{
    CancelReason, OperationCancelled, OperationControl, OperationControlResult,
    OperationLedgerError, OperationPhase, OperationResourceDomain, OperationResourceLimitExceeded,
    OperationResourceOverride, OperationResourceProvenance,
};
pub use source::SourceSpan;

pub use semantic::{ParsedSemanticDiagram, SemanticDiagramModel, SemanticMetadata};

/// Compatibility ceiling for nested Mermaid diagram structures.
pub const MAX_DIAGRAM_NESTING_DEPTH: usize = 256;
