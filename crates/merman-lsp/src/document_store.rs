use merman_analysis::{SourceMap, markdown::extract_charts_with_spans};
use std::collections::HashMap;
use std::path::Path;
use tower_lsp::lsp_types::{Position, TextDocumentIdentifier, Url};

#[derive(Debug, Clone)]
pub struct DocumentSnapshot {
    pub uri: Url,
    pub version: i32,
    pub text: String,
    pub source_map: SourceMap,
    pub fences: Vec<FenceSnapshot>,
}

#[derive(Debug, Clone)]
pub struct FenceSnapshot {
    pub index: usize,
    pub start: usize,
    pub body_start: usize,
    pub end: usize,
    pub text: String,
}

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
                .map(|(index, chart)| FenceSnapshot {
                    index,
                    start: chart.start,
                    body_start: chart.body_start,
                    end: chart.end,
                    text: chart.definition,
                })
                .collect::<Vec<_>>()
        } else {
            vec![FenceSnapshot {
                index: 0,
                start: 0,
                body_start: 0,
                end: text.len(),
                text: text.clone(),
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

impl DocumentSnapshot {
    pub fn fence_at_position(&self, position: Position) -> Option<&FenceSnapshot> {
        let offset =
            self.source_map
                .byte_offset_for_utf16_position(merman_analysis::Utf16Position {
                    line: position.line as usize,
                    character: position.character as usize,
                })?;

        self.fences
            .iter()
            .find(|fence| offset >= fence.start && offset < fence.end)
    }

    pub fn text_document_identifier(&self) -> TextDocumentIdentifier {
        TextDocumentIdentifier {
            uri: self.uri.clone(),
        }
    }

    pub fn is_markdown_document(&self) -> bool {
        is_markdown_uri(&self.uri)
    }
}

fn is_markdown_uri(uri: &Url) -> bool {
    merman_analysis::markdown::is_markdown_path(Path::new(uri.path()))
}
