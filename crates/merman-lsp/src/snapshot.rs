use merman_analysis::{FenceTextIndex, SourceMap};
use tower_lsp::lsp_types::{Position, Url};

#[derive(Debug, Clone)]
pub struct DocumentSnapshot {
    pub uri: Url,
    pub version: i32,
    pub text: String,
    pub source_map: SourceMap,
    pub fences: Vec<FenceSnapshot>,
}

#[derive(Debug, Clone)]
pub struct FenceSnapshot {
    pub index: usize,
    pub start: usize,
    pub body_start: usize,
    pub end: usize,
    pub text: String,
    pub diagram_type: Option<String>,
    pub text_index: FenceTextIndex,
}

impl DocumentSnapshot {
    pub fn byte_offset_for_position(&self, position: Position) -> Option<usize> {
        self.source_map
            .byte_offset_for_utf16_position(merman_analysis::Utf16Position {
                line: position.line as usize,
                character: position.character as usize,
            })
    }

    pub fn fence_at_position(&self, position: Position) -> Option<&FenceSnapshot> {
        let offset = self.byte_offset_for_position(position)?;

        self.fences
            .iter()
            .find(|fence| offset >= fence.start && offset <= fence.end)
    }
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
