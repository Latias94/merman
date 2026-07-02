use crate::types::{DocumentKind, DocumentUri, Position};
use merman_analysis::{FenceDelimiter, FenceTextIndex, SourceDescriptor, SourceMap};

#[derive(Debug, Clone)]
pub struct DocumentSnapshot {
    pub uri: DocumentUri,
    pub version: i32,
    pub kind: DocumentKind,
    pub source: SourceDescriptor,
    pub text: String,
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
    pub fn byte_offset_for_position(&self, position: Position) -> Option<usize> {
        self.source_map
            .byte_offset_for_utf16_position(merman_analysis::Utf16Position {
                line: position.line,
                character: position.character,
            })
    }

    pub fn fence_at_position(&self, position: Position) -> Option<&FenceSnapshot> {
        let offset = self.byte_offset_for_position(position)?;

        self.fences.iter().find(|fence| {
            offset >= fence.start
                && (offset < fence.end || (fence.fence_delimiter.is_none() && offset == fence.end))
        })
    }
}
