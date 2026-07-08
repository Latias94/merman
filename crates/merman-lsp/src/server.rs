use crate::code_actions::code_actions_for_params_with_encoding;
use crate::completion::{completion_for_snapshot, resolve_completion_item};
use crate::diagnostics::analysis_payload_to_versioned_diagnostics;
use crate::document_store::{
    AnalyzerConfigurationChange, DiagnosticContext, DocumentDiagnosticState, DocumentStore,
    SemanticTokensState, SnapshotContext, StoredDocument, TextDocumentUpdate,
    WORKSPACE_SYMBOL_SNAPSHOT_BATCH_SIZE, analysis_options_with_lsp_resource_defaults,
    default_lsp_analysis_options,
};
use crate::protocol::{
    CONFIG_SCHEMA_METHOD, ConfigSchemaResponse, RULE_CATALOG_METHOD, RuleCatalogResponse,
    WorkspaceEditEncoding, experimental_capabilities,
};
use crate::semantic_tokens::{
    semantic_tokens_delta_result, semantic_tokens_for_snapshot, semantic_tokens_for_snapshot_range,
    semantic_tokens_options, semantic_tokens_result_id,
};
use crate::snapshot::DocumentSnapshot;
use crate::snapshot_context::{self, SnapshotContextKind};
use crate::structure::{
    document_symbols_with_hierarchy_support as structure_document_symbols_with_hierarchy_support,
    folding_ranges as structure_folding_ranges, goto_definition as structure_goto_definition,
    hover as structure_hover, prepare_rename as structure_prepare_rename,
    references as structure_references,
    rename_with_workspace_edit_encoding as structure_rename_with_workspace_edit_encoding,
    selection_ranges as structure_selection_ranges,
    workspace_symbols_for_snapshots as structure_workspace_symbols_for_snapshots,
};
use merman_analysis::{
    AnalysisOptions, AnalysisPayload, Analyzer, SourceKind, document::analyze_document,
    options_json::analysis_options_from_json_value, source_descriptor_for_kind,
    source_discarded_after_limit_change_diagnostic, source_limit_diagnostic_for_len,
};
use merman_editor_core::DocumentKind;
use std::hash::{Hash, Hasher};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use tokio::sync::Mutex;
use tower_lsp::jsonrpc::Result;
use tower_lsp::lsp_types::{
    CodeActionKind, CodeActionOptions, CodeActionParams, CodeActionProviderCapability,
    CodeActionResponse, CompletionItem, CompletionOptions, CompletionParams, CompletionResponse,
    DiagnosticOptions, DiagnosticServerCapabilities, DidChangeTextDocumentParams,
    DidCloseTextDocumentParams, DidOpenTextDocumentParams, DidSaveTextDocumentParams,
    DocumentDiagnosticParams, DocumentDiagnosticReport, DocumentDiagnosticReportResult,
    DocumentSymbolParams, DocumentSymbolResponse, FoldingRange, FoldingRangeParams,
    FoldingRangeProviderCapability, FullDocumentDiagnosticReport, GotoDefinitionParams,
    GotoDefinitionResponse, Hover, HoverParams, HoverProviderCapability, InitializeParams,
    InitializeResult, MessageType, OneOf, PrepareRenameResponse, ReferenceParams,
    RelatedFullDocumentDiagnosticReport, RelatedUnchangedDocumentDiagnosticReport, RenameOptions,
    RenameParams, SelectionRange, SelectionRangeParams, SelectionRangeProviderCapability,
    SemanticTokensDeltaParams, SemanticTokensFullDeltaResult, SemanticTokensParams,
    SemanticTokensRangeParams, SemanticTokensRangeResult, SemanticTokensResult,
    SemanticTokensServerCapabilities, ServerCapabilities, TextDocumentPositionParams,
    TextDocumentSyncCapability, TextDocumentSyncKind, UnchangedDocumentDiagnosticReport,
    WorkspaceEdit, WorkspaceSymbolParams,
};
use tower_lsp::{Client, ClientSocket, LanguageServer, LspService};

const MAX_DIAGNOSTIC_RECOMPUTE_ATTEMPTS: usize = 3;

#[derive(Debug)]
pub struct MermanLanguageServer {
    client: Client,
    store: Arc<Mutex<DocumentStore>>,
    semantic_tokens_refresh_supported: AtomicBool,
    diagnostic_pull_supported: AtomicBool,
    diagnostic_refresh_supported: AtomicBool,
    workspace_edit_document_changes_supported: AtomicBool,
    hierarchical_document_symbols_supported: AtomicBool,
}

impl MermanLanguageServer {
    pub fn new(client: Client) -> Self {
        Self {
            client,
            store: Arc::new(Mutex::new(DocumentStore::new())),
            semantic_tokens_refresh_supported: AtomicBool::new(false),
            diagnostic_pull_supported: AtomicBool::new(false),
            diagnostic_refresh_supported: AtomicBool::new(false),
            workspace_edit_document_changes_supported: AtomicBool::new(false),
            hierarchical_document_symbols_supported: AtomicBool::new(false),
        }
    }

    pub fn service() -> (LspService<Self>, ClientSocket) {
        LspService::build(Self::new)
            .custom_method(RULE_CATALOG_METHOD, Self::rule_catalog)
            .custom_method(CONFIG_SCHEMA_METHOD, Self::config_schema)
            .finish()
    }

    pub fn capabilities() -> ServerCapabilities {
        ServerCapabilities {
            text_document_sync: Some(TextDocumentSyncCapability::Kind(
                TextDocumentSyncKind::INCREMENTAL,
            )),
            selection_range_provider: Some(SelectionRangeProviderCapability::Simple(true)),
            hover_provider: Some(HoverProviderCapability::Simple(true)),
            completion_provider: Some(CompletionOptions {
                resolve_provider: Some(true),
                trigger_characters: Some(vec![
                    " ".to_string(),
                    "\n".to_string(),
                    "-".to_string(),
                    ">".to_string(),
                    "%".to_string(),
                    "[".to_string(),
                    "(".to_string(),
                    "{".to_string(),
                    "/".to_string(),
                    "\\".to_string(),
                    "@".to_string(),
                    ":".to_string(),
                ]),
                ..CompletionOptions::default()
            }),
            definition_provider: Some(OneOf::Left(true)),
            references_provider: Some(OneOf::Left(true)),
            rename_provider: Some(OneOf::Right(RenameOptions {
                prepare_provider: Some(true),
                work_done_progress_options: Default::default(),
            })),
            document_symbol_provider: Some(OneOf::Left(true)),
            workspace_symbol_provider: Some(OneOf::Left(true)),
            diagnostic_provider: Some(DiagnosticServerCapabilities::Options(
                Self::diagnostic_options(),
            )),
            code_action_provider: Some(CodeActionProviderCapability::Options(CodeActionOptions {
                code_action_kinds: Some(vec![CodeActionKind::QUICKFIX]),
                work_done_progress_options: Default::default(),
                resolve_provider: Some(false),
            })),
            folding_range_provider: Some(FoldingRangeProviderCapability::Simple(true)),
            semantic_tokens_provider: Some(
                SemanticTokensServerCapabilities::SemanticTokensOptions(semantic_tokens_options()),
            ),
            experimental: Some(experimental_capabilities()),
            ..ServerCapabilities::default()
        }
    }

    pub async fn rule_catalog(&self) -> Result<RuleCatalogResponse> {
        Ok(RuleCatalogResponse::current())
    }

    pub async fn config_schema(&self) -> Result<ConfigSchemaResponse> {
        Ok(ConfigSchemaResponse::current())
    }

    fn diagnostic_options() -> DiagnosticOptions {
        DiagnosticOptions {
            identifier: Some("merman".to_string()),
            inter_file_dependencies: false,
            workspace_diagnostics: false,
            work_done_progress_options: Default::default(),
        }
    }

    #[cfg(test)]
    async fn snapshot_for_uri(
        &self,
        uri: &tower_lsp::lsp_types::Url,
    ) -> Option<Arc<DocumentSnapshot>> {
        snapshot_context::snapshot_context_for_uri(&self.store, uri, SnapshotContextKind::Structure)
            .await
            .ok()
            .flatten()
            .map(|context| context.snapshot)
    }

    async fn structure_snapshot_result<T>(
        &self,
        uri: &tower_lsp::lsp_types::Url,
        compute: impl FnOnce(&DocumentSnapshot) -> Result<Option<T>>,
    ) -> Result<Option<T>> {
        snapshot_context::snapshot_result(&self.store, uri, SnapshotContextKind::Structure, compute)
            .await
    }

    async fn semantic_snapshot_context_for_uri(
        &self,
        uri: &tower_lsp::lsp_types::Url,
    ) -> Result<Option<SnapshotContext>> {
        snapshot_context::snapshot_context_for_uri(
            &self.store,
            uri,
            SnapshotContextKind::SemanticTokens,
        )
        .await
    }

    fn diagnostics_for_document(
        document: &StoredDocument,
        analyzer: &Analyzer,
    ) -> Vec<tower_lsp::lsp_types::Diagnostic> {
        let source = source_descriptor_for_document(&document.uri, document.kind);
        let payload = if let Some(resource_limit) = document.resource_limit {
            AnalysisPayload::new(
                source,
                vec![source_limit_diagnostic_for_len(
                    resource_limit.source_len,
                    resource_limit.max_source_bytes,
                )],
            )
        } else if let Some(discarded_source) = document.discarded_source {
            AnalysisPayload::new(
                source,
                vec![source_discarded_after_limit_change_diagnostic(
                    discarded_source.source_len,
                    discarded_source.previous_max_source_bytes,
                )],
            )
        } else {
            analyze_document(document.text.as_ref(), analyzer, source)
        };
        analysis_payload_to_versioned_diagnostics(&payload, &document.uri, document.version)
    }

    fn diagnostic_result_id(diagnostics: &[tower_lsp::lsp_types::Diagnostic]) -> String {
        let serialized = serde_json::to_vec(diagnostics).unwrap_or_default();
        let mut hasher = std::collections::hash_map::DefaultHasher::new();
        serialized.hash(&mut hasher);
        format!("{:016x}", hasher.finish())
    }

    fn document_diagnostic_report(
        diagnostics: Vec<tower_lsp::lsp_types::Diagnostic>,
        result_id: Option<String>,
        previous_result_id: Option<&str>,
    ) -> DocumentDiagnosticReportResult {
        if let Some(result_id) = result_id.clone()
            && previous_result_id == Some(result_id.as_str())
        {
            return DocumentDiagnosticReportResult::Report(DocumentDiagnosticReport::Unchanged(
                RelatedUnchangedDocumentDiagnosticReport {
                    related_documents: None,
                    unchanged_document_diagnostic_report: UnchangedDocumentDiagnosticReport {
                        result_id,
                    },
                },
            ));
        }

        DocumentDiagnosticReportResult::Report(DocumentDiagnosticReport::Full(
            RelatedFullDocumentDiagnosticReport {
                related_documents: None,
                full_document_diagnostic_report: FullDocumentDiagnosticReport {
                    result_id,
                    items: diagnostics,
                },
            },
        ))
    }

    async fn diagnostics_for_current_context(
        &self,
        context: &DiagnosticContext,
    ) -> Option<Vec<tower_lsp::lsp_types::Diagnostic>> {
        let diagnostics = Self::diagnostics_for_document(&context.document, &context.analyzer);
        let store = self.store.lock().await;
        store
            .is_diagnostic_context_current(context)
            .then_some(diagnostics)
    }

    async fn diagnostics_or_recompute_latest(
        &self,
        mut context: DiagnosticContext,
    ) -> Result<(DiagnosticContext, Vec<tower_lsp::lsp_types::Diagnostic>)> {
        for _ in 0..MAX_DIAGNOSTIC_RECOMPUTE_ATTEMPTS {
            if let Some(diagnostics) = self.diagnostics_for_current_context(&context).await {
                return Ok((context, diagnostics));
            }

            let Some(latest_context) = ({
                let store = self.store.lock().await;
                store.diagnostic_context(&context.document.uri)
            }) else {
                return Ok((context, Vec::new()));
            };
            context = latest_context;
        }

        Err(stale_diagnostic_recompute_error())
    }

    async fn commit_diagnostic_state_if_current(
        &self,
        context: &DiagnosticContext,
        state: DocumentDiagnosticState,
    ) -> Result<()> {
        let mut store = self.store.lock().await;
        if store.set_diagnostic_state_if_current(context, state) {
            Ok(())
        } else {
            Err(stale_diagnostic_recompute_error())
        }
    }

    async fn publish_for_uri(&self, uri: &tower_lsp::lsp_types::Url) {
        if self.diagnostic_pull_supported.load(Ordering::Relaxed) {
            return;
        }

        let context = {
            let store = self.store.lock().await;
            store.diagnostic_context(uri)
        };

        let Some(context) = context else {
            return;
        };

        self.publish_current_diagnostics(&context).await;
    }

    async fn publish_current_diagnostics(&self, context: &DiagnosticContext) {
        if let Some(diagnostics) = self.diagnostics_for_current_context(context).await {
            self.client
                .publish_diagnostics(
                    context.document.uri.clone(),
                    diagnostics,
                    Some(context.document.version),
                )
                .await;
        }
    }

    async fn record_semantic_tokens_state(
        &self,
        context: &SnapshotContext,
        tokens: Vec<tower_lsp::lsp_types::SemanticToken>,
        result_id: Option<String>,
    ) -> Result<()> {
        let mut store = self.store.lock().await;
        if store.set_semantic_tokens_state_if_current(
            context,
            SemanticTokensState::new(result_id, tokens),
        ) {
            Ok(())
        } else {
            Err(SnapshotContextKind::SemanticTokens.stale_error())
        }
    }

    async fn ensure_workspace_symbol_snapshots_current(
        &self,
        contexts: &[SnapshotContext],
    ) -> Result<()> {
        let store = self.store.lock().await;
        if store.workspace_symbol_snapshot_contexts_current(contexts) {
            Ok(())
        } else {
            Err(SnapshotContextKind::WorkspaceSymbols.stale_error())
        }
    }

    async fn workspace_symbol_snapshot_contexts(&self) -> Result<Vec<SnapshotContext>> {
        loop {
            let plan = {
                let store = self.store.lock().await;
                store.workspace_symbol_snapshot_build_plan(WORKSPACE_SYMBOL_SNAPSHOT_BATCH_SIZE)
            };

            if plan.batches.is_empty() {
                return Ok(plan.contexts);
            }

            for batch in plan.batches {
                let built = batch
                    .into_iter()
                    .map(|request| {
                        let snapshot = request.build();
                        (request, snapshot)
                    })
                    .collect();
                let commit = self
                    .store
                    .lock()
                    .await
                    .snapshot_contexts_for_requests(built);
                if commit.stale_open_documents {
                    return Err(SnapshotContextKind::WorkspaceSymbols.stale_error());
                }
                // The current tower-lsp handler path exposes no explicit cancel token here.
                tokio::task::yield_now().await;
            }
        }
    }

    async fn replace_analyzer(&self, options: AnalysisOptions) -> AnalyzerConfigurationChange {
        let mut store = self.store.lock().await;
        store.apply_analyzer_options(options)
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

    fn client_supports_diagnostic_pull(params: &InitializeParams) -> bool {
        params
            .capabilities
            .text_document
            .as_ref()
            .and_then(|text_document| text_document.diagnostic.as_ref())
            .is_some()
    }

    fn client_supports_diagnostic_refresh(params: &InitializeParams) -> bool {
        params
            .capabilities
            .workspace
            .as_ref()
            .and_then(|workspace| workspace.diagnostic.as_ref())
            .and_then(|diagnostic| diagnostic.refresh_support)
            .unwrap_or(false)
    }

    fn client_supports_workspace_edit_document_changes(params: &InitializeParams) -> bool {
        params
            .capabilities
            .workspace
            .as_ref()
            .and_then(|workspace| workspace.workspace_edit.as_ref())
            .and_then(|workspace_edit| workspace_edit.document_changes)
            .unwrap_or(false)
    }

    fn client_supports_hierarchical_document_symbols(params: &InitializeParams) -> bool {
        params
            .capabilities
            .text_document
            .as_ref()
            .and_then(|text_document| text_document.document_symbol.as_ref())
            .and_then(|document_symbol| document_symbol.hierarchical_document_symbol_support)
            .unwrap_or(false)
    }

    async fn apply_initialization_options(
        &self,
        initialization_options: Option<serde_json::Value>,
    ) -> tower_lsp::jsonrpc::Result<()> {
        match initialization_options {
            None => {
                self.replace_analyzer(default_lsp_analysis_options()).await;
                Ok(())
            }
            Some(value) => {
                let options = analysis_options_with_lsp_resource_defaults(
                    analysis_options_from_json_value(&value).map_err(|err| {
                        tower_lsp::jsonrpc::Error::invalid_params(err.to_string())
                    })?,
                );
                self.replace_analyzer(options).await;
                Ok(())
            }
        }
    }

    async fn republish_all(&self) {
        if self.diagnostic_pull_supported.load(Ordering::Relaxed) {
            return;
        }

        let contexts = {
            let store = self.store.lock().await;
            store.diagnostic_contexts()
        };

        for context in contexts {
            self.publish_current_diagnostics(&context).await;
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
        self.diagnostic_pull_supported.store(
            Self::client_supports_diagnostic_pull(&params),
            Ordering::Relaxed,
        );
        self.diagnostic_refresh_supported.store(
            Self::client_supports_diagnostic_refresh(&params),
            Ordering::Relaxed,
        );
        self.workspace_edit_document_changes_supported.store(
            Self::client_supports_workspace_edit_document_changes(&params),
            Ordering::Relaxed,
        );
        self.hierarchical_document_symbols_supported.store(
            Self::client_supports_hierarchical_document_symbols(&params),
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
        let kind = document_kind_for_language_id(&doc.language_id, &doc.uri);
        self.store
            .lock()
            .await
            .open_text(doc.uri.clone(), doc.version, doc.text, kind);
        self.publish_for_uri(&doc.uri).await;
    }

    async fn did_change(&self, params: DidChangeTextDocumentParams) {
        let doc = params.text_document;
        let update = self.store.lock().await.apply_text_changes(
            doc.uri.clone(),
            doc.version,
            params.content_changes,
        );
        if matches!(update, TextDocumentUpdate::Applied) {
            self.publish_for_uri(&doc.uri).await;
        }
    }

    async fn did_save(&self, params: DidSaveTextDocumentParams) {
        let uri = params.text_document.uri;
        self.publish_for_uri(&uri).await;
    }

    async fn did_close(&self, params: DidCloseTextDocumentParams) {
        let uri = params.text_document.uri;
        self.store.lock().await.remove(&uri);
        if !self.diagnostic_pull_supported.load(Ordering::Relaxed) {
            self.client.publish_diagnostics(uri, Vec::new(), None).await;
        }
    }

    async fn did_change_configuration(
        &self,
        params: tower_lsp::lsp_types::DidChangeConfigurationParams,
    ) {
        let options = if params.settings.is_null() {
            default_lsp_analysis_options()
        } else {
            match analysis_options_from_json_value(&params.settings) {
                Ok(options) => analysis_options_with_lsp_resource_defaults(options),
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

        let change = self.replace_analyzer(options).await;
        if change.affects_diagnostics() {
            self.republish_all().await;
        }
        if change.affects_snapshots()
            && self
                .semantic_tokens_refresh_supported
                .load(Ordering::Relaxed)
        {
            let _ = self.client.semantic_tokens_refresh().await;
        }
        if change.affects_diagnostics()
            && self.diagnostic_pull_supported.load(Ordering::Relaxed)
            && self.diagnostic_refresh_supported.load(Ordering::Relaxed)
        {
            let _ = self.client.workspace_diagnostic_refresh().await;
        }
    }

    async fn diagnostic(
        &self,
        params: DocumentDiagnosticParams,
    ) -> Result<DocumentDiagnosticReportResult> {
        let uri = params.text_document.uri;
        let previous_result_id = params.previous_result_id.as_deref();
        let (context, cached) = {
            let store = self.store.lock().await;
            (store.diagnostic_context(&uri), store.diagnostic_state(&uri))
        };
        if let Some(cached) = cached {
            return Ok(Self::document_diagnostic_report(
                cached.diagnostics,
                Some(cached.result_id),
                previous_result_id,
            ));
        }

        let Some(context) = context else {
            let diagnostics = Vec::new();
            let result_id = Some(Self::diagnostic_result_id(&diagnostics));
            return Ok(Self::document_diagnostic_report(
                diagnostics,
                result_id,
                previous_result_id,
            ));
        };

        let (context, diagnostics) = self.diagnostics_or_recompute_latest(context).await?;
        let state = DocumentDiagnosticState {
            result_id: Self::diagnostic_result_id(&diagnostics),
            diagnostics,
        };
        self.commit_diagnostic_state_if_current(&context, state.clone())
            .await?;
        Ok(Self::document_diagnostic_report(
            state.diagnostics,
            Some(state.result_id),
            previous_result_id,
        ))
    }

    async fn completion(&self, params: CompletionParams) -> Result<Option<CompletionResponse>> {
        let uri = params.text_document_position.text_document.uri;
        let position = params.text_document_position.position;

        self.structure_snapshot_result(&uri, |snapshot| {
            Ok(Some(CompletionResponse::List(completion_for_snapshot(
                snapshot, position,
            ))))
        })
        .await
    }

    async fn completion_resolve(&self, item: CompletionItem) -> Result<CompletionItem> {
        Ok(resolve_completion_item(item))
    }

    async fn code_action(&self, params: CodeActionParams) -> Result<Option<CodeActionResponse>> {
        let current_document_version = {
            let store = self.store.lock().await;
            store
                .get(&params.text_document.uri)
                .map(|document| document.version)
        };
        let workspace_edit_encoding = WorkspaceEditEncoding::from_document_changes_support(
            self.workspace_edit_document_changes_supported
                .load(Ordering::Relaxed),
        );
        Ok(code_actions_for_params_with_encoding(
            &params,
            current_document_version,
            workspace_edit_encoding,
        ))
    }

    async fn semantic_tokens_full(
        &self,
        params: SemanticTokensParams,
    ) -> Result<Option<SemanticTokensResult>> {
        let uri = params.text_document.uri;
        let snapshot_context = self.semantic_snapshot_context_for_uri(&uri).await?;

        let Some(snapshot_context) = snapshot_context else {
            return Ok(None);
        };
        let snapshot = &snapshot_context.snapshot;

        let mut tokens = semantic_tokens_for_snapshot(snapshot);
        let result_id = semantic_tokens_result_id(snapshot, &tokens.data);
        tokens.result_id = Some(result_id.clone());
        self.record_semantic_tokens_state(&snapshot_context, tokens.data.clone(), Some(result_id))
            .await?;

        Ok(Some(SemanticTokensResult::Tokens(tokens)))
    }

    async fn semantic_tokens_full_delta(
        &self,
        params: SemanticTokensDeltaParams,
    ) -> Result<Option<SemanticTokensFullDeltaResult>> {
        let uri = params.text_document.uri;
        let snapshot_context = self.semantic_snapshot_context_for_uri(&uri).await?;
        let Some(snapshot_context) = snapshot_context else {
            return Ok(None);
        };
        let snapshot = &snapshot_context.snapshot;

        let current_tokens = semantic_tokens_for_snapshot(snapshot);
        let current_result_id = semantic_tokens_result_id(snapshot, &current_tokens.data);
        let previous = {
            let store = self.store.lock().await;
            store.semantic_tokens_state_for_delta(&uri, params.previous_result_id.as_str())
        };
        let delta = match previous {
            Some(previous) => semantic_tokens_delta_result(
                &previous.tokens,
                &current_tokens.data,
                current_result_id.clone(),
            ),
            _ => {
                let mut tokens = current_tokens.clone();
                tokens.result_id = Some(current_result_id.clone());
                SemanticTokensFullDeltaResult::Tokens(tokens)
            }
        };

        self.record_semantic_tokens_state(
            &snapshot_context,
            current_tokens.data,
            Some(current_result_id),
        )
        .await?;

        Ok(Some(delta))
    }

    async fn semantic_tokens_range(
        &self,
        params: SemanticTokensRangeParams,
    ) -> Result<Option<SemanticTokensRangeResult>> {
        let uri = params.text_document.uri;
        let snapshot_context = self.semantic_snapshot_context_for_uri(&uri).await?;

        let Some(snapshot_context) = snapshot_context else {
            return Ok(None);
        };
        let result = semantic_tokens_for_snapshot_range(&snapshot_context.snapshot, params.range);
        snapshot_context::ensure_snapshot_current(
            &self.store,
            &snapshot_context,
            SnapshotContextKind::SemanticTokens,
        )
        .await?;

        Ok(Some(result.into()))
    }

    async fn hover(&self, params: HoverParams) -> Result<Option<Hover>> {
        let uri = params.text_document_position_params.text_document.uri;
        let position = params.text_document_position_params.position;

        self.structure_snapshot_result(&uri, |snapshot| Ok(structure_hover(snapshot, position)))
            .await
    }

    async fn selection_range(
        &self,
        params: SelectionRangeParams,
    ) -> Result<Option<Vec<SelectionRange>>> {
        let uri = params.text_document.uri;

        self.structure_snapshot_result(&uri, |snapshot| {
            Ok(structure_selection_ranges(snapshot, &params.positions))
        })
        .await
    }

    async fn folding_range(&self, params: FoldingRangeParams) -> Result<Option<Vec<FoldingRange>>> {
        let uri = params.text_document.uri;

        self.structure_snapshot_result(&uri, |snapshot| {
            Ok(Some(structure_folding_ranges(snapshot)))
        })
        .await
    }

    async fn document_symbol(
        &self,
        params: DocumentSymbolParams,
    ) -> Result<Option<DocumentSymbolResponse>> {
        let uri = params.text_document.uri;
        let hierarchical_supported = self
            .hierarchical_document_symbols_supported
            .load(Ordering::Relaxed);

        self.structure_snapshot_result(&uri, |snapshot| {
            Ok(Some(structure_document_symbols_with_hierarchy_support(
                snapshot,
                hierarchical_supported,
            )))
        })
        .await
    }

    async fn goto_definition(
        &self,
        params: GotoDefinitionParams,
    ) -> Result<Option<GotoDefinitionResponse>> {
        let uri = params.text_document_position_params.text_document.uri;
        let position = params.text_document_position_params.position;

        self.structure_snapshot_result(&uri, |snapshot| {
            Ok(structure_goto_definition(snapshot, position))
        })
        .await
    }

    async fn references(
        &self,
        params: ReferenceParams,
    ) -> Result<Option<Vec<tower_lsp::lsp_types::Location>>> {
        let uri = params.text_document_position.text_document.uri;
        let position = params.text_document_position.position;

        self.structure_snapshot_result(&uri, |snapshot| {
            Ok(structure_references(
                snapshot,
                position,
                params.context.include_declaration,
            ))
        })
        .await
    }

    async fn prepare_rename(
        &self,
        params: TextDocumentPositionParams,
    ) -> Result<Option<PrepareRenameResponse>> {
        let uri = params.text_document.uri;
        let position = params.position;

        self.structure_snapshot_result(&uri, |snapshot| {
            Ok(structure_prepare_rename(snapshot, position))
        })
        .await
    }

    async fn rename(&self, params: RenameParams) -> Result<Option<WorkspaceEdit>> {
        let uri = params.text_document_position.text_document.uri.clone();
        let workspace_edit_encoding = WorkspaceEditEncoding::from_document_changes_support(
            self.workspace_edit_document_changes_supported
                .load(Ordering::Relaxed),
        );

        self.structure_snapshot_result(&uri, |snapshot| {
            structure_rename_with_workspace_edit_encoding(snapshot, params, workspace_edit_encoding)
        })
        .await
    }

    async fn symbol(
        &self,
        params: WorkspaceSymbolParams,
    ) -> Result<Option<Vec<tower_lsp::lsp_types::SymbolInformation>>> {
        let contexts = self.workspace_symbol_snapshot_contexts().await?;

        let snapshots = contexts
            .iter()
            .map(|context| Arc::clone(&context.snapshot))
            .collect::<Vec<_>>();
        let symbols = structure_workspace_symbols_for_snapshots(&snapshots, &params.query);

        self.ensure_workspace_symbol_snapshots_current(&contexts)
            .await?;

        Ok(Some(symbols))
    }
}

fn stale_diagnostic_recompute_error() -> tower_lsp::jsonrpc::Error {
    let mut error = tower_lsp::jsonrpc::Error::content_modified();
    error.message = "diagnostic document changed while recomputing".into();
    error
}

fn source_descriptor_for_document(
    uri: &tower_lsp::lsp_types::Url,
    kind: DocumentKind,
) -> merman_analysis::SourceDescriptor {
    let source_kind = match kind {
        DocumentKind::Diagram => SourceKind::Diagram,
        DocumentKind::Markdown => SourceKind::Markdown,
        DocumentKind::Mdx => SourceKind::Mdx,
    };
    source_descriptor_for_kind(Some(uri.as_str()), source_kind)
}

fn document_kind_for_language_id(
    language_id: &str,
    uri: &tower_lsp::lsp_types::Url,
) -> DocumentKind {
    match language_id {
        "markdown" => DocumentKind::Markdown,
        "mdx" => DocumentKind::Mdx,
        "mermaid" => DocumentKind::Diagram,
        _ => DocumentKind::from_path(uri.path()),
    }
}

#[cfg(test)]
mod tests;
