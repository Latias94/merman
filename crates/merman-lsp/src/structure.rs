use crate::snapshot::DocumentSnapshot;
use merman_analysis::EditorSymbolKind;
use merman_editor_core::{
    EditorDocumentSymbol, EditorFoldingRange, EditorFoldingRangeKind, EditorHover, EditorLocation,
    EditorPrepareRename, EditorSelectionRange, EditorSymbolInformation, EditorWorkspaceEdit,
    Range as CoreRange, RenameError, document_symbols as core_document_symbols,
    folding_ranges as core_folding_ranges, goto_definition as core_goto_definition,
    hover as core_hover, prepare_rename as core_prepare_rename, references as core_references,
    rename as core_rename, selection_ranges as core_selection_ranges,
    workspace_symbols as core_workspace_symbols,
    workspace_symbols_for_snapshots as core_workspace_symbols_for_snapshots,
};
use std::collections::HashMap;
use tower_lsp::jsonrpc::{Error, Result};
use tower_lsp::lsp_types::{
    DocumentSymbol, DocumentSymbolResponse, FoldingRange, FoldingRangeKind, GotoDefinitionResponse,
    Hover, HoverContents, Location, MarkupContent, MarkupKind, Position, PrepareRenameResponse,
    Range, RenameParams, SelectionRange, SymbolInformation, SymbolKind, TextEdit, Url,
    WorkspaceEdit,
};

#[allow(deprecated)]
pub fn document_symbols(snapshot: &DocumentSnapshot) -> DocumentSymbolResponse {
    DocumentSymbolResponse::Nested(
        core_document_symbols(&snapshot.to_editor())
            .into_iter()
            .map(document_symbol_to_lsp)
            .collect(),
    )
}

#[allow(deprecated)]
pub fn workspace_symbols(snapshot: &DocumentSnapshot, query: &str) -> Vec<SymbolInformation> {
    core_workspace_symbols(&snapshot.to_editor(), query)
        .into_iter()
        .filter_map(|symbol| symbol_information_to_lsp(symbol, Some(&snapshot.uri)))
        .collect()
}

pub fn workspace_symbols_for_snapshots(
    snapshots: &[DocumentSnapshot],
    query: &str,
) -> Vec<SymbolInformation> {
    let editor_snapshots = snapshots
        .iter()
        .map(DocumentSnapshot::to_editor)
        .collect::<Vec<_>>();
    let uri_lookup = snapshots
        .iter()
        .map(|snapshot| (snapshot.uri.as_str().to_string(), snapshot.uri.clone()))
        .collect::<HashMap<_, _>>();

    core_workspace_symbols_for_snapshots(&editor_snapshots, query)
        .into_iter()
        .filter_map(|symbol| {
            let uri = uri_lookup.get(symbol.location.uri.as_str());
            symbol_information_to_lsp(symbol, uri)
        })
        .collect()
}

pub fn hover(snapshot: &DocumentSnapshot, position: Position) -> Option<Hover> {
    core_hover(&snapshot.to_editor(), position_to_core(position)).map(hover_to_lsp)
}

pub fn selection_ranges(
    snapshot: &DocumentSnapshot,
    positions: &[Position],
) -> Option<Vec<SelectionRange>> {
    let core_positions = positions
        .iter()
        .copied()
        .map(position_to_core)
        .collect::<Vec<_>>();

    Some(
        core_selection_ranges(&snapshot.to_editor(), &core_positions)
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
    core_folding_ranges(&snapshot.to_editor())
        .into_iter()
        .map(folding_range_to_lsp)
        .collect()
}

pub fn goto_definition(
    snapshot: &DocumentSnapshot,
    position: Position,
) -> Option<GotoDefinitionResponse> {
    core_goto_definition(&snapshot.to_editor(), position_to_core(position))
        .and_then(|location| location_to_lsp(location, &snapshot.uri))
        .map(Into::into)
}

pub fn references(
    snapshot: &DocumentSnapshot,
    position: Position,
    include_declaration: bool,
) -> Option<Vec<Location>> {
    core_references(
        &snapshot.to_editor(),
        position_to_core(position),
        include_declaration,
    )
    .map(|locations| {
        locations
            .into_iter()
            .filter_map(|location| location_to_lsp(location, &snapshot.uri))
            .collect()
    })
}

pub fn prepare_rename(
    snapshot: &DocumentSnapshot,
    position: Position,
) -> Option<PrepareRenameResponse> {
    core_prepare_rename(&snapshot.to_editor(), position_to_core(position)).map(prepare_to_lsp)
}

pub fn rename(snapshot: &DocumentSnapshot, params: RenameParams) -> Result<Option<WorkspaceEdit>> {
    let position = params.text_document_position.position;
    core_rename(
        &snapshot.to_editor(),
        position_to_core(position),
        &params.new_name,
    )
    .map(|edit| edit.and_then(|edit| workspace_edit_to_lsp(edit, &snapshot.uri)))
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
    let location = location_to_lsp(symbol.location, fallback_uri?)?;
    Some(SymbolInformation {
        name: symbol.name,
        kind: symbol_kind(symbol.kind),
        tags: None,
        deprecated: None,
        location,
        container_name: symbol.container_name,
    })
}

fn location_to_lsp(location: EditorLocation, fallback_uri: &Url) -> Option<Location> {
    let uri = Url::parse(location.uri.as_str()).unwrap_or_else(|_| fallback_uri.clone());
    Some(Location::new(uri, range_to_lsp(location.range)))
}

fn prepare_to_lsp(rename: EditorPrepareRename) -> PrepareRenameResponse {
    PrepareRenameResponse::RangeWithPlaceholder {
        range: range_to_lsp(rename.range),
        placeholder: rename.placeholder,
    }
}

fn workspace_edit_to_lsp(edit: EditorWorkspaceEdit, fallback_uri: &Url) -> Option<WorkspaceEdit> {
    let mut changes = HashMap::new();
    for (uri, edits) in edit.changes {
        let uri = Url::parse(uri.as_str()).unwrap_or_else(|_| fallback_uri.clone());
        let edits = edits
            .into_iter()
            .map(|edit| TextEdit::new(range_to_lsp(edit.range), edit.new_text))
            .collect::<Vec<_>>();
        changes.insert(uri, edits);
    }

    Some(WorkspaceEdit {
        changes: Some(changes),
        document_changes: None,
        change_annotations: None,
    })
}

fn rename_error_to_lsp(error: RenameError) -> Error {
    Error::invalid_params(error.to_string())
}

fn position_to_core(position: Position) -> merman_editor_core::Position {
    merman_editor_core::Position::new(position.line as usize, position.character as usize)
}

fn range_to_lsp(range: CoreRange) -> Range {
    Range::new(
        Position::new(range.start.line as u32, range.start.character as u32),
        Position::new(range.end.line as u32, range.end.character as u32),
    )
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
        DocumentSymbolResponse, FoldingRangeKind, GotoDefinitionResponse, HoverContents, Position,
        PrepareRenameResponse, RenameParams, TextDocumentIdentifier, TextDocumentPositionParams,
        Url,
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
        assert_eq!(
            edit.changes
                .as_ref()
                .unwrap()
                .get(&snapshot.uri)
                .unwrap()
                .len(),
            2
        );

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
