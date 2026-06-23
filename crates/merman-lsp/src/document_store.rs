use crate::snapshot::{DocumentSnapshot, FenceCompletionIndex, FenceSnapshot};
use merman_analysis::{
    SourceMap,
    lsp::{diagram_type_for_text, uri_is_markdown},
    markdown::extract_charts_with_spans,
};
use std::collections::HashMap;
use tower_lsp::lsp_types::Url;

#[derive(Debug, Default)]
pub struct DocumentStore {
    documents: HashMap<Url, DocumentSnapshot>,
}

impl DocumentStore {
    pub fn new() -> Self {
        Self {
            documents: HashMap::new(),
        }
    }

    pub fn upsert(&mut self, uri: Url, version: i32, text: String) -> DocumentSnapshot {
        let fences = if is_markdown_uri(&uri) {
            extract_charts_with_spans(&text)
                .into_iter()
                .enumerate()
                .map(|(index, chart)| {
                    let definition = chart.definition;
                    FenceSnapshot {
                        index,
                        start: chart.start,
                        body_start: chart.body_start,
                        end: chart.end,
                        text: definition.clone(),
                        diagram_type: diagram_type_for_text(&definition),
                        completion: FenceCompletionIndex::from_text(&definition),
                    }
                })
                .collect::<Vec<_>>()
        } else {
            vec![FenceSnapshot {
                index: 0,
                start: 0,
                body_start: 0,
                end: text.len(),
                text: text.clone(),
                diagram_type: diagram_type_for_text(&text),
                completion: FenceCompletionIndex::from_text(&text),
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
}

pub(crate) fn is_markdown_uri(uri: &Url) -> bool {
    uri_is_markdown(uri)
}
