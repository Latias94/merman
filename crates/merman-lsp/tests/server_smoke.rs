mod prelude {
    pub use futures::{SinkExt, StreamExt};
    pub use merman_lsp::{
        CONFIG_SCHEMA_METHOD, CONFIG_SCHEMA_RESPONSE_VERSION, FIXED_TODAY_SCHEMA_PATTERN,
        MermanLanguageServer, RULE_CATALOG_METHOD, RULE_CATALOG_RESPONSE_VERSION,
    };
    pub use serde_json::from_value;
    pub use tokio::time::{Duration, timeout};
    pub use tower::{Service, ServiceExt};
    pub use tower_lsp_server::jsonrpc::{ErrorCode, Request};
    pub use tower_lsp_server::ls_types::{
        CodeActionContext, CodeActionKind, CodeActionOrCommand, CodeActionParams,
        DiagnosticServerCapabilities, DidChangeConfigurationParams, DidChangeTextDocumentParams,
        DidCloseTextDocumentParams, DidOpenTextDocumentParams, DidSaveTextDocumentParams,
        DocumentChanges, DocumentDiagnosticParams, DocumentDiagnosticReport,
        DocumentDiagnosticReportResult, DocumentSymbolParams, FoldingRange, FoldingRangeKind,
        FoldingRangeParams, GotoDefinitionParams, HoverContents, HoverParams, InitializeParams,
        LogMessageParams, NumberOrString, Position, PrepareRenameResponse,
        PublishDiagnosticsParams, Range, ReferenceContext, ReferenceParams, RenameParams,
        SelectionRange, SelectionRangeParams, SemanticTokensDeltaParams,
        SemanticTokensFullDeltaResult, SemanticTokensParams, SemanticTokensRangeParams,
        SemanticTokensRangeResult, SemanticTokensResult, TextDocumentContentChangeEvent,
        TextDocumentIdentifier, TextDocumentItem, TextDocumentPositionParams,
        VersionedTextDocumentIdentifier,
    };
}

#[path = "server_smoke/capabilities.rs"]
mod capabilities;

#[path = "server_smoke/completion.rs"]
mod completion;

#[path = "server_smoke/code_actions.rs"]
mod code_actions;

#[path = "server_smoke/configuration.rs"]
mod configuration;

#[path = "server_smoke/diagnostics.rs"]
mod diagnostics;

#[path = "server_smoke/semantic_tokens.rs"]
mod semantic_tokens;

#[path = "server_smoke/refresh.rs"]
mod refresh;

#[path = "server_smoke/structure_navigation.rs"]
mod structure_navigation;
