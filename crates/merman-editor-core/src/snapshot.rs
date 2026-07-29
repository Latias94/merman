use crate::types::{DocumentKind, DocumentUri, Position};
use merman_analysis::{
    AnalysisGeneration, AnalyzedDiagram, FenceDelimiter, FenceDelimiterSpans, FenceTextIndex,
    SharedTextSlice, SourceDescriptor, SourceMap,
};
use std::fmt;
use std::mem::size_of;
use std::ops::Range;
use std::sync::Arc;

#[derive(Clone)]
pub struct DocumentSnapshot {
    inner: Arc<DocumentSnapshotData>,
}

struct DocumentSnapshotData {
    uri: DocumentUri,
    version: i32,
    kind: DocumentKind,
    text: Arc<str>,
    generation: Arc<AnalysisGeneration>,
    fences: Box<[FenceSnapshot]>,
}

#[derive(Clone)]
pub struct FenceSnapshot {
    generation: Arc<AnalysisGeneration>,
    diagram_ordinal: usize,
}

impl DocumentSnapshot {
    pub(crate) fn new(
        uri: DocumentUri,
        version: i32,
        kind: DocumentKind,
        generation: Arc<AnalysisGeneration>,
    ) -> Self {
        let text = generation.source_map().source_arc();
        let fences = (0..generation.diagrams().len())
            .map(|ordinal| FenceSnapshot::new(Arc::clone(&generation), ordinal))
            .collect::<Vec<_>>()
            .into_boxed_slice();
        Self {
            inner: Arc::new(DocumentSnapshotData {
                uri,
                version,
                kind,
                text,
                generation,
                fences,
            }),
        }
    }

    pub fn uri(&self) -> &DocumentUri {
        &self.inner.uri
    }

    pub fn version(&self) -> i32 {
        self.inner.version
    }

    pub fn kind(&self) -> DocumentKind {
        self.inner.kind
    }

    pub fn source(&self) -> &SourceDescriptor {
        self.inner.generation.source()
    }

    pub fn text(&self) -> &str {
        &self.inner.text
    }

    pub fn shared_text(&self) -> &Arc<str> {
        &self.inner.text
    }

    /// Estimates snapshot-owned heap excluding the shared source and analysis generation targets.
    pub fn estimated_owned_heap_bytes_excluding_shared_analysis(&self) -> usize {
        let arc_overhead = 2usize.saturating_mul(size_of::<usize>());
        arc_overhead
            .saturating_add(size_of::<DocumentSnapshotData>())
            .saturating_add(self.inner.uri.capacity())
            .saturating_add(
                self.inner
                    .fences
                    .len()
                    .saturating_mul(size_of::<FenceSnapshot>()),
            )
    }

    pub fn source_map(&self) -> &SourceMap {
        self.inner.generation.source_map()
    }

    pub fn fences(&self) -> &[FenceSnapshot] {
        &self.inner.fences
    }

    pub(crate) fn shared_analysis_generation(&self) -> Arc<AnalysisGeneration> {
        Arc::clone(&self.inner.generation)
    }

    pub(crate) fn analysis_generation(&self) -> &AnalysisGeneration {
        self.inner.generation.as_ref()
    }

    pub fn byte_offset_for_position(&self, position: Position) -> Option<usize> {
        self.source_map()
            .byte_offset_for_utf16_position(merman_analysis::Utf16Position {
                line: position.line,
                character: position.character,
            })
    }

    pub fn fence_at_position(&self, position: Position) -> Option<&FenceSnapshot> {
        let offset = self.byte_offset_for_position(position)?;

        self.fences()
            .iter()
            .find(|fence| fence.includes_document_offset(offset))
    }
}

impl fmt::Debug for DocumentSnapshot {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("DocumentSnapshot")
            .field("uri", &self.inner.uri)
            .field("version", &self.inner.version)
            .field("kind", &self.inner.kind)
            .field("text_len", &self.inner.text.len())
            .field("fence_count", &self.inner.fences.len())
            .finish()
    }
}

impl FenceSnapshot {
    fn new(generation: Arc<AnalysisGeneration>, diagram_ordinal: usize) -> Self {
        Self {
            generation,
            diagram_ordinal,
        }
    }

    fn diagram(&self) -> &AnalyzedDiagram {
        self.generation
            .diagrams()
            .get(self.diagram_ordinal)
            .expect("fence diagram ordinal must belong to its analysis generation")
    }

    pub fn source_id(&self) -> &str {
        self.diagram().source_id()
    }

    pub fn index(&self) -> usize {
        self.diagram().index()
    }

    pub fn source(&self) -> &SourceDescriptor {
        self.diagram().source()
    }

    pub fn document_range(&self) -> Range<usize> {
        self.diagram().document_range()
    }

    pub fn body_range(&self) -> Range<usize> {
        self.diagram().body_range()
    }

    pub fn text(&self) -> &str {
        self.diagram().text().as_str()
    }

    pub fn shared_text(&self) -> &SharedTextSlice {
        self.diagram().text()
    }

    pub fn fence_delimiter(&self) -> Option<FenceDelimiter> {
        self.diagram().fence_delimiter()
    }

    pub fn fence_delimiter_spans(&self) -> Option<&FenceDelimiterSpans> {
        self.diagram().fence_delimiter_spans()
    }

    pub fn diagram_type(&self) -> Option<&str> {
        self.diagram().syntax().diagram_type.as_deref()
    }

    pub fn text_index(&self) -> &FenceTextIndex {
        &self.diagram().syntax().text_index
    }

    fn includes_document_offset(&self, offset: usize) -> bool {
        let document_range = self.diagram().document_range();
        if offset < document_range.start {
            return false;
        }
        offset < document_range.end || (offset == document_range.end && self.includes_end_offset())
    }

    fn includes_end_offset(&self) -> bool {
        let diagram = self.diagram();
        diagram.fence_delimiter().is_none()
            || diagram.document_range().end == diagram.body_range().end
    }
}

impl fmt::Debug for FenceSnapshot {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("FenceSnapshot")
            .field("diagram_ordinal", &self.diagram_ordinal)
            .field("source_id", &self.source_id())
            .field("index", &self.index())
            .field("document_range", &self.document_range())
            .field("diagram_type", &self.diagram_type())
            .finish()
    }
}
