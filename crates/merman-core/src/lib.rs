#![forbid(unsafe_code)]
//! Mermaid parser + semantic model (headless).
//!
//! Design goals:
//! - 1:1 parity with the repository's pinned upstream Mermaid baseline
//! - deterministic, testable outputs (semantic snapshot goldens)
//! - runtime-agnostic async APIs (no specific executor required)
//!
//! Default features are empty. Select `diagram-*` features for the required logical families or
//! `all-diagrams` for the complete parser surface. Family-exclusive public model types and enum
//! variants are conditional; recognition and custom registries remain available without a family.

pub mod baseline;
pub mod common;
pub mod common_db;
mod compatibility_json;
pub mod config;
pub mod detect;
pub mod diagram;
pub mod diagrams;
pub mod editor;
pub mod entities;
pub mod error;
mod family;
pub mod generated;
#[cfg(any(
    test,
    feature = "diagram-flowchart",
    feature = "diagram-swimlane",
    feature = "diagram-agentflow",
    feature = "diagram-kanban",
    feature = "diagram-sequence"
))]
mod inline_config;
mod json_value;
pub mod models;
pub mod operation;
mod parse_diagnostic;
pub mod preprocess;
pub mod presentation;
pub mod resources;
pub mod runtime;
pub mod sanitize;
#[cfg(any(
    feature = "diagram-flowchart",
    feature = "diagram-swimlane",
    feature = "diagram-sequence",
    feature = "diagram-state",
    feature = "diagram-timeline",
    feature = "diagram-cynefin",
    feature = "diagram-info",
    feature = "diagram-journey",
    feature = "diagram-pie"
))]
pub mod semantic;
mod source_span;
pub mod style;
pub mod svg_security;
#[doc(hidden)]
pub mod terminal_text;
mod theme;
pub mod theme_color;
mod theme_id;
pub mod time;
pub mod utils;
mod yaml_config;

pub use compatibility_json::ManagedSemanticJson;
pub use config::MermaidConfig;
pub use detect::{Detector, DetectorRegistry};
pub use diagram::{
    AGENTFLOW_CONTAINMENT_VIOLATION_WARNING_RULE_ID, AGENTFLOW_SHAPE_REMOVED_WARNING_RULE_ID,
    AGENTFLOW_SHAPE_UNSUPPORTED_WARNING_RULE_ID, BLOCK_WIDTH_WARNING_RULE_ID,
    BuiltinRenderSemantic, CapturedPanic, CustomJsonProvenance, CustomJsonRenderModel,
    CustomJsonRenderParser, DiagramParseOutcome, DiagramParseSnapshot, DiagramRegistry,
    DiagramSemanticParser, DiagramSnapshotCapture, DiagramWarningFact,
    FLOWCHART_EXPLICIT_DIRECTION_WARNING_RULE_ID, FLOWCHART_UNKNOWN_STYLE_TARGET_WARNING_RULE_ID,
    GIT_GRAPH_DUPLICATE_COMMIT_WARNING_RULE_ID, ParsedDiagram, ParsedDiagramRender,
    ParsedEditorFacts, RenderDiagramRegistry, RenderSemanticModel,
};
pub use editor::{
    EditorExpectedSyntax, EditorExpectedSyntaxKind, EditorFamilySemantics, EditorRenamePolicy,
    EditorSemanticCompleteness, EditorSemanticDiagnostic, EditorSemanticDiagnosticKind,
    EditorSemanticFacts, EditorSemanticKind, EditorSemanticRole, EditorSemanticSymbol,
};
pub use error::{
    Error, InternalFailure, ParseDiagnostic, ParseDiagnosticSpanKind, Result,
    ThemeEvaluationLimitExceeded,
};
pub use family::{
    BuiltInTypedRenderFamily, DiagramFamilyCapability, DiagramFamilyId, DiagramFamilySelector,
    DiagramHeaderFact, diagram_type_family_id, diagram_type_family_kind, diagram_type_metadata_id,
    diagram_type_render_model_kind,
};
pub use operation::{
    CancelReason, OperationCancelled, OperationControl, OperationControlResult,
    OperationLedgerError, OperationPhase, OperationResourceDomain, OperationResourceLimitExceeded,
    OperationResourceOverride, OperationResourceProvenance,
};
pub use preprocess::{
    PreprocessResult, PreprocessedSource, preprocess_diagram, preprocess_diagram_with_known_type,
};
#[cfg(test)]
use presentation::PresentationConfig;
#[cfg(any(
    feature = "diagram-flowchart",
    feature = "diagram-swimlane",
    feature = "diagram-sequence",
    feature = "diagram-state",
    feature = "diagram-timeline",
    feature = "diagram-cynefin",
    feature = "diagram-info",
    feature = "diagram-journey",
    feature = "diagram-pie"
))]
pub use semantic::{ParsedSemanticDiagram, SemanticDiagramModel, SemanticMetadata, SemanticParser};
pub use source_span::SourceSpan;
pub use theme_id::{MermaidThemeId, MermaidThemeIdParseError};

pub use presentation::{MermaidEngine, ParseOptions, PresentationMetadata};
// Transitional source names; removed when full consumers migrate.
#[doc(hidden)]
pub use presentation::engine::__private;
#[cfg(test)]
pub(crate) use presentation::engine::parse_pipeline;
#[doc(hidden)]
pub use presentation::{MermaidEngine as Engine, PresentationMetadata as ParseMetadata};

/// Maximum nested diagram/include depth accepted by recursive parsers.
pub const MAX_DIAGRAM_NESTING_DEPTH: usize = 256;

/// Returns Mermaid theme names supported by the pinned baseline.
pub fn supported_themes() -> &'static [&'static str] {
    MermaidThemeId::NAMES
}

/// Returns the typed Mermaid theme catalog for the pinned baseline.
pub fn supported_theme_ids() -> &'static [MermaidThemeId] {
    MermaidThemeId::ALL
}

/// Returns metadata names backed by semantic parsers compiled into this build.
///
/// Aliases share their catalog-owned metadata name. This does not promise an output adapter;
/// use the complete [`diagram_family_capabilities`] catalog to distinguish known identities
/// from available implementations.
pub fn supported_diagrams() -> &'static [&'static str] {
    family::supported_diagram_metadata_ids()
}

/// Returns every known Mermaid diagram id in the pinned baseline, including unavailable ones.
///
/// Identity, detector, header, and configuration facts are retained independently of the parser,
/// editor, and typed render-model callbacks compiled into this build.
pub fn diagram_family_capabilities() -> &'static [DiagramFamilyCapability] {
    family::diagram_family_capabilities()
}

/// Returns the complete catalog-owned mapping from logical families to Cargo selectors.
///
/// These declarations are independent of enabled features and exclude infrastructure models.
/// Selecting a family does not enable an output format or optional layout engine.
pub fn diagram_family_selectors() -> &'static [DiagramFamilySelector] {
    family::diagram_family_selectors()
}

/// Returns each compiled concrete built-in typed render-model family exactly once.
///
/// This is independent of output adapters and deduplicates aliases sharing a typed model.
pub fn built_in_typed_render_families() -> &'static [BuiltInTypedRenderFamily] {
    family::built_in_typed_render_families()
}

/// Returns known header facts for Mermaid diagram starters in the pinned baseline.
///
/// A header identifies a known family; actionable suggestions must also check that its semantic
/// parser is available in [`diagram_family_capabilities`].
pub fn diagram_header_facts() -> &'static [DiagramHeaderFact] {
    family::diagram_header_facts()
}

#[cfg(test)]
mod tests;
