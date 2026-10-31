#![forbid(unsafe_code)]
#![cfg_attr(
    not(feature = "presentation"),
    doc = r#"
Presentation definitions require the `presentation` capability:

```compile_fail,E0432
use merman_core::presentation::MermaidEngine;
```

```compile_fail,E0432
use merman_core::SourcePresentationMetadata;
```
"#
)]
#![cfg_attr(
    not(feature = "editor-facts"),
    doc = r#"
The editor namespace requires the `editor-facts` capability:

```compile_fail,E0432
use merman_core::editor::EditorSemanticFacts;
```

Source-configuration provenance is part of the same capability:

```compile_fail,E0432
use merman_core::preprocess::SourceConfigEvidence;
```
"#
)]
//! Mermaid source parsing and preprocessing (headless).
//!
//! Design goals:
//! - 1:1 parity with the repository's pinned upstream Mermaid baseline
//! - deterministic, testable outputs (semantic snapshot goldens)
//! - runtime-agnostic async APIs (no specific executor required)
//!
//! Default features are empty. Select `diagram-*` features for the required logical families or
//! `all-diagrams` for the complete parser surface. Portable data and read-only presentation
//! definitions are owned by `merman-model`; shared markup policy is owned by `merman-markup`.
//! Recognition and custom source registries remain available without a diagram family.

use merman_markup::sanitize;
#[cfg(feature = "diagram-class")]
use merman_markup::style;
#[cfg(feature = "presentation")]
use merman_markup::theme_color;
#[cfg(feature = "diagram-class")]
use merman_model::text as common;
#[cfg(any(test, feature = "presentation", feature = "diagram-gantt"))]
use merman_model::time;
use merman_model::{baseline, json as json_value, operation, text as entities};
mod common_db;
mod config;
pub mod detect;
pub mod diagram;
mod diagrams;
#[cfg(feature = "editor-facts")]
pub mod editor;
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
#[cfg(test)]
mod operation_tests;
mod parse_diagnostic;
mod parse_options;
pub mod preprocess;
#[cfg(feature = "presentation")]
pub mod presentation;
#[cfg(test)]
mod resource_tests;
mod runtime;
pub mod semantic;
mod source_span;
#[cfg(feature = "presentation")]
mod theme;
mod utils;
mod yaml_config;

#[cfg(feature = "presentation")]
pub use detect::{Detector, DetectorRegistry};
pub use diagram::CapturedPanic;
#[cfg(feature = "editor-facts")]
pub use editor::ParsedEditorFacts;
#[cfg(feature = "editor-facts")]
pub use editor::{
    EditorExpectedSyntax, EditorExpectedSyntaxKind, EditorFamilySemantics, EditorRenamePolicy,
    EditorSemanticCompleteness, EditorSemanticDiagnostic, EditorSemanticDiagnosticKind,
    EditorSemanticFacts, EditorSemanticKind, EditorSemanticRole, EditorSemanticSymbol,
};
pub use error::{Error, InternalFailure, ParseDiagnostic, ParseDiagnosticSpanKind, Result};
pub use family::{BuiltInTypedRenderFamily, DiagramFamilyCapability};
#[cfg(feature = "compat-json")]
use json_value::ManagedJson as ManagedSemanticJson;
use merman_model::config::{MermaidConfig, MermaidThemeId};
use merman_model::family::DiagramHeaderFact;
#[cfg(all(test, feature = "presentation"))]
use merman_model::presentation::CustomJsonProvenance;
#[cfg(all(test, feature = "presentation"))]
use merman_model::presentation::CustomJsonRenderModel;
#[cfg(feature = "presentation")]
use merman_model::presentation::PresentationMetadata;
#[cfg(all(test, feature = "presentation"))]
use merman_model::presentation::RenderSemanticModel;
use merman_model::semantic::{SemanticDiagramModel, SemanticMetadata};
use merman_model::source::SourceSpan;
use merman_model::warnings::DiagramWarningFact;
#[cfg(any(test, feature = "diagram-flowchart", feature = "diagram-swimlane"))]
use merman_model::warnings::FLOWCHART_EXPLICIT_DIRECTION_WARNING_RULE_ID;
#[cfg(test)]
use operation::CancelReason;
use operation::{OperationCancelled, OperationControl, OperationControlResult, OperationPhase};
pub use parse_options::ParseOptions;
pub use preprocess::{PreprocessResult, PreprocessedSource};
#[cfg(feature = "presentation")]
pub use preprocess::{preprocess_diagram, preprocess_diagram_with_known_type};
#[cfg(feature = "presentation")]
pub use presentation::diagram::{
    CustomJsonRenderParser, ParsedDiagramRender, RenderDiagramRegistry,
};
#[cfg(all(feature = "compat-json", feature = "editor-facts"))]
pub use presentation::diagram::{
    DiagramParseOutcome, DiagramParseSnapshot, DiagramSnapshotCapture,
};
#[cfg(feature = "compat-json")]
pub use presentation::diagram::{DiagramRegistry, DiagramSemanticParser, ParsedDiagram};
pub use semantic::SemanticParser;
#[cfg(feature = "editor-facts")]
pub use semantic::{SemanticDiagramSnapshot, SemanticParseOutcome, SemanticSnapshotCapture};

#[cfg(all(test, feature = "presentation"))]
pub(crate) use presentation::engine::parse_pipeline;
#[cfg(feature = "presentation")]
pub use presentation::{
    MermaidEngine, SourcePresentationMetadata, ThemeCompatibilityPlan, ThemeCompatibilityPlanError,
};

use merman_model::MAX_DIAGRAM_NESTING_DEPTH;

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
