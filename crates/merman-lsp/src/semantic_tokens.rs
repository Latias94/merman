use crate::client_profile::{ClientProtocolProfile, SemanticTokenProjection};
use crate::snapshot::DocumentSnapshot;
use merman_editor_core::{
    SemanticToken as CoreSemanticToken,
    semantic_tokens_for_snapshot as core_semantic_tokens_for_snapshot,
    semantic_tokens_for_snapshot_range as core_semantic_tokens_for_snapshot_range,
};
#[cfg(test)]
use merman_editor_core::{
    SemanticTokenKind, SemanticTokenModifier as CoreSemanticTokenModifier,
    semantic_token_legend as core_semantic_token_legend, token_modifier_index, token_type_index,
};
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use tower_lsp::lsp_types::{
    Range, SemanticToken, SemanticTokens, SemanticTokensDelta, SemanticTokensEdit,
    SemanticTokensFullDeltaResult, SemanticTokensOptions,
};
#[cfg(test)]
use tower_lsp::lsp_types::{SemanticTokenModifier, SemanticTokenType, SemanticTokensLegend};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct AbsoluteToken {
    line: u32,
    start: u32,
    length: u32,
    token_type: u32,
    token_modifiers_bitset: u32,
}

pub(crate) fn semantic_tokens_options_with_profile(
    profile: &ClientProtocolProfile,
) -> Option<SemanticTokensOptions> {
    profile
        .semantic_tokens
        .as_ref()
        .map(SemanticTokenProjection::options)
}

#[cfg(test)]
pub fn semantic_tokens_legend() -> SemanticTokensLegend {
    ClientProtocolProfile::permissive()
        .semantic_tokens
        .expect("permissive profile enables semantic tokens")
        .legend()
}

#[cfg(test)]
pub fn semantic_tokens_for_snapshot(snapshot: &DocumentSnapshot) -> SemanticTokens {
    semantic_tokens_for_snapshot_with_profile(snapshot, &ClientProtocolProfile::permissive())
        .expect("permissive profile enables semantic tokens")
}

pub(crate) fn semantic_tokens_for_snapshot_with_profile(
    snapshot: &DocumentSnapshot,
    profile: &ClientProtocolProfile,
) -> Option<SemanticTokens> {
    let projection = profile.semantic_tokens.as_ref()?;
    Some(semantic_tokens_from_absolute_tokens_with_result_id(
        absolute_tokens_for_snapshot(snapshot, projection),
        None,
    ))
}

#[cfg(test)]
pub fn semantic_tokens_for_snapshot_range(
    snapshot: &DocumentSnapshot,
    range: Range,
) -> SemanticTokens {
    semantic_tokens_for_snapshot_range_with_profile(
        snapshot,
        range,
        &ClientProtocolProfile::permissive(),
    )
    .expect("permissive profile enables semantic tokens")
}

pub(crate) fn semantic_tokens_for_snapshot_range_with_profile(
    snapshot: &DocumentSnapshot,
    range: Range,
    profile: &ClientProtocolProfile,
) -> Option<SemanticTokens> {
    let projection = profile.semantic_tokens.as_ref()?;
    let absolute_tokens = absolute_tokens_for_snapshot_range(snapshot, &range, projection)
        .into_iter()
        .filter(|token| token_overlaps_range(token, &range))
        .collect();

    Some(semantic_tokens_from_absolute_tokens_with_result_id(
        absolute_tokens,
        None,
    ))
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

fn absolute_tokens_for_snapshot(
    snapshot: &DocumentSnapshot,
    projection: &SemanticTokenProjection,
) -> Vec<AbsoluteToken> {
    core_semantic_tokens_for_snapshot(snapshot.as_editor())
        .into_iter()
        .filter_map(|token| project_absolute_token(token, projection))
        .collect()
}

fn absolute_tokens_for_snapshot_range(
    snapshot: &DocumentSnapshot,
    range: &Range,
    projection: &SemanticTokenProjection,
) -> Vec<AbsoluteToken> {
    core_semantic_tokens_for_snapshot_range(snapshot.as_editor(), range.start.line, range.end.line)
        .into_iter()
        .filter_map(|token| project_absolute_token(token, projection))
        .collect()
}

fn project_absolute_token(
    token: CoreSemanticToken,
    projection: &SemanticTokenProjection,
) -> Option<AbsoluteToken> {
    Some(AbsoluteToken {
        line: token.line,
        start: token.start,
        length: token.length,
        token_type: projection.token_type(token.kind)?,
        token_modifiers_bitset: projection.token_modifier_bitset(token.modifier),
    })
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

#[cfg(test)]
fn token_type(kind: SemanticTokenKind) -> u32 {
    token_type_index(kind)
}

#[cfg(test)]
fn token_modifier_bitset(modifier: CoreSemanticTokenModifier) -> u32 {
    1 << token_modifier_index(modifier)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::document_store::DocumentStore;
    use tower_lsp::lsp_types::{Position, Url};

    #[test]
    fn semantic_tokens_legend_and_encoding_follow_editor_core_order() {
        let core_legend = core_semantic_token_legend();
        let lsp_legend = semantic_tokens_legend();

        assert_eq!(
            lsp_legend.token_types,
            vec![
                SemanticTokenType::NAMESPACE,
                SemanticTokenType::CLASS,
                SemanticTokenType::STRUCT,
                SemanticTokenType::VARIABLE,
                SemanticTokenType::PROPERTY,
                SemanticTokenType::EVENT,
                SemanticTokenType::FUNCTION,
                SemanticTokenType::STRING,
            ]
        );
        assert_eq!(
            lsp_legend.token_modifiers,
            vec![
                SemanticTokenModifier::new("mermanEntity"),
                SemanticTokenModifier::new("mermanOutline"),
                SemanticTokenModifier::new("mermanPayload"),
            ]
        );
        for (index, kind) in core_legend.token_types.iter().copied().enumerate() {
            assert_eq!(token_type(kind), index as u32);
        }
        for (index, modifier) in core_legend.token_modifiers.iter().copied().enumerate() {
            assert_eq!(token_modifier_index(modifier), index as u32);
        }
    }

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
            token.token_type == token_type(SemanticTokenKind::Variable)
                && token.token_modifiers_bitset
                    == token_modifier_bitset(CoreSemanticTokenModifier::Entity)
        }));
        assert!(tokens.data.iter().any(|token| {
            token.token_type == token_type(SemanticTokenKind::Namespace)
                && token.token_modifiers_bitset
                    == token_modifier_bitset(CoreSemanticTokenModifier::Outline)
        }));
        assert!(tokens.data.iter().any(|token| {
            token.token_type == token_type(SemanticTokenKind::String)
                && token.token_modifiers_bitset
                    == token_modifier_bitset(CoreSemanticTokenModifier::Payload)
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
                && token.token_type == token_type(SemanticTokenKind::String)
                && token.token_modifiers_bitset
                    == token_modifier_bitset(CoreSemanticTokenModifier::Payload)
        }));
    }

    #[test]
    fn semantic_tokens_range_filters_to_requested_markdown_fence() {
        let mut store = DocumentStore::new();
        let uri = Url::parse("file:///tmp/example.md").unwrap();
        let snapshot = store.upsert(
            uri,
            1,
            concat!(
                "intro\n",
                "```mermaid\n",
                "sequenceDiagram\n",
                "title: First\n",
                "```\n",
                "middle\n",
                "```mermaid\n",
                "sequenceDiagram\n",
                "title: Second\n",
                "```\n",
                "outro\n",
            )
            .to_string(),
        );

        let tokens = semantic_tokens_for_snapshot_range(
            &snapshot,
            Range::new(Position::new(6, 0), Position::new(10, 0)),
        );
        let decoded = decode_tokens(&tokens.data);

        let first_start = snapshot.text.find("First").unwrap();
        let first_span = snapshot
            .source_map
            .span(first_start, first_start + "First".len())
            .expect("first title span should map");
        let second_start = snapshot.text.find("Second").unwrap();
        let second_span = snapshot
            .source_map
            .span(second_start, second_start + "Second".len())
            .expect("second title span should map");
        let second_line = second_span.lsp_range.start.line as u32;
        let second_start_character = second_span.lsp_range.start.character as u32;
        let second_length =
            (second_span.lsp_range.end.character - second_span.lsp_range.start.character) as u32;
        let payload_bitset = token_modifier_bitset(CoreSemanticTokenModifier::Payload);

        assert!(decoded.iter().any(|token| {
            token.line == second_line
                && token.start == second_start_character
                && token.length == second_length
                && token.token_type == token_type(SemanticTokenKind::String)
                && token.token_modifiers_bitset == payload_bitset
        }));
        assert!(
            !decoded
                .iter()
                .any(|token| token.line == first_span.lsp_range.start.line as u32),
            "range request should not return tokens from the first fence: {decoded:?}"
        );
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
        let payload_bitset = token_modifier_bitset(CoreSemanticTokenModifier::Payload);
        let payload_tokens = decoded
            .iter()
            .filter(|token| {
                token.token_type == token_type(SemanticTokenKind::String)
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
                token_type: token_type(SemanticTokenKind::Namespace),
                token_modifiers_bitset: 0,
            },
            SemanticToken {
                delta_line: 0,
                delta_start: 4,
                length: 2,
                token_type: token_type(SemanticTokenKind::String),
                token_modifiers_bitset: token_modifier_bitset(CoreSemanticTokenModifier::Payload),
            },
        ];
        let current = vec![
            SemanticToken {
                delta_line: 0,
                delta_start: 0,
                length: 3,
                token_type: token_type(SemanticTokenKind::Namespace),
                token_modifiers_bitset: 0,
            },
            SemanticToken {
                delta_line: 0,
                delta_start: 5,
                length: 2,
                token_type: token_type(SemanticTokenKind::String),
                token_modifiers_bitset: token_modifier_bitset(CoreSemanticTokenModifier::Payload),
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
