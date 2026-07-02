use crate::snapshot::DocumentSnapshot;
use merman_analysis::Analyzer;
use merman_editor_core::{DocumentKind, DocumentWorkspace};
use std::collections::HashMap;
use tower_lsp::lsp_types::{SemanticToken, Url};

#[derive(Debug, Default)]
pub struct DocumentStore {
    workspace: DocumentWorkspace,
    documents: HashMap<Url, StoredDocument>,
    snapshots: HashMap<Url, DocumentSnapshot>,
    semantic_tokens_state: HashMap<Url, SemanticTokensState>,
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
        Self {
            workspace: DocumentWorkspace::new(),
            documents: HashMap::new(),
            snapshots: HashMap::new(),
            semantic_tokens_state: HashMap::new(),
        }
    }

    pub fn set_analyzer(&mut self, analyzer: Analyzer) {
        self.workspace.set_analyzer(analyzer);
    }

    pub fn replace_analyzer(&mut self, analyzer: Analyzer) {
        self.workspace.replace_analyzer(analyzer);
        self.snapshots.clear();
        self.semantic_tokens_state.clear();
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
        self.documents.insert(uri, document.clone());
        document
    }

    pub fn upsert(&mut self, uri: Url, version: i32, text: String) -> DocumentSnapshot {
        self.upsert_text(uri.clone(), version, text);
        self.snapshot_cloned(&uri)
            .expect("snapshot should exist after inserting document text")
    }

    pub fn get(&self, uri: &Url) -> Option<&StoredDocument> {
        self.documents.get(uri)
    }

    pub fn snapshot_cloned(&mut self, uri: &Url) -> Option<DocumentSnapshot> {
        // Request handlers own this snapshot so they can release the store mutex before
        // running editor queries. Projection code should borrow from it instead of cloning it.
        if let Some(snapshot) = self.snapshots.get(uri) {
            return Some(snapshot.clone());
        }

        let document = self.documents.get(uri)?;
        let snapshot = self.workspace.upsert(
            document.uri.as_str(),
            document.version,
            document.text.clone(),
            document.kind,
        );
        let snapshot = DocumentSnapshot::from_editor(snapshot, document.uri.clone());
        self.snapshots
            .insert(document.uri.clone(), snapshot.clone());
        Some(snapshot)
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
        self.documents.values().cloned().collect()
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
