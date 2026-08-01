use super::LanguageSession;
use crate::session::analysis::executor::{AnalysisExecutor, DiagnosticReprojectionLease};
#[cfg(test)]
use crate::session::analysis::request::TestAnalysisGate;
use crate::session::analysis::request::{
    AnalysisBuildKey, AnalysisBuildRequest, AnalysisJobGeneration, DiagnosticProjectionOrigin,
    DiagnosticReprojectionKey, DiagnosticReprojectionRequest,
};
#[cfg(test)]
use crate::session::cache::WeightedCacheStatistics;
use crate::session::cache::{WeightedLru, WeightedReplacement, conservative_weighted_entry_bytes};
use crate::snapshot::{
    DiagnosticGeneration, DocumentAnalysisContext, DocumentEpoch, DocumentSnapshot,
    SnapshotContext, SnapshotGeneration,
};
use merman_analysis::{
    AnalysisCancellationToken, AnalysisCancelled, AnalysisOptions, AnalysisRejection,
    AnalysisResourceLimit, AnalysisResourceLimits, Analyzer, DiagnosticSpan, SourceDescriptor,
    source_descriptor_for_kind,
};
use merman_editor_core::DocumentKind;
use ropey::{Rope, RopeSlice};
use std::collections::HashMap;
use std::sync::{Arc, Weak};
use tower_lsp_server::ls_types::{
    Diagnostic, Position, Range, SemanticToken, TextDocumentContentChangeEvent, Uri,
};

mod analysis_state;
mod configuration;

pub(crate) const DEFAULT_LSP_MAX_SOURCE_BYTES: usize = 4 * 1024 * 1024;
pub(crate) const DEFAULT_LSP_MAX_DOCUMENT_DIAGRAMS: usize = 256;
pub(crate) const DEFAULT_LSP_ANALYSIS_CACHE_BUDGET_BYTES: usize = 64 * 1024 * 1024;

pub(crate) fn default_lsp_analysis_options() -> AnalysisOptions {
    AnalysisOptions::default()
        .with_max_source_bytes(Some(DEFAULT_LSP_MAX_SOURCE_BYTES))
        .with_max_document_diagrams(Some(DEFAULT_LSP_MAX_DOCUMENT_DIAGRAMS))
}

pub(crate) fn analysis_options_with_lsp_resource_defaults(
    mut options: AnalysisOptions,
) -> AnalysisOptions {
    if options.max_source_bytes().is_none() {
        options = options.with_max_source_bytes(Some(DEFAULT_LSP_MAX_SOURCE_BYTES));
    }
    if options.max_document_diagrams().is_none() {
        options = options.with_max_document_diagrams(Some(DEFAULT_LSP_MAX_DOCUMENT_DIAGRAMS));
    }
    options
}

fn document_source_descriptor(uri: &Uri, kind: DocumentKind) -> SourceDescriptor {
    source_descriptor_for_kind(Some(uri.as_str()), kind.source_kind())
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
}

#[derive(Debug, Clone)]
pub(super) struct SnapshotLease {
    pub(super) snapshot: Arc<DocumentSnapshot>,
    snapshot_generation: SnapshotGeneration,
    document_epoch: DocumentEpoch,
    analysis_job_generation: AnalysisJobGeneration,
}

impl SnapshotLease {
    fn new(
        snapshot: Arc<DocumentSnapshot>,
        snapshot_generation: SnapshotGeneration,
        document_epoch: DocumentEpoch,
        analysis_job_generation: AnalysisJobGeneration,
    ) -> Self {
        Self {
            snapshot,
            snapshot_generation,
            document_epoch,
            analysis_job_generation,
        }
    }
}

#[derive(Debug, Clone)]
struct WeakDocumentSnapshot {
    snapshot: Weak<DocumentSnapshot>,
    snapshot_generation: SnapshotGeneration,
    document_epoch: DocumentEpoch,
    analysis_job_generation: AnalysisJobGeneration,
}

pub(super) enum DiagnosticProjectionPreparation {
    Ready(SnapshotContext),
    Project(DiagnosticReprojectionRequest),
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
    documents: Option<Vec<ResourceDocumentSnapshot>>,
}

#[derive(Debug)]
struct ResourceDocumentSnapshot {
    uri: Uri,
    source: SourceDescriptor,
    text: Arc<str>,
}

#[derive(Debug)]
struct SnapshotConfigurationBatch {
    request: ConfigurationRequestId,
    expected_configuration_revision: ConfigurationRevision,
    expected_documents_revision: Option<DocumentsRevision>,
    next_options: AnalysisOptions,
    analyzer: Analyzer,
    resource_rejections: Option<HashMap<Uri, Option<AnalysisRejection>>>,
}

#[derive(Debug)]
enum AnalyzerOptionsPreparation {
    Applied(AnalyzerConfigurationChange),
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
    AnalysisRejected {
        text: Arc<str>,
        rejection: AnalysisRejection,
    },
    ResourceLimited(DocumentResourceLimit),
    Discarded(DocumentDiscardedSource),
    SyncError(DocumentSyncError),
}

#[derive(Debug)]
struct PreparedDocumentText {
    text: String,
    rejection: Option<AnalysisRejection>,
}

#[derive(Debug)]
struct OpenDocumentTicket {
    uri: Uri,
    expected_document_epoch: Option<DocumentEpoch>,
    expected_uri_revision: u64,
    expected_configuration_revision: ConfigurationRevision,
    resource_limits: AnalysisResourceLimits,
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
        resource_limits: AnalysisResourceLimits,
        source: &SourceDescriptor,
        cancellation: &AnalysisCancellationToken,
    ) -> Result<Self, AnalysisCancelled> {
        cancellation.checkpoint()?;
        let rejection =
            resource_limits.preflight_document_cancellable(&text, source, cancellation)?;
        Ok(Self { text, rejection })
    }
}

fn document_source_from_rejection(text: Arc<str>, rejection: AnalysisRejection) -> DocumentSource {
    match rejection.resource_limit() {
        AnalysisResourceLimit::SourceBytes {
            source_len,
            max_source_bytes,
        } => {
            let span = rejection
                .payload()
                .diagnostics
                .first()
                .and_then(|diagnostic| diagnostic.span)
                .expect("source-byte rejection must carry its canonical diagnostic span");
            DocumentSource::ResourceLimited(DocumentResourceLimit {
                source_len,
                max_source_bytes,
                span,
            })
        }
        _ => DocumentSource::AnalysisRejected { text, rejection },
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
            resource_limits,
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
                resource_limits,
                &document_source_descriptor(&uri, kind),
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
    resource_limits: AnalysisResourceLimits,
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

    #[cfg(test)]
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
    pub(crate) fn analysis_rejected(
        uri: Uri,
        version: i32,
        kind: DocumentKind,
        text: Arc<str>,
        rejection: AnalysisRejection,
    ) -> Self {
        Self {
            uri,
            version,
            kind,
            source: DocumentSource::AnalysisRejected { text, rejection },
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

    pub(crate) fn retained_text(&self) -> Option<&Arc<str>> {
        match &self.source {
            DocumentSource::Available(text) | DocumentSource::AnalysisRejected { text, .. } => {
                Some(text)
            }
            _ => None,
        }
    }

    #[cfg(test)]
    pub(crate) fn text(&self) -> Option<&Arc<str>> {
        self.retained_text()
    }

    fn analysis_text(&self) -> Option<&Arc<str>> {
        match &self.source {
            DocumentSource::Available(text) => Some(text),
            _ => None,
        }
    }

    pub(crate) fn analysis_rejection(&self) -> Option<&AnalysisRejection> {
        match &self.source {
            DocumentSource::AnalysisRejected { rejection, .. } => Some(rejection),
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

    pub fn is_analysis_unavailable(&self) -> bool {
        !matches!(&self.source, DocumentSource::Available(_))
    }

    fn captured_source(&self) -> CapturedDocumentSource {
        match &self.source {
            DocumentSource::Available(text) | DocumentSource::AnalysisRejected { text, .. } => {
                CapturedDocumentSource::Available(Arc::clone(text))
            }
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

    fn capture_open_document(&self, uri: Uri) -> OpenDocumentTicket {
        let expected_uri_revision = self.open_document_tracker.capture(&uri);
        OpenDocumentTicket {
            expected_document_epoch: self.documents.get(&uri).map(|record| record.epoch),
            uri,
            expected_uri_revision,
            expected_configuration_revision: self.configuration_revision,
            resource_limits: self.analyzer.options().resource_limits(),
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
        let text = Arc::<str>::from(prepared.text);
        let document = match prepared.rejection {
            Some(rejection) => StoredDocument {
                uri: uri.clone(),
                version,
                kind,
                source: document_source_from_rejection(text, rejection),
            },
            None => StoredDocument::available(uri.clone(), version, kind, text),
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
                record.snapshot = None;
                record.diagnostic_state = None;
            }
            std::collections::hash_map::Entry::Vacant(entry) => {
                entry.insert(DocumentRecord {
                    document: document.clone(),
                    epoch,
                    snapshot: None,
                    diagnostic_state: None,
                    semantic_tokens_state: None,
                });
            }
        }
        self.advance_documents_revision();
        document
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
            resource_limits: self.analyzer.options().resource_limits(),
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

    pub(super) fn remove(&mut self, uri: &Uri) {
        self.analysis_executor.forget(uri);
        if self.documents.remove(uri).is_some() {
            self.open_document_tracker.advance(uri);
            self.advance_documents_revision();
        }
        self.analysis_generations.remove(uri);
    }
}

#[derive(Debug, Clone)]
struct DocumentRecord {
    document: StoredDocument,
    epoch: DocumentEpoch,
    snapshot: Option<WeakDocumentSnapshot>,
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

impl LanguageSession {
    pub(crate) async fn open_document(
        &self,
        uri: Uri,
        version: i32,
        text: String,
        kind: DocumentKind,
    ) -> bool {
        let ticket = {
            let mut state = self.inner.state.lock().await;
            let Some(ticket) = self.commit_state_if_active(&mut state, |state| {
                state.capture_open_document(uri.clone())
            }) else {
                return false;
            };
            ticket
        };
        let resource_limits = ticket.resource_limits;
        let source = document_source_descriptor(&uri, kind);
        let cancellation = self.inner.cancellation.child();
        let prepared = match tokio::task::spawn_blocking(move || {
            PreparedDocumentText::new_cancellable(text, resource_limits, &source, &cancellation)
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
        let mut state = self.inner.state.lock().await;
        self.commit_state_if_active(&mut state, |state| {
            state.commit_open_document(ticket, version, prepared, kind)
        })
        .unwrap_or(false)
    }

    pub(crate) async fn change_document(
        &self,
        uri: Uri,
        version: i32,
        changes: Vec<TextDocumentContentChangeEvent>,
    ) -> Option<TextDocumentUpdate> {
        let preparation = {
            let mut state = self.inner.state.lock().await;
            self.commit_state_if_active(&mut state, |state| {
                state.capture_text_changes(uri.clone(), version, changes)
            })?
        };
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
                let mut state = self.inner.state.lock().await;
                self.commit_state_if_active(&mut state, |state| {
                    state.commit_prepared_text_changes(prepared)
                })
            }
        }
    }

    pub(crate) async fn close_document(&self, uri: &Uri) {
        let mut state = self.inner.state.lock().await;
        let _ = self.commit_state_if_active(&mut state, |state| state.remove(uri));
    }
}

#[cfg(test)]
#[path = "documents_tests.rs"]
mod tests;
