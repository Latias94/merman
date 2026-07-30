use super::LanguageSession;
use crate::session::analysis::executor::{
    AnalysisExecutor, DiagnosticReprojectionLease, DiagnosticReprojectionRequest,
    LSP_ANALYSIS_IN_FLIGHT_LIMIT,
};
#[cfg(test)]
use crate::session::analysis::request::TestAnalysisGate;
use crate::session::analysis::request::{AnalysisBuildKey, AnalysisBuildRequest};
#[cfg(test)]
use crate::session::cache::WeightedCacheStatistics;
use crate::session::cache::{WeightedLru, WeightedReplacement, conservative_weighted_entry_bytes};
#[cfg(test)]
use crate::snapshot::DocumentSnapshot;
use crate::snapshot::{
    DiagnosticGeneration, DocumentAnalysisContext, DocumentEpoch, SnapshotContext,
    SnapshotGeneration,
};
use futures::stream::{self, StreamExt};
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
pub(crate) const DEFAULT_LSP_ANALYSIS_CACHE_BUDGET_BYTES: usize = 64 * 1024 * 1024;

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
pub(super) struct SessionState {
    analyzer: Analyzer,
    analysis_executor: AnalysisExecutor,
    session_cancellation: AnalysisCancellationToken,
    diagnostic_reprojection_cancellation: AnalysisCancellationToken,
    snapshot_generation: SnapshotGeneration,
    diagnostic_generation: DiagnosticGeneration,
    latest_configuration_request: ConfigurationRequestId,
    configuration_revision: ConfigurationRevision,
    documents_revision: DocumentsRevision,
    next_document_epoch: u64,
    documents: HashMap<Uri, DocumentRecord>,
    open_document_tracker: Arc<OpenDocumentTracker>,
    analysis_generations: WeightedLru<Uri, CachedAnalysisGeneration>,
    #[cfg(test)]
    analysis_test_gate: Option<Arc<TestAnalysisGate>>,
}

#[derive(Debug)]
struct CachedAnalysisGeneration {
    context: Arc<DocumentAnalysisContext>,
    document_epoch: DocumentEpoch,
    snapshot_generation: SnapshotGeneration,
    diagnostic_generation: DiagnosticGeneration,
}

fn cached_analysis_weight(uri: &Uri, context: &DocumentAnalysisContext) -> usize {
    context
        .estimated_owned_weight()
        .total()
        .saturating_add(conservative_weighted_entry_bytes::<
            Uri,
            CachedAnalysisGeneration,
        >(uri.as_str().len()))
}

#[derive(Debug)]
struct SnapshotConfigurationPlan {
    cancellation: AnalysisCancellationToken,
    request: ConfigurationRequestId,
    expected_configuration_revision: ConfigurationRevision,
    expected_documents_revision: Option<DocumentsRevision>,
    base_analyzer: Analyzer,
    next_options: AnalysisOptions,
    documents: Option<Vec<SourceLimitDocumentSnapshot>>,
}

#[derive(Debug)]
struct SourceLimitDocumentSnapshot {
    uri: Uri,
    oversized_source: Option<Arc<str>>,
}

#[derive(Debug)]
struct SnapshotConfigurationBatch {
    request: ConfigurationRequestId,
    expected_configuration_revision: ConfigurationRevision,
    expected_documents_revision: Option<DocumentsRevision>,
    next_options: AnalysisOptions,
    analyzer: Analyzer,
    oversized_spans: Option<HashMap<Uri, DiagnosticSpan>>,
}

#[derive(Debug)]
enum AnalyzerOptionsPreparation {
    Applied(
        AnalyzerConfigurationChange,
        Vec<DiagnosticReprojectionRequest>,
    ),
    RequiresSnapshotPreparation(Box<SnapshotConfigurationPlan>),
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
struct ConfigurationRequestId(u64);

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
struct ConfigurationRevision(u64);

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
struct DocumentsRevision(u64);

#[derive(Debug, Clone)]
pub(crate) struct StoredDocument {
    pub uri: Uri,
    pub version: i32,
    pub kind: DocumentKind,
    source: DocumentSource,
}

#[derive(Debug, Clone)]
enum DocumentSource {
    Available(Arc<str>),
    ResourceLimited(DocumentResourceLimit),
    Discarded(DocumentDiscardedSource),
    SyncError(DocumentSyncError),
}

#[derive(Debug)]
struct PreparedDocumentText {
    text: String,
    oversized_span: Option<DiagnosticSpan>,
}

#[derive(Debug)]
struct OpenDocumentTicket {
    uri: Uri,
    expected_document_epoch: Option<DocumentEpoch>,
    expected_uri_revision: u64,
    expected_configuration_revision: ConfigurationRevision,
    max_source_bytes: Option<usize>,
    tracker: Arc<OpenDocumentTracker>,
}

impl Drop for OpenDocumentTicket {
    fn drop(&mut self) {
        self.tracker.release(&self.uri);
    }
}

#[derive(Debug, Default)]
struct OpenDocumentTracker {
    entries: std::sync::Mutex<HashMap<Uri, OpenDocumentClock>>,
}

#[derive(Debug)]
struct OpenDocumentClock {
    revision: u64,
    active_tickets: usize,
}

impl OpenDocumentTracker {
    fn capture(&self, uri: &Uri) -> u64 {
        let mut entries = crate::sync::lock_recovering_poison(&self.entries);
        let clock = entries.entry(uri.clone()).or_insert(OpenDocumentClock {
            revision: 0,
            active_tickets: 0,
        });
        clock.active_tickets = clock
            .active_tickets
            .checked_add(1)
            .expect("open document ticket count exhausted");
        clock.revision
    }

    fn is_current(&self, uri: &Uri, expected_revision: u64) -> bool {
        crate::sync::lock_recovering_poison(&self.entries)
            .get(uri)
            .is_some_and(|clock| clock.revision == expected_revision)
    }

    fn advance(&self, uri: &Uri) {
        let mut entries = crate::sync::lock_recovering_poison(&self.entries);
        let Some(clock) = entries.get_mut(uri) else {
            return;
        };
        clock.revision = clock
            .revision
            .checked_add(1)
            .expect("open document URI revision exhausted");
    }

    fn release(&self, uri: &Uri) {
        let mut entries = crate::sync::lock_recovering_poison(&self.entries);
        let remove = if let Some(clock) = entries.get_mut(uri) {
            clock.active_tickets = clock
                .active_tickets
                .checked_sub(1)
                .expect("open document ticket count underflow");
            clock.active_tickets == 0
        } else {
            debug_assert!(false, "open document ticket tracker entry disappeared");
            false
        };
        if remove {
            entries.remove(uri);
        }
    }
}

impl PreparedDocumentText {
    pub(crate) fn new_cancellable(
        text: String,
        max_source_bytes: Option<usize>,
        cancellation: &AnalysisCancellationToken,
    ) -> Result<Self, AnalysisCancelled> {
        cancellation.checkpoint()?;
        let oversized_span = match max_source_bytes {
            Some(limit) if text.len() > limit => Some(source_limit_diagnostic_span_cancellable(
                &text,
                cancellation,
            )?),
            _ => None,
        };
        Ok(Self {
            text,
            oversized_span,
        })
    }
}

impl TextChangePlan {
    fn prepare(self) -> Result<PreparedTextChanges, AnalysisCancelled> {
        let Self {
            uri,
            version,
            kind,
            expected_epoch,
            expected_configuration_revision,
            max_source_bytes,
            source,
            changes,
            cancellation,
        } = self;
        cancellation.checkpoint()?;
        let prepared_text = match source {
            CapturedDocumentSource::Available(current_text) => {
                apply_available_text_changes(&current_text, changes, &cancellation)?
            }
            CapturedDocumentSource::Unavailable(unavailable_source) => {
                let Some(recovery_start) =
                    changes.iter().rposition(|change| change.range.is_none())
                else {
                    return Ok(PreparedTextChanges {
                        uri,
                        version,
                        kind,
                        expected_epoch,
                        expected_configuration_revision,
                        mutation: full_sync_mutation(unavailable_source),
                    });
                };
                apply_changes_from_full_replacement(changes, recovery_start, &cancellation)?
            }
        };
        let mutation = match prepared_text {
            Ok(text) => PreparedTextMutation::Text(PreparedDocumentText::new_cancellable(
                text,
                max_source_bytes,
                &cancellation,
            )?),
            Err(()) => invalid_range_mutation(),
        };
        cancellation.checkpoint()?;
        Ok(PreparedTextChanges {
            uri,
            version,
            kind,
            expected_epoch,
            expected_configuration_revision,
            mutation,
        })
    }
}

fn apply_available_text_changes(
    current_text: &str,
    changes: Vec<TextDocumentContentChangeEvent>,
    cancellation: &AnalysisCancellationToken,
) -> Result<Result<String, ()>, AnalysisCancelled> {
    if let Some(replacement_index) = changes.iter().rposition(|change| change.range.is_none()) {
        apply_changes_from_full_replacement(changes, replacement_index, cancellation)
    } else {
        apply_changes_to_base(current_text, changes, cancellation)
    }
}

fn apply_changes_from_full_replacement(
    changes: Vec<TextDocumentContentChangeEvent>,
    replacement_index: usize,
    cancellation: &AnalysisCancellationToken,
) -> Result<Result<String, ()>, AnalysisCancelled> {
    let mut changes = changes.into_iter().skip(replacement_index);
    let replacement = changes
        .next()
        .expect("full-replacement position must name an existing change");
    debug_assert!(replacement.range.is_none());
    cancellation.checkpoint()?;

    let mut changes = changes.peekable();
    if changes.peek().is_none() {
        return Ok(Ok(replacement.text));
    }
    apply_changes_to_base(&replacement.text, changes, cancellation)
}

fn apply_changes_to_base(
    base: &str,
    changes: impl IntoIterator<Item = TextDocumentContentChangeEvent>,
    cancellation: &AnalysisCancellationToken,
) -> Result<Result<String, ()>, AnalysisCancelled> {
    cancellation.checkpoint()?;
    let mut text = Rope::from_str(base);
    cancellation.checkpoint()?;
    match apply_text_content_changes(&mut text, changes, cancellation)? {
        Ok(()) => Ok(Ok(text.to_string())),
        Err(()) => Ok(Err(())),
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
enum TextChangePreparation {
    Immediate(TextDocumentUpdate),
    Prepare(Box<TextChangePlan>),
}

#[derive(Debug)]
struct TextChangePlan {
    uri: Uri,
    version: i32,
    kind: DocumentKind,
    expected_epoch: DocumentEpoch,
    expected_configuration_revision: ConfigurationRevision,
    max_source_bytes: Option<usize>,
    source: CapturedDocumentSource,
    changes: Vec<TextDocumentContentChangeEvent>,
    cancellation: AnalysisCancellationToken,
}

#[derive(Debug)]
enum CapturedDocumentSource {
    Available(Arc<str>),
    Unavailable(UnavailableSourceState),
}

#[derive(Debug)]
struct PreparedTextChanges {
    uri: Uri,
    version: i32,
    kind: DocumentKind,
    expected_epoch: DocumentEpoch,
    expected_configuration_revision: ConfigurationRevision,
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
pub(crate) struct DocumentResourceLimit {
    pub source_len: usize,
    pub max_source_bytes: usize,
    pub span: DiagnosticSpan,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct DocumentDiscardedSource {
    pub source_len: usize,
    pub previous_max_source_bytes: usize,
    pub span: DiagnosticSpan,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum DocumentSyncError {
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
    pub(crate) fn available(
        uri: Uri,
        version: i32,
        kind: DocumentKind,
        text: impl Into<Arc<str>>,
    ) -> Self {
        Self {
            uri,
            version,
            kind,
            source: DocumentSource::Available(text.into()),
        }
    }

    pub(crate) fn resource_limited(
        uri: Uri,
        version: i32,
        kind: DocumentKind,
        limit: DocumentResourceLimit,
    ) -> Self {
        Self {
            uri,
            version,
            kind,
            source: DocumentSource::ResourceLimited(limit),
        }
    }

    #[cfg(test)]
    pub(crate) fn discarded(
        uri: Uri,
        version: i32,
        kind: DocumentKind,
        discarded: DocumentDiscardedSource,
    ) -> Self {
        Self {
            uri,
            version,
            kind,
            source: DocumentSource::Discarded(discarded),
        }
    }

    pub(crate) fn sync_error(
        uri: Uri,
        version: i32,
        kind: DocumentKind,
        error: DocumentSyncError,
    ) -> Self {
        Self {
            uri,
            version,
            kind,
            source: DocumentSource::SyncError(error),
        }
    }

    pub(crate) fn text(&self) -> Option<&Arc<str>> {
        match &self.source {
            DocumentSource::Available(text) => Some(text),
            _ => None,
        }
    }

    pub(crate) fn resource_limit(&self) -> Option<DocumentResourceLimit> {
        match &self.source {
            DocumentSource::ResourceLimited(limit) => Some(*limit),
            _ => None,
        }
    }

    pub(crate) fn discarded_source(&self) -> Option<DocumentDiscardedSource> {
        match &self.source {
            DocumentSource::Discarded(discarded) => Some(*discarded),
            _ => None,
        }
    }

    pub(crate) fn sync_error_state(&self) -> Option<DocumentSyncError> {
        match &self.source {
            DocumentSource::SyncError(error) => Some(*error),
            _ => None,
        }
    }

    pub fn has_unavailable_source(&self) -> bool {
        !matches!(&self.source, DocumentSource::Available(_))
    }

    fn captured_source(&self) -> CapturedDocumentSource {
        match &self.source {
            DocumentSource::Available(text) => CapturedDocumentSource::Available(Arc::clone(text)),
            DocumentSource::ResourceLimited(limit) => {
                CapturedDocumentSource::Unavailable(UnavailableSourceState::ResourceLimited(*limit))
            }
            DocumentSource::Discarded(discarded) => {
                CapturedDocumentSource::Unavailable(UnavailableSourceState::Discarded(*discarded))
            }
            DocumentSource::SyncError(error) => {
                CapturedDocumentSource::Unavailable(UnavailableSourceState::SyncError(*error))
            }
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum TextDocumentUpdate {
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
pub(crate) struct SemanticTokensState {
    pub result_id: Option<String>,
    pub tokens: Vec<SemanticToken>,
}

impl SemanticTokensState {
    pub fn new(result_id: Option<String>, tokens: Vec<SemanticToken>) -> Self {
        Self { result_id, tokens }
    }
}

impl SnapshotConfigurationPlan {
    fn prepare(self) -> Result<SnapshotConfigurationBatch, AnalysisCancelled> {
        self.cancellation.checkpoint()?;
        let analyzer = self
            .base_analyzer
            .with_snapshot_policy(self.next_options.snapshot.clone())
            .with_diagnostic_policy(self.next_options.diagnostics.clone());
        self.cancellation.checkpoint()?;

        let oversized_spans = self
            .documents
            .map(|documents| {
                let mut oversized_spans = HashMap::new();
                for document in documents {
                    self.cancellation.checkpoint()?;
                    if let Some(source) = document.oversized_source.as_deref() {
                        oversized_spans.insert(
                            document.uri,
                            source_limit_diagnostic_span_cancellable(source, &self.cancellation)?,
                        );
                    }
                }
                Ok(oversized_spans)
            })
            .transpose()?;
        self.cancellation.checkpoint()?;
        Ok(SnapshotConfigurationBatch {
            request: self.request,
            expected_configuration_revision: self.expected_configuration_revision,
            expected_documents_revision: self.expected_documents_revision,
            next_options: self.next_options,
            analyzer,
            oversized_spans,
        })
    }
}

impl SessionState {
    pub(super) fn with_session_cancellation(
        session_cancellation: AnalysisCancellationToken,
    ) -> Self {
        Self::with_session_cancellation_and_cache_budget(
            session_cancellation,
            DEFAULT_LSP_ANALYSIS_CACHE_BUDGET_BYTES,
        )
    }

    pub(super) fn with_session_cancellation_and_cache_budget(
        session_cancellation: AnalysisCancellationToken,
        analysis_cache_budget: usize,
    ) -> Self {
        let analyzer = Analyzer::with_options(default_lsp_analysis_options());
        Self::with_analyzer_and_cache_budget(analyzer, session_cancellation, analysis_cache_budget)
    }

    pub(super) fn with_analyzer_and_cache_budget(
        analyzer: Analyzer,
        session_cancellation: AnalysisCancellationToken,
        analysis_cache_budget: usize,
    ) -> Self {
        Self {
            analyzer,
            analysis_executor: AnalysisExecutor::new(session_cancellation.child()),
            diagnostic_reprojection_cancellation: session_cancellation.child(),
            session_cancellation,
            snapshot_generation: SnapshotGeneration::default(),
            diagnostic_generation: DiagnosticGeneration::default(),
            latest_configuration_request: ConfigurationRequestId::default(),
            configuration_revision: ConfigurationRevision::default(),
            documents_revision: DocumentsRevision::default(),
            next_document_epoch: 0,
            documents: HashMap::new(),
            open_document_tracker: Arc::new(OpenDocumentTracker::default()),
            analysis_generations: WeightedLru::new(analysis_cache_budget),
            #[cfg(test)]
            analysis_test_gate: None,
        }
    }

    #[cfg(test)]
    pub(super) fn analysis_cache_total_weight(&self) -> usize {
        self.analysis_generations.total_weight()
    }

    #[cfg(test)]
    pub(super) fn analysis_cache_len(&self) -> usize {
        self.analysis_generations.len()
    }

    #[cfg(test)]
    fn analysis_cache_statistics(&self) -> WeightedCacheStatistics {
        self.analysis_generations.statistics()
    }

    #[cfg(test)]
    fn estimated_analysis_cache_entry_weight(
        uri: &Uri,
        context: &DocumentAnalysisContext,
    ) -> usize {
        cached_analysis_weight(uri, context)
    }

    #[cfg(test)]
    pub(super) fn set_analysis_test_gate(&mut self, gate: Option<Arc<TestAnalysisGate>>) {
        self.analysis_test_gate = gate;
    }

    fn begin_analyzer_configuration_request(&mut self) -> ConfigurationRequestId {
        self.latest_configuration_request = ConfigurationRequestId(
            self.latest_configuration_request
                .0
                .checked_add(1)
                .expect("configuration request id exhausted"),
        );
        self.latest_configuration_request
    }

    fn is_analyzer_configuration_request_current(&self, request: ConfigurationRequestId) -> bool {
        request == self.latest_configuration_request
    }

    fn prepare_analyzer_options(
        &mut self,
        request: ConfigurationRequestId,
        options: AnalysisOptions,
    ) -> Option<AnalyzerOptionsPreparation> {
        if !self.is_analyzer_configuration_request_current(request) {
            return None;
        }
        let change = analyzer_configuration_change(self.analyzer.options(), &options);
        if change.affects_snapshots() {
            Some(AnalyzerOptionsPreparation::RequiresSnapshotPreparation(
                Box::new(self.prepare_snapshot_configuration_for(request, options)),
            ))
        } else {
            let requests = if matches!(change, AnalyzerConfigurationChange::Unchanged) {
                Vec::new()
            } else {
                self.set_diagnostic_analyzer(
                    self.analyzer.with_diagnostic_policy(options.diagnostics),
                )
            };
            Some(AnalyzerOptionsPreparation::Applied(change, requests))
        }
    }

    fn prepare_snapshot_configuration_for(
        &self,
        request: ConfigurationRequestId,
        next_options: AnalysisOptions,
    ) -> SnapshotConfigurationPlan {
        let source_limit_changed =
            self.analyzer.options().max_source_bytes() != next_options.max_source_bytes();
        let documents = source_limit_changed.then(|| {
            let max_source_bytes = next_options.max_source_bytes();
            self.documents
                .iter()
                .map(|(uri, record)| {
                    let oversized_source = record.document.text().and_then(|text| {
                        max_source_bytes
                            .filter(|limit| text.len() > *limit)
                            .map(|_| Arc::clone(text))
                    });
                    SourceLimitDocumentSnapshot {
                        uri: uri.clone(),
                        oversized_source,
                    }
                })
                .collect()
        });
        SnapshotConfigurationPlan {
            cancellation: self.session_cancellation.child(),
            request,
            expected_configuration_revision: self.configuration_revision,
            expected_documents_revision: source_limit_changed.then_some(self.documents_revision),
            base_analyzer: self.analyzer.clone(),
            next_options,
            documents,
        }
    }

    fn commit_snapshot_configuration(
        &mut self,
        batch: SnapshotConfigurationBatch,
    ) -> Option<(
        AnalyzerConfigurationChange,
        Vec<DiagnosticReprojectionRequest>,
    )> {
        if !self.is_analyzer_configuration_request_current(batch.request)
            || self.configuration_revision != batch.expected_configuration_revision
            || batch
                .expected_documents_revision
                .is_some_and(|revision| self.documents_revision != revision)
        {
            return None;
        }

        let change = analyzer_configuration_change(self.analyzer.options(), &batch.next_options);
        if !change.affects_snapshots() {
            return None;
        }

        self.replace_analyzer(batch.analyzer, batch.oversized_spans.as_ref());
        Some((change, Vec::new()))
    }

    fn set_diagnostic_analyzer(
        &mut self,
        analyzer: Analyzer,
    ) -> Vec<DiagnosticReprojectionRequest> {
        self.diagnostic_reprojection_cancellation.cancel();
        self.diagnostic_reprojection_cancellation = self.session_cancellation.child();
        self.analyzer = analyzer;
        self.advance_configuration_revision();
        self.advance_diagnostic_generation();
        self.analysis_executor.invalidate_all();
        self.analysis_generations
            .iter()
            .filter_map(|(uri, cached)| {
                self.documents.get(uri).map(|record| {
                    DiagnosticReprojectionRequest::new(
                        self.analyzer.clone(),
                        self.diagnostic_reprojection_cancellation.clone(),
                        self.diagnostic_generation,
                        uri.clone(),
                        self.analysis_executor.generation_for(uri),
                        record.epoch,
                        cached.snapshot_generation,
                        cached.diagnostic_generation,
                        Arc::clone(&cached.context),
                    )
                })
            })
            .collect()
    }

    pub(super) fn commit_diagnostic_reprojection_context(
        &mut self,
        projection: &DiagnosticReprojectionLease,
    ) -> Option<SnapshotContext> {
        let uri = projection.uri();
        let record = self.documents.get(uri)?;
        if record.epoch != projection.document_epoch()
            || self.snapshot_generation != projection.snapshot_generation()
            || self.diagnostic_generation != projection.target_diagnostic_generation()
            || !self
                .analysis_executor
                .is_generation_current(uri, projection.analysis_job_generation())
        {
            return None;
        }

        if let Some(cached) = self.analysis_generations.peek(uri)
            && cached.document_epoch == projection.document_epoch()
            && cached.snapshot_generation == projection.snapshot_generation()
            && cached.diagnostic_generation == projection.target_diagnostic_generation()
        {
            return Some(Self::cached_snapshot_context(cached));
        }

        let Some(cached) = self.analysis_generations.peek(uri) else {
            return Some(Self::reprojected_snapshot_context(projection));
        };
        if cached.document_epoch != projection.document_epoch()
            || cached.snapshot_generation != projection.snapshot_generation()
            || cached.diagnostic_generation != projection.source_diagnostic_generation()
            || !Arc::ptr_eq(&cached.context, projection.original())
        {
            return None;
        }

        let context = Self::reprojected_snapshot_context(projection);
        let weight = cached_analysis_weight(uri, projection.projected());
        self.analysis_generations
            .replace_batch_preserving_recency(vec![WeightedReplacement {
                key: uri.clone(),
                value: CachedAnalysisGeneration {
                    context: Arc::clone(projection.projected()),
                    document_epoch: projection.document_epoch(),
                    snapshot_generation: projection.snapshot_generation(),
                    diagnostic_generation: projection.target_diagnostic_generation(),
                },
                weight,
            }]);
        Some(context)
    }

    fn reprojected_snapshot_context(projection: &DiagnosticReprojectionLease) -> SnapshotContext {
        SnapshotContext::with_analysis(
            Arc::clone(&projection.projected().snapshot),
            Arc::clone(&projection.projected().payload),
            projection.snapshot_generation(),
            projection.target_diagnostic_generation(),
            projection.document_epoch(),
        )
    }

    fn discard_stale_analysis_generations(&mut self) {
        let generation = self.diagnostic_generation;
        self.analysis_generations
            .retain(|_, cached| cached.diagnostic_generation == generation);
    }

    fn replace_analyzer(
        &mut self,
        analyzer: Analyzer,
        oversized_spans: Option<&HashMap<Uri, DiagnosticSpan>>,
    ) {
        self.diagnostic_reprojection_cancellation.cancel();
        self.diagnostic_reprojection_cancellation = self.session_cancellation.child();
        self.analyzer = analyzer;
        if let Some(oversized_spans) = oversized_spans {
            self.reclassify_documents_for_current_limit(oversized_spans);
        }
        self.advance_configuration_revision();
        self.advance_snapshot_generation();
        self.advance_diagnostic_generation();
        self.analysis_generations.clear();
        for record in self.documents.values_mut() {
            record.semantic_tokens_state = None;
        }
        self.analysis_executor.invalidate_all();
    }

    fn advance_snapshot_generation(&mut self) {
        self.snapshot_generation = SnapshotGeneration(
            self.snapshot_generation
                .0
                .checked_add(1)
                .expect("snapshot generation exhausted"),
        );
    }

    fn advance_diagnostic_generation(&mut self) {
        self.diagnostic_generation = DiagnosticGeneration(
            self.diagnostic_generation
                .0
                .checked_add(1)
                .expect("diagnostic generation exhausted"),
        );
        for record in self.documents.values_mut() {
            record.diagnostic_state = None;
        }
    }

    fn advance_configuration_revision(&mut self) {
        self.configuration_revision = ConfigurationRevision(
            self.configuration_revision
                .0
                .checked_add(1)
                .expect("configuration revision exhausted"),
        );
    }

    fn advance_documents_revision(&mut self) {
        self.documents_revision = DocumentsRevision(
            self.documents_revision
                .0
                .checked_add(1)
                .expect("documents revision exhausted"),
        );
    }

    fn next_document_epoch(&mut self) -> DocumentEpoch {
        self.next_document_epoch = self
            .next_document_epoch
            .checked_add(1)
            .expect("document epoch exhausted");
        DocumentEpoch(self.next_document_epoch)
    }

    pub(super) fn diagnostic_context(&self, uri: &Uri) -> Option<DiagnosticContext> {
        self.documents.get(uri).map(|record| {
            DiagnosticContext::new(
                record.document.clone(),
                self.diagnostic_generation,
                record.epoch,
            )
        })
    }

    pub(super) fn is_diagnostic_context_current(&self, context: &DiagnosticContext) -> bool {
        self.diagnostic_generation == context.generation
            && self.is_document_epoch_current(&context.document.uri, context.document_epoch)
    }

    fn capture_open_document(&self, uri: Uri) -> OpenDocumentTicket {
        let expected_uri_revision = self.open_document_tracker.capture(&uri);
        OpenDocumentTicket {
            expected_document_epoch: self.documents.get(&uri).map(|record| record.epoch),
            uri,
            expected_uri_revision,
            expected_configuration_revision: self.configuration_revision,
            max_source_bytes: self.analyzer.options().max_source_bytes(),
            tracker: Arc::clone(&self.open_document_tracker),
        }
    }

    fn commit_open_document(
        &mut self,
        ticket: OpenDocumentTicket,
        version: i32,
        prepared: PreparedDocumentText,
        kind: DocumentKind,
    ) -> bool {
        if self.configuration_revision != ticket.expected_configuration_revision
            || !ticket
                .tracker
                .is_current(&ticket.uri, ticket.expected_uri_revision)
            || self.documents.get(&ticket.uri).map(|record| record.epoch)
                != ticket.expected_document_epoch
        {
            return false;
        }
        self.open_prepared_text(ticket.uri.clone(), version, prepared, kind);
        true
    }

    fn open_prepared_text(
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
                    span: prepared
                        .oversized_span
                        .expect("oversized prepared source must carry a diagnostic span"),
                },
            );
        }

        let document =
            StoredDocument::available(uri.clone(), version, kind, Arc::<str>::from(prepared.text));
        self.upsert_document(uri, document)
    }

    fn upsert_resource_limited(
        &mut self,
        uri: Uri,
        version: i32,
        kind: DocumentKind,
        resource_limit: DocumentResourceLimit,
    ) -> StoredDocument {
        let document = StoredDocument::resource_limited(uri.clone(), version, kind, resource_limit);
        self.upsert_document(uri, document)
    }

    fn upsert_sync_error(
        &mut self,
        uri: Uri,
        version: i32,
        kind: DocumentKind,
        sync_error: DocumentSyncError,
    ) -> StoredDocument {
        let document = StoredDocument::sync_error(uri.clone(), version, kind, sync_error);
        self.upsert_document(uri, document)
    }

    fn upsert_document(&mut self, uri: Uri, document: StoredDocument) -> StoredDocument {
        self.open_document_tracker.advance(&uri);
        self.analysis_executor.invalidate(&uri);
        self.analysis_generations.remove(&uri);
        let epoch = self.next_document_epoch();
        match self.documents.entry(uri) {
            std::collections::hash_map::Entry::Occupied(mut entry) => {
                let record = entry.get_mut();
                record.document = document.clone();
                record.epoch = epoch;
                record.diagnostic_state = None;
            }
            std::collections::hash_map::Entry::Vacant(entry) => {
                entry.insert(DocumentRecord {
                    document: document.clone(),
                    epoch,
                    diagnostic_state: None,
                    semantic_tokens_state: None,
                });
            }
        }
        self.advance_documents_revision();
        document
    }

    fn reclassify_documents_for_current_limit(
        &mut self,
        oversized_spans: &HashMap<Uri, DiagnosticSpan>,
    ) {
        let current_limit = self.analyzer.options().max_source_bytes();
        for (uri, record) in &mut self.documents {
            let next_source = match &record.document.source {
                DocumentSource::ResourceLimited(limit) => {
                    let source_len = limit.source_len;
                    let previous_max_source_bytes = limit.max_source_bytes;
                    let span = limit.span;
                    match current_limit {
                        Some(max_source_bytes) if source_len > max_source_bytes => {
                            DocumentSource::ResourceLimited(DocumentResourceLimit {
                                source_len,
                                max_source_bytes,
                                span,
                            })
                        }
                        _ => DocumentSource::Discarded(DocumentDiscardedSource {
                            source_len,
                            previous_max_source_bytes,
                            span,
                        }),
                    }
                }
                DocumentSource::Discarded(discarded) => match current_limit {
                    Some(max_source_bytes) if discarded.source_len > max_source_bytes => {
                        DocumentSource::ResourceLimited(DocumentResourceLimit {
                            source_len: discarded.source_len,
                            max_source_bytes,
                            span: discarded.span,
                        })
                    }
                    _ => DocumentSource::Discarded(*discarded),
                },
                DocumentSource::Available(text) => {
                    let Some(max_source_bytes) = current_limit else {
                        continue;
                    };
                    if text.len() <= max_source_bytes {
                        continue;
                    }
                    let span = *oversized_spans.get(uri).expect(
                        "source-limit projection must cover every newly oversized document",
                    );
                    DocumentSource::ResourceLimited(DocumentResourceLimit {
                        source_len: text.len(),
                        max_source_bytes,
                        span,
                    })
                }
                DocumentSource::SyncError(_) => continue,
            };
            record.document.source = next_source;
        }
    }

    fn capture_text_changes(
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
            expected_configuration_revision: self.configuration_revision,
            max_source_bytes: self.analyzer.options().max_source_bytes(),
            source: current.captured_source(),
            changes,
            cancellation: self.session_cancellation.child(),
        }))
    }

    fn commit_prepared_text_changes(
        &mut self,
        prepared: PreparedTextChanges,
    ) -> TextDocumentUpdate {
        let Some(record) = self.documents.get(&prepared.uri) else {
            return TextDocumentUpdate::MissingDocument;
        };
        if record.epoch != prepared.expected_epoch
            || self.configuration_revision != prepared.expected_configuration_revision
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

    pub(super) fn get(&self, uri: &Uri) -> Option<&StoredDocument> {
        self.documents.get(uri).map(|record| &record.document)
    }

    #[cfg(test)]
    fn analyzer_options(&self) -> &AnalysisOptions {
        self.analyzer.options()
    }

    #[cfg(test)]
    fn analyzer_environment_identity(&self) -> &merman_analysis::AnalysisEnvironmentIdentity {
        self.analyzer.environment_identity()
    }

    pub(super) fn cached_snapshot_context_for_uri(&mut self, uri: &Uri) -> Option<SnapshotContext> {
        let document_epoch = self.documents.get(uri)?.epoch;
        let snapshot_generation = self.snapshot_generation;
        if let Some(cached) = self.analysis_generations.get_if(uri, |cached| {
            cached.document_epoch == document_epoch
                && cached.snapshot_generation == snapshot_generation
        }) {
            return Some(Self::cached_snapshot_context(cached));
        }
        self.analysis_generations.remove(uri);
        None
    }

    #[cfg(test)]
    pub(super) fn cached_snapshot_for_probe(&self, uri: &Uri) -> Option<Arc<DocumentSnapshot>> {
        let document_epoch = self.documents.get(uri)?.epoch;
        let cached = self.analysis_generations.peek(uri)?;
        (cached.document_epoch == document_epoch
            && cached.snapshot_generation == self.snapshot_generation)
            .then(|| Arc::clone(&cached.context.snapshot))
    }

    pub(super) fn snapshot_build_request(&self, uri: &Uri) -> Option<AnalysisBuildRequest> {
        let record = self.documents.get(uri)?;
        if record.document.has_unavailable_source() || self.has_snapshot(uri) {
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
        let request = AnalysisBuildRequest::new(
            key,
            Arc::clone(record.document.text()?),
            record.document.kind,
            self.analyzer.clone(),
        );
        #[cfg(test)]
        let request = match &self.analysis_test_gate {
            Some(gate) => request.with_test_gate(Arc::clone(gate)),
            None => request,
        };
        Some(request)
    }

    pub(super) fn insert_built_analysis(
        &mut self,
        request: &AnalysisBuildRequest,
        analysis: Arc<DocumentAnalysisContext>,
    ) -> Option<SnapshotContext> {
        if self.snapshot_generation != request.snapshot_generation()
            || self.diagnostic_generation != request.diagnostic_generation()
            || !self.is_document_epoch_current(request.uri(), request.document_epoch())
            || !self
                .analysis_executor
                .is_generation_current(request.uri(), request.analysis_job_generation())
        {
            return None;
        }

        let weight = cached_analysis_weight(request.uri(), &analysis);
        self.analysis_generations.insert(
            request.uri().clone(),
            CachedAnalysisGeneration {
                context: Arc::clone(&analysis),
                document_epoch: request.document_epoch(),
                snapshot_generation: request.snapshot_generation(),
                diagnostic_generation: request.diagnostic_generation(),
            },
            weight,
        );
        Some(SnapshotContext::with_analysis(
            Arc::clone(&analysis.snapshot),
            Arc::clone(&analysis.payload),
            request.snapshot_generation(),
            request.diagnostic_generation(),
            request.document_epoch(),
        ))
    }

    pub(super) fn is_snapshot_context_current(&self, context: &SnapshotContext) -> bool {
        self.snapshot_generation == context.generation
            && self.is_document_epoch_current(context.snapshot.uri(), context.document_epoch)
    }

    pub(super) fn is_analysis_context_current(&self, context: &SnapshotContext) -> bool {
        self.is_snapshot_context_current(context)
            && context.diagnostic_generation() == self.diagnostic_generation
    }

    pub(super) fn diagnostic_contexts_are_current(
        &self,
        diagnostic: &DiagnosticContext,
        analysis: Option<&SnapshotContext>,
    ) -> bool {
        self.is_diagnostic_context_current(diagnostic)
            && analysis.is_none_or(|context| self.is_analysis_context_current(context))
    }

    fn is_document_epoch_current(&self, uri: &Uri, document_epoch: DocumentEpoch) -> bool {
        self.documents
            .get(uri)
            .is_some_and(|record| record.epoch == document_epoch)
    }

    pub(super) fn has_snapshot(&self, uri: &Uri) -> bool {
        let Some(record) = self.documents.get(uri) else {
            return false;
        };
        self.analysis_generations.peek(uri).is_some_and(|cached| {
            cached.document_epoch == record.epoch
                && cached.snapshot_generation == self.snapshot_generation
        })
    }

    pub(super) fn has_analysis_payload(&self, uri: &Uri) -> bool {
        self.analysis_generations.peek(uri).is_some_and(|cached| {
            cached.diagnostic_generation == self.diagnostic_generation
                && cached.snapshot_generation == self.snapshot_generation
                && self.is_document_epoch_current(uri, cached.document_epoch)
        })
    }

    pub(super) fn diagnostic_reprojection_request(
        &self,
        uri: &Uri,
    ) -> Option<DiagnosticReprojectionRequest> {
        let cached = self.analysis_generations.peek(uri)?;
        if cached.diagnostic_generation == self.diagnostic_generation {
            return None;
        }
        let record = self.documents.get(uri)?;
        if cached.document_epoch != record.epoch
            || cached.snapshot_generation != self.snapshot_generation
        {
            return None;
        }
        Some(DiagnosticReprojectionRequest::new(
            self.analyzer.clone(),
            self.diagnostic_reprojection_cancellation.clone(),
            self.diagnostic_generation,
            uri.clone(),
            self.analysis_executor.generation_for(uri),
            record.epoch,
            cached.snapshot_generation,
            cached.diagnostic_generation,
            Arc::clone(&cached.context),
        ))
    }

    #[cfg(test)]
    fn cached_analysis_generation(&self, uri: &Uri) -> Option<&Arc<DocumentAnalysisContext>> {
        self.analysis_generations
            .peek(uri)
            .map(|cached| &cached.context)
    }

    pub(super) fn analysis_executor(&self) -> AnalysisExecutor {
        self.analysis_executor.clone()
    }

    pub(super) fn remove(&mut self, uri: &Uri) {
        self.analysis_executor.forget(uri);
        if self.documents.remove(uri).is_some() {
            self.open_document_tracker.advance(uri);
            self.advance_documents_revision();
        }
        self.analysis_generations.remove(uri);
    }

    fn diagnostic_contexts(&self) -> Vec<DiagnosticContext> {
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

    fn cached_snapshot_context(cached: &CachedAnalysisGeneration) -> SnapshotContext {
        SnapshotContext::with_analysis(
            Arc::clone(&cached.context.snapshot),
            Arc::clone(&cached.context.payload),
            cached.snapshot_generation,
            cached.diagnostic_generation,
            cached.document_epoch,
        )
    }

    #[cfg(test)]
    pub(super) fn semantic_tokens_state(&self, uri: &Uri) -> Option<&SemanticTokensState> {
        self.documents
            .get(uri)
            .and_then(|record| record.semantic_tokens_state.as_ref())
            .map(|stored| stored.state.as_ref())
    }

    pub(super) fn semantic_tokens_state_for_delta(
        &self,
        uri: &Uri,
        previous_result_id: &str,
    ) -> Option<Arc<SemanticTokensState>> {
        self.documents
            .get(uri)
            .and_then(|record| record.semantic_tokens_state.as_ref())
            .and_then(|stored| {
                (stored.snapshot_generation == self.snapshot_generation
                    && stored.state.result_id.as_deref() == Some(previous_result_id))
                .then(|| Arc::clone(&stored.state))
            })
    }

    pub(super) fn set_semantic_tokens_state_if_current(
        &mut self,
        context: &SnapshotContext,
        state: SemanticTokensState,
    ) -> bool {
        if !self.is_snapshot_context_current(context) {
            return false;
        }

        let Some(record) = self.documents.get_mut(context.snapshot.uri()) else {
            return false;
        };
        record.semantic_tokens_state = Some(StoredSemanticTokensState {
            snapshot_generation: context.generation,
            state: Arc::new(state),
        });
        true
    }

    pub(super) fn diagnostic_state(&self, uri: &Uri) -> Option<DocumentDiagnosticState> {
        self.documents.get(uri).and_then(|record| {
            record.diagnostic_state.as_ref().and_then(|stored| {
                (stored.generation == self.diagnostic_generation
                    && stored.document_epoch == record.epoch)
                    .then(|| stored.state.clone())
            })
        })
    }

    pub(super) fn set_diagnostic_state_if_current(
        &mut self,
        context: &DiagnosticContext,
        state: DocumentDiagnosticState,
    ) -> bool {
        if !self.is_diagnostic_context_current(context) {
            return false;
        }

        let Some(record) = self.documents.get_mut(&context.document.uri) else {
            return false;
        };
        record.diagnostic_state = Some(StoredDiagnosticState {
            generation: context.generation,
            document_epoch: context.document_epoch,
            state,
        });
        true
    }
}

#[derive(Debug, Clone)]
struct DocumentRecord {
    document: StoredDocument,
    epoch: DocumentEpoch,
    diagnostic_state: Option<StoredDiagnosticState>,
    semantic_tokens_state: Option<StoredSemanticTokensState>,
}

#[derive(Debug, Clone)]
struct StoredSemanticTokensState {
    snapshot_generation: SnapshotGeneration,
    state: Arc<SemanticTokensState>,
}

#[derive(Debug, Clone)]
struct StoredDiagnosticState {
    generation: DiagnosticGeneration,
    document_epoch: DocumentEpoch,
    state: DocumentDiagnosticState,
}

#[derive(Debug, Clone)]
pub(crate) struct DocumentDiagnosticState {
    pub result_id: String,
    pub diagnostics: Vec<Diagnostic>,
}

#[derive(Debug, Clone)]
pub(crate) struct DiagnosticContext {
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
pub(crate) enum AnalyzerConfigurationChange {
    Unchanged,
    DiagnosticsOnly,
    SnapshotAffecting,
}

impl AnalyzerConfigurationChange {
    pub(crate) fn affects_diagnostics(self) -> bool {
        !matches!(self, Self::Unchanged)
    }

    pub(crate) fn affects_snapshots(self) -> bool {
        matches!(self, Self::SnapshotAffecting)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ConfigurationUpdateOutcome {
    Unchanged,
    Applied(AnalyzerConfigurationChange),
    Superseded,
    Cancelled,
    Failed,
}

impl ConfigurationUpdateOutcome {
    fn applied(change: AnalyzerConfigurationChange) -> Self {
        match change {
            AnalyzerConfigurationChange::Unchanged => Self::Unchanged,
            change => Self::Applied(change),
        }
    }

    pub(crate) fn affects_diagnostics(self) -> bool {
        matches!(self, Self::Applied(change) if change.affects_diagnostics())
    }

    pub(crate) fn affects_snapshots(self) -> bool {
        matches!(self, Self::Applied(change) if change.affects_snapshots())
    }

    pub(crate) fn failed(self) -> bool {
        matches!(self, Self::Failed)
    }

    pub(crate) fn accepted(self) -> bool {
        matches!(self, Self::Unchanged | Self::Applied(_))
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

struct AnalyzerReplacement {
    outcome: ConfigurationUpdateOutcome,
    reprojections: Vec<DiagnosticReprojectionRequest>,
}

impl AnalyzerReplacement {
    fn applied(
        change: AnalyzerConfigurationChange,
        reprojections: Vec<DiagnosticReprojectionRequest>,
    ) -> Self {
        Self {
            outcome: ConfigurationUpdateOutcome::applied(change),
            reprojections,
        }
    }

    fn terminal(outcome: ConfigurationUpdateOutcome) -> Self {
        Self {
            outcome,
            reprojections: Vec::new(),
        }
    }
}

impl LanguageSession {
    pub(crate) async fn open_document(
        &self,
        uri: Uri,
        version: i32,
        text: String,
        kind: DocumentKind,
    ) -> bool {
        let ticket = self
            .inner
            .state
            .lock()
            .await
            .capture_open_document(uri.clone());
        let max_source_bytes = ticket.max_source_bytes;
        let cancellation = self.inner.cancellation.child();
        let prepared = match tokio::task::spawn_blocking(move || {
            PreparedDocumentText::new_cancellable(text, max_source_bytes, &cancellation)
        })
        .await
        {
            Ok(Ok(prepared)) => prepared,
            Ok(Err(_)) => return false,
            Err(error) => {
                tracing::error!(%error, uri = %uri.as_str(), "document open preparation worker failed");
                return false;
            }
        };
        self.inner
            .state
            .lock()
            .await
            .commit_open_document(ticket, version, prepared, kind)
    }

    pub(crate) async fn change_document(
        &self,
        uri: Uri,
        version: i32,
        changes: Vec<TextDocumentContentChangeEvent>,
    ) -> Option<TextDocumentUpdate> {
        let preparation =
            self.inner
                .state
                .lock()
                .await
                .capture_text_changes(uri.clone(), version, changes);
        match preparation {
            TextChangePreparation::Immediate(update) => Some(update),
            TextChangePreparation::Prepare(plan) => {
                let prepared = match tokio::task::spawn_blocking(move || plan.prepare()).await {
                    Ok(Ok(prepared)) => prepared,
                    Ok(Err(_)) => return None,
                    Err(error) => {
                        tracing::error!(
                            %error,
                            uri = %uri.as_str(),
                            "document change preparation worker failed"
                        );
                        return None;
                    }
                };
                Some(
                    self.inner
                        .state
                        .lock()
                        .await
                        .commit_prepared_text_changes(prepared),
                )
            }
        }
    }

    pub(crate) async fn close_document(&self, uri: &Uri) {
        self.inner.state.lock().await.remove(uri);
    }

    pub(crate) async fn update_configuration(
        &self,
        options: AnalysisOptions,
    ) -> ConfigurationUpdateOutcome {
        let replacement = self.prepare_analyzer_replacement(options).await;
        self.finish_analyzer_replacement(replacement).await
    }

    async fn prepare_analyzer_replacement(&self, options: AnalysisOptions) -> AnalyzerReplacement {
        let request = self
            .inner
            .state
            .lock()
            .await
            .begin_analyzer_configuration_request();
        let (change, reprojections) = loop {
            let preparation = self
                .inner
                .state
                .lock()
                .await
                .prepare_analyzer_options(request, options.clone());
            let Some(preparation) = preparation else {
                return AnalyzerReplacement::terminal(ConfigurationUpdateOutcome::Superseded);
            };
            match preparation {
                AnalyzerOptionsPreparation::Applied(change, reprojections) => {
                    break (change, reprojections);
                }
                AnalyzerOptionsPreparation::RequiresSnapshotPreparation(plan) => {
                    let batch = match tokio::task::spawn_blocking(move || plan.prepare()).await {
                        Ok(Ok(batch)) => batch,
                        Ok(Err(_)) => {
                            return AnalyzerReplacement::terminal(
                                ConfigurationUpdateOutcome::Cancelled,
                            );
                        }
                        Err(error) => {
                            tracing::error!(%error, "snapshot configuration preparation worker failed");
                            return AnalyzerReplacement::terminal(
                                ConfigurationUpdateOutcome::Failed,
                            );
                        }
                    };
                    let mut state = self.inner.state.lock().await;
                    if let Some(applied) = state.commit_snapshot_configuration(batch) {
                        break applied;
                    }
                    if !state.is_analyzer_configuration_request_current(request) {
                        return AnalyzerReplacement::terminal(
                            ConfigurationUpdateOutcome::Superseded,
                        );
                    }
                }
            }
        };
        AnalyzerReplacement::applied(change, reprojections)
    }

    async fn finish_analyzer_replacement(
        &self,
        replacement: AnalyzerReplacement,
    ) -> ConfigurationUpdateOutcome {
        let AnalyzerReplacement {
            outcome,
            mut reprojections,
        } = replacement;
        if reprojections.is_empty() {
            return outcome;
        }
        reprojections.sort_by(|left, right| left.uri().cmp(right.uri()));
        let mut pending = stream::iter(reprojections)
            .map(|request| {
                let executor = self.inner.analysis_executor.clone();
                async move {
                    let uri = request.uri().clone();
                    (
                        uri,
                        executor.execute_diagnostic_reprojection(&request).await,
                    )
                }
            })
            .buffered(LSP_ANALYSIS_IN_FLIGHT_LIMIT);

        let mut worker_failed = false;
        while let Some((uri, projection)) = pending.next().await {
            match projection {
                Ok(projection) => {
                    self.inner
                        .state
                        .lock()
                        .await
                        .commit_diagnostic_reprojection_context(&projection);
                }
                Err(error) if error.is_stale() => {}
                Err(error) => {
                    worker_failed = true;
                    tracing::error!(%error, uri = %uri.as_str(), "diagnostic reprojection worker failed");
                }
            }
        }
        if worker_failed {
            self.inner
                .state
                .lock()
                .await
                .discard_stale_analysis_generations();
        }
        outcome
    }

    pub(crate) async fn diagnostic_context(&self, uri: &Uri) -> Option<DiagnosticContext> {
        self.inner.state.lock().await.diagnostic_context(uri)
    }

    pub(crate) async fn diagnostic_contexts(&self) -> Vec<DiagnosticContext> {
        self.inner.state.lock().await.diagnostic_contexts()
    }
}

#[cfg(test)]
#[path = "documents_tests.rs"]
mod tests;
