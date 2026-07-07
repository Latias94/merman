mod prelude {
    pub use futures::{SinkExt, StreamExt};
    pub use merman_lsp::{CONFIG_SCHEMA_METHOD, MermanLanguageServer, RULE_CATALOG_METHOD};
    pub use serde_json::from_value;
    pub use tokio::time::{Duration, timeout};
    pub use tower::{Service, ServiceExt};
    pub use tower_lsp::jsonrpc::{ErrorCode, Request};
    pub use tower_lsp::lsp_types::{
        CodeActionContext, CodeActionKind, CodeActionParams, DiagnosticServerCapabilities,
        DidChangeConfigurationParams, DidChangeTextDocumentParams, DidCloseTextDocumentParams,
        DidOpenTextDocumentParams, DidSaveTextDocumentParams, DocumentChanges,
        DocumentDiagnosticParams, DocumentDiagnosticReport, DocumentDiagnosticReportResult,
        DocumentSymbolParams, GotoDefinitionParams, HoverContents, HoverParams, InitializeParams,
        LogMessageParams, NumberOrString, Position, PrepareRenameResponse,
        PublishDiagnosticsParams, Range, ReferenceContext, ReferenceParams, RenameParams,
        SelectionRange, SelectionRangeParams, SemanticTokensDeltaParams,
        SemanticTokensFullDeltaResult, SemanticTokensParams, SemanticTokensRangeParams,
        SemanticTokensRangeResult, SemanticTokensResult, SymbolInformation,
        TextDocumentContentChangeEvent, TextDocumentIdentifier, TextDocumentItem,
        TextDocumentPositionParams, VersionedTextDocumentIdentifier, WorkspaceSymbolParams,
    };
}

mod capabilities {
    use super::prelude::*;
    include!("server_smoke/capabilities.rs");
}

mod completion {
    use super::prelude::*;
    include!("server_smoke/completion.rs");
}

mod configuration {
    use super::prelude::*;
    include!("server_smoke/configuration.rs");
}

mod diagnostics {
    use super::prelude::*;
    include!("server_smoke/diagnostics.rs");
}

mod semantic_tokens {
    use super::prelude::*;
    include!("server_smoke/semantic_tokens.rs");
}

mod structure_navigation {
    use super::prelude::*;
    include!("server_smoke/structure_navigation.rs");
}
