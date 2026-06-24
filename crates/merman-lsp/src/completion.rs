use crate::context::CompletionContext;
use crate::snapshot::{DocumentSnapshot, FenceSnapshot};
use tower_lsp::lsp_types::{
    CompletionItem, CompletionItemKind, CompletionItemLabelDetails, CompletionList,
    CompletionTextEdit, InsertTextFormat, Position, Range, TextEdit,
};

pub fn completion_for_snapshot(snapshot: &DocumentSnapshot, position: Position) -> CompletionList {
    let Some(context) = CompletionContext::from_snapshot(snapshot, position) else {
        return CompletionList {
            is_incomplete: false,
            items: vec![keyword_completion(
                "flowchart TD",
                "diagram kind",
                None,
                None,
            )],
        };
    };

    let mut items = Vec::new();

    if context.offer_diagram_headers() {
        items.extend(diagram_header_items(context.prefix_range()));
    }

    if context.offer_operator_items() {
        items.extend(operator_items(context.operator_range()));
    }

    if context.offer_direction_items() {
        items.extend(direction_items(context.prefix_range()));
    }

    if context.offer_directive_items() {
        items.extend(directive_items(&context));
    }

    if context.offer_shape_items() {
        items.extend(shape_items(&context));
    }

    if context.offer_node_items() {
        items.extend(node_items(
            context.fence(),
            context.uri(),
            context.node_text_edit_range(),
        ));
    }

    if items.is_empty() {
        items.push(keyword_completion(
            "flowchart TD",
            "diagram kind",
            None,
            None,
        ));
    }

    CompletionList {
        is_incomplete: false,
        items,
    }
}

fn diagram_header_items(range: Option<Range>) -> Vec<CompletionItem> {
    vec![
        keyword_completion("flowchart TD", "flowchart header", range.clone(), None),
        keyword_completion("sequenceDiagram", "sequence header", range.clone(), None),
        keyword_completion("stateDiagram-v2", "state header", range.clone(), None),
        keyword_completion("mindmap", "mindmap header", range, None),
    ]
}

fn operator_items(range: Option<Range>) -> Vec<CompletionItem> {
    vec![
        keyword_completion("-->", "edge operator", range.clone(), None),
        keyword_completion("---", "edge operator", range.clone(), None),
        keyword_completion("-.->", "edge operator", range.clone(), None),
        keyword_completion("==>", "edge operator", range, None),
    ]
}

fn directive_items(context: &CompletionContext<'_>) -> Vec<CompletionItem> {
    let range = context.prefix_range();
    let has_directives = context.fence().text_index.has_directive_prefix("classDef")
        || context.fence().text_index.has_directive_prefix(":::")
        || context.fence().text_index.has_directive_prefix("init")
        || context.fence().text_index.has_directive_prefix("wrap");
    let directive_label = if has_directives {
        "node class directive"
    } else {
        "comment"
    };
    vec![
        keyword_completion(":::className", directive_label, range.clone(), None),
        keyword_completion("::icon(name)", "node icon directive", range.clone(), None),
        keyword_completion("%% comment", "comment", range, None),
    ]
}

fn direction_items(range: Option<Range>) -> Vec<CompletionItem> {
    vec![
        keyword_completion("direction TB", "top to bottom", range.clone(), None),
        keyword_completion("direction BT", "bottom to top", range.clone(), None),
        keyword_completion("direction LR", "left to right", range.clone(), None),
        keyword_completion("direction RL", "right to left", range, None),
    ]
}

fn shape_items(context: &CompletionContext<'_>) -> Vec<CompletionItem> {
    vec![
        shape_completion("circle", "circle shape", context),
        shape_completion("rounded", "rounded shape", context),
        shape_completion("diamond", "diamond shape", context),
        shape_completion("hexagon", "hexagon shape", context),
        shape_completion("stadium", "stadium shape", context),
        shape_completion("subroutine", "subroutine shape", context),
        shape_completion("cylinder", "cylinder shape", context),
        shape_completion("trapezoid", "trapezoid shape", context),
        shape_completion("inv_trapezoid", "inverse trapezoid shape", context),
        shape_completion("doublecircle", "double circle shape", context),
    ]
}

fn node_items(
    fence: &FenceSnapshot,
    uri: &tower_lsp::lsp_types::Url,
    range: Option<Range>,
) -> Vec<CompletionItem> {
    fence
        .text_index
        .node_ids()
        .into_iter()
        .map(|id| CompletionItem {
            label: id.clone(),
            kind: Some(CompletionItemKind::VARIABLE),
            detail: Some("node identifier".to_string()),
            insert_text: Some(id.clone()),
            insert_text_format: Some(InsertTextFormat::PLAIN_TEXT),
            text_edit: range
                .clone()
                .map(|range| CompletionTextEdit::from(TextEdit::new(range, id.clone()))),
            label_details: Some(CompletionItemLabelDetails {
                description: Some(uri.to_string()),
                detail: Some(format!("fence {}", fence.index + 1)),
            }),
            ..CompletionItem::default()
        })
        .collect()
}

fn keyword_completion(
    label: &str,
    detail: &str,
    range: Option<Range>,
    replacement: Option<&str>,
) -> CompletionItem {
    CompletionItem {
        label: label.to_string(),
        kind: Some(CompletionItemKind::KEYWORD),
        detail: Some(detail.to_string()),
        insert_text: Some(label.to_string()),
        insert_text_format: Some(InsertTextFormat::PLAIN_TEXT),
        text_edit: range.map(|range| {
            CompletionTextEdit::from(TextEdit::new(
                range,
                replacement.unwrap_or(label).to_string(),
            ))
        }),
        ..CompletionItem::default()
    }
}

fn shape_completion(value: &str, detail: &str, context: &CompletionContext<'_>) -> CompletionItem {
    let label = format!("@{{ shape: {value} }}");
    if let Some((range, replacement)) = context.shape_value_edit(value) {
        keyword_completion(&label, detail, Some(range), Some(&replacement))
    } else {
        keyword_completion(&label, detail, context.shape_trigger_range(), Some(&label))
    }
}
