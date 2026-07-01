use merman_analysis::{FenceDelimiter, FenceTextIndex, SourceDescriptor, SourceMap};
use tower_lsp::lsp_types::{Position, Url};

#[derive(Debug, Clone)]
pub struct DocumentSnapshot {
    pub uri: Url,
    pub version: i32,
    pub text: String,
    pub source: SourceDescriptor,
    pub source_map: SourceMap,
    pub fences: Vec<FenceSnapshot>,
}

#[derive(Debug, Clone)]
pub struct FenceSnapshot {
    pub source_id: String,
    pub index: usize,
    pub source: SourceDescriptor,
    pub start: usize,
    pub body_start: usize,
    pub body_end: usize,
    pub end: usize,
    pub text: String,
    pub fence_delimiter: Option<FenceDelimiter>,
    pub diagram_type: Option<String>,
    pub text_index: FenceTextIndex,
}

impl DocumentSnapshot {
    pub fn from_editor(snapshot: merman_editor_core::DocumentSnapshot, uri: Url) -> Self {
        Self {
            uri,
            version: snapshot.version,
            text: snapshot.text,
            source: snapshot.source,
            source_map: snapshot.source_map,
            fences: snapshot
                .fences
                .into_iter()
                .map(|fence| FenceSnapshot {
                    source_id: fence.source_id,
                    index: fence.index,
                    source: fence.source,
                    start: fence.start,
                    body_start: fence.body_start,
                    body_end: fence.body_end,
                    end: fence.end,
                    text: fence.text,
                    fence_delimiter: fence.fence_delimiter,
                    diagram_type: fence.diagram_type,
                    text_index: fence.text_index,
                })
                .collect(),
        }
    }

    pub fn to_editor(&self) -> merman_editor_core::DocumentSnapshot {
        merman_editor_core::DocumentSnapshot {
            uri: merman_editor_core::DocumentUri::new(self.uri.as_str()),
            version: self.version,
            kind: merman_editor_core::DocumentKind::from_path(self.uri.path()),
            text: self.text.clone(),
            source: self.source.clone(),
            source_map: self.source_map.clone(),
            fences: self
                .fences
                .iter()
                .map(|fence| merman_editor_core::FenceSnapshot {
                    source_id: fence.source_id.clone(),
                    index: fence.index,
                    source: fence.source.clone(),
                    start: fence.start,
                    body_start: fence.body_start,
                    body_end: fence.body_end,
                    end: fence.end,
                    text: fence.text.clone(),
                    fence_delimiter: fence.fence_delimiter,
                    diagram_type: fence.diagram_type.clone(),
                    text_index: fence.text_index.clone(),
                })
                .collect(),
        }
    }

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
