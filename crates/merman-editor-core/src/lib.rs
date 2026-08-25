#![forbid(unsafe_code)]

//! Protocol-neutral editor intelligence for Merman.
//!
//! This crate owns parser-backed editor snapshots and query semantics without depending on LSP,
//! WASM, Monaco, or TypeScript protocol types. Hosts own document lifecycles and call the one-shot
//! construction functions when their source generation changes.
//!
//! ```no_run
//! use merman_analysis::Analyzer;
//! use merman_editor_core::{
//!     DocumentKind, Position, analyze_document_snapshot_with_shared_text,
//!     completion_for_snapshot,
//! };
//! use std::sync::Arc;
//!
//! let snapshot = analyze_document_snapshot_with_shared_text(
//!     &Analyzer::new(),
//!     "file:///workspace/diagram.mmd",
//!     1,
//!     Arc::from("flowchart TD\nA --> B\nB -->"),
//!     DocumentKind::Diagram,
//! )
//! .expect("source is within the configured analysis limit");
//! let completion = completion_for_snapshot(&snapshot, Position::new(2, 5));
//! assert!(!completion.items.is_empty());
//! ```
//!
//! Completion policy is exposed through [`completion_for_snapshot`]. The former public
//! `CompletionContext` wrapper was deleted rather than retained as a compatibility alias.
//!
//! ```compile_fail
//! use merman_editor_core::CompletionContext;
//! ```
//!
//! Core completion candidates and the former generic expected-syntax variants were also deleted;
//! parser facts now expose typed slots while editor-core owns the candidate policy.
//!
//! ```compile_fail
//! use merman_core::{EditorCompletionCandidate, EditorCompletionVocabulary};
//! ```
//!
//! ```compile_fail
//! use merman_core::EditorExpectedSyntaxKind;
//! let _ = EditorExpectedSyntaxKind::Operator;
//! let _ = EditorExpectedSyntaxKind::DirectionValue;
//! ```
//!
//! The former stateful workspace and outcome wrappers were also deleted instead of retained as
//! compatibility aliases.
//!
//! ```compile_fail
//! use merman_editor_core::DocumentAnalysisOutcome;
//! ```
//!
//! ```compile_fail
//! use merman_editor_core::DocumentWorkspace;
//! ```

mod code_actions;
mod completion;
mod context;
mod diagnostics;
mod document_analysis;
mod snapshot;
mod structure;
mod types;

pub use code_actions::{
    EditorCodeAction, EditorCodeActionEdit, code_action_from_fix, code_actions_from_fixes,
};
pub use completion::{
    COMPLETION_TRIGGER_CHARACTERS, CompletionDataKind, CompletionInsertTextFormat, CompletionItem,
    CompletionItemKind, CompletionItemLabelDetails, CompletionList, CompletionResolveData,
    CompletionTextEdit, completion_documentation, completion_for_snapshot,
};
pub use diagnostics::{
    DiagnosticCodeActionData, EditorDiagnostic, EditorDiagnosticRelated,
    analysis_diagnostic_to_editor, analysis_payload_to_diagnostics,
};
pub use document_analysis::{
    DocumentAnalysisContext, analyze_document_context_with_shared_text,
    analyze_document_context_with_shared_text_cancellable,
    analyze_document_snapshot_with_shared_text,
};
pub use merman_analysis::FenceTextIndexSource;
pub use merman_core::{EditorSemanticKind, EditorSemanticRole};
pub use snapshot::{
    DiagramDetectionValidity, DocumentSnapshot, DocumentSnapshotError, EditorDiagramDetection,
    FenceSnapshot,
};
pub use structure::{
    EditorDocumentSymbol, EditorFoldingRange, EditorFoldingRangeKind, EditorHover, EditorLocation,
    EditorMarkupContent, EditorPrepareRename, EditorSelectionRange, EditorSymbolInformation,
    EditorTextEdit, EditorWorkspaceEdit, RenameError, document_symbols, folding_ranges,
    goto_definition, hover, prepare_rename, references, rename, search_document_symbols,
    selection_range, selection_ranges,
};
pub use types::{DocumentKind, DocumentUri, Position, Range};
