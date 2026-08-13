use crate::client_profile::{ClientProtocolProfile, SemanticTokenProjection};
use crate::snapshot::DocumentSnapshot;
use merman_editor_core::{
    SEMANTIC_TOKEN_DESCRIPTOR_DIGEST, SEMANTIC_TOKEN_PACKED_WORDS_PER_TOKEN, SemanticTokenPlan,
    SemanticTokenSupport, TokenPlanError, plan_semantic_tokens_for_snapshot_range_with_support,
    plan_semantic_tokens_for_snapshot_with_support,
};
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use tower_lsp_server::ls_types::{
    Range, SemanticToken, SemanticTokensDelta, SemanticTokensEdit, SemanticTokensFullDeltaResult,
    SemanticTokensOptions,
};
pub(crate) fn semantic_tokens_options_with_profile(
    profile: &ClientProtocolProfile,
) -> Option<SemanticTokensOptions> {
    profile
        .semantic_tokens
        .as_ref()
        .map(SemanticTokenProjection::options)
}

pub(crate) fn semantic_token_plan_for_snapshot_with_profile(
    snapshot: &DocumentSnapshot,
    profile: &ClientProtocolProfile,
) -> Result<Option<SemanticTokenPlan>, TokenPlanError> {
    let projection = profile.semantic_tokens.as_ref();
    let Some(projection) = projection.filter(|projection| projection.supports_full()) else {
        return Ok(None);
    };
    token_plan(snapshot, projection.support()).map(Some)
}

pub(crate) fn semantic_token_plan_for_snapshot_range_with_profile(
    snapshot: &DocumentSnapshot,
    range: Range,
    profile: &ClientProtocolProfile,
) -> Result<Option<SemanticTokenPlan>, TokenPlanError> {
    let projection = profile.semantic_tokens.as_ref();
    let Some(projection) = projection.filter(|projection| projection.supports_range()) else {
        return Ok(None);
    };
    token_plan_range(snapshot, range, projection.support()).map(Some)
}

pub fn semantic_tokens_delta_result(
    previous_packed: &[u32],
    current_packed: &[u32],
    result_id: String,
) -> SemanticTokensFullDeltaResult {
    let Some(edit) = semantic_tokens_delta_edit(previous_packed, current_packed) else {
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

pub fn semantic_tokens_result_id(snapshot: &DocumentSnapshot, packed: &[u32]) -> String {
    let mut hasher = DefaultHasher::new();
    snapshot.version().hash(&mut hasher);
    for word in packed {
        word.hash(&mut hasher);
    }
    format!(
        "{}:{}:{:016x}",
        snapshot.version(),
        SEMANTIC_TOKEN_DESCRIPTOR_DIGEST,
        hasher.finish()
    )
}

fn token_plan(
    snapshot: &DocumentSnapshot,
    support: SemanticTokenSupport,
) -> Result<SemanticTokenPlan, TokenPlanError> {
    plan_semantic_tokens_for_snapshot_with_support(snapshot.as_editor(), support)
}

fn token_plan_range(
    snapshot: &DocumentSnapshot,
    range: Range,
    support: SemanticTokenSupport,
) -> Result<SemanticTokenPlan, TokenPlanError> {
    plan_semantic_tokens_for_snapshot_range_with_support(
        snapshot.as_editor(),
        merman_editor_core::Range::new(
            merman_editor_core::Position::new(
                range.start.line as usize,
                range.start.character as usize,
            ),
            merman_editor_core::Position::new(
                range.end.line as usize,
                range.end.character as usize,
            ),
        ),
        support,
    )
}

pub(crate) fn semantic_tokens_from_packed(packed: &[u32]) -> Vec<SemanticToken> {
    debug_assert_eq!(packed.len() % SEMANTIC_TOKEN_PACKED_WORDS_PER_TOKEN, 0);
    packed
        .chunks_exact(SEMANTIC_TOKEN_PACKED_WORDS_PER_TOKEN)
        .map(|words| SemanticToken {
            delta_line: words[0],
            delta_start: words[1],
            length: words[2],
            token_type: words[3],
            token_modifiers_bitset: words[4],
        })
        .collect()
}

fn semantic_tokens_delta_edit(
    previous_packed: &[u32],
    current_packed: &[u32],
) -> Option<SemanticTokensEdit> {
    debug_assert_eq!(
        previous_packed.len() % SEMANTIC_TOKEN_PACKED_WORDS_PER_TOKEN,
        0
    );
    debug_assert_eq!(
        current_packed.len() % SEMANTIC_TOKEN_PACKED_WORDS_PER_TOKEN,
        0
    );
    let previous_tokens = previous_packed.chunks_exact(SEMANTIC_TOKEN_PACKED_WORDS_PER_TOKEN);
    let current_tokens = current_packed.chunks_exact(SEMANTIC_TOKEN_PACKED_WORDS_PER_TOKEN);
    let previous_token_count = previous_tokens.len();
    let current_token_count = current_tokens.len();
    let prefix_tokens = previous_tokens
        .clone()
        .zip(current_tokens.clone())
        .take_while(|(previous, current)| previous == current)
        .count();

    if prefix_tokens == previous_token_count && prefix_tokens == current_token_count {
        return None;
    }

    let suffix_tokens = previous_tokens
        .skip(prefix_tokens)
        .rev()
        .zip(current_tokens.skip(prefix_tokens).rev())
        .take_while(|(previous, current)| previous == current)
        .count();

    let previous_end = previous_token_count.saturating_sub(suffix_tokens);
    let current_end = current_token_count.saturating_sub(suffix_tokens);
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
            let start = prefix_tokens * SEMANTIC_TOKEN_PACKED_WORDS_PER_TOKEN;
            let end = current_end * SEMANTIC_TOKEN_PACKED_WORDS_PER_TOKEN;
            Some(semantic_tokens_from_packed(&current_packed[start..end]))
        } else {
            None
        },
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::client_profile::ClientProtocolProfile;
    use crate::snapshot::snapshot_for_test;
    use merman_editor_core::{PlannedTokenKind, PlannedTokenModifier};
    use serde::Deserialize;
    use std::{fs, path::PathBuf, str::FromStr};
    use tower_lsp_server::ls_types::{Position, Uri};

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
    fn full_sequence_is_the_negotiated_planner_packed_sequence() {
        let capabilities: tower_lsp_server::ls_types::ClientCapabilities =
            serde_json::from_value(serde_json::json!({
                "textDocument": {
                    "semanticTokens": {
                        "requests": { "full": true },
                        "tokenTypes": ["string"],
                        "tokenModifiers": ["mermanPayload"],
                        "formats": ["relative"]
                    }
                }
            }))
            .unwrap();
        let profile = ClientProtocolProfile::negotiate(&capabilities);
        let projection = profile
            .semantic_tokens
            .as_ref()
            .expect("string-only client should negotiate semantic tokens");
        let uri = Uri::from_str("file:///tmp/subset.mmd").unwrap();
        let snapshot = snapshot_for_test(
            uri,
            1,
            "flowchart TD\nA[\"first\"] --> B\nC[\"second\"] --> D\n",
        );
        let plan = plan_semantic_tokens_for_snapshot_with_support(
            snapshot.as_editor(),
            projection.support(),
        )
        .expect("negotiated planner output");
        let lsp_plan = semantic_token_plan_for_snapshot_with_profile(&snapshot, &profile)
            .expect("LSP token planning")
            .expect("semantic tokens enabled");
        let lsp_data = semantic_tokens_from_packed(lsp_plan.packed());

        assert_eq!(lsp_plan.packed(), plan.packed());
        assert_eq!(flatten_tokens(&lsp_data), plan.packed());
        assert!(
            plan.tokens()
                .iter()
                .all(|token| token.kind == PlannedTokenKind::String)
        );
    }

    #[test]
    fn unavailable_semantics_are_a_valid_empty_planner_result() {
        let uri = Uri::from_str("file:///tmp/unknown.mmd").unwrap();
        let snapshot = snapshot_for_test(uri, 1, "not a Mermaid diagram\n");

        let plan = token_plan(&snapshot, SemanticTokenSupport::all()).unwrap();
        assert!(plan.packed().is_empty());
    }

    #[test]
    fn request_specific_planners_respect_negotiated_modes() {
        let uri = Uri::from_str("file:///tmp/modes.mmd").unwrap();
        let snapshot = snapshot_for_test(uri, 1, "flowchart TD\nA-->B\n");
        let range = Range::new(Position::new(0, 0), Position::new(2, 0));
        let profile = |requests| {
            let capabilities = serde_json::from_value(serde_json::json!({
                "textDocument": {
                    "semanticTokens": {
                        "requests": requests,
                        "tokenTypes": ["keyword"],
                        "tokenModifiers": [],
                        "formats": ["relative"]
                    }
                }
            }))
            .unwrap();
            ClientProtocolProfile::negotiate(&capabilities)
        };

        let range_only = profile(serde_json::json!({ "range": true }));
        assert!(
            semantic_token_plan_for_snapshot_with_profile(&snapshot, &range_only)
                .unwrap()
                .is_none()
        );
        assert!(
            semantic_token_plan_for_snapshot_range_with_profile(&snapshot, range, &range_only)
                .unwrap()
                .is_some()
        );

        let full_only = profile(serde_json::json!({ "full": true }));
        assert!(
            semantic_token_plan_for_snapshot_with_profile(&snapshot, &full_only)
                .unwrap()
                .is_some()
        );
        assert!(
            semantic_token_plan_for_snapshot_range_with_profile(&snapshot, range, &full_only)
                .unwrap()
                .is_none()
        );
    }

    #[test]
    fn all_family_and_recovery_sequences_match_the_generated_cross_surface_evidence() {
        let evidence_path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../..")
            .join("contracts/editor-language/token-equivalence-v1.json");
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

        for (version, case) in evidence
            .family_cases
            .iter()
            .chain(&evidence.recovery_cases)
            .enumerate()
        {
            let uri = Uri::from_str(&format!("file:///token-equivalence/{}.mmd", case.id)).unwrap();
            let snapshot = snapshot_for_test(uri, version as i32 + 1, case.source.clone());
            let plan = token_plan(&snapshot, SemanticTokenSupport::all()).unwrap();

            assert_eq!(
                plan.packed(),
                case.packed_words,
                "{} ({}) planner-packed sequence",
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
    fn packed_words_project_to_tower_tokens_without_reencoding() {
        let packed = [1, 2, 3, 4, 5, 0, 6, 7, 8, 9];

        assert_eq!(
            flatten_tokens(&semantic_tokens_from_packed(&packed)),
            packed
        );
    }

    #[test]
    fn range_sequence_is_the_negotiated_planner_packed_sequence() {
        let uri = Uri::from_str("file:///tmp/example.md").unwrap();
        let snapshot = snapshot_for_test(
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
            ),
        );
        let range = Range::new(Position::new(6, 0), Position::new(10, 0));
        let capabilities: tower_lsp_server::ls_types::ClientCapabilities =
            serde_json::from_value(serde_json::json!({
                "textDocument": {
                    "semanticTokens": {
                        "requests": { "range": true },
                        "tokenTypes": ["keyword", "string"],
                        "tokenModifiers": [],
                        "formats": ["relative"]
                    }
                }
            }))
            .unwrap();
        let profile = ClientProtocolProfile::negotiate(&capabilities);
        let support = profile
            .semantic_tokens
            .as_ref()
            .expect("range semantic tokens")
            .support();
        let planner_range = merman_editor_core::Range::new(
            merman_editor_core::Position::new(6, 0),
            merman_editor_core::Position::new(10, 0),
        );
        let plan = plan_semantic_tokens_for_snapshot_range_with_support(
            snapshot.as_editor(),
            planner_range,
            support,
        )
        .expect("negotiated range plan");
        let lsp_plan =
            semantic_token_plan_for_snapshot_range_with_profile(&snapshot, range, &profile)
                .expect("LSP range planning")
                .expect("range semantic tokens enabled");
        let lsp_data = semantic_tokens_from_packed(lsp_plan.packed());
        let actual = decode_tokens(&lsp_data);

        assert_eq!(lsp_plan.packed(), plan.packed());
        assert_eq!(flatten_tokens(&lsp_data), plan.packed());
        assert!(actual.iter().all(|token| (6..=10).contains(&token.0)));
    }

    #[test]
    fn result_id_is_stable_for_the_same_canonical_stream_and_changes_with_content() {
        let uri = Uri::from_str("file:///tmp/example.mmd").unwrap();
        let snapshot = snapshot_for_test(uri, 7, "flowchart TD\nA --> B\n");
        let plan = token_plan(&snapshot, SemanticTokenSupport::all()).unwrap();
        let result_id = semantic_tokens_result_id(&snapshot, plan.packed());

        assert_eq!(
            result_id,
            semantic_tokens_result_id(&snapshot, plan.packed())
        );
        let mut changed = plan.packed().to_vec();
        changed.push(0);
        assert_ne!(result_id, semantic_tokens_result_id(&snapshot, &changed));
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

        let result = semantic_tokens_delta_result(
            &flatten_tokens(&previous),
            &flatten_tokens(&current),
            "next".to_string(),
        );
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
        assert_eq!(
            apply_delta(&flatten_tokens(&previous), &delta.edits),
            flatten_tokens(&current)
        );
    }

    #[test]
    fn delta_round_trips_insertions_removals_and_noop_streams() {
        let first = vec![semantic_token(0, 0, 3, 0, 0), semantic_token(0, 4, 2, 1, 0)];
        let inserted = vec![
            semantic_token(0, 0, 3, 0, 0),
            semantic_token(0, 4, 1, 2, 0),
            semantic_token(0, 2, 2, 1, 0),
        ];

        for (previous, current) in [(&first, &inserted), (&inserted, &first)] {
            let result = semantic_tokens_delta_result(
                &flatten_tokens(previous),
                &flatten_tokens(current),
                "next".to_string(),
            );
            let SemanticTokensFullDeltaResult::TokensDelta(delta) = result else {
                panic!("expected delta tokens");
            };
            assert_eq!(
                apply_delta(&flatten_tokens(previous), &delta.edits),
                flatten_tokens(current)
            );
        }

        let unchanged = flatten_tokens(&first);
        let SemanticTokensFullDeltaResult::TokensDelta(delta) =
            semantic_tokens_delta_result(&unchanged, &unchanged, "same".to_string())
        else {
            panic!("expected no-op delta tokens");
        };
        assert!(delta.edits.is_empty());
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

    fn apply_delta(previous: &[u32], edits: &[SemanticTokensEdit]) -> Vec<u32> {
        let mut result = previous.to_vec();
        for edit in edits.iter().rev() {
            let start = edit.start as usize;
            let end = start + edit.delete_count as usize;
            let replacement = edit.data.as_deref().map(flatten_tokens).unwrap_or_default();
            result.splice(start..end, replacement);
        }
        result
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
