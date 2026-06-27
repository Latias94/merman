---
type: Work Progress
status: active
related_plan: docs/plans/2026-06-24-003-refactor-mature-mermaid-lsp-roadmap-plan.md
git_branch: feat/diagnostics-analysis-contract
---

# Summary

Flowchart `direction` values now participate in parser-backed completion instead of relying only on
the `direction` keyword prefix.

# Details

- `merman-core` now emits `EditorExpectedSyntaxKind::DirectionValue` for flowchart `direction`
  statements and captures the value span from the lexer token.
- `merman-analysis` maps that syntax kind into `FenceExpectedSyntaxKind::Direction` so cursor
  contexts inside an existing `direction LR` value keep direction completions active.
- `merman-lsp` now offers direction completion items at parser-backed direction value spans, not
  only when the user has typed the `direction` keyword prefix.

# Verification

- `cargo test -p merman-core --lib parse_flowchart_editor_facts_emit_direction_value_expected_syntax -- --nocapture`
- `cargo test -p merman-analysis --lib cursor_context_uses_parser_expected_direction_value_to_override_generic_completion -- --nocapture`
- `cargo test -p merman-lsp --test completion completion_uses_flowchart_parser_expected_direction_value_context -- --nocapture`
- `cargo fmt --all`
- `cargo test -p merman-core --lib flowchart -- --nocapture`
- `cargo test -p merman-analysis --lib editor -- --nocapture`
- `cargo test -p merman-lsp --test completion -- --nocapture`
- `cargo fmt --all --check`
- `git diff --check`
