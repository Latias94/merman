use crate::diagnostic_round_trip::DiagnosticRoundTrip;
use merman_analysis::{
    AnalysisCancellationToken, AnalysisCancelled, AnalysisDiagnosticPolicy, AnalysisGeneration,
    AnalysisPayload,
};
#[cfg(test)]
use merman_analysis::{
    AnalysisCaptureOutcome, Analyzer, analyze_document_generation_shared,
    source_descriptor_for_kind,
};
#[cfg(test)]
use merman_editor_core::EditorDiagramDetection;
use std::mem::size_of;
use std::str::FromStr;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use tower_lsp_server::ls_types::Uri;

#[derive(Debug, Clone)]
pub(crate) struct DocumentSnapshot {
    uri: Uri,
    editor: merman_editor_core::DocumentSnapshot,
    analysis_result_identity: AnalysisResultIdentity,
    generation_weight: usize,
    snapshot_weight: usize,
}

static NEXT_ANALYSIS_RESULT_IDENTITY: AtomicU64 = AtomicU64::new(1);

#[derive(Debug, Clone, Copy, Hash, PartialEq, Eq)]
pub(crate) struct AnalysisResultIdentity(u64);

impl AnalysisResultIdentity {
    fn issue() -> Self {
        let identity = NEXT_ANALYSIS_RESULT_IDENTITY
            .fetch_update(Ordering::Relaxed, Ordering::Relaxed, |current| {
                current.checked_add(1)
            })
            .expect("analysis result identity exhausted");
        Self(identity)
    }

    pub(crate) const fn get(self) -> u64 {
        self.0
    }

    pub(crate) const fn from_wire_value(value: u64) -> Self {
        Self(value)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct InvalidDocumentUri {
    uri: String,
}

impl std::fmt::Display for InvalidDocumentUri {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            formatter,
            "editor snapshot source path is not a valid absolute URI: {}",
            self.uri
        )
    }
}

impl std::error::Error for InvalidDocumentUri {}

/// The LSP-owned form of one editor-core analysis generation.
///
/// The snapshot and payload originate from the same cancellable analysis request and remain
/// paired while the private session cache entry is current.
#[derive(Debug)]
pub(crate) struct DocumentAnalysisContext {
    pub(crate) snapshot: Arc<DocumentSnapshot>,
    round_trip: Arc<DiagnosticRoundTrip>,
    diagnostic_generation: DiagnosticGeneration,
    owned_weight: AnalysisOwnedWeight,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct AnalysisOwnedWeight {
    pub(crate) generation: usize,
    pub(crate) projection: usize,
    pub(crate) snapshots: usize,
}

impl AnalysisOwnedWeight {
    pub(crate) fn total(self) -> usize {
        self.generation
            .saturating_add(self.projection)
            .saturating_add(self.snapshots)
    }
}

#[derive(Debug, Clone, Copy, Default, Hash, PartialEq, Eq)]
pub(crate) struct DocumentEpoch(pub(crate) u64);

#[derive(Debug, Clone, Copy, Default, Hash, PartialEq, Eq)]
pub(crate) struct SnapshotGeneration(pub(crate) u64);

#[derive(Debug, Clone, Copy, Default, Hash, PartialEq, Eq)]
pub(crate) struct DiagnosticGeneration(pub(crate) u64);

#[derive(Debug, Clone)]
pub(crate) struct SnapshotContext {
    pub(crate) snapshot: Arc<DocumentSnapshot>,
    analysis: SnapshotAnalysis,
    pub(crate) generation: SnapshotGeneration,
    pub(crate) document_epoch: DocumentEpoch,
}

#[derive(Debug, Clone)]
struct SnapshotAnalysis {
    round_trip: Arc<DiagnosticRoundTrip>,
    generation: DiagnosticGeneration,
}

impl SnapshotContext {
    pub(crate) fn with_analysis(
        snapshot: Arc<DocumentSnapshot>,
        round_trip: Arc<DiagnosticRoundTrip>,
        generation: SnapshotGeneration,
        diagnostic_generation: DiagnosticGeneration,
        document_epoch: DocumentEpoch,
    ) -> Self {
        Self {
            snapshot,
            analysis: SnapshotAnalysis {
                round_trip,
                generation: diagnostic_generation,
            },
            generation,
            document_epoch,
        }
    }

    pub(crate) fn diagnostic_generation(&self) -> DiagnosticGeneration {
        self.analysis.generation
    }

    pub(crate) fn diagnostic_round_trip(&self) -> &DiagnosticRoundTrip {
        self.analysis.round_trip.as_ref()
    }
}

impl DocumentSnapshot {
    pub(crate) fn try_from_editor(
        editor: merman_editor_core::DocumentSnapshot,
    ) -> Result<Self, InvalidDocumentUri> {
        let source_uri = editor.uri().as_str();
        let uri = Uri::from_str(source_uri).map_err(|_| InvalidDocumentUri {
            uri: source_uri.to_string(),
        })?;
        let arc_overhead = 2usize.saturating_mul(size_of::<usize>());
        let generation_weight = arc_overhead
            .saturating_add(size_of::<AnalysisGeneration>())
            .saturating_add(
                editor
                    .analysis_generation()
                    .estimated_owned_heap_bytes_excluding_source(),
            );
        let snapshot_weight = arc_overhead
            .saturating_add(size_of::<DocumentSnapshot>())
            .saturating_add(uri.as_str().len())
            .saturating_add(editor.estimated_owned_heap_bytes_excluding_shared_analysis());
        Ok(Self {
            uri,
            editor,
            analysis_result_identity: AnalysisResultIdentity::issue(),
            generation_weight,
            snapshot_weight,
        })
    }

    pub(crate) fn analysis_generation(&self) -> &AnalysisGeneration {
        self.editor.analysis_generation()
    }

    pub(crate) fn analysis_result_identity(&self) -> AnalysisResultIdentity {
        self.analysis_result_identity
    }

    pub(crate) fn estimated_owned_weight(&self) -> usize {
        self.generation_weight.saturating_add(self.snapshot_weight)
    }

    #[cfg(test)]
    pub(crate) fn shared_analysis_generation(&self) -> Arc<AnalysisGeneration> {
        self.editor.shared_analysis_generation()
    }
}

impl DocumentAnalysisContext {
    #[cfg(test)]
    pub(crate) fn from_editor(
        context: merman_editor_core::DocumentAnalysisContext,
        diagnostic_generation: DiagnosticGeneration,
    ) -> Result<Self, InvalidDocumentUri> {
        let editor = context.snapshot().clone();
        let payload = context.shared_payload();
        let snapshot = Arc::new(DocumentSnapshot::try_from_editor(editor)?);
        Ok(Self::from_projected_parts(
            snapshot,
            payload.as_ref(),
            DocumentEpoch::default(),
            diagnostic_generation,
            &AnalysisCancellationToken::new(),
        )
        .expect("a private analysis cancellation token cannot be cancelled"))
    }

    pub(crate) fn project_cancellable(
        snapshot: Arc<DocumentSnapshot>,
        policy: &AnalysisDiagnosticPolicy,
        document_epoch: DocumentEpoch,
        diagnostic_generation: DiagnosticGeneration,
        cancellation: &AnalysisCancellationToken,
    ) -> Result<Self, AnalysisCancelled> {
        let payload = snapshot
            .analysis_generation()
            .project_cancellable(policy, cancellation)?;
        cancellation.checkpoint()?;
        Self::from_projected_parts(
            snapshot,
            &payload,
            document_epoch,
            diagnostic_generation,
            cancellation,
        )
    }

    fn from_projected_parts(
        snapshot: Arc<DocumentSnapshot>,
        payload: &AnalysisPayload,
        document_epoch: DocumentEpoch,
        diagnostic_generation: DiagnosticGeneration,
        cancellation: &AnalysisCancellationToken,
    ) -> Result<Self, AnalysisCancelled> {
        let round_trip = Arc::new(DiagnosticRoundTrip::build(
            &snapshot,
            document_epoch,
            diagnostic_generation,
            payload,
            cancellation,
        )?);
        cancellation.checkpoint()?;
        let arc_overhead = 2usize.saturating_mul(size_of::<usize>());
        let owned_weight = AnalysisOwnedWeight {
            generation: snapshot.generation_weight,
            projection: arc_overhead.saturating_add(round_trip.estimated_owned_heap_bytes()),
            snapshots: arc_overhead
                .saturating_add(size_of::<DocumentAnalysisContext>())
                .saturating_add(snapshot.snapshot_weight),
        };
        Ok(Self {
            snapshot,
            round_trip,
            diagnostic_generation,
            owned_weight,
        })
    }

    pub(crate) fn diagnostic_generation(&self) -> DiagnosticGeneration {
        self.diagnostic_generation
    }

    pub(crate) fn analysis_result_identity(&self) -> AnalysisResultIdentity {
        self.snapshot.analysis_result_identity()
    }

    pub(crate) fn diagnostic_round_trip(&self) -> &Arc<DiagnosticRoundTrip> {
        &self.round_trip
    }

    pub(crate) fn estimated_owned_weight(&self) -> AnalysisOwnedWeight {
        self.owned_weight
    }

    #[cfg(test)]
    pub(crate) fn shared_analysis_generation(&self) -> Arc<AnalysisGeneration> {
        self.snapshot.shared_analysis_generation()
    }
}

#[cfg(test)]
pub(crate) fn snapshot_for_test(
    uri: Uri,
    version: i32,
    source: impl Into<Arc<str>>,
) -> Arc<DocumentSnapshot> {
    let kind = merman_editor_core::DocumentKind::from_path(uri.path().as_str());
    let source = source.into();
    let analyzer = Analyzer::new();
    let descriptor = source_descriptor_for_kind(Some(uri.as_str()), kind.source_kind());
    let generation = match analyze_document_generation_shared(source, &analyzer, descriptor) {
        AnalysisCaptureOutcome::Ready(generation) => Arc::new(generation),
        AnalysisCaptureOutcome::Rejected(rejection) => {
            panic!("test source must be within the default analysis limit: {rejection:?}")
        }
    };
    let editor =
        merman_editor_core::DocumentSnapshot::try_from_analysis_generation(version, generation)
            .expect("test analysis generation must preserve its document URI");
    Arc::new(
        DocumentSnapshot::try_from_editor(editor)
            .expect("test analysis source must contain a valid LSP URI"),
    )
}

impl DocumentSnapshot {
    pub(crate) fn uri(&self) -> &Uri {
        &self.uri
    }

    pub(crate) fn version(&self) -> i32 {
        self.editor.version()
    }

    #[cfg(test)]
    pub(crate) fn fences(&self) -> &[merman_editor_core::FenceSnapshot] {
        self.editor.fences()
    }

    pub(crate) fn as_editor(&self) -> &merman_editor_core::DocumentSnapshot {
        &self.editor
    }

    #[cfg(test)]
    pub(crate) fn detection(&self) -> Option<&EditorDiagramDetection> {
        self.editor.detection()
    }

    #[cfg(test)]
    pub(crate) fn fence_at_position(
        &self,
        position: tower_lsp_server::ls_types::Position,
    ) -> Option<&merman_editor_core::FenceSnapshot> {
        self.editor.fence_at_position(position_to_editor(position))
    }
}

#[cfg(test)]
fn position_to_editor(
    position: tower_lsp_server::ls_types::Position,
) -> merman_editor_core::Position {
    merman_editor_core::Position::new(position.line as usize, position.character as usize)
}

#[cfg(test)]
mod tests {
    use merman_analysis::{
        AnalysisCancellationToken, AnalysisCaptureOutcome, AnalysisOptions, AnalysisRuleConfig,
        Analyzer, DiagnosticSeverity as AnalysisDiagnosticSeverity, SourceKind,
        analyze_document_generation_shared, source_descriptor_for_kind,
    };
    use merman_editor_core::{
        DocumentKind, DocumentSnapshot as EditorDocumentSnapshot, DocumentWorkspace,
    };
    use std::str::FromStr;
    use std::sync::Arc;
    use tower_lsp_server::ls_types::{
        DiagnosticSeverity as LspDiagnosticSeverity, NumberOrString, Position, Uri,
    };

    #[test]
    fn lsp_context_reuses_the_editor_generation_and_text_index() {
        let uri = Uri::from_str("file:///tmp/canonical%20source.mmd").unwrap();
        let editor = DocumentWorkspace::build_analysis_context_with_shared_text(
            &Analyzer::new(),
            uri.as_str(),
            5,
            Arc::from("flowchart TD\nA-->B\n"),
            DocumentKind::Diagram,
        )
        .into_ready()
        .expect("source is within the analysis limit");
        let generation = editor.shared_analysis_generation();
        let text_index = editor.snapshot().fences()[0].text_index() as *const _;

        let lsp =
            super::DocumentAnalysisContext::from_editor(editor, super::DiagnosticGeneration(1))
                .expect("editor source must contain a valid LSP URI");

        assert!(Arc::ptr_eq(&lsp.shared_analysis_generation(), &generation));
        assert_eq!(lsp.snapshot.uri().as_str(), uri.as_str());
        assert_eq!(
            lsp.snapshot.analysis_generation().source().path.as_deref(),
            Some(uri.as_str())
        );
        assert_eq!(
            lsp.snapshot.fences()[0].text_index() as *const _,
            text_index
        );
    }

    #[test]
    fn lsp_snapshot_rejects_an_editor_source_without_an_absolute_uri() {
        let generation = match analyze_document_generation_shared(
            Arc::from("flowchart TD\nA-->B\n"),
            &Analyzer::new(),
            source_descriptor_for_kind(Some("relative/path.mmd"), SourceKind::Diagram),
        ) {
            AnalysisCaptureOutcome::Ready(generation) => Arc::new(generation),
            AnalysisCaptureOutcome::Rejected(rejection) => {
                panic!("source should be within the analysis limit: {rejection:?}")
            }
        };
        let editor = EditorDocumentSnapshot::try_from_analysis_generation(1, generation)
            .expect("editor snapshots accept protocol-neutral document paths");

        let error = super::DocumentSnapshot::try_from_editor(editor)
            .expect_err("LSP snapshots require an absolute URI");

        assert_eq!(
            error.to_string(),
            "editor snapshot source path is not a valid absolute URI: relative/path.mmd"
        );
    }

    #[test]
    fn markdown_fence_identity_and_protocol_ranges_survive_reprojection() {
        let source = concat!(
            "# Title\n\n",
            "```mermaid\n",
            "flowchart TD\n",
            "A[unterminated\n",
            "```\n\n",
            "```mermaid\n",
            "flowchart TD\n",
            "B[unterminated\n",
            "```\n",
        );
        let uri = Uri::from_str("file:///tmp/example.md").unwrap();
        let editor = DocumentWorkspace::build_analysis_context_with_shared_text(
            &Analyzer::new(),
            uri.as_str(),
            7,
            Arc::from(source),
            DocumentKind::Markdown,
        )
        .into_ready()
        .expect("source is within the analysis limit");
        let lsp =
            super::DocumentAnalysisContext::from_editor(editor, super::DiagnosticGeneration(1))
                .expect("editor source must contain a valid LSP URI");
        let fence_identity = lsp
            .snapshot
            .fences()
            .iter()
            .map(|fence| (fence.source_id().to_owned(), fence.index()))
            .collect::<Vec<_>>();
        let fence_ranges = lsp
            .snapshot
            .fences()
            .iter()
            .map(merman_editor_core::FenceSnapshot::document_range)
            .collect::<Vec<_>>();
        let initial = lsp
            .diagnostic_round_trip()
            .diagnostics_with_profile(&crate::client_profile::ClientProtocolProfile::permissive());
        let initial_parse = initial
            .iter()
            .filter(|diagnostic| {
                diagnostic.code.as_ref()
                    == Some(&NumberOrString::String(
                        "merman.parse.diagram_parse".to_string(),
                    ))
            })
            .collect::<Vec<_>>();
        assert_eq!(
            initial_parse
                .iter()
                .map(|diagnostic| diagnostic.range.start.line)
                .collect::<Vec<_>>(),
            vec![4, 9]
        );
        let initial_ranges = initial_parse
            .iter()
            .map(|diagnostic| diagnostic.range)
            .collect::<Vec<_>>();

        let analyzer = Analyzer::with_options(
            AnalysisOptions::default().with_rule_config(
                AnalysisRuleConfig::default()
                    .with_rule_severity(
                        "merman.parse.diagram_parse",
                        AnalysisDiagnosticSeverity::Hint,
                    )
                    .unwrap(),
            ),
        );
        let projected = super::DocumentAnalysisContext::project_cancellable(
            Arc::clone(&lsp.snapshot),
            analyzer.options().diagnostic_policy(),
            super::DocumentEpoch::default(),
            super::DiagnosticGeneration(2),
            &AnalysisCancellationToken::new(),
        )
        .expect("diagnostic reprojection should complete");
        let projected_protocol = projected
            .diagnostic_round_trip()
            .diagnostics_with_profile(&crate::client_profile::ClientProtocolProfile::permissive());
        let projected_parse = projected_protocol
            .iter()
            .filter(|diagnostic| {
                diagnostic.code.as_ref()
                    == Some(&NumberOrString::String(
                        "merman.parse.diagram_parse".to_string(),
                    ))
            })
            .collect::<Vec<_>>();

        assert!(Arc::ptr_eq(&projected.snapshot, &lsp.snapshot));
        assert!(Arc::ptr_eq(
            &projected.shared_analysis_generation(),
            &lsp.shared_analysis_generation()
        ));
        assert_eq!(
            projected
                .snapshot
                .fences()
                .iter()
                .map(|fence| (fence.source_id().to_owned(), fence.index()))
                .collect::<Vec<_>>(),
            fence_identity
        );
        assert_eq!(
            projected
                .snapshot
                .fences()
                .iter()
                .map(merman_editor_core::FenceSnapshot::document_range)
                .collect::<Vec<_>>(),
            fence_ranges
        );
        assert_eq!(
            projected_parse
                .iter()
                .map(|diagnostic| diagnostic.range)
                .collect::<Vec<_>>(),
            initial_ranges
        );
        assert!(
            projected_parse
                .iter()
                .all(|diagnostic| { diagnostic.severity == Some(LspDiagnosticSeverity::HINT) })
        );
    }

    #[test]
    fn fence_lookup_includes_end_position_for_completion() {
        let uri = Uri::from_str("file:///tmp/example.mmd").unwrap();
        let snapshot = super::snapshot_for_test(uri, 1, "flowchart");

        assert!(snapshot.fence_at_position(Position::new(0, 9)).is_some());
    }

    #[test]
    fn cached_snapshot_retains_detection_from_the_same_analysis() {
        let uri = Uri::from_str("file:///tmp/example.mmd").unwrap();
        let snapshot = super::snapshot_for_test(uri, 7, "flowchart TD\nA[unterminated\n");

        assert_eq!(snapshot.version(), 7);
        assert_eq!(
            snapshot
                .detection()
                .map(|detection| detection.diagram_type.as_str()),
            Some("flowchart")
        );
    }
}
