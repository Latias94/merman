use crate::diagnostics::{EditorDiagnostic, analysis_payload_to_diagnostics};
use crate::snapshot::{DocumentSnapshot, FenceSnapshot};
use crate::types::{DocumentKind, DocumentUri};
use merman_analysis::{
    AnalysisCancellationToken, AnalysisCancelled, AnalysisOutcome, AnalysisPayload,
    AnalysisRejection, AnalysisResult, AnalyzedDiagram, Analyzer, DiagramParseDisposition,
    SourceDescriptor, SourceKind, analyze_document_result_shared,
    analyze_document_result_shared_cancellable,
};
use std::collections::HashMap;
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
    inner: Box<DocumentAnalysisContextInner>,
}

#[derive(Debug, Clone)]
struct DocumentAnalysisContextInner {
    snapshot: DocumentSnapshot,
    analysis: Arc<AnalysisResult>,
    diagnostics: Arc<[EditorDiagnostic]>,
    detection: Option<EditorDiagramDetection>,
}

#[derive(Debug, Clone)]
pub enum DocumentAnalysisOutcome {
    Ready(DocumentAnalysisContext),
    Rejected(AnalysisRejection),
}

impl DocumentAnalysisOutcome {
    pub fn as_ready(&self) -> Option<&DocumentAnalysisContext> {
        match self {
            Self::Ready(context) => Some(context),
            Self::Rejected(_) => None,
        }
    }

    pub fn into_ready(self) -> Result<DocumentAnalysisContext, AnalysisRejection> {
        match self {
            Self::Ready(context) => Ok(context),
            Self::Rejected(rejection) => Err(rejection),
        }
    }

    pub fn rejection(&self) -> Option<&AnalysisRejection> {
        match self {
            Self::Ready(_) => None,
            Self::Rejected(rejection) => Some(rejection),
        }
    }
}

impl DocumentAnalysisContext {
    pub fn snapshot(&self) -> &DocumentSnapshot {
        &self.inner.snapshot
    }

    pub fn payload(&self) -> &AnalysisPayload {
        self.inner.analysis.payload()
    }

    pub fn diagnostics(&self) -> &[EditorDiagnostic] {
        &self.inner.diagnostics
    }

    pub fn detection(&self) -> Option<&EditorDiagramDetection> {
        self.inner.detection.as_ref()
    }

    pub fn analysis_result(&self) -> &AnalysisResult {
        &self.inner.analysis
    }

    pub fn shared_analysis_result(&self) -> Arc<AnalysisResult> {
        Arc::clone(&self.inner.analysis)
    }

    pub fn reproject_payload(&self, analyzer: &Analyzer) -> AnalysisPayload {
        analyzer.reproject_payload(&self.inner.analysis)
    }

    pub fn reproject_diagnostics(&self, analyzer: &Analyzer) -> Arc<[EditorDiagnostic]> {
        analysis_payload_to_diagnostics(&self.reproject_payload(analyzer)).into()
    }

    pub fn into_parts(self) -> (DocumentSnapshot, AnalysisPayload) {
        let inner = *self.inner;
        (inner.snapshot, inner.analysis.payload().clone())
    }

    pub fn into_canonical_parts(self) -> (DocumentSnapshot, Arc<AnalysisResult>) {
        let inner = *self.inner;
        (inner.snapshot, inner.analysis)
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
    ) -> Result<DocumentSnapshot, AnalysisRejection> {
        let uri = uri.into();
        let snapshot = Self::build_analysis_context_with_shared_text(
            &self.analyzer,
            uri.clone(),
            version,
            Arc::from(text),
            kind,
        )
        .into_ready()?
        .into_parts()
        .0;
        self.documents.insert(uri, snapshot.clone());
        Ok(snapshot)
    }

    pub fn build_snapshot(
        &self,
        uri: impl Into<DocumentUri>,
        version: i32,
        text: String,
        kind: DocumentKind,
    ) -> Result<DocumentSnapshot, AnalysisRejection> {
        Self::build_snapshot_with_analyzer(&self.analyzer, uri, version, text, kind)
    }

    pub fn build_snapshot_with_analyzer(
        analyzer: &Analyzer,
        uri: impl Into<DocumentUri>,
        version: i32,
        text: String,
        kind: DocumentKind,
    ) -> Result<DocumentSnapshot, AnalysisRejection> {
        Self::build_snapshot_with_shared_text(analyzer, uri, version, Arc::from(text), kind)
    }

    pub fn build_snapshot_with_shared_text(
        analyzer: &Analyzer,
        uri: impl Into<DocumentUri>,
        version: i32,
        text: Arc<str>,
        kind: DocumentKind,
    ) -> Result<DocumentSnapshot, AnalysisRejection> {
        Self::build_analysis_context_with_shared_text(analyzer, uri, version, text, kind)
            .into_ready()
            .map(|context| context.into_parts().0)
    }

    pub fn build_analysis_context_with_shared_text(
        analyzer: &Analyzer,
        uri: impl Into<DocumentUri>,
        version: i32,
        text: Arc<str>,
        kind: DocumentKind,
    ) -> DocumentAnalysisOutcome {
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
    ) -> Result<DocumentAnalysisOutcome, AnalysisCancelled> {
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
        analysis: AnalysisOutcome,
    ) -> DocumentAnalysisOutcome {
        match analysis {
            AnalysisOutcome::Ready(analysis) => DocumentAnalysisOutcome::Ready(
                Self::analysis_context_ready(uri, version, text, kind, source, analysis),
            ),
            AnalysisOutcome::Rejected(rejection) => DocumentAnalysisOutcome::Rejected(rejection),
        }
    }

    fn analysis_context_ready(
        uri: DocumentUri,
        version: i32,
        text: Arc<str>,
        kind: DocumentKind,
        source: SourceDescriptor,
        analysis: AnalysisResult,
    ) -> DocumentAnalysisContext {
        let diagnostics = analysis_payload_to_diagnostics(analysis.payload()).into();
        let detection = detection_for_analysis(&analysis);
        let fences = analysis
            .diagrams()
            .iter()
            .map(Self::fence_snapshot)
            .collect::<Vec<_>>();
        let source_map = analysis.source_map().clone();
        let analysis = Arc::new(analysis);
        DocumentAnalysisContext {
            inner: Box::new(DocumentAnalysisContextInner {
                snapshot: DocumentSnapshot::new(
                    uri, version, kind, source, text, source_map, fences,
                ),
                analysis,
                diagnostics,
                detection,
            }),
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
        FenceSnapshot::from_analyzed_diagram(diagram)
    }
}

fn detection_for_analysis(
    analysis: &merman_analysis::AnalysisResult,
) -> Option<EditorDiagramDetection> {
    let [diagram] = analysis.diagrams() else {
        return None;
    };
    let syntax_id = diagram.syntax().diagram_type.as_ref()?.trim();
    let effective_layout_id = diagram.syntax().effective_layout.as_ref()?.trim();
    if syntax_id.is_empty() || effective_layout_id.is_empty() {
        return None;
    }
    let diagram_type = merman_core::diagram_type_metadata_id(syntax_id)?;
    let validity = match diagram.parse_disposition() {
        DiagramParseDisposition::Parsed => DiagramDetectionValidity::Valid,
        DiagramParseDisposition::Recovered => DiagramDetectionValidity::RecoverableInvalid,
        DiagramParseDisposition::Unavailable => return None,
    };

    Some(EditorDiagramDetection {
        validity,
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

#[cfg(test)]
mod tests {
    use super::*;
    use merman_analysis::{AnalysisOptions, AnalysisRuleConfig, AnalysisRuleProfile};

    #[test]
    fn diagnostic_reprojection_keeps_the_canonical_snapshot_generation() {
        let context = DocumentWorkspace::build_analysis_context_with_shared_text(
            &Analyzer::new(),
            "file:///tmp/example.mmd",
            7,
            Arc::from("%%{ initialize: {\"theme\":\"dark\"} }%%\nflowchart TD\nA-->B\n"),
            DocumentKind::Diagram,
        )
        .into_ready()
        .expect("source is within the analysis limit");
        let snapshot = context.snapshot() as *const DocumentSnapshot;
        let analysis = context.analysis_result() as *const AnalysisResult;
        let analyzer = Analyzer::with_options(
            AnalysisOptions::default().with_rule_config(
                AnalysisRuleConfig::default()
                    .with_profile(AnalysisRuleProfile::Recommended)
                    .with_rule_disabled("merman.authoring.config.prefer_frontmatter_config")
                    .unwrap(),
            ),
        );

        let diagnostics = context.reproject_diagnostics(&analyzer);

        assert_eq!(context.snapshot() as *const DocumentSnapshot, snapshot);
        assert_eq!(context.analysis_result() as *const AnalysisResult, analysis);
        assert!(diagnostics.iter().any(|diagnostic| {
            diagnostic.code == "merman.authoring.config.prefer_init_directive"
        }));
    }
}
