use crate::snapshot::{DocumentSnapshot, FenceSnapshot};
use merman_analysis::SourceMap;
use tower_lsp::lsp_types::{
    DocumentSymbol, DocumentSymbolResponse, Hover, HoverContents, MarkupContent, MarkupKind,
    Position, Range, SymbolKind,
};

#[derive(Debug, Clone, Copy)]
struct ByteSpan {
    start: usize,
    end: usize,
}

impl ByteSpan {
    fn contains(self, offset: usize) -> bool {
        offset >= self.start && offset <= self.end
    }
}

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
        .filter_map(|fence| outline_for_fence(snapshot, fence))
        .filter_map(|item| item.to_document_symbol(&snapshot.source_map))
        .collect::<Vec<_>>();

    DocumentSymbolResponse::Nested(symbols)
}

pub fn hover(snapshot: &DocumentSnapshot, position: Position) -> Option<Hover> {
    let offset = snapshot.byte_offset_for_position(position)?;
    let fence = snapshot.fence_at_position(position)?;
    let outline = outline_for_fence(snapshot, fence)?;
    let item = outline.find_deepest(offset).unwrap_or(&outline);
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

fn outline_for_fence(snapshot: &DocumentSnapshot, fence: &FenceSnapshot) -> Option<OutlineItem> {
    let root_span = ByteSpan {
        start: fence.start,
        end: fence.end,
    };
    let selection = ByteSpan {
        start: fence.body_start,
        end: fence.body_start.saturating_add(fence.text.len()),
    };

    let children = outline_children(snapshot, fence);
    let name = fence_name(fence);

    Some(OutlineItem {
        name,
        detail: fence_detail(fence),
        kind: fence_kind(fence),
        span: root_span,
        selection,
        children,
    })
}

fn outline_children(snapshot: &DocumentSnapshot, fence: &FenceSnapshot) -> Vec<OutlineItem> {
    let mut items = Vec::new();
    let mut relative_start = 0usize;

    for line in fence.text.split_inclusive('\n') {
        let line_end = relative_start + line.len();
        let line_no_newline = line.strip_suffix('\n').unwrap_or(line);
        let trimmed = line_no_newline.trim_start();
        let leading = line_no_newline.len().saturating_sub(trimmed.len());

        if let Some(item) = outline_item_for_line(
            fence.diagram_type.as_deref(),
            trimmed,
            fence.body_start + relative_start + leading,
            fence.body_start + line_end,
        ) {
            if item.span.start <= item.span.end {
                items.push(item);
            }
        }

        relative_start = line_end;
    }

    // If there was no trailing newline, the final line was not yielded by split_inclusive.
    if !fence.text.ends_with('\n') && relative_start < fence.text.len() {
        let line_no_newline = &fence.text[relative_start..];
        let trimmed = line_no_newline.trim_start();
        let leading = line_no_newline.len().saturating_sub(trimmed.len());
        if let Some(item) = outline_item_for_line(
            fence.diagram_type.as_deref(),
            trimmed,
            fence.body_start + relative_start + leading,
            fence.body_start + fence.text.len(),
        ) {
            if item.span.start <= item.span.end {
                items.push(item);
            }
        }
    }

    // Deduplicate by byte span and name so node identifiers repeated across the same fence do not
    // produce a noisy outline.
    items.sort_by_key(|item| (item.span.start, item.span.end, item.name.clone()));
    items.dedup_by(|left, right| {
        left.span.start == right.span.start
            && left.span.end == right.span.end
            && left.name == right.name
    });
    let _ = snapshot;
    items
}

fn outline_item_for_line(
    diagram_type: Option<&str>,
    trimmed: &str,
    abs_start: usize,
    abs_end: usize,
) -> Option<OutlineItem> {
    if trimmed.is_empty()
        || is_header_line(trimmed)
        || trimmed.starts_with("%%")
        || trimmed.starts_with(":::")
    {
        return None;
    }

    if let Some(item) = special_line_item(diagram_type, trimmed, abs_start, abs_end) {
        return Some(item);
    }

    let (name, selection) = first_symbol_token(trimmed, abs_start)?;
    Some(OutlineItem {
        name,
        detail: Some("diagram element".to_string()),
        kind: SymbolKind::VARIABLE,
        span: ByteSpan {
            start: abs_start,
            end: abs_end,
        },
        selection,
        children: Vec::new(),
    })
}

fn special_line_item(
    diagram_type: Option<&str>,
    trimmed: &str,
    abs_start: usize,
    abs_end: usize,
) -> Option<OutlineItem> {
    if let Some(rest) = trimmed.strip_prefix("subgraph ") {
        let (name, selection) = token_after_prefix(trimmed, "subgraph", abs_start)?;
        return Some(OutlineItem {
            name: if rest.trim().is_empty() {
                "subgraph".to_string()
            } else {
                name
            },
            detail: Some("subgraph".to_string()),
            kind: SymbolKind::NAMESPACE,
            span: ByteSpan {
                start: abs_start,
                end: abs_end,
            },
            selection,
            children: Vec::new(),
        });
    }

    if let Some((keyword, kind, detail)) = [
        ("participant", SymbolKind::VARIABLE, "sequence participant"),
        ("actor", SymbolKind::VARIABLE, "sequence actor"),
        ("box", SymbolKind::PACKAGE, "sequence box"),
        ("note", SymbolKind::EVENT, "note"),
        ("state", SymbolKind::CLASS, "state"),
        ("classDef", SymbolKind::PROPERTY, "class definition"),
        ("class", SymbolKind::CLASS, "class assignment"),
        ("style", SymbolKind::PROPERTY, "style"),
        ("click", SymbolKind::FUNCTION, "interaction"),
        ("linkStyle", SymbolKind::PROPERTY, "link style"),
        ("accTitle", SymbolKind::STRING, "accessibility title"),
        ("accDescr", SymbolKind::STRING, "accessibility description"),
        ("title", SymbolKind::STRING, "title"),
    ]
    .into_iter()
    .find_map(|(keyword, kind, detail)| {
        trimmed
            .strip_prefix(keyword)
            .map(|_| (keyword, kind, detail))
    }) {
        let (name, selection) = token_after_prefix(trimmed, keyword, abs_start)?;
        return Some(OutlineItem {
            name,
            detail: Some(detail.to_string()),
            kind,
            span: ByteSpan {
                start: abs_start,
                end: abs_end,
            },
            selection,
            children: Vec::new(),
        });
    }

    if matches!(diagram_type, Some("mindmap")) {
        let (name, selection) = first_symbol_token(trimmed, abs_start)?;
        return Some(OutlineItem {
            name,
            detail: Some("mindmap node".to_string()),
            kind: SymbolKind::STRING,
            span: ByteSpan {
                start: abs_start,
                end: abs_end,
            },
            selection,
            children: Vec::new(),
        });
    }

    let (name, selection) = first_symbol_token(trimmed, abs_start)?;
    Some(OutlineItem {
        name,
        detail: Some("diagram element".to_string()),
        kind: generic_kind(diagram_type),
        span: ByteSpan {
            start: abs_start,
            end: abs_end,
        },
        selection,
        children: Vec::new(),
    })
}

fn first_symbol_token(trimmed: &str, abs_start: usize) -> Option<(String, ByteSpan)> {
    let mut token_end = 0usize;
    for (idx, ch) in trimmed.char_indices() {
        if idx == 0 && matches!(ch, '[' | '(' | '{' | '<' | ':' | '%' | ';') {
            token_end = ch.len_utf8();
            break;
        }
        if ch.is_whitespace()
            || matches!(
                ch,
                '[' | ']'
                    | '('
                    | ')'
                    | '{'
                    | '}'
                    | '-'
                    | '='
                    | '.'
                    | '<'
                    | '>'
                    | '|'
                    | ':'
                    | ','
                    | ';'
            )
        {
            token_end = idx;
            break;
        }
        token_end = idx + ch.len_utf8();
    }

    if token_end == 0 {
        token_end = trimmed.len();
    }

    let token = trimmed[..token_end].trim_matches(|ch: char| matches!(ch, '[' | ']' | '(' | ')'));
    if token.is_empty() || token.starts_with('%') || is_header_line(token) {
        return None;
    }

    let leading = trimmed.len().saturating_sub(trimmed.trim_start().len());
    Some((
        token.to_string(),
        ByteSpan {
            start: abs_start + leading,
            end: abs_start + leading + token.len(),
        },
    ))
}

fn token_after_prefix(trimmed: &str, prefix: &str, abs_start: usize) -> Option<(String, ByteSpan)> {
    let rest = trimmed.strip_prefix(prefix)?.trim_start();
    let rest_offset = trimmed.len().saturating_sub(rest.len());
    let token = rest
        .split(|ch: char| ch.is_whitespace() || matches!(ch, ':' | '{' | '(' | '['))
        .next()
        .filter(|token| !token.is_empty())?;

    Some((
        token.to_string(),
        ByteSpan {
            start: abs_start + rest_offset,
            end: abs_start + rest_offset + token.len(),
        },
    ))
}

fn is_header_line(trimmed: &str) -> bool {
    matches!(
        trimmed,
        "flowchart"
            | "flowchart TD"
            | "flowchart TB"
            | "flowchart BT"
            | "flowchart LR"
            | "flowchart RL"
            | "graph"
            | "graph TD"
            | "graph TB"
            | "graph BT"
            | "graph LR"
            | "graph RL"
            | "sequenceDiagram"
            | "stateDiagram"
            | "stateDiagram-v2"
            | "mindmap"
            | "classDiagram"
            | "erDiagram"
            | "gantt"
            | "block-beta"
            | "journey"
            | "timeline"
            | "pie"
            | "quadrantChart"
            | "xychart-beta"
            | "C4Context"
            | "C4Container"
            | "C4Component"
            | "C4Dynamic"
    ) || trimmed.starts_with("flowchart ")
        || trimmed.starts_with("graph ")
        || trimmed.starts_with("sequenceDiagram ")
        || trimmed.starts_with("stateDiagram ")
        || trimmed.starts_with("stateDiagram-v2 ")
        || trimmed.starts_with("classDiagram ")
        || trimmed.starts_with("erDiagram ")
        || trimmed.starts_with("mindmap ")
        || trimmed.starts_with("gantt ")
        || trimmed.starts_with("block-beta ")
        || trimmed.starts_with("journey ")
        || trimmed.starts_with("timeline ")
        || trimmed.starts_with("pie ")
        || trimmed.starts_with("quadrantChart ")
        || trimmed.starts_with("xychart-beta ")
        || trimmed.starts_with("C4")
}

fn generic_kind(diagram_type: Option<&str>) -> SymbolKind {
    match diagram_type {
        Some("sequence") => SymbolKind::EVENT,
        Some("state") => SymbolKind::CLASS,
        Some("mindmap") => SymbolKind::NAMESPACE,
        Some("class") => SymbolKind::CLASS,
        Some("er") => SymbolKind::STRUCT,
        Some("block") => SymbolKind::OBJECT,
        Some("flowchart-v2") | Some("flowchart-elk") => SymbolKind::MODULE,
        _ => SymbolKind::VARIABLE,
    }
}

fn fence_kind(fence: &FenceSnapshot) -> SymbolKind {
    generic_kind(fence.diagram_type.as_deref())
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
    if !fence.completion.directive_prefixes().next().is_none() {
        let prefixes = fence
            .completion
            .directive_prefixes()
            .cloned()
            .collect::<Vec<_>>();
        if !prefixes.is_empty() {
            parts.push(format!("directives {}", prefixes.join(", ")));
        }
    }

    if parts.is_empty() {
        None
    } else {
        Some(parts.join(" · "))
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

pub fn outline_hover(snapshot: &DocumentSnapshot, position: Position) -> Option<Hover> {
    hover(snapshot, position)
}

pub fn outline_document_symbols(snapshot: &DocumentSnapshot) -> DocumentSymbolResponse {
    document_symbols(snapshot)
}

#[cfg(test)]
mod tests {
    use super::{outline_document_symbols, outline_hover};
    use crate::document_store::DocumentStore;
    use tower_lsp::lsp_types::{Position, Url};

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
        assert!(text.contains("diagram type"));
    }
}
