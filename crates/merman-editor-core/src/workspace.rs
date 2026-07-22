use crate::diagnostics::{EditorDiagnostic, analysis_payload_to_diagnostics};
use crate::snapshot::{DocumentSnapshot, FenceSnapshot};
use crate::types::{DocumentKind, DocumentUri};
use merman_analysis::{
    AnalysisCancellationToken, AnalysisCancelled, AnalysisPayload, AnalyzedDiagram, Analyzer,
    SourceDescriptor, SourceKind, analyze_document_result_shared,
    analyze_document_result_shared_cancellable,
};
use std::collections::HashMap;
use std::ops::Deref;
use std::sync::Arc;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DiagramDetectionValidity {
    Valid,
    RecoverableInvalid,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EditorDiagramDetection {
    pub validity: DiagramDetectionValidity,
    pub diagram_type: String,
    pub syntax_id: String,
    pub effective_layout_id: String,
}

#[derive(Debug)]
pub struct DocumentWorkspace {
    documents: HashMap<DocumentUri, DocumentSnapshot>,
    analyzer: Analyzer,
}

/// One canonical analysis generation shared by diagnostics and editor projections.
#[derive(Debug, Clone)]
pub struct DocumentAnalysisContext {
    snapshot: DocumentSnapshot,
    payload: AnalysisPayload,
    diagnostics: Arc<[EditorDiagnostic]>,
    detection: Option<EditorDiagramDetection>,
}

impl DocumentAnalysisContext {
    pub fn snapshot(&self) -> &DocumentSnapshot {
        &self.snapshot
    }

    /// Compatibility name for consumers that treat this as an analyzed snapshot.
    pub fn document(&self) -> &DocumentSnapshot {
        self.snapshot()
    }

    pub fn payload(&self) -> &AnalysisPayload {
        &self.payload
    }

    pub fn diagnostics(&self) -> &[EditorDiagnostic] {
        &self.diagnostics
    }

    pub fn detection(&self) -> Option<&EditorDiagramDetection> {
        self.detection.as_ref()
    }

    pub fn into_document(self) -> DocumentSnapshot {
        self.snapshot
    }

    pub fn into_parts(self) -> (DocumentSnapshot, AnalysisPayload) {
        (self.snapshot, self.payload)
    }
}

impl Deref for DocumentAnalysisContext {
    type Target = DocumentSnapshot;

    fn deref(&self) -> &Self::Target {
        self.snapshot()
    }
}

/// Backwards-compatible name for the canonical analysis context.
pub type AnalyzedDocumentSnapshot = DocumentAnalysisContext;

impl Default for DocumentWorkspace {
    fn default() -> Self {
        Self::new()
    }
}

impl DocumentWorkspace {
    pub fn new() -> Self {
        Self::with_analyzer(Analyzer::new())
    }

    pub fn with_analyzer(analyzer: Analyzer) -> Self {
        Self {
            documents: HashMap::new(),
            analyzer,
        }
    }

    pub fn replace_analyzer(&mut self, analyzer: Analyzer) {
        self.analyzer = analyzer;
        self.documents.clear();
    }

    pub fn upsert(
        &mut self,
        uri: impl Into<DocumentUri>,
        version: i32,
        text: String,
        kind: DocumentKind,
    ) -> DocumentSnapshot {
        let uri = uri.into();
        let snapshot = self.build_snapshot(uri.clone(), version, text, kind);
        self.documents.insert(uri, snapshot.clone());
        snapshot
    }

    pub fn build_snapshot(
        &self,
        uri: impl Into<DocumentUri>,
        version: i32,
        text: String,
        kind: DocumentKind,
    ) -> DocumentSnapshot {
        Self::build_snapshot_with_analyzer(&self.analyzer, uri, version, text, kind)
    }

    pub fn build_snapshot_with_analyzer(
        analyzer: &Analyzer,
        uri: impl Into<DocumentUri>,
        version: i32,
        text: String,
        kind: DocumentKind,
    ) -> DocumentSnapshot {
        Self::build_snapshot_with_shared_text(analyzer, uri, version, Arc::from(text), kind)
    }

    pub fn build_snapshot_with_shared_text(
        analyzer: &Analyzer,
        uri: impl Into<DocumentUri>,
        version: i32,
        text: Arc<str>,
        kind: DocumentKind,
    ) -> DocumentSnapshot {
        Self::build_analysis_context_with_shared_text(analyzer, uri, version, text, kind)
            .into_parts()
            .0
    }

    pub fn build_analyzed_snapshot_with_shared_text(
        analyzer: &Analyzer,
        uri: impl Into<DocumentUri>,
        version: i32,
        text: Arc<str>,
        kind: DocumentKind,
    ) -> AnalyzedDocumentSnapshot {
        Self::build_analysis_context_with_shared_text(analyzer, uri, version, text, kind)
    }

    pub fn build_analysis_context_with_shared_text(
        analyzer: &Analyzer,
        uri: impl Into<DocumentUri>,
        version: i32,
        text: Arc<str>,
        kind: DocumentKind,
    ) -> DocumentAnalysisContext {
        let uri = uri.into();
        let source = source_descriptor_for_document(&uri, kind);
        let analysis = analyze_document_result_shared(Arc::clone(&text), analyzer, source.clone());
        Self::analysis_context(uri, version, text, kind, source, analysis)
    }

    pub fn build_analysis_context_with_shared_text_cancellable(
        analyzer: &Analyzer,
        uri: impl Into<DocumentUri>,
        version: i32,
        text: Arc<str>,
        kind: DocumentKind,
        cancellation: &AnalysisCancellationToken,
    ) -> Result<DocumentAnalysisContext, AnalysisCancelled> {
        let uri = uri.into();
        let source = source_descriptor_for_document(&uri, kind);
        let analysis = analyze_document_result_shared_cancellable(
            Arc::clone(&text),
            analyzer,
            source.clone(),
            cancellation,
        )?;
        Ok(Self::analysis_context(
            uri, version, text, kind, source, analysis,
        ))
    }

    fn analysis_context(
        uri: DocumentUri,
        version: i32,
        text: Arc<str>,
        kind: DocumentKind,
        source: SourceDescriptor,
        analysis: merman_analysis::AnalysisResult,
    ) -> DocumentAnalysisContext {
        let diagnostics = analysis_payload_to_diagnostics(analysis.payload()).into();
        let detection = detection_for_analysis(&analysis);
        let fences = analysis
            .diagrams()
            .iter()
            .map(Self::fence_snapshot)
            .collect::<Vec<_>>();
        let source_map = analysis.source_map().clone();
        let payload = analysis.into_payload();
        DocumentAnalysisContext {
            snapshot: DocumentSnapshot {
                uri,
                version,
                kind,
                source,
                text,
                source_map,
                fences,
            },
            payload,
            diagnostics,
            detection,
        }
    }

    pub fn get(&self, uri: &DocumentUri) -> Option<&DocumentSnapshot> {
        self.documents.get(uri)
    }

    pub fn remove(&mut self, uri: &DocumentUri) {
        self.documents.remove(uri);
    }

    pub fn snapshots(&self) -> Vec<DocumentSnapshot> {
        self.documents.values().cloned().collect()
    }

    fn fence_snapshot(diagram: &AnalyzedDiagram) -> FenceSnapshot {
        FenceSnapshot {
            source_id: diagram.source_id.clone(),
            index: diagram.index,
            source: diagram.source.clone(),
            start: diagram.start,
            body_start: diagram.body_start,
            body_end: diagram.body_end,
            end: diagram.end,
            text: diagram.text.clone(),
            fence_delimiter: diagram.fence_delimiter,
            fence_delimiter_spans: diagram.fence_delimiter_spans.clone(),
            diagram_type: diagram.syntax.diagram_type.clone(),
            text_index: diagram.syntax.text_index.clone(),
        }
    }
}

fn detection_for_analysis(
    analysis: &merman_analysis::AnalysisResult,
) -> Option<EditorDiagramDetection> {
    let [diagram] = analysis.diagrams() else {
        return None;
    };
    let syntax_id = diagram.syntax.diagram_type.as_ref()?.trim();
    let effective_layout_id = diagram.syntax.effective_layout.as_ref()?.trim();
    if syntax_id.is_empty() || effective_layout_id.is_empty() {
        return None;
    }
    let diagram_type = merman_core::diagram_type_metadata_id(syntax_id)?;

    Some(EditorDiagramDetection {
        validity: if analysis.payload().valid {
            DiagramDetectionValidity::Valid
        } else {
            DiagramDetectionValidity::RecoverableInvalid
        },
        diagram_type: diagram_type.to_string(),
        syntax_id: syntax_id.to_string(),
        effective_layout_id: effective_layout_id.to_string(),
    })
}

fn source_descriptor_for_document(uri: &DocumentUri, kind: DocumentKind) -> SourceDescriptor {
    let source_kind = match kind {
        DocumentKind::Diagram => SourceKind::Diagram,
        DocumentKind::Markdown => SourceKind::Markdown,
        DocumentKind::Mdx => SourceKind::Mdx,
    };
    merman_analysis::source_descriptor_for_kind(Some(uri.as_str()), source_kind)
}
