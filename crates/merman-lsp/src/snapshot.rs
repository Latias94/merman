use merman_analysis::{
    AnalysisCancellationToken, AnalysisCancelled, AnalysisGeneration, AnalysisPayload, Analyzer,
};
#[cfg(test)]
use merman_editor_core::EditorDiagramDetection;
use std::sync::Arc;
use tower_lsp_server::ls_types::Uri;

#[derive(Debug, Clone)]
pub struct DocumentSnapshot {
    uri: Uri,
    editor: merman_editor_core::DocumentSnapshot,
    #[cfg(test)]
    detection: Option<EditorDiagramDetection>,
}

/// The LSP-owned form of one editor-core analysis generation.
///
/// The snapshot and payload originate from the same cancellable analysis request and remain
/// paired while the document-store cache is current.
#[derive(Debug)]
pub struct DocumentAnalysisContext {
    pub snapshot: Arc<DocumentSnapshot>,
    generation: Arc<AnalysisGeneration>,
    pub payload: Arc<AnalysisPayload>,
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
    payload: Arc<AnalysisPayload>,
    generation: DiagnosticGeneration,
}

impl SnapshotContext {
    pub(crate) fn with_analysis(
        snapshot: Arc<DocumentSnapshot>,
        payload: Arc<AnalysisPayload>,
        generation: SnapshotGeneration,
        diagnostic_generation: DiagnosticGeneration,
        document_epoch: DocumentEpoch,
    ) -> Self {
        Self {
            snapshot,
            analysis: SnapshotAnalysis {
                payload,
                generation: diagnostic_generation,
            },
            generation,
            document_epoch,
        }
    }

    pub(crate) fn analysis_payload(&self) -> &AnalysisPayload {
        self.analysis.payload.as_ref()
    }

    pub(crate) fn diagnostic_generation(&self) -> DiagnosticGeneration {
        self.analysis.generation
    }
}

impl DocumentAnalysisContext {
    pub fn from_editor(context: merman_editor_core::DocumentAnalysisContext, uri: Uri) -> Self {
        debug_assert_eq!(context.snapshot().uri().as_str(), uri.as_str());
        #[cfg(test)]
        let detection = context.detection().cloned();
        let editor = context.snapshot().clone();
        let generation = context.shared_analysis_generation();
        let payload = context.shared_payload();
        Self {
            snapshot: Arc::new(DocumentSnapshot {
                uri,
                editor,
                #[cfg(test)]
                detection,
            }),
            generation,
            payload,
        }
    }

    pub fn reproject_cancellable(
        &self,
        analyzer: &Analyzer,
        cancellation: &AnalysisCancellationToken,
    ) -> Result<Self, AnalysisCancelled> {
        let payload = Arc::new(
            self.generation
                .project_cancellable(analyzer.options().diagnostic_policy(), cancellation)?,
        );
        cancellation.checkpoint()?;
        Ok(Self {
            snapshot: Arc::clone(&self.snapshot),
            generation: Arc::clone(&self.generation),
            payload,
        })
    }

    #[cfg(test)]
    pub fn generation(&self) -> &Arc<AnalysisGeneration> {
        &self.generation
    }
}

impl DocumentSnapshot {
    pub fn uri(&self) -> &Uri {
        &self.uri
    }

    pub fn version(&self) -> i32 {
        self.editor.version()
    }

    #[cfg(test)]
    pub fn kind(&self) -> merman_editor_core::DocumentKind {
        self.editor.kind()
    }

    #[cfg(test)]
    pub fn fences(&self) -> &[merman_editor_core::FenceSnapshot] {
        self.editor.fences()
    }

    pub fn as_editor(&self) -> &merman_editor_core::DocumentSnapshot {
        &self.editor
    }

    #[cfg(test)]
    pub fn detection(&self) -> Option<&EditorDiagramDetection> {
        self.detection.as_ref()
    }

    #[cfg(test)]
    pub fn fence_at_position(
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
        AnalysisCancellationToken, AnalysisOptions, AnalysisRuleConfig, Analyzer,
        DiagnosticSeverity as AnalysisDiagnosticSeverity,
    };
    use merman_editor_core::{DocumentKind, DocumentWorkspace};
    use std::str::FromStr;
    use std::sync::Arc;
    use tower_lsp_server::ls_types::{
        DiagnosticSeverity as LspDiagnosticSeverity, NumberOrString, Position, Uri,
    };

    #[test]
    fn lsp_context_reuses_the_editor_generation_payload_and_text_index() {
        let uri = Uri::from_str("file:///tmp/example.mmd").unwrap();
        let editor = DocumentWorkspace::build_analysis_context_with_shared_text(
            &Analyzer::new(),
            uri.as_str(),
            5,
            Arc::from("flowchart TD\nA-->B\n"),
            DocumentKind::Diagram,
        )
        .into_ready()
        .expect("source is within the analysis limit");
        let payload = editor.shared_payload();
        let generation = editor.shared_analysis_generation();
        let text_index = editor.snapshot().fences()[0].text_index() as *const _;

        let lsp = super::DocumentAnalysisContext::from_editor(editor, uri);

        assert!(Arc::ptr_eq(&lsp.payload, &payload));
        assert!(Arc::ptr_eq(lsp.generation(), &generation));
        assert_eq!(
            lsp.snapshot.fences()[0].text_index() as *const _,
            text_index
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
        let lsp = super::DocumentAnalysisContext::from_editor(editor, uri.clone());
        let fence_ids = lsp
            .snapshot
            .fences()
            .iter()
            .map(merman_editor_core::FenceSnapshot::diagram_id)
            .collect::<Vec<_>>();
        let fence_ranges = lsp
            .snapshot
            .fences()
            .iter()
            .map(merman_editor_core::FenceSnapshot::document_range)
            .collect::<Vec<_>>();
        let initial = crate::diagnostics::analysis_payload_to_diagnostics(&lsp.payload, &uri);
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
        let projected = lsp
            .reproject_cancellable(&analyzer, &AnalysisCancellationToken::new())
            .expect("diagnostic reprojection should complete");
        let projected_protocol =
            crate::diagnostics::analysis_payload_to_diagnostics(&projected.payload, &uri);
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
        assert!(Arc::ptr_eq(projected.generation(), lsp.generation()));
        assert_eq!(
            projected
                .snapshot
                .fences()
                .iter()
                .map(merman_editor_core::FenceSnapshot::diagram_id)
                .collect::<Vec<_>>(),
            fence_ids
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
        let mut store = crate::document_store::DocumentStore::new();
        let uri = Uri::from_str("file:///tmp/example.mmd").unwrap();
        let snapshot = store.upsert(uri, 1, "flowchart".to_string());

        assert!(snapshot.fence_at_position(Position::new(0, 9)).is_some());
    }

    #[test]
    fn cached_snapshot_retains_detection_from_the_same_analysis() {
        let mut store = crate::document_store::DocumentStore::new();
        let uri = Uri::from_str("file:///tmp/example.mmd").unwrap();
        let snapshot = store.upsert(uri, 7, "flowchart TD\nA[unterminated\n".to_string());

        assert_eq!(snapshot.version(), 7);
        assert_eq!(
            snapshot
                .detection()
                .map(|detection| detection.diagram_type.as_str()),
            Some("flowchart")
        );
    }
}
