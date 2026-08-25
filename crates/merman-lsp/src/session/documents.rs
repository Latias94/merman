use super::LanguageSession;
use crate::session::analysis::acquisition::{AcquiredSnapshot, ProjectionDecision};
use crate::session::analysis::executor::{
    AnalysisExecutionLease, AnalysisExecutor, DiagnosticReprojectionLease,
};
#[cfg(test)]
use crate::session::analysis::request::TestAnalysisGate;
use crate::session::analysis::request::{
    AnalysisBuildKey, AnalysisBuildRequest, DiagnosticReprojectionRequest,
};
use crate::session::analysis_cache::{AnalysisCache, AnalysisCacheAuthority, AnalysisCacheStamp};
use crate::snapshot::{
    DiagnosticGeneration, DocumentAnalysisContext, DocumentEpoch, DocumentSnapshot,
    SnapshotContext, SnapshotGeneration,
};
use crate::syntax_highlighting::SyntaxDocumentState;
use merman_analysis::{
    AnalysisCancellationToken, AnalysisCancelled, AnalysisConfigChange, AnalysisConfigContract,
    AnalysisDiagnosticPolicy, AnalysisOptions, AnalysisRejection, AnalysisResourceLimit,
    AnalysisResourceLimits, Analyzer, DiagnosticSpan, SourceDescriptor, source_descriptor_for_kind,
};
use merman_editor_core::DocumentKind;
use ropey::{Rope, RopeSlice};
use std::collections::HashMap;
use std::sync::Arc;
use tower_lsp_server::ls_types::{
    Diagnostic, Position, Range, TextDocumentContentChangeEvent, Uri,
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
    analysis_cache: AnalysisCache,
    #[cfg(test)]
    analysis_test_gate: Option<Arc<TestAnalysisGate>>,
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
    Applied(AnalysisConfigChange),
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
    ResourceLimited(SourceLimitEvidence),
    Discarded(SourceLimitEvidence),
    SyncError,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct SourceLimitEvidence {
    source_len: usize,
    last_max_source_bytes: usize,
    span: DiagnosticSpan,
    incremental_sync_lost: bool,
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

fn prepare_document_source_cancellable(
    text: String,
    resource_limits: AnalysisResourceLimits,
    source: &SourceDescriptor,
    cancellation: &AnalysisCancellationToken,
) -> Result<DocumentSource, AnalysisCancelled> {
    cancellation.checkpoint()?;
    let rejection = resource_limits.preflight_document_cancellable(&text, source, cancellation)?;
    cancellation.checkpoint()?;
    Ok(DocumentSource::from_preflight(text, rejection))
}

fn prepare_syntax_document_cancellable(
    uri: &Uri,
    version: i32,
    kind: DocumentKind,
    source: &DocumentSource,
    previous: Option<&SyntaxDocumentState>,
    cancellation: &AnalysisCancellationToken,
) -> Result<Option<Arc<SyntaxDocumentState>>, AnalysisCancelled> {
    cancellation.checkpoint()?;
    let Some(text) = source.retained_text() else {
        return Ok(None);
    };
    let parsed = match previous {
        Some(previous) => previous.update(version, kind, Arc::clone(text), cancellation),
        None => SyntaxDocumentState::parse(version, kind, Arc::clone(text), cancellation),
    };
    cancellation.checkpoint()?;
    Ok(match parsed {
        Ok(document) => Some(Arc::new(document)),
        Err(error) => {
            tracing::error!(
                uri = uri.as_str(),
                version,
                %error,
                "Tree-sitter syntax document preparation failed"
            );
            None
        }
    })
}

fn prepare_document_content_cancellable(
    uri: &Uri,
    version: i32,
    kind: DocumentKind,
    text: String,
    resource_limits: AnalysisResourceLimits,
    previous: Option<&SyntaxDocumentState>,
    cancellation: &AnalysisCancellationToken,
) -> Result<PreparedDocumentContent, AnalysisCancelled> {
    let source_descriptor = document_source_descriptor(uri, kind);
    let source = prepare_document_source_cancellable(
        text,
        resource_limits,
        &source_descriptor,
        cancellation,
    )?;
    let syntax_document =
        prepare_syntax_document_cancellable(uri, version, kind, &source, previous, cancellation)?;
    Ok(PreparedDocumentContent {
        source,
        syntax_document,
    })
}

impl TextChangePlan {
    fn prepare(self) -> Result<PreparedDocumentChange, AnalysisCancelled> {
        let Self {
            uri,
            version,
            kind,
            expected_epoch,
            expected_configuration_revision,
            resource_limits,
            source,
            syntax_document,
            changes,
            cancellation,
        } = self;
        cancellation.checkpoint()?;
        let prepared_text = if let Some(current_text) = source.retained_text() {
            apply_available_text_changes(current_text, changes, &cancellation)?
        } else if let Some(recovery_start) =
            changes.iter().rposition(|change| change.range.is_none())
        {
            apply_changes_from_full_replacement(changes, recovery_start, &cancellation)?
        } else {
            cancellation.checkpoint()?;
            return Ok(PreparedDocumentChange {
                uri,
                version,
                kind,
                expected_epoch,
                expected_configuration_revision,
                content: PreparedDocumentContent {
                    source: source.with_incremental_sync_lost(),
                    syntax_document: None,
                },
            });
        };
        let content = match prepared_text {
            Ok(text) => prepare_document_content_cancellable(
                &uri,
                version,
                kind,
                text,
                resource_limits,
                syntax_document.as_deref(),
                &cancellation,
            )?,
            Err(()) => PreparedDocumentContent {
                source: DocumentSource::SyncError,
                syntax_document: None,
            },
        };
        cancellation.checkpoint()?;
        Ok(PreparedDocumentChange {
            uri,
            version,
            kind,
            expected_epoch,
            expected_configuration_revision,
            content,
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

#[derive(Debug)]
struct TextChangePlan {
    uri: Uri,
    version: i32,
    kind: DocumentKind,
    expected_epoch: DocumentEpoch,
    expected_configuration_revision: ConfigurationRevision,
    resource_limits: AnalysisResourceLimits,
    source: DocumentSource,
    syntax_document: Option<Arc<SyntaxDocumentState>>,
    changes: Vec<TextDocumentContentChangeEvent>,
    cancellation: AnalysisCancellationToken,
}

#[derive(Debug)]
struct PreparedDocumentChange {
    uri: Uri,
    version: i32,
    kind: DocumentKind,
    expected_epoch: DocumentEpoch,
    expected_configuration_revision: ConfigurationRevision,
    content: PreparedDocumentContent,
}

#[derive(Debug)]
struct PreparedDocumentContent {
    source: DocumentSource,
    syntax_document: Option<Arc<SyntaxDocumentState>>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum DocumentSyncLoss {
    InvalidIncrementalRange,
    SourceUnavailable {
        source_len: usize,
        last_max_source_bytes: usize,
    },
}

pub(crate) enum DocumentUnavailableDiagnostic<'a> {
    AnalysisRejected(&'a AnalysisRejection),
    ResourceLimited {
        source_len: usize,
        max_source_bytes: usize,
        span: DiagnosticSpan,
    },
    Discarded {
        source_len: usize,
        previous_max_source_bytes: usize,
        span: DiagnosticSpan,
    },
    SyncLost(DocumentSyncLoss),
}

impl DocumentSource {
    fn from_preflight(text: String, rejection: Option<AnalysisRejection>) -> Self {
        match rejection {
            None => Self::Available(Arc::from(text)),
            Some(rejection) => Self::source_limit_evidence(&rejection).map_or_else(
                || Self::AnalysisRejected {
                    text: Arc::from(text),
                    rejection,
                },
                Self::ResourceLimited,
            ),
        }
    }

    fn from_rejection(text: Arc<str>, rejection: AnalysisRejection) -> Self {
        Self::source_limit_evidence(&rejection).map_or_else(
            || Self::AnalysisRejected { text, rejection },
            Self::ResourceLimited,
        )
    }

    fn source_limit_evidence(rejection: &AnalysisRejection) -> Option<SourceLimitEvidence> {
        let AnalysisResourceLimit::SourceBytes {
            source_len,
            max_source_bytes,
        } = rejection.resource_limit()
        else {
            return None;
        };
        let span = rejection
            .payload()
            .diagnostics
            .first()
            .and_then(|diagnostic| diagnostic.span)
            .expect("source-byte rejection must carry its canonical diagnostic span");
        Some(SourceLimitEvidence {
            source_len,
            last_max_source_bytes: max_source_bytes,
            span,
            incremental_sync_lost: false,
        })
    }

    fn retained_text(&self) -> Option<&Arc<str>> {
        match self {
            Self::Available(text) | Self::AnalysisRejected { text, .. } => Some(text),
            Self::ResourceLimited(_) | Self::Discarded(_) | Self::SyncError => None,
        }
    }

    fn analysis_text(&self) -> Option<&Arc<str>> {
        match self {
            Self::Available(text) => Some(text),
            Self::AnalysisRejected { .. }
            | Self::ResourceLimited(_)
            | Self::Discarded(_)
            | Self::SyncError => None,
        }
    }

    fn with_incremental_sync_lost(mut self) -> Self {
        match &mut self {
            Self::ResourceLimited(evidence) | Self::Discarded(evidence) => {
                evidence.incremental_sync_lost = true;
            }
            Self::SyncError => {}
            Self::Available(_) | Self::AnalysisRejected { .. } => {
                debug_assert!(false, "retained sources apply ranged edits directly");
            }
        }
        self
    }

    fn unavailable_diagnostic(&self) -> Option<DocumentUnavailableDiagnostic<'_>> {
        match self {
            Self::Available(_) => None,
            Self::AnalysisRejected { rejection, .. } => {
                Some(DocumentUnavailableDiagnostic::AnalysisRejected(rejection))
            }
            Self::ResourceLimited(evidence) if evidence.incremental_sync_lost => Some(
                DocumentUnavailableDiagnostic::SyncLost(DocumentSyncLoss::SourceUnavailable {
                    source_len: evidence.source_len,
                    last_max_source_bytes: evidence.last_max_source_bytes,
                }),
            ),
            Self::ResourceLimited(evidence) => {
                Some(DocumentUnavailableDiagnostic::ResourceLimited {
                    source_len: evidence.source_len,
                    max_source_bytes: evidence.last_max_source_bytes,
                    span: evidence.span,
                })
            }
            Self::Discarded(evidence) if evidence.incremental_sync_lost => Some(
                DocumentUnavailableDiagnostic::SyncLost(DocumentSyncLoss::SourceUnavailable {
                    source_len: evidence.source_len,
                    last_max_source_bytes: evidence.last_max_source_bytes,
                }),
            ),
            Self::Discarded(evidence) => Some(DocumentUnavailableDiagnostic::Discarded {
                source_len: evidence.source_len,
                previous_max_source_bytes: evidence.last_max_source_bytes,
                span: evidence.span,
            }),
            Self::SyncError => Some(DocumentUnavailableDiagnostic::SyncLost(
                DocumentSyncLoss::InvalidIncrementalRange,
            )),
        }
    }

    fn reclassify(
        &self,
        max_source_bytes: Option<usize>,
        retained_rejection: Option<&AnalysisRejection>,
    ) -> Self {
        match self {
            Self::Available(text) | Self::AnalysisRejected { text, .. } => retained_rejection
                .map_or_else(
                    || Self::Available(Arc::clone(text)),
                    |rejection| Self::from_rejection(Arc::clone(text), rejection.clone()),
                ),
            Self::ResourceLimited(evidence) | Self::Discarded(evidence) => match max_source_bytes {
                Some(limit) if evidence.source_len > limit => {
                    Self::ResourceLimited(SourceLimitEvidence {
                        last_max_source_bytes: limit,
                        ..*evidence
                    })
                }
                _ => Self::Discarded(*evidence),
            },
            Self::SyncError => Self::SyncError,
        }
    }
}

impl StoredDocument {
    pub(crate) fn retained_text(&self) -> Option<&Arc<str>> {
        self.source.retained_text()
    }

    fn analysis_text(&self) -> Option<&Arc<str>> {
        self.source.analysis_text()
    }

    pub(crate) fn unavailable_diagnostic(&self) -> Option<DocumentUnavailableDiagnostic<'_>> {
        self.source.unavailable_diagnostic()
    }

    pub fn is_analysis_unavailable(&self) -> bool {
        !matches!(&self.source, DocumentSource::Available(_))
    }
}

#[derive(Debug, Clone)]
pub(crate) struct SemanticTokensState {
    pub result_id: String,
    pub packed: Vec<u32>,
}

#[derive(Debug, Clone)]
pub(crate) struct SyntaxDocumentSnapshot {
    pub(crate) uri: Uri,
    pub(crate) document: Arc<SyntaxDocumentState>,
    document_epoch: DocumentEpoch,
    cancellation: AnalysisCancellationToken,
}

impl SyntaxDocumentSnapshot {
    pub(crate) fn cancellation(&self) -> &AnalysisCancellationToken {
        &self.cancellation
    }
}

impl SemanticTokensState {
    pub fn new(result_id: String, packed: Vec<u32>) -> Self {
        Self { result_id, packed }
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
            analysis_cache: AnalysisCache::new(analysis_cache_budget),
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

    fn capture_open_document(&mut self, uri: Uri) -> OpenDocumentTicket {
        let expected_uri_revision = self.open_document_tracker.capture(&uri);
        if let Some(record) = self.documents.get(&uri) {
            record.syntax_cancellation.cancel();
        }
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
        content: PreparedDocumentContent,
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
        self.open_prepared_document(ticket.uri.clone(), version, content, kind, false);
        true
    }

    fn open_prepared_document(
        &mut self,
        uri: Uri,
        version: i32,
        content: PreparedDocumentContent,
        kind: DocumentKind,
        preserve_semantic_tokens_state: bool,
    ) -> StoredDocument {
        let PreparedDocumentContent {
            source,
            syntax_document,
        } = content;
        let document = StoredDocument {
            uri: uri.clone(),
            version,
            kind,
            source,
        };
        self.upsert_document(
            uri,
            document,
            syntax_document,
            preserve_semantic_tokens_state,
        )
    }

    #[cfg(test)]
    fn open_document_source(
        &mut self,
        uri: Uri,
        version: i32,
        source: DocumentSource,
        kind: DocumentKind,
    ) -> StoredDocument {
        let syntax_document = source.retained_text().and_then(|text| {
            SyntaxDocumentState::parse(version, kind, Arc::clone(text), &self.session_cancellation)
                .ok()
                .map(Arc::new)
        });
        self.open_prepared_document(
            uri,
            version,
            PreparedDocumentContent {
                source,
                syntax_document,
            },
            kind,
            false,
        )
    }

    fn upsert_document(
        &mut self,
        uri: Uri,
        document: StoredDocument,
        syntax_document: Option<Arc<SyntaxDocumentState>>,
        preserve_semantic_tokens_state: bool,
    ) -> StoredDocument {
        self.open_document_tracker.advance(&uri);
        self.analysis_executor.invalidate(&uri);
        self.analysis_cache.remove(&uri);
        let epoch = self.next_document_epoch();
        let syntax_cancellation = self.session_cancellation.child();
        match self.documents.entry(uri) {
            std::collections::hash_map::Entry::Occupied(mut entry) => {
                let record = entry.get_mut();
                record.syntax_cancellation.cancel();
                record.document = document.clone();
                record.epoch = epoch;
                record.diagnostic_state = None;
                record.syntax_document = syntax_document;
                record.syntax_cancellation = syntax_cancellation;
                if !preserve_semantic_tokens_state {
                    record.semantic_tokens_state = None;
                }
            }
            std::collections::hash_map::Entry::Vacant(entry) => {
                entry.insert(DocumentRecord {
                    document: document.clone(),
                    epoch,
                    diagnostic_state: None,
                    syntax_document,
                    syntax_cancellation,
                    semantic_tokens_state: None,
                });
            }
        }
        self.advance_documents_revision();
        document
    }

    fn capture_text_changes(
        &mut self,
        uri: Uri,
        version: i32,
        changes: impl IntoIterator<Item = TextDocumentContentChangeEvent>,
    ) -> Option<Box<TextChangePlan>> {
        let record = self.documents.get_mut(&uri)?;
        if version <= record.document.version {
            return None;
        }
        let changes = changes.into_iter().collect::<Vec<_>>();
        if changes.is_empty() {
            return None;
        }
        let kind = record.document.kind;
        let expected_epoch = record.epoch;
        let source = record.document.source.clone();
        let syntax_document = record.syntax_document.as_ref().map(Arc::clone);
        record.syntax_cancellation.cancel();

        Some(Box::new(TextChangePlan {
            uri,
            version,
            kind,
            expected_epoch,
            expected_configuration_revision: self.configuration_revision,
            resource_limits: self.analyzer.options().resource_limits(),
            source,
            syntax_document,
            changes,
            cancellation: self.session_cancellation.child(),
        }))
    }

    fn commit_prepared_document_change(&mut self, prepared: PreparedDocumentChange) -> bool {
        let Some(record) = self.documents.get(&prepared.uri) else {
            return false;
        };
        if record.epoch != prepared.expected_epoch
            || self.configuration_revision != prepared.expected_configuration_revision
        {
            return false;
        }

        self.open_prepared_document(
            prepared.uri,
            prepared.version,
            prepared.content,
            prepared.kind,
            true,
        );
        true
    }

    pub(super) fn get(&self, uri: &Uri) -> Option<&StoredDocument> {
        self.documents.get(uri).map(|record| &record.document)
    }

    pub(in crate::session) fn syntax_document_snapshot(
        &self,
        uri: &Uri,
    ) -> Option<SyntaxDocumentSnapshot> {
        let record = self.documents.get(uri)?;
        Some(SyntaxDocumentSnapshot {
            uri: uri.clone(),
            document: Arc::clone(record.syntax_document.as_ref()?),
            document_epoch: record.epoch,
            cancellation: record.syntax_cancellation.clone(),
        })
    }

    pub(in crate::session) fn is_syntax_document_current(
        &self,
        snapshot: &SyntaxDocumentSnapshot,
    ) -> bool {
        self.documents.get(&snapshot.uri).is_some_and(|record| {
            record.epoch == snapshot.document_epoch
                && !record.syntax_cancellation.is_cancelled()
                && record
                    .syntax_document
                    .as_ref()
                    .is_some_and(|document| Arc::ptr_eq(document, &snapshot.document))
        })
    }

    pub(in crate::session) fn semantic_tokens_state_for_delta(
        &self,
        uri: &Uri,
        previous_result_id: &str,
    ) -> Option<Arc<SemanticTokensState>> {
        self.documents
            .get(uri)
            .and_then(|record| record.semantic_tokens_state.as_ref())
            .and_then(|state| (state.result_id == previous_result_id).then(|| Arc::clone(state)))
    }

    pub(in crate::session) fn set_semantic_tokens_state_if_syntax_current(
        &mut self,
        snapshot: &SyntaxDocumentSnapshot,
        state: SemanticTokensState,
    ) -> bool {
        if !self.is_syntax_document_current(snapshot) {
            return false;
        }
        let Some(record) = self.documents.get_mut(&snapshot.uri) else {
            return false;
        };
        record.semantic_tokens_state = Some(Arc::new(state));
        true
    }

    pub(super) fn remove(&mut self, uri: &Uri) {
        self.analysis_executor.forget(uri);
        self.open_document_tracker.advance(uri);
        if let Some(record) = self.documents.remove(uri) {
            record.syntax_cancellation.cancel();
            self.advance_documents_revision();
        }
        self.analysis_cache.remove(uri);
    }
}

#[derive(Debug, Clone)]
struct DocumentRecord {
    document: StoredDocument,
    epoch: DocumentEpoch,
    diagnostic_state: Option<StoredDiagnosticState>,
    syntax_document: Option<Arc<SyntaxDocumentState>>,
    syntax_cancellation: AnalysisCancellationToken,
    semantic_tokens_state: Option<Arc<SemanticTokensState>>,
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

    pub(crate) const fn diagnostic_generation(&self) -> DiagnosticGeneration {
        self.generation
    }

    pub(crate) const fn document_epoch(&self) -> DocumentEpoch {
        self.document_epoch
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ConfigurationUpdateOutcome {
    Unchanged,
    Applied(AnalysisConfigChange),
    Superseded,
    Cancelled,
    Failed,
}

impl ConfigurationUpdateOutcome {
    fn applied(change: AnalysisConfigChange) -> Self {
        match change {
            AnalysisConfigChange::Unchanged => Self::Unchanged,
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
        let cancellation = self.inner.cancellation.child();
        let preparation_uri = uri.clone();
        let prepared = match tokio::task::spawn_blocking(move || {
            prepare_document_content_cancellable(
                &preparation_uri,
                version,
                kind,
                text,
                resource_limits,
                None,
                &cancellation,
            )
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
    ) -> Option<bool> {
        let preparation = {
            let mut state = self.inner.state.lock().await;
            self.commit_state_if_active(&mut state, |state| {
                state.capture_text_changes(uri.clone(), version, changes)
            })?
        };
        let Some(plan) = preparation else {
            return Some(false);
        };
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
            state.commit_prepared_document_change(prepared)
        })
    }

    pub(crate) async fn close_document(&self, uri: &Uri) {
        let mut state = self.inner.state.lock().await;
        let _ = self.commit_state_if_active(&mut state, |state| state.remove(uri));
    }
}

#[cfg(test)]
#[path = "documents_tests.rs"]
mod tests;
