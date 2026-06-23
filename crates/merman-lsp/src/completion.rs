use crate::context::CompletionContext;
use crate::snapshot::{DocumentSnapshot, FenceSnapshot};
use tower_lsp::lsp_types::{
    CompletionItem, CompletionItemKind, CompletionItemLabelDetails, CompletionList,
    InsertTextFormat, Position,
};

pub fn completion_for_snapshot(snapshot: &DocumentSnapshot, position: Position) -> CompletionList {
    let Some(context) = CompletionContext::from_snapshot(snapshot, position) else {
        return CompletionList {
            is_incomplete: false,
            items: vec![keyword_completion("flowchart TD", "diagram kind")],
        };
    };

    let mut items = Vec::new();

    if context.offer_diagram_headers() {
        items.extend(diagram_header_items());
    }

    if context.offer_operator_items() {
        items.extend(operator_items());
    }

    if context.offer_direction_items() {
        items.extend(direction_items());
    }

    if context.offer_directive_items() {
        items.extend(directive_items());
    }

    if context.offer_shape_items() {
        items.extend(shape_items());
    }

    items.extend(node_items(context.fence(), context.uri()));

    if items.is_empty() {
        items.push(keyword_completion("flowchart TD", "diagram kind"));
    }

    CompletionList {
        is_incomplete: false,
        items,
    }
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

fn direction_items() -> Vec<CompletionItem> {
    vec![
        keyword_completion("direction TB", "top to bottom"),
        keyword_completion("direction BT", "bottom to top"),
        keyword_completion("direction LR", "left to right"),
        keyword_completion("direction RL", "right to left"),
    ]
}

fn shape_items() -> Vec<CompletionItem> {
    vec![
        keyword_completion("@{ shape: circle }", "circle shape"),
        keyword_completion("@{ shape: rounded }", "rounded shape"),
        keyword_completion("@{ shape: diamond }", "diamond shape"),
        keyword_completion("@{ shape: hexagon }", "hexagon shape"),
        keyword_completion("@{ shape: stadium }", "stadium shape"),
        keyword_completion("@{ shape: subroutine }", "subroutine shape"),
        keyword_completion("@{ shape: cylinder }", "cylinder shape"),
        keyword_completion("@{ shape: trapezoid }", "trapezoid shape"),
        keyword_completion("@{ shape: inv_trapezoid }", "inverse trapezoid shape"),
        keyword_completion("@{ shape: doublecircle }", "double circle shape"),
    ]
}

fn node_items(fence: &FenceSnapshot, uri: &tower_lsp::lsp_types::Url) -> Vec<CompletionItem> {
    fence
        .completion
        .node_ids()
        .into_iter()
        .map(|id| CompletionItem {
            label: id.clone(),
            kind: Some(CompletionItemKind::VARIABLE),
            detail: Some("node identifier".to_string()),
            insert_text: Some(id.clone()),
            insert_text_format: Some(InsertTextFormat::PLAIN_TEXT),
            label_details: Some(CompletionItemLabelDetails {
                description: Some(uri.to_string()),
                detail: Some(format!("fence {}", fence.index + 1)),
            }),
            ..CompletionItem::default()
        })
        .collect()
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
