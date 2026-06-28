use crate::context::CompletionContext;
use crate::snapshot::{DocumentSnapshot, FenceSnapshot};
use crate::types::{Position, Range};
use merman_core::{diagram_header_facts_for_profile, selected_baseline_registry_profile};
use serde::{Deserialize, Serialize};

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
            context.document_uri(),
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
                range,
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
            range,
            None,
            CompletionDataKind::Operator,
        ),
        keyword_completion(
            "---",
            "edge operator",
            range,
            None,
            CompletionDataKind::Operator,
        ),
        keyword_completion(
            "-.->",
            "edge operator",
            range,
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
            range,
            None,
            CompletionDataKind::Directive,
        ),
        keyword_completion(
            "::icon(name)",
            "node icon directive",
            range,
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
            range,
            None,
            CompletionDataKind::Direction,
        ),
        keyword_completion(
            "direction BT",
            "bottom to top",
            range,
            None,
            CompletionDataKind::Direction,
        ),
        keyword_completion(
            "direction LR",
            "left to right",
            range,
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
    document_uri: &str,
    range: Option<Range>,
) -> Vec<CompletionItem> {
    fence
        .text_index
        .node_ids()
        .into_iter()
        .map(|id| CompletionItem {
            label: id.clone(),
            kind: CompletionItemKind::Variable,
            detail: Some("node identifier".to_string()),
            data: Some(CompletionResolveData {
                kind: CompletionDataKind::NodeIdentifier,
                label: id.clone(),
            }),
            insert_text: Some(id.clone()),
            text_edit: range.map(|range| CompletionTextEdit {
                range,
                new_text: id.clone(),
            }),
            label_details: Some(CompletionItemLabelDetails {
                description: Some(document_uri.to_string()),
                detail: Some(format!("fence {}", fence.index + 1)),
            }),
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
        kind: CompletionItemKind::Keyword,
        detail: Some(detail.to_string()),
        data: Some(CompletionResolveData {
            kind: data_kind,
            label: label.to_string(),
        }),
        insert_text: Some(label.to_string()),
        text_edit: range.map(|range| CompletionTextEdit {
            range,
            new_text: replacement.unwrap_or(label).to_string(),
        }),
        label_details: None,
    }
}

fn shape_completion(value: &str, detail: &str, context: &CompletionContext<'_>) -> CompletionItem {
    let label = format!("@{{ shape: {value} }}");
    if let Some(edit) = context.shape_value_edit(value) {
        keyword_completion(
            &label,
            detail,
            Some(edit.range),
            Some(&edit.replacement),
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

pub fn completion_documentation(data: &CompletionResolveData) -> String {
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

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CompletionList {
    pub is_incomplete: bool,
    pub items: Vec<CompletionItem>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CompletionItem {
    pub label: String,
    pub kind: CompletionItemKind,
    pub detail: Option<String>,
    pub data: Option<CompletionResolveData>,
    pub insert_text: Option<String>,
    pub text_edit: Option<CompletionTextEdit>,
    pub label_details: Option<CompletionItemLabelDetails>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CompletionItemKind {
    Keyword,
    Variable,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CompletionTextEdit {
    pub range: Range,
    pub new_text: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CompletionItemLabelDetails {
    pub description: Option<String>,
    pub detail: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CompletionDataKind {
    DiagramHeader,
    Operator,
    Direction,
    Directive,
    Shape,
    NodeIdentifier,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CompletionResolveData {
    pub kind: CompletionDataKind,
    pub label: String,
}
