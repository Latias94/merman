use std::collections::{BTreeMap, BTreeSet};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ByteSpan {
    pub start: usize,
    pub end: usize,
}

impl ByteSpan {
    pub fn contains(self, offset: usize) -> bool {
        offset >= self.start && offset <= self.end
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EditorSymbolKind {
    Class,
    Event,
    Function,
    Module,
    Namespace,
    Object,
    Package,
    Property,
    String,
    Struct,
    Variable,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FenceLineItem {
    pub name: String,
    pub detail: Option<String>,
    pub kind: EditorSymbolKind,
    pub span: ByteSpan,
    pub selection: ByteSpan,
}

#[derive(Debug, Clone, Default)]
pub struct FenceTextIndex {
    node_ids: BTreeSet<String>,
    directive_prefixes: BTreeSet<String>,
    references: BTreeMap<String, Vec<ByteSpan>>,
    outline_items: Vec<FenceLineItem>,
}

impl FenceTextIndex {
    pub fn from_text(text: &str, diagram_type: Option<&str>) -> Self {
        let mut index = Self::default();
        let mut relative_start = 0usize;

        for line in text.split_inclusive('\n') {
            let line_end = relative_start + line.len();
            let line_no_newline = line.strip_suffix('\n').unwrap_or(line);
            let trimmed = line_no_newline.trim_start();
            let leading = line_no_newline.len().saturating_sub(trimmed.len());
            let abs_start = relative_start + leading;
            let abs_end = line_end;

            index.record_line(diagram_type, line_no_newline, trimmed, abs_start, abs_end);
            relative_start = line_end;
        }

        if !text.ends_with('\n') && relative_start < text.len() {
            let line_no_newline = &text[relative_start..];
            let trimmed = line_no_newline.trim_start();
            let leading = line_no_newline.len().saturating_sub(trimmed.len());
            index.record_line(
                diagram_type,
                line_no_newline,
                trimmed,
                relative_start + leading,
                text.len(),
            );
        }

        index.outline_items.sort_by(|left, right| {
            (
                left.span.start,
                left.span.end,
                left.name.as_str(),
                left.selection.start,
                left.selection.end,
            )
                .cmp(&(
                    right.span.start,
                    right.span.end,
                    right.name.as_str(),
                    right.selection.start,
                    right.selection.end,
                ))
        });
        index.outline_items.dedup_by(|left, right| {
            left.span.start == right.span.start
                && left.span.end == right.span.end
                && left.name == right.name
        });

        index
    }

    pub fn node_ids(&self) -> impl Iterator<Item = &String> {
        self.node_ids.iter()
    }

    pub fn directive_prefixes(&self) -> impl Iterator<Item = &String> {
        self.directive_prefixes.iter()
    }

    pub fn has_directive_prefix(&self, prefix: &str) -> bool {
        self.directive_prefixes.contains(prefix)
    }

    pub fn first_reference_span(&self, name: &str) -> Option<ByteSpan> {
        self.references
            .get(name)
            .and_then(|spans| spans.first().copied())
    }

    pub fn reference_spans(&self, name: &str) -> &[ByteSpan] {
        self.references.get(name).map(Vec::as_slice).unwrap_or(&[])
    }

    pub fn symbol_at_offset(&self, offset: usize) -> Option<(String, ByteSpan)> {
        self.references.iter().find_map(|(name, spans)| {
            spans
                .iter()
                .copied()
                .find(|span| span.contains(offset))
                .map(|span| (name.clone(), span))
        })
    }

    pub fn outline_items(&self) -> &[FenceLineItem] {
        &self.outline_items
    }

    fn record_line(
        &mut self,
        diagram_type: Option<&str>,
        line_no_newline: &str,
        trimmed: &str,
        abs_start: usize,
        abs_end: usize,
    ) {
        collect_node_ids(line_no_newline, &mut self.node_ids);

        if let Some(prefix) = directive_prefix(line_no_newline) {
            self.directive_prefixes.insert(prefix.to_string());
        }

        if let Some(item) = classify_line_item(diagram_type, trimmed, abs_start, abs_end) {
            self.references
                .entry(item.name.clone())
                .or_default()
                .push(item.selection);
            self.outline_items.push(item);
        }
    }
}

fn directive_prefix(line: &str) -> Option<&'static str> {
    let trimmed = line.trim_start();

    if let Some(rest) = trimmed.strip_prefix("%%{") {
        let name = rest
            .split(|ch: char| ch.is_whitespace() || matches!(ch, ':' | '}'))
            .next()
            .filter(|name| !name.is_empty())?;

        return matches!(name, "init" | "initialize" | "wrap").then_some(match name {
            "init" => "init",
            "initialize" => "initialize",
            "wrap" => "wrap",
            _ => unreachable!(),
        });
    }

    if trimmed.starts_with(":::") {
        return Some(":::");
    }

    for prefix in [
        "classDef",
        "class",
        "style",
        "linkStyle",
        "click",
        "accTitle",
        "accDescr",
        "accDescription",
        "title",
    ] {
        if has_word_boundary(trimmed, prefix) {
            return Some(prefix);
        }
    }

    None
}

fn has_word_boundary(text: &str, prefix: &str) -> bool {
    text.strip_prefix(prefix).is_some_and(|rest| {
        rest.is_empty()
            || rest
                .chars()
                .next()
                .is_some_and(|ch| ch.is_whitespace() || matches!(ch, ':' | '{'))
    })
}

fn is_candidate_node_id(token: &str) -> bool {
    if token.is_empty() || token.starts_with('%') {
        return false;
    }

    !matches!(
        token,
        "flowchart"
            | "graph"
            | "sequenceDiagram"
            | "stateDiagram"
            | "stateDiagram-v2"
            | "mindmap"
            | "TD"
            | "TB"
            | "BT"
            | "LR"
            | "RL"
            | "classDef"
            | "class"
            | "style"
            | "linkStyle"
    )
}

fn collect_node_ids(text: &str, ids: &mut BTreeSet<String>) {
    for token in text.split(|ch: char| {
        ch.is_whitespace()
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
    }) {
        if is_candidate_node_id(token) {
            ids.insert(token.to_string());
        }
    }
}

fn classify_line_item(
    diagram_type: Option<&str>,
    trimmed: &str,
    abs_start: usize,
    abs_end: usize,
) -> Option<FenceLineItem> {
    if trimmed.is_empty()
        || is_header_line(trimmed)
        || trimmed.starts_with("%%")
        || trimmed.starts_with(":::")
    {
        return None;
    }

    if let Some(rest) = trimmed.strip_prefix("subgraph ") {
        let (name, selection) = token_after_prefix(trimmed, "subgraph", abs_start)?;
        return Some(FenceLineItem {
            name: if rest.trim().is_empty() {
                "subgraph".to_string()
            } else {
                name
            },
            detail: Some("subgraph".to_string()),
            kind: EditorSymbolKind::Namespace,
            span: ByteSpan {
                start: abs_start,
                end: abs_end,
            },
            selection,
        });
    }

    if let Some((keyword, kind, detail)) = [
        (
            "participant",
            EditorSymbolKind::Variable,
            "sequence participant",
        ),
        ("actor", EditorSymbolKind::Variable, "sequence actor"),
        ("box", EditorSymbolKind::Package, "sequence box"),
        ("note", EditorSymbolKind::Event, "note"),
        ("state", EditorSymbolKind::Class, "state"),
        ("classDef", EditorSymbolKind::Property, "class definition"),
        ("class", EditorSymbolKind::Class, "class assignment"),
        ("style", EditorSymbolKind::Property, "style"),
        ("click", EditorSymbolKind::Function, "interaction"),
        ("linkStyle", EditorSymbolKind::Property, "link style"),
        ("accTitle", EditorSymbolKind::String, "accessibility title"),
        (
            "accDescr",
            EditorSymbolKind::String,
            "accessibility description",
        ),
        ("title", EditorSymbolKind::String, "title"),
    ]
    .into_iter()
    .find_map(|(keyword, kind, detail)| {
        trimmed
            .strip_prefix(keyword)
            .map(|_| (keyword, kind, detail))
    }) {
        let (name, selection) = token_after_prefix(trimmed, keyword, abs_start)?;
        return Some(FenceLineItem {
            name,
            detail: Some(detail.to_string()),
            kind,
            span: ByteSpan {
                start: abs_start,
                end: abs_end,
            },
            selection,
        });
    }

    if matches!(diagram_type, Some("mindmap")) {
        let (name, selection) = first_symbol_token(trimmed, abs_start)?;
        return Some(FenceLineItem {
            name,
            detail: Some("mindmap node".to_string()),
            kind: EditorSymbolKind::String,
            span: ByteSpan {
                start: abs_start,
                end: abs_end,
            },
            selection,
        });
    }

    let (name, selection) = first_symbol_token(trimmed, abs_start)?;
    Some(FenceLineItem {
        name,
        detail: Some("diagram element".to_string()),
        kind: generic_kind(diagram_type),
        span: ByteSpan {
            start: abs_start,
            end: abs_end,
        },
        selection,
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

#[cfg(test)]
mod tests {
    use super::{FenceTextIndex, is_candidate_node_id};

    #[test]
    fn text_index_collects_node_ids() {
        let index = FenceTextIndex::from_text("flowchart TD\nA-->B\nB-->C\n", Some("flowchart-v2"));
        let ids = index.node_ids().cloned().collect::<Vec<_>>();

        assert_eq!(ids, vec!["A", "B", "C"]);
    }

    #[test]
    fn node_id_filter_skips_keywords_and_empty_tokens() {
        assert!(!is_candidate_node_id("flowchart"));
        assert!(!is_candidate_node_id("%comment"));
        assert!(is_candidate_node_id("node_1"));
    }

    #[test]
    fn text_index_tracks_directive_prefixes() {
        let index = FenceTextIndex::from_text(
            "%%{init: {\"theme\": \"dark\"}}%%\nclassDef foo fill:#f00\n:::className\n",
            None,
        );

        assert!(index.has_directive_prefix("init"));
        assert!(index.has_directive_prefix("classDef"));
        assert!(index.has_directive_prefix(":::"));
    }
}
