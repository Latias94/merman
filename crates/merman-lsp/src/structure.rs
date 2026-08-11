use crate::client_profile::{ClientProtocolProfile, MarkupPreference};
use crate::protocol::{
    core_position_from_lsp, generated_markdown_to_plain_text, location_to_lsp, range_to_lsp,
};
use crate::snapshot::DocumentSnapshot;
use crate::workspace_edit::{WorkspaceEditChange, WorkspaceEditEncoding, project_workspace_edit};
use merman_editor_core::{
    EditorDocumentSymbol, EditorFoldingRange, EditorFoldingRangeKind, EditorHover,
    EditorPrepareRename, EditorSelectionRange, EditorSemanticKind, RenameError,
    document_symbols as core_document_symbols, folding_ranges as core_folding_ranges,
    goto_definition as core_goto_definition, hover as core_hover,
    prepare_rename as core_prepare_rename, references as core_references, rename as core_rename,
    selection_ranges as core_selection_ranges,
};
use tower_lsp_server::jsonrpc::{Error, Result};
use tower_lsp_server::ls_types::{
    DocumentSymbol, DocumentSymbolResponse, FoldingRange, FoldingRangeKind, GotoDefinitionResponse,
    Hover, HoverContents, Location, MarkedString, MarkupContent, MarkupKind, Position,
    PrepareRenameResponse, Range, RenameParams, SelectionRange, SymbolInformation, SymbolKind, Uri,
    WorkspaceEdit,
};

#[allow(deprecated)]
#[cfg(test)]
pub fn document_symbols(snapshot: &DocumentSnapshot) -> DocumentSymbolResponse {
    document_symbols_with_hierarchy_support(snapshot, true)
}

#[allow(deprecated)]
pub fn document_symbols_with_hierarchy_support(
    snapshot: &DocumentSnapshot,
    hierarchical_supported: bool,
) -> DocumentSymbolResponse {
    let symbols = core_document_symbols(snapshot.as_editor());
    if !hierarchical_supported {
        let mut flat = Vec::new();
        flatten_document_symbols(symbols, snapshot.uri(), None, &mut flat);
        return DocumentSymbolResponse::Flat(flat);
    }

    DocumentSymbolResponse::Nested(symbols.into_iter().map(document_symbol_to_lsp).collect())
}

#[cfg(test)]
pub fn hover(snapshot: &DocumentSnapshot, position: Position) -> Option<Hover> {
    hover_with_profile(snapshot, position, &ClientProtocolProfile::permissive())
}

pub(crate) fn hover_with_profile(
    snapshot: &DocumentSnapshot,
    position: Position,
    profile: &ClientProtocolProfile,
) -> Option<Hover> {
    core_hover(snapshot.as_editor(), core_position_from_lsp(position))
        .map(|hover| hover_to_lsp(hover, profile.hover))
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
        .map(|location| location_to_lsp(location, snapshot.uri()))
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
            .map(|location| location_to_lsp(location, snapshot.uri()))
            .collect()
    })
}

pub fn prepare_rename(
    snapshot: &DocumentSnapshot,
    position: Position,
) -> Option<PrepareRenameResponse> {
    core_prepare_rename(snapshot.as_editor(), core_position_from_lsp(position)).map(prepare_to_lsp)
}

#[cfg(test)]
pub fn rename(snapshot: &DocumentSnapshot, params: RenameParams) -> Result<Option<WorkspaceEdit>> {
    rename_with_workspace_edit_encoding(snapshot, params, WorkspaceEditEncoding::DocumentChanges)
}

pub fn rename_with_workspace_edit_encoding(
    snapshot: &DocumentSnapshot,
    params: RenameParams,
    workspace_edit_encoding: WorkspaceEditEncoding,
) -> Result<Option<WorkspaceEdit>> {
    let position = params.text_document_position.position;
    core_rename(
        snapshot.as_editor(),
        core_position_from_lsp(position),
        &params.new_name,
    )
    .map(|edit| {
        edit.and_then(|edit| {
            let changes = edit.changes.into_iter().flat_map(|(uri, edits)| {
                edits.into_iter().map(move |edit| {
                    WorkspaceEditChange::new(uri.clone(), edit.range, edit.new_text)
                })
            });
            project_workspace_edit(
                changes,
                snapshot.uri(),
                snapshot.version(),
                workspace_edit_encoding,
            )
        })
    })
    .map_err(rename_error_to_lsp)
}

fn hover_to_lsp(hover: EditorHover, markup: MarkupPreference) -> Hover {
    let contents = match markup {
        MarkupPreference::Markdown => HoverContents::Markup(MarkupContent {
            kind: MarkupKind::Markdown,
            value: hover.contents.value,
        }),
        MarkupPreference::PlainText => HoverContents::Markup(MarkupContent {
            kind: MarkupKind::PlainText,
            value: generated_markdown_to_plain_text(&hover.contents.value),
        }),
        MarkupPreference::String => HoverContents::Scalar(MarkedString::String(
            generated_markdown_to_plain_text(&hover.contents.value),
        )),
    };
    Hover {
        contents,
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
fn flatten_document_symbols(
    symbols: Vec<EditorDocumentSymbol>,
    uri: &Uri,
    container_name: Option<String>,
    out: &mut Vec<SymbolInformation>,
) {
    for symbol in symbols {
        let child_container = Some(symbol.name.clone());
        out.push(SymbolInformation {
            name: symbol.name,
            kind: symbol_kind(symbol.kind),
            tags: None,
            deprecated: None,
            location: Location::new(uri.clone(), range_to_lsp(symbol.range)),
            container_name: container_name.clone(),
        });
        flatten_document_symbols(symbol.children, uri, child_container, out);
    }
}

fn prepare_to_lsp(rename: EditorPrepareRename) -> PrepareRenameResponse {
    PrepareRenameResponse::RangeWithPlaceholder {
        range: range_to_lsp(rename.range),
        placeholder: rename.placeholder,
    }
}

fn rename_error_to_lsp(error: RenameError) -> Error {
    Error::invalid_params(error.to_string())
}

fn symbol_kind(kind: EditorSemanticKind) -> SymbolKind {
    match kind {
        EditorSemanticKind::Class => SymbolKind::CLASS,
        EditorSemanticKind::Event => SymbolKind::EVENT,
        EditorSemanticKind::Function => SymbolKind::FUNCTION,
        EditorSemanticKind::Module => SymbolKind::MODULE,
        EditorSemanticKind::Namespace => SymbolKind::NAMESPACE,
        EditorSemanticKind::Object => SymbolKind::OBJECT,
        EditorSemanticKind::Package => SymbolKind::PACKAGE,
        EditorSemanticKind::Property => SymbolKind::PROPERTY,
        EditorSemanticKind::String => SymbolKind::STRING,
        EditorSemanticKind::Struct => SymbolKind::STRUCT,
        EditorSemanticKind::Variable => SymbolKind::VARIABLE,
    }
}

#[cfg(test)]
mod tests {
    use super::{
        document_symbols, document_symbols_with_hierarchy_support, folding_ranges, goto_definition,
        hover, prepare_rename, references, rename, rename_with_workspace_edit_encoding,
        selection_ranges,
    };
    use crate::snapshot::snapshot_for_test;
    use crate::workspace_edit::WorkspaceEditEncoding;
    use std::str::FromStr;
    use tower_lsp_server::ls_types::{
        DocumentChanges, DocumentSymbolResponse, FoldingRangeKind, GotoDefinitionResponse,
        HoverContents, Position, PrepareRenameResponse, RenameParams, TextDocumentIdentifier,
        TextDocumentPositionParams, Uri,
    };

    #[test]
    fn document_symbols_include_root_and_child_items() {
        let uri = Uri::from_str("file:///tmp/example.mmd").unwrap();
        let snapshot = snapshot_for_test(uri, 1, "flowchart TD\nsubgraph group\nA-->B\nend\n");

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
    fn document_symbols_can_fall_back_to_flat_symbol_information() {
        let uri = Uri::from_str("file:///tmp/example.mmd").unwrap();
        let snapshot =
            snapshot_for_test(uri.clone(), 1, "flowchart TD\nsubgraph group\nA-->B\nend\n");

        let response = document_symbols_with_hierarchy_support(&snapshot, false);
        let flat = match response {
            DocumentSymbolResponse::Flat(symbols) => symbols,
            other => panic!("unexpected symbol response: {other:?}"),
        };

        assert!(
            flat.iter()
                .any(|symbol| symbol.name == "flowchart-v2 diagram")
        );
        assert!(flat.iter().any(|symbol| {
            symbol.name == "group"
                && symbol.container_name.as_deref() == Some("flowchart-v2 diagram")
                && symbol.location.uri == uri
        }));
    }

    #[test]
    fn hover_reports_the_active_outline_entry() {
        let uri = Uri::from_str("file:///tmp/example.mmd").unwrap();
        let snapshot = snapshot_for_test(uri, 1, "flowchart TD\nA-->B\n");

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
        let uri = Uri::from_str("file:///tmp/example.mmd").unwrap();
        let snapshot = snapshot_for_test(uri, 1, "flowchart TD\nsubgraph group\nA-->B\nend\n");

        let ranges = selection_ranges(&snapshot, &[Position::new(2, 0)]).unwrap();

        assert_eq!(ranges.len(), 1);
        assert_eq!(ranges[0].range.start, Position::new(2, 0));
        assert_eq!(ranges[0].range.end, Position::new(2, 1));
        assert!(ranges[0].parent.is_some());
    }

    #[test]
    fn folding_ranges_return_lsp_regions() {
        let uri = Uri::from_str("file:///tmp/example.md").unwrap();
        let snapshot = snapshot_for_test(
            uri,
            1,
            "before\n```mermaid\nflowchart TD\nA-->B\n```\nafter\n",
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
        let uri = Uri::from_str("file:///tmp/example.mmd").unwrap();
        let snapshot = snapshot_for_test(uri, 1, "flowchart TD\nA-->B\nA-->C\n");

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
                        uri: snapshot.uri().clone(),
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
        assert_eq!(&document_changes[0].text_document.uri, snapshot.uri());
        assert_eq!(document_changes[0].text_document.version, Some(1));
        assert_eq!(document_changes[0].edits.len(), 2);

        let def = goto_definition(&snapshot, position).unwrap();
        assert!(matches!(def, GotoDefinitionResponse::Scalar(_)));
    }

    #[test]
    fn rename_can_fall_back_to_workspace_edit_changes() {
        let uri = Uri::from_str("file:///tmp/example.mmd").unwrap();
        let snapshot = snapshot_for_test(uri.clone(), 1, "flowchart TD\nA-->B\nA-->C\n");

        let edit = rename_with_workspace_edit_encoding(
            &snapshot,
            RenameParams {
                text_document_position: TextDocumentPositionParams::new(
                    TextDocumentIdentifier { uri: uri.clone() },
                    Position::new(1, 0),
                ),
                new_name: "X".to_string(),
                work_done_progress_params: Default::default(),
            },
            WorkspaceEditEncoding::Changes,
        )
        .unwrap()
        .expect("expected rename edit");

        assert!(edit.document_changes.is_none());
        let changes = edit.changes.as_ref().expect("plain changes");
        assert_eq!(changes[&uri].len(), 2);
    }
}
