#![forbid(unsafe_code)]

//! Protocol-neutral editor intelligence for Merman.
//!
//! This crate owns editor-facing document state and query semantics without depending on LSP,
//! WASM, Monaco, or TypeScript protocol types.

mod code_actions;
mod completion;
mod context;
mod diagnostics;
mod semantic_tokens;
mod snapshot;
mod structure;
mod types;
mod workspace;

pub use code_actions::{
    EditorCodeAction, EditorCodeActionEdit, code_action_from_fix, code_actions_from_fixes,
};
pub use completion::{
    CompletionDataKind, CompletionInsertTextFormat, CompletionItem, CompletionItemKind,
    CompletionItemLabelDetails, CompletionList, CompletionResolveData, CompletionTextEdit,
    completion_documentation, completion_for_snapshot,
};
pub use context::CompletionContext;
pub use diagnostics::{
    DiagnosticCodeActionData, EditorDiagnostic, EditorDiagnosticRelated,
    analysis_diagnostic_to_editor, analysis_payload_to_diagnostics,
};
pub use merman_analysis::FenceTextIndexSource;
pub use semantic_tokens::{
    SemanticToken, SemanticTokenKind, SemanticTokenLegend, SemanticTokenModifier,
    semantic_token_legend, semantic_tokens_for_snapshot, semantic_tokens_for_snapshot_range,
    token_modifier_index, token_type_index,
};
pub use snapshot::{DocumentSnapshot, FenceSnapshot};
pub use structure::{
    EditorDocumentSymbol, EditorFoldingRange, EditorFoldingRangeKind, EditorHover, EditorLocation,
    EditorMarkupContent, EditorPrepareRename, EditorSelectionRange, EditorSymbolInformation,
    EditorTextEdit, EditorWorkspaceEdit, RenameError, document_symbols, folding_ranges,
    goto_definition, hover, prepare_rename, references, rename, selection_range, selection_ranges,
    workspace_symbols, workspace_symbols_for_snapshots,
};
pub use types::{DocumentKind, DocumentUri, Position, Range};
pub use workspace::{DocumentAnalysisContext, DocumentWorkspace};
