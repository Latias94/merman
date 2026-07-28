use merman_analysis::{AnalysisPayload, AnalysisResult, Analyzer};
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
    canonical: Arc<AnalysisResult>,
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
    analysis: Option<SnapshotAnalysis>,
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
            analysis: Some(SnapshotAnalysis {
                payload,
                generation: diagnostic_generation,
            }),
            generation,
            document_epoch,
        }
    }

    pub(crate) fn analysis_payload(&self) -> Option<&AnalysisPayload> {
        self.analysis
            .as_ref()
            .map(|analysis| analysis.payload.as_ref())
    }

    pub(crate) fn analysis_generation(&self) -> Option<DiagnosticGeneration> {
        self.analysis.as_ref().map(|analysis| analysis.generation)
    }
}

impl DocumentAnalysisContext {
    pub fn from_editor(context: merman_editor_core::DocumentAnalysisContext, uri: Uri) -> Self {
        debug_assert_eq!(context.snapshot().uri().as_str(), uri.as_str());
        #[cfg(test)]
        let detection = context.detection().cloned();
        let (editor, canonical) = context.into_canonical_parts();
        let payload = Arc::new(canonical.payload().clone());
        Self {
            snapshot: Arc::new(DocumentSnapshot {
                uri,
                editor,
                #[cfg(test)]
                detection,
            }),
            canonical,
            payload,
        }
    }

    pub fn reproject(&self, analyzer: &Analyzer) -> Self {
        Self {
            snapshot: Arc::clone(&self.snapshot),
            canonical: Arc::clone(&self.canonical),
            payload: Arc::new(analyzer.reproject_payload(&self.canonical)),
        }
    }

    #[cfg(test)]
    pub fn canonical(&self) -> &Arc<AnalysisResult> {
        &self.canonical
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
    use std::str::FromStr;
    use tower_lsp_server::ls_types::{Position, Uri};

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
