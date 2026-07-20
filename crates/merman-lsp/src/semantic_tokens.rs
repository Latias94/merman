use crate::snapshot::DocumentSnapshot;
use merman_editor_core::{
    PlannedToken, SEMANTIC_TOKEN_DESCRIPTOR_DIGEST, SEMANTIC_TOKEN_PACKED_WORDS_PER_TOKEN,
    SemanticTokenPlan, TokenPlanError, plan_semantic_tokens_for_snapshot,
    semantic_token_descriptor,
};
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use tower_lsp::lsp_types::{
    Range, SemanticToken, SemanticTokenModifier, SemanticTokenType, SemanticTokens,
    SemanticTokensDelta, SemanticTokensEdit, SemanticTokensFullDeltaResult,
    SemanticTokensFullOptions, SemanticTokensLegend, SemanticTokensOptions,
};

pub fn semantic_tokens_options() -> SemanticTokensOptions {
    SemanticTokensOptions {
        work_done_progress_options: Default::default(),
        legend: semantic_tokens_legend(),
        range: Some(true),
        full: Some(SemanticTokensFullOptions::Delta { delta: Some(true) }),
    }
}

pub fn semantic_tokens_legend() -> SemanticTokensLegend {
    let descriptor = semantic_token_descriptor();
    SemanticTokensLegend {
        token_types: descriptor
            .token_kinds
            .iter()
            .map(|kind| SemanticTokenType::new(kind.lsp_name))
            .collect(),
        token_modifiers: descriptor
            .modifiers
            .iter()
            .map(|modifier| SemanticTokenModifier::new(modifier.lsp_name))
            .collect(),
    }
}

pub fn semantic_tokens_for_snapshot(
    snapshot: &DocumentSnapshot,
) -> Result<SemanticTokens, TokenPlanError> {
    let plan = token_plan(snapshot)?;
    Ok(SemanticTokens {
        result_id: None,
        data: encode_relative_tokens(plan.tokens()),
    })
}

pub fn semantic_tokens_for_snapshot_range(
    snapshot: &DocumentSnapshot,
    range: Range,
) -> Result<SemanticTokens, TokenPlanError> {
    let plan = token_plan(snapshot)?;
    let data = encode_relative_tokens(
        plan.tokens()
            .iter()
            .filter(|token| token_overlaps_range(token, &range)),
    );
    Ok(SemanticTokens {
        result_id: None,
        data,
    })
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
    format!(
        "{}:{}:{:016x}",
        snapshot.version,
        SEMANTIC_TOKEN_DESCRIPTOR_DIGEST,
        hasher.finish()
    )
}

fn token_plan(snapshot: &DocumentSnapshot) -> Result<SemanticTokenPlan, TokenPlanError> {
    plan_semantic_tokens_for_snapshot(snapshot.as_editor())
}

fn token_overlaps_range(token: &PlannedToken, range: &Range) -> bool {
    if token.line < range.start.line || token.line > range.end.line {
        return false;
    }

    let token_end = token.start + token.length;
    if range.start.line == range.end.line {
        return token.line == range.start.line
            && token_end > range.start.character
            && token.start < range.end.character;
    }
    if token.line == range.start.line {
        return token_end > range.start.character;
    }
    if token.line == range.end.line {
        return token.start < range.end.character;
    }
    true
}

fn encode_relative_tokens<'a>(
    tokens: impl IntoIterator<Item = &'a PlannedToken>,
) -> Vec<SemanticToken> {
    let mut previous_line = 0u32;
    let mut previous_start = 0u32;
    tokens
        .into_iter()
        .map(|token| {
            let delta_line = token.line - previous_line;
            let delta_start = if delta_line == 0 {
                token.start - previous_start
            } else {
                token.start
            };
            previous_line = token.line;
            previous_start = token.start;
            SemanticToken {
                delta_line,
                delta_start,
                length: token.length,
                token_type: token.kind.code(),
                token_modifiers_bitset: token.modifier_bits,
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
    let flattened_prefix = prefix_tokens
        .checked_mul(SEMANTIC_TOKEN_PACKED_WORDS_PER_TOKEN)
        .and_then(|value| u32::try_from(value).ok())
        .expect("semantic token delta prefix fits the LSP u32 contract");
    let flattened_delete_count = (previous_end - prefix_tokens)
        .checked_mul(SEMANTIC_TOKEN_PACKED_WORDS_PER_TOKEN)
        .and_then(|value| u32::try_from(value).ok())
        .expect("semantic token delta deletion fits the LSP u32 contract");

    Some(SemanticTokensEdit {
        start: flattened_prefix,
        delete_count: flattened_delete_count,
        data: if prefix_tokens < current_end {
            Some(current_tokens[prefix_tokens..current_end].to_vec())
        } else {
            None
        },
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::document_store::DocumentStore;
    use merman_editor_core::{PlannedTokenKind, PlannedTokenModifier};
    use serde::Deserialize;
    use std::fs;
    use std::path::PathBuf;
    use tower_lsp::lsp_types::{Position, Url};

    #[derive(Debug, Deserialize)]
    struct TokenEquivalenceEvidence {
        descriptor_digest: String,
        words_per_token: usize,
        family_cases: Vec<TokenEquivalenceCase>,
        recovery_cases: Vec<TokenEquivalenceCase>,
    }

    #[derive(Debug, Deserialize)]
    struct TokenEquivalenceCase {
        id: String,
        family: String,
        source: String,
        detection_validity: String,
        syntax_id: String,
        effective_layout_id: String,
        packed_words: Vec<u32>,
    }

    #[test]
    fn legend_is_the_generated_descriptor_order() {
        let descriptor = semantic_token_descriptor();
        let legend = semantic_tokens_legend();

        assert_eq!(
            legend.token_types,
            descriptor
                .token_kinds
                .iter()
                .map(|kind| SemanticTokenType::new(kind.lsp_name))
                .collect::<Vec<_>>()
        );
        assert_eq!(
            legend.token_modifiers,
            descriptor
                .modifiers
                .iter()
                .map(|modifier| SemanticTokenModifier::new(modifier.lsp_name))
                .collect::<Vec<_>>()
        );
        assert!(
            descriptor
                .token_kinds
                .iter()
                .enumerate()
                .all(|(index, kind)| kind.kind.code() == index as u32
                    && kind.lsp_index == index as u32)
        );
        assert!(
            descriptor
                .modifiers
                .iter()
                .enumerate()
                .all(|(index, modifier)| {
                    modifier.modifier.index() == index as u32
                        && modifier.lsp_index == index as u32
                        && modifier.bit == 1 << index
                })
        );
    }

    #[test]
    fn full_sequence_is_the_planner_packed_sequence() {
        let mut store = DocumentStore::new();
        let uri = Url::parse("file:///tmp/example.mmd").unwrap();
        let snapshot = store.upsert(
            uri,
            1,
            concat!(
                "flowchart TD\n",
                "alpha[\"Emoji 🤓\"] --> beta\n",
                "%% trailing comment\n",
            )
            .to_string(),
        );
        let plan = plan_semantic_tokens_for_snapshot(snapshot.as_editor()).unwrap();
        let lsp = semantic_tokens_for_snapshot(&snapshot).unwrap();

        assert_eq!(flatten_tokens(&lsp.data), plan.packed());
        assert!(
            plan.tokens()
                .iter()
                .any(|token| token.kind == PlannedTokenKind::Keyword)
        );
        assert!(
            plan.tokens()
                .iter()
                .any(|token| { token.modifier_bits & PlannedTokenModifier::Entity.bit() != 0 })
        );
    }

    #[test]
    fn unavailable_semantics_are_a_valid_empty_plan_not_a_planner_failure() {
        let mut store = DocumentStore::new();
        let uri = Url::parse("file:///tmp/unknown.mmd").unwrap();
        let snapshot = store.upsert(uri, 1, "not a Mermaid diagram\n".to_string());

        let tokens = semantic_tokens_for_snapshot(&snapshot).unwrap();
        assert!(tokens.data.is_empty());
    }

    #[test]
    fn all_family_and_recovery_sequences_match_the_generated_cross_surface_evidence() {
        let evidence_path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../..")
            .join("editor-language/token-equivalence-v1.json");
        let evidence: TokenEquivalenceEvidence = serde_json::from_str(
            &fs::read_to_string(&evidence_path).expect("generated token equivalence evidence"),
        )
        .expect("valid token equivalence evidence");
        assert_eq!(evidence.descriptor_digest, SEMANTIC_TOKEN_DESCRIPTOR_DIGEST);
        assert_eq!(
            evidence.words_per_token,
            SEMANTIC_TOKEN_PACKED_WORDS_PER_TOKEN
        );
        assert_eq!(evidence.family_cases.len(), 35);
        assert_eq!(evidence.recovery_cases.len(), 1);

        let mut store = DocumentStore::new();
        for (version, case) in evidence
            .family_cases
            .iter()
            .chain(&evidence.recovery_cases)
            .enumerate()
        {
            let uri = Url::parse(&format!("file:///token-equivalence/{}.mmd", case.id)).unwrap();
            let snapshot = store.upsert(uri, version as i32 + 1, case.source.clone());
            let tokens = semantic_tokens_for_snapshot(&snapshot).unwrap();

            assert_eq!(
                flatten_tokens(&tokens.data),
                case.packed_words,
                "{} ({}) LSP packed sequence",
                case.id,
                case.family
            );
            assert_eq!(
                snapshot
                    .detection()
                    .map(|detection| detection.diagram_type.as_str()),
                Some(case.family.as_str()),
                "{} diagram detection",
                case.id
            );
            assert_eq!(
                snapshot
                    .detection()
                    .map(|detection| match detection.validity {
                        merman_editor_core::DiagramDetectionValidity::Valid => "valid",
                        merman_editor_core::DiagramDetectionValidity::RecoverableInvalid => {
                            "recoverable-invalid"
                        }
                    }),
                Some(case.detection_validity.as_str()),
                "{} recovery identity",
                case.id
            );
            assert_eq!(
                snapshot
                    .detection()
                    .map(|detection| detection.syntax_id.as_str()),
                Some(case.syntax_id.as_str()),
                "{} syntax identity",
                case.id
            );
            assert_eq!(
                snapshot
                    .detection()
                    .map(|detection| detection.effective_layout_id.as_str()),
                Some(case.effective_layout_id.as_str()),
                "{} effective layout identity",
                case.id
            );
        }
    }

    #[test]
    fn range_filters_absolute_plan_then_reencodes_relative_tokens() {
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
                "title: Second 🤓\n",
                "```\n",
                "outro\n",
            )
            .to_string(),
        );
        let range = Range::new(Position::new(6, 0), Position::new(10, 0));
        let plan = plan_semantic_tokens_for_snapshot(snapshot.as_editor()).unwrap();
        let expected = plan
            .tokens()
            .iter()
            .filter(|token| token_overlaps_range(token, &range))
            .map(|token| {
                (
                    token.line,
                    token.start,
                    token.length,
                    token.kind.code(),
                    token.modifier_bits,
                )
            })
            .collect::<Vec<_>>();
        let actual = decode_tokens(
            &semantic_tokens_for_snapshot_range(&snapshot, range)
                .unwrap()
                .data,
        );

        assert_eq!(actual, expected);
        assert!(actual.iter().all(|token| (6..=10).contains(&token.0)));
    }

    #[test]
    fn result_id_binds_document_tokens_to_descriptor_digest() {
        let mut store = DocumentStore::new();
        let uri = Url::parse("file:///tmp/example.mmd").unwrap();
        let snapshot = store.upsert(uri, 7, "flowchart TD\nA --> B\n".to_string());
        let tokens = semantic_tokens_for_snapshot(&snapshot).unwrap();
        let result_id = semantic_tokens_result_id(&snapshot, &tokens.data);

        assert!(result_id.starts_with("7:sha256:"));
        assert!(result_id.contains(SEMANTIC_TOKEN_DESCRIPTOR_DIGEST));
    }

    #[test]
    fn delta_uses_generated_record_width_and_preserves_sequence_suffix() {
        let previous = vec![
            semantic_token(0, 0, 3, 0, 0),
            semantic_token(0, 4, 2, 9, PlannedTokenModifier::Payload.bit()),
            semantic_token(1, 0, 1, 1, 0),
        ];
        let current = vec![
            semantic_token(0, 0, 3, 0, 0),
            semantic_token(0, 5, 2, 9, PlannedTokenModifier::Payload.bit()),
            semantic_token(1, 0, 1, 1, 0),
        ];

        let result = semantic_tokens_delta_result(&previous, &current, "next".to_string());
        let SemanticTokensFullDeltaResult::TokensDelta(delta) = result else {
            panic!("expected delta tokens");
        };
        assert_eq!(delta.result_id.as_deref(), Some("next"));
        assert_eq!(delta.edits.len(), 1);
        assert_eq!(
            delta.edits[0].start,
            SEMANTIC_TOKEN_PACKED_WORDS_PER_TOKEN as u32
        );
        assert_eq!(
            delta.edits[0].delete_count,
            SEMANTIC_TOKEN_PACKED_WORDS_PER_TOKEN as u32
        );
        assert_eq!(delta.edits[0].data.as_deref(), Some(&current[1..2]));
    }

    fn semantic_token(
        delta_line: u32,
        delta_start: u32,
        length: u32,
        token_type: u32,
        token_modifiers_bitset: u32,
    ) -> SemanticToken {
        SemanticToken {
            delta_line,
            delta_start,
            length,
            token_type,
            token_modifiers_bitset,
        }
    }

    fn flatten_tokens(tokens: &[SemanticToken]) -> Vec<u32> {
        tokens
            .iter()
            .flat_map(|token| {
                [
                    token.delta_line,
                    token.delta_start,
                    token.length,
                    token.token_type,
                    token.token_modifiers_bitset,
                ]
            })
            .collect()
    }

    fn decode_tokens(tokens: &[SemanticToken]) -> Vec<(u32, u32, u32, u32, u32)> {
        let mut line = 0u32;
        let mut start = 0u32;
        tokens
            .iter()
            .map(|token| {
                line += token.delta_line;
                if token.delta_line == 0 {
                    start += token.delta_start;
                } else {
                    start = token.delta_start;
                }
                (
                    line,
                    start,
                    token.length,
                    token.token_type,
                    token.token_modifiers_bitset,
                )
            })
            .collect()
    }
}
