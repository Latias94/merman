use crate::types::{DocumentKind, DocumentUri, Position};
use merman_analysis::{
    AnalysisDiagramId, AnalysisGeneration, AnalyzedDiagram, FenceDelimiter, FenceDelimiterSpans,
    FenceTextIndex, SharedTextSlice, SourceDescriptor, SourceMap,
};
use std::ops::Range;
use std::sync::Arc;

#[derive(Debug, Clone)]
pub struct DocumentSnapshot {
    inner: Arc<DocumentSnapshotData>,
}

#[derive(Debug)]
struct DocumentSnapshotData {
    uri: DocumentUri,
    version: i32,
    kind: DocumentKind,
    text: Arc<str>,
    generation: Arc<AnalysisGeneration>,
    fences: Box<[FenceSnapshot]>,
}

#[derive(Debug, Clone)]
pub struct FenceSnapshot {
    generation: Arc<AnalysisGeneration>,
    diagram_id: AnalysisDiagramId,
}

impl DocumentSnapshot {
    pub(crate) fn new(
        uri: DocumentUri,
        version: i32,
        kind: DocumentKind,
        generation: Arc<AnalysisGeneration>,
    ) -> Self {
        let text = generation.source_map().source_arc();
        let fences = generation
            .diagram_ids()
            .map(|diagram_id| FenceSnapshot::new(Arc::clone(&generation), diagram_id))
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
        &self.inner.generation.snapshot_policy().source
    }

    pub fn text(&self) -> &str {
        &self.inner.text
    }

    pub fn shared_text(&self) -> &Arc<str> {
        &self.inner.text
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

impl FenceSnapshot {
    fn new(generation: Arc<AnalysisGeneration>, diagram_id: AnalysisDiagramId) -> Self {
        Self {
            generation,
            diagram_id,
        }
    }

    fn diagram(&self) -> &AnalyzedDiagram {
        self.generation
            .diagram(self.diagram_id)
            .expect("fence diagram id must belong to its analysis generation")
    }

    pub const fn diagram_id(&self) -> AnalysisDiagramId {
        self.diagram_id
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
