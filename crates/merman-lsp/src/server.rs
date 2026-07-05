use crate::code_actions::code_actions_for_params;
use crate::completion::{completion_for_snapshot, resolve_completion_item};
use crate::diagnostics::analysis_payload_to_versioned_diagnostics;
use crate::document_store::{
    AnalyzerConfigurationChange, DiagnosticContext, DocumentStore, SemanticTokensState,
    SnapshotContext, StoredDocument, WorkspaceSnapshotRefreshBudget,
};
use crate::protocol::{
    CONFIG_SCHEMA_METHOD, ConfigSchemaResponse, RULE_CATALOG_METHOD, RuleCatalogResponse,
    experimental_capabilities,
};
use crate::semantic_tokens::{
    semantic_tokens_delta_result, semantic_tokens_for_snapshot, semantic_tokens_for_snapshot_range,
    semantic_tokens_options, semantic_tokens_result_id,
};
use crate::snapshot::DocumentSnapshot;
use crate::snapshot_context::{self, SnapshotContextKind};
use crate::structure::{
    document_symbols as structure_document_symbols, folding_ranges as structure_folding_ranges,
    goto_definition as structure_goto_definition, hover as structure_hover,
    prepare_rename as structure_prepare_rename, references as structure_references,
    rename as structure_rename, selection_ranges as structure_selection_ranges,
    workspace_symbols_for_snapshots as structure_workspace_symbols_for_snapshots,
};
use merman_analysis::{
    AnalysisOptions, Analyzer, SourceKind, document::analyze_document,
    options_json::analysis_options_from_json_value, source_descriptor_for_kind,
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
}

impl MermanLanguageServer {
    pub fn new(client: Client) -> Self {
        Self {
            client,
            store: Arc::new(Mutex::new(DocumentStore::new())),
            semantic_tokens_refresh_supported: AtomicBool::new(false),
            diagnostic_pull_supported: AtomicBool::new(false),
            diagnostic_refresh_supported: AtomicBool::new(false),
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
            text_document_sync: Some(TextDocumentSyncCapability::Kind(TextDocumentSyncKind::FULL)),
            selection_range_provider: Some(SelectionRangeProviderCapability::Simple(true)),
            hover_provider: Some(HoverProviderCapability::Simple(true)),
            completion_provider: Some(CompletionOptions {
                resolve_provider: Some(true),
                trigger_characters: Some(vec![
                    " ".to_string(),
                    "\n".to_string(),
                    "-".to_string(),
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
        let payload = analyze_document(document.text.as_ref(), analyzer, source);
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
    ) -> Result<Vec<tower_lsp::lsp_types::Diagnostic>> {
        for _ in 0..MAX_DIAGNOSTIC_RECOMPUTE_ATTEMPTS {
            if let Some(diagnostics) = self.diagnostics_for_current_context(&context).await {
                return Ok(diagnostics);
            }

            let Some(latest_context) = ({
                let store = self.store.lock().await;
                store.diagnostic_context(&context.document.uri)
            }) else {
                return Ok(Vec::new());
            };
            context = latest_context;
        }

        Err(stale_diagnostic_recompute_error())
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
        snapshot_context::ensure_snapshot_contexts_current(
            &self.store,
            contexts,
            SnapshotContextKind::WorkspaceSymbols,
        )
        .await
    }

    async fn workspace_symbol_snapshot_contexts(&self) -> Result<Vec<SnapshotContext>> {
        loop {
            let plan = {
                let store = self.store.lock().await;
                store.workspace_symbol_snapshot_build_plan(
                    WorkspaceSnapshotRefreshBudget::workspace_symbols(),
                )
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
            .upsert_text(doc.uri.clone(), doc.version, doc.text, kind);
        self.publish_for_uri(&doc.uri).await;
    }

    async fn did_change(&self, params: DidChangeTextDocumentParams) {
        let doc = params.text_document;
        let Some(change) = params.content_changes.into_iter().last() else {
            return;
        };
        let mut store = self.store.lock().await;
        let Some(kind) = store.get(&doc.uri).map(|current| current.kind) else {
            return;
        };

        store.upsert_text(doc.uri.clone(), doc.version, change.text, kind);
        drop(store);
        self.publish_for_uri(&doc.uri).await;
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
        let context = {
            let store = self.store.lock().await;
            store.diagnostic_context(&uri)
        };
        let Some(context) = context else {
            let diagnostics = Vec::new();
            let result_id = Some(Self::diagnostic_result_id(&diagnostics));
            return Ok(Self::document_diagnostic_report(
                diagnostics,
                result_id,
                params.previous_result_id.as_deref(),
            ));
        };

        let diagnostics = self.diagnostics_or_recompute_latest(context).await?;
        let result_id = Some(Self::diagnostic_result_id(&diagnostics));
        Ok(Self::document_diagnostic_report(
            diagnostics,
            result_id,
            params.previous_result_id.as_deref(),
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
        Ok(code_actions_for_params(&params, current_document_version))
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
        let delta = {
            let store = self.store.lock().await;
            let previous =
                store.semantic_tokens_state_for_delta(&uri, params.previous_result_id.as_str());
            match previous {
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

        self.structure_snapshot_result(&uri, |snapshot| {
            Ok(Some(structure_document_symbols(snapshot)))
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

        self.structure_snapshot_result(&uri, |snapshot| structure_rename(snapshot, params))
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
mod tests {
    use super::MermanLanguageServer;
    use super::stale_diagnostic_recompute_error;
    use crate::diagnostics::analysis_diagnostic_to_versioned_lsp;
    use crate::document_store::{
        DocumentStore, StoredDocument, WORKSPACE_SYMBOL_SNAPSHOT_BATCH_SIZE,
        WORKSPACE_SYMBOL_SNAPSHOT_BUILD_BUDGET,
    };
    use crate::protocol::{
        CONFIG_SCHEMA_METHOD, RULE_CATALOG_METHOD, RULE_CATALOG_RESPONSE_VERSION,
    };
    use crate::structure::{
        document_symbols, folding_ranges, goto_definition, hover, prepare_rename, references,
        rename, selection_ranges,
    };
    use merman_analysis::{
        AnalysisDiagnostic, AnalysisOptions, AnalysisRuleConfig, DiagnosticCategory, DiagnosticFix,
        DiagnosticFixEdit, DiagnosticSeverity, SourceMap,
    };
    use merman_core::ParseOptions;
    use merman_editor_core::DocumentKind;
    use tower::{Service, ServiceExt};
    use tower_lsp::LanguageServer;
    use tower_lsp::jsonrpc::Request;
    use tower_lsp::lsp_types::SemanticTokensResult;
    use tower_lsp::lsp_types::{
        CodeActionContext, CodeActionKind, CodeActionOrCommand, CodeActionParams,
        CodeActionProviderCapability, DidChangeTextDocumentParams, DidOpenTextDocumentParams,
        DocumentChanges, DocumentSymbolResponse, FoldingRangeParams,
        FoldingRangeProviderCapability, GotoDefinitionResponse, HoverContents, HoverParams,
        InitializeParams, NumberOrString, Position, Range, RenameParams, SelectionRangeParams,
        SelectionRangeProviderCapability, SemanticTokensFullOptions, SemanticTokensParams,
        SemanticTokensRangeParams, SemanticTokensRangeResult, SemanticTokensServerCapabilities,
        TextDocumentContentChangeEvent, TextDocumentIdentifier, TextDocumentItem,
        TextDocumentPositionParams, TextDocumentSyncCapability, TextDocumentSyncKind, Url,
        VersionedTextDocumentIdentifier, WorkspaceSymbolParams,
    };
    use tower_lsp::lsp_types::{HoverProviderCapability, OneOf};

    #[test]
    fn snapshot_build_requests_keep_cached_contexts_invalidatable() {
        let mut store = DocumentStore::new();
        let uri = Url::parse("file:///tmp/workspace-symbols.mmd").unwrap();
        store.upsert(uri.clone(), 1, "flowchart TD\nA[old] --> B\n".to_string());

        let (contexts, requests) = store.snapshot_build_requests();
        assert_eq!(contexts.len(), 1);
        assert!(requests.is_empty());
        assert!(store.is_snapshot_contexts_current(&contexts));

        store.upsert_text(
            uri,
            2,
            "flowchart TD\nA[new] --> C\n".to_string(),
            DocumentKind::Diagram,
        );

        assert!(!store.is_snapshot_contexts_current(&contexts));
    }

    #[test]
    fn analyzer_configuration_change_classifies_diagnostic_only_rule_changes() {
        let current = AnalysisOptions::default();
        let next = AnalysisOptions::default().with_rule_config(
            AnalysisRuleConfig::default()
                .with_rule_severity("merman.parse.no_diagram", DiagnosticSeverity::Hint),
        );

        assert_eq!(
            crate::document_store::analyzer_configuration_change(&current, &next),
            crate::document_store::AnalyzerConfigurationChange::DiagnosticsOnly
        );
    }

    #[test]
    fn analyzer_configuration_change_classifies_snapshot_affecting_changes() {
        let current = AnalysisOptions::default();
        let changed_parse = AnalysisOptions::default().with_parse_options(ParseOptions::lenient());
        let changed_resource = AnalysisOptions::default().with_max_source_bytes(Some(1));
        let changed_date =
            AnalysisOptions::default().with_fixed_today(Some("2026-07-02".parse().unwrap()));

        for next in [changed_parse, changed_resource, changed_date] {
            assert_eq!(
                crate::document_store::analyzer_configuration_change(&current, &next),
                crate::document_store::AnalyzerConfigurationChange::SnapshotAffecting
            );
        }
    }

    #[test]
    fn analyzer_configuration_change_classifies_unchanged_options() {
        let current = AnalysisOptions::default();

        assert_eq!(
            crate::document_store::analyzer_configuration_change(&current, &current),
            crate::document_store::AnalyzerConfigurationChange::Unchanged
        );
    }

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
            capabilities.selection_range_provider,
            Some(SelectionRangeProviderCapability::Simple(true))
        ));
        assert!(matches!(
            capabilities.folding_range_provider,
            Some(FoldingRangeProviderCapability::Simple(true))
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
            Some(OneOf::Right(options)) if options.prepare_provider == Some(true)
        ));
        assert!(matches!(
            capabilities.workspace_symbol_provider,
            Some(OneOf::Left(true))
        ));
        assert!(matches!(
            capabilities.completion_provider,
            Some(ref options) if options.resolve_provider == Some(true)
                && options.trigger_characters.as_deref() == Some(&[
                    " ".to_string(),
                    "\n".to_string(),
                    "-".to_string(),
                    "@".to_string(),
                    ":".to_string(),
                ])
        ));
        assert!(matches!(
            capabilities.semantic_tokens_provider,
            Some(SemanticTokensServerCapabilities::SemanticTokensOptions(ref options))
                if matches!(options.full, Some(SemanticTokensFullOptions::Delta { delta: Some(true) }))
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
        assert_eq!(
            capabilities.experimental.as_ref().unwrap()["merman"]["requests"]["ruleCatalog"],
            RULE_CATALOG_METHOD
        );
        assert_eq!(
            capabilities.experimental.as_ref().unwrap()["merman"]["requests"]["configSchema"],
            CONFIG_SCHEMA_METHOD
        );
    }

    #[test]
    fn diagnostics_use_stored_markdown_kind_for_extensionless_documents() {
        let uri = Url::parse("untitled:notes").unwrap();
        let document = StoredDocument {
            uri: uri.clone(),
            version: 7,
            text: "before\n```mermaid\nflowchart TD\nA[unterminated\n```\nafter\n".into(),
            kind: DocumentKind::Markdown,
        };
        let diagnostics = MermanLanguageServer::diagnostics_for_document(
            &document,
            &merman_analysis::Analyzer::new(),
        );

        assert!(
            diagnostics.iter().all(|diagnostic| {
                diagnostic.code
                    != Some(NumberOrString::String(
                        "merman.parse.no_diagram".to_string(),
                    ))
            }),
            "expected markdown document analysis, got {diagnostics:?}"
        );
        let parse_diagnostic = diagnostics
            .iter()
            .find(|diagnostic| {
                diagnostic.code
                    == Some(NumberOrString::String(
                        "merman.parse.diagram_parse".to_string(),
                    ))
            })
            .expect("expected diagram parse diagnostic from markdown fence");
        assert!(
            parse_diagnostic.range.start.line >= 2,
            "expected markdown fence body range, got {:?}",
            parse_diagnostic.range
        );
        assert_eq!(
            parse_diagnostic
                .data
                .as_ref()
                .and_then(|data| data.get("documentVersion")),
            Some(&serde_json::json!(7))
        );
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
    fn diagnostic_pull_support_comes_from_text_document_client_capabilities() {
        let mut params = InitializeParams::default();
        assert!(!MermanLanguageServer::client_supports_diagnostic_pull(
            &params
        ));

        params.capabilities.workspace = Some(Default::default());
        params.capabilities.workspace.as_mut().unwrap().diagnostic = Some(
            tower_lsp::lsp_types::DiagnosticWorkspaceClientCapabilities {
                refresh_support: Some(true),
            },
        );
        assert!(!MermanLanguageServer::client_supports_diagnostic_pull(
            &params
        ));

        params.capabilities.text_document = Some(Default::default());
        params
            .capabilities
            .text_document
            .as_mut()
            .unwrap()
            .diagnostic = Some(tower_lsp::lsp_types::DiagnosticClientCapabilities {
            dynamic_registration: None,
            related_document_support: None,
        });
        assert!(MermanLanguageServer::client_supports_diagnostic_pull(
            &params
        ));
    }

    #[test]
    fn diagnostic_refresh_support_comes_from_workspace_client_capabilities() {
        let mut params = InitializeParams::default();
        assert!(!MermanLanguageServer::client_supports_diagnostic_refresh(
            &params
        ));

        params.capabilities.text_document = Some(Default::default());
        params
            .capabilities
            .text_document
            .as_mut()
            .unwrap()
            .diagnostic = Some(tower_lsp::lsp_types::DiagnosticClientCapabilities {
            dynamic_registration: None,
            related_document_support: None,
        });
        assert!(!MermanLanguageServer::client_supports_diagnostic_refresh(
            &params
        ));

        params.capabilities.workspace = Some(Default::default());
        params.capabilities.workspace.as_mut().unwrap().diagnostic = Some(
            tower_lsp::lsp_types::DiagnosticWorkspaceClientCapabilities {
                refresh_support: None,
            },
        );
        assert!(!MermanLanguageServer::client_supports_diagnostic_refresh(
            &params
        ));

        params
            .capabilities
            .workspace
            .as_mut()
            .unwrap()
            .diagnostic
            .as_mut()
            .unwrap()
            .refresh_support = Some(true);
        assert!(MermanLanguageServer::client_supports_diagnostic_refresh(
            &params
        ));
    }

    #[tokio::test(flavor = "current_thread")]
    async fn did_open_defers_editor_snapshot_until_editor_request() {
        let (service, _socket) = MermanLanguageServer::service();
        let server = service.inner();
        let uri = Url::parse("file:///tmp/example.mmd").unwrap();

        server
            .did_open(DidOpenTextDocumentParams {
                text_document: TextDocumentItem {
                    uri: uri.clone(),
                    language_id: "mermaid".to_string(),
                    version: 1,
                    text: "flowchart TD\nsubgraph group\nA-->B\nend\n".to_string(),
                },
            })
            .await;

        {
            let store = server.store.lock().await;
            assert!(store.get(&uri).is_some());
            assert!(!store.has_snapshot(&uri));
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
        let store = server.store.lock().await;
        assert!(store.has_snapshot(&uri));
    }

    #[tokio::test(flavor = "current_thread")]
    async fn did_open_uses_language_id_and_change_preserves_document_kind() {
        let (service, _socket) = MermanLanguageServer::service();
        let server = service.inner();
        let uri = Url::parse("untitled:notes").unwrap();

        server
            .did_open(DidOpenTextDocumentParams {
                text_document: TextDocumentItem {
                    uri: uri.clone(),
                    language_id: "markdown".to_string(),
                    version: 1,
                    text: "```mermaid\nflowchart TD\nA-->B\n```\n".to_string(),
                },
            })
            .await;

        let snapshot = server
            .snapshot_for_uri(&uri)
            .await
            .expect("expected markdown snapshot");
        assert_eq!(snapshot.kind, DocumentKind::Markdown);
        assert_eq!(snapshot.fences.len(), 1);
        assert_eq!(
            snapshot.fences[0].diagram_type.as_deref(),
            Some("flowchart-v2")
        );

        server
            .did_change(DidChangeTextDocumentParams {
                text_document: VersionedTextDocumentIdentifier {
                    uri: uri.clone(),
                    version: 2,
                },
                content_changes: vec![TextDocumentContentChangeEvent {
                    range: None,
                    range_length: None,
                    text: "```mermaid\nsequenceDiagram\nAlice->>Bob: Hi\n```\n".to_string(),
                }],
            })
            .await;

        let snapshot = server
            .snapshot_for_uri(&uri)
            .await
            .expect("expected changed markdown snapshot");
        assert_eq!(snapshot.kind, DocumentKind::Markdown);
        assert_eq!(snapshot.fences.len(), 1);
        assert_eq!(snapshot.fences[0].diagram_type.as_deref(), Some("sequence"));
    }

    #[tokio::test(flavor = "current_thread")]
    async fn stale_diagnostic_context_returns_content_modified_error() {
        let (service, _socket) = MermanLanguageServer::service();
        let server = service.inner();
        let uri = Url::parse("file:///tmp/example.mmd").unwrap();

        {
            let mut store = server.store.lock().await;
            store.upsert_text(
                uri.clone(),
                1,
                "flowchart TD\nA-->B\n".to_string(),
                DocumentKind::Diagram,
            );
        }
        let context = {
            let store = server.store.lock().await;
            store
                .diagnostic_context(&uri)
                .expect("expected diagnostic context")
        };
        {
            let mut store = server.store.lock().await;
            store.upsert_text(
                uri.clone(),
                2,
                "flowchart TD\nA-->C\n".to_string(),
                DocumentKind::Diagram,
            );
        }

        let error = server
            .diagnostics_for_current_context(&context)
            .await
            .ok_or_else(stale_diagnostic_recompute_error)
            .expect_err("stale context should fail");

        assert_eq!(error.code, tower_lsp::jsonrpc::ErrorCode::ContentModified);
        assert!(error.message.contains("diagnostic document changed"));
    }

    #[tokio::test(flavor = "current_thread")]
    async fn stale_semantic_tokens_record_returns_content_modified_error() {
        let (service, _socket) = MermanLanguageServer::service();
        let server = service.inner();
        let uri = Url::parse("file:///tmp/example.mmd").unwrap();

        {
            let mut store = server.store.lock().await;
            store.upsert_text(
                uri.clone(),
                1,
                "flowchart TD\nA-->B\n".to_string(),
                DocumentKind::Diagram,
            );
        }
        let context = crate::snapshot_context::snapshot_context_for_uri(
            &server.store,
            &uri,
            crate::snapshot_context::SnapshotContextKind::SemanticTokens,
        )
        .await
        .expect("snapshot context build should not fail")
        .expect("expected snapshot context");
        {
            let mut store = server.store.lock().await;
            store.upsert_text(
                uri.clone(),
                2,
                "flowchart TD\nA-->C\n".to_string(),
                DocumentKind::Diagram,
            );
        }

        let error = server
            .record_semantic_tokens_state(&context, Vec::new(), Some("stale-result".to_string()))
            .await
            .expect_err("stale semantic tokens should fail");

        assert_eq!(error.code, tower_lsp::jsonrpc::ErrorCode::ContentModified);
        assert!(error.message.contains("semantic tokens document changed"));
    }

    #[tokio::test(flavor = "current_thread")]
    async fn stale_initial_diagnostic_context_recomputes_latest_document() {
        let (service, _socket) = MermanLanguageServer::service();
        let server = service.inner();
        let uri = Url::parse("file:///tmp/example.mmd").unwrap();

        {
            let mut store = server.store.lock().await;
            store.upsert_text(
                uri.clone(),
                1,
                "flowchart TD\nA-->B\n".to_string(),
                DocumentKind::Diagram,
            );
        }
        let context = {
            let store = server.store.lock().await;
            store
                .diagnostic_context(&uri)
                .expect("expected diagnostic context")
        };
        {
            let mut store = server.store.lock().await;
            store.upsert_text(
                uri.clone(),
                2,
                "flowchart TD\nA[unterminated\n".to_string(),
                DocumentKind::Diagram,
            );
        }

        let diagnostics = server
            .diagnostics_or_recompute_latest(context)
            .await
            .expect("latest diagnostic context should recompute");
        let parse_diagnostic = diagnostics
            .iter()
            .find(|diagnostic| {
                diagnostic.code
                    == Some(NumberOrString::String(
                        "merman.parse.diagram_parse".to_string(),
                    ))
            })
            .expect("expected latest parse diagnostic");
        let data = parse_diagnostic
            .data
            .as_ref()
            .expect("expected diagnostic data");
        assert_eq!(data["documentVersion"], 2);
    }

    #[tokio::test(flavor = "current_thread")]
    async fn code_action_rejects_stale_diagnostic_edits_after_document_change() {
        let (service, _socket) = MermanLanguageServer::service();
        let server = service.inner();
        let uri = Url::parse("file:///tmp/example.mmd").unwrap();

        server
            .did_open(DidOpenTextDocumentParams {
                text_document: TextDocumentItem {
                    uri: uri.clone(),
                    language_id: "mermaid".to_string(),
                    version: 1,
                    text: "bad".to_string(),
                },
            })
            .await;

        let map = SourceMap::new("bad");
        let stale_diagnostic = AnalysisDiagnostic::error(
            "merman.test.fix",
            DiagnosticCategory::Semantic,
            "test diagnostic",
        )
        .with_fix(
            DiagnosticFix::new(
                "Replace invalid text",
                vec![DiagnosticFixEdit::new(
                    map.whole_source_span().unwrap(),
                    "fixed",
                )],
            )
            .preferred(),
        );

        server
            .did_change(DidChangeTextDocumentParams {
                text_document: VersionedTextDocumentIdentifier {
                    uri: uri.clone(),
                    version: 2,
                },
                content_changes: vec![TextDocumentContentChangeEvent {
                    range: None,
                    range_length: None,
                    text: "flowchart TD\nA-->B\n".to_string(),
                }],
            })
            .await;

        let actions = server
            .code_action(CodeActionParams {
                text_document: TextDocumentIdentifier { uri: uri.clone() },
                range: Range {
                    start: Position::new(0, 0),
                    end: Position::new(0, 3),
                },
                context: CodeActionContext {
                    diagnostics: vec![analysis_diagnostic_to_versioned_lsp(
                        &stale_diagnostic,
                        &uri,
                        1,
                    )],
                    only: Some(vec![CodeActionKind::QUICKFIX]),
                    trigger_kind: None,
                },
                work_done_progress_params: Default::default(),
                partial_result_params: Default::default(),
            })
            .await
            .unwrap();

        assert!(actions.is_none());
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

        let selection_ranges = selection_ranges(&snapshot, &[Position::new(2, 0)]).unwrap();
        assert_eq!(selection_ranges.len(), 1);
        assert!(selection_ranges[0].parent.is_some());

        let markdown_uri = Url::parse("file:///tmp/example.md").unwrap();
        let markdown_snapshot = store.upsert(
            markdown_uri,
            1,
            "before\n```mermaid\nflowchart TD\nA-->B\n```\nafter\n".to_string(),
        );
        let folding_ranges = folding_ranges(&markdown_snapshot);
        assert!(
            folding_ranges
                .iter()
                .any(|range| range.start_line == 1 && range.end_line == 4)
        );

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
        let edit = rename.unwrap();
        assert!(edit.changes.is_none());
        let document_changes = match edit.document_changes.unwrap() {
            DocumentChanges::Edits(edits) => edits,
            other => panic!("unexpected document changes: {other:?}"),
        };
        assert_eq!(document_changes.len(), 1);
        assert_eq!(document_changes[0].text_document.version, Some(1));
        assert_eq!(document_changes[0].edits.len(), 2);
    }

    #[tokio::test(flavor = "current_thread")]
    async fn lsp_handlers_return_hover_and_symbols() {
        let (service, _socket) = MermanLanguageServer::service();
        let server = service.inner();
        let uri = Url::parse("file:///tmp/example.mmd").unwrap();

        {
            let mut store = server.store.lock().await;
            store.upsert(
                uri.clone(),
                1,
                "flowchart TD\nsubgraph group\nA-->B\nend\n".to_string(),
            );
            store.upsert(
                Url::parse("file:///tmp/example.md").unwrap(),
                1,
                "before\n```mermaid\nflowchart TD\nA-->B\n```\nafter\n".to_string(),
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

        let selection_ranges = server
            .selection_range(SelectionRangeParams {
                text_document: TextDocumentIdentifier { uri: uri.clone() },
                positions: vec![Position::new(2, 0)],
                work_done_progress_params: Default::default(),
                partial_result_params: Default::default(),
            })
            .await
            .unwrap()
            .expect("expected selection range response");
        assert_eq!(selection_ranges.len(), 1);
        assert!(selection_ranges[0].parent.is_some());

        let folding_ranges = server
            .folding_range(FoldingRangeParams {
                text_document: TextDocumentIdentifier {
                    uri: Url::parse("file:///tmp/example.md").unwrap(),
                },
                work_done_progress_params: Default::default(),
                partial_result_params: Default::default(),
            })
            .await
            .unwrap()
            .expect("expected folding range response");
        assert!(
            folding_ranges
                .iter()
                .any(|range| range.start_line == 1 && range.end_line == 4)
        );

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
                    diagnostics: vec![analysis_diagnostic_to_versioned_lsp(&diagnostic, &uri, 1)],
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

    #[tokio::test(flavor = "current_thread")]
    async fn workspace_symbols_refreshes_more_than_the_snapshot_budget_on_first_request() {
        let (service, _socket) = MermanLanguageServer::service();
        let server = service.inner();
        let document_count =
            WORKSPACE_SYMBOL_SNAPSHOT_BUILD_BUDGET + WORKSPACE_SYMBOL_SNAPSHOT_BATCH_SIZE + 1;
        let last_index = document_count - 1;
        let last_symbol = format!("target_{last_index:02}");
        let last_uri = Url::parse(&format!("file:///tmp/workspace-{last_index:02}.mmd")).unwrap();

        {
            let mut store = server.store.lock().await;
            for index in 0..document_count {
                let uri = Url::parse(&format!("file:///tmp/workspace-{index:02}.mmd")).unwrap();
                store.upsert_text(
                    uri,
                    1,
                    format!("flowchart TD\nsubgraph target_{index:02}\nA{index}-->B{index}\nend\n"),
                    DocumentKind::Diagram,
                );
            }
        }

        let workspace_symbols = server
            .symbol(WorkspaceSymbolParams {
                partial_result_params: Default::default(),
                work_done_progress_params: Default::default(),
                query: last_symbol.clone(),
            })
            .await
            .unwrap()
            .expect("expected workspace symbol response");

        assert!(
            workspace_symbols
                .iter()
                .any(|symbol| symbol.name == last_symbol && symbol.location.uri == last_uri),
            "workspace symbol request omitted the document beyond the old snapshot budget"
        );

        let store = server.store.lock().await;
        let (_contexts, requests) = store.snapshot_build_requests();
        assert!(
            requests.is_empty(),
            "workspace symbol refresh should not leave current open documents uncached"
        );
    }

    #[tokio::test(flavor = "current_thread")]
    async fn lsp_service_serves_rule_catalog_custom_request() {
        let (mut service, _socket) = MermanLanguageServer::service();
        let initialize = Request::build("initialize")
            .params(serde_json::to_value(InitializeParams::default()).unwrap())
            .id(1)
            .finish();

        let initialize_response = service
            .ready()
            .await
            .unwrap()
            .call(initialize)
            .await
            .unwrap()
            .expect("initialize response");
        assert!(initialize_response.is_ok());

        let request = Request::build(RULE_CATALOG_METHOD).id(2).finish();
        let response = service
            .ready()
            .await
            .unwrap()
            .call(request)
            .await
            .unwrap()
            .expect("rule catalog response");
        let result = response.result().expect("rule catalog result");

        assert_eq!(result["version"], RULE_CATALOG_RESPONSE_VERSION);
        assert!(result["rules"].as_array().unwrap().iter().any(|rule| {
            rule["id"] == "merman.authoring.flowchart.explicit_direction"
                && rule["origin"] == "merman_authoring"
                && rule["evidence"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .any(|value| value == "docs/adr/0072-lint-rule-governance.md")
        }));
    }
}
