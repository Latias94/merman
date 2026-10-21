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
#[cfg(feature = "presentation")]
pub mod presentation;
pub mod resources;
pub mod runtime;
pub mod sanitize;
pub mod semantic;
mod source_span;
pub mod style;
pub mod svg_security;
#[doc(hidden)]
pub mod terminal_text;
#[cfg(feature = "presentation")]
mod theme;
#[cfg(feature = "presentation")]
pub mod theme_color;
mod theme_id;
pub mod time;
pub mod utils;
mod yaml_config;

pub use config::MermaidConfig;
#[cfg(feature = "presentation")]
pub use detect::{Detector, DetectorRegistry};
pub use diagram::{
    AGENTFLOW_CONTAINMENT_VIOLATION_WARNING_RULE_ID, AGENTFLOW_SHAPE_REMOVED_WARNING_RULE_ID,
    AGENTFLOW_SHAPE_UNSUPPORTED_WARNING_RULE_ID, BLOCK_WIDTH_WARNING_RULE_ID, CapturedPanic,
    DiagramWarningFact, FLOWCHART_EXPLICIT_DIRECTION_WARNING_RULE_ID,
    FLOWCHART_UNKNOWN_STYLE_TARGET_WARNING_RULE_ID, GIT_GRAPH_DUPLICATE_COMMIT_WARNING_RULE_ID,
};
pub use editor::{
    EditorExpectedSyntax, EditorExpectedSyntaxKind, EditorFamilySemantics, EditorRenamePolicy,
    EditorSemanticCompleteness, EditorSemanticDiagnostic, EditorSemanticDiagnosticKind,
    EditorSemanticFacts, EditorSemanticKind, EditorSemanticRole, EditorSemanticSymbol,
};
#[cfg(feature = "presentation")]
pub use error::ThemeEvaluationLimitExceeded;
pub use error::{Error, InternalFailure, ParseDiagnostic, ParseDiagnosticSpanKind, Result};
pub use family::{
    BuiltInTypedRenderFamily, DiagramFamilyCapability, DiagramFamilyId, DiagramFamilySelector,
    DiagramHeaderFact, diagram_type_family_id, diagram_type_family_kind, diagram_type_metadata_id,
    diagram_type_render_model_kind,
};
#[cfg(feature = "compat-json")]
pub use json_value::ManagedJson as ManagedSemanticJson;
pub use json_value::ManagedJson;
pub use operation::{
    CancelReason, OperationCancelled, OperationControl, OperationControlResult,
    OperationLedgerError, OperationPhase, OperationResourceDomain, OperationResourceLimitExceeded,
    OperationResourceOverride, OperationResourceProvenance,
};
pub use preprocess::{PreprocessResult, PreprocessedSource};
#[cfg(feature = "presentation")]
pub use preprocess::{preprocess_diagram, preprocess_diagram_with_known_type};
#[cfg(all(test, feature = "presentation"))]
use presentation::PresentationConfig;
#[cfg(feature = "compat-json")]
pub use presentation::diagram::{
    BuiltinRenderSemantic, DiagramParseOutcome, DiagramParseSnapshot, DiagramRegistry,
    DiagramSemanticParser, DiagramSnapshotCapture, ParsedDiagram, ParsedEditorFacts,
};
#[cfg(feature = "presentation")]
pub use presentation::diagram::{
    CustomJsonProvenance, CustomJsonRenderModel, CustomJsonRenderParser, ParsedDiagramRender,
    RenderDiagramRegistry, RenderSemanticModel,
};
pub use semantic::{ParsedSemanticDiagram, SemanticDiagramModel, SemanticMetadata, SemanticParser};
pub use source_span::SourceSpan;
pub use theme_id::{MermaidThemeId, MermaidThemeIdParseError};

#[cfg(feature = "presentation")]
pub use presentation::{MermaidEngine, ParseOptions, PresentationMetadata};
// Transitional source names; removed when full consumers migrate.
#[doc(hidden)]
#[cfg(feature = "presentation")]
pub use presentation::engine::__private;
#[cfg(all(test, feature = "presentation"))]
pub(crate) use presentation::engine::parse_pipeline;
#[doc(hidden)]
#[cfg(feature = "presentation")]
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

#[cfg(all(test, feature = "compat-json", feature = "editor-facts"))]
mod tests;
