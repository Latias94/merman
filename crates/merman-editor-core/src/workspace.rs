use crate::snapshot::DocumentSnapshot;
use crate::types::{DocumentKind, DocumentUri};
use merman_analysis::{
    AnalysisCancellationToken, AnalysisCancelled, AnalysisCaptureOutcome, AnalysisDiagnosticPolicy,
    AnalysisGeneration, AnalysisPayload, AnalysisRejection, Analyzer, DiagramParseDisposition,
    SourceDescriptor, SourceKind, analyze_document_generation_shared,
    analyze_document_generation_shared_cancellable,
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
    inner: Arc<DocumentAnalysisContextInner>,
}

#[derive(Debug, Clone)]
struct DocumentAnalysisContextInner {
    snapshot: DocumentSnapshot,
    payload: Arc<AnalysisPayload>,
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
        self.inner.payload.as_ref()
    }

    pub fn detection(&self) -> Option<&EditorDiagramDetection> {
        self.inner.detection.as_ref()
    }

    pub fn analysis_generation(&self) -> &AnalysisGeneration {
        self.inner.snapshot.analysis_generation()
    }

    pub fn shared_analysis_generation(&self) -> Arc<AnalysisGeneration> {
        self.inner.snapshot.shared_analysis_generation()
    }

    pub fn shared_payload(&self) -> Arc<AnalysisPayload> {
        Arc::clone(&self.inner.payload)
    }

    pub fn reproject_payload(&self, policy: &AnalysisDiagnosticPolicy) -> AnalysisPayload {
        self.analysis_generation().project(policy)
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
        let analyzed = Self::build_analysis_context_with_shared_text(
            &self.analyzer,
            uri.clone(),
            version,
            Arc::from(text),
            kind,
        )
        .into_ready()?;
        let snapshot = analyzed.snapshot().clone();
        self.documents.insert(uri, snapshot.clone());
        Ok(snapshot)
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
        let analysis = analyze_document_generation_shared(text, analyzer, source);
        Self::analysis_context(uri, version, kind, analyzer, analysis)
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
        let analysis =
            analyze_document_generation_shared_cancellable(text, analyzer, source, cancellation)?;
        Ok(Self::analysis_context(
            uri, version, kind, analyzer, analysis,
        ))
    }

    fn analysis_context(
        uri: DocumentUri,
        version: i32,
        kind: DocumentKind,
        analyzer: &Analyzer,
        analysis: AnalysisCaptureOutcome,
    ) -> DocumentAnalysisOutcome {
        match analysis {
            AnalysisCaptureOutcome::Ready(generation) => DocumentAnalysisOutcome::Ready(
                Self::analysis_context_ready(uri, version, kind, analyzer, generation),
            ),
            AnalysisCaptureOutcome::Rejected(rejection) => {
                DocumentAnalysisOutcome::Rejected(rejection)
            }
        }
    }

    fn analysis_context_ready(
        uri: DocumentUri,
        version: i32,
        kind: DocumentKind,
        analyzer: &Analyzer,
        generation: AnalysisGeneration,
    ) -> DocumentAnalysisContext {
        let payload = Arc::new(generation.project(analyzer.options().diagnostic_policy()));
        let detection = detection_for_generation(&generation);
        let generation = Arc::new(generation);
        DocumentAnalysisContext {
            inner: Arc::new(DocumentAnalysisContextInner {
                snapshot: DocumentSnapshot::new(uri, version, kind, generation),
                payload,
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
}

fn detection_for_generation(
    generation: &merman_analysis::AnalysisGeneration,
) -> Option<EditorDiagramDetection> {
    let [diagram] = generation.diagrams() else {
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
    use crate::diagnostics::{
        analysis_payload_to_diagnostics, diagnostic_projection_count,
        reset_diagnostic_projection_count,
    };
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
        let analysis = context.analysis_generation() as *const AnalysisGeneration;
        let analyzer = Analyzer::with_options(
            AnalysisOptions::default().with_rule_config(
                AnalysisRuleConfig::default()
                    .with_profile(AnalysisRuleProfile::Recommended)
                    .with_rule_disabled("merman.authoring.config.prefer_frontmatter_config")
                    .unwrap(),
            ),
        );

        let initial_payload = context.shared_payload();
        let projected = context.reproject_payload(analyzer.options().diagnostic_policy());

        assert_eq!(context.snapshot() as *const DocumentSnapshot, snapshot);
        assert_eq!(
            context.analysis_generation() as *const AnalysisGeneration,
            analysis
        );
        assert!(std::ptr::eq(context.payload(), initial_payload.as_ref()));
        assert!(projected.diagnostics.iter().any(|diagnostic| {
            diagnostic.id == "merman.authoring.config.prefer_init_directive"
        }));
    }

    #[test]
    fn canonical_context_defers_editor_diagnostic_projection() {
        reset_diagnostic_projection_count();
        let context = DocumentWorkspace::build_analysis_context_with_shared_text(
            &Analyzer::new(),
            "file:///tmp/example.mmd",
            1,
            Arc::from("flowchart TD\nA-->\n"),
            DocumentKind::Diagram,
        )
        .into_ready()
        .expect("source is within the analysis limit");

        assert_eq!(diagnostic_projection_count(), 0);
        assert!(!analysis_payload_to_diagnostics(context.payload()).is_empty());
        assert_eq!(diagnostic_projection_count(), 1);
    }
}
