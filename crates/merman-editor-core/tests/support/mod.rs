use merman_analysis::{AnalysisRejection, Analyzer};
use merman_editor_core::{
    DocumentKind, DocumentSnapshot, DocumentUri, analyze_document_snapshot_with_shared_text,
};
use std::sync::Arc;

pub struct SnapshotHarness {
    analyzer: Analyzer,
}

impl SnapshotHarness {
    pub fn new() -> Self {
        Self {
            analyzer: Analyzer::new(),
        }
    }

    pub fn analyze(
        &self,
        uri: impl Into<DocumentUri>,
        version: i32,
        text: impl Into<Arc<str>>,
        kind: DocumentKind,
    ) -> Result<DocumentSnapshot, AnalysisRejection> {
        analyze_document_snapshot_with_shared_text(&self.analyzer, uri, version, text.into(), kind)
    }
}
