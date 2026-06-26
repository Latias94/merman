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
pub mod options_json;
mod payload;
mod rules;
mod source_config_rewrite;
mod source_directives;
mod source_map;
mod status;

pub use analyzer::{AnalysisOptions, Analyzer};
pub use editor::{
    ByteSpan, EditorSymbolKind, FenceCursorCompletionKind, FenceCursorContext, FenceLineItem,
    FenceReferenceGroup, FenceSemanticItem, FenceSemanticRole, FenceTextIndex,
    FenceTextIndexSource,
};
pub use options_json::{
    AnalysisOptionsJson, AnalysisOptionsJsonError, LintOptionsJson, LintRuleSeverityOverrideJson,
    ParseOptionsJson, ResourceOptionsJson, analysis_options_from_json_value,
};
pub use payload::{
    AnalysisDiagnostic, AnalysisPayload, DiagnosticCategory, DiagnosticFix, DiagnosticFixEdit,
    DiagnosticRelated, DiagnosticSeverity, DiagnosticSpan, SourceDescriptor, SourceKind, Summary,
    Utf16Position,
};
pub use rules::{
    AnalysisRuleConfig, AnalysisRuleProfile, RuleCatalogEntry, RuleDescriptor, RuleOrigin,
    configurable_rule_catalog, configurable_rule_catalog_json_bytes, configurable_rule_descriptor,
    configurable_rule_descriptors, rule_catalog, rule_catalog_json_bytes, rule_descriptors,
};
pub use source_map::{LineCol, SourceMap, SourceMapError};
pub use status::AnalysisStatus;
