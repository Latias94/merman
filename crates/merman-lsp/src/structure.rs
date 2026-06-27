use crate::snapshot::{DocumentSnapshot, FenceSnapshot};
use merman_analysis::{ByteSpan, EditorSymbolKind, FenceLineItem, FenceSemanticItem, SourceMap};
use std::collections::HashMap;
use tower_lsp::jsonrpc::{Error, Result};
use tower_lsp::lsp_types::{
    DocumentSymbol, DocumentSymbolResponse, GotoDefinitionResponse, Hover, HoverContents, Location,
    MarkupContent, MarkupKind, Position, PrepareRenameResponse, Range, RenameParams,
    SymbolInformation, SymbolKind, TextEdit, WorkspaceEdit,
};

#[derive(Debug, Clone)]
struct OutlineItem {
    name: String,
    detail: Option<String>,
    kind: SymbolKind,
    span: ByteSpan,
    selection: ByteSpan,
    children: Vec<OutlineItem>,
}

impl OutlineItem {
    #[allow(deprecated)]
    fn to_document_symbol(&self, source_map: &SourceMap) -> Option<DocumentSymbol> {
        let range = range_from_span(source_map, self.span)?;
        let selection_range = range_from_span(source_map, self.selection)?;

        Some(DocumentSymbol {
            name: self.name.clone(),
            detail: self.detail.clone(),
            kind: self.kind,
            tags: None,
            deprecated: None,
            range,
            selection_range,
            children: if self.children.is_empty() {
                None
            } else {
                Some(
                    self.children
                        .iter()
                        .filter_map(|child| child.to_document_symbol(source_map))
                        .collect(),
                )
            },
        })
    }

    fn hover_markdown(&self, fence: &FenceSnapshot) -> MarkupContent {
        let mut value = format!("### {}\n\n", self.name);
        if let Some(detail) = &self.detail {
            value.push_str(detail);
            value.push_str("\n\n");
        }
        if let Some(kind) = fence.diagram_type.as_deref() {
            value.push_str(&format!("Diagram: `{kind}`\n"));
        }
        value.push_str(&format!("Scope: fence {}\n", fence.index + 1));

        MarkupContent {
            kind: MarkupKind::Markdown,
            value,
        }
    }

    fn find_deepest(&self, offset: usize) -> Option<&OutlineItem> {
        if !self.span.contains(offset) {
            return None;
        }

        for child in &self.children {
            if let Some(found) = child.find_deepest(offset) {
                return Some(found);
            }
        }

        Some(self)
    }
}

pub fn document_symbols(snapshot: &DocumentSnapshot) -> DocumentSymbolResponse {
    let symbols = snapshot
        .fences
        .iter()
        .map(outline_for_fence)
        .filter_map(|item| item.to_document_symbol(&snapshot.source_map))
        .collect::<Vec<_>>();

    DocumentSymbolResponse::Nested(symbols)
}

#[allow(deprecated)]
pub fn workspace_symbols(snapshot: &DocumentSnapshot, query: &str) -> Vec<SymbolInformation> {
    let query = query.trim();
    let query = if query.is_empty() {
        None
    } else {
        Some(query.to_lowercase())
    };
    let mut symbols = Vec::new();
    for fence in &snapshot.fences {
        let outline = outline_for_fence(fence);
        collect_workspace_symbols(snapshot, &outline, None, query.as_deref(), &mut symbols);
    }

    sort_workspace_symbols(&mut symbols);
    symbols
}

pub fn workspace_symbols_for_snapshots(
    snapshots: &[DocumentSnapshot],
    query: &str,
) -> Vec<SymbolInformation> {
    let mut symbols = snapshots
        .iter()
        .flat_map(|snapshot| workspace_symbols(snapshot, query))
        .collect::<Vec<_>>();
    sort_workspace_symbols(&mut symbols);
    symbols
}

pub fn hover(snapshot: &DocumentSnapshot, position: Position) -> Option<Hover> {
    let fence = snapshot.fence_at_position(position)?;
    let absolute_offset = snapshot.byte_offset_for_position(position)?;
    let relative_offset = fence_relative_offset(snapshot, fence, position)?;
    let outline = outline_for_fence(fence);
    let item = fence
        .text_index
        .semantic_item_at_offset(relative_offset)
        .map(|item| outline_item_from_semantic(fence, item))
        .or_else(|| outline.find_deepest(absolute_offset).cloned())
        .unwrap_or(outline);
    let range = range_from_span(&snapshot.source_map, item.selection).or_else(|| {
        range_from_span(
            &snapshot.source_map,
            ByteSpan {
                start: fence.start,
                end: fence.end,
            },
        )
    });

    Some(Hover {
        contents: HoverContents::Markup(item.hover_markdown(fence)),
        range,
    })
}

pub fn goto_definition(
    snapshot: &DocumentSnapshot,
    position: Position,
) -> Option<GotoDefinitionResponse> {
    let fence = snapshot.fence_at_position(position)?;
    let offset = fence_relative_offset(snapshot, fence, position)?;
    let item = fence.text_index.entity_item_at_offset(offset)?;
    let span = absolute_span(fence, fence.text_index.first_reference_span_for_item(item)?);
    let range = range_from_span(&snapshot.source_map, span)?;
    Some(Location::new(snapshot.uri.clone(), range).into())
}

pub fn references(
    snapshot: &DocumentSnapshot,
    position: Position,
    include_declaration: bool,
) -> Option<Vec<Location>> {
    let fence = snapshot.fence_at_position(position)?;
    let offset = fence_relative_offset(snapshot, fence, position)?;
    let item = fence.text_index.entity_item_at_offset(offset)?;
    let mut locations = fence
        .text_index
        .reference_spans_for_item(item)
        .iter()
        .copied()
        .filter_map(|span| {
            let span = absolute_span(fence, span);
            range_from_span(&snapshot.source_map, span)
                .map(|range| Location::new(snapshot.uri.clone(), range))
        })
        .collect::<Vec<_>>();

    if !include_declaration {
        if let Some(def_span) = fence.text_index.first_reference_span_for_item(item) {
            let def_span = absolute_span(fence, def_span);
            locations.retain(|location| !same_span(&snapshot.source_map, location.range, def_span));
        }
    }

    locations.sort_by(|a, b| compare_range(&a.range, &b.range));
    Some(locations)
}

pub fn prepare_rename(
    snapshot: &DocumentSnapshot,
    position: Position,
) -> Option<PrepareRenameResponse> {
    let fence = snapshot.fence_at_position(position)?;
    let offset = fence_relative_offset(snapshot, fence, position)?;
    let item = fence.text_index.entity_item_at_offset(offset)?;
    let selection = absolute_span(fence, item.selection);
    let range = range_from_span(&snapshot.source_map, selection)?;
    let placeholder = snapshot
        .text
        .get(selection.start..selection.end)?
        .to_string();
    Some(PrepareRenameResponse::RangeWithPlaceholder { range, placeholder })
}

pub fn rename(snapshot: &DocumentSnapshot, params: RenameParams) -> Result<Option<WorkspaceEdit>> {
    if !is_valid_rename_name(&params.new_name) {
        return Err(Error::invalid_params(
            "new name must use letters, numbers, underscore, or dash",
        ));
    }
    let position = params.text_document_position.position;
    let fence = snapshot
        .fence_at_position(position)
        .ok_or_else(|| Error::invalid_params("position is outside a Mermaid fence"))?;
    let offset = fence_relative_offset(snapshot, fence, position)
        .ok_or_else(|| Error::invalid_params("no renameable symbol at the requested position"))?;
    let item = fence
        .text_index
        .entity_item_at_offset(offset)
        .ok_or_else(|| Error::invalid_params("no renameable symbol at the requested position"))?;
    Ok(rename_edits(snapshot, fence, item, &params.new_name))
}

fn rename_edits(
    snapshot: &DocumentSnapshot,
    fence: &FenceSnapshot,
    item: &FenceSemanticItem,
    new_name: &str,
) -> Option<WorkspaceEdit> {
    let spans = fence.text_index.reference_spans_for_item(item);
    let mut edits = spans
        .iter()
        .copied()
        .filter_map(|span| range_from_span(&snapshot.source_map, absolute_span(fence, span)))
        .map(|range| TextEdit::new(range, new_name.to_string()))
        .collect::<Vec<_>>();
    if edits.is_empty() {
        return None;
    }
    edits.sort_by(|a, b| compare_range(&a.range, &b.range));

    let mut changes = HashMap::new();
    changes.insert(snapshot.uri.clone(), edits);
    Some(WorkspaceEdit {
        changes: Some(changes),
        document_changes: None,
        change_annotations: None,
    })
}

fn outline_for_fence(fence: &FenceSnapshot) -> OutlineItem {
    OutlineItem {
        name: fence_name(fence),
        detail: fence_detail(fence),
        kind: fence_kind(fence),
        span: ByteSpan {
            start: fence.start,
            end: fence.end,
        },
        selection: ByteSpan {
            start: fence.body_start,
            end: fence.body_start.saturating_add(fence.text.len()),
        },
        children: outline_children(fence),
    }
}

fn outline_children(fence: &FenceSnapshot) -> Vec<OutlineItem> {
    fence
        .text_index
        .outline_items()
        .iter()
        .map(|item| outline_item_from_index(fence, item))
        .collect()
}

fn outline_item_from_index(fence: &FenceSnapshot, item: &FenceLineItem) -> OutlineItem {
    OutlineItem {
        name: item.name.clone(),
        detail: item.detail.clone(),
        kind: symbol_kind(item.kind),
        span: absolute_span(fence, item.span),
        selection: absolute_span(fence, item.selection),
        children: Vec::new(),
    }
}

fn outline_item_from_semantic(fence: &FenceSnapshot, item: &FenceSemanticItem) -> OutlineItem {
    OutlineItem {
        name: item.name.clone(),
        detail: item.detail.clone(),
        kind: symbol_kind(item.kind),
        span: absolute_span(fence, item.span),
        selection: absolute_span(fence, item.selection),
        children: Vec::new(),
    }
}

fn collect_workspace_symbols(
    snapshot: &DocumentSnapshot,
    item: &OutlineItem,
    container_name: Option<&str>,
    query: Option<&str>,
    symbols: &mut Vec<SymbolInformation>,
) {
    if query.is_none_or(|query| workspace_symbol_matches(&item.name, query))
        && let Some(location) = workspace_symbol_location(snapshot, item)
    {
        #[allow(deprecated)]
        {
            symbols.push(SymbolInformation {
                name: item.name.clone(),
                kind: item.kind,
                tags: None,
                deprecated: None,
                location,
                container_name: container_name.map(str::to_string),
            });
        }
    }

    let container_name = Some(item.name.as_str());
    for child in &item.children {
        collect_workspace_symbols(snapshot, child, container_name, query, symbols);
    }
}

fn workspace_symbol_matches(name: &str, query: &str) -> bool {
    name.to_lowercase().contains(query)
}

fn workspace_symbol_location(snapshot: &DocumentSnapshot, item: &OutlineItem) -> Option<Location> {
    let range = range_from_span(&snapshot.source_map, item.selection)?;
    Some(Location::new(snapshot.uri.clone(), range))
}

pub(crate) fn sort_workspace_symbols(symbols: &mut [SymbolInformation]) {
    symbols.sort_by(|left, right| {
        left.name
            .cmp(&right.name)
            .then_with(|| left.location.uri.as_str().cmp(right.location.uri.as_str()))
            .then_with(|| compare_range(&left.location.range, &right.location.range))
            .then_with(|| left.container_name.cmp(&right.container_name))
    });
}

fn fence_relative_offset(
    snapshot: &DocumentSnapshot,
    fence: &FenceSnapshot,
    position: Position,
) -> Option<usize> {
    let offset = snapshot.byte_offset_for_position(position)?;
    (offset >= fence.body_start).then_some(offset - fence.body_start)
}

fn absolute_span(fence: &FenceSnapshot, span: ByteSpan) -> ByteSpan {
    ByteSpan {
        start: fence.body_start + span.start,
        end: fence.body_start + span.end,
    }
}

fn fence_kind(fence: &FenceSnapshot) -> SymbolKind {
    symbol_kind(generic_kind(fence.diagram_type.as_deref()))
}

fn fence_name(fence: &FenceSnapshot) -> String {
    match fence.diagram_type.as_deref() {
        Some(kind) => format!("{kind} diagram"),
        None => "Mermaid diagram".to_string(),
    }
}

fn fence_detail(fence: &FenceSnapshot) -> Option<String> {
    let mut parts = Vec::new();

    if let Some(kind) = fence.diagram_type.as_deref() {
        parts.push(format!("diagram type `{kind}`"));
    }

    let prefixes = fence
        .text_index
        .directive_prefixes()
        .cloned()
        .collect::<Vec<_>>();
    if !prefixes.is_empty() {
        parts.push(format!("directives {}", prefixes.join(", ")));
    }

    if parts.is_empty() {
        None
    } else {
        Some(parts.join(" · "))
    }
}

fn generic_kind(diagram_type: Option<&str>) -> EditorSymbolKind {
    match diagram_type {
        Some("sequence") => EditorSymbolKind::Event,
        Some("state") => EditorSymbolKind::Class,
        Some("mindmap") => EditorSymbolKind::Namespace,
        Some("class") => EditorSymbolKind::Class,
        Some("er") => EditorSymbolKind::Struct,
        Some("block") => EditorSymbolKind::Object,
        Some("flowchart-v2") | Some("flowchart-elk") => EditorSymbolKind::Module,
        _ => EditorSymbolKind::Variable,
    }
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

fn range_from_span(source_map: &SourceMap, span: ByteSpan) -> Option<Range> {
    source_map.span(span.start, span.end).ok().map(|span| {
        Range::new(
            Position::new(
                span.lsp_range.start.line as u32,
                span.lsp_range.start.character as u32,
            ),
            Position::new(
                span.lsp_range.end.line as u32,
                span.lsp_range.end.character as u32,
            ),
        )
    })
}

fn same_span(source_map: &SourceMap, range: Range, span: ByteSpan) -> bool {
    let Some(expected) = range_from_span(source_map, span) else {
        return false;
    };
    expected == range
}

fn compare_range(left: &Range, right: &Range) -> std::cmp::Ordering {
    (
        left.start.line,
        left.start.character,
        left.end.line,
        left.end.character,
    )
        .cmp(&(
            right.start.line,
            right.start.character,
            right.end.line,
            right.end.character,
        ))
}

fn is_valid_rename_name(new_name: &str) -> bool {
    !new_name.is_empty()
        && new_name
            .chars()
            .all(|ch| ch.is_alphanumeric() || matches!(ch, '_' | '-'))
}

pub fn outline_hover(snapshot: &DocumentSnapshot, position: Position) -> Option<Hover> {
    hover(snapshot, position)
}

pub fn outline_document_symbols(snapshot: &DocumentSnapshot) -> DocumentSymbolResponse {
    document_symbols(snapshot)
}

pub fn outline_definition(
    snapshot: &DocumentSnapshot,
    position: Position,
) -> Option<GotoDefinitionResponse> {
    goto_definition(snapshot, position)
}

pub fn outline_references(
    snapshot: &DocumentSnapshot,
    position: Position,
    include_declaration: bool,
) -> Option<Vec<Location>> {
    references(snapshot, position, include_declaration)
}

pub fn outline_prepare_rename(
    snapshot: &DocumentSnapshot,
    position: Position,
) -> Option<PrepareRenameResponse> {
    prepare_rename(snapshot, position)
}

pub fn outline_rename(
    snapshot: &DocumentSnapshot,
    params: RenameParams,
) -> Result<Option<WorkspaceEdit>> {
    rename(snapshot, params)
}

#[cfg(test)]
mod tests {
    use super::{
        outline_definition, outline_document_symbols, outline_hover, outline_prepare_rename,
        outline_references, outline_rename, workspace_symbols,
    };
    use crate::document_store::DocumentStore;
    use crate::snapshot::{DocumentSnapshot, FenceSnapshot};
    use merman_analysis::{FenceTextIndex, SourceMap};
    use merman_core::{EditorSemanticFacts, EditorSemanticKind, EditorSemanticSymbol, SourceSpan};
    use tower_lsp::lsp_types::{
        DocumentSymbolResponse, GotoDefinitionResponse, HoverContents, Position,
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

        let response = outline_document_symbols(&snapshot);
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

        let hover = outline_hover(&snapshot, Position::new(1, 0)).unwrap();
        let text = match hover.contents {
            HoverContents::Markup(markup) => markup.value,
            other => panic!("unexpected hover contents: {other:?}"),
        };

        assert!(text.contains("A"));
        assert!(text.contains("Diagram:"));
    }

    #[test]
    fn hover_reports_payload_semantic_items() {
        let mut store = DocumentStore::new();
        let uri = Url::parse("file:///tmp/example.mmd").unwrap();
        let snapshot = store.upsert(
            uri,
            1,
            "sequenceDiagram\ntitle: Diagram Title\nAlice->>Bob: Hello\n".to_string(),
        );

        let hover = outline_hover(&snapshot, Position::new(1, 8)).unwrap();
        let text = match hover.contents {
            HoverContents::Markup(markup) => markup.value,
            other => panic!("unexpected hover contents: {other:?}"),
        };

        assert!(text.contains("Diagram Title"));
        assert!(text.contains("sequence title"));
    }

    #[test]
    fn payload_semantic_items_are_not_navigation_targets() {
        let mut store = DocumentStore::new();
        let uri = Url::parse("file:///tmp/example.mmd").unwrap();
        let snapshot = store.upsert(
            uri,
            1,
            "sequenceDiagram\ntitle: Diagram Title\nAlice->>Bob: Hello\n".to_string(),
        );

        let position = Position::new(1, 8);
        assert!(outline_definition(&snapshot, position).is_none());
        assert!(outline_references(&snapshot, position, true).is_none());
        assert!(outline_prepare_rename(&snapshot, position).is_none());
    }

    #[test]
    fn rename_and_references_track_simple_identifiers() {
        let mut store = DocumentStore::new();
        let uri = Url::parse("file:///tmp/example.mmd").unwrap();
        let snapshot = store.upsert(uri, 1, "flowchart TD\nA-->B\nA-->C\n".to_string());

        let position = Position::new(1, 0);
        let prepare = outline_prepare_rename(&snapshot, position).unwrap();
        match prepare {
            PrepareRenameResponse::RangeWithPlaceholder { placeholder, .. } => {
                assert_eq!(placeholder, "A");
            }
            other => panic!("unexpected prepare rename response: {other:?}"),
        }

        let refs = outline_references(&snapshot, position, true).unwrap();
        assert_eq!(refs.len(), 2);

        let rename = outline_rename(
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

        let def = outline_definition(&snapshot, position).unwrap();
        assert!(matches!(def, GotoDefinitionResponse::Scalar(_)));
    }

    #[test]
    fn gantt_rename_and_references_track_dependency_refs() {
        let mut store = DocumentStore::new();
        let uri = Url::parse("file:///tmp/example.mmd").unwrap();
        let snapshot = store.upsert(
            uri,
            1,
            "gantt\ndateFormat YYYY-MM-DD\nsection Demo\nTask 1: id1,2014-01-01,1d\nTask 2: id2,after id1,1d\nclick id2 href \"https://example.com/\"\n"
                .to_string(),
        );

        let position = Position::new(3, 8);
        let prepare = outline_prepare_rename(&snapshot, position).unwrap();
        match prepare {
            PrepareRenameResponse::RangeWithPlaceholder { placeholder, .. } => {
                assert_eq!(placeholder, "id1");
            }
            other => panic!("unexpected prepare rename response: {other:?}"),
        }

        let refs = outline_references(&snapshot, position, true).unwrap();
        assert_eq!(refs.len(), 2);

        let rename = outline_rename(
            &snapshot,
            RenameParams {
                text_document_position: TextDocumentPositionParams::new(
                    TextDocumentIdentifier {
                        uri: snapshot.uri.clone(),
                    },
                    position,
                ),
                new_name: "task_alpha".to_string(),
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
            2,
            "rename should update the task and its dependency reference"
        );
    }

    #[test]
    fn gantt_click_targets_share_task_reference_groups() {
        let mut store = DocumentStore::new();
        let uri = Url::parse("file:///tmp/example.mmd").unwrap();
        let snapshot = store.upsert(
            uri,
            1,
            "gantt\ndateFormat YYYY-MM-DD\nsection Demo\nTask 1: id1,2014-01-01,1d\nclick id1 href \"https://example.com/\"\n"
                .to_string(),
        );

        let position = Position::new(4, 8);
        let prepare = outline_prepare_rename(&snapshot, position).unwrap();
        match prepare {
            PrepareRenameResponse::RangeWithPlaceholder { placeholder, .. } => {
                assert_eq!(placeholder, "id1");
            }
            other => panic!("unexpected prepare rename response: {other:?}"),
        }

        let refs = outline_references(&snapshot, position, true).unwrap();
        assert_eq!(refs.len(), 2);

        let rename = outline_rename(
            &snapshot,
            RenameParams {
                text_document_position: TextDocumentPositionParams::new(
                    TextDocumentIdentifier {
                        uri: snapshot.uri.clone(),
                    },
                    position,
                ),
                new_name: "task_beta".to_string(),
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
            2,
            "rename should update the task and its click target reference"
        );
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

    fn typed_reference_snapshot() -> DocumentSnapshot {
        let text = "Shared\nShared\n".to_string();
        let mut facts = EditorSemanticFacts::new();
        facts.push_symbol(EditorSemanticSymbol::new(
            "Shared",
            Some("module entity".to_string()),
            EditorSemanticKind::Module,
            SourceSpan::new(0, 6),
            SourceSpan::new(0, 6),
        ));
        facts.push_symbol(EditorSemanticSymbol::new(
            "Shared",
            Some("property entity".to_string()),
            EditorSemanticKind::Property,
            SourceSpan::new(7, 13),
            SourceSpan::new(7, 13),
        ));

        let text_index = FenceTextIndex::from_core_facts(facts);
        DocumentSnapshot {
            uri: Url::parse("file:///tmp/example.mmd").unwrap(),
            version: 1,
            text: text.clone(),
            source_map: SourceMap::new(text.clone()),
            fences: vec![FenceSnapshot {
                index: 0,
                start: 0,
                body_start: 0,
                end: text.len(),
                text,
                diagram_type: Some("flowchart-v2".to_string()),
                text_index,
            }],
        }
    }

    #[test]
    fn typed_reference_groups_keep_same_name_different_kinds_separate() {
        let snapshot = typed_reference_snapshot();

        let module_refs = outline_references(&snapshot, Position::new(0, 0), true).unwrap();
        let property_refs = outline_references(&snapshot, Position::new(1, 0), true).unwrap();

        assert_eq!(module_refs.len(), 1);
        assert_eq!(property_refs.len(), 1);

        let module_rename = outline_rename(
            &snapshot,
            RenameParams {
                text_document_position: TextDocumentPositionParams::new(
                    TextDocumentIdentifier {
                        uri: snapshot.uri.clone(),
                    },
                    Position::new(0, 0),
                ),
                new_name: "ModuleShared".to_string(),
                work_done_progress_params: Default::default(),
            },
        )
        .unwrap()
        .unwrap();
        let module_edits = module_rename.changes.unwrap();
        assert_eq!(
            module_edits.get(&snapshot.uri).unwrap().len(),
            1,
            "rename should only touch the module group"
        );
    }
}
