use crate::snapshot::{DocumentSnapshot, FenceSnapshot};
use crate::types::{DocumentKind, DocumentUri};
use merman_analysis::{
    DocumentDiagram, DocumentSource, FenceTextIndex, SourceDescriptor, SourceKind,
};
use merman_core::{Engine, ParseOptions};
use std::collections::HashMap;

#[derive(Debug)]
pub struct DocumentWorkspace {
    documents: HashMap<DocumentUri, DocumentSnapshot>,
    engine: Engine,
}

impl Default for DocumentWorkspace {
    fn default() -> Self {
        Self::new()
    }
}

impl DocumentWorkspace {
    pub fn new() -> Self {
        Self {
            documents: HashMap::new(),
            engine: Engine::new(),
        }
    }

    pub fn upsert(
        &mut self,
        uri: impl Into<DocumentUri>,
        version: i32,
        text: String,
        kind: DocumentKind,
    ) -> DocumentSnapshot {
        let uri = uri.into();
        let source = source_descriptor_for_document(&uri, kind);
        let document = DocumentSource::new(text.clone(), source.clone());
        let fences = document
            .diagrams()
            .iter()
            .map(|diagram| self.fence_snapshot(diagram))
            .collect::<Vec<_>>();
        let snapshot = DocumentSnapshot {
            uri: uri.clone(),
            version,
            kind,
            source,
            text,
            source_map: document.source_map().clone(),
            fences,
        };
        self.documents.insert(uri, snapshot.clone());
        snapshot
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

    fn diagram_type_for_text(&self, text: &str) -> Option<String> {
        self.engine
            .parse_metadata_sync(text, ParseOptions::strict())
            .ok()
            .flatten()
            .map(|meta| meta.diagram_type)
    }

    fn text_index(&self, text: &str, diagram_type: Option<&str>) -> FenceTextIndex {
        if let Some(diagram_type) = diagram_type
            && let Ok(Some(facts)) = self.engine.parse_editor_semantic_facts_with_type_sync(
                diagram_type,
                text,
                ParseOptions::strict(),
            )
        {
            return FenceTextIndex::from_core_facts(facts);
        }

        FenceTextIndex::from_text(text, diagram_type)
    }

    fn fence_snapshot(&self, diagram: &DocumentDiagram) -> FenceSnapshot {
        let diagram_type = self.diagram_type_for_text(&diagram.text);
        let text_index = self.text_index(&diagram.text, diagram_type.as_deref());
        FenceSnapshot {
            source_id: diagram.id.clone(),
            index: diagram.index,
            source: diagram.source.clone(),
            start: diagram.start,
            body_start: diagram.body_start,
            body_end: diagram.body_end,
            end: diagram.end,
            text: diagram.text.clone(),
            fence_delimiter: diagram.fence_delimiter,
            diagram_type,
            text_index,
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
