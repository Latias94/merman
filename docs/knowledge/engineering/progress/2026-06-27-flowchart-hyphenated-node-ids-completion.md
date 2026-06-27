---
type: Work Progress
status: active
related_plan: docs/plans/2026-06-24-003-refactor-mature-mermaid-lsp-roadmap-plan.md
git_branch: feat/diagnostics-analysis-contract
---

# Summary

Flowchart completion no longer treats a single trailing hyphen as an operator-only context. That
change keeps valid hyphenated node ids such as `wi-fi` completion-visible through the parser-backed
semantic index instead of suppressing them behind a prefix heuristic.

# Details

- `merman-analysis::FenceCursorCompletionKind::Operator` now triggers only on clearer partial
  operator prefixes (`--` / `->`), not on a lone trailing `-`.
- `FenceCursorContext::node_text_edit_range` now keeps node completion edits available when the
  operator heuristic does not actually fire.
- `flowchart` parser facts already preserve hyphenated node ids, so this change lets completion
  use the parser-backed identifiers instead of hiding them.

# Verification

- `cargo test -p merman-core --lib parse_flowchart_editor_facts_preserve -- --nocapture`
- `cargo test -p merman-analysis --lib cursor_context_classifies_header_operator_directive_and_nodes -- --nocapture`
- `cargo test -p merman-lsp --test completion -- --nocapture`
- `cargo fmt --all --check`
- `git diff --check`

