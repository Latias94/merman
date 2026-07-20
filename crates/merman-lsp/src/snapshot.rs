#[cfg(test)]
use merman_editor_core::EditorDiagramDetection;
use merman_editor_core::{AnalyzedDocumentSnapshot, EditorDiagnostic};
use std::ops::Deref;
use tower_lsp::lsp_types::Url;

#[derive(Debug, Clone)]
pub struct DocumentSnapshot {
    pub uri: Url,
    analyzed: AnalyzedDocumentSnapshot,
}

impl DocumentSnapshot {
    pub fn from_editor(snapshot: AnalyzedDocumentSnapshot, uri: Url) -> Self {
        Self {
            uri,
            analyzed: snapshot,
        }
    }

    pub fn as_editor(&self) -> &merman_editor_core::DocumentSnapshot {
        self.analyzed.document()
    }

    pub fn diagnostics(&self) -> &[EditorDiagnostic] {
        self.analyzed.diagnostics()
    }

    #[cfg(test)]
    pub fn detection(&self) -> Option<&EditorDiagramDetection> {
        self.analyzed.detection()
    }

    #[cfg(test)]
    pub fn fence_at_position(
        &self,
        position: tower_lsp::lsp_types::Position,
    ) -> Option<&merman_editor_core::FenceSnapshot> {
        self.as_editor()
            .fence_at_position(position_to_editor(position))
    }
}

impl Deref for DocumentSnapshot {
    type Target = merman_editor_core::DocumentSnapshot;

    fn deref(&self) -> &Self::Target {
        self.as_editor()
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

    #[test]
    fn cached_snapshot_owns_diagnostics_from_the_same_analysis() {
        let mut store = crate::document_store::DocumentStore::new();
        let uri = Url::parse("file:///tmp/example.mmd").unwrap();
        let snapshot = store.upsert(uri, 7, "flowchart TD\nA[unterminated\n".to_string());

        assert_eq!(snapshot.version, 7);
        assert!(
            snapshot
                .diagnostics()
                .iter()
                .any(|diagnostic| { diagnostic.code == "merman.parse.diagram_parse" })
        );
    }
}
