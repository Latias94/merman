use std::ops::Deref;
use std::sync::Arc;

use merman_analysis::AnalysisPayload;
use tower_lsp::lsp_types::Url;

#[derive(Debug, Clone)]
pub struct DocumentSnapshot {
    pub uri: Url,
    editor: merman_editor_core::DocumentSnapshot,
}

#[derive(Debug)]
pub struct DocumentAnalysisContext {
    pub snapshot: Arc<DocumentSnapshot>,
    pub payload: Arc<AnalysisPayload>,
}

#[derive(Debug, Clone, Copy, Default, Hash, PartialEq, Eq)]
pub(crate) struct DocumentEpoch(pub(crate) u64);

#[derive(Debug, Clone, Copy, Default, Hash, PartialEq, Eq)]
pub(crate) struct SnapshotGeneration(pub(crate) u64);

#[derive(Debug, Clone, Copy, Default, Hash, PartialEq, Eq)]
pub(crate) struct DiagnosticGeneration(pub(crate) u64);

#[derive(Debug, Clone)]
pub(crate) struct SnapshotContext {
    pub(crate) snapshot: Arc<DocumentSnapshot>,
    analysis: Option<SnapshotAnalysis>,
    pub(crate) generation: SnapshotGeneration,
    pub(crate) document_epoch: DocumentEpoch,
}

#[derive(Debug, Clone)]
struct SnapshotAnalysis {
    payload: Arc<AnalysisPayload>,
    generation: DiagnosticGeneration,
}

impl SnapshotContext {
    pub(crate) fn new(
        snapshot: Arc<DocumentSnapshot>,
        generation: SnapshotGeneration,
        document_epoch: DocumentEpoch,
    ) -> Self {
        Self {
            snapshot,
            analysis: None,
            generation,
            document_epoch,
        }
    }

    pub(crate) fn with_analysis(
        snapshot: Arc<DocumentSnapshot>,
        payload: Arc<AnalysisPayload>,
        generation: SnapshotGeneration,
        diagnostic_generation: DiagnosticGeneration,
        document_epoch: DocumentEpoch,
    ) -> Self {
        Self {
            snapshot,
            analysis: Some(SnapshotAnalysis {
                payload,
                generation: diagnostic_generation,
            }),
            generation,
            document_epoch,
        }
    }

    pub(crate) fn analysis_payload(&self) -> Option<&AnalysisPayload> {
        self.analysis
            .as_ref()
            .map(|analysis| analysis.payload.as_ref())
    }

    pub(crate) fn analysis_generation(&self) -> Option<DiagnosticGeneration> {
        self.analysis.as_ref().map(|analysis| analysis.generation)
    }
}

impl DocumentAnalysisContext {
    pub fn from_editor(context: merman_editor_core::DocumentAnalysisContext, uri: Url) -> Self {
        let (snapshot, payload) = context.into_parts();
        Self {
            snapshot: Arc::new(DocumentSnapshot::from_editor(snapshot, uri)),
            payload: Arc::new(payload),
        }
    }
}

impl DocumentSnapshot {
    pub fn from_editor(snapshot: merman_editor_core::DocumentSnapshot, uri: Url) -> Self {
        Self {
            uri,
            editor: snapshot,
        }
    }

    pub fn as_editor(&self) -> &merman_editor_core::DocumentSnapshot {
        &self.editor
    }

    #[cfg(test)]
    pub fn fence_at_position(
        &self,
        position: tower_lsp::lsp_types::Position,
    ) -> Option<&merman_editor_core::FenceSnapshot> {
        self.editor.fence_at_position(position_to_editor(position))
    }
}

impl Deref for DocumentSnapshot {
    type Target = merman_editor_core::DocumentSnapshot;

    fn deref(&self) -> &Self::Target {
        &self.editor
    }
}

#[cfg(test)]
fn position_to_editor(position: tower_lsp::lsp_types::Position) -> merman_editor_core::Position {
    merman_editor_core::Position::new(position.line as usize, position.character as usize)
}

#[cfg(test)]
mod tests {
    use tower_lsp::lsp_types::{Position, Url};

    #[test]
    fn fence_lookup_includes_end_position_for_completion() {
        let mut store = crate::document_store::DocumentStore::new();
        let uri = Url::parse("file:///tmp/example.mmd").unwrap();
        let snapshot = store.upsert(uri, 1, "flowchart".to_string());

        assert!(snapshot.fence_at_position(Position::new(0, 9)).is_some());
    }
}
