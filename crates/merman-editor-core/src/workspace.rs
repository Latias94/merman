use crate::snapshot::{DocumentSnapshot, EditorDiagramDetection};
use crate::types::{DocumentKind, DocumentUri};
use merman_analysis::{
    AnalysisCancellationToken, AnalysisCancelled, AnalysisCaptureOutcome, AnalysisGeneration,
    AnalysisPayload, AnalysisRejection, Analyzer, SourceDescriptor,
    analyze_document_generation_shared, analyze_document_generation_shared_cancellable,
};
use std::collections::HashMap;
use std::sync::Arc;

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
        self.inner.snapshot.detection()
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
        let snapshot =
            Self::capture_snapshot(&self.analyzer, uri.clone(), version, Arc::from(text), kind)?;
        self.documents.insert(uri, snapshot.clone());
        Ok(snapshot)
    }

    fn capture_snapshot(
        analyzer: &Analyzer,
        uri: DocumentUri,
        version: i32,
        text: Arc<str>,
        kind: DocumentKind,
    ) -> Result<DocumentSnapshot, AnalysisRejection> {
        let source = source_descriptor_for_document(&uri, kind);
        let analysis = analyze_document_generation_shared(text, analyzer, source);
        Self::snapshot_from_capture(version, analysis)
    }

    fn capture_snapshot_cancellable(
        analyzer: &Analyzer,
        uri: DocumentUri,
        version: i32,
        text: Arc<str>,
        kind: DocumentKind,
        cancellation: &AnalysisCancellationToken,
    ) -> Result<Result<DocumentSnapshot, AnalysisRejection>, AnalysisCancelled> {
        let source = source_descriptor_for_document(&uri, kind);
        let analysis =
            analyze_document_generation_shared_cancellable(text, analyzer, source, cancellation)?;
        cancellation.checkpoint()?;
        Ok(Self::snapshot_from_capture(version, analysis))
    }

    pub fn build_analysis_context_with_shared_text(
        analyzer: &Analyzer,
        uri: impl Into<DocumentUri>,
        version: i32,
        text: Arc<str>,
        kind: DocumentKind,
    ) -> DocumentAnalysisOutcome {
        let uri = uri.into();
        match Self::capture_snapshot(analyzer, uri, version, text, kind) {
            Ok(snapshot) => {
                DocumentAnalysisOutcome::Ready(Self::analysis_context(analyzer, snapshot))
            }
            Err(rejection) => DocumentAnalysisOutcome::Rejected(rejection),
        }
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
        let snapshot =
            Self::capture_snapshot_cancellable(analyzer, uri, version, text, kind, cancellation)?;
        match snapshot {
            Ok(snapshot) => Ok(DocumentAnalysisOutcome::Ready(
                Self::analysis_context_cancellable(analyzer, snapshot, cancellation)?,
            )),
            Err(rejection) => Ok(DocumentAnalysisOutcome::Rejected(rejection)),
        }
    }

    fn analysis_context(
        analyzer: &Analyzer,
        snapshot: DocumentSnapshot,
    ) -> DocumentAnalysisContext {
        let payload = snapshot
            .analysis_generation()
            .project(analyzer.options().diagnostic_policy());
        Self::analysis_context_ready(snapshot, payload)
    }

    fn analysis_context_cancellable(
        analyzer: &Analyzer,
        snapshot: DocumentSnapshot,
        cancellation: &AnalysisCancellationToken,
    ) -> Result<DocumentAnalysisContext, AnalysisCancelled> {
        let payload = snapshot
            .analysis_generation()
            .project_cancellable(analyzer.options().diagnostic_policy(), cancellation)?;
        Ok(Self::analysis_context_ready(snapshot, payload))
    }

    fn analysis_context_ready(
        snapshot: DocumentSnapshot,
        payload: AnalysisPayload,
    ) -> DocumentAnalysisContext {
        let payload = Arc::new(payload);
        DocumentAnalysisContext {
            inner: Arc::new(DocumentAnalysisContextInner { snapshot, payload }),
        }
    }

    fn snapshot_from_capture(
        version: i32,
        analysis: AnalysisCaptureOutcome,
    ) -> Result<DocumentSnapshot, AnalysisRejection> {
        match analysis {
            AnalysisCaptureOutcome::Ready(generation) => Ok(
                DocumentSnapshot::try_from_analysis_generation(version, Arc::new(generation))
                    .expect(
                        "workspace analysis generation must preserve the requested document URI",
                    ),
            ),
            AnalysisCaptureOutcome::Rejected(rejection) => Err(rejection),
        }
    }

    pub fn get(&self, uri: &DocumentUri) -> Option<&DocumentSnapshot> {
        self.documents.get(uri)
    }

    pub fn remove(&mut self, uri: &DocumentUri) {
        self.documents.remove(uri);
    }
}

fn source_descriptor_for_document(uri: &DocumentUri, kind: DocumentKind) -> SourceDescriptor {
    merman_analysis::source_descriptor_for_kind(Some(uri.as_str()), kind.source_kind())
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
        let projected = context
            .analysis_generation()
            .project(analyzer.options().diagnostic_policy());

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

    #[test]
    fn initial_payload_projection_propagates_cancellation_after_capture() {
        let analyzer = Analyzer::new();
        let uri = DocumentUri::new("file:///tmp/example.mmd");
        let kind = DocumentKind::Diagram;
        let analysis = analyze_document_generation_shared(
            Arc::from("flowchart TD\nA-->B\n"),
            &analyzer,
            source_descriptor_for_document(&uri, kind),
        );
        let cancellation = AnalysisCancellationToken::new();
        cancellation.cancel();

        let snapshot = DocumentWorkspace::snapshot_from_capture(1, analysis)
            .expect("capture should produce a snapshot");
        let result =
            DocumentWorkspace::analysis_context_cancellable(&analyzer, snapshot, &cancellation);

        assert_eq!(
            result.expect_err("initial payload projection should observe cancellation"),
            AnalysisCancelled
        );
    }
}
