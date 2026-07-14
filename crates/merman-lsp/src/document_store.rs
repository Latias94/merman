use crate::analysis_executor::AnalysisExecutor;
use crate::analysis_request::{
    AnalysisBuildKey, AnalysisBuildRequest, SnapshotBatchCommit, WorkspaceSnapshotBuildPlan,
};
use crate::snapshot::{
    DiagnosticGeneration, DocumentAnalysisContext, DocumentEpoch, DocumentSnapshot,
    SnapshotContext, SnapshotGeneration,
};
use merman_analysis::{AnalysisOptions, AnalysisPayload, Analyzer};
use merman_editor_core::DocumentKind;
use ropey::{Rope, RopeSlice};
use std::collections::HashMap;
use std::sync::Arc;
use tower_lsp::lsp_types::{
    Diagnostic, Position, Range, SemanticToken, TextDocumentContentChangeEvent, Url,
};

pub const WORKSPACE_SYMBOL_SNAPSHOT_BATCH_SIZE: usize = 8;
pub(crate) const DEFAULT_LSP_MAX_SOURCE_BYTES: usize = 4 * 1024 * 1024;

pub(crate) fn default_lsp_analysis_options() -> AnalysisOptions {
    AnalysisOptions::default().with_max_source_bytes(Some(DEFAULT_LSP_MAX_SOURCE_BYTES))
}

pub(crate) fn analysis_options_with_lsp_resource_defaults(
    mut options: AnalysisOptions,
) -> AnalysisOptions {
    if options.max_source_bytes.is_none() {
        options.max_source_bytes = Some(DEFAULT_LSP_MAX_SOURCE_BYTES);
    }
    options
}

#[derive(Debug)]
pub struct DocumentStore {
    analyzer: Analyzer,
    analysis_executor: AnalysisExecutor,
    snapshot_generation: SnapshotGeneration,
    diagnostic_generation: DiagnosticGeneration,
    next_document_epoch: u64,
    documents: HashMap<Url, DocumentRecord>,
    snapshots: HashMap<Url, Arc<DocumentSnapshot>>,
    analysis_payloads: HashMap<Url, Arc<AnalysisPayload>>,
    diagnostic_state: HashMap<Url, StoredDiagnosticState>,
    semantic_tokens_state: HashMap<Url, StoredSemanticTokensState>,
}

impl Default for DocumentStore {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug, Clone)]
pub struct StoredDocument {
    pub uri: Url,
    pub version: i32,
    pub text: Arc<str>,
    pub kind: DocumentKind,
    pub resource_limit: Option<DocumentResourceLimit>,
    pub discarded_source: Option<DocumentDiscardedSource>,
    pub sync_error: Option<DocumentSyncError>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DocumentResourceLimit {
    pub source_len: usize,
    pub max_source_bytes: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DocumentDiscardedSource {
    pub source_len: usize,
    pub previous_max_source_bytes: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DocumentSyncError {
    InvalidIncrementalRange,
}

#[derive(Debug, Clone, Copy)]
enum UnavailableSourceState {
    ResourceLimited(DocumentResourceLimit),
    Discarded(DocumentDiscardedSource),
    SyncError(DocumentSyncError),
}

impl StoredDocument {
    pub fn has_unavailable_source(&self) -> bool {
        self.resource_limit.is_some()
            || self.discarded_source.is_some()
            || self.sync_error.is_some()
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
) -> Option<(usize, usize)> {
    if let Some(resource_limit) = document.resource_limit {
        return Some((resource_limit.source_len, resource_limit.max_source_bytes));
    }
    document
        .discarded_source
        .map(|discarded| (discarded.source_len, discarded.previous_max_source_bytes))
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TextDocumentUpdate {
    Applied,
    MissingDocument,
    EmptyChangeSet,
    InvalidRange,
    StaleVersion {
        current_version: i32,
        attempted_version: i32,
    },
}

impl TextDocumentUpdate {
    pub fn affects_document_state(self) -> bool {
        matches!(self, Self::Applied | Self::InvalidRange)
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

impl DocumentStore {
    pub fn new() -> Self {
        let analyzer = Analyzer::with_options(default_lsp_analysis_options());
        Self {
            analyzer,
            analysis_executor: AnalysisExecutor::new(),
            snapshot_generation: SnapshotGeneration::default(),
            diagnostic_generation: DiagnosticGeneration::default(),
            next_document_epoch: 0,
            documents: HashMap::new(),
            snapshots: HashMap::new(),
            analysis_payloads: HashMap::new(),
            diagnostic_state: HashMap::new(),
            semantic_tokens_state: HashMap::new(),
        }
    }

    pub fn apply_analyzer_options(
        &mut self,
        options: AnalysisOptions,
    ) -> AnalyzerConfigurationChange {
        let change = analyzer_configuration_change(self.analyzer.options(), &options);
        if matches!(change, AnalyzerConfigurationChange::Unchanged) {
            return change;
        }

        let analyzer = Analyzer::with_options(options);
        if change.affects_snapshots() {
            self.replace_analyzer(analyzer);
        } else {
            self.set_diagnostic_analyzer(analyzer);
        }
        change
    }

    fn set_diagnostic_analyzer(&mut self, analyzer: Analyzer) {
        self.analyzer = analyzer;
        self.advance_diagnostic_generation();
        self.analysis_payloads.clear();
        self.analysis_executor.invalidate_all();
    }

    fn replace_analyzer(&mut self, analyzer: Analyzer) {
        self.analyzer = analyzer;
        self.reclassify_unavailable_documents_for_current_limit();
        self.advance_snapshot_generation();
        self.advance_diagnostic_generation();
        self.snapshots.clear();
        self.analysis_payloads.clear();
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

    pub fn diagnostic_context(&self, uri: &Url) -> Option<DiagnosticContext> {
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

    pub fn upsert_text(
        &mut self,
        uri: Url,
        version: i32,
        text: String,
        kind: DocumentKind,
    ) -> StoredDocument {
        if let Some(resource_limit) = self.resource_limit_for_source_len(text.len()) {
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

    fn upsert_resource_limited(
        &mut self,
        uri: Url,
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

    fn upsert_discarded_source(
        &mut self,
        uri: Url,
        version: i32,
        kind: DocumentKind,
        discarded_source: DocumentDiscardedSource,
    ) -> StoredDocument {
        let document = StoredDocument {
            uri: uri.clone(),
            version,
            text: Arc::<str>::from(""),
            kind,
            resource_limit: None,
            discarded_source: Some(discarded_source),
            sync_error: None,
        };
        self.upsert_document(uri, document)
    }

    fn upsert_sync_error(
        &mut self,
        uri: Url,
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

    fn upsert_document(&mut self, uri: Url, document: StoredDocument) -> StoredDocument {
        self.analysis_executor.invalidate(&uri);
        self.snapshots.remove(&uri);
        self.analysis_payloads.remove(&uri);
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

    fn resource_limit_for_source_len(&self, source_len: usize) -> Option<DocumentResourceLimit> {
        let max_source_bytes = self.analyzer.options().max_source_bytes?;
        (source_len > max_source_bytes).then_some(DocumentResourceLimit {
            source_len,
            max_source_bytes,
        })
    }

    fn reclassify_unavailable_documents_for_current_limit(&mut self) {
        let current_limit = self.analyzer.options().max_source_bytes;
        for record in self.documents.values_mut() {
            let Some((source_len, previous_max_source_bytes)) =
                resource_state_source_len_and_previous_limit(&record.document)
            else {
                continue;
            };

            match current_limit {
                Some(max_source_bytes) if source_len > max_source_bytes => {
                    record.document.resource_limit = Some(DocumentResourceLimit {
                        source_len,
                        max_source_bytes,
                    });
                    record.document.discarded_source = None;
                }
                _ => {
                    record.document.resource_limit = None;
                    record.document.discarded_source = Some(DocumentDiscardedSource {
                        source_len,
                        previous_max_source_bytes,
                    });
                }
            }
        }
    }

    pub fn open_text(
        &mut self,
        uri: Url,
        version: i32,
        text: String,
        kind: DocumentKind,
    ) -> StoredDocument {
        self.upsert_text(uri, version, text, kind)
    }

    pub fn apply_text_changes(
        &mut self,
        uri: Url,
        version: i32,
        changes: impl IntoIterator<Item = TextDocumentContentChangeEvent>,
    ) -> TextDocumentUpdate {
        let Some(current) = self.get(&uri) else {
            return TextDocumentUpdate::MissingDocument;
        };
        let current_version = current.version;
        let kind = current.kind;
        let unavailable_source = current.unavailable_source_state();
        let current_text = current.text.clone();
        let changes = changes.into_iter().collect::<Vec<_>>();

        if version <= current_version {
            return TextDocumentUpdate::StaleVersion {
                current_version,
                attempted_version: version,
            };
        }

        if changes.is_empty() {
            return TextDocumentUpdate::EmptyChangeSet;
        }

        if let Some(unavailable_source) = unavailable_source {
            return self.apply_unavailable_source_text_changes(
                uri,
                version,
                kind,
                unavailable_source,
                changes,
            );
        }

        let changes = changes_from_last_full_replacement(changes);
        let mut text = Rope::from_str(&current_text);
        for change in changes {
            if !apply_text_content_change(&mut text, change) {
                self.upsert_sync_error(
                    uri,
                    version,
                    kind,
                    DocumentSyncError::InvalidIncrementalRange,
                );
                return TextDocumentUpdate::InvalidRange;
            }
        }

        self.upsert_text(uri, version, text.to_string(), kind);
        TextDocumentUpdate::Applied
    }

    fn apply_unavailable_source_text_changes(
        &mut self,
        uri: Url,
        version: i32,
        kind: DocumentKind,
        unavailable_source: UnavailableSourceState,
        changes: Vec<TextDocumentContentChangeEvent>,
    ) -> TextDocumentUpdate {
        let Some(recovery_start) = changes.iter().rposition(|change| change.range.is_none()) else {
            match unavailable_source {
                UnavailableSourceState::ResourceLimited(resource_limit) => {
                    self.upsert_resource_limited(uri, version, kind, resource_limit);
                }
                UnavailableSourceState::Discarded(discarded_source) => {
                    self.upsert_discarded_source(uri, version, kind, discarded_source);
                }
                UnavailableSourceState::SyncError(sync_error) => {
                    self.upsert_sync_error(uri, version, kind, sync_error);
                }
            }
            return TextDocumentUpdate::Applied;
        };
        let mut known_text = None::<Rope>;

        for change in changes.into_iter().skip(recovery_start) {
            match known_text.as_mut() {
                Some(text) => {
                    if !apply_text_content_change(text, change) {
                        self.upsert_sync_error(
                            uri,
                            version,
                            kind,
                            DocumentSyncError::InvalidIncrementalRange,
                        );
                        return TextDocumentUpdate::InvalidRange;
                    }
                }
                None => known_text = Some(Rope::from_str(&change.text)),
            }
        }

        let Some(text) = known_text else {
            return TextDocumentUpdate::EmptyChangeSet;
        };
        self.upsert_text(uri, version, text.to_string(), kind);
        TextDocumentUpdate::Applied
    }

    #[cfg(test)]
    pub fn upsert(&mut self, uri: Url, version: i32, text: String) -> Arc<DocumentSnapshot> {
        let kind = DocumentKind::from_path(uri.path());
        self.upsert_text(uri.clone(), version, text, kind);
        self.snapshot(&uri)
            .expect("snapshot should exist after inserting document text")
    }

    pub fn get(&self, uri: &Url) -> Option<&StoredDocument> {
        self.documents.get(uri).map(|record| &record.document)
    }

    #[cfg(test)]
    pub fn analyzer_options(&self) -> &AnalysisOptions {
        self.analyzer.options()
    }

    #[cfg(test)]
    pub fn snapshot(&mut self, uri: &Url) -> Option<Arc<DocumentSnapshot>> {
        self.snapshot_context(uri).map(|context| context.snapshot)
    }

    pub fn snapshot_context(&mut self, uri: &Url) -> Option<SnapshotContext> {
        if let Some(snapshot) = self.snapshots.get(uri) {
            return Some(self.cached_snapshot_context(
                uri,
                snapshot,
                self.documents.get(uri)?.epoch,
            ));
        }

        let request = self.snapshot_build_request(uri)?;
        let analysis = request.build();
        self.insert_built_analysis(&request, analysis)
    }

    pub(crate) fn snapshot_build_request(&self, uri: &Url) -> Option<AnalysisBuildRequest> {
        let record = self.documents.get(uri)?;
        if record.document.has_unavailable_source() {
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
        self.analysis_executor.release(request);
        if self.snapshot_generation != request.snapshot_generation()
            || self.diagnostic_generation != request.diagnostic_generation()
            || !self.is_document_epoch_current(request.uri(), request.document_epoch())
        {
            return None;
        }

        let snapshot = self
            .snapshots
            .entry(request.uri().clone())
            .or_insert_with(|| Arc::clone(&analysis.snapshot))
            .clone();
        self.analysis_payloads
            .insert(request.uri().clone(), Arc::clone(&analysis.payload));
        Some(SnapshotContext::with_analysis(
            snapshot,
            Arc::clone(&analysis.payload),
            request.snapshot_generation(),
            request.diagnostic_generation(),
            request.document_epoch(),
        ))
    }

    pub fn is_snapshot_context_current(&self, context: &SnapshotContext) -> bool {
        self.snapshot_generation == context.generation
            && self.is_document_epoch_current(&context.snapshot.uri, context.document_epoch)
    }

    pub fn is_analysis_context_current(&self, context: &SnapshotContext) -> bool {
        self.is_snapshot_context_current(context)
            && context.analysis_generation() == Some(self.diagnostic_generation)
    }

    pub fn is_snapshot_contexts_current(&self, contexts: &[SnapshotContext]) -> bool {
        contexts
            .iter()
            .all(|context| self.is_snapshot_context_current(context))
    }

    fn is_document_epoch_current(&self, uri: &Url, document_epoch: DocumentEpoch) -> bool {
        self.documents
            .get(uri)
            .is_some_and(|record| record.epoch == document_epoch)
    }

    pub fn has_snapshot(&self, uri: &Url) -> bool {
        self.snapshots.contains_key(uri)
    }

    pub fn has_analysis_payload(&self, uri: &Url) -> bool {
        self.analysis_payloads.contains_key(uri)
    }

    pub(crate) fn analysis_executor(&self) -> AnalysisExecutor {
        self.analysis_executor.clone()
    }

    pub fn remove(&mut self, uri: &Url) {
        self.analysis_executor.forget(uri);
        self.documents.remove(uri);
        self.snapshots.remove(uri);
        self.analysis_payloads.remove(uri);
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
            if let Some(snapshot) = self.snapshots.get(uri) {
                contexts.push(self.cached_snapshot_context(uri, snapshot, record.epoch));
            } else if let Some(request) = self.snapshot_build_request(uri) {
                requests.push(request);
            }
        }

        (contexts, requests)
    }

    pub fn workspace_symbol_snapshot_build_plan(
        &self,
        batch_size: usize,
    ) -> WorkspaceSnapshotBuildPlan {
        let mut contexts = Vec::new();
        let mut requests = Vec::new();
        let mut documents = self.documents.iter().collect::<Vec<_>>();
        documents.sort_by(|(left, _), (right, _)| left.as_str().cmp(right.as_str()));

        for (uri, record) in documents {
            if let Some(snapshot) = self.snapshots.get(uri) {
                contexts.push(self.cached_snapshot_context(uri, snapshot, record.epoch));
            } else if let Some(request) = self.snapshot_build_request(uri) {
                requests.push(request);
            }
        }

        let batch_size = batch_size.max(1);
        let mut requests = requests.into_iter();
        let mut batches = Vec::new();
        loop {
            let batch = requests.by_ref().take(batch_size).collect::<Vec<_>>();
            if batch.is_empty() {
                break;
            }
            batches.push(batch);
        }

        WorkspaceSnapshotBuildPlan { contexts, batches }
    }

    pub fn workspace_symbol_snapshot_contexts_current(&self, contexts: &[SnapshotContext]) -> bool {
        contexts.len() == self.snapshot_eligible_document_count()
            && self.is_snapshot_contexts_current(contexts)
    }

    fn snapshot_eligible_document_count(&self) -> usize {
        self.documents
            .values()
            .filter(|record| !record.document.has_unavailable_source())
            .count()
    }

    fn cached_snapshot_context(
        &self,
        uri: &Url,
        snapshot: &Arc<DocumentSnapshot>,
        document_epoch: DocumentEpoch,
    ) -> SnapshotContext {
        match self.analysis_payloads.get(uri) {
            Some(payload) => SnapshotContext::with_analysis(
                Arc::clone(snapshot),
                Arc::clone(payload),
                self.snapshot_generation,
                self.diagnostic_generation,
                document_epoch,
            ),
            None => SnapshotContext::new(
                Arc::clone(snapshot),
                self.snapshot_generation,
                document_epoch,
            ),
        }
    }

    pub fn snapshot_contexts_for_requests(
        &mut self,
        requests: Vec<(AnalysisBuildRequest, Arc<DocumentAnalysisContext>)>,
    ) -> SnapshotBatchCommit {
        #[cfg(test)]
        let mut contexts = Vec::new();
        let mut stale_open_documents = false;

        for (request, analysis) in requests {
            match self.insert_built_analysis(&request, analysis) {
                Some(_context) => {
                    #[cfg(test)]
                    contexts.push(_context);
                }
                None if self.get(request.uri()).is_some() => stale_open_documents = true,
                None => {}
            }
        }

        SnapshotBatchCommit {
            #[cfg(test)]
            contexts,
            stale_open_documents,
        }
    }

    #[cfg(test)]
    pub fn semantic_tokens_state(&self, uri: &Url) -> Option<&SemanticTokensState> {
        self.semantic_tokens_state
            .get(uri)
            .map(|stored| &stored.state)
    }

    pub fn semantic_tokens_state_for_delta(
        &self,
        uri: &Url,
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
            context.snapshot.uri.clone(),
            StoredSemanticTokensState {
                snapshot_generation: context.generation,
                state,
            },
        );
        true
    }

    pub fn diagnostic_state(&self, uri: &Url) -> Option<DocumentDiagnosticState> {
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
    } else if current.snapshot_affecting_eq(next) {
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
