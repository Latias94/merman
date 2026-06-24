use crate::snapshot::{DocumentSnapshot, FenceSnapshot};
use merman_analysis::{
    FenceTextIndex, SourceMap,
    lsp::{diagram_type_for_text, uri_is_markdown},
    markdown::extract_charts_with_spans,
};
use merman_core::{Engine, ParseOptions};
use std::collections::HashMap;
use tower_lsp::lsp_types::Url;

#[derive(Debug, Default)]
pub struct DocumentStore {
    documents: HashMap<Url, DocumentSnapshot>,
    engine: Engine,
}

impl DocumentStore {
    pub fn new() -> Self {
        Self {
            documents: HashMap::new(),
            engine: Engine::new(),
        }
    }

    pub fn upsert(&mut self, uri: Url, version: i32, text: String) -> DocumentSnapshot {
        let fences = if is_markdown_uri(&uri) {
            extract_charts_with_spans(&text)
                .into_iter()
                .enumerate()
                .map(|(index, chart)| {
                    let definition = chart.definition;
                    let diagram_type = diagram_type_for_text(&definition);
                    let text_index = self.text_index(&definition, diagram_type.as_deref());
                    FenceSnapshot {
                        index,
                        start: chart.start,
                        body_start: chart.body_start,
                        end: chart.end,
                        text: definition.clone(),
                        diagram_type,
                        text_index,
                    }
                })
                .collect::<Vec<_>>()
        } else {
            let diagram_type = diagram_type_for_text(&text);
            let text_index = self.text_index(&text, diagram_type.as_deref());
            vec![FenceSnapshot {
                index: 0,
                start: 0,
                body_start: 0,
                end: text.len(),
                text: text.clone(),
                diagram_type,
                text_index,
            }]
        };
        let snapshot = DocumentSnapshot {
            uri: uri.clone(),
            version,
            source_map: SourceMap::new(text.clone()),
            text,
            fences,
        };
        self.documents.insert(uri, snapshot.clone());
        snapshot
    }

    pub fn get(&self, uri: &Url) -> Option<&DocumentSnapshot> {
        self.documents.get(uri)
    }

    pub fn remove(&mut self, uri: &Url) {
        self.documents.remove(uri);
    }

    pub fn snapshots(&self) -> Vec<DocumentSnapshot> {
        self.documents.values().cloned().collect()
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
}

pub(crate) fn is_markdown_uri(uri: &Url) -> bool {
    uri_is_markdown(uri)
}
