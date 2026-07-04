use crate::protocol::{core_position_from_lsp, document_uri_to_lsp, location_to_lsp, range_to_lsp};
use crate::snapshot::DocumentSnapshot;
use merman_analysis::EditorSymbolKind;
#[cfg(test)]
use merman_editor_core::workspace_symbols as core_workspace_symbols;
use merman_editor_core::{
    EditorDocumentSymbol, EditorFoldingRange, EditorFoldingRangeKind, EditorHover,
    EditorPrepareRename, EditorSelectionRange, EditorSymbolInformation, EditorWorkspaceEdit,
    RenameError, document_symbols as core_document_symbols, folding_ranges as core_folding_ranges,
    goto_definition as core_goto_definition, hover as core_hover,
    prepare_rename as core_prepare_rename, references as core_references, rename as core_rename,
    selection_ranges as core_selection_ranges,
    workspace_symbols_for_snapshots as core_workspace_symbols_for_snapshots,
};
use std::collections::HashMap;
use std::sync::Arc;
use tower_lsp::jsonrpc::{Error, Result};
use tower_lsp::lsp_types::{DocumentChanges, OneOf};
use tower_lsp::lsp_types::{
    DocumentSymbol, DocumentSymbolResponse, FoldingRange, FoldingRangeKind, GotoDefinitionResponse,
    Hover, HoverContents, Location, MarkupContent, MarkupKind,
    OptionalVersionedTextDocumentIdentifier, Position, PrepareRenameResponse, Range, RenameParams,
    SelectionRange, SymbolInformation, SymbolKind, TextDocumentEdit, TextEdit, Url, WorkspaceEdit,
};

#[allow(deprecated)]
pub fn document_symbols(snapshot: &DocumentSnapshot) -> DocumentSymbolResponse {
    DocumentSymbolResponse::Nested(
        core_document_symbols(snapshot.as_editor())
            .into_iter()
            .map(document_symbol_to_lsp)
            .collect(),
    )
}

#[allow(deprecated)]
#[cfg(test)]
pub fn workspace_symbols(snapshot: &DocumentSnapshot, query: &str) -> Vec<SymbolInformation> {
    core_workspace_symbols(snapshot.as_editor(), query)
        .into_iter()
        .filter_map(|symbol| symbol_information_to_lsp(symbol, Some(&snapshot.uri)))
        .collect()
}

pub fn workspace_symbols_for_snapshots(
    snapshots: &[Arc<DocumentSnapshot>],
    query: &str,
) -> Vec<SymbolInformation> {
    let uri_lookup = snapshots
        .iter()
        .map(|snapshot| (snapshot.uri.as_str().to_string(), snapshot.uri.clone()))
        .collect::<HashMap<_, _>>();

    core_workspace_symbols_for_snapshots(
        snapshots
            .iter()
            .map(|snapshot| snapshot.as_ref().as_editor()),
        query,
    )
    .into_iter()
    .filter_map(|symbol| {
        let uri = uri_lookup.get(symbol.location.uri.as_str());
        symbol_information_to_lsp(symbol, uri)
    })
    .collect()
}

pub fn hover(snapshot: &DocumentSnapshot, position: Position) -> Option<Hover> {
    core_hover(snapshot.as_editor(), core_position_from_lsp(position)).map(hover_to_lsp)
}

pub fn selection_ranges(
    snapshot: &DocumentSnapshot,
    positions: &[Position],
) -> Option<Vec<SelectionRange>> {
    let core_positions = positions
        .iter()
        .copied()
        .map(core_position_from_lsp)
        .collect::<Vec<_>>();

    Some(
        core_selection_ranges(snapshot.as_editor(), &core_positions)
            .into_iter()
            .zip(positions.iter().copied())
            .map(|(range, position)| {
                range
                    .and_then(selection_range_to_lsp)
                    .unwrap_or_else(|| fallback_selection_range(position))
            })
            .collect(),
    )
}

pub fn folding_ranges(snapshot: &DocumentSnapshot) -> Vec<FoldingRange> {
    core_folding_ranges(snapshot.as_editor())
        .into_iter()
        .map(folding_range_to_lsp)
        .collect()
}

pub fn goto_definition(
    snapshot: &DocumentSnapshot,
    position: Position,
) -> Option<GotoDefinitionResponse> {
    core_goto_definition(snapshot.as_editor(), core_position_from_lsp(position))
        .map(|location| location_to_lsp(location, &snapshot.uri))
        .map(Into::into)
}

pub fn references(
    snapshot: &DocumentSnapshot,
    position: Position,
    include_declaration: bool,
) -> Option<Vec<Location>> {
    core_references(
        snapshot.as_editor(),
        core_position_from_lsp(position),
        include_declaration,
    )
    .map(|locations| {
        locations
            .into_iter()
            .map(|location| location_to_lsp(location, &snapshot.uri))
            .collect()
    })
}

pub fn prepare_rename(
    snapshot: &DocumentSnapshot,
    position: Position,
) -> Option<PrepareRenameResponse> {
    core_prepare_rename(snapshot.as_editor(), core_position_from_lsp(position)).map(prepare_to_lsp)
}

pub fn rename(snapshot: &DocumentSnapshot, params: RenameParams) -> Result<Option<WorkspaceEdit>> {
    let position = params.text_document_position.position;
    core_rename(
        snapshot.as_editor(),
        core_position_from_lsp(position),
        &params.new_name,
    )
    .map(|edit| edit.and_then(|edit| workspace_edit_to_lsp(edit, &snapshot.uri, snapshot.version)))
    .map_err(rename_error_to_lsp)
}

fn hover_to_lsp(hover: EditorHover) -> Hover {
    Hover {
        contents: HoverContents::Markup(MarkupContent {
            kind: MarkupKind::Markdown,
            value: hover.contents.value,
        }),
        range: hover.range.map(range_to_lsp),
    }
}

fn selection_range_to_lsp(selection_range: EditorSelectionRange) -> Option<SelectionRange> {
    let parent = match selection_range.parent {
        Some(parent) => Some(Box::new(selection_range_to_lsp(*parent)?)),
        None => None,
    };

    Some(SelectionRange {
        range: range_to_lsp(selection_range.range),
        parent,
    })
}

fn fallback_selection_range(position: Position) -> SelectionRange {
    SelectionRange {
        range: Range::new(position, position),
        parent: None,
    }
}

fn folding_range_to_lsp(folding_range: EditorFoldingRange) -> FoldingRange {
    let kind = match folding_range.kind {
        EditorFoldingRangeKind::Region => FoldingRangeKind::Region,
    };

    FoldingRange {
        start_line: folding_range.range.start.line as u32,
        start_character: Some(folding_range.range.start.character as u32),
        end_line: folding_range.range.end.line as u32,
        end_character: Some(folding_range.range.end.character as u32),
        kind: Some(kind),
        collapsed_text: None,
    }
}

#[allow(deprecated)]
fn document_symbol_to_lsp(symbol: EditorDocumentSymbol) -> DocumentSymbol {
    DocumentSymbol {
        name: symbol.name,
        detail: symbol.detail,
        kind: symbol_kind(symbol.kind),
        tags: None,
        deprecated: None,
        range: range_to_lsp(symbol.range),
        selection_range: range_to_lsp(symbol.selection_range),
        children: if symbol.children.is_empty() {
            None
        } else {
            Some(
                symbol
                    .children
                    .into_iter()
                    .map(document_symbol_to_lsp)
                    .collect(),
            )
        },
    }
}

#[allow(deprecated)]
fn symbol_information_to_lsp(
    symbol: EditorSymbolInformation,
    fallback_uri: Option<&Url>,
) -> Option<SymbolInformation> {
    let location = location_to_lsp(symbol.location, fallback_uri?);
    Some(SymbolInformation {
        name: symbol.name,
        kind: symbol_kind(symbol.kind),
        tags: None,
        deprecated: None,
        location,
        container_name: symbol.container_name,
    })
}

fn prepare_to_lsp(rename: EditorPrepareRename) -> PrepareRenameResponse {
    PrepareRenameResponse::RangeWithPlaceholder {
        range: range_to_lsp(rename.range),
        placeholder: rename.placeholder,
    }
}

fn workspace_edit_to_lsp(
    edit: EditorWorkspaceEdit,
    fallback_uri: &Url,
    version: i32,
) -> Option<WorkspaceEdit> {
    let mut document_edits = Vec::new();
    for (uri, edits) in edit.changes {
        let uri = document_uri_to_lsp(&uri, fallback_uri);
        let edits = edits
            .into_iter()
            .map(|edit| OneOf::Left(TextEdit::new(range_to_lsp(edit.range), edit.new_text)))
            .collect::<Vec<_>>();
        if edits.is_empty() {
            continue;
        }
        document_edits.push(TextDocumentEdit {
            text_document: OptionalVersionedTextDocumentIdentifier {
                uri,
                version: Some(version),
            },
            edits,
        });
    }

    if document_edits.is_empty() {
        return None;
    }

    Some(WorkspaceEdit {
        changes: None,
        document_changes: Some(DocumentChanges::Edits(document_edits)),
        change_annotations: None,
    })
}

fn rename_error_to_lsp(error: RenameError) -> Error {
    Error::invalid_params(error.to_string())
}

fn symbol_kind(kind: EditorSymbolKind) -> SymbolKind {
    match kind {
        EditorSymbolKind::Class => SymbolKind::CLASS,
        EditorSymbolKind::Event => SymbolKind::EVENT,
        EditorSymbolKind::Function => SymbolKind::FUNCTION,
        EditorSymbolKind::Module => SymbolKind::MODULE,
        EditorSymbolKind::Namespace => SymbolKind::NAMESPACE,
        EditorSymbolKind::Object => SymbolKind::OBJECT,
        EditorSymbolKind::Package => SymbolKind::PACKAGE,
        EditorSymbolKind::Property => SymbolKind::PROPERTY,
        EditorSymbolKind::String => SymbolKind::STRING,
        EditorSymbolKind::Struct => SymbolKind::STRUCT,
        EditorSymbolKind::Variable => SymbolKind::VARIABLE,
    }
}

#[cfg(test)]
mod tests {
    use super::{
        document_symbols, folding_ranges, goto_definition, hover, prepare_rename, references,
        rename, selection_ranges, workspace_symbols,
    };
    use crate::document_store::DocumentStore;
    use tower_lsp::lsp_types::{
        DocumentChanges, DocumentSymbolResponse, FoldingRangeKind, GotoDefinitionResponse,
        HoverContents, Position, PrepareRenameResponse, RenameParams, TextDocumentIdentifier,
        TextDocumentPositionParams, Url,
    };

    #[test]
    fn document_symbols_include_root_and_child_items() {
        let mut store = DocumentStore::new();
        let uri = Url::parse("file:///tmp/example.mmd").unwrap();
        let snapshot = store.upsert(
            uri,
            1,
            "flowchart TD\nsubgraph group\nA-->B\nend\n".to_string(),
        );

        let response = document_symbols(&snapshot);
        let nested = match response {
            DocumentSymbolResponse::Nested(symbols) => symbols,
            other => panic!("unexpected symbol response: {other:?}"),
        };

        assert_eq!(nested.len(), 1);
        assert_eq!(nested[0].name, "flowchart-v2 diagram");
        assert!(
            nested[0]
                .children
                .as_ref()
                .unwrap()
                .iter()
                .any(|symbol| symbol.name == "group")
        );
    }

    #[test]
    fn hover_reports_the_active_outline_entry() {
        let mut store = DocumentStore::new();
        let uri = Url::parse("file:///tmp/example.mmd").unwrap();
        let snapshot = store.upsert(uri, 1, "flowchart TD\nA-->B\n".to_string());

        let hover = hover(&snapshot, Position::new(1, 0)).unwrap();
        let text = match hover.contents {
            HoverContents::Markup(markup) => markup.value,
            other => panic!("unexpected hover contents: {other:?}"),
        };

        assert!(text.contains("A"));
        assert!(text.contains("Diagram:"));
    }

    #[test]
    fn selection_ranges_return_nested_parser_backed_ranges() {
        let mut store = DocumentStore::new();
        let uri = Url::parse("file:///tmp/example.mmd").unwrap();
        let snapshot = store.upsert(
            uri,
            1,
            "flowchart TD\nsubgraph group\nA-->B\nend\n".to_string(),
        );

        let ranges = selection_ranges(&snapshot, &[Position::new(2, 0)]).unwrap();

        assert_eq!(ranges.len(), 1);
        assert_eq!(ranges[0].range.start, Position::new(2, 0));
        assert_eq!(ranges[0].range.end, Position::new(2, 1));
        assert!(ranges[0].parent.is_some());
    }

    #[test]
    fn folding_ranges_return_lsp_regions() {
        let mut store = DocumentStore::new();
        let uri = Url::parse("file:///tmp/example.md").unwrap();
        let snapshot = store.upsert(
            uri,
            1,
            "before\n```mermaid\nflowchart TD\nA-->B\n```\nafter\n".to_string(),
        );

        let ranges = folding_ranges(&snapshot);

        assert!(ranges.iter().any(|range| {
            range.start_line == 1
                && range.end_line == 4
                && range.kind == Some(FoldingRangeKind::Region)
        }));
    }

    #[test]
    fn rename_and_references_track_simple_identifiers() {
        let mut store = DocumentStore::new();
        let uri = Url::parse("file:///tmp/example.mmd").unwrap();
        let snapshot = store.upsert(uri, 1, "flowchart TD\nA-->B\nA-->C\n".to_string());

        let position = Position::new(1, 0);
        let prepare = prepare_rename(&snapshot, position).unwrap();
        match prepare {
            PrepareRenameResponse::RangeWithPlaceholder { placeholder, .. } => {
                assert_eq!(placeholder, "A");
            }
            other => panic!("unexpected prepare rename response: {other:?}"),
        }

        let refs = references(&snapshot, position, true).unwrap();
        assert_eq!(refs.len(), 2);

        let rename = rename(
            &snapshot,
            RenameParams {
                text_document_position: TextDocumentPositionParams::new(
                    TextDocumentIdentifier {
                        uri: snapshot.uri.clone(),
                    },
                    position,
                ),
                new_name: "X".to_string(),
                work_done_progress_params: Default::default(),
            },
        )
        .unwrap();
        let edit = rename.expect("expected rename edit");
        assert!(edit.changes.is_none());
        let document_changes = match edit.document_changes.as_ref().unwrap() {
            DocumentChanges::Edits(edits) => edits,
            other => panic!("unexpected document changes: {other:?}"),
        };
        assert_eq!(document_changes.len(), 1);
        assert_eq!(document_changes[0].text_document.uri, snapshot.uri);
        assert_eq!(document_changes[0].text_document.version, Some(1));
        assert_eq!(document_changes[0].edits.len(), 2);

        let def = goto_definition(&snapshot, position).unwrap();
        assert!(matches!(def, GotoDefinitionResponse::Scalar(_)));
    }

    #[test]
    fn workspace_symbols_filter_and_include_outline_items() {
        let mut store = DocumentStore::new();
        let uri = Url::parse("file:///tmp/example.mmd").unwrap();
        let snapshot = store.upsert(
            uri,
            1,
            "flowchart TD\nsubgraph group\nA-->B\nend\n".to_string(),
        );

        let all_symbols = workspace_symbols(&snapshot, "");
        assert!(all_symbols.iter().any(|symbol| symbol.name == "group"));
        assert!(all_symbols.iter().any(|symbol| symbol.name == "A"));

        let group_symbols = workspace_symbols(&snapshot, "group");
        assert_eq!(group_symbols.len(), 1);
        assert_eq!(group_symbols[0].name, "group");
    }
}
