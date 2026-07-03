use std::collections::BTreeSet;
use std::sync::OnceLock;

use super::{ByteSpan, EditorSymbolKind, FenceLineItem};

pub(super) fn is_candidate_node_id(token: &str) -> bool {
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
            | "init"
            | "initialize"
            | "wrap"
    )
}

pub(super) fn collect_node_ids(diagram_type: Option<&str>, text: &str, ids: &mut BTreeSet<String>) {
    if matches!(diagram_type, Some("mindmap")) {
        collect_mindmap_node_ids(text, ids);
        return;
    }

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

fn collect_mindmap_node_ids(text: &str, ids: &mut BTreeSet<String>) {
    let trimmed = text.trim_start();
    if trimmed.is_empty()
        || trimmed.starts_with('%')
        || trimmed.starts_with(':')
        || is_header_line(trimmed)
    {
        return;
    }

    if let Some((token, _)) = first_symbol_token(trimmed, 0) {
        if token.starts_with(':') {
            return;
        }
        if is_candidate_node_id(&token) {
            ids.insert(token);
        }
    }
}

pub(super) fn classify_line_item(
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
        ("section", EditorSymbolKind::Namespace, "gantt section"),
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
    let trimmed = trimmed.trim_end();
    if trimmed.is_empty() {
        return false;
    }

    diagram_header_facts()
        .iter()
        .any(|fact| header_line_matches_fact(trimmed, fact.label))
}

pub(super) fn diagram_header_facts() -> &'static [merman_core::DiagramHeaderFact] {
    static FACTS: OnceLock<Vec<merman_core::DiagramHeaderFact>> = OnceLock::new();
    FACTS
        .get_or_init(|| {
            merman_core::diagram_header_facts_for_profile(
                merman_core::selected_baseline_registry_profile(),
            )
            .iter()
            .copied()
            .collect()
        })
        .as_slice()
}

fn header_line_matches_fact(trimmed: &str, label: &str) -> bool {
    if trimmed == label {
        return true;
    }

    let starter = label.split_whitespace().next().unwrap_or(label);
    if trimmed == starter {
        return true;
    }

    trimmed
        .strip_prefix(starter)
        .is_some_and(|rest| rest.chars().next().is_some_and(|ch| ch.is_whitespace()))
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
