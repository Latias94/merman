#![forbid(unsafe_code)]
#![doc = include_str!("../README.md")]

//! Diagnostics-first analysis contracts and source mapping for Merman.
//!
//! This crate is intentionally render-free. It owns the JSON payload shape and source-position
//! helpers that FFI, UniFFI, WASM, CLI linting, Markdown scanning, and the LSP adapter share.

mod analyzer;
mod cancellation;
mod config_contract;
mod diagnostic_projection;
pub mod document;
mod document_limits;
pub mod editor;
pub mod markdown;
pub mod options_json;
mod payload;
mod recovery;
mod result;
mod retained_weight;
mod rules;
mod source_config_rewrite;
mod source_limits;
mod source_map;
mod status;

pub use analyzer::{
    ANALYSIS_RESOURCE_LIMIT_DESCRIPTORS, AnalysisDiagnosticPolicy, AnalysisEnvironmentIdentity,
    AnalysisOptions, AnalysisResourceLimitDescriptor, AnalysisResourceLimits,
    AnalysisSnapshotPolicy, Analyzer, MAX_DOCUMENT_DIAGRAMS_RESOURCE_LIMIT_ID,
    analysis_resource_profile_value,
};
pub use cancellation::{AnalysisCancellationToken, AnalysisCancelled};
pub use config_contract::{
    AnalysisConfigChange, AnalysisConfigContract, AnalysisConfigHostDefaults,
    AnalysisConfigSchemaProjection, FIXED_TODAY_SCHEMA_PATTERN,
};
pub use document::{
    DocumentDiagram, DocumentDiagramKind, DocumentSource, FenceDelimiter, FenceDelimiterSpans,
    FenceMarker, SharedTextSlice, analyze_document, analyze_document_facts,
    analyze_document_generation, analyze_document_generation_shared,
    analyze_document_generation_shared_cancellable, source_descriptor_for_kind,
    source_descriptor_for_markdown_path, source_descriptor_for_uri, source_language,
};
pub use editor::{ByteSpan, FenceTextIndex, FenceTextIndexSource};
pub use options_json::{
    AnalysisOptionsJson, AnalysisOptionsJsonError, LintOptionsJson, LintRuleSeverityOverrideJson,
    ResourceOptionsJson, analysis_options_from_json_value, analysis_options_json_from_json_value,
};
pub use payload::{
    ANALYSIS_FACTS_PAYLOAD_VERSION, ANALYSIS_PAYLOAD_VERSION, AnalysisDiagnostic,
    AnalysisDiagnosticTag, AnalysisPayload, DiagnosticCategory, DiagnosticFix, DiagnosticFixEdit,
    DiagnosticRelated, DiagnosticSeverity, DiagnosticSpan, LspRange, SourceDescriptor, SourceKind,
    SourcePosition, Summary, Utf16Position,
};
pub use result::{
    AnalysisCaptureOutcome, AnalysisDiagramFacts, AnalysisDiagramSyntaxFacts,
    AnalysisEditorSymbolKind, AnalysisExpectedSyntaxFacts, AnalysisExpectedSyntaxKind,
    AnalysisFactSpan, AnalysisFactsPayload, AnalysisFenceDelimiterFacts, AnalysisGeneration,
    AnalysisLineItemFacts, AnalysisReferenceFacts, AnalysisRejection, AnalysisResourceLimit,
    AnalysisSemanticItemFacts, AnalysisSemanticRole, AnalysisSyntaxFacts, AnalyzedDiagram,
    DiagramParseDisposition,
};
pub use rules::{
    AnalysisRuleConfig, AnalysisRuleConfigError, AnalysisRuleProfile,
    RULE_CATALOG_RESPONSE_VERSION, RuleCatalogEntry, RuleCatalogResponse, RuleDescriptor,
    RuleOrigin, configurable_rule_catalog, configurable_rule_catalog_response,
    configurable_rule_catalog_response_json_bytes, configurable_rule_descriptor,
    configurable_rule_descriptors, rule_catalog, rule_catalog_response,
    rule_catalog_response_json_bytes, rule_descriptors,
};
pub use source_limits::{
    source_discarded_after_limit_change_diagnostic,
    source_discarded_after_limit_change_diagnostic_with_span, source_limit_diagnostic_for_len,
    source_limit_diagnostic_for_len_and_span, source_limit_diagnostic_span,
    source_limit_diagnostic_span_cancellable,
};
pub use source_map::{LineCol, SourceMap, SourceMapError};
pub use status::AnalysisStatus;
