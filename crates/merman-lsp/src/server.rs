use crate::client_profile::ClientProtocolProfile;
use crate::code_actions::code_actions_for_snapshot_diagnostics_with_profile;
use crate::completion::{
    completion_for_snapshot_with_profile, resolve_completion_item_with_profile,
};
use crate::diagnostics::analysis_payload_to_versioned_diagnostics_with_profile;
use crate::document_store::{
    AnalyzerConfigurationChange, AnalyzerOptionsPreparation, DiagnosticContext,
    DiagnosticReprojectionPlan, DocumentDiagnosticState, DocumentStore, DocumentSyncError,
    PreparedDocumentText, SemanticTokensState, StoredDocument, TextChangePreparation,
    analysis_options_with_lsp_resource_defaults, default_lsp_analysis_options,
};
use crate::protocol::{
    CONFIG_SCHEMA_METHOD, ConfigSchemaResponse, DiagnosticVersionData, RULE_CATALOG_METHOD,
    RuleCatalogResponse, experimental_capabilities,
};
use crate::refresh_coordinator::RefreshCoordinator;
use crate::refresh_transport::{MermanClientSocket, RefreshClient};
use crate::semantic_tokens::{
    semantic_tokens_delta_result, semantic_tokens_for_snapshot_range_with_profile,
    semantic_tokens_for_snapshot_with_profile, semantic_tokens_options_with_profile,
    semantic_tokens_result_id,
};
use crate::session::{
    ClientEffectDispatcher, ClientEffectKey, MermanLspService, commit_active_mutation,
};
use crate::snapshot::{DocumentSnapshot, SnapshotContext};
use crate::snapshot_context::{self, SnapshotContextKind};
use crate::structure::{
    document_symbols_with_hierarchy_support as structure_document_symbols_with_hierarchy_support,
    folding_ranges as structure_folding_ranges, goto_definition as structure_goto_definition,
    hover_with_profile as structure_hover_with_profile, prepare_rename as structure_prepare_rename,
    references as structure_references,
    rename_with_workspace_edit_encoding as structure_rename_with_workspace_edit_encoding,
    selection_ranges as structure_selection_ranges,
};
use merman_analysis::{
    AnalysisCancellationToken, AnalysisOptions, AnalysisPayload, SourceKind,
    options_json::analysis_options_from_json_value, source_descriptor_for_kind,
    source_discarded_after_limit_change_diagnostic_with_span,
    source_limit_diagnostic_for_len_and_span,
};
#[cfg(test)]
use merman_analysis::{Analyzer, analyze_document_result_shared};
use merman_editor_core::{DocumentKind, TokenPlanError, analysis_payload_to_diagnostics};
use std::hash::{Hash, Hasher};
use std::sync::{Arc, OnceLock};
use tokio::sync::Mutex;
use tower_lsp_server::jsonrpc::Result;
use tower_lsp_server::ls_types::{
    CodeActionKind, CodeActionOptions, CodeActionParams, CodeActionProviderCapability,
    CodeActionResponse, CompletionItem, CompletionOptions, CompletionParams, CompletionResponse,
    Diagnostic, DiagnosticOptions, DiagnosticServerCapabilities, DiagnosticSeverity,
    DidChangeTextDocumentParams, DidCloseTextDocumentParams, DidOpenTextDocumentParams,
    DidSaveTextDocumentParams, DocumentDiagnosticParams, DocumentDiagnosticReport,
    DocumentDiagnosticReportResult, DocumentSymbolParams, DocumentSymbolResponse, FoldingRange,
    FoldingRangeParams, FoldingRangeProviderCapability, FullDocumentDiagnosticReport,
    GotoDefinitionParams, GotoDefinitionResponse, Hover, HoverParams, HoverProviderCapability,
    InitializeParams, InitializeResult, MessageType, NumberOrString, OneOf, Position,
    PrepareRenameResponse, Range, ReferenceParams, RelatedFullDocumentDiagnosticReport,
    RelatedUnchangedDocumentDiagnosticReport, RenameOptions, RenameParams, SelectionRange,
    SelectionRangeParams, SelectionRangeProviderCapability, SemanticTokensDeltaParams,
    SemanticTokensFullDeltaResult, SemanticTokensParams, SemanticTokensRangeParams,
    SemanticTokensRangeResult, SemanticTokensResult, SemanticTokensServerCapabilities,
    ServerCapabilities, TextDocumentPositionParams, TextDocumentSyncCapability,
    TextDocumentSyncKind, TextDocumentSyncOptions, TextDocumentSyncSaveOptions,
    UnchangedDocumentDiagnosticReport, WorkspaceEdit,
};
use tower_lsp_server::{Client, LanguageServer, LspService};

const MAX_DIAGNOSTIC_RECOMPUTE_ATTEMPTS: usize = 3;

struct AnalyzerReplacement {
    change: AnalyzerConfigurationChange,
    reprojection: Option<DiagnosticReprojectionPlan>,
}

#[derive(Clone)]
struct DiagnosticPublisher {
    client: Client,
    store: Arc<Mutex<DocumentStore>>,
    profile: ClientProtocolProfile,
}

#[derive(Debug, Clone)]
pub struct MermanLanguageServer {
    client: Client,
    store: Arc<Mutex<DocumentStore>>,
    client_profile: Arc<OnceLock<ClientProtocolProfile>>,
    refresh_coordinator: RefreshCoordinator,
    session_cancellation: AnalysisCancellationToken,
    client_effects: ClientEffectDispatcher,
    _lifetime: Arc<MermanLanguageServerLifetime>,
}

#[derive(Debug)]
struct MermanLanguageServerLifetime {
    session_cancellation: AnalysisCancellationToken,
    analysis_executor: crate::analysis_executor::AnalysisExecutor,
    client_effects: ClientEffectDispatcher,
}

impl MermanLanguageServer {
    fn new(client: Client, refresh_client: RefreshClient) -> Self {
        let session_cancellation = AnalysisCancellationToken::new();
        let store = DocumentStore::with_session_cancellation(session_cancellation.clone());
        let analysis_executor = store.analysis_executor();
        let client_effects = ClientEffectDispatcher::new();
        Self {
            client,
            store: Arc::new(Mutex::new(store)),
            client_profile: Arc::new(OnceLock::new()),
            refresh_coordinator: RefreshCoordinator::new(refresh_client),
            session_cancellation: session_cancellation.clone(),
            client_effects: client_effects.clone(),
            _lifetime: Arc::new(MermanLanguageServerLifetime {
                session_cancellation,
                analysis_executor,
                client_effects,
            }),
        }
    }

    /// Builds the service with a cancellation-safe, supervised client socket.
    pub fn service() -> (MermanLspService, MermanClientSocket) {
        let (service, socket, _) = Self::service_components();
        (service, socket)
    }

    fn service_components() -> (MermanLspService, MermanClientSocket, RefreshClient) {
        let (refresh_client, refresh_requests, refresh_responses) = RefreshClient::channel();
        let refresh_handle = refresh_client.clone();
        let backend = Arc::new(OnceLock::new());
        let backend_slot = Arc::clone(&backend);
        let (service, socket) = LspService::build(move |client| {
            let server = Self::new(client, refresh_client);
            backend_slot
                .set(server.clone())
                .expect("LSP backend is constructed exactly once");
            server
        })
        .custom_method(RULE_CATALOG_METHOD, Self::rule_catalog)
        .custom_method(CONFIG_SCHEMA_METHOD, Self::config_schema)
        .finish();
        let backend = backend
            .get()
            .expect("LSP service builder constructs its backend")
            .clone();
        (
            MermanLspService::new(service, backend),
            MermanClientSocket::new(socket, refresh_requests, refresh_responses),
            refresh_handle,
        )
    }

    #[cfg(test)]
    pub(crate) fn service_with_refresh_client()
    -> (MermanLspService, MermanClientSocket, RefreshClient) {
        Self::service_components()
    }

    #[cfg(test)]
    pub(crate) fn client_for_tests(&self) -> Client {
        self.client.clone()
    }

    /// Returns the server's full capability envelope without client-side negotiation.
    ///
    /// Live `initialize` responses are projected from the connecting client's capabilities.
    pub fn capabilities() -> ServerCapabilities {
        Self::capabilities_for_profile(&ClientProtocolProfile::permissive())
    }

    fn capabilities_for_profile(profile: &ClientProtocolProfile) -> ServerCapabilities {
        ServerCapabilities {
            text_document_sync: Some(TextDocumentSyncCapability::Options(
                TextDocumentSyncOptions {
                    open_close: Some(true),
                    change: Some(TextDocumentSyncKind::INCREMENTAL),
                    will_save: None,
                    will_save_wait_until: None,
                    save: Some(TextDocumentSyncSaveOptions::Supported(true)),
                },
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
            workspace_symbol_provider: None,
            diagnostic_provider: Some(DiagnosticServerCapabilities::Options(
                Self::diagnostic_options(),
            )),
            code_action_provider: profile.code_actions.as_ref().map(|_| {
                CodeActionProviderCapability::Options(CodeActionOptions {
                    code_action_kinds: Some(vec![CodeActionKind::QUICKFIX]),
                    work_done_progress_options: Default::default(),
                    resolve_provider: Some(false),
                })
            }),
            folding_range_provider: Some(FoldingRangeProviderCapability::Simple(true)),
            semantic_tokens_provider: semantic_tokens_options_with_profile(profile)
                .map(SemanticTokensServerCapabilities::SemanticTokensOptions),
            experimental: Some(experimental_capabilities()),
            ..ServerCapabilities::default()
        }
    }

    fn client_profile(&self) -> &ClientProtocolProfile {
        match self.client_profile.get() {
            Some(profile) => profile,
            None => ClientProtocolProfile::conservative_ref(),
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
        uri: &tower_lsp_server::ls_types::Uri,
    ) -> Option<Arc<DocumentSnapshot>> {
        snapshot_context::snapshot_context_for_uri(&self.store, uri, SnapshotContextKind::Structure)
            .await
            .ok()
            .flatten()
            .map(|context| context.snapshot)
    }

    async fn structure_snapshot_result<T>(
        &self,
        uri: &tower_lsp_server::ls_types::Uri,
        compute: impl FnOnce(&DocumentSnapshot) -> Result<Option<T>>,
    ) -> Result<Option<T>> {
        snapshot_context::snapshot_result(&self.store, uri, SnapshotContextKind::Structure, compute)
            .await
    }

    async fn semantic_snapshot_context_for_uri(
        &self,
        uri: &tower_lsp_server::ls_types::Uri,
    ) -> Result<Option<SnapshotContext>> {
        snapshot_context::snapshot_context_for_uri(
            &self.store,
            uri,
            SnapshotContextKind::SemanticTokens,
        )
        .await
    }

    #[cfg(test)]
    fn diagnostics_for_document(document: &StoredDocument, analyzer: &Analyzer) -> Vec<Diagnostic> {
        let profile = ClientProtocolProfile::permissive();
        if let Some(diagnostics) =
            Self::unavailable_document_diagnostics_with_profile(document, &profile)
        {
            return diagnostics;
        }

        let source = source_descriptor_for_document(&document.uri, document.kind);
        let analysis = analyze_document_result_shared(Arc::clone(&document.text), analyzer, source);
        Self::analysis_payload_diagnostics_with_profile(document, analysis.payload(), &profile)
    }

    fn unavailable_document_diagnostics_with_profile(
        document: &StoredDocument,
        profile: &ClientProtocolProfile,
    ) -> Option<Vec<Diagnostic>> {
        let diagnostic = if let Some(resource_limit) = document.resource_limit {
            source_limit_diagnostic_for_len_and_span(
                resource_limit.source_len,
                resource_limit.max_source_bytes,
                resource_limit.span,
            )
        } else if let Some(discarded_source) = document.discarded_source {
            source_discarded_after_limit_change_diagnostic_with_span(
                discarded_source.source_len,
                discarded_source.previous_max_source_bytes,
                discarded_source.span,
            )
        } else if let Some(sync_error) = document.sync_error {
            return Some(vec![document_sync_error_diagnostic(
                sync_error,
                document.version,
                profile,
            )]);
        } else {
            return None;
        };
        let payload = AnalysisPayload::new(
            source_descriptor_for_document(&document.uri, document.kind),
            vec![diagnostic],
        );

        Some(Self::analysis_payload_diagnostics_with_profile(
            document, &payload, profile,
        ))
    }

    fn analysis_payload_diagnostics_with_profile(
        document: &StoredDocument,
        payload: &AnalysisPayload,
        profile: &ClientProtocolProfile,
    ) -> Vec<Diagnostic> {
        analysis_payload_to_versioned_diagnostics_with_profile(
            payload,
            &document.uri,
            document.version,
            profile,
        )
    }

    fn diagnostic_result_id(diagnostics: &[tower_lsp_server::ls_types::Diagnostic]) -> String {
        let serialized = serde_json::to_vec(diagnostics).unwrap_or_default();
        let mut hasher = std::collections::hash_map::DefaultHasher::new();
        serialized.hash(&mut hasher);
        format!("{:016x}", hasher.finish())
    }

    fn document_diagnostic_report(
        diagnostics: Vec<tower_lsp_server::ls_types::Diagnostic>,
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
    ) -> Result<Option<Vec<tower_lsp_server::ls_types::Diagnostic>>> {
        self.diagnostic_publisher()
            .diagnostics_for_current_context(context)
            .await
    }

    async fn diagnostics_or_recompute_latest(
        &self,
        mut context: DiagnosticContext,
    ) -> Result<(
        DiagnosticContext,
        Vec<tower_lsp_server::ls_types::Diagnostic>,
    )> {
        for _ in 0..MAX_DIAGNOSTIC_RECOMPUTE_ATTEMPTS {
            if let Some(diagnostics) = self.diagnostics_for_current_context(&context).await? {
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

    fn diagnostic_publisher(&self) -> DiagnosticPublisher {
        DiagnosticPublisher {
            client: self.client.clone(),
            store: Arc::clone(&self.store),
            profile: self.client_profile().clone(),
        }
    }

    async fn enqueue_publish_for_uri(&self, uri: tower_lsp_server::ls_types::Uri) {
        let publisher = self.diagnostic_publisher();
        let key = ClientEffectKey::Document(uri.as_str().to_owned());
        self.client_effects
            .enqueue_latest(key, async move {
                publisher.publish_for_uri(uri).await;
            })
            .await;
    }

    async fn enqueue_clear_diagnostics(&self, uri: tower_lsp_server::ls_types::Uri) {
        let publisher = self.diagnostic_publisher();
        let key = ClientEffectKey::Document(uri.as_str().to_owned());
        self.client_effects
            .enqueue_latest(key, async move {
                publisher.clear(uri).await;
            })
            .await;
    }

    async fn enqueue_republish_all(&self) {
        let publisher = self.diagnostic_publisher();
        self.client_effects
            .enqueue_latest(ClientEffectKey::AllDiagnostics, async move {
                publisher.publish_all().await;
            })
            .await;
    }

    async fn record_semantic_tokens_state(
        &self,
        context: &SnapshotContext,
        tokens: Vec<tower_lsp_server::ls_types::SemanticToken>,
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

    async fn replace_analyzer(&self, options: AnalysisOptions) -> AnalyzerConfigurationChange {
        let replacement = self.prepare_analyzer_replacement(options).await;
        self.finish_analyzer_replacement(replacement).await
    }

    async fn prepare_analyzer_replacement(&self, options: AnalysisOptions) -> AnalyzerReplacement {
        let request = self
            .store
            .lock()
            .await
            .begin_analyzer_configuration_request();
        let (change, reprojection) = loop {
            let preparation = self
                .store
                .lock()
                .await
                .prepare_analyzer_options(request, options.clone());
            let Some(preparation) = preparation else {
                return AnalyzerReplacement {
                    change: AnalyzerConfigurationChange::Unchanged,
                    reprojection: None,
                };
            };
            match preparation {
                AnalyzerOptionsPreparation::Applied(change, reprojection) => {
                    break (change, reprojection);
                }
                AnalyzerOptionsPreparation::RequiresSourceLimitProjection(plan) => {
                    let batch = match tokio::task::spawn_blocking(move || plan.project()).await {
                        Ok(Ok(batch)) => batch,
                        Ok(Err(_cancelled)) => {
                            return AnalyzerReplacement {
                                change: AnalyzerConfigurationChange::Unchanged,
                                reprojection: None,
                            };
                        }
                        Err(error) => {
                            tracing::error!(%error, "source-limit projection worker failed");
                            return AnalyzerReplacement {
                                change: AnalyzerConfigurationChange::Unchanged,
                                reprojection: None,
                            };
                        }
                    };
                    let mut store = self.store.lock().await;
                    if let Some(applied) = store.commit_source_limit_reclassification(batch) {
                        break applied;
                    }
                    if !store.is_analyzer_configuration_request_current(request) {
                        return AnalyzerReplacement {
                            change: AnalyzerConfigurationChange::Unchanged,
                            reprojection: None,
                        };
                    }
                }
            }
        };
        AnalyzerReplacement {
            change,
            reprojection,
        }
    }

    async fn finish_analyzer_replacement(
        &self,
        replacement: AnalyzerReplacement,
    ) -> AnalyzerConfigurationChange {
        let AnalyzerReplacement {
            change,
            reprojection,
        } = replacement;
        let Some(reprojection) = reprojection else {
            return change;
        };

        match tokio::task::spawn_blocking(move || reprojection.project()).await {
            Ok(Ok(batch)) => {
                self.store
                    .lock()
                    .await
                    .commit_diagnostic_reprojection(batch);
            }
            Ok(Err(_cancelled)) => {}
            Err(error) => {
                tracing::error!(%error, "diagnostic reprojection worker failed");
                self.store.lock().await.discard_stale_analysis_generations();
            }
        }
        change
    }

    async fn apply_initialization_options(
        &self,
        initialization_options: Option<serde_json::Value>,
    ) -> tower_lsp_server::jsonrpc::Result<()> {
        match initialization_options {
            None => {
                self.replace_analyzer(default_lsp_analysis_options()).await;
                Ok(())
            }
            Some(value) => {
                let options = analysis_options_with_lsp_resource_defaults(
                    analysis_options_from_json_value(&value).map_err(|err| {
                        tower_lsp_server::jsonrpc::Error::invalid_params(err.to_string())
                    })?,
                );
                self.replace_analyzer(options).await;
                Ok(())
            }
        }
    }
}

impl DiagnosticPublisher {
    async fn diagnostics_for_current_context(
        &self,
        context: &DiagnosticContext,
    ) -> Result<Option<Vec<tower_lsp_server::ls_types::Diagnostic>>> {
        let (diagnostics, analysis_context) =
            match MermanLanguageServer::unavailable_document_diagnostics_with_profile(
                &context.document,
                &self.profile,
            ) {
                Some(diagnostics) => (diagnostics, None),
                None => {
                    let analysis_context = snapshot_context::snapshot_context_for_uri(
                        &self.store,
                        &context.document.uri,
                        SnapshotContextKind::Diagnostics,
                    )
                    .await?;
                    let Some(analysis_context) = analysis_context else {
                        return Ok(None);
                    };
                    let diagnostics =
                        MermanLanguageServer::analysis_payload_diagnostics_with_profile(
                            &context.document,
                            analysis_context.analysis_payload(),
                            &self.profile,
                        );
                    (diagnostics, Some(analysis_context))
                }
            };
        let store = self.store.lock().await;
        Ok((store.is_diagnostic_context_current(context)
            && analysis_context
                .as_ref()
                .is_none_or(|context| store.is_analysis_context_current(context)))
        .then_some(diagnostics))
    }

    async fn publish_for_uri(&self, uri: tower_lsp_server::ls_types::Uri) {
        if self.profile.diagnostic_pull {
            return;
        }
        let context = self.store.lock().await.diagnostic_context(&uri);
        if let Some(context) = context {
            self.publish_current(&context).await;
        }
    }

    async fn publish_all(&self) {
        if self.profile.diagnostic_pull {
            return;
        }
        let contexts = self.store.lock().await.diagnostic_contexts();
        for context in contexts {
            self.publish_current(&context).await;
        }
    }

    async fn publish_current(&self, context: &DiagnosticContext) {
        match self.diagnostics_for_current_context(context).await {
            Ok(Some(diagnostics)) => {
                self.client
                    .publish_diagnostics(
                        context.document.uri.clone(),
                        diagnostics,
                        self.profile
                            .diagnostics
                            .version
                            .then_some(context.document.version),
                    )
                    .await;
            }
            Ok(None) => {}
            Err(error) => {
                tracing::error!(
                    uri = %context.document.uri.as_str(),
                    code = ?error.code,
                    message = %error.message,
                    "failed to compute push diagnostics"
                );
            }
        }
    }

    async fn clear(&self, uri: tower_lsp_server::ls_types::Uri) {
        if !self.profile.diagnostic_pull {
            self.client.publish_diagnostics(uri, Vec::new(), None).await;
        }
    }
}

impl Drop for MermanLanguageServerLifetime {
    fn drop(&mut self) {
        self.session_cancellation.cancel();
        self.analysis_executor.invalidate_all();
        self.client_effects.cancel();
    }
}

impl LanguageServer for MermanLanguageServer {
    async fn initialize(&self, params: InitializeParams) -> Result<InitializeResult> {
        let profile = ClientProtocolProfile::negotiate(&params.capabilities);
        let capabilities = Self::capabilities_for_profile(&profile);
        self.apply_initialization_options(params.initialization_options)
            .await?;
        self.client_profile
            .set(profile)
            .map_err(|_| tower_lsp_server::jsonrpc::Error::invalid_request())?;
        Ok(InitializeResult {
            capabilities,
            ..InitializeResult::default()
        })
    }

    async fn initialized(&self, _: tower_lsp_server::ls_types::InitializedParams) {
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
        let uri = doc.uri;
        let version = doc.version;
        let cancellation = self.session_cancellation.child();
        let prepared = match tokio::task::spawn_blocking(move || {
            PreparedDocumentText::new_cancellable(doc.text, &cancellation)
        })
        .await
        {
            Ok(Ok(prepared)) => prepared,
            Ok(Err(_cancelled)) => return,
            Err(error) => {
                tracing::error!(%error, uri = %uri.as_str(), "source-limit projection worker failed");
                return;
            }
        };
        self.store
            .lock()
            .await
            .open_prepared_text(uri.clone(), version, prepared, kind);
        self.enqueue_publish_for_uri(uri).await;
        commit_active_mutation();
    }

    async fn did_change(&self, params: DidChangeTextDocumentParams) {
        let doc = params.text_document;
        let preparation = self.store.lock().await.capture_text_changes(
            doc.uri.clone(),
            doc.version,
            params.content_changes,
        );
        let update = match preparation {
            TextChangePreparation::Immediate(update) => update,
            TextChangePreparation::Prepare(plan) => {
                let prepared = match tokio::task::spawn_blocking(move || plan.prepare()).await {
                    Ok(Ok(prepared)) => prepared,
                    Ok(Err(_cancelled)) => return,
                    Err(error) => {
                        tracing::error!(
                            %error,
                            uri = %doc.uri.as_str(),
                            "document change projection worker failed"
                        );
                        commit_active_mutation();
                        return;
                    }
                };
                self.store
                    .lock()
                    .await
                    .commit_prepared_text_changes(prepared)
            }
        };
        if update.affects_document_state() {
            self.enqueue_publish_for_uri(doc.uri).await;
        }
        commit_active_mutation();
    }

    async fn did_save(&self, params: DidSaveTextDocumentParams) {
        let uri = params.text_document.uri;
        self.enqueue_publish_for_uri(uri).await;
        commit_active_mutation();
    }

    async fn did_close(&self, params: DidCloseTextDocumentParams) {
        let uri = params.text_document.uri;
        self.store.lock().await.remove(&uri);
        self.enqueue_clear_diagnostics(uri).await;
        commit_active_mutation();
    }

    async fn did_change_configuration(
        &self,
        params: tower_lsp_server::ls_types::DidChangeConfigurationParams,
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

        let replacement = self.prepare_analyzer_replacement(options).await;
        commit_active_mutation();
        let change = self.finish_analyzer_replacement(replacement).await;
        if change.affects_diagnostics() {
            self.enqueue_republish_all().await;
        }
        let profile = self.client_profile();
        self.refresh_coordinator.request(
            change.affects_snapshots()
                && profile.semantic_tokens.is_some()
                && profile.semantic_tokens_refresh,
            change.affects_diagnostics() && profile.diagnostic_pull && profile.diagnostic_refresh,
        );
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
        let profile = self.client_profile();

        self.structure_snapshot_result(&uri, |snapshot| {
            Ok(Some(CompletionResponse::List(
                completion_for_snapshot_with_profile(snapshot, position, profile),
            )))
        })
        .await
    }

    async fn completion_resolve(&self, item: CompletionItem) -> Result<CompletionItem> {
        Ok(resolve_completion_item_with_profile(
            item,
            self.client_profile(),
        ))
    }

    async fn code_action(&self, params: CodeActionParams) -> Result<Option<CodeActionResponse>> {
        let profile = self.client_profile();
        let uri = params.text_document.uri.clone();
        let Some(snapshot_context) = snapshot_context::snapshot_context_for_uri(
            &self.store,
            &uri,
            SnapshotContextKind::CodeActions,
        )
        .await?
        else {
            return Ok(None);
        };
        let diagnostics = analysis_payload_to_diagnostics(snapshot_context.analysis_payload());
        let actions = code_actions_for_snapshot_diagnostics_with_profile(
            &snapshot_context.snapshot,
            &diagnostics,
            &params,
            profile,
        );
        snapshot_context::ensure_snapshot_current(
            &self.store,
            &snapshot_context,
            SnapshotContextKind::CodeActions,
        )
        .await?;
        Ok(actions)
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
        let profile = self.client_profile();

        let Some(mut tokens) = semantic_tokens_for_snapshot_with_profile(snapshot, profile)
            .map_err(|error| semantic_token_planning_error(snapshot, error))?
        else {
            return Ok(None);
        };
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
        let profile = self.client_profile();

        let Some(current_tokens) = semantic_tokens_for_snapshot_with_profile(snapshot, profile)
            .map_err(|error| semantic_token_planning_error(snapshot, error))?
        else {
            return Ok(None);
        };
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
        let profile = self.client_profile();
        let Some(result) = semantic_tokens_for_snapshot_range_with_profile(
            &snapshot_context.snapshot,
            params.range,
            profile,
        )
        .map_err(|error| semantic_token_planning_error(&snapshot_context.snapshot, error))?
        else {
            return Ok(None);
        };
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
        let profile = self.client_profile();

        self.structure_snapshot_result(&uri, |snapshot| {
            Ok(structure_hover_with_profile(snapshot, position, profile))
        })
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
        let hierarchical_supported = self.client_profile().hierarchical_document_symbols;

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
    ) -> Result<Option<Vec<tower_lsp_server::ls_types::Location>>> {
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
        let workspace_edit_encoding = self.client_profile().workspace_edit_encoding;

        self.structure_snapshot_result(&uri, |snapshot| {
            structure_rename_with_workspace_edit_encoding(snapshot, params, workspace_edit_encoding)
        })
        .await
    }
}

fn stale_diagnostic_recompute_error() -> tower_lsp_server::jsonrpc::Error {
    let mut error = tower_lsp_server::jsonrpc::Error::content_modified();
    error.message = "diagnostic document changed while recomputing".into();
    error
}

fn semantic_token_planning_error(
    snapshot: &DocumentSnapshot,
    error: TokenPlanError,
) -> tower_lsp_server::jsonrpc::Error {
    if error.is_invalid_range() {
        return tower_lsp_server::jsonrpc::Error::invalid_params(error.to_string());
    }

    tracing::error!(
        uri = snapshot.uri().as_str(),
        version = snapshot.version(),
        %error,
        "semantic token planning failed"
    );
    let mut response = tower_lsp_server::jsonrpc::Error::internal_error();
    response.message = "semantic token planning failed".into();
    response.data = Some(serde_json::json!({
        "code": "merman.lsp.semantic_token_planning_failed",
        "detail": error.to_string(),
    }));
    response
}

fn document_sync_error_diagnostic(
    sync_error: DocumentSyncError,
    document_version: i32,
    profile: &ClientProtocolProfile,
) -> Diagnostic {
    let message = match sync_error {
        DocumentSyncError::InvalidIncrementalRange => {
            "document text is out of sync after an invalid incremental edit range; send a full document replacement or reopen the document".to_string()
        }
        DocumentSyncError::FullReplacementRequired {
            source_len,
            last_max_source_bytes,
        } => format!(
            "document text is unavailable after rejecting a {source_len}-byte source with a {last_max_source_bytes}-byte limit; ranged edits cannot recover discarded text, so send a full document replacement or reopen the document"
        ),
    };
    Diagnostic {
        range: Range::new(Position::new(0, 0), Position::new(0, 0)),
        severity: Some(DiagnosticSeverity::ERROR),
        code: Some(NumberOrString::String(
            "merman.lsp.document_sync_lost".to_string(),
        )),
        source: Some("merman".to_string()),
        message,
        related_information: None,
        tags: None,
        code_description: None,
        data: profile
            .diagnostics
            .data
            .then(|| serde_json::to_value(DiagnosticVersionData { document_version }).ok())
            .flatten(),
    }
}

fn source_descriptor_for_document(
    uri: &tower_lsp_server::ls_types::Uri,
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
    uri: &tower_lsp_server::ls_types::Uri,
) -> DocumentKind {
    match language_id {
        "markdown" => DocumentKind::Markdown,
        "mdx" => DocumentKind::Mdx,
        "mermaid" => DocumentKind::Diagram,
        _ => DocumentKind::from_path(uri.path().as_str()),
    }
}

#[cfg(test)]
mod tests;
