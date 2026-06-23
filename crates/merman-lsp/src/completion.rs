use crate::document_store::{DocumentSnapshot, FenceSnapshot};
use std::collections::BTreeSet;
use tower_lsp::lsp_types::{
    CompletionItem, CompletionItemKind, CompletionItemLabelDetails, CompletionList,
    InsertTextFormat, Position,
};

pub fn completion_for_snapshot(snapshot: &DocumentSnapshot, position: Position) -> CompletionList {
    let Some(fence) = snapshot.fence_at_position(position) else {
        return CompletionList {
            is_incomplete: false,
            items: vec![keyword_completion("flowchart TD", "diagram kind")],
        };
    };

    let prefix = completion_prefix(snapshot, fence, position);
    let mut items = Vec::new();

    if prefix.is_empty()
        || prefix == "flowchart"
        || prefix == "sequenceDiagram"
        || prefix == "stateDiagram"
    {
        items.extend(diagram_header_items());
    }

    if prefix.ends_with("-") || prefix.ends_with("--") || prefix.ends_with("->") {
        items.extend(operator_items());
    }

    if prefix.contains("class") || prefix.contains(":::") {
        items.extend(directive_items());
    }

    items.extend(node_items(snapshot, fence));

    if items.is_empty() {
        items.push(keyword_completion("flowchart TD", "diagram kind"));
    }

    CompletionList {
        is_incomplete: false,
        items,
    }
}

fn completion_prefix(
    snapshot: &DocumentSnapshot,
    fence: &FenceSnapshot,
    position: Position,
) -> String {
    let Some(offset) =
        snapshot
            .source_map
            .byte_offset_for_utf16_position(merman_analysis::Utf16Position {
                line: position.line as usize,
                character: position.character as usize,
            })
    else {
        return String::new();
    };

    let rel = offset.saturating_sub(fence.body_start);
    fence.text[..rel.min(fence.text.len())]
        .rsplit_once('\n')
        .map(|(_, tail)| tail.trim_start().to_string())
        .unwrap_or_else(|| {
            fence.text[..rel.min(fence.text.len())]
                .trim_start()
                .to_string()
        })
}

fn diagram_header_items() -> Vec<CompletionItem> {
    vec![
        keyword_completion("flowchart TD", "flowchart header"),
        keyword_completion("sequenceDiagram", "sequence header"),
        keyword_completion("stateDiagram-v2", "state header"),
        keyword_completion("mindmap", "mindmap header"),
    ]
}

fn operator_items() -> Vec<CompletionItem> {
    vec![
        keyword_completion("-->", "edge operator"),
        keyword_completion("---", "edge operator"),
        keyword_completion("-.->", "edge operator"),
        keyword_completion("==>", "edge operator"),
    ]
}

fn directive_items() -> Vec<CompletionItem> {
    vec![
        keyword_completion(":::className", "node class directive"),
        keyword_completion("::icon(name)", "node icon directive"),
        keyword_completion("%% comment", "comment"),
    ]
}

fn node_items(snapshot: &DocumentSnapshot, fence: &FenceSnapshot) -> Vec<CompletionItem> {
    node_ids(&fence.text)
        .into_iter()
        .map(|id| CompletionItem {
            label: id.clone(),
            kind: Some(CompletionItemKind::VARIABLE),
            detail: Some("node identifier".to_string()),
            insert_text: Some(id),
            insert_text_format: Some(InsertTextFormat::PLAIN_TEXT),
            label_details: Some(CompletionItemLabelDetails {
                description: Some(snapshot.uri.to_string()),
                detail: Some(format!("fence {}", fence.index + 1)),
            }),
            ..CompletionItem::default()
        })
        .collect()
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

fn keyword_completion(label: &str, detail: &str) -> CompletionItem {
    CompletionItem {
        label: label.to_string(),
        kind: Some(CompletionItemKind::KEYWORD),
        detail: Some(detail.to_string()),
        insert_text: Some(label.to_string()),
        insert_text_format: Some(InsertTextFormat::PLAIN_TEXT),
        ..CompletionItem::default()
    }
}
