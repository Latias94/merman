use crate::snapshot::DocumentSnapshot;
use merman_analysis::{AnalysisOptions, Analyzer};
use merman_editor_core::{DocumentKind, DocumentWorkspace};
use std::collections::HashMap;
use tower_lsp::lsp_types::{SemanticToken, Url};

#[derive(Debug)]
pub struct DocumentStore {
    workspace: DocumentWorkspace,
    analyzer: Analyzer,
    snapshot_generation: SnapshotGeneration,
    diagnostic_generation: DiagnosticGeneration,
    next_document_epoch: u64,
    documents: HashMap<Url, DocumentRecord>,
    snapshots: HashMap<Url, DocumentSnapshot>,
    semantic_tokens_state: HashMap<Url, SemanticTokensState>,
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
    pub text: String,
    pub kind: DocumentKind,
}

#[derive(Debug, Clone, Default)]
pub struct SemanticTokensState {
    pub version: Option<i32>,
    pub result_id: Option<String>,
    pub tokens: Vec<SemanticToken>,
}

impl DocumentStore {
    pub fn new() -> Self {
        let analyzer = Analyzer::new();
        Self {
            workspace: DocumentWorkspace::with_analyzer(analyzer.clone()),
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
        self.analyzer = analyzer.clone();
        self.workspace.replace_analyzer(analyzer);
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
            && self
                .documents
                .get(&context.document.uri)
                .is_some_and(|record| record.epoch == context.document_epoch)
    }

    pub fn upsert_text(&mut self, uri: Url, version: i32, text: String) -> StoredDocument {
        let document = StoredDocument {
            kind: DocumentKind::from_path(uri.path()),
            uri: uri.clone(),
            version,
            text,
        };
        self.workspace
            .remove(&merman_editor_core::DocumentUri::new(uri.as_str()));
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

    pub fn upsert(&mut self, uri: Url, version: i32, text: String) -> DocumentSnapshot {
        self.upsert_text(uri.clone(), version, text);
        self.snapshot_cloned(&uri)
            .expect("snapshot should exist after inserting document text")
    }

    pub fn get(&self, uri: &Url) -> Option<&StoredDocument> {
        self.documents.get(uri).map(|record| &record.document)
    }

    pub fn snapshot_cloned(&mut self, uri: &Url) -> Option<DocumentSnapshot> {
        self.snapshot_context(uri).map(|context| context.snapshot)
    }

    pub fn snapshot_context(&mut self, uri: &Url) -> Option<SnapshotContext> {
        // Request handlers own this snapshot so they can release the store mutex before
        // running editor queries. Projection code should borrow from it instead of cloning it.
        let record = self.documents.get(uri)?;
        let document = record.document.clone();
        let document_epoch = record.epoch;

        if let Some(snapshot) = self.snapshots.get(uri) {
            return Some(SnapshotContext::new(
                snapshot.clone(),
                self.snapshot_generation,
                document_epoch,
            ));
        }

        let snapshot = self.workspace.upsert(
            document.uri.as_str(),
            document.version,
            document.text.clone(),
            document.kind,
        );
        let snapshot = DocumentSnapshot::from_editor(snapshot, document.uri.clone());
        self.snapshots
            .insert(document.uri.clone(), snapshot.clone());
        Some(SnapshotContext::new(
            snapshot,
            self.snapshot_generation,
            document_epoch,
        ))
    }

    pub fn is_snapshot_context_current(&self, context: &SnapshotContext) -> bool {
        self.snapshot_generation == context.generation
            && self
                .documents
                .get(&context.snapshot.uri)
                .is_some_and(|record| record.epoch == context.document_epoch)
    }

    pub fn has_snapshot(&self, uri: &Url) -> bool {
        self.snapshots.contains_key(uri)
    }

    pub fn remove(&mut self, uri: &Url) {
        self.workspace
            .remove(&merman_editor_core::DocumentUri::new(uri.as_str()));
        self.documents.remove(uri);
        self.snapshots.remove(uri);
        self.semantic_tokens_state.remove(uri);
    }

    pub fn documents(&self) -> Vec<StoredDocument> {
        self.documents
            .values()
            .map(|record| record.document.clone())
            .collect()
    }

    pub(crate) fn documents_with_analyzer(&self) -> (Vec<StoredDocument>, Analyzer) {
        (self.documents(), self.analyzer.clone())
    }

    pub fn snapshots(&mut self) -> Vec<DocumentSnapshot> {
        let uris = self.documents.keys().cloned().collect::<Vec<_>>();
        uris.into_iter()
            .filter_map(|uri| self.snapshot_cloned(&uri))
            .collect()
    }

    pub fn semantic_tokens_state(&self, uri: &Url) -> Option<&SemanticTokensState> {
        self.semantic_tokens_state.get(uri)
    }

    pub fn semantic_tokens_state_cloned(&self, uri: &Url) -> Option<SemanticTokensState> {
        self.semantic_tokens_state.get(uri).cloned()
    }

    pub fn set_semantic_tokens_state(&mut self, uri: Url, state: SemanticTokensState) {
        self.semantic_tokens_state.insert(uri, state);
    }
}

#[derive(Debug, Clone)]
struct DocumentRecord {
    document: StoredDocument,
    epoch: DocumentEpoch,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct DocumentEpoch(u64);

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct SnapshotGeneration(u64);

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct DiagnosticGeneration(u64);

#[derive(Debug, Clone)]
pub struct SnapshotContext {
    pub snapshot: DocumentSnapshot,
    generation: SnapshotGeneration,
    document_epoch: DocumentEpoch,
}

impl SnapshotContext {
    fn new(
        snapshot: DocumentSnapshot,
        generation: SnapshotGeneration,
        document_epoch: DocumentEpoch,
    ) -> Self {
        Self {
            snapshot,
            generation,
            document_epoch,
        }
    }

    pub fn generation(&self) -> SnapshotGeneration {
        self.generation
    }

    pub fn document_epoch(&self) -> DocumentEpoch {
        self.document_epoch
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

    pub fn generation(&self) -> DiagnosticGeneration {
        self.generation
    }

    pub fn document_epoch(&self) -> DocumentEpoch {
        self.document_epoch
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
