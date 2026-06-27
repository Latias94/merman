---
type: Work Progress
status: active
related_plan: docs/plans/2026-06-24-003-refactor-mature-mermaid-lsp-roadmap-plan.md
git_branch: feat/diagnostics-analysis-contract
---

# Summary

Parser-backed completion has expanded past payload-only spans. ER id lists now surface as
expected-syntax facts, and class diagrams now mark identifier positions as parser-backed
`NodeIdentifier` contexts. That makes `classDef`/`class`-style name positions completion-visible
through the shared analysis/LSP contract instead of relying on transport-local heuristics.

# Details

- `merman-core::EditorExpectedSyntaxKind` gained `IdList` and ER editor facts now emit it for
  parser-recognized id lists.
- `merman-analysis::FenceCursorContext` projects `IdList` into completion as a node-identifier
  context, so directive lines can switch back to real identifier completion when the parser proves
  the syntax.
- Class diagram editor facts now emit `NodeIdentifier` expected syntax for name positions, and
  `classDef` symbols were promoted from outline-only to completion-visible entity symbols so the LSP
  can actually surface those names as candidates.
- LSP completion regressions now cover both ER `classDef` id lists and class-diagram `classDef`
  name positions.

# Verification

- `cargo test -p merman-core --lib parse_er_editor_facts_record_expected_id_list_spans -- --nocapture`
- `cargo test -p merman-core --lib -j1 parse_class_editor_facts -- --nocapture`
- `cargo test -p merman-analysis --lib -j1 cursor_context_uses_parser_expected -- --nocapture`
- `cargo test -p merman-lsp --test completion -j1 completion_uses_er_parser_expected_id_list_context_for_class_def -- --nocapture`
- `cargo test -p merman-lsp --test completion -j1 completion_uses_class_parser_expected_node_identifier_context_for_class_def -- --nocapture`
- `cargo fmt --all --check`

