use crate::client_profile::ClientProtocolProfile;
use crate::code_actions::code_actions_for_snapshot_diagnostics_with_profile;
use crate::completion::{
    completion_for_snapshot_with_profile, resolve_completion_item_with_profile,
};
use crate::diagnostics::analysis_payload_to_versioned_diagnostics_with_profile;
use crate::protocol::{
    CONFIG_SCHEMA_METHOD, ConfigSchemaResponse, DiagnosticVersionData, RULE_CATALOG_METHOD,
    RuleCatalogResponse, experimental_capabilities,
};
use crate::refresh_transport::{MermanClientSocket, RefreshClient};
use crate::semantic_tokens::{
    semantic_tokens_delta_result, semantic_tokens_for_snapshot_range_with_profile,
    semantic_tokens_for_snapshot_with_profile, semantic_tokens_options_with_profile,
    semantic_tokens_result_id,
};
use crate::session::{ClientEffectKey, LanguageSession, MermanLspService, commit_active_mutation};
use crate::session::{
    DiagnosticContext, DocumentDiagnosticState, DocumentSyncError, SemanticTokensState,
    StoredDocument, analysis_options_with_lsp_resource_defaults, default_lsp_analysis_options,
};
use crate::snapshot::DocumentSnapshot;
use crate::structure::{
    document_symbols_with_hierarchy_support as structure_document_symbols_with_hierarchy_support,
    folding_ranges as structure_folding_ranges, goto_definition as structure_goto_definition,
    hover_with_profile as structure_hover_with_profile, prepare_rename as structure_prepare_rename,
    references as structure_references,
    rename_with_workspace_edit_encoding as structure_rename_with_workspace_edit_encoding,
    selection_ranges as structure_selection_ranges,
};
use merman_analysis::{
    AnalysisPayload, SourceKind, options_json::analysis_options_from_json_value,
    source_descriptor_for_kind, source_discarded_after_limit_change_diagnostic_with_span,
    source_limit_diagnostic_for_len_and_span,
};
#[cfg(test)]
use merman_analysis::{Analyzer, analyze_document};
use merman_editor_core::{DocumentKind, TokenPlanError, analysis_payload_to_diagnostics};
use std::hash::{Hash, Hasher};
use std::sync::{Arc, OnceLock};
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

#[derive(Clone)]
struct DiagnosticPublisher {
    client: Client,
    session: LanguageSession,
    profile: ClientProtocolProfile,
}

#[derive(Debug, Clone)]
pub struct MermanLanguageServer {
    client: Client,
    session: LanguageSession,
    client_profile: Arc<OnceLock<ClientProtocolProfile>>,
}

impl MermanLanguageServer {
    fn new(client: Client, session: LanguageSession) -> Self {
        Self {
            client,
            session,
            client_profile: Arc::new(OnceLock::new()),
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
        let session = LanguageSession::with_refresh_client(refresh_client);
        let backend_session = session.clone();
        #[cfg(test)]
        let backend = Arc::new(OnceLock::new());
        #[cfg(test)]
        let backend_slot = Arc::clone(&backend);
        let (service, socket) = LspService::build(move |client| {
            let server = Self::new(client, backend_session.clone());
            #[cfg(test)]
            backend_slot
                .set(server.clone())
                .expect("LSP backend is constructed exactly once");
            server
        })
        .custom_method(RULE_CATALOG_METHOD, Self::rule_catalog)
        .custom_method(CONFIG_SCHEMA_METHOD, Self::config_schema)
        .finish();
        #[cfg(test)]
        let backend = backend
            .get()
            .expect("LSP service builder constructs its backend")
            .clone();
        #[cfg(test)]
        let service = MermanLspService::with_backend_for_tests(service, session.clone(), backend);
        #[cfg(not(test))]
        let service = MermanLspService::new(service, session.clone());
        let socket = MermanClientSocket::new(
            socket,
            refresh_requests,
            refresh_responses,
            session.endpoint_guard(),
        );
        (service, socket, refresh_handle)
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

    async fn rule_catalog(&self) -> Result<RuleCatalogResponse> {
        Ok(RuleCatalogResponse::current())
    }

    async fn config_schema(&self) -> Result<ConfigSchemaResponse> {
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
        self.session.structure_snapshot(uri).await
    }

    async fn structure_snapshot_result<T>(
        &self,
        uri: &tower_lsp_server::ls_types::Uri,
        compute: impl FnOnce(&DocumentSnapshot) -> Result<Option<T>>,
    ) -> Result<Option<T>> {
        self.session.query_structure(uri, compute).await
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
        let payload = analyze_document(
            document
                .text()
                .expect("available document diagnostics require retained source"),
            analyzer,
            source,
        );
        Self::analysis_payload_diagnostics_with_profile(document, &payload, &profile)
    }

    fn unavailable_document_diagnostics_with_profile(
        document: &StoredDocument,
        profile: &ClientProtocolProfile,
    ) -> Option<Vec<Diagnostic>> {
        let diagnostic = if let Some(resource_limit) = document.resource_limit() {
            source_limit_diagnostic_for_len_and_span(
                resource_limit.source_len,
                resource_limit.max_source_bytes,
                resource_limit.span,
            )
        } else if let Some(discarded_source) = document.discarded_source() {
            source_discarded_after_limit_change_diagnostic_with_span(
                discarded_source.source_len,
                discarded_source.previous_max_source_bytes,
                discarded_source.span,
            )
        } else if let Some(sync_error) = document.sync_error_state() {
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

    fn diagnostic_publisher(&self) -> Option<DiagnosticPublisher> {
        let profile = self.client_profile();
        if profile.diagnostic_pull {
            return None;
        }
        Some(DiagnosticPublisher {
            client: self.client.clone(),
            session: self.session.clone(),
            profile: profile.clone(),
        })
    }

    async fn enqueue_publish_for_uri(&self, uri: tower_lsp_server::ls_types::Uri) {
        let Some(publisher) = self.diagnostic_publisher() else {
            return;
        };
        let key = ClientEffectKey::Document(uri.as_str().to_owned());
        self.session
            .enqueue_latest_client_effect(key, async move {
                publisher.publish_for_uri(uri).await;
            })
            .await;
    }

    async fn enqueue_clear_diagnostics(&self, uri: tower_lsp_server::ls_types::Uri) {
        let Some(publisher) = self.diagnostic_publisher() else {
            return;
        };
        let key = ClientEffectKey::Document(uri.as_str().to_owned());
        self.session
            .enqueue_latest_client_effect(key, async move {
                publisher.clear(uri).await;
            })
            .await;
    }

    async fn enqueue_republish_all(&self) {
        let Some(publisher) = self.diagnostic_publisher() else {
            return;
        };
        self.session
            .enqueue_latest_client_effect(ClientEffectKey::AllDiagnostics, async move {
                publisher.publish_all().await;
            })
            .await;
    }

    async fn enqueue_log_message(&self, kind: MessageType, message: impl Into<String>) {
        let client = self.client.clone();
        let message = message.into();
        self.session
            .enqueue_client_effect(async move {
                client.log_message(kind, message).await;
            })
            .await;
    }

    async fn apply_initialization_options(
        &self,
        initialization_options: Option<serde_json::Value>,
    ) -> tower_lsp_server::jsonrpc::Result<()> {
        let options = match initialization_options {
            None => default_lsp_analysis_options(),
            Some(value) => analysis_options_with_lsp_resource_defaults(
                analysis_options_from_json_value(&value).map_err(|err| {
                    tower_lsp_server::jsonrpc::Error::invalid_params(err.to_string())
                })?,
            ),
        };
        if self.session.update_configuration(options).await.accepted() {
            Ok(())
        } else {
            let mut error = tower_lsp_server::jsonrpc::Error::internal_error();
            error.message = "analysis configuration did not commit".into();
            Err(error)
        }
    }
}

impl DiagnosticPublisher {
    async fn diagnostics_for_current_context(
        &self,
        context: &DiagnosticContext,
    ) -> Result<Option<Vec<tower_lsp_server::ls_types::Diagnostic>>> {
        self.session
            .query_push_diagnostics(context, |document, payload| {
                MermanLanguageServer::unavailable_document_diagnostics_with_profile(
                    document,
                    &self.profile,
                )
                .unwrap_or_else(|| {
                    MermanLanguageServer::analysis_payload_diagnostics_with_profile(
                        document,
                        payload.expect("available documents require an analysis payload"),
                        &self.profile,
                    )
                })
            })
            .await
    }

    async fn publish_for_uri(&self, uri: tower_lsp_server::ls_types::Uri) {
        let context = self.session.diagnostic_context(&uri).await;
        if let Some(context) = context {
            self.publish_current(&context).await;
        }
    }

    async fn publish_all(&self) {
        let contexts = self.session.diagnostic_contexts().await;
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
        self.client.publish_diagnostics(uri, Vec::new(), None).await;
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
        commit_active_mutation();
        self.enqueue_log_message(MessageType::INFO, "merman-lsp initialized")
            .await;
    }

    async fn shutdown(&self) -> Result<()> {
        Ok(())
    }

    async fn did_open(&self, params: DidOpenTextDocumentParams) {
        let doc = params.text_document;
        let kind = document_kind_for_language_id(&doc.language_id, &doc.uri);
        let uri = doc.uri;
        let committed = self
            .session
            .open_document(uri.clone(), doc.version, doc.text, kind)
            .await;
        commit_active_mutation();
        if committed {
            self.enqueue_publish_for_uri(uri).await;
        }
    }

    async fn did_change(&self, params: DidChangeTextDocumentParams) {
        let doc = params.text_document;
        let update = self
            .session
            .change_document(doc.uri.clone(), doc.version, params.content_changes)
            .await;
        commit_active_mutation();
        if update.is_some_and(|update| update.affects_document_state()) {
            self.enqueue_publish_for_uri(doc.uri).await;
        }
    }

    async fn did_save(&self, params: DidSaveTextDocumentParams) {
        let uri = params.text_document.uri;
        commit_active_mutation();
        self.enqueue_publish_for_uri(uri).await;
    }

    async fn did_close(&self, params: DidCloseTextDocumentParams) {
        let uri = params.text_document.uri;
        self.session.close_document(&uri).await;
        commit_active_mutation();
        self.enqueue_clear_diagnostics(uri).await;
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
                    commit_active_mutation();
                    self.enqueue_log_message(
                        MessageType::ERROR,
                        format!("invalid merman analysis settings: {err}"),
                    )
                    .await;
                    return;
                }
            }
        };

        let change = self.session.update_configuration(options).await;
        commit_active_mutation();
        if change.failed() {
            self.enqueue_log_message(
                MessageType::ERROR,
                "failed to apply merman analysis settings",
            )
            .await;
            return;
        }
        if change.affects_diagnostics() {
            self.enqueue_republish_all().await;
        }
        let profile = self.client_profile();
        self.session.request_refresh(
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
        let profile = self.client_profile();
        let state = self
            .session
            .pull_diagnostics(&uri, |document, payload| {
                let diagnostics =
                    Self::unavailable_document_diagnostics_with_profile(document, profile)
                        .unwrap_or_else(|| {
                            Self::analysis_payload_diagnostics_with_profile(
                                document,
                                payload.expect("available documents require an analysis payload"),
                                profile,
                            )
                        });
                DocumentDiagnosticState {
                    result_id: Self::diagnostic_result_id(&diagnostics),
                    diagnostics,
                }
            })
            .await?;
        let Some(state) = state else {
            let diagnostics = Vec::new();
            let result_id = Some(Self::diagnostic_result_id(&diagnostics));
            return Ok(Self::document_diagnostic_report(
                diagnostics,
                result_id,
                previous_result_id,
            ));
        };

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
        self.session
            .query_code_actions(&uri, |context| {
                let diagnostics = analysis_payload_to_diagnostics(context.analysis_payload());
                Ok(code_actions_for_snapshot_diagnostics_with_profile(
                    &context.snapshot,
                    &diagnostics,
                    &params,
                    profile,
                ))
            })
            .await
    }

    async fn semantic_tokens_full(
        &self,
        params: SemanticTokensParams,
    ) -> Result<Option<SemanticTokensResult>> {
        let uri = params.text_document.uri;
        let profile = self.client_profile();
        self.session
            .query_semantic_tokens(&uri, None, |snapshot, _| {
                let Some(mut tokens) = semantic_tokens_for_snapshot_with_profile(snapshot, profile)
                    .map_err(|error| semantic_token_planning_error(snapshot, error))?
                else {
                    return Ok(None);
                };
                let result_id = semantic_tokens_result_id(snapshot, &tokens.data);
                tokens.result_id = Some(result_id.clone());
                let state = SemanticTokensState::new(tokens.result_id.clone(), tokens.data.clone());
                Ok(Some((SemanticTokensResult::Tokens(tokens), Some(state))))
            })
            .await
    }

    async fn semantic_tokens_full_delta(
        &self,
        params: SemanticTokensDeltaParams,
    ) -> Result<Option<SemanticTokensFullDeltaResult>> {
        let uri = params.text_document.uri;
        let profile = self.client_profile();
        self.session
            .query_semantic_tokens(
                &uri,
                Some(params.previous_result_id.as_str()),
                |snapshot, previous| {
                    let Some(current_tokens) =
                        semantic_tokens_for_snapshot_with_profile(snapshot, profile)
                            .map_err(|error| semantic_token_planning_error(snapshot, error))?
                    else {
                        return Ok(None);
                    };
                    let current_result_id =
                        semantic_tokens_result_id(snapshot, &current_tokens.data);
                    let delta = match previous {
                        Some(previous) => semantic_tokens_delta_result(
                            &previous.tokens,
                            &current_tokens.data,
                            current_result_id.clone(),
                        ),
                        None => {
                            let mut tokens = current_tokens.clone();
                            tokens.result_id = Some(current_result_id.clone());
                            SemanticTokensFullDeltaResult::Tokens(tokens)
                        }
                    };
                    let state =
                        SemanticTokensState::new(Some(current_result_id), current_tokens.data);
                    Ok(Some((delta, Some(state))))
                },
            )
            .await
    }

    async fn semantic_tokens_range(
        &self,
        params: SemanticTokensRangeParams,
    ) -> Result<Option<SemanticTokensRangeResult>> {
        let uri = params.text_document.uri;
        let profile = self.client_profile();
        self.session
            .query_semantic_tokens(&uri, None, |snapshot, _| {
                let Some(result) = semantic_tokens_for_snapshot_range_with_profile(
                    snapshot,
                    params.range,
                    profile,
                )
                .map_err(|error| semantic_token_planning_error(snapshot, error))?
                else {
                    return Ok(None);
                };
                Ok(Some((SemanticTokensRangeResult::from(result), None)))
            })
            .await
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
