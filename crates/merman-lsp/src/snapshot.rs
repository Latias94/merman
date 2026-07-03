use std::ops::Deref;

use tower_lsp::lsp_types::Url;

#[derive(Debug, Clone)]
pub struct DocumentSnapshot {
    pub uri: Url,
    editor: merman_editor_core::DocumentSnapshot,
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
