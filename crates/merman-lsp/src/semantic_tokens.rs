use crate::snapshot::{DocumentSnapshot, FenceSnapshot};
use merman_analysis::{
    ByteSpan, EditorSymbolKind, FenceSemanticItem, FenceSemanticRole, SourceMap,
};
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use tower_lsp::lsp_types::{
    Range, SemanticToken, SemanticTokenModifier, SemanticTokenType, SemanticTokens,
    SemanticTokensDelta, SemanticTokensEdit, SemanticTokensFullDeltaResult,
    SemanticTokensFullOptions, SemanticTokensLegend, SemanticTokensOptions,
};

const TOKEN_TYPE_NAMESPACE: u32 = 0;
const TOKEN_TYPE_CLASS: u32 = 1;
const TOKEN_TYPE_STRUCT: u32 = 2;
const TOKEN_TYPE_VARIABLE: u32 = 3;
const TOKEN_TYPE_PROPERTY: u32 = 4;
const TOKEN_TYPE_EVENT: u32 = 5;
const TOKEN_TYPE_FUNCTION: u32 = 6;
const TOKEN_TYPE_STRING: u32 = 7;

pub const TOKEN_MODIFIER_ENTITY: u32 = 0;
pub const TOKEN_MODIFIER_OUTLINE: u32 = 1;
pub const TOKEN_MODIFIER_PAYLOAD: u32 = 2;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct AbsoluteToken {
    line: u32,
    start: u32,
    length: u32,
    token_type: u32,
    token_modifiers_bitset: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct TokenPiece {
    line: u32,
    start: u32,
    length: u32,
}

pub fn semantic_tokens_options() -> SemanticTokensOptions {
    SemanticTokensOptions {
        work_done_progress_options: Default::default(),
        legend: semantic_tokens_legend(),
        range: Some(true),
        full: Some(SemanticTokensFullOptions::Delta { delta: Some(true) }),
    }
}

pub fn semantic_tokens_legend() -> SemanticTokensLegend {
    SemanticTokensLegend {
        token_types: vec![
            SemanticTokenType::NAMESPACE,
            SemanticTokenType::CLASS,
            SemanticTokenType::STRUCT,
            SemanticTokenType::VARIABLE,
            SemanticTokenType::PROPERTY,
            SemanticTokenType::EVENT,
            SemanticTokenType::FUNCTION,
            SemanticTokenType::STRING,
        ],
        token_modifiers: vec![
            SemanticTokenModifier::new("mermanEntity"),
            SemanticTokenModifier::new("mermanOutline"),
            SemanticTokenModifier::new("mermanPayload"),
        ],
    }
}

pub fn semantic_tokens_for_snapshot(snapshot: &DocumentSnapshot) -> SemanticTokens {
    semantic_tokens_from_absolute_tokens_with_result_id(
        absolute_tokens_for_snapshot(snapshot),
        None,
    )
}

pub fn semantic_tokens_for_snapshot_with_result_id(
    snapshot: &DocumentSnapshot,
    result_id: String,
) -> SemanticTokens {
    semantic_tokens_from_absolute_tokens_with_result_id(
        absolute_tokens_for_snapshot(snapshot),
        Some(result_id),
    )
}

pub fn semantic_tokens_for_snapshot_range(
    snapshot: &DocumentSnapshot,
    range: Range,
) -> SemanticTokens {
    let absolute_tokens = absolute_tokens_for_snapshot(snapshot)
        .into_iter()
        .filter(|token| token_overlaps_range(token, &range))
        .collect();

    semantic_tokens_from_absolute_tokens_with_result_id(absolute_tokens, None)
}

pub fn semantic_tokens_delta_result(
    previous_tokens: &[SemanticToken],
    current_tokens: &[SemanticToken],
    result_id: String,
) -> SemanticTokensFullDeltaResult {
    let Some(edit) = semantic_tokens_delta_edit(previous_tokens, current_tokens) else {
        return SemanticTokensFullDeltaResult::TokensDelta(SemanticTokensDelta {
            result_id: Some(result_id),
            edits: Vec::new(),
        });
    };

    SemanticTokensFullDeltaResult::TokensDelta(SemanticTokensDelta {
        result_id: Some(result_id),
        edits: vec![edit],
    })
}

fn semantic_tokens_from_absolute_tokens_with_result_id(
    absolute_tokens: Vec<AbsoluteToken>,
    result_id: Option<String>,
) -> SemanticTokens {
    SemanticTokens {
        result_id,
        data: encode_relative_tokens(absolute_tokens),
    }
}

pub fn semantic_tokens_result_id(snapshot: &DocumentSnapshot, tokens: &[SemanticToken]) -> String {
    let mut hasher = DefaultHasher::new();
    snapshot.version.hash(&mut hasher);
    for token in tokens {
        token.delta_line.hash(&mut hasher);
        token.delta_start.hash(&mut hasher);
        token.length.hash(&mut hasher);
        token.token_type.hash(&mut hasher);
        token.token_modifiers_bitset.hash(&mut hasher);
    }
    format!("{}:{:016x}", snapshot.version, hasher.finish())
}

fn absolute_tokens_for_snapshot(snapshot: &DocumentSnapshot) -> Vec<AbsoluteToken> {
    let mut absolute_tokens = Vec::new();

    for fence in &snapshot.fences {
        for item in fence.text_index.semantic_items() {
            absolute_tokens.extend(tokens_for_item(&snapshot.source_map, fence, item));
        }
    }

    absolute_tokens.sort_by(|left, right| {
        (
            left.line,
            left.start,
            left.length,
            left.token_type,
            left.token_modifiers_bitset,
        )
            .cmp(&(
                right.line,
                right.start,
                right.length,
                right.token_type,
                right.token_modifiers_bitset,
            ))
    });
    absolute_tokens.dedup();
    absolute_tokens
}

fn tokens_for_item(
    source_map: &SourceMap,
    fence: &FenceSnapshot,
    item: &FenceSemanticItem,
) -> Vec<AbsoluteToken> {
    let span = ByteSpan {
        start: fence.body_start + item.selection.start,
        end: fence.body_start + item.selection.end,
    };
    if span.start >= span.end {
        return Vec::new();
    }

    token_pieces_for_span(source_map, span)
        .into_iter()
        .map(|piece| AbsoluteToken {
            line: piece.line,
            start: piece.start,
            length: piece.length,
            token_type: token_type_for_kind(item.kind),
            token_modifiers_bitset: token_modifier_bitset_for_role(item.role),
        })
        .collect()
}

fn token_pieces_for_span(source_map: &SourceMap, span: ByteSpan) -> Vec<TokenPiece> {
    let Ok(start) = source_map.utf16_position(span.start) else {
        return Vec::new();
    };
    let Ok(end) = source_map.utf16_position(span.end) else {
        return Vec::new();
    };

    let mut pieces = Vec::new();
    for line in start.line..=end.line {
        let Some((line_start, line_end)) = source_map.line_bounds(line) else {
            continue;
        };
        let segment_start = span.start.max(line_start);
        let segment_end = span.end.min(line_end);
        if segment_start >= segment_end {
            continue;
        }

        let Ok(segment_start_pos) = source_map.utf16_position(segment_start) else {
            continue;
        };
        let Ok(segment_end_pos) = source_map.utf16_position(segment_end) else {
            continue;
        };
        if segment_start_pos.line != segment_end_pos.line
            || segment_end_pos.character <= segment_start_pos.character
        {
            continue;
        }

        pieces.push(TokenPiece {
            line: segment_start_pos.line as u32,
            start: segment_start_pos.character as u32,
            length: (segment_end_pos.character - segment_start_pos.character) as u32,
        });
    }

    pieces
}

fn token_overlaps_range(token: &AbsoluteToken, range: &Range) -> bool {
    let range_start_line = range.start.line;
    let range_start_character = range.start.character;
    let range_end_line = range.end.line;
    let range_end_character = range.end.character;
    let token_line = token.line;
    let token_start = token.start;
    let token_end = token.start + token.length;

    if token_line < range_start_line || token_line > range_end_line {
        return false;
    }

    if range_start_line == range_end_line {
        return token_line == range_start_line
            && token_end > range_start_character
            && token_start < range_end_character;
    }

    if token_line == range_start_line {
        return token_end > range_start_character;
    }

    if token_line == range_end_line {
        return token_start < range_end_character;
    }

    true
}

fn encode_relative_tokens(absolute_tokens: Vec<AbsoluteToken>) -> Vec<SemanticToken> {
    let mut previous_line = 0u32;
    let mut previous_start = 0u32;

    absolute_tokens
        .into_iter()
        .map(|token| {
            let delta_line = token.line.saturating_sub(previous_line);
            let delta_start = if delta_line == 0 {
                token.start.saturating_sub(previous_start)
            } else {
                token.start
            };

            previous_line = token.line;
            previous_start = token.start;

            SemanticToken {
                delta_line,
                delta_start,
                length: token.length,
                token_type: token.token_type,
                token_modifiers_bitset: token.token_modifiers_bitset,
            }
        })
        .collect()
}

fn semantic_tokens_delta_edit(
    previous_tokens: &[SemanticToken],
    current_tokens: &[SemanticToken],
) -> Option<SemanticTokensEdit> {
    let prefix_tokens = previous_tokens
        .iter()
        .zip(current_tokens.iter())
        .take_while(|(previous, current)| previous == current)
        .count();

    if prefix_tokens == previous_tokens.len() && prefix_tokens == current_tokens.len() {
        return None;
    }

    let previous_remainder = &previous_tokens[prefix_tokens..];
    let current_remainder = &current_tokens[prefix_tokens..];
    let suffix_tokens = previous_remainder
        .iter()
        .rev()
        .zip(current_remainder.iter().rev())
        .take_while(|(previous, current)| previous == current)
        .count();

    let previous_end = previous_tokens.len().saturating_sub(suffix_tokens);
    let current_end = current_tokens.len().saturating_sub(suffix_tokens);

    Some(SemanticTokensEdit {
        start: (prefix_tokens * 5) as u32,
        delete_count: ((previous_end - prefix_tokens) * 5) as u32,
        data: if prefix_tokens < current_end {
            Some(current_tokens[prefix_tokens..current_end].to_vec())
        } else {
            None
        },
    })
}

fn token_type_for_kind(kind: EditorSymbolKind) -> u32 {
    match kind {
        EditorSymbolKind::Class => TOKEN_TYPE_CLASS,
        EditorSymbolKind::Event => TOKEN_TYPE_EVENT,
        EditorSymbolKind::Function => TOKEN_TYPE_FUNCTION,
        EditorSymbolKind::Module | EditorSymbolKind::Namespace | EditorSymbolKind::Package => {
            TOKEN_TYPE_NAMESPACE
        }
        EditorSymbolKind::Object | EditorSymbolKind::Variable => TOKEN_TYPE_VARIABLE,
        EditorSymbolKind::Property => TOKEN_TYPE_PROPERTY,
        EditorSymbolKind::String => TOKEN_TYPE_STRING,
        EditorSymbolKind::Struct => TOKEN_TYPE_STRUCT,
    }
}

fn token_modifier_bitset_for_role(role: FenceSemanticRole) -> u32 {
    1 << match role {
        FenceSemanticRole::Entity => TOKEN_MODIFIER_ENTITY,
        FenceSemanticRole::Outline => TOKEN_MODIFIER_OUTLINE,
        FenceSemanticRole::Payload => TOKEN_MODIFIER_PAYLOAD,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::document_store::DocumentStore;
    use tower_lsp::lsp_types::Url;

    #[test]
    fn semantic_tokens_project_entity_outline_and_payload_roles() {
        let mut store = DocumentStore::new();
        let uri = Url::parse("file:///tmp/example.mmd").unwrap();
        let snapshot = store.upsert(
            uri,
            1,
            concat!(
                "gantt\n",
                "title Roadmap\n",
                "section Demo\n",
                "Task 1: id1,2014-01-01,1d\n",
                "Task 2: id2,after id1,2d\n",
            )
            .to_string(),
        );

        let tokens = semantic_tokens_for_snapshot(&snapshot);

        assert!(tokens.data.iter().any(|token| {
            token.token_type == TOKEN_TYPE_VARIABLE
                && token.token_modifiers_bitset
                    == token_modifier_bitset_for_role(FenceSemanticRole::Entity)
        }));
        assert!(tokens.data.iter().any(|token| {
            token.token_type == TOKEN_TYPE_NAMESPACE
                && token.token_modifiers_bitset
                    == token_modifier_bitset_for_role(FenceSemanticRole::Outline)
        }));
        assert!(tokens.data.iter().any(|token| {
            token.token_type == TOKEN_TYPE_STRING
                && token.token_modifiers_bitset
                    == token_modifier_bitset_for_role(FenceSemanticRole::Payload)
        }));
    }

    #[test]
    fn semantic_tokens_use_absolute_markdown_ranges_and_utf16_lengths() {
        let mut store = DocumentStore::new();
        let uri = Url::parse("file:///tmp/example.md").unwrap();
        let snapshot = store.upsert(
            uri,
            1,
            "before\n```mermaid\nsequenceDiagram\ntitle: Diagram 🤓\n```\nafter\n".to_string(),
        );
        let tokens = semantic_tokens_for_snapshot(&snapshot);
        let decoded = decode_tokens(&tokens.data);

        let payload_start = snapshot.text.find("Diagram 🤓").unwrap();
        let payload_end = payload_start + "Diagram 🤓".len();
        let payload_span = snapshot
            .source_map
            .span(payload_start, payload_end)
            .expect("payload span should map");
        let payload_line = payload_span.lsp_range.start.line as u32;
        let payload_start_character = payload_span.lsp_range.start.character as u32;
        let payload_length =
            (payload_span.lsp_range.end.character - payload_span.lsp_range.start.character) as u32;

        assert!(decoded.iter().any(|token| {
            token.line == payload_line
                && token.start == payload_start_character
                && token.length == payload_length
                && token.token_type == TOKEN_TYPE_STRING
                && token.token_modifiers_bitset
                    == token_modifier_bitset_for_role(FenceSemanticRole::Payload)
        }));
    }

    #[test]
    fn semantic_tokens_split_multiline_payload_spans() {
        let mut store = DocumentStore::new();
        let uri = Url::parse("file:///tmp/example.mmd").unwrap();
        let snapshot = store.upsert(
            uri,
            1,
            "gantt\naccDescr {\nline one\nline two\n}\n".to_string(),
        );
        let tokens = semantic_tokens_for_snapshot(&snapshot);
        let decoded = decode_tokens(&tokens.data);
        let payload_bitset = token_modifier_bitset_for_role(FenceSemanticRole::Payload);
        let payload_tokens = decoded
            .iter()
            .filter(|token| {
                token.token_type == TOKEN_TYPE_STRING
                    && token.token_modifiers_bitset == payload_bitset
            })
            .collect::<Vec<_>>();

        assert!(
            payload_tokens.len() >= 2,
            "expected multiline payload to produce per-line semantic tokens: {decoded:?}"
        );
    }

    #[test]
    fn semantic_tokens_delta_result_prefers_edits_over_full_tokens() {
        let previous = vec![
            SemanticToken {
                delta_line: 0,
                delta_start: 0,
                length: 3,
                token_type: TOKEN_TYPE_NAMESPACE,
                token_modifiers_bitset: 0,
            },
            SemanticToken {
                delta_line: 0,
                delta_start: 4,
                length: 2,
                token_type: TOKEN_TYPE_STRING,
                token_modifiers_bitset: 1 << TOKEN_MODIFIER_PAYLOAD,
            },
        ];
        let current = vec![
            SemanticToken {
                delta_line: 0,
                delta_start: 0,
                length: 3,
                token_type: TOKEN_TYPE_NAMESPACE,
                token_modifiers_bitset: 0,
            },
            SemanticToken {
                delta_line: 0,
                delta_start: 5,
                length: 2,
                token_type: TOKEN_TYPE_STRING,
                token_modifiers_bitset: 1 << TOKEN_MODIFIER_PAYLOAD,
            },
        ];

        let result = semantic_tokens_delta_result(&previous, &current, "next".to_string());
        let SemanticTokensFullDeltaResult::TokensDelta(delta) = result else {
            panic!("expected delta tokens");
        };

        assert_eq!(delta.result_id.as_deref(), Some("next"));
        assert_eq!(delta.edits.len(), 1);
        assert!(!delta.edits[0].data.as_ref().unwrap().is_empty());
    }

    fn decode_tokens(tokens: &[SemanticToken]) -> Vec<AbsoluteToken> {
        let mut line = 0u32;
        let mut start = 0u32;
        let mut decoded = Vec::new();

        for token in tokens {
            line += token.delta_line;
            if token.delta_line == 0 {
                start += token.delta_start;
            } else {
                start = token.delta_start;
            }

            decoded.push(AbsoluteToken {
                line,
                start,
                length: token.length,
                token_type: token.token_type,
                token_modifiers_bitset: token.token_modifiers_bitset,
            });
        }

        decoded
    }
}
