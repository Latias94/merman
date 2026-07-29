use crate::types::{DocumentKind, DocumentUri, Position};
use merman_analysis::{
    AnalyzedDiagram, FenceDelimiter, FenceDelimiterSpans, FenceTextIndex, SharedTextSlice,
    SourceDescriptor, SourceMap,
};
use std::ops::Range;
use std::sync::Arc;

#[derive(Debug, Clone)]
pub struct DocumentSnapshot {
    uri: DocumentUri,
    version: i32,
    kind: DocumentKind,
    source: SourceDescriptor,
    text: Arc<str>,
    source_map: SourceMap,
    fences: Vec<FenceSnapshot>,
}

#[derive(Debug, Clone)]
pub struct FenceSnapshot {
    source_id: String,
    index: usize,
    source: SourceDescriptor,
    start: usize,
    body_start: usize,
    body_end: usize,
    end: usize,
    text: SharedTextSlice,
    fence_delimiter: Option<FenceDelimiter>,
    fence_delimiter_spans: Option<FenceDelimiterSpans>,
    diagram_type: Option<String>,
    text_index: FenceTextIndex,
}

impl DocumentSnapshot {
    pub(crate) fn new(
        uri: DocumentUri,
        version: i32,
        kind: DocumentKind,
        source: SourceDescriptor,
        text: Arc<str>,
        source_map: SourceMap,
        fences: Vec<FenceSnapshot>,
    ) -> Self {
        debug_assert_eq!(source_map.source(), text.as_ref());
        Self {
            uri,
            version,
            kind,
            source,
            text,
            source_map,
            fences,
        }
    }

    pub fn uri(&self) -> &DocumentUri {
        &self.uri
    }

    pub const fn version(&self) -> i32 {
        self.version
    }

    pub const fn kind(&self) -> DocumentKind {
        self.kind
    }

    pub fn source(&self) -> &SourceDescriptor {
        &self.source
    }

    pub fn text(&self) -> &str {
        &self.text
    }

    pub fn shared_text(&self) -> &Arc<str> {
        &self.text
    }

    pub fn source_map(&self) -> &SourceMap {
        &self.source_map
    }

    pub fn fences(&self) -> &[FenceSnapshot] {
        &self.fences
    }

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
            .find(|fence| fence.includes_document_offset(offset))
    }
}

impl FenceSnapshot {
    pub(crate) fn from_analyzed_diagram(diagram: &AnalyzedDiagram) -> Self {
        let document_range = diagram.document_range();
        let body_range = diagram.body_range();
        let syntax = diagram.syntax();
        debug_assert!(document_range.start <= body_range.start);
        debug_assert!(body_range.start <= body_range.end);
        debug_assert!(body_range.end <= document_range.end);
        Self {
            source_id: diagram.source_id().to_owned(),
            index: diagram.index(),
            source: diagram.source().clone(),
            start: document_range.start,
            body_start: body_range.start,
            body_end: body_range.end,
            end: document_range.end,
            text: diagram.text().clone(),
            fence_delimiter: diagram.fence_delimiter(),
            fence_delimiter_spans: diagram.fence_delimiter_spans().cloned(),
            diagram_type: syntax.diagram_type.clone(),
            text_index: syntax.text_index.clone(),
        }
    }

    pub fn source_id(&self) -> &str {
        &self.source_id
    }

    pub const fn index(&self) -> usize {
        self.index
    }

    pub fn source(&self) -> &SourceDescriptor {
        &self.source
    }

    pub fn document_range(&self) -> Range<usize> {
        self.start..self.end
    }

    pub fn body_range(&self) -> Range<usize> {
        self.body_start..self.body_end
    }

    pub fn text(&self) -> &str {
        self.text.as_str()
    }

    pub fn shared_text(&self) -> &SharedTextSlice {
        &self.text
    }

    pub const fn fence_delimiter(&self) -> Option<FenceDelimiter> {
        self.fence_delimiter
    }

    pub fn fence_delimiter_spans(&self) -> Option<&FenceDelimiterSpans> {
        self.fence_delimiter_spans.as_ref()
    }

    pub fn diagram_type(&self) -> Option<&str> {
        self.diagram_type.as_deref()
    }

    pub fn text_index(&self) -> &FenceTextIndex {
        &self.text_index
    }

    fn includes_document_offset(&self, offset: usize) -> bool {
        if offset < self.start {
            return false;
        }
        offset < self.end || (offset == self.end && self.includes_end_offset())
    }

    fn includes_end_offset(&self) -> bool {
        self.fence_delimiter.is_none() || self.end == self.body_end
    }
}
