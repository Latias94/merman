use crate::snapshot::DocumentSnapshot;
use merman_analysis::{AnalysisOptions, Analyzer};
use merman_editor_core::{DocumentKind, DocumentWorkspace};
use std::collections::HashMap;
use std::sync::Arc;
use tower_lsp::lsp_types::{SemanticToken, Url};

#[derive(Debug)]
pub struct DocumentStore {
    analyzer: Analyzer,
    snapshot_generation: SnapshotGeneration,
    diagnostic_generation: DiagnosticGeneration,
    next_document_epoch: u64,
    documents: HashMap<Url, DocumentRecord>,
    snapshots: HashMap<Url, Arc<DocumentSnapshot>>,
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
        let analyzer = Analyzer::new();
        Self {
            analyzer,
            snapshot_generation: SnapshotGeneration::default(),
            diagnostic_generation: DiagnosticGeneration::default(),
            next_document_epoch: 0,
            documents: HashMap::new(),
            snapshots: HashMap::new(),
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
        let document = StoredDocument {
            uri: uri.clone(),
            version,
            text: Arc::<str>::from(text),
            kind,
        };
        self.snapshots.remove(&uri);
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

    pub fn snapshot_contexts_for_requests(
        &mut self,
        requests: Vec<(SnapshotBuildRequest, Arc<DocumentSnapshot>)>,
    ) -> SnapshotBatchCommit {
        let mut contexts = Vec::new();
        let mut stale_open_documents = false;

        for (request, snapshot) in requests {
            match self.insert_built_snapshot(&request, snapshot) {
                Some(context) => contexts.push(context),
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
    pub contexts: Vec<SnapshotContext>,
    pub stale_open_documents: bool,
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
        let snapshot = DocumentWorkspace::build_snapshot_with_analyzer(
            &self.analyzer,
            self.document.uri.as_str(),
            self.document.version,
            self.document.text.to_string(),
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
