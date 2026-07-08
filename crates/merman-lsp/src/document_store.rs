use crate::snapshot::DocumentSnapshot;
use merman_analysis::{AnalysisOptions, Analyzer, SourceMap, Utf16Position};
use merman_editor_core::{DocumentKind, DocumentWorkspace};
use std::collections::HashMap;
use std::ops::Range as ByteRange;
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
    snapshot_generation: SnapshotGeneration,
    diagnostic_generation: DiagnosticGeneration,
    next_document_epoch: u64,
    documents: HashMap<Url, DocumentRecord>,
    snapshots: HashMap<Url, Arc<DocumentSnapshot>>,
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

impl StoredDocument {
    pub fn has_unavailable_source(&self) -> bool {
        self.resource_limit.is_some()
            || self.discarded_source.is_some()
            || self.sync_error.is_some()
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
            snapshot_generation: SnapshotGeneration::default(),
            diagnostic_generation: DiagnosticGeneration::default(),
            next_document_epoch: 0,
            documents: HashMap::new(),
            snapshots: HashMap::new(),
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
    }

    fn replace_analyzer(&mut self, analyzer: Analyzer) {
        self.analyzer = analyzer;
        self.reclassify_unavailable_documents_for_current_limit();
        self.advance_snapshot_generation();
        self.advance_diagnostic_generation();
        self.snapshots.clear();
        self.semantic_tokens_state.clear();
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
                self.analyzer.clone(),
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
        self.snapshots.remove(&uri);
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
        let resource_limit = current.resource_limit;
        let discarded_source = current.discarded_source;
        let sync_error = current.sync_error;
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

        if resource_limit.is_some() || discarded_source.is_some() || sync_error.is_some() {
            return self.apply_unavailable_source_text_changes(
                uri,
                version,
                kind,
                resource_limit,
                discarded_source,
                sync_error,
                changes,
            );
        }

        let changes = changes_from_last_full_replacement(changes);
        let mut text = current_text.to_string();
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

        self.upsert_text(uri, version, text, kind);
        TextDocumentUpdate::Applied
    }

    fn apply_unavailable_source_text_changes(
        &mut self,
        uri: Url,
        version: i32,
        kind: DocumentKind,
        resource_limit: Option<DocumentResourceLimit>,
        discarded_source: Option<DocumentDiscardedSource>,
        sync_error: Option<DocumentSyncError>,
        changes: Vec<TextDocumentContentChangeEvent>,
    ) -> TextDocumentUpdate {
        let Some(recovery_start) = changes.iter().rposition(|change| change.range.is_none()) else {
            match (resource_limit, discarded_source, sync_error) {
                (Some(resource_limit), _, _) => {
                    self.upsert_resource_limited(uri, version, kind, resource_limit);
                }
                (None, Some(discarded_source), _) => {
                    self.upsert_discarded_source(uri, version, kind, discarded_source);
                }
                (None, None, Some(sync_error)) => {
                    self.upsert_sync_error(uri, version, kind, sync_error);
                }
                (None, None, None) => {
                    unreachable!("checked unavailable source before applying edits")
                }
            }
            return TextDocumentUpdate::Applied;
        };
        let mut known_text = None::<String>;

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
                None => known_text = Some(change.text),
            }
        }

        let Some(text) = known_text else {
            return TextDocumentUpdate::EmptyChangeSet;
        };
        self.upsert_text(uri, version, text, kind);
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
            return Some(SnapshotContext::new(
                Arc::clone(snapshot),
                self.snapshot_generation,
                self.documents.get(uri)?.epoch,
            ));
        }

        let request = self.snapshot_build_request(uri)?;
        let snapshot = request.build();
        self.insert_built_snapshot(&request, snapshot)
    }

    pub fn snapshot_build_request(&self, uri: &Url) -> Option<SnapshotBuildRequest> {
        let record = self.documents.get(uri)?;
        if record.document.has_unavailable_source() {
            return None;
        }
        Some(SnapshotBuildRequest {
            document: record.document.clone(),
            analyzer: self.analyzer.clone(),
            generation: self.snapshot_generation,
            document_epoch: record.epoch,
        })
    }

    pub fn insert_built_snapshot(
        &mut self,
        request: &SnapshotBuildRequest,
        snapshot: Arc<DocumentSnapshot>,
    ) -> Option<SnapshotContext> {
        if self.snapshot_generation != request.generation
            || !self.is_document_epoch_current(&request.document.uri, request.document_epoch)
        {
            return None;
        }

        if let Some(cached) = self.snapshots.get(&request.document.uri) {
            return Some(SnapshotContext::new(
                Arc::clone(cached),
                request.generation,
                request.document_epoch,
            ));
        }

        self.snapshots
            .insert(request.document.uri.clone(), Arc::clone(&snapshot));
        Some(SnapshotContext::new(
            snapshot,
            request.generation,
            request.document_epoch,
        ))
    }

    pub fn is_snapshot_context_current(&self, context: &SnapshotContext) -> bool {
        self.snapshot_generation == context.generation
            && self.is_document_epoch_current(&context.snapshot.uri, context.document_epoch)
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

    pub fn remove(&mut self, uri: &Url) {
        self.documents.remove(uri);
        self.snapshots.remove(uri);
        self.diagnostic_state.remove(uri);
        self.semantic_tokens_state.remove(uri);
    }

    pub(crate) fn diagnostic_contexts(&self) -> Vec<DiagnosticContext> {
        self.documents
            .values()
            .map(|record| {
                DiagnosticContext::new(
                    record.document.clone(),
                    self.analyzer.clone(),
                    self.diagnostic_generation,
                    record.epoch,
                )
            })
            .collect()
    }

    #[cfg(test)]
    pub fn snapshot_build_requests(&self) -> (Vec<SnapshotContext>, Vec<SnapshotBuildRequest>) {
        let mut contexts = Vec::new();
        let mut requests = Vec::new();

        for (uri, record) in &self.documents {
            if let Some(snapshot) = self.snapshots.get(uri) {
                contexts.push(SnapshotContext::new(
                    Arc::clone(snapshot),
                    self.snapshot_generation,
                    record.epoch,
                ));
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
                contexts.push(SnapshotContext::new(
                    Arc::clone(snapshot),
                    self.snapshot_generation,
                    record.epoch,
                ));
            } else if let Some(request) = self.snapshot_build_request(uri) {
                requests.push(request);
            }
        }

        let batch_size = batch_size.max(1);
        let batches = requests
            .chunks(batch_size)
            .map(|chunk| chunk.to_vec())
            .collect();

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

    pub fn snapshot_contexts_for_requests(
        &mut self,
        requests: Vec<(SnapshotBuildRequest, Arc<DocumentSnapshot>)>,
    ) -> SnapshotBatchCommit {
        #[cfg(test)]
        let mut contexts = Vec::new();
        let mut stale_open_documents = false;

        for (request, snapshot) in requests {
            match self.insert_built_snapshot(&request, snapshot) {
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

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct DocumentEpoch(u64);

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct SnapshotGeneration(u64);

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct DiagnosticGeneration(u64);

#[derive(Debug, Clone)]
pub struct SnapshotContext {
    pub snapshot: Arc<DocumentSnapshot>,
    generation: SnapshotGeneration,
    document_epoch: DocumentEpoch,
}

impl SnapshotContext {
    fn new(
        snapshot: Arc<DocumentSnapshot>,
        generation: SnapshotGeneration,
        document_epoch: DocumentEpoch,
    ) -> Self {
        Self {
            snapshot,
            generation,
            document_epoch,
        }
    }
}

#[derive(Debug, Clone)]
pub struct SnapshotBatchCommit {
    #[cfg(test)]
    pub contexts: Vec<SnapshotContext>,
    pub stale_open_documents: bool,
}

#[derive(Debug, Clone)]
pub struct WorkspaceSnapshotBuildPlan {
    pub contexts: Vec<SnapshotContext>,
    pub batches: Vec<Vec<SnapshotBuildRequest>>,
}

impl WorkspaceSnapshotBuildPlan {
    #[cfg(test)]
    pub fn new_snapshot_request_count(&self) -> usize {
        self.batches.iter().map(Vec::len).sum()
    }
}

#[derive(Debug, Clone)]
pub struct SnapshotBuildRequest {
    document: StoredDocument,
    analyzer: Analyzer,
    generation: SnapshotGeneration,
    document_epoch: DocumentEpoch,
}

impl SnapshotBuildRequest {
    pub fn uri(&self) -> &Url {
        &self.document.uri
    }

    pub fn build(&self) -> Arc<DocumentSnapshot> {
        let snapshot = DocumentWorkspace::build_snapshot_with_shared_text(
            &self.analyzer,
            self.document.uri.as_str(),
            self.document.version,
            self.document.text.clone(),
            self.document.kind,
        );
        Arc::new(DocumentSnapshot::from_editor(
            snapshot,
            self.document.uri.clone(),
        ))
    }
}

#[derive(Debug, Clone)]
pub struct DiagnosticContext {
    pub document: StoredDocument,
    pub analyzer: Analyzer,
    generation: DiagnosticGeneration,
    document_epoch: DocumentEpoch,
}

impl DiagnosticContext {
    fn new(
        document: StoredDocument,
        analyzer: Analyzer,
        generation: DiagnosticGeneration,
        document_epoch: DocumentEpoch,
    ) -> Self {
        Self {
            document,
            analyzer,
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

fn apply_text_content_change(text: &mut String, change: TextDocumentContentChangeEvent) -> bool {
    if let Some(range) = change.range {
        let Some(byte_range) = lsp_range_to_byte_range(text, range) else {
            return false;
        };
        text.replace_range(byte_range, &change.text);
    } else {
        text.clear();
        text.push_str(&change.text);
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

fn lsp_range_to_byte_range(text: &str, range: Range) -> Option<ByteRange<usize>> {
    if !position_le(range.start, range.end) {
        return None;
    }

    let source_map = SourceMap::new(text);
    let start = strict_byte_offset_for_lsp_position(&source_map, range.start)?;
    let end = strict_byte_offset_for_lsp_position(&source_map, range.end)?;
    (start <= end).then_some(start..end)
}

fn strict_byte_offset_for_lsp_position(
    source_map: &SourceMap,
    position: Position,
) -> Option<usize> {
    let position = position_to_utf16(position);
    let (line_start, line_end) = source_map.line_bounds(position.line)?;
    let line_utf16_len = source_map.source()[line_start..line_end]
        .chars()
        .map(char::len_utf16)
        .sum::<usize>();
    if position.character > line_utf16_len {
        return None;
    }
    source_map.byte_offset_for_utf16_position(position)
}

fn position_to_utf16(position: Position) -> Utf16Position {
    Utf16Position {
        line: position.line as usize,
        character: position.character as usize,
    }
}

fn position_le(left: Position, right: Position) -> bool {
    left.line < right.line || (left.line == right.line && left.character <= right.character)
}
