use crate::protocol::{core_position_from_lsp, range_to_lsp};
use crate::snapshot::DocumentSnapshot;
use merman_editor_core::{
    CompletionDataKind, CompletionInsertTextFormat, CompletionItemKind,
    CompletionList as CoreCompletionList, CompletionResolveData, completion_documentation,
    completion_for_snapshot as core_completion_for_snapshot,
};
use serde_json::json;
use tower_lsp::lsp_types::{
    CompletionItem, CompletionItemKind as LspCompletionItemKind, CompletionItemLabelDetails,
    CompletionList, CompletionTextEdit, Documentation, InsertTextFormat, MarkupContent, MarkupKind,
    Position, TextEdit,
};

pub fn completion_for_snapshot(snapshot: &DocumentSnapshot, position: Position) -> CompletionList {
    core_completion_to_lsp(core_completion_for_snapshot(
        snapshot.as_editor(),
        core_position_from_lsp(position),
    ))
}

fn core_completion_to_lsp(list: CoreCompletionList) -> CompletionList {
    CompletionList {
        is_incomplete: list.is_incomplete,
        items: list.items.into_iter().map(core_item_to_lsp).collect(),
    }
}

fn core_item_to_lsp(item: merman_editor_core::CompletionItem) -> CompletionItem {
    CompletionItem {
        label: item.label.clone(),
        kind: Some(match item.kind {
            CompletionItemKind::Keyword => LspCompletionItemKind::KEYWORD,
            CompletionItemKind::Variable => LspCompletionItemKind::VARIABLE,
            CompletionItemKind::Class => LspCompletionItemKind::CLASS,
            CompletionItemKind::Snippet => LspCompletionItemKind::SNIPPET,
        }),
        detail: item.detail,
        data: item.data.map(|data| {
            json!({
                "kind": data.kind,
                "label": data.label,
            })
        }),
        insert_text: item.insert_text,
        insert_text_format: Some(match item.insert_text_format {
            CompletionInsertTextFormat::PlainText => InsertTextFormat::PLAIN_TEXT,
            CompletionInsertTextFormat::Snippet => InsertTextFormat::SNIPPET,
        }),
        text_edit: item.text_edit.map(|edit| {
            CompletionTextEdit::from(TextEdit::new(range_to_lsp(edit.range), edit.new_text))
        }),
        label_details: item
            .label_details
            .map(|details| CompletionItemLabelDetails {
                description: details.description,
                detail: details.detail,
            }),
        ..CompletionItem::default()
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

#[allow(dead_code)]
fn _assert_completion_data_kind_is_lsp_serializable(kind: CompletionDataKind) -> serde_json::Value {
    json!(kind)
}

#[cfg(test)]
mod tests {
    use super::completion_for_snapshot;
    use crate::document_store::DocumentStore;
    use merman_core::{diagram_header_facts_for_profile, selected_baseline_registry_profile};
    use tower_lsp::lsp_types::{Position, Url};

    #[test]
    fn diagram_header_items_follow_core_header_facts() {
        let mut store = DocumentStore::new();
        let uri = Url::parse("file:///tmp/example.mmd").unwrap();
        let snapshot = store.upsert(uri, 1, "flow".to_string());
        let labels: Vec<_> = completion_for_snapshot(&snapshot, Position::new(0, 4))
            .items
            .into_iter()
            .filter(|item| {
                item.data
                    .as_ref()
                    .and_then(|data| data.get("kind"))
                    .and_then(|kind| kind.as_str())
                    == Some("diagram_header")
            })
            .map(|item| item.label)
            .collect();
        let expected: Vec<_> =
            diagram_header_facts_for_profile(selected_baseline_registry_profile())
                .iter()
                .map(|fact| fact.label.to_string())
                .collect();

        assert_eq!(labels, expected);
    }
}
