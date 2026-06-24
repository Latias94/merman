use crate::code_actions::code_actions_for_params;
use crate::completion::completion_for_snapshot;
use crate::document_store::DocumentStore;
use crate::semantic_tokens::{
    semantic_tokens_for_snapshot, semantic_tokens_for_snapshot_range, semantic_tokens_options,
};
use crate::snapshot::DocumentSnapshot;
use crate::structure::{
    document_symbols as structure_document_symbols, goto_definition as structure_goto_definition,
    hover as structure_hover, prepare_rename as structure_prepare_rename,
    references as structure_references, rename as structure_rename,
    workspace_symbols_for_snapshots as structure_workspace_symbols_for_snapshots,
};
use merman_analysis::{
    AnalysisOptions, Analyzer,
    document::analyze_document,
    lsp::{analysis_payload_to_diagnostics, uri_is_markdown},
    markdown::markdown_source_descriptor,
    options_json::analysis_options_from_json_value,
};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use tokio::sync::Mutex;
use tower_lsp::jsonrpc::Result;
use tower_lsp::lsp_types::{
    CodeActionKind, CodeActionOptions, CodeActionParams, CodeActionProviderCapability,
    CodeActionResponse, CompletionOptions, CompletionParams, CompletionResponse,
    DidChangeTextDocumentParams, DidCloseTextDocumentParams, DidOpenTextDocumentParams,
    DidSaveTextDocumentParams, DocumentSymbolParams, DocumentSymbolResponse, GotoDefinitionParams,
    GotoDefinitionResponse, Hover, HoverParams, HoverProviderCapability, InitializeParams,
    InitializeResult, MessageType, OneOf, PrepareRenameResponse, ReferenceParams, RenameParams,
    SemanticTokensParams, SemanticTokensRangeParams, SemanticTokensRangeResult,
    SemanticTokensResult, SemanticTokensServerCapabilities, ServerCapabilities,
    TextDocumentPositionParams, TextDocumentSyncCapability, TextDocumentSyncKind, WorkspaceEdit,
    WorkspaceSymbolParams,
};
use tower_lsp::{Client, LanguageServer};

#[derive(Debug)]
pub struct MermanLanguageServer {
    client: Client,
    store: Arc<Mutex<DocumentStore>>,
    analyzer: Arc<Mutex<Analyzer>>,
    semantic_tokens_refresh_supported: AtomicBool,
}

impl MermanLanguageServer {
    pub fn new(client: Client) -> Self {
        Self {
            client,
            store: Arc::new(Mutex::new(DocumentStore::new())),
            analyzer: Arc::new(Mutex::new(Analyzer::new())),
            semantic_tokens_refresh_supported: AtomicBool::new(false),
        }
    }

    pub fn capabilities() -> ServerCapabilities {
        ServerCapabilities {
            text_document_sync: Some(TextDocumentSyncCapability::Kind(TextDocumentSyncKind::FULL)),
            hover_provider: Some(HoverProviderCapability::Simple(true)),
            completion_provider: Some(CompletionOptions::default()),
            definition_provider: Some(OneOf::Left(true)),
            references_provider: Some(OneOf::Left(true)),
            rename_provider: Some(OneOf::Left(true)),
            document_symbol_provider: Some(OneOf::Left(true)),
            workspace_symbol_provider: Some(OneOf::Left(true)),
            code_action_provider: Some(CodeActionProviderCapability::Options(CodeActionOptions {
                code_action_kinds: Some(vec![CodeActionKind::QUICKFIX]),
                work_done_progress_options: Default::default(),
                resolve_provider: Some(false),
            })),
            semantic_tokens_provider: Some(
                SemanticTokensServerCapabilities::SemanticTokensOptions(semantic_tokens_options()),
            ),
            ..ServerCapabilities::default()
        }
    }

    async fn snapshot_for_uri(&self, uri: &tower_lsp::lsp_types::Url) -> Option<DocumentSnapshot> {
        let store = self.store.lock().await;
        store.get(uri).cloned()
    }

    async fn publish_for_uri(&self, uri: &tower_lsp::lsp_types::Url, version: Option<i32>) {
        let snapshot = self.snapshot_for_uri(uri).await;

        let Some(snapshot) = snapshot else {
            return;
        };

        let source = if uri_is_markdown(&snapshot.uri) {
            markdown_source_descriptor(Some(snapshot.uri.as_str()))
        } else {
            merman_analysis::SourceDescriptor::diagram().with_path(snapshot.uri.as_str())
        };
        let analyzer = self.analyzer.lock().await;
        let payload = analyze_document(&snapshot.text, &analyzer, source);

        let diagnostics = analysis_payload_to_diagnostics(&payload, uri);
        self.client
            .publish_diagnostics(uri.clone(), diagnostics, version)
            .await;
    }

    async fn replace_analyzer(&self, options: AnalysisOptions) {
        let mut analyzer = self.analyzer.lock().await;
        *analyzer = Analyzer::with_options(options);
    }

    fn client_supports_semantic_tokens_refresh(params: &InitializeParams) -> bool {
        params
            .capabilities
            .workspace
            .as_ref()
            .and_then(|workspace| workspace.semantic_tokens.as_ref())
            .and_then(|semantic_tokens| semantic_tokens.refresh_support)
            .unwrap_or(false)
    }

    async fn apply_initialization_options(
        &self,
        initialization_options: Option<serde_json::Value>,
    ) -> tower_lsp::jsonrpc::Result<()> {
        match initialization_options {
            None => {
                self.replace_analyzer(AnalysisOptions::default()).await;
                Ok(())
            }
            Some(value) => {
                let options = analysis_options_from_json_value(&value)
                    .map_err(|err| tower_lsp::jsonrpc::Error::invalid_params(err.to_string()))?;
                self.replace_analyzer(options).await;
                Ok(())
            }
        }
    }

    async fn republish_all(&self) {
        let snapshots = {
            let store = self.store.lock().await;
            store.snapshots()
        };

        for snapshot in snapshots {
            self.publish_for_uri(&snapshot.uri, Some(snapshot.version))
                .await;
        }
    }
}

#[tower_lsp::async_trait]
impl LanguageServer for MermanLanguageServer {
    async fn initialize(&self, params: InitializeParams) -> Result<InitializeResult> {
        self.semantic_tokens_refresh_supported.store(
            Self::client_supports_semantic_tokens_refresh(&params),
            Ordering::Relaxed,
        );
        self.apply_initialization_options(params.initialization_options)
            .await?;
        Ok(InitializeResult {
            capabilities: Self::capabilities(),
            ..InitializeResult::default()
        })
    }

    async fn initialized(&self, _: tower_lsp::lsp_types::InitializedParams) {
        self.client
            .log_message(MessageType::INFO, "merman-lsp initialized")
            .await;
    }

    async fn shutdown(&self) -> Result<()> {
        Ok(())
    }

    async fn did_open(&self, params: DidOpenTextDocumentParams) {
        let doc = params.text_document;
        self.store
            .lock()
            .await
            .upsert(doc.uri.clone(), doc.version, doc.text);
        self.publish_for_uri(&doc.uri, Some(doc.version)).await;
    }

    async fn did_change(&self, params: DidChangeTextDocumentParams) {
        let doc = params.text_document;
        let mut store = self.store.lock().await;
        let Some(current) = store.get(&doc.uri).cloned() else {
            return;
        };

        let mut text = current.text;
        for change in params.content_changes {
            text = change.text;
        }
        store.upsert(doc.uri.clone(), doc.version, text);
        drop(store);
        self.publish_for_uri(&doc.uri, Some(doc.version)).await;
    }

    async fn did_save(&self, params: DidSaveTextDocumentParams) {
        let uri = params.text_document.uri;
        let version = {
            let store = self.store.lock().await;
            store.get(&uri).map(|doc| doc.version)
        };
        self.publish_for_uri(&uri, version).await;
    }

    async fn did_close(&self, params: DidCloseTextDocumentParams) {
        self.store.lock().await.remove(&params.text_document.uri);
    }

    async fn did_change_configuration(
        &self,
        params: tower_lsp::lsp_types::DidChangeConfigurationParams,
    ) {
        let options = if params.settings.is_null() {
            AnalysisOptions::default()
        } else {
            match analysis_options_from_json_value(&params.settings) {
                Ok(options) => options,
                Err(err) => {
                    self.client
                        .log_message(
                            MessageType::ERROR,
                            format!("invalid merman analysis settings: {err}"),
                        )
                        .await;
                    return;
                }
            }
        };

        self.replace_analyzer(options).await;
        self.republish_all().await;
        if self
            .semantic_tokens_refresh_supported
            .load(Ordering::Relaxed)
        {
            let _ = self.client.semantic_tokens_refresh().await;
        }
    }

    async fn completion(&self, params: CompletionParams) -> Result<Option<CompletionResponse>> {
        let uri = params.text_document_position.text_document.uri;
        let position = params.text_document_position.position;
        let snapshot = self.snapshot_for_uri(&uri).await;

        Ok(snapshot
            .map(|snapshot| CompletionResponse::List(completion_for_snapshot(&snapshot, position))))
    }

    async fn code_action(&self, params: CodeActionParams) -> Result<Option<CodeActionResponse>> {
        Ok(code_actions_for_params(&params))
    }

    async fn semantic_tokens_full(
        &self,
        params: SemanticTokensParams,
    ) -> Result<Option<SemanticTokensResult>> {
        let uri = params.text_document.uri;
        let snapshot = self.snapshot_for_uri(&uri).await;

        Ok(snapshot.map(|snapshot| semantic_tokens_for_snapshot(&snapshot).into()))
    }

    async fn semantic_tokens_range(
        &self,
        params: SemanticTokensRangeParams,
    ) -> Result<Option<SemanticTokensRangeResult>> {
        let uri = params.text_document.uri;
        let snapshot = self.snapshot_for_uri(&uri).await;

        Ok(snapshot
            .map(|snapshot| semantic_tokens_for_snapshot_range(&snapshot, params.range).into()))
    }

    async fn hover(&self, params: HoverParams) -> Result<Option<Hover>> {
        let uri = params.text_document_position_params.text_document.uri;
        let position = params.text_document_position_params.position;
        let snapshot = self.snapshot_for_uri(&uri).await;

        Ok(snapshot.and_then(|snapshot| structure_hover(&snapshot, position)))
    }

    async fn document_symbol(
        &self,
        params: DocumentSymbolParams,
    ) -> Result<Option<DocumentSymbolResponse>> {
        let uri = params.text_document.uri;
        let snapshot = self.snapshot_for_uri(&uri).await;

        Ok(snapshot.map(|snapshot| structure_document_symbols(&snapshot)))
    }

    async fn goto_definition(
        &self,
        params: GotoDefinitionParams,
    ) -> Result<Option<GotoDefinitionResponse>> {
        let uri = params.text_document_position_params.text_document.uri;
        let position = params.text_document_position_params.position;
        let snapshot = self.snapshot_for_uri(&uri).await;

        Ok(snapshot.and_then(|snapshot| structure_goto_definition(&snapshot, position)))
    }

    async fn references(
        &self,
        params: ReferenceParams,
    ) -> Result<Option<Vec<tower_lsp::lsp_types::Location>>> {
        let uri = params.text_document_position.text_document.uri;
        let position = params.text_document_position.position;
        let snapshot = self.snapshot_for_uri(&uri).await;

        Ok(snapshot.and_then(|snapshot| {
            structure_references(&snapshot, position, params.context.include_declaration)
        }))
    }

    async fn prepare_rename(
        &self,
        params: TextDocumentPositionParams,
    ) -> Result<Option<PrepareRenameResponse>> {
        let uri = params.text_document.uri;
        let position = params.position;
        let snapshot = self.snapshot_for_uri(&uri).await;

        Ok(snapshot.and_then(|snapshot| structure_prepare_rename(&snapshot, position)))
    }

    async fn rename(&self, params: RenameParams) -> Result<Option<WorkspaceEdit>> {
        let uri = params.text_document_position.text_document.uri.clone();
        let snapshot = self.snapshot_for_uri(&uri).await;

        match snapshot {
            Some(snapshot) => structure_rename(&snapshot, params),
            None => Ok(None),
        }
    }

    async fn symbol(
        &self,
        params: WorkspaceSymbolParams,
    ) -> Result<Option<Vec<tower_lsp::lsp_types::SymbolInformation>>> {
        let snapshots = {
            let store = self.store.lock().await;
            store.snapshots()
        };

        Ok(Some(structure_workspace_symbols_for_snapshots(
            &snapshots,
            &params.query,
        )))
    }
}

#[cfg(test)]
mod tests {
    use super::MermanLanguageServer;
    use crate::document_store::DocumentStore;
    use crate::structure::{
        document_symbols, goto_definition, hover, prepare_rename, references, rename,
    };
    use merman_analysis::{
        AnalysisDiagnostic, DiagnosticCategory, DiagnosticFix, DiagnosticFixEdit, SourceMap,
        lsp::analysis_diagnostic_to_lsp,
    };
    use tower_lsp::LanguageServer;
    use tower_lsp::lsp_types::SemanticTokensResult;
    use tower_lsp::lsp_types::{
        CodeActionContext, CodeActionKind, CodeActionOrCommand, CodeActionParams,
        CodeActionProviderCapability, DocumentSymbolResponse, GotoDefinitionResponse,
        HoverContents, HoverParams, InitializeParams, Position, Range, RenameParams,
        SemanticTokensFullOptions, SemanticTokensParams, SemanticTokensRangeParams,
        SemanticTokensRangeResult, SemanticTokensServerCapabilities, TextDocumentIdentifier,
        TextDocumentPositionParams, TextDocumentSyncCapability, TextDocumentSyncKind, Url,
        WorkspaceSymbolParams,
    };
    use tower_lsp::lsp_types::{HoverProviderCapability, OneOf};

    #[test]
    fn capabilities_advertise_completion_and_full_sync() {
        let capabilities = MermanLanguageServer::capabilities();

        assert!(matches!(
            capabilities.text_document_sync,
            Some(TextDocumentSyncCapability::Kind(TextDocumentSyncKind::FULL))
        ));
        assert!(matches!(
            capabilities.hover_provider,
            Some(HoverProviderCapability::Simple(true))
        ));
        assert!(matches!(
            capabilities.document_symbol_provider,
            Some(OneOf::Left(true))
        ));
        assert!(matches!(
            capabilities.definition_provider,
            Some(OneOf::Left(true))
        ));
        assert!(matches!(
            capabilities.references_provider,
            Some(OneOf::Left(true))
        ));
        assert!(matches!(
            capabilities.rename_provider,
            Some(OneOf::Left(true))
        ));
        assert!(matches!(
            capabilities.workspace_symbol_provider,
            Some(OneOf::Left(true))
        ));
        assert!(capabilities.completion_provider.is_some());
        assert!(matches!(
            capabilities.semantic_tokens_provider,
            Some(SemanticTokensServerCapabilities::SemanticTokensOptions(ref options))
                if matches!(options.full, Some(SemanticTokensFullOptions::Bool(true)))
                    && options.range == Some(true)
                    && !options.legend.token_types.is_empty()
                    && !options.legend.token_modifiers.is_empty()
        ));
        assert!(matches!(
            capabilities.code_action_provider,
            Some(CodeActionProviderCapability::Options(ref options))
                if options
                    .code_action_kinds
                    .as_ref()
                    .is_some_and(|kinds| kinds.contains(&CodeActionKind::QUICKFIX))
                    && options.resolve_provider == Some(false)
        ));
    }

    #[test]
    fn semantic_tokens_refresh_support_comes_from_client_capabilities() {
        let mut params = InitializeParams::default();
        assert!(!MermanLanguageServer::client_supports_semantic_tokens_refresh(&params));

        params.capabilities.workspace = Some(Default::default());
        assert!(!MermanLanguageServer::client_supports_semantic_tokens_refresh(&params));

        params
            .capabilities
            .workspace
            .as_mut()
            .unwrap()
            .semantic_tokens = Some(
            tower_lsp::lsp_types::SemanticTokensWorkspaceClientCapabilities {
                refresh_support: None,
            },
        );
        assert!(!MermanLanguageServer::client_supports_semantic_tokens_refresh(&params));

        params
            .capabilities
            .workspace
            .as_mut()
            .unwrap()
            .semantic_tokens
            .as_mut()
            .unwrap()
            .refresh_support = Some(true);
        assert!(MermanLanguageServer::client_supports_semantic_tokens_refresh(&params));
    }

    #[test]
    fn structure_helpers_produce_hover_and_nested_symbols() {
        let mut store = DocumentStore::new();
        let uri = Url::parse("file:///tmp/example.mmd").unwrap();
        let snapshot = store.upsert(
            uri.clone(),
            1,
            "flowchart TD\nsubgraph group\nA-->B\nend\n".to_string(),
        );

        let hover = hover(&snapshot, Position::new(1, 0)).unwrap();
        let text = match hover.contents {
            HoverContents::Markup(markup) => markup.value,
            other => panic!("unexpected hover contents: {other:?}"),
        };
        assert!(text.contains("group"));

        let symbols = match document_symbols(&snapshot) {
            DocumentSymbolResponse::Nested(symbols) => symbols,
            other => panic!("unexpected symbol response: {other:?}"),
        };
        assert_eq!(symbols.len(), 1);
        assert!(
            symbols[0]
                .children
                .as_ref()
                .unwrap()
                .iter()
                .any(|symbol| symbol.name == "group")
        );
    }

    #[test]
    fn structure_helpers_cover_navigation_surface() {
        let mut store = DocumentStore::new();
        let uri = Url::parse("file:///tmp/example.mmd").unwrap();
        let snapshot = store.upsert(uri.clone(), 1, "flowchart TD\nA-->B\nA-->C\n".to_string());
        let position = Position::new(1, 0);

        assert!(matches!(
            goto_definition(&snapshot, position),
            Some(GotoDefinitionResponse::Scalar(_))
        ));
        assert_eq!(references(&snapshot, position, true).unwrap().len(), 2);
        assert!(prepare_rename(&snapshot, position).is_some());
        let rename = rename(
            &snapshot,
            RenameParams {
                text_document_position: TextDocumentPositionParams::new(
                    TextDocumentIdentifier { uri },
                    position,
                ),
                new_name: "X".to_string(),
                work_done_progress_params: Default::default(),
            },
        )
        .unwrap();
        assert_eq!(
            rename
                .unwrap()
                .changes
                .unwrap()
                .values()
                .next()
                .unwrap()
                .len(),
            2
        );
    }

    #[tokio::test(flavor = "current_thread")]
    async fn lsp_handlers_return_hover_and_symbols() {
        let (service, _socket) = tower_lsp::LspService::new(MermanLanguageServer::new);
        let server = service.inner();
        let uri = Url::parse("file:///tmp/example.mmd").unwrap();

        {
            let mut store = server.store.lock().await;
            store.upsert(
                uri.clone(),
                1,
                "flowchart TD\nsubgraph group\nA-->B\nend\n".to_string(),
            );
        }

        let hover = server
            .hover(HoverParams {
                text_document_position_params: TextDocumentPositionParams::new(
                    TextDocumentIdentifier { uri: uri.clone() },
                    Position::new(1, 0),
                ),
                work_done_progress_params: Default::default(),
            })
            .await
            .unwrap();
        assert!(hover.is_some());

        let semantic_tokens = server
            .semantic_tokens_full(SemanticTokensParams {
                text_document: TextDocumentIdentifier { uri: uri.clone() },
                work_done_progress_params: Default::default(),
                partial_result_params: Default::default(),
            })
            .await
            .unwrap();
        assert!(matches!(
            semantic_tokens,
            Some(SemanticTokensResult::Tokens(tokens)) if !tokens.data.is_empty()
        ));

        let semantic_tokens_range = server
            .semantic_tokens_range(SemanticTokensRangeParams {
                text_document: TextDocumentIdentifier { uri: uri.clone() },
                range: Range {
                    start: Position::new(1, 0),
                    end: Position::new(2, 7),
                },
                work_done_progress_params: Default::default(),
                partial_result_params: Default::default(),
            })
            .await
            .unwrap();
        assert!(matches!(
            semantic_tokens_range,
            Some(SemanticTokensRangeResult::Tokens(tokens)) if !tokens.data.is_empty()
        ));

        let map = SourceMap::new("bad");
        let fix_span = map.whole_source_span().unwrap();
        let diagnostic = AnalysisDiagnostic::error(
            "merman.test.fix",
            DiagnosticCategory::Semantic,
            "test diagnostic",
        )
        .with_fix(
            DiagnosticFix::new(
                "Replace invalid text",
                vec![DiagnosticFixEdit::new(fix_span, "fixed")],
            )
            .preferred(),
        );
        let code_actions = server
            .code_action(CodeActionParams {
                text_document: TextDocumentIdentifier { uri: uri.clone() },
                range: Range {
                    start: Position::new(0, 0),
                    end: Position::new(0, 3),
                },
                context: CodeActionContext {
                    diagnostics: vec![analysis_diagnostic_to_lsp(&diagnostic, &uri)],
                    only: Some(vec![CodeActionKind::QUICKFIX]),
                    trigger_kind: None,
                },
                work_done_progress_params: Default::default(),
                partial_result_params: Default::default(),
            })
            .await
            .unwrap()
            .expect("expected code action response");
        assert_eq!(code_actions.len(), 1);
        assert!(matches!(
            &code_actions[0],
            CodeActionOrCommand::CodeAction(action)
                if action.title == "Replace invalid text"
                    && action.kind == Some(CodeActionKind::QUICKFIX)
                    && action.is_preferred == Some(true)
        ));

        let document_symbols = server
            .document_symbol(tower_lsp::lsp_types::DocumentSymbolParams {
                text_document: TextDocumentIdentifier { uri },
                work_done_progress_params: Default::default(),
                partial_result_params: Default::default(),
            })
            .await
            .unwrap();
        assert!(matches!(
            document_symbols,
            Some(DocumentSymbolResponse::Nested(_))
        ));

        let workspace_symbols = server
            .symbol(WorkspaceSymbolParams {
                partial_result_params: Default::default(),
                work_done_progress_params: Default::default(),
                query: "group".to_string(),
            })
            .await
            .unwrap()
            .expect("expected workspace symbol response");
        assert!(!workspace_symbols.is_empty());
        assert!(
            workspace_symbols
                .iter()
                .any(|symbol| symbol.name == "group")
        );
    }
}
