use crate::analysis_executor::AnalysisExecutor;
#[cfg(test)]
use crate::analysis_request::SnapshotBatchCommit;
use crate::analysis_request::{AnalysisBuildKey, AnalysisBuildRequest};
#[cfg(test)]
use crate::snapshot::DocumentSnapshot;
use crate::snapshot::{
    DiagnosticGeneration, DocumentAnalysisContext, DocumentEpoch, SnapshotContext,
    SnapshotGeneration,
};
#[cfg(test)]
use merman_analysis::source_limit_diagnostic_span;
use merman_analysis::{
    AnalysisCancellationToken, AnalysisCancelled, AnalysisOptions, Analyzer, DiagnosticSpan,
    source_limit_diagnostic_span_cancellable,
};
use merman_editor_core::DocumentKind;
use ropey::{Rope, RopeSlice};
use std::collections::HashMap;
use std::sync::Arc;
use tower_lsp_server::ls_types::{
    Diagnostic, Position, Range, SemanticToken, TextDocumentContentChangeEvent, Uri,
};

pub(crate) const DEFAULT_LSP_MAX_SOURCE_BYTES: usize = 4 * 1024 * 1024;

pub(crate) fn default_lsp_analysis_options() -> AnalysisOptions {
    AnalysisOptions::default().with_max_source_bytes(Some(DEFAULT_LSP_MAX_SOURCE_BYTES))
}

pub(crate) fn analysis_options_with_lsp_resource_defaults(
    options: AnalysisOptions,
) -> AnalysisOptions {
    if options.max_source_bytes().is_none() {
        options.with_max_source_bytes(Some(DEFAULT_LSP_MAX_SOURCE_BYTES))
    } else {
        options
    }
}

#[derive(Debug)]
pub struct DocumentStore {
    analyzer: Analyzer,
    analysis_executor: AnalysisExecutor,
    session_cancellation: AnalysisCancellationToken,
    diagnostic_reprojection_cancellation: AnalysisCancellationToken,
    snapshot_generation: SnapshotGeneration,
    diagnostic_generation: DiagnosticGeneration,
    analyzer_configuration_request: AnalyzerConfigurationRequest,
    next_document_epoch: u64,
    documents: HashMap<Uri, DocumentRecord>,
    analysis_generations: HashMap<Uri, CachedAnalysisGeneration>,
    diagnostic_state: HashMap<Uri, StoredDiagnosticState>,
    semantic_tokens_state: HashMap<Uri, StoredSemanticTokensState>,
}

#[derive(Debug)]
struct CachedAnalysisGeneration {
    context: Arc<DocumentAnalysisContext>,
    diagnostic_generation: DiagnosticGeneration,
}

#[derive(Debug)]
pub(crate) struct DiagnosticReprojectionPlan {
    analyzer: Analyzer,
    cancellation: AnalysisCancellationToken,
    generation: DiagnosticGeneration,
    sources: Vec<DiagnosticReprojectionSource>,
}

#[derive(Debug)]
pub(crate) struct DiagnosticReprojectionRequest {
    analyzer: Analyzer,
    cancellation: AnalysisCancellationToken,
    generation: DiagnosticGeneration,
    source: DiagnosticReprojectionSource,
}

#[derive(Debug)]
struct DiagnosticReprojectionSource {
    uri: Uri,
    document_epoch: DocumentEpoch,
    context: Arc<DocumentAnalysisContext>,
}

#[derive(Debug)]
pub(crate) struct DiagnosticReprojectionBatch {
    generation: DiagnosticGeneration,
    projections: Vec<DiagnosticReprojection>,
}

#[derive(Debug)]
pub(crate) struct SourceLimitReclassificationPlan {
    cancellation: AnalysisCancellationToken,
    request: AnalyzerConfigurationRequest,
    expected_options: AnalysisOptions,
    next_options: AnalysisOptions,
    documents: Vec<SourceLimitDocumentSnapshot>,
}

#[derive(Debug)]
struct SourceLimitDocumentSnapshot {
    uri: Uri,
    document_epoch: DocumentEpoch,
    oversized_source: Option<Arc<str>>,
}

#[derive(Debug)]
pub(crate) struct SourceLimitReclassificationBatch {
    request: AnalyzerConfigurationRequest,
    expected_options: AnalysisOptions,
    next_options: AnalysisOptions,
    documents: Vec<SourceLimitDocumentProjection>,
}

#[derive(Debug)]
struct SourceLimitDocumentProjection {
    uri: Uri,
    document_epoch: DocumentEpoch,
    oversized_span: Option<DiagnosticSpan>,
}

#[derive(Debug)]
struct DiagnosticReprojection {
    uri: Uri,
    document_epoch: DocumentEpoch,
    original: Arc<DocumentAnalysisContext>,
    projected: Arc<DocumentAnalysisContext>,
}

#[derive(Debug)]
pub(crate) enum AnalyzerOptionsPreparation {
    Applied(
        AnalyzerConfigurationChange,
        Option<DiagnosticReprojectionPlan>,
    ),
    RequiresSourceLimitProjection(SourceLimitReclassificationPlan),
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub(crate) struct AnalyzerConfigurationRequest(u64);

impl Default for DocumentStore {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug, Clone)]
pub struct StoredDocument {
    pub uri: Uri,
    pub version: i32,
    pub text: Arc<str>,
    pub kind: DocumentKind,
    pub resource_limit: Option<DocumentResourceLimit>,
    pub discarded_source: Option<DocumentDiscardedSource>,
    pub sync_error: Option<DocumentSyncError>,
}

#[derive(Debug)]
pub(crate) struct PreparedDocumentText {
    text: String,
    span: DiagnosticSpan,
}

impl PreparedDocumentText {
    #[cfg(test)]
    pub(crate) fn new(text: String) -> Self {
        let span = source_limit_diagnostic_span(&text);
        Self { text, span }
    }

    pub(crate) fn new_cancellable(
        text: String,
        cancellation: &AnalysisCancellationToken,
    ) -> Result<Self, AnalysisCancelled> {
        let span = source_limit_diagnostic_span_cancellable(&text, cancellation)?;
        Ok(Self { text, span })
    }
}

impl TextChangePlan {
    pub(crate) fn prepare(self) -> Result<PreparedTextChanges, AnalysisCancelled> {
        let Self {
            uri,
            version,
            kind,
            expected_epoch,
            expected_configuration,
            current_text,
            unavailable_source,
            changes,
            cancellation,
        } = self;
        cancellation.checkpoint()?;
        let mutation = match (current_text.as_deref(), unavailable_source) {
            (Some(current_text), None) => {
                cancellation.checkpoint()?;
                let mut text = Rope::from_str(current_text);
                cancellation.checkpoint()?;
                let changes = changes_from_last_full_replacement(changes);
                match apply_text_content_changes(&mut text, changes, &cancellation)? {
                    Ok(()) => {
                        let text = text.to_string();
                        PreparedTextMutation::Text(PreparedDocumentText::new_cancellable(
                            text,
                            &cancellation,
                        )?)
                    }
                    Err(()) => invalid_range_mutation(),
                }
            }
            (None, Some(unavailable_source)) => {
                let Some(recovery_start) =
                    changes.iter().rposition(|change| change.range.is_none())
                else {
                    return Ok(PreparedTextChanges {
                        uri,
                        version,
                        kind,
                        expected_epoch,
                        expected_configuration,
                        mutation: full_sync_mutation(unavailable_source),
                    });
                };
                let mut changes = changes.into_iter().skip(recovery_start);
                let replacement = changes
                    .next()
                    .expect("full-replacement position must name an existing change");
                debug_assert!(replacement.range.is_none());
                cancellation.checkpoint()?;
                let mut text = Rope::from_str(&replacement.text);
                cancellation.checkpoint()?;
                match apply_text_content_changes(&mut text, changes, &cancellation)? {
                    Ok(()) => {
                        let text = text.to_string();
                        PreparedTextMutation::Text(PreparedDocumentText::new_cancellable(
                            text,
                            &cancellation,
                        )?)
                    }
                    Err(()) => invalid_range_mutation(),
                }
            }
            _ => unreachable!("captured text availability must match its resource state"),
        };
        cancellation.checkpoint()?;
        Ok(PreparedTextChanges {
            uri,
            version,
            kind,
            expected_epoch,
            expected_configuration,
            mutation,
        })
    }
}

fn apply_text_content_changes(
    text: &mut Rope,
    changes: impl IntoIterator<Item = TextDocumentContentChangeEvent>,
    cancellation: &AnalysisCancellationToken,
) -> Result<Result<(), ()>, AnalysisCancelled> {
    for change in changes {
        cancellation.checkpoint()?;
        if !apply_text_content_change(text, change) {
            return Ok(Err(()));
        }
    }
    cancellation.checkpoint()?;
    Ok(Ok(()))
}

fn invalid_range_mutation() -> PreparedTextMutation {
    PreparedTextMutation::SyncError {
        error: DocumentSyncError::InvalidIncrementalRange,
        update: TextDocumentUpdate::InvalidRange,
    }
}

fn full_sync_mutation(unavailable_source: UnavailableSourceState) -> PreparedTextMutation {
    let error = match unavailable_source {
        UnavailableSourceState::ResourceLimited(resource_limit) => {
            DocumentSyncError::FullReplacementRequired {
                source_len: resource_limit.source_len,
                last_max_source_bytes: resource_limit.max_source_bytes,
            }
        }
        UnavailableSourceState::Discarded(discarded_source) => {
            DocumentSyncError::FullReplacementRequired {
                source_len: discarded_source.source_len,
                last_max_source_bytes: discarded_source.previous_max_source_bytes,
            }
        }
        UnavailableSourceState::SyncError(sync_error) => sync_error,
    };
    PreparedTextMutation::SyncError {
        error,
        update: TextDocumentUpdate::NeedsFullSync,
    }
}

#[derive(Debug)]
pub(crate) enum TextChangePreparation {
    Immediate(TextDocumentUpdate),
    Prepare(Box<TextChangePlan>),
}

#[derive(Debug)]
pub(crate) struct TextChangePlan {
    uri: Uri,
    version: i32,
    kind: DocumentKind,
    expected_epoch: DocumentEpoch,
    expected_configuration: AnalyzerConfigurationRequest,
    current_text: Option<Arc<str>>,
    unavailable_source: Option<UnavailableSourceState>,
    changes: Vec<TextDocumentContentChangeEvent>,
    cancellation: AnalysisCancellationToken,
}

#[derive(Debug)]
pub(crate) struct PreparedTextChanges {
    uri: Uri,
    version: i32,
    kind: DocumentKind,
    expected_epoch: DocumentEpoch,
    expected_configuration: AnalyzerConfigurationRequest,
    mutation: PreparedTextMutation,
}

#[derive(Debug)]
enum PreparedTextMutation {
    Text(PreparedDocumentText),
    SyncError {
        error: DocumentSyncError,
        update: TextDocumentUpdate,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DocumentResourceLimit {
    pub source_len: usize,
    pub max_source_bytes: usize,
    pub span: DiagnosticSpan,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DocumentDiscardedSource {
    pub source_len: usize,
    pub previous_max_source_bytes: usize,
    pub span: DiagnosticSpan,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DocumentSyncError {
    InvalidIncrementalRange,
    FullReplacementRequired {
        source_len: usize,
        last_max_source_bytes: usize,
    },
}

#[derive(Debug, Clone, Copy)]
enum UnavailableSourceState {
    ResourceLimited(DocumentResourceLimit),
    Discarded(DocumentDiscardedSource),
    SyncError(DocumentSyncError),
}

impl StoredDocument {
    pub fn has_unavailable_source(&self) -> bool {
        self.unavailable_source_state().is_some()
    }

    fn unavailable_source_state(&self) -> Option<UnavailableSourceState> {
        self.resource_limit
            .map(UnavailableSourceState::ResourceLimited)
            .or_else(|| self.discarded_source.map(UnavailableSourceState::Discarded))
            .or_else(|| self.sync_error.map(UnavailableSourceState::SyncError))
    }
}

fn resource_state_source_len_and_previous_limit(
    document: &StoredDocument,
) -> Option<(usize, usize, DiagnosticSpan)> {
    if let Some(resource_limit) = document.resource_limit {
        return Some((
            resource_limit.source_len,
            resource_limit.max_source_bytes,
            resource_limit.span,
        ));
    }
    document.discarded_source.map(|discarded| {
        (
            discarded.source_len,
            discarded.previous_max_source_bytes,
            discarded.span,
        )
    })
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TextDocumentUpdate {
    Applied,
    NeedsFullSync,
    MissingDocument,
    EmptyChangeSet,
    InvalidRange,
    StaleVersion {
        current_version: i32,
        attempted_version: i32,
    },
    Superseded,
}

impl TextDocumentUpdate {
    pub fn affects_document_state(self) -> bool {
        matches!(
            self,
            Self::Applied | Self::NeedsFullSync | Self::InvalidRange
        )
    }
}

#[derive(Debug, Clone, Default)]
pub struct SemanticTokensState {
    pub result_id: Option<String>,
    pub tokens: Vec<SemanticToken>,
}

impl SemanticTokensState {
    pub fn new(result_id: Option<String>, tokens: Vec<SemanticToken>) -> Self {
        Self { result_id, tokens }
    }
}

impl DiagnosticReprojectionPlan {
    pub(crate) fn project(self) -> Result<DiagnosticReprojectionBatch, AnalysisCancelled> {
        project_diagnostics(
            &self.analyzer,
            &self.cancellation,
            self.generation,
            self.sources,
        )
    }
}

impl DiagnosticReprojectionRequest {
    pub(crate) fn project(self) -> Result<DiagnosticReprojectionBatch, AnalysisCancelled> {
        project_diagnostics(
            &self.analyzer,
            &self.cancellation,
            self.generation,
            vec![self.source],
        )
    }
}

fn project_diagnostics(
    analyzer: &Analyzer,
    cancellation: &AnalysisCancellationToken,
    generation: DiagnosticGeneration,
    sources: Vec<DiagnosticReprojectionSource>,
) -> Result<DiagnosticReprojectionBatch, AnalysisCancelled> {
    let mut projections = Vec::with_capacity(sources.len());
    for source in sources {
        cancellation.checkpoint()?;
        projections.push(DiagnosticReprojection {
            uri: source.uri,
            document_epoch: source.document_epoch,
            projected: Arc::new(
                source
                    .context
                    .reproject_cancellable(analyzer, cancellation)?,
            ),
            original: source.context,
        });
    }
    cancellation.checkpoint()?;
    Ok(DiagnosticReprojectionBatch {
        generation,
        projections,
    })
}

impl SourceLimitReclassificationPlan {
    pub(crate) fn project(self) -> Result<SourceLimitReclassificationBatch, AnalysisCancelled> {
        let mut documents = Vec::with_capacity(self.documents.len());
        for document in self.documents {
            self.cancellation.checkpoint()?;
            let oversized_span = match document.oversized_source.as_deref() {
                Some(source) => Some(source_limit_diagnostic_span_cancellable(
                    source,
                    &self.cancellation,
                )?),
                None => None,
            };
            documents.push(SourceLimitDocumentProjection {
                uri: document.uri,
                document_epoch: document.document_epoch,
                oversized_span,
            });
        }
        self.cancellation.checkpoint()?;
        Ok(SourceLimitReclassificationBatch {
            request: self.request,
            expected_options: self.expected_options,
            next_options: self.next_options,
            documents,
        })
    }
}

impl DocumentStore {
    pub fn new() -> Self {
        Self::with_session_cancellation(AnalysisCancellationToken::new())
    }

    pub(crate) fn with_session_cancellation(
        session_cancellation: AnalysisCancellationToken,
    ) -> Self {
        let analyzer = Analyzer::with_options(default_lsp_analysis_options());
        Self {
            analyzer,
            analysis_executor: AnalysisExecutor::new(session_cancellation.child()),
            diagnostic_reprojection_cancellation: session_cancellation.child(),
            session_cancellation,
            snapshot_generation: SnapshotGeneration::default(),
            diagnostic_generation: DiagnosticGeneration::default(),
            analyzer_configuration_request: AnalyzerConfigurationRequest::default(),
            next_document_epoch: 0,
            documents: HashMap::new(),
            analysis_generations: HashMap::new(),
            diagnostic_state: HashMap::new(),
            semantic_tokens_state: HashMap::new(),
        }
    }

    #[cfg(test)]
    pub(crate) fn begin_analyzer_options(
        &mut self,
        options: AnalysisOptions,
    ) -> (
        AnalyzerConfigurationChange,
        Option<DiagnosticReprojectionPlan>,
    ) {
        let request = self.begin_analyzer_configuration_request();
        let change = analyzer_configuration_change(self.analyzer.options(), &options);
        if matches!(change, AnalyzerConfigurationChange::Unchanged) {
            return (change, None);
        }

        if change.affects_snapshots() {
            let batch = self
                .prepare_source_limit_reclassification_for(request, options)
                .project()
                .expect("a synchronous analyzer update cannot be cancelled");
            self.commit_source_limit_reclassification(batch)
                .expect("a synchronous analyzer update cannot become stale")
        } else {
            let plan = self.set_diagnostic_analyzer(Analyzer::with_options(options));
            (change, plan)
        }
    }

    pub(crate) fn begin_analyzer_configuration_request(&mut self) -> AnalyzerConfigurationRequest {
        self.analyzer_configuration_request =
            AnalyzerConfigurationRequest(self.analyzer_configuration_request.0.wrapping_add(1));
        self.analyzer_configuration_request
    }

    pub(crate) fn is_analyzer_configuration_request_current(
        &self,
        request: AnalyzerConfigurationRequest,
    ) -> bool {
        request == self.analyzer_configuration_request
    }

    pub(crate) fn prepare_analyzer_options(
        &mut self,
        request: AnalyzerConfigurationRequest,
        options: AnalysisOptions,
    ) -> Option<AnalyzerOptionsPreparation> {
        if !self.is_analyzer_configuration_request_current(request) {
            return None;
        }
        let change = analyzer_configuration_change(self.analyzer.options(), &options);
        if change.affects_snapshots() {
            Some(AnalyzerOptionsPreparation::RequiresSourceLimitProjection(
                self.prepare_source_limit_reclassification_for(request, options),
            ))
        } else {
            let reprojection = if matches!(change, AnalyzerConfigurationChange::Unchanged) {
                None
            } else {
                self.set_diagnostic_analyzer(Analyzer::with_options(options))
            };
            Some(AnalyzerOptionsPreparation::Applied(change, reprojection))
        }
    }

    #[cfg(test)]
    pub(crate) fn prepare_source_limit_reclassification(
        &self,
        next_options: AnalysisOptions,
    ) -> SourceLimitReclassificationPlan {
        self.prepare_source_limit_reclassification_for(
            self.analyzer_configuration_request,
            next_options,
        )
    }

    fn prepare_source_limit_reclassification_for(
        &self,
        request: AnalyzerConfigurationRequest,
        next_options: AnalysisOptions,
    ) -> SourceLimitReclassificationPlan {
        let max_source_bytes = next_options.max_source_bytes();
        let documents = self
            .documents
            .iter()
            .map(|(uri, record)| {
                let oversized_source = max_source_bytes
                    .filter(|limit| {
                        record.document.sync_error.is_none()
                            && !record.document.has_unavailable_source()
                            && record.document.text.len() > *limit
                    })
                    .map(|_| Arc::clone(&record.document.text));
                SourceLimitDocumentSnapshot {
                    uri: uri.clone(),
                    document_epoch: record.epoch,
                    oversized_source,
                }
            })
            .collect();
        SourceLimitReclassificationPlan {
            cancellation: self.session_cancellation.child(),
            request,
            expected_options: self.analyzer.options().clone(),
            next_options,
            documents,
        }
    }

    pub(crate) fn commit_source_limit_reclassification(
        &mut self,
        batch: SourceLimitReclassificationBatch,
    ) -> Option<(
        AnalyzerConfigurationChange,
        Option<DiagnosticReprojectionPlan>,
    )> {
        if !self.is_analyzer_configuration_request_current(batch.request)
            || self.analyzer.options() != &batch.expected_options
            || self.documents.len() != batch.documents.len()
            || batch.documents.iter().any(|document| {
                !self.is_document_epoch_current(&document.uri, document.document_epoch)
            })
        {
            return None;
        }

        let change = analyzer_configuration_change(self.analyzer.options(), &batch.next_options);
        if !change.affects_snapshots() {
            return None;
        }

        let oversized_spans = batch
            .documents
            .into_iter()
            .filter_map(|document| document.oversized_span.map(|span| (document.uri, span)))
            .collect();
        self.replace_analyzer(Analyzer::with_options(batch.next_options), &oversized_spans);
        Some((change, None))
    }

    #[cfg(test)]
    pub fn apply_analyzer_options(
        &mut self,
        options: AnalysisOptions,
    ) -> AnalyzerConfigurationChange {
        let (change, plan) = self.begin_analyzer_options(options);
        if let Some(plan) = plan {
            self.commit_diagnostic_reprojection(
                plan.project()
                    .expect("current diagnostic reprojection cannot be stale"),
            );
        }
        change
    }

    fn set_diagnostic_analyzer(
        &mut self,
        analyzer: Analyzer,
    ) -> Option<DiagnosticReprojectionPlan> {
        self.diagnostic_reprojection_cancellation.cancel();
        self.diagnostic_reprojection_cancellation = self.session_cancellation.child();
        self.analyzer = analyzer;
        self.advance_diagnostic_generation();
        self.analysis_executor.invalidate_all();
        let sources = self
            .analysis_generations
            .iter()
            .filter_map(|(uri, cached)| {
                self.documents
                    .get(uri)
                    .map(|record| DiagnosticReprojectionSource {
                        uri: uri.clone(),
                        document_epoch: record.epoch,
                        context: Arc::clone(&cached.context),
                    })
            })
            .collect::<Vec<_>>();
        (!sources.is_empty()).then(|| DiagnosticReprojectionPlan {
            analyzer: self.analyzer.clone(),
            cancellation: self.diagnostic_reprojection_cancellation.clone(),
            generation: self.diagnostic_generation,
            sources,
        })
    }

    pub(crate) fn commit_diagnostic_reprojection(
        &mut self,
        batch: DiagnosticReprojectionBatch,
    ) -> usize {
        if batch.generation != self.diagnostic_generation {
            return 0;
        }

        let mut committed = 0;
        for projection in batch.projections {
            if !self.is_document_epoch_current(&projection.uri, projection.document_epoch) {
                continue;
            }
            let Some(cached) = self.analysis_generations.get_mut(&projection.uri) else {
                continue;
            };
            if !Arc::ptr_eq(&cached.context, &projection.original) {
                continue;
            }
            cached.context = projection.projected;
            cached.diagnostic_generation = batch.generation;
            committed += 1;
        }
        committed
    }

    pub(crate) fn discard_stale_analysis_generations(&mut self) {
        let generation = self.diagnostic_generation;
        self.analysis_generations
            .retain(|_, cached| cached.diagnostic_generation == generation);
    }

    fn replace_analyzer(
        &mut self,
        analyzer: Analyzer,
        oversized_spans: &HashMap<Uri, DiagnosticSpan>,
    ) {
        self.diagnostic_reprojection_cancellation.cancel();
        self.diagnostic_reprojection_cancellation = self.session_cancellation.child();
        self.analyzer = analyzer;
        self.reclassify_documents_for_current_limit(oversized_spans);
        self.advance_snapshot_generation();
        self.advance_diagnostic_generation();
        self.analysis_generations.clear();
        self.semantic_tokens_state.clear();
        self.analysis_executor.invalidate_all();
    }

    fn advance_snapshot_generation(&mut self) {
        self.snapshot_generation = SnapshotGeneration(self.snapshot_generation.0.wrapping_add(1));
    }

    fn advance_diagnostic_generation(&mut self) {
        self.diagnostic_generation =
            DiagnosticGeneration(self.diagnostic_generation.0.wrapping_add(1));
        self.diagnostic_state.clear();
    }

    fn next_document_epoch(&mut self) -> DocumentEpoch {
        self.next_document_epoch = self.next_document_epoch.wrapping_add(1);
        DocumentEpoch(self.next_document_epoch)
    }

    pub fn diagnostic_context(&self, uri: &Uri) -> Option<DiagnosticContext> {
        self.documents.get(uri).map(|record| {
            DiagnosticContext::new(
                record.document.clone(),
                self.diagnostic_generation,
                record.epoch,
            )
        })
    }

    pub fn is_diagnostic_context_current(&self, context: &DiagnosticContext) -> bool {
        self.diagnostic_generation == context.generation
            && self.is_document_epoch_current(&context.document.uri, context.document_epoch)
    }

    #[cfg(test)]
    pub fn upsert_text(
        &mut self,
        uri: Uri,
        version: i32,
        text: String,
        kind: DocumentKind,
    ) -> StoredDocument {
        if let Some(resource_limit) = self.resource_limit_for_source(&text) {
            return self.upsert_resource_limited(uri, version, kind, resource_limit);
        }

        let document = StoredDocument {
            uri: uri.clone(),
            version,
            text: Arc::<str>::from(text),
            kind,
            resource_limit: None,
            discarded_source: None,
            sync_error: None,
        };
        self.upsert_document(uri, document)
    }

    pub(crate) fn open_prepared_text(
        &mut self,
        uri: Uri,
        version: i32,
        prepared: PreparedDocumentText,
        kind: DocumentKind,
    ) -> StoredDocument {
        if let Some(max_source_bytes) = self.analyzer.options().max_source_bytes()
            && prepared.text.len() > max_source_bytes
        {
            return self.upsert_resource_limited(
                uri,
                version,
                kind,
                DocumentResourceLimit {
                    source_len: prepared.text.len(),
                    max_source_bytes,
                    span: prepared.span,
                },
            );
        }

        let document = StoredDocument {
            uri: uri.clone(),
            version,
            text: Arc::<str>::from(prepared.text),
            kind,
            resource_limit: None,
            discarded_source: None,
            sync_error: None,
        };
        self.upsert_document(uri, document)
    }

    fn upsert_resource_limited(
        &mut self,
        uri: Uri,
        version: i32,
        kind: DocumentKind,
        resource_limit: DocumentResourceLimit,
    ) -> StoredDocument {
        let document = StoredDocument {
            uri: uri.clone(),
            version,
            text: Arc::<str>::from(""),
            kind,
            resource_limit: Some(resource_limit),
            discarded_source: None,
            sync_error: None,
        };
        self.upsert_document(uri, document)
    }

    fn upsert_sync_error(
        &mut self,
        uri: Uri,
        version: i32,
        kind: DocumentKind,
        sync_error: DocumentSyncError,
    ) -> StoredDocument {
        let document = StoredDocument {
            uri: uri.clone(),
            version,
            text: Arc::<str>::from(""),
            kind,
            resource_limit: None,
            discarded_source: None,
            sync_error: Some(sync_error),
        };
        self.upsert_document(uri, document)
    }

    fn upsert_document(&mut self, uri: Uri, document: StoredDocument) -> StoredDocument {
        self.analysis_executor.invalidate(&uri);
        self.analysis_generations.remove(&uri);
        self.diagnostic_state.remove(&uri);
        let epoch = self.next_document_epoch();
        self.documents.insert(
            uri,
            DocumentRecord {
                document: document.clone(),
                epoch,
            },
        );
        document
    }

    #[cfg(test)]
    fn resource_limit_for_source(&self, source: &str) -> Option<DocumentResourceLimit> {
        let max_source_bytes = self.analyzer.options().max_source_bytes()?;
        (source.len() > max_source_bytes).then(|| DocumentResourceLimit {
            source_len: source.len(),
            max_source_bytes,
            span: source_limit_diagnostic_span(source),
        })
    }

    fn reclassify_documents_for_current_limit(
        &mut self,
        oversized_spans: &HashMap<Uri, DiagnosticSpan>,
    ) {
        let current_limit = self.analyzer.options().max_source_bytes();
        for (uri, record) in &mut self.documents {
            if let Some((source_len, previous_max_source_bytes, span)) =
                resource_state_source_len_and_previous_limit(&record.document)
            {
                match current_limit {
                    Some(max_source_bytes) if source_len > max_source_bytes => {
                        record.document.resource_limit = Some(DocumentResourceLimit {
                            source_len,
                            max_source_bytes,
                            span,
                        });
                        record.document.discarded_source = None;
                    }
                    _ => {
                        record.document.resource_limit = None;
                        record.document.discarded_source = Some(DocumentDiscardedSource {
                            source_len,
                            previous_max_source_bytes,
                            span,
                        });
                    }
                }
                continue;
            }

            let Some(max_source_bytes) = current_limit else {
                continue;
            };
            let source_len = record.document.text.len();
            if record.document.sync_error.is_none() && source_len > max_source_bytes {
                let span = *oversized_spans
                    .get(uri)
                    .expect("source-limit projection must cover every newly oversized document");
                record.document.text = Arc::<str>::from("");
                record.document.resource_limit = Some(DocumentResourceLimit {
                    source_len,
                    max_source_bytes,
                    span,
                });
            }
        }
    }

    #[cfg(test)]
    pub fn open_text(
        &mut self,
        uri: Uri,
        version: i32,
        text: String,
        kind: DocumentKind,
    ) -> StoredDocument {
        self.upsert_text(uri, version, text, kind)
    }

    #[cfg(test)]
    pub fn apply_text_changes(
        &mut self,
        uri: Uri,
        version: i32,
        changes: impl IntoIterator<Item = TextDocumentContentChangeEvent>,
    ) -> TextDocumentUpdate {
        match self.capture_text_changes(uri, version, changes) {
            TextChangePreparation::Immediate(update) => update,
            TextChangePreparation::Prepare(plan) => self.commit_prepared_text_changes(
                plan.prepare()
                    .expect("a private text-change token cannot be cancelled"),
            ),
        }
    }

    pub(crate) fn capture_text_changes(
        &self,
        uri: Uri,
        version: i32,
        changes: impl IntoIterator<Item = TextDocumentContentChangeEvent>,
    ) -> TextChangePreparation {
        let Some(record) = self.documents.get(&uri) else {
            return TextChangePreparation::Immediate(TextDocumentUpdate::MissingDocument);
        };
        let current = &record.document;
        if version <= current.version {
            return TextChangePreparation::Immediate(TextDocumentUpdate::StaleVersion {
                current_version: current.version,
                attempted_version: version,
            });
        }
        let changes = changes.into_iter().collect::<Vec<_>>();
        if changes.is_empty() {
            return TextChangePreparation::Immediate(TextDocumentUpdate::EmptyChangeSet);
        }

        TextChangePreparation::Prepare(Box::new(TextChangePlan {
            uri,
            version,
            kind: current.kind,
            expected_epoch: record.epoch,
            expected_configuration: self.analyzer_configuration_request,
            current_text: (!current.has_unavailable_source()).then(|| Arc::clone(&current.text)),
            unavailable_source: current.unavailable_source_state(),
            changes,
            cancellation: self.session_cancellation.child(),
        }))
    }

    pub(crate) fn commit_prepared_text_changes(
        &mut self,
        prepared: PreparedTextChanges,
    ) -> TextDocumentUpdate {
        let Some(record) = self.documents.get(&prepared.uri) else {
            return TextDocumentUpdate::MissingDocument;
        };
        if record.epoch != prepared.expected_epoch
            || self.analyzer_configuration_request != prepared.expected_configuration
        {
            return TextDocumentUpdate::Superseded;
        }

        match prepared.mutation {
            PreparedTextMutation::Text(text) => {
                self.open_prepared_text(prepared.uri, prepared.version, text, prepared.kind);
                TextDocumentUpdate::Applied
            }
            PreparedTextMutation::SyncError { error, update } => {
                self.upsert_sync_error(prepared.uri, prepared.version, prepared.kind, error);
                update
            }
        }
    }

    #[cfg(test)]
    pub fn upsert(&mut self, uri: Uri, version: i32, text: String) -> Arc<DocumentSnapshot> {
        let kind = DocumentKind::from_path(uri.path().as_str());
        self.upsert_text(uri.clone(), version, text, kind);
        self.snapshot(&uri)
            .expect("snapshot should exist after inserting document text")
    }

    pub fn get(&self, uri: &Uri) -> Option<&StoredDocument> {
        self.documents.get(uri).map(|record| &record.document)
    }

    #[cfg(test)]
    pub fn analyzer_options(&self) -> &AnalysisOptions {
        self.analyzer.options()
    }

    #[cfg(test)]
    pub fn snapshot(&mut self, uri: &Uri) -> Option<Arc<DocumentSnapshot>> {
        self.snapshot_context(uri).map(|context| context.snapshot)
    }

    pub fn snapshot_context(&mut self, uri: &Uri) -> Option<SnapshotContext> {
        if let Some(cached) = self.analysis_generations.get(uri) {
            return Some(self.cached_snapshot_context(cached, self.documents.get(uri)?.epoch));
        }

        let request = self.snapshot_build_request(uri)?;
        let analysis = match request.build() {
            Ok(analysis) => analysis,
            Err(rejection) => {
                let document = self.documents.get(uri)?.document.clone();
                self.upsert_resource_limited(
                    document.uri,
                    document.version,
                    document.kind,
                    DocumentResourceLimit {
                        source_len: rejection.source_len(),
                        max_source_bytes: rejection.max_source_bytes(),
                        span: rejection
                            .payload()
                            .diagnostics
                            .first()
                            .and_then(|diagnostic| diagnostic.span)
                            .expect("source-limit rejection must retain its source span"),
                    },
                );
                return None;
            }
        };
        self.insert_built_analysis(&request, analysis)
    }

    pub(crate) fn snapshot_build_request(&self, uri: &Uri) -> Option<AnalysisBuildRequest> {
        let record = self.documents.get(uri)?;
        if record.document.has_unavailable_source() || self.analysis_generations.contains_key(uri) {
            return None;
        }
        let key = AnalysisBuildKey::new(
            record.document.uri.clone(),
            record.document.version,
            self.analysis_executor.generation_for(uri),
            self.snapshot_generation,
            self.diagnostic_generation,
            record.epoch,
        );
        Some(AnalysisBuildRequest::new(
            key,
            Arc::clone(&record.document.text),
            record.document.kind,
            self.analyzer.clone(),
        ))
    }

    pub fn insert_built_analysis(
        &mut self,
        request: &AnalysisBuildRequest,
        analysis: Arc<DocumentAnalysisContext>,
    ) -> Option<SnapshotContext> {
        if self.snapshot_generation != request.snapshot_generation()
            || self.diagnostic_generation != request.diagnostic_generation()
            || !self.is_document_epoch_current(request.uri(), request.document_epoch())
        {
            return None;
        }

        self.analysis_generations.insert(
            request.uri().clone(),
            CachedAnalysisGeneration {
                context: Arc::clone(&analysis),
                diagnostic_generation: request.diagnostic_generation(),
            },
        );
        Some(SnapshotContext::with_analysis(
            Arc::clone(&analysis.snapshot),
            Arc::clone(&analysis.payload),
            request.snapshot_generation(),
            request.diagnostic_generation(),
            request.document_epoch(),
        ))
    }

    pub fn is_snapshot_context_current(&self, context: &SnapshotContext) -> bool {
        self.snapshot_generation == context.generation
            && self.is_document_epoch_current(context.snapshot.uri(), context.document_epoch)
    }

    pub fn is_analysis_context_current(&self, context: &SnapshotContext) -> bool {
        self.is_snapshot_context_current(context)
            && context.analysis_generation() == self.diagnostic_generation
    }

    #[cfg(test)]
    pub fn is_snapshot_contexts_current(&self, contexts: &[SnapshotContext]) -> bool {
        contexts
            .iter()
            .all(|context| self.is_snapshot_context_current(context))
    }

    fn is_document_epoch_current(&self, uri: &Uri, document_epoch: DocumentEpoch) -> bool {
        self.documents
            .get(uri)
            .is_some_and(|record| record.epoch == document_epoch)
    }

    pub fn has_snapshot(&self, uri: &Uri) -> bool {
        self.analysis_generations.contains_key(uri)
    }

    pub fn has_analysis_payload(&self, uri: &Uri) -> bool {
        self.analysis_generations
            .get(uri)
            .is_some_and(|cached| cached.diagnostic_generation == self.diagnostic_generation)
    }

    pub(crate) fn diagnostic_reprojection_request(
        &self,
        uri: &Uri,
    ) -> Option<DiagnosticReprojectionRequest> {
        let cached = self.analysis_generations.get(uri)?;
        if cached.diagnostic_generation == self.diagnostic_generation {
            return None;
        }
        let record = self.documents.get(uri)?;
        Some(DiagnosticReprojectionRequest {
            analyzer: self.analyzer.clone(),
            cancellation: self.diagnostic_reprojection_cancellation.clone(),
            generation: self.diagnostic_generation,
            source: DiagnosticReprojectionSource {
                uri: uri.clone(),
                document_epoch: record.epoch,
                context: Arc::clone(&cached.context),
            },
        })
    }

    #[cfg(test)]
    pub(crate) fn cached_analysis_generation(
        &self,
        uri: &Uri,
    ) -> Option<&Arc<DocumentAnalysisContext>> {
        self.analysis_generations
            .get(uri)
            .map(|cached| &cached.context)
    }

    pub(crate) fn analysis_executor(&self) -> AnalysisExecutor {
        self.analysis_executor.clone()
    }

    pub fn remove(&mut self, uri: &Uri) {
        self.analysis_executor.forget(uri);
        self.documents.remove(uri);
        self.analysis_generations.remove(uri);
        self.diagnostic_state.remove(uri);
        self.semantic_tokens_state.remove(uri);
    }

    pub(crate) fn diagnostic_contexts(&self) -> Vec<DiagnosticContext> {
        self.documents
            .values()
            .map(|record| {
                DiagnosticContext::new(
                    record.document.clone(),
                    self.diagnostic_generation,
                    record.epoch,
                )
            })
            .collect()
    }

    #[cfg(test)]
    pub fn snapshot_build_requests(&self) -> (Vec<SnapshotContext>, Vec<AnalysisBuildRequest>) {
        let mut contexts = Vec::new();
        let mut requests = Vec::new();

        for (uri, record) in &self.documents {
            if let Some(cached) = self.analysis_generations.get(uri) {
                contexts.push(self.cached_snapshot_context(cached, record.epoch));
            } else if let Some(request) = self.snapshot_build_request(uri) {
                requests.push(request);
            }
        }

        (contexts, requests)
    }

    fn cached_snapshot_context(
        &self,
        cached: &CachedAnalysisGeneration,
        document_epoch: DocumentEpoch,
    ) -> SnapshotContext {
        SnapshotContext::with_analysis(
            Arc::clone(&cached.context.snapshot),
            Arc::clone(&cached.context.payload),
            self.snapshot_generation,
            cached.diagnostic_generation,
            document_epoch,
        )
    }

    #[cfg(test)]
    pub fn snapshot_contexts_for_requests(
        &mut self,
        requests: Vec<(AnalysisBuildRequest, Arc<DocumentAnalysisContext>)>,
    ) -> SnapshotBatchCommit {
        let mut contexts = Vec::new();
        let mut stale_open_documents = false;

        for (request, analysis) in requests {
            match self.insert_built_analysis(&request, analysis) {
                Some(_context) => {
                    contexts.push(_context);
                }
                None if self.get(request.uri()).is_some() => stale_open_documents = true,
                None => {}
            }
        }

        SnapshotBatchCommit {
            contexts,
            stale_open_documents,
        }
    }

    #[cfg(test)]
    pub fn semantic_tokens_state(&self, uri: &Uri) -> Option<&SemanticTokensState> {
        self.semantic_tokens_state
            .get(uri)
            .map(|stored| &stored.state)
    }

    pub fn semantic_tokens_state_for_delta(
        &self,
        uri: &Uri,
        previous_result_id: &str,
    ) -> Option<SemanticTokensState> {
        self.semantic_tokens_state.get(uri).and_then(|stored| {
            (stored.snapshot_generation == self.snapshot_generation
                && stored.state.result_id.as_deref() == Some(previous_result_id))
            .then(|| stored.state.clone())
        })
    }

    pub fn set_semantic_tokens_state_if_current(
        &mut self,
        context: &SnapshotContext,
        state: SemanticTokensState,
    ) -> bool {
        if !self.is_snapshot_context_current(context) {
            return false;
        }

        self.semantic_tokens_state.insert(
            context.snapshot.uri().clone(),
            StoredSemanticTokensState {
                snapshot_generation: context.generation,
                state,
            },
        );
        true
    }

    pub fn diagnostic_state(&self, uri: &Uri) -> Option<DocumentDiagnosticState> {
        self.diagnostic_state.get(uri).and_then(|stored| {
            (stored.generation == self.diagnostic_generation
                && self.is_document_epoch_current(uri, stored.document_epoch))
            .then(|| stored.state.clone())
        })
    }

    pub fn set_diagnostic_state_if_current(
        &mut self,
        context: &DiagnosticContext,
        state: DocumentDiagnosticState,
    ) -> bool {
        if !self.is_diagnostic_context_current(context) {
            return false;
        }

        self.diagnostic_state.insert(
            context.document.uri.clone(),
            StoredDiagnosticState {
                generation: context.generation,
                document_epoch: context.document_epoch,
                state,
            },
        );
        true
    }
}

#[derive(Debug, Clone)]
struct DocumentRecord {
    document: StoredDocument,
    epoch: DocumentEpoch,
}

#[derive(Debug, Clone)]
struct StoredSemanticTokensState {
    snapshot_generation: SnapshotGeneration,
    state: SemanticTokensState,
}

#[derive(Debug, Clone)]
struct StoredDiagnosticState {
    generation: DiagnosticGeneration,
    document_epoch: DocumentEpoch,
    state: DocumentDiagnosticState,
}

#[derive(Debug, Clone)]
pub struct DocumentDiagnosticState {
    pub result_id: String,
    pub diagnostics: Vec<Diagnostic>,
}

#[derive(Debug, Clone)]
pub struct DiagnosticContext {
    pub document: StoredDocument,
    generation: DiagnosticGeneration,
    document_epoch: DocumentEpoch,
}

impl DiagnosticContext {
    fn new(
        document: StoredDocument,
        generation: DiagnosticGeneration,
        document_epoch: DocumentEpoch,
    ) -> Self {
        Self {
            document,
            generation,
            document_epoch,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AnalyzerConfigurationChange {
    Unchanged,
    DiagnosticsOnly,
    SnapshotAffecting,
}

impl AnalyzerConfigurationChange {
    pub fn affects_diagnostics(self) -> bool {
        !matches!(self, Self::Unchanged)
    }

    pub fn affects_snapshots(self) -> bool {
        matches!(self, Self::SnapshotAffecting)
    }
}

pub(crate) fn analyzer_configuration_change(
    current: &AnalysisOptions,
    next: &AnalysisOptions,
) -> AnalyzerConfigurationChange {
    if current == next {
        AnalyzerConfigurationChange::Unchanged
    } else if current.snapshot_policy() == next.snapshot_policy() {
        AnalyzerConfigurationChange::DiagnosticsOnly
    } else {
        AnalyzerConfigurationChange::SnapshotAffecting
    }
}

fn apply_text_content_change(text: &mut Rope, change: TextDocumentContentChangeEvent) -> bool {
    if let Some(range) = change.range {
        let Some(char_range) = lsp_range_to_char_range(text, range) else {
            return false;
        };
        text.remove(char_range.clone());
        text.insert(char_range.start, &change.text);
    } else {
        *text = Rope::from_str(&change.text);
    }
    true
}

fn changes_from_last_full_replacement(
    changes: Vec<TextDocumentContentChangeEvent>,
) -> Vec<TextDocumentContentChangeEvent> {
    let Some(recovery_start) = changes.iter().rposition(|change| change.range.is_none()) else {
        return changes;
    };
    changes.into_iter().skip(recovery_start).collect()
}

fn lsp_range_to_char_range(text: &Rope, range: Range) -> Option<std::ops::Range<usize>> {
    if !position_le(range.start, range.end) {
        return None;
    }

    let start = char_offset_for_lsp_position(text, range.start)?;
    let end = char_offset_for_lsp_position(text, range.end)?;
    (start <= end).then_some(start..end)
}

fn char_offset_for_lsp_position(text: &Rope, position: Position) -> Option<usize> {
    let line_index = position.line as usize;
    let line = text.get_line(line_index)?;
    let line_start = text.try_line_to_char(line_index).ok()?;
    let content_len = line_content_char_len(line);
    let target_utf16 = position.character as usize;
    let mut utf16 = 0usize;

    for (relative_char, ch) in line.chars().take(content_len).enumerate() {
        if utf16 == target_utf16 {
            return Some(line_start + relative_char);
        }
        let next_utf16 = utf16 + ch.len_utf16();
        if target_utf16 < next_utf16 {
            return None;
        }
        utf16 = next_utf16;
    }

    Some(line_start + content_len)
}

fn line_content_char_len(line: RopeSlice<'_>) -> usize {
    let mut len = line.len_chars();
    if len > 0 && line.char(len - 1) == '\n' {
        len -= 1;
        if len > 0 && line.char(len - 1) == '\r' {
            len -= 1;
        }
    } else if len > 0 && line.char(len - 1) == '\r' {
        len -= 1;
    }
    len
}

fn position_le(left: Position, right: Position) -> bool {
    left.line < right.line || (left.line == right.line && left.character <= right.character)
}
