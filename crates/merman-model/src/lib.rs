#![forbid(unsafe_code)]
//! Canonical Mermaid data and shared controlled execution contracts.
//!
//! This package does not ingest Mermaid source or select an output backend. System adapters are
//! opt-in; default operation runtime values are deterministic.

pub mod config;
pub mod json;
pub mod operation;
pub mod resources;
pub mod runtime;
pub mod source;
pub mod time;

pub use config::MermaidConfig;
pub use json::ManagedJson;
pub use operation::{
    CancelReason, OperationCancelled, OperationControl, OperationControlResult,
    OperationLedgerError, OperationPhase, OperationResourceDomain, OperationResourceLimitExceeded,
    OperationResourceOverride, OperationResourceProvenance,
};
pub use source::SourceSpan;
