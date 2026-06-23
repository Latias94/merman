use merman_analysis::SourceMap;
use std::collections::BTreeSet;
use tower_lsp::lsp_types::{Position, Url};

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
    pub completion: FenceCompletionIndex,
}

#[derive(Debug, Clone, Default)]
pub struct FenceCompletionIndex {
    node_ids: BTreeSet<String>,
}

impl FenceCompletionIndex {
    pub fn from_text(text: &str) -> Self {
        Self {
            node_ids: node_ids(text),
        }
    }

    pub fn node_ids(&self) -> impl Iterator<Item = &String> {
        self.node_ids.iter()
    }
}

impl DocumentSnapshot {
    pub fn byte_offset_for_position(&self, position: Position) -> Option<usize> {
        self.source_map
            .byte_offset_for_utf16_position(merman_analysis::Utf16Position {
                line: position.line as usize,
                character: position.character as usize,
            })
    }

    pub fn fence_at_position(&self, position: Position) -> Option<&FenceSnapshot> {
        let offset = self.byte_offset_for_position(position)?;

        self.fences
            .iter()
            .find(|fence| offset >= fence.start && offset <= fence.end)
    }
}

fn node_ids(text: &str) -> BTreeSet<String> {
    text.lines()
        .flat_map(|line| {
            line.split(|ch: char| {
                ch.is_whitespace()
                    || matches!(
                        ch,
                        '[' | ']'
                            | '('
                            | ')'
                            | '{'
                            | '}'
                            | '-'
                            | '='
                            | '.'
                            | '<'
                            | '>'
                            | '|'
                            | ':'
                            | ','
                            | ';'
                    )
            })
        })
        .filter(|token| is_candidate_node_id(token))
        .map(ToString::to_string)
        .collect()
}

fn is_candidate_node_id(token: &str) -> bool {
    if token.is_empty() || token.starts_with('%') {
        return false;
    }

    !matches!(
        token,
        "flowchart"
            | "graph"
            | "sequenceDiagram"
            | "stateDiagram"
            | "stateDiagram-v2"
            | "mindmap"
            | "TD"
            | "TB"
            | "BT"
            | "LR"
            | "RL"
            | "classDef"
            | "class"
            | "style"
            | "linkStyle"
    )
}

#[cfg(test)]
mod tests {
    use super::{FenceCompletionIndex, is_candidate_node_id};
    use tower_lsp::lsp_types::{Position, Url};

    #[test]
    fn completion_index_collects_node_ids() {
        let index = FenceCompletionIndex::from_text("flowchart TD\nA-->B\nB-->C\n");
        let ids = index.node_ids().cloned().collect::<Vec<_>>();

        assert_eq!(ids, vec!["A", "B", "C"]);
    }

    #[test]
    fn node_id_filter_skips_keywords_and_empty_tokens() {
        assert!(!is_candidate_node_id("flowchart"));
        assert!(!is_candidate_node_id("%comment"));
        assert!(is_candidate_node_id("node_1"));
    }

    #[test]
    fn fence_lookup_includes_end_position_for_completion() {
        let mut store = crate::document_store::DocumentStore::new();
        let uri = Url::parse("file:///tmp/example.mmd").unwrap();
        let snapshot = store.upsert(uri, 1, "flowchart".to_string());

        assert!(snapshot.fence_at_position(Position::new(0, 9)).is_some());
    }
}
