use crate::types::{DocumentKind, DocumentUri, Position};
use merman_analysis::{FenceTextIndex, SourceMap};

#[derive(Debug, Clone)]
pub struct DocumentSnapshot {
    pub uri: DocumentUri,
    pub version: i32,
    pub kind: DocumentKind,
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
                line: position.line,
                character: position.character,
            })
    }

    pub fn fence_at_position(&self, position: Position) -> Option<&FenceSnapshot> {
        let offset = self.byte_offset_for_position(position)?;

        self.fences
            .iter()
            .find(|fence| offset >= fence.start && offset <= fence.end)
    }
}
