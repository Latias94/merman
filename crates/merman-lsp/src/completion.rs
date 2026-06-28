use crate::context::CompletionContext;
use crate::snapshot::{DocumentSnapshot, FenceSnapshot};
use merman_core::{diagram_header_facts_for_profile, selected_baseline_registry_profile};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use tower_lsp::lsp_types::{
    CompletionItem, CompletionItemKind, CompletionItemLabelDetails, CompletionList,
    CompletionTextEdit, Documentation, InsertTextFormat, MarkupContent, MarkupKind, Position,
    Range, TextEdit,
};

pub fn completion_for_snapshot(snapshot: &DocumentSnapshot, position: Position) -> CompletionList {
    let Some(context) = CompletionContext::from_snapshot(snapshot, position) else {
        return CompletionList {
            is_incomplete: false,
            items: diagram_header_items(None),
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
        if context.is_comment_or_directive_line() || context.is_parser_controlled_payload() {
            return CompletionList {
                is_incomplete: false,
                items,
            };
        }

        items.extend(diagram_header_items(context.prefix_range()));
    }

    CompletionList {
        is_incomplete: false,
        items,
    }
}

fn diagram_header_items(range: Option<Range>) -> Vec<CompletionItem> {
    diagram_header_facts_for_profile(selected_baseline_registry_profile())
        .iter()
        .map(|fact| {
            keyword_completion(
                fact.label,
                fact.detail,
                range.clone(),
                None,
                CompletionDataKind::DiagramHeader,
            )
        })
        .collect()
}

fn operator_items(range: Option<Range>) -> Vec<CompletionItem> {
    vec![
        keyword_completion(
            "-->",
            "edge operator",
            range.clone(),
            None,
            CompletionDataKind::Operator,
        ),
        keyword_completion(
            "---",
            "edge operator",
            range.clone(),
            None,
            CompletionDataKind::Operator,
        ),
        keyword_completion(
            "-.->",
            "edge operator",
            range.clone(),
            None,
            CompletionDataKind::Operator,
        ),
        keyword_completion(
            "==>",
            "edge operator",
            range,
            None,
            CompletionDataKind::Operator,
        ),
    ]
}

fn directive_items(context: &CompletionContext<'_>) -> Vec<CompletionItem> {
    let range = context.prefix_range();
    let has_directives = context
        .fence()
        .text_index
        .directive_prefixes()
        .any(|prefix| is_directive_helper_prefix(prefix.as_str()));
    let directive_label = if has_directives {
        "directive helper"
    } else {
        "comment"
    };
    vec![
        keyword_completion(
            ":::className",
            directive_label,
            range.clone(),
            None,
            CompletionDataKind::Directive,
        ),
        keyword_completion(
            "::icon(name)",
            "node icon directive",
            range.clone(),
            None,
            CompletionDataKind::Directive,
        ),
        keyword_completion(
            "%% comment",
            "comment",
            range,
            None,
            CompletionDataKind::Directive,
        ),
    ]
}

fn is_directive_helper_prefix(prefix: &str) -> bool {
    matches!(
        prefix,
        "classDef"
            | "class"
            | "style"
            | "cssClass"
            | "linkStyle"
            | "click"
            | "link"
            | "callback"
            | ":::"
    )
}

fn direction_items(range: Option<Range>) -> Vec<CompletionItem> {
    vec![
        keyword_completion(
            "direction TB",
            "top to bottom",
            range.clone(),
            None,
            CompletionDataKind::Direction,
        ),
        keyword_completion(
            "direction BT",
            "bottom to top",
            range.clone(),
            None,
            CompletionDataKind::Direction,
        ),
        keyword_completion(
            "direction LR",
            "left to right",
            range.clone(),
            None,
            CompletionDataKind::Direction,
        ),
        keyword_completion(
            "direction RL",
            "right to left",
            range,
            None,
            CompletionDataKind::Direction,
        ),
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
            data: Some(completion_data(CompletionDataKind::NodeIdentifier, &id)),
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
    data_kind: CompletionDataKind,
) -> CompletionItem {
    CompletionItem {
        label: label.to_string(),
        kind: Some(CompletionItemKind::KEYWORD),
        detail: Some(detail.to_string()),
        data: Some(completion_data(data_kind, label)),
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
        keyword_completion(
            &label,
            detail,
            Some(range),
            Some(&replacement),
            CompletionDataKind::Shape,
        )
    } else {
        keyword_completion(
            &label,
            detail,
            context.shape_trigger_range(),
            Some(&label),
            CompletionDataKind::Shape,
        )
    }
}

pub fn resolve_completion_item(mut item: CompletionItem) -> CompletionItem {
    if item.documentation.is_some() {
        return item;
    }

    let Some(data) = item
        .data
        .as_ref()
        .and_then(|value| serde_json::from_value::<CompletionResolveData>(value.clone()).ok())
    else {
        return item;
    };

    item.documentation = Some(Documentation::MarkupContent(MarkupContent {
        kind: MarkupKind::Markdown,
        value: completion_documentation(&data),
    }));
    item
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
enum CompletionDataKind {
    DiagramHeader,
    Operator,
    Direction,
    Directive,
    Shape,
    NodeIdentifier,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
struct CompletionResolveData {
    kind: CompletionDataKind,
    label: String,
}

fn completion_data(kind: CompletionDataKind, label: &str) -> Value {
    json!({
        "kind": kind,
        "label": label
    })
}

fn completion_documentation(data: &CompletionResolveData) -> String {
    match data.kind {
        CompletionDataKind::DiagramHeader => format!(
            "Starts a Mermaid `{}` diagram. Use it as the first statement in a plain Mermaid file or fenced Mermaid block.",
            data.label
        ),
        CompletionDataKind::Operator => format!(
            "Inserts the Mermaid `{}` relationship operator between diagram identifiers.",
            data.label
        ),
        CompletionDataKind::Direction => format!(
            "Sets flow direction with `{}`. Direction statements are valid inside flowchart subgraphs and supported flowchart contexts.",
            data.label
        ),
        CompletionDataKind::Directive => format!(
            "Inserts `{}` as a Mermaid directive or comment helper for the current fence.",
            data.label
        ),
        CompletionDataKind::Shape => format!(
            "Inserts Mermaid flowchart shape object syntax for `{}`.",
            data.label
        ),
        CompletionDataKind::NodeIdentifier => format!(
            "Reuses the `{}` identifier already present in the current Mermaid fence.",
            data.label
        ),
    }
}

#[cfg(test)]
mod tests {
    use super::diagram_header_items;
    use merman_core::{diagram_header_facts_for_profile, selected_baseline_registry_profile};
    use tower_lsp::lsp_types::Range;

    #[test]
    fn diagram_header_items_follow_core_header_facts() {
        let labels: Vec<_> = diagram_header_items(Some(Range::default()))
            .into_iter()
            .map(|item| item.label)
            .collect();
        let expected: Vec<_> =
            diagram_header_facts_for_profile(selected_baseline_registry_profile())
                .iter()
                .map(|fact| fact.label)
                .collect();

        assert_eq!(labels, expected);
    }
}
