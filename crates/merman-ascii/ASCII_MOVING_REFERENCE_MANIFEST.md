# ASCII Moving Reference Manifest

Status: discovery evidence, not a byte oracle
Last updated: 2026-08-10

This manifest records the fixture delta discovered after the immutable v1 copy. It exists so a
local `repo-ref/` checkout can inform development without becoming an undeclared release
dependency or turning reference-specific layout into a Merman contract.

## Snapshot

- Moving `mermaid-ascii` reference: `b1b35f67d6a5dd0699ccfc968c00a763db573076`
- Immutable copied baseline: `6fffb8e2714acab2c4cb41c78894fabbc62cee56`
- `beautiful-mermaid` capability-prior-art snapshot:
  `2ac8bbbb060ca0a65a6a21f3200bd99b1587b488`
- Mermaid validity authority: local pinned Mermaid `11.16.1`
- Delta inventory: 137 paths: 5 Flowchart, 51 Sequence, and 81 ER fixtures

The 96 paths already present in the copied v1 corpus remain governed by
`V1_MERMAID_ASCII_COVERAGE.md`. The entries below are only the moving-reference delta. Reference
expected bytes are never imported by this manifest.

## Dispositions

- Classification `mermaid_valid`: Mermaid 11.16.1 accepts the input.
- Classification `mixed_valid_private_behavior`: Mermaid accepts the input, but the moving
  reference assigns at least one construct a different, reference-private meaning.
- Admission `semantic_probe`: equivalent behavior is owned by parser-backed local tests; the
  reference output is not an oracle.
- Admission `discovery_only`: the case remains useful research input but authorizes no support
  claim.

Each entry inherits classification, admission, semantic feature, and evidence from its section.

## Flowchart Bidirectional Edges

- Classification: `mermaid_valid`
- Admission: `semantic_probe`
- Semantic feature: bidirectional endpoints, labels, and LR/TD direction
- Evidence: `tests/flowchart_model.rs` endpoint-marker and direction tests

- `ascii/bidirectional_labeled_lr.txt`
- `ascii/bidirectional_lr.txt`
- `ascii/bidirectional_td.txt`
- `extended-chars/bidirectional_lr.txt`
- `extended-chars/bidirectional_td.txt`

## Sequence Actor Identity

- Classification: `mermaid_valid`
- Admission: `semantic_probe`
- Semantic feature: aliases, actor/participant mixing, spaces, dashes, quoted arrow text, and actor
  identity inside central, fragment, note, and self-message contexts
- Evidence: `merman-core/src/tests/sequence.rs` and `tests/sequence_model.rs`

- `sequence/actor_alias.txt`
- `sequence/actor_participant_mix.txt`
- `sequence/central_spaced_names.txt`
- `sequence/fragment_spaced_names.txt`
- `sequence/note_over_spaced_names.txt`
- `sequence/participant_names_dashes.txt`
- `sequence/participant_names_spaces.txt`
- `sequence/quoted_name_with_arrow.txt`
- `sequence/self_message_spaced_name.txt`
- `sequence-ascii/actor_alias.txt`
- `sequence-ascii/actor_participant_mix.txt`
- `sequence-ascii/fragment_spaced_names.txt`
- `sequence-ascii/note_over_spaced_names.txt`
- `sequence-ascii/participant_names_dashes.txt`
- `sequence-ascii/participant_names_spaces.txt`

## Sequence Mixed Alias Behavior

- Classification: `mixed_valid_private_behavior`
- Admission: `discovery_only`
- Semantic feature: `data=svc as DS` is a Mermaid alias, while `cron job as Cron` is one literal
  spaced actor id under pinned Mermaid and a split alias under the moving reference
- Evidence: `merman-core/src/tests/sequence.rs` locks the pinned spaced-alias boundary

- `sequence/participant_names_alias_equals.txt`
- `sequence-ascii/participant_names_alias_equals.txt`

## Sequence Message Markers

- Classification: `mermaid_valid`
- Admission: `semantic_probe`
- Semantic feature: open/filled/cross/point/bidirectional markers, central connections, and self
  message endpoint ownership in ASCII and Unicode
- Evidence: typed signal and glyph-disjointness tests in `tests/sequence_model.rs`

- `sequence/arrow_types.txt`
- `sequence/async_point_arrows.txt`
- `sequence/bidirectional_arrows.txt`
- `sequence/central_connection_self.txt`
- `sequence/central_connections.txt`
- `sequence/cross_arrows.txt`
- `sequence/self_arrow_variants.txt`
- `sequence-ascii/arrow_types.txt`
- `sequence-ascii/async_point_arrows.txt`
- `sequence-ascii/bidirectional_arrows.txt`
- `sequence-ascii/central_connection_self.txt`
- `sequence-ascii/central_connections.txt`
- `sequence-ascii/cross_arrows.txt`
- `sequence-ascii/self_arrow_variants.txt`

## Sequence Controls And Notes

- Classification: `mermaid_valid`
- Admission: `semantic_probe`
- Semantic feature: empty and nested controls, alternate sections, autonumber, notes, participant
  span, and control labels
- Evidence: recursive control-plan, note, and autonumber tests in `tests/sequence_model.rs`

- `sequence/alt_basic.txt`
- `sequence/alt_multiple_else.txt`
- `sequence/break_rect.txt`
- `sequence/critical_basic.txt`
- `sequence/fragment_partial_span.txt`
- `sequence/loop_autonumber.txt`
- `sequence/loop_basic.txt`
- `sequence/loop_empty.txt`
- `sequence/note_in_loop.txt`
- `sequence/note_over_single.txt`
- `sequence/note_over_span.txt`
- `sequence/opt_basic.txt`
- `sequence/opt_no_label.txt`
- `sequence/par_basic.txt`
- `sequence-ascii/alt_basic.txt`
- `sequence-ascii/loop_basic.txt`
- `sequence-ascii/loop_empty.txt`
- `sequence-ascii/note_over_single.txt`
- `sequence-ascii/opt_basic.txt`
- `sequence-ascii/par_basic.txt`

## ER Attributes

- Classification: `mermaid_valid`
- Admission: `semantic_probe`
- Semantic feature: attribute type/name roles, keys, comments, quoted names, repeated blocks, and
  relationship coexistence
- Evidence: typed attribute-role and entity-section tests in `tests/er_model.rs`

- `er/attributes_asterisk_names.txt`
- `er/attributes_backtick_names.txt`
- `er/attributes_basic.txt`
- `er/attributes_closing_brace_inline.txt`
- `er/attributes_complex_types.txt`
- `er/attributes_composite_key.txt`
- `er/attributes_empty_block.txt`
- `er/attributes_keys_and_comments.txt`
- `er/attributes_multiple_blocks.txt`
- `er/attributes_with_relationship.txt`
- `er-ascii/attributes_keys_and_comments.txt`
- `er-ascii/attributes_with_relationship.txt`

## ER Cardinality Matrix

- Classification: `mermaid_valid`
- Admission: `semantic_probe`
- Semantic feature: numeric and word aliases plus every identifying/non-identifying cardinality
  endpoint combination
- Evidence: parser cardinality tests in `merman-core` and marker ownership tests in
  `tests/er_model.rs`

- `er/cardinality_numeric.txt`
- `er/cardinality_numeric_all.txt`
- `er/cardinality_one_or_more.txt`
- `er/cardinality_one_to_one.txt`
- `er/cardinality_word_aliases.txt`
- `er/cardinality_words.txt`
- `er/cardinality_zero_or_more.txt`
- `er/cardinality_zero_or_one.txt`
- `er/matrix_one_or_more_to_one.txt`
- `er/matrix_one_or_more_to_one_dashed.txt`
- `er/matrix_one_or_more_to_one_or_more.txt`
- `er/matrix_one_or_more_to_one_or_more_dashed.txt`
- `er/matrix_one_or_more_to_zero_or_more.txt`
- `er/matrix_one_or_more_to_zero_or_more_dashed.txt`
- `er/matrix_one_or_more_to_zero_or_one.txt`
- `er/matrix_one_or_more_to_zero_or_one_dashed.txt`
- `er/matrix_one_to_one.txt`
- `er/matrix_one_to_one_dashed.txt`
- `er/matrix_one_to_one_or_more.txt`
- `er/matrix_one_to_one_or_more_dashed.txt`
- `er/matrix_one_to_zero_or_more.txt`
- `er/matrix_one_to_zero_or_more_dashed.txt`
- `er/matrix_one_to_zero_or_one.txt`
- `er/matrix_one_to_zero_or_one_dashed.txt`
- `er/matrix_zero_or_more_to_one.txt`
- `er/matrix_zero_or_more_to_one_dashed.txt`
- `er/matrix_zero_or_more_to_one_or_more.txt`
- `er/matrix_zero_or_more_to_one_or_more_dashed.txt`
- `er/matrix_zero_or_more_to_zero_or_more.txt`
- `er/matrix_zero_or_more_to_zero_or_more_dashed.txt`
- `er/matrix_zero_or_more_to_zero_or_one.txt`
- `er/matrix_zero_or_more_to_zero_or_one_dashed.txt`
- `er/matrix_zero_or_one_to_one.txt`
- `er/matrix_zero_or_one_to_one_dashed.txt`
- `er/matrix_zero_or_one_to_one_or_more.txt`
- `er/matrix_zero_or_one_to_one_or_more_dashed.txt`
- `er/matrix_zero_or_one_to_zero_or_more.txt`
- `er/matrix_zero_or_one_to_zero_or_more_dashed.txt`
- `er/matrix_zero_or_one_to_zero_or_one.txt`
- `er/matrix_zero_or_one_to_zero_or_one_dashed.txt`
- `er-ascii/cardinality_one_or_more.txt`

## ER Entity Identity

- Classification: `mermaid_valid`
- Admission: `semantic_probe`
- Semantic feature: aliases, quoted and multiword names, CJK, empty labels, standalone entities,
  and long terminal labels
- Evidence: parser identity tests in `merman-core` and terminal-width tests in `tests/er_model.rs`

- `er/alias_forms.txt`
- `er/cjk_entities.txt`
- `er/empty_label.txt`
- `er/entities_without_relationships.txt`
- `er/long_entity_names.txt`
- `er/multiword_labels.txt`
- `er/quoted_entity_names.txt`
- `er-ascii/alias_forms.txt`
- `er-ascii/cjk_entities.txt`
- `er-ascii/multiword_labels.txt`
- `er-ascii/quoted_entity_names.txt`

## ER Relation Topology

- Classification: `mermaid_valid`
- Admission: `semantic_probe`
- Semantic feature: declaration order, dense components, duplicate and self relationships,
  identifying kind, dashed variants, and relation labels
- Evidence: shared relation-component, self-loop, parallel-lane, and summary tests in
  `tests/er_model.rs`

- `er/classic_order_example.txt`
- `er/dash_dot_variants.txt`
- `er/dense_five_entities.txt`
- `er/duplicate_relationships.txt`
- `er/mixed_identifying.txt`
- `er/multiple_self_relationships.txt`
- `er/non_identifying.txt`
- `er/self_relationship.txt`
- `er/single_relationship.txt`
- `er-ascii/classic_order_example.txt`
- `er-ascii/duplicate_relationships.txt`
- `er-ascii/non_identifying.txt`
- `er-ascii/self_relationship.txt`
- `er-ascii/single_relationship.txt`

## ER Directives And Omission Policy

- Classification: `mermaid_valid`
- Admission: `semantic_probe`
- Semantic feature: accessibility metadata, comments, style, and direction are either consumed by
  the typed model or intentionally classified instead of leaking parser bookkeeping
- Evidence: ER parser tests, capability limits, and `tests/er_model.rs`

- `er/acc_title_descr_skipped.txt`
- `er/comments_skipped.txt`
- `er/style_and_direction_skipped.txt`

## Update Rule

When the moving reference changes, add only the new delta and record its new commit. A fixture may
move from `discovery_only` to `semantic_probe` only when a tracked parser-backed test owns its
semantic feature. Do not copy or rewrite reference output merely to make the manifest green.
