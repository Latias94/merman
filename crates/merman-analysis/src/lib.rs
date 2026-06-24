#![forbid(unsafe_code)]

//! Diagnostics-first analysis contracts and source mapping for Merman.
//!
//! This crate is intentionally render-free. It owns the JSON payload shape and source-position
//! helpers that FFI, UniFFI, WASM, CLI linting, Markdown scanning, and future LSP adapters can share.

mod analyzer;
pub mod document;
pub mod editor;
pub mod lsp;
pub mod markdown;
mod payload;
mod rules;
mod source_map;
mod status;

pub use analyzer::{AnalysisOptions, Analyzer};
pub use editor::{ByteSpan, EditorSymbolKind, FenceLineItem, FenceTextIndex};
pub use payload::{
    AnalysisDiagnostic, AnalysisPayload, DiagnosticCategory, DiagnosticRelated, DiagnosticSeverity,
    DiagnosticSpan, SourceDescriptor, SourceKind, Summary, Utf16Position,
};
pub use source_map::{LineCol, SourceMap, SourceMapError};
pub use status::AnalysisStatus;
