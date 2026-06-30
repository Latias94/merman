---
type: Current State
status: active
---

# Current State

- Goal: Implement `docs/plans/2026-06-30-004-refactor-editor-diagnostics-architecture-cleanup-plan.md` to collapse temporary editor diagnostic ownership layers and finish the preview UX cleanup.
- Branch: `feat/editor-core-language-intelligence`.
- Last verified: U3 first parser-span slice committed as `298433194 refactor(core): thread spans through handwritten parsers`; `cargo fmt --all --check`, `cargo test -p merman-core --no-run`, `cargo test -p merman-core --lib gitgraph_unknown_command_reports_exact_command_span -- --nocapture`, `cargo test -p merman-core --lib xychart_invalid_plot_number_reports_exact_token_span -- --nocapture`, `cargo test -p merman-core --lib xychart_comment_after_plot_does_not_merge_next_statement -- --nocapture`, and `git diff --check` passed.
- Done: U1 core parse diagnostic API cleanup, U2 analysis-owned recovery/duplicate policy, and U3 first wave for XY Chart plot number spans, Gantt weekday/weekend spans, and GitGraph unknown-command spans.
- In progress: U3 remaining handwritten parser coverage and explicit fallback ledger, especially Timeline/C4/Architecture/Kanban capability boundaries.
- Blocked: none.
- Next action: inspect Timeline, C4, Architecture, and Kanban parser error sites; migrate only parser-local deterministic spans and document remaining named fallback cases without message scraping.

# Citations

- [Editor diagnostics architecture cleanup plan](../../plans/2026-06-30-004-refactor-editor-diagnostics-architecture-cleanup-plan.md)
