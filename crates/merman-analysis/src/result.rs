use crate::{
    AnalysisDiagnostic, AnalysisPayload, DocumentDiagram, DocumentDiagramKind, FenceDelimiter,
    FenceTextIndex, FenceTextIndexSource, SourceDescriptor, SourceMap,
};

#[derive(Debug, Clone)]
pub struct AnalysisResult {
    payload: AnalysisPayload,
    source_map: SourceMap,
    diagrams: Vec<AnalyzedDiagram>,
}

impl AnalysisResult {
    pub fn new(
        source: SourceDescriptor,
        source_map: SourceMap,
        diagnostics: Vec<AnalysisDiagnostic>,
        diagrams: Vec<AnalyzedDiagram>,
    ) -> Self {
        Self {
            payload: AnalysisPayload::new(source, diagnostics),
            source_map,
            diagrams,
        }
    }

    pub fn payload(&self) -> &AnalysisPayload {
        &self.payload
    }

    pub fn into_payload(self) -> AnalysisPayload {
        self.payload
    }

    pub fn source_map(&self) -> &SourceMap {
        &self.source_map
    }

    pub fn diagrams(&self) -> &[AnalyzedDiagram] {
        &self.diagrams
    }

    pub fn diagnostics(&self) -> &[AnalysisDiagnostic] {
        &self.payload.diagnostics
    }
}

#[derive(Debug, Clone)]
pub struct AnalyzedDiagram {
    pub source_id: String,
    pub index: usize,
    pub kind: DocumentDiagramKind,
    pub source: SourceDescriptor,
    pub start: usize,
    pub body_start: usize,
    pub body_end: usize,
    pub end: usize,
    pub text: String,
    pub fence_delimiter: Option<FenceDelimiter>,
    pub diagnostics: Vec<AnalysisDiagnostic>,
    pub syntax: AnalysisSyntaxFacts,
}

impl AnalyzedDiagram {
    pub fn from_document_diagram(
        diagram: &DocumentDiagram,
        diagnostics: Vec<AnalysisDiagnostic>,
        syntax: AnalysisSyntaxFacts,
    ) -> Self {
        Self {
            source_id: diagram.id.clone(),
            index: diagram.index,
            kind: diagram.kind,
            source: diagram.source.clone(),
            start: diagram.start,
            body_start: diagram.body_start,
            body_end: diagram.body_end,
            end: diagram.end,
            text: diagram.text.clone(),
            fence_delimiter: diagram.fence_delimiter,
            diagnostics,
            syntax,
        }
    }
}

#[derive(Debug, Clone)]
pub struct AnalysisSyntaxFacts {
    pub diagram_type: Option<String>,
    pub text_index: FenceTextIndex,
}

impl AnalysisSyntaxFacts {
    pub fn new(diagram_type: Option<String>, text_index: FenceTextIndex) -> Self {
        Self {
            diagram_type,
            text_index,
        }
    }

    pub fn text_scan(text: &str, diagram_type: Option<String>) -> Self {
        Self {
            text_index: FenceTextIndex::from_text(text, diagram_type.as_deref()),
            diagram_type,
        }
    }

    pub fn source(&self) -> FenceTextIndexSource {
        self.text_index.source()
    }
}
