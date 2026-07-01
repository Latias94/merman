use crate::snapshot::{DocumentSnapshot, FenceSnapshot};
use crate::types::{DocumentUri, Position, Range};
use merman_analysis::{
    ByteSpan, EditorSymbolKind, FenceLineItem, FenceSemanticItem, FenceTextIndexSource, SourceMap,
};
use std::collections::HashMap;
use std::fmt;

#[derive(Debug, Clone, PartialEq, Eq)]
struct OutlineItem {
    name: String,
    detail: Option<String>,
    kind: EditorSymbolKind,
    fact_source: FenceTextIndexSource,
    span: ByteSpan,
    selection: ByteSpan,
    children: Vec<OutlineItem>,
}

impl OutlineItem {
    fn to_document_symbol(&self, source_map: &SourceMap) -> Option<EditorDocumentSymbol> {
        let range = range_from_span(source_map, self.span)?;
        let selection_range = range_from_span(source_map, self.selection)?;

        Some(EditorDocumentSymbol {
            name: self.name.clone(),
            detail: self.detail.clone(),
            kind: self.kind,
            fact_source: self.fact_source,
            range,
            selection_range,
            children: self
                .children
                .iter()
                .filter_map(|child| child.to_document_symbol(source_map))
                .collect(),
        })
    }

    fn hover_markdown(&self, fence: &FenceSnapshot) -> EditorMarkupContent {
        let mut value = format!("### {}\n\n", self.name);
        if let Some(detail) = &self.detail {
            value.push_str(detail);
            value.push_str("\n\n");
        }
        if let Some(kind) = fence.diagram_type.as_deref() {
            value.push_str(&format!("Diagram: `{kind}`\n"));
        }
        value.push_str(&format!("Scope: fence {}\n", fence.index + 1));

        EditorMarkupContent { value }
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

pub fn document_symbols(snapshot: &DocumentSnapshot) -> Vec<EditorDocumentSymbol> {
    snapshot
        .fences
        .iter()
        .map(outline_for_fence)
        .filter_map(|item| item.to_document_symbol(&snapshot.source_map))
        .collect()
}

pub fn workspace_symbols(snapshot: &DocumentSnapshot, query: &str) -> Vec<EditorSymbolInformation> {
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
) -> Vec<EditorSymbolInformation> {
    let mut symbols = snapshots
        .iter()
        .flat_map(|snapshot| workspace_symbols(snapshot, query))
        .collect::<Vec<_>>();
    sort_workspace_symbols(&mut symbols);
    symbols
}

pub fn hover(snapshot: &DocumentSnapshot, position: Position) -> Option<EditorHover> {
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

    Some(EditorHover {
        contents: item.hover_markdown(fence),
        fact_source: item.fact_source,
        range,
    })
}

pub fn goto_definition(snapshot: &DocumentSnapshot, position: Position) -> Option<EditorLocation> {
    let fence = snapshot.fence_at_position(position)?;
    let offset = fence_relative_offset(snapshot, fence, position)?;
    let item = fence.text_index.entity_item_at_offset(offset)?;
    let span = absolute_span(fence, fence.text_index.first_reference_span_for_item(item)?);
    let range = range_from_span(&snapshot.source_map, span)?;
    Some(EditorLocation {
        uri: snapshot.uri.clone(),
        fact_source: fence.text_index.source(),
        range,
    })
}

pub fn references(
    snapshot: &DocumentSnapshot,
    position: Position,
    include_declaration: bool,
) -> Option<Vec<EditorLocation>> {
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
            range_from_span(&snapshot.source_map, span).map(|range| EditorLocation {
                uri: snapshot.uri.clone(),
                fact_source: fence.text_index.source(),
                range,
            })
        })
        .collect::<Vec<_>>();

    if !include_declaration
        && let Some(def_span) = fence.text_index.first_reference_span_for_item(item)
    {
        let def_span = absolute_span(fence, def_span);
        locations.retain(|location| !same_span(&snapshot.source_map, location.range, def_span));
    }

    locations.sort_by(|a, b| compare_range(&a.range, &b.range));
    Some(locations)
}

pub fn prepare_rename(
    snapshot: &DocumentSnapshot,
    position: Position,
) -> Option<EditorPrepareRename> {
    let fence = snapshot.fence_at_position(position)?;
    let offset = fence_relative_offset(snapshot, fence, position)?;
    let item = fence.text_index.entity_item_at_offset(offset)?;
    let selection = absolute_span(fence, item.selection);
    let range = range_from_span(&snapshot.source_map, selection)?;
    let placeholder = snapshot
        .text
        .get(selection.start..selection.end)?
        .to_string();
    Some(EditorPrepareRename {
        fact_source: fence.text_index.source(),
        range,
        placeholder,
    })
}

pub fn rename(
    snapshot: &DocumentSnapshot,
    position: Position,
    new_name: &str,
) -> Result<Option<EditorWorkspaceEdit>, RenameError> {
    if !is_valid_rename_name(new_name) {
        return Err(RenameError::InvalidName);
    }
    let fence = snapshot
        .fence_at_position(position)
        .ok_or(RenameError::OutsideFence)?;
    let offset =
        fence_relative_offset(snapshot, fence, position).ok_or(RenameError::NoRenameableSymbol)?;
    let item = fence
        .text_index
        .entity_item_at_offset(offset)
        .ok_or(RenameError::NoRenameableSymbol)?;
    Ok(rename_edits(snapshot, fence, item, new_name))
}

fn rename_edits(
    snapshot: &DocumentSnapshot,
    fence: &FenceSnapshot,
    item: &FenceSemanticItem,
    new_name: &str,
) -> Option<EditorWorkspaceEdit> {
    let spans = fence.text_index.reference_spans_for_item(item);
    let mut edits = spans
        .iter()
        .copied()
        .filter_map(|span| range_from_span(&snapshot.source_map, absolute_span(fence, span)))
        .map(|range| EditorTextEdit {
            fact_source: fence.text_index.source(),
            range,
            new_text: new_name.to_string(),
        })
        .collect::<Vec<_>>();
    if edits.is_empty() {
        return None;
    }
    edits.sort_by(|a, b| compare_range(&a.range, &b.range));

    let mut changes = HashMap::new();
    changes.insert(snapshot.uri.clone(), edits);
    Some(EditorWorkspaceEdit {
        fact_source: fence.text_index.source(),
        changes,
    })
}

fn outline_for_fence(fence: &FenceSnapshot) -> OutlineItem {
    OutlineItem {
        name: fence_name(fence),
        detail: fence_detail(fence),
        kind: generic_kind(fence.diagram_type.as_deref()),
        fact_source: fence.text_index.source(),
        span: ByteSpan {
            start: fence.start,
            end: fence.end,
        },
        selection: ByteSpan {
            start: fence.body_start,
            end: fence.body_end,
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
        kind: item.kind,
        fact_source: fence.text_index.source(),
        span: absolute_span(fence, item.span),
        selection: absolute_span(fence, item.selection),
        children: Vec::new(),
    }
}

fn outline_item_from_semantic(fence: &FenceSnapshot, item: &FenceSemanticItem) -> OutlineItem {
    OutlineItem {
        name: item.name.clone(),
        detail: item.detail.clone(),
        kind: item.kind,
        fact_source: fence.text_index.source(),
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
    symbols: &mut Vec<EditorSymbolInformation>,
) {
    if query.is_none_or(|query| workspace_symbol_matches(&item.name, query))
        && let Some(location) = workspace_symbol_location(snapshot, item)
    {
        symbols.push(EditorSymbolInformation {
            name: item.name.clone(),
            kind: item.kind,
            fact_source: item.fact_source,
            location,
            container_name: container_name.map(str::to_string),
        });
    }

    let container_name = Some(item.name.as_str());
    for child in &item.children {
        collect_workspace_symbols(snapshot, child, container_name, query, symbols);
    }
}

fn workspace_symbol_matches(name: &str, query: &str) -> bool {
    name.to_lowercase().contains(query)
}

fn workspace_symbol_location(
    snapshot: &DocumentSnapshot,
    item: &OutlineItem,
) -> Option<EditorLocation> {
    let range = range_from_span(&snapshot.source_map, item.selection)?;
    Some(EditorLocation {
        uri: snapshot.uri.clone(),
        fact_source: item.fact_source,
        range,
    })
}

pub fn sort_workspace_symbols(symbols: &mut [EditorSymbolInformation]) {
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

fn range_from_span(source_map: &SourceMap, span: ByteSpan) -> Option<Range> {
    source_map.span(span.start, span.end).ok().map(|span| {
        Range::new(
            Position::new(span.lsp_range.start.line, span.lsp_range.start.character),
            Position::new(span.lsp_range.end.line, span.lsp_range.end.character),
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

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EditorDocumentSymbol {
    pub name: String,
    pub detail: Option<String>,
    pub kind: EditorSymbolKind,
    pub fact_source: FenceTextIndexSource,
    pub range: Range,
    pub selection_range: Range,
    pub children: Vec<EditorDocumentSymbol>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EditorSymbolInformation {
    pub name: String,
    pub kind: EditorSymbolKind,
    pub fact_source: FenceTextIndexSource,
    pub location: EditorLocation,
    pub container_name: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EditorLocation {
    pub uri: DocumentUri,
    pub fact_source: FenceTextIndexSource,
    pub range: Range,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EditorHover {
    pub contents: EditorMarkupContent,
    pub fact_source: FenceTextIndexSource,
    pub range: Option<Range>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EditorMarkupContent {
    pub value: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EditorPrepareRename {
    pub fact_source: FenceTextIndexSource,
    pub range: Range,
    pub placeholder: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EditorWorkspaceEdit {
    pub fact_source: FenceTextIndexSource,
    pub changes: HashMap<DocumentUri, Vec<EditorTextEdit>>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EditorTextEdit {
    pub fact_source: FenceTextIndexSource,
    pub range: Range,
    pub new_text: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RenameError {
    InvalidName,
    OutsideFence,
    NoRenameableSymbol,
}

impl fmt::Display for RenameError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let message = match self {
            Self::InvalidName => "new name must use letters, numbers, underscore, or dash",
            Self::OutsideFence => "position is outside a Mermaid fence",
            Self::NoRenameableSymbol => "no renameable symbol at the requested position",
        };
        formatter.write_str(message)
    }
}
