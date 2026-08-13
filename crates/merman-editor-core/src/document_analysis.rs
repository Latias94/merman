use crate::snapshot::{DocumentSnapshot, EditorDiagramDetection};
use crate::types::{DocumentKind, DocumentUri};
use merman_analysis::{
    AnalysisCancellationToken, AnalysisCancelled, AnalysisCaptureOutcome, AnalysisGeneration,
    AnalysisPayload, AnalysisRejection, Analyzer, SourceDescriptor,
    analyze_document_generation_shared, analyze_document_generation_shared_cancellable,
};
use std::sync::Arc;

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

/// Builds one parser-backed editor snapshot from caller-owned shared source text.
pub fn analyze_document_snapshot_with_shared_text(
    analyzer: &Analyzer,
    uri: impl Into<DocumentUri>,
    version: i32,
    text: Arc<str>,
    kind: DocumentKind,
) -> Result<DocumentSnapshot, AnalysisRejection> {
    capture_snapshot(analyzer, uri.into(), version, text, kind)
}

/// Builds the snapshot plus initial diagnostic projection for one document generation.
pub fn analyze_document_context_with_shared_text(
    analyzer: &Analyzer,
    uri: impl Into<DocumentUri>,
    version: i32,
    text: Arc<str>,
    kind: DocumentKind,
) -> Result<DocumentAnalysisContext, AnalysisRejection> {
    let snapshot = capture_snapshot(analyzer, uri.into(), version, text, kind)?;
    Ok(analysis_context(analyzer, snapshot))
}

/// Cancellable one-shot document analysis.
///
/// Outer [`AnalysisCancelled`] means cooperative cancellation won before a ready context was
/// published. Inner [`AnalysisRejection`] means analysis completed but rejected the input.
pub fn analyze_document_context_with_shared_text_cancellable(
    analyzer: &Analyzer,
    uri: impl Into<DocumentUri>,
    version: i32,
    text: Arc<str>,
    kind: DocumentKind,
    cancellation: &AnalysisCancellationToken,
) -> Result<Result<DocumentAnalysisContext, AnalysisRejection>, AnalysisCancelled> {
    let snapshot =
        capture_snapshot_cancellable(analyzer, uri.into(), version, text, kind, cancellation)?;
    match snapshot {
        Ok(snapshot) => Ok(Ok(analysis_context_cancellable(
            analyzer,
            snapshot,
            cancellation,
        )?)),
        Err(rejection) => Ok(Err(rejection)),
    }
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
    snapshot_from_capture(version, analysis)
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
    Ok(snapshot_from_capture(version, analysis))
}

fn analysis_context(analyzer: &Analyzer, snapshot: DocumentSnapshot) -> DocumentAnalysisContext {
    let payload = snapshot
        .analysis_generation()
        .project(analyzer.options().diagnostic_policy());
    analysis_context_ready(snapshot, payload)
}

fn analysis_context_cancellable(
    analyzer: &Analyzer,
    snapshot: DocumentSnapshot,
    cancellation: &AnalysisCancellationToken,
) -> Result<DocumentAnalysisContext, AnalysisCancelled> {
    let payload = snapshot
        .analysis_generation()
        .project_cancellable(analyzer.options().diagnostic_policy(), cancellation)?;
    cancellation.checkpoint()?;
    Ok(analysis_context_ready(snapshot, payload))
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
                .expect("editor analysis generation must preserve the requested document URI"),
        ),
        AnalysisCaptureOutcome::Rejected(rejection) => Err(rejection),
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
        let context = analyze_document_context_with_shared_text(
            &Analyzer::new(),
            "file:///tmp/example.mmd",
            7,
            Arc::from("%%{ initialize: {\"theme\":\"dark\"} }%%\nflowchart TD\nA-->B\n"),
            DocumentKind::Diagram,
        )
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
        let context = analyze_document_context_with_shared_text(
            &Analyzer::new(),
            "file:///tmp/example.mmd",
            1,
            Arc::from("flowchart TD\nA-->\n"),
            DocumentKind::Diagram,
        )
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

        let snapshot =
            snapshot_from_capture(1, analysis).expect("capture should produce a snapshot");
        let result = analysis_context_cancellable(&analyzer, snapshot, &cancellation);

        assert_eq!(
            result.expect_err("initial payload projection should observe cancellation"),
            AnalysisCancelled
        );
    }

    #[test]
    fn cancellable_context_keeps_rejection_inside_the_operation_result() {
        let source = "flowchart TD\nA-->B\n";
        let analyzer = Analyzer::with_options(
            AnalysisOptions::default().with_max_source_bytes(Some(source.len() - 1)),
        );

        let result = analyze_document_context_with_shared_text_cancellable(
            &analyzer,
            "file:///tmp/limited.mmd",
            1,
            Arc::from(source),
            DocumentKind::Diagram,
            &AnalysisCancellationToken::new(),
        )
        .expect("resource rejection is not cancellation");

        assert!(result.is_err());
    }

    #[test]
    fn cancellable_context_never_publishes_after_pre_cancel() {
        let cancellation = AnalysisCancellationToken::new();
        cancellation.cancel();

        let result = analyze_document_context_with_shared_text_cancellable(
            &Analyzer::new(),
            "file:///tmp/cancelled.mmd",
            1,
            Arc::from("flowchart TD\nA-->B\n"),
            DocumentKind::Diagram,
            &cancellation,
        );

        assert_eq!(
            result.expect_err("cancellation must stay outer"),
            AnalysisCancelled
        );
    }
}
