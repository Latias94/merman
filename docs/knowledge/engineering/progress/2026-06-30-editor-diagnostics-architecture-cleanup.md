---
type: Work Progress
status: active
related_plan: docs/plans/2026-06-30-004-refactor-editor-diagnostics-architecture-cleanup-plan.md
git_branch: feat/editor-core-language-intelligence
timestamp: 2026-06-30
---

# Editor Diagnostics Architecture Cleanup

- Goal: execute the cleanup plan that makes `merman-analysis` the canonical diagnostic owner, tightens core parse diagnostics, removes redundant projection policy, and keeps the VS Code preview focused on common user actions.
- Baseline: `49359a043 docs(plan): add editor diagnostics cleanup plan`.
- Completed: U1 collapsed core parse errors onto one structured `ParseDiagnostic` path and converted legacy parser errors through explicit fallback construction.
- Completed: U2 moved parser/recovery duplicate handling into `merman-analysis` and removed editor-core semantic deduplication plus recovered-message rewriting.
- Active unit: U3, migrating high-value handwritten parser families to span-aware diagnostics where parser-local offsets are available.
- Completed in U3 first wave: `298433194 refactor(core): thread spans through handwritten parsers` added explicit exact/insertion/fallback-at parse constructors, threaded source offsets through XY Chart plot number parsing, Gantt weekday/weekend directives, and GitGraph body commands, and deleted the replaced XY Chart unspanned plot/comment statement path.
- Completed in U3 second wave: `3345e9cd3 refactor(core): span timeline and c4 parser errors` threaded line offsets through Timeline and C4 render parsers, added insertion-point diagnostics for Timeline missing event whitespace and C4 missing relation/style arguments, and deleted replaced unspanned C4 macro argument parsing helpers.
- U3 fallback boundary: Architecture and Kanban still use named fallback diagnostics for render-time semantic validation and render parser paths that do not preserve byte offsets. Their editor facts remain parser-backed, but exact render error underlines should come from a future shared spanned render parser rather than message scraping.
- Verification so far: `cargo nextest run -p merman-core error::tests editor::tests --no-fail-fast`; `cargo check -p merman-core -p merman-analysis --tests`; `cargo check -p merman-core -p merman-analysis -p merman-editor-core --tests`; `cargo test -p merman-analysis --lib deduplicates_matching_recovery_diagnostic -- --nocapture`; `cargo test -p merman-editor-core --test diagnostics -- --nocapture`; `cargo test -p merman-lsp --test diagnostics -- --nocapture`; `cargo fmt --all --check`; `cargo test -p merman-core --no-run`; `cargo test -p merman-core --lib gitgraph_unknown_command_reports_exact_command_span -- --nocapture`; `cargo test -p merman-core --lib xychart_invalid_plot_number_reports_exact_token_span -- --nocapture`; `cargo test -p merman-core --lib xychart_comment_after_plot_does_not_merge_next_statement -- --nocapture`; `git diff --check`.
- Verification note: the new Gantt assertion compiles under `cargo test -p merman-core --no-run`, but repeated focused `cargo test -p merman-core --lib gantt_weekday_rejects_unknown_values` invocations hit a local PTY/tool EOF issue after the test binary launched; no cargo/test process remained in `ps` during those hangs.

# Next Action

Finish verification and commit the U3 fallback-ledger documentation, then start U4 LSP diagnostic boundary work.

# Citations

- [Plan](../../../plans/2026-06-30-004-refactor-editor-diagnostics-architecture-cleanup-plan.md)
