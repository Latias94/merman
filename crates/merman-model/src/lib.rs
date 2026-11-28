#![forbid(unsafe_code)]
//! Canonical Mermaid data and shared controlled execution contracts.
//!
//! This package does not ingest Mermaid source or select an output backend. System adapters are
//! opt-in; default operation runtime values are deterministic.

pub mod baseline;
#[cfg(feature = "benchmark-stages")]
#[doc(hidden)]
pub mod benchmark_stages;
pub mod config;
#[doc(hidden)]
pub mod css_color;
pub mod families;
pub mod family;
#[doc(hidden)]
pub mod grid;
pub mod json;
pub mod number;
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

/// Appends a lowercase hexadecimal representation, preserving leading zeroes.
#[doc(hidden)]
pub fn push_hex_lower(output: &mut String, bytes: &[u8]) {
    const DIGITS: &[u8; 16] = b"0123456789abcdef";
    output.reserve(bytes.len() * 2);
    for &byte in bytes {
        output.push(char::from(DIGITS[usize::from(byte >> 4)]));
        output.push(char::from(DIGITS[usize::from(byte & 0x0f)]));
    }
}
