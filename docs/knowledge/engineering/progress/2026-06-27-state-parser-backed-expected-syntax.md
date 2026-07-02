---
type: Work Progress
status: active
related_plan: docs/plans/2026-06-24-003-refactor-mature-mermaid-lsp-roadmap-plan.md
git_branch: feat/diagnostics-analysis-contract
---

# Summary

State diagram editor facts now carry parser-backed expected syntax for node identifiers,
id-lists, and payload spans. That lets completion treat state labels, class targets, style targets,
and click payloads as parser-controlled positions instead of relying only on line-prefix heuristics.

# Details

- `crates/merman-core/src/diagrams/state/parse.rs` now emits `EditorExpectedSyntax` for state
  entity ids, class/style id lists, display labels, descriptions, style payloads, and click
  payloads.
- `crates/merman-lsp/src/context.rs` now treats parser-controlled payload spans as non-directive
  completion zones, and state-specific completion coverage was added for payload and id-list
  contexts.
- `crates/merman-core/src/tests/state.rs` now asserts the expected-syntax spans for the state
  parser contract directly, so the new parser-backed completion behavior is guarded at the core
  layer instead of only at the protocol edge.

# Verification

- `cargo test -p merman-core state -- --nocapture`
- `cargo test -p merman-lsp context::tests::context_classifies_header_operator_and_directive_prefixes -- --nocapture`
- `cargo test -p merman-lsp context::tests::context_uses_state_parser_expected_syntax_for_payload_and_id_lists -- --nocapture`
- `cargo test -p merman-lsp --test completion completion_offers_directive_items_for_class_directive_variants -- --nocapture`
- `cargo fmt --all`
- `git diff --check`
