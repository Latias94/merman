use crate::snapshot::{DocumentSnapshot, FenceSnapshot};
use crate::types::{DocumentKind, DocumentUri};
use merman_analysis::{
    AnalysisCancellationToken, AnalysisCancelled, AnalysisPayload, AnalyzedDiagram, Analyzer,
    SourceDescriptor, SourceKind, analyze_document_result_shared,
    analyze_document_result_shared_cancellable,
};
use std::collections::HashMap;
use std::sync::Arc;

#[derive(Debug)]
pub struct DocumentWorkspace {
    documents: HashMap<DocumentUri, DocumentSnapshot>,
    analyzer: Analyzer,
}

/// One rich analysis generation shared by diagnostics and editor projections.
#[derive(Debug, Clone)]
pub struct DocumentAnalysisContext {
    snapshot: DocumentSnapshot,
    payload: AnalysisPayload,
}

impl DocumentAnalysisContext {
    pub fn snapshot(&self) -> &DocumentSnapshot {
        &self.snapshot
    }

    pub fn payload(&self) -> &AnalysisPayload {
        &self.payload
    }

    pub fn into_parts(self) -> (DocumentSnapshot, AnalysisPayload) {
        (self.snapshot, self.payload)
    }
}

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
            diagram_type: diagram.syntax.diagram_type.clone(),
            text_index: diagram.syntax.text_index.clone(),
        }
    }
}

fn source_descriptor_for_document(uri: &DocumentUri, kind: DocumentKind) -> SourceDescriptor {
    let source_kind = match kind {
        DocumentKind::Diagram => SourceKind::Diagram,
        DocumentKind::Markdown => SourceKind::Markdown,
        DocumentKind::Mdx => SourceKind::Mdx,
    };
    merman_analysis::source_descriptor_for_kind(Some(uri.as_str()), source_kind)
}
