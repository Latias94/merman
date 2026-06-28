use crate::snapshot::DocumentSnapshot;
use merman_editor_core::{DocumentKind, DocumentWorkspace};
use std::collections::HashMap;
use tower_lsp::lsp_types::{SemanticToken, Url};

#[derive(Debug, Default)]
pub struct DocumentStore {
    workspace: DocumentWorkspace,
    documents: HashMap<Url, DocumentSnapshot>,
    semantic_tokens_state: HashMap<Url, SemanticTokensState>,
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
            semantic_tokens_state: HashMap::new(),
        }
    }

    pub fn upsert(&mut self, uri: Url, version: i32, text: String) -> DocumentSnapshot {
        let snapshot = self.workspace.upsert(
            uri.as_str(),
            version,
            text,
            DocumentKind::from_path(uri.path()),
        );
        let snapshot = DocumentSnapshot::from_editor(snapshot, uri.clone());
        self.documents.insert(uri, snapshot.clone());
        snapshot
    }

    pub fn get(&self, uri: &Url) -> Option<&DocumentSnapshot> {
        self.documents.get(uri)
    }

    pub fn remove(&mut self, uri: &Url) {
        self.workspace
            .remove(&merman_editor_core::DocumentUri::new(uri.as_str()));
        self.documents.remove(uri);
        self.semantic_tokens_state.remove(uri);
    }

    pub fn snapshots(&self) -> Vec<DocumentSnapshot> {
        self.documents.values().cloned().collect()
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

pub(crate) fn is_markdown_uri(uri: &Url) -> bool {
    DocumentKind::from_path(uri.path()).is_markdown()
}
