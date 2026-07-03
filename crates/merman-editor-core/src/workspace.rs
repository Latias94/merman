use crate::snapshot::{DocumentSnapshot, FenceSnapshot};
use crate::types::{DocumentKind, DocumentUri};
use merman_analysis::{
    AnalyzedDiagram, Analyzer, SourceDescriptor, SourceKind, analyze_document_result,
};
use std::collections::HashMap;

#[derive(Debug)]
pub struct DocumentWorkspace {
    documents: HashMap<DocumentUri, DocumentSnapshot>,
    analyzer: Analyzer,
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
        let uri = uri.into();
        let source = source_descriptor_for_document(&uri, kind);
        let analysis = analyze_document_result(&text, analyzer, source.clone());
        let fences = analysis
            .diagrams()
            .iter()
            .map(Self::fence_snapshot)
            .collect::<Vec<_>>();
        DocumentSnapshot {
            uri,
            version,
            kind,
            source,
            text,
            source_map: analysis.source_map().clone(),
            fences,
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
