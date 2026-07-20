#![forbid(unsafe_code)]

//! Protocol-neutral editor intelligence for Merman.
//!
//! This crate owns editor-facing document state and query semantics without depending on LSP,
//! WASM, Monaco, or TypeScript protocol types.

mod code_actions;
mod completion;
mod context;
mod diagnostics;
mod generated;
mod snapshot;
mod structure;
mod token_planner;
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
pub use generated::{
    PlannedTokenKind, PlannedTokenModifier, SEMANTIC_TOKEN_DESCRIPTOR,
    SEMANTIC_TOKEN_DESCRIPTOR_DIGEST, SEMANTIC_TOKEN_PACKED_WORDS_PER_TOKEN,
    SEMANTIC_TOKEN_VALID_MODIFIER_MASK, SEMANTIC_TOKEN_VALID_TYPE_CODE_MAX,
    SemanticTokenDescriptor, SemanticTokenKindDescriptor, SemanticTokenModifierDescriptor,
    SemanticTokenPackedDescriptor, TokenOverlayKind, semantic_token_descriptor,
};
pub use merman_analysis::FenceTextIndexSource;
pub use snapshot::{DocumentSnapshot, FenceSnapshot};
pub use structure::{
    EditorDocumentSymbol, EditorFoldingRange, EditorFoldingRangeKind, EditorHover, EditorLocation,
    EditorMarkupContent, EditorPrepareRename, EditorSelectionRange, EditorSymbolInformation,
    EditorTextEdit, EditorWorkspaceEdit, RenameError, document_symbols, folding_ranges,
    goto_definition, hover, prepare_rename, references, rename, selection_range, selection_ranges,
    workspace_symbols, workspace_symbols_for_snapshots,
};
pub use token_planner::{
    PlannedToken, SemanticTokenPlan, TokenPlanError, plan_semantic_tokens_for_snapshot,
};
pub use types::{DocumentKind, DocumentUri, Position, Range};
pub use workspace::{
    AnalyzedDocumentSnapshot, DiagramDetectionValidity, DocumentWorkspace, EditorDiagramDetection,
};
