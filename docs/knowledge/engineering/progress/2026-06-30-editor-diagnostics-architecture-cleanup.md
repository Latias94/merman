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
- Active unit: U7, diagnostics documentation and validation matrix.
- Completed in U3 first wave: `298433194 refactor(core): thread spans through handwritten parsers` added explicit exact/insertion/fallback-at parse constructors, threaded source offsets through XY Chart plot number parsing, Gantt weekday/weekend directives, and GitGraph body commands, and deleted the replaced XY Chart unspanned plot/comment statement path.
- Completed in U3 second wave: `3345e9cd3 refactor(core): span timeline and c4 parser errors` threaded line offsets through Timeline and C4 render parsers, added insertion-point diagnostics for Timeline missing event whitespace and C4 missing relation/style arguments, and deleted replaced unspanned C4 macro argument parsing helpers.
- U3 fallback boundary: Architecture and Kanban still use named fallback diagnostics for render-time semantic validation and render parser paths that do not preserve byte offsets. Their editor facts remain parser-backed, but exact render error underlines should come from a future shared spanned render parser rather than message scraping.
- Completed in U4: removed the unadvertised `workspace/diagnostic` handler while keeping `workspace_diagnostics: false`, added a smoke regression that unadvertised workspace diagnostics return JSON-RPC method-not-found, and added a pull-client close regression proving closed documents return a stable empty report and then unchanged empty report.
- Completed in U5: renamed the source CodeLens menu from `More...` to `Export / Copy`, made the preview toolbar responsive around the source picker, kept SVG copy/export controls visible in SVG mode, and added webview regressions for drag pan, wheel zoom, Fit, 1:1 reset, sourcebar layout state, and source-sized SVG copy.
- Completed in U6: removed the editor-core numeric/string diagnostic-code compatibility enum so projection emits analysis rule ids as string Problems codes only, while preserving numeric status metadata in diagnostic `data` for code actions and clients that need it.
- Completed in U6: replaced analysis recovery merge's remaining `related.message.contains(...)` fallback detection with a private structured `ParseDiagnosticLocation` side channel from core parse diagnostic projection, backed by `fallback_recovery_merge_uses_structured_location_metadata`.
- Completed in U6: renamed VS Code source-action internals from the old `More` abstraction to `Export / Copy`, and removed the stale `togglePin` preview message test fixture.
- Verification so far: `cargo nextest run -p merman-core error::tests editor::tests --no-fail-fast`; `cargo check -p merman-core -p merman-analysis --tests`; `cargo check -p merman-core -p merman-analysis -p merman-editor-core --tests`; `cargo test -p merman-analysis --lib deduplicates_matching_recovery_diagnostic -- --nocapture`; `cargo test -p merman-editor-core --test diagnostics -- --nocapture`; `cargo test -p merman-lsp --test diagnostics -- --nocapture`; `cargo fmt --all --check`; `cargo test -p merman-core --no-run`; `cargo test -p merman-core --lib gitgraph_unknown_command_reports_exact_command_span -- --nocapture`; `cargo test -p merman-core --lib xychart_invalid_plot_number_reports_exact_token_span -- --nocapture`; `cargo test -p merman-core --lib xychart_comment_after_plot_does_not_merge_next_statement -- --nocapture`; `git diff --check`.
- Verification note: the new Gantt assertion compiles under `cargo test -p merman-core --no-run`, but repeated focused `cargo test -p merman-core --lib gantt_weekday_rejects_unknown_values` invocations hit a local PTY/tool EOF issue after the test binary launched; no cargo/test process remained in `ps` during those hangs.
- U6 verification: `cargo check -p merman-analysis --tests`; `cargo test -p merman-analysis --lib fallback_recovery_merge_uses_structured_location_metadata -- --nocapture`; `cargo test -p merman-analysis --lib parse_failure_deduplicates -- --nocapture`; `cargo test -p merman-editor-core --test diagnostics`; `cargo test -p merman-lsp --test diagnostics` passed once after the U6 code changes; a later repeat after formatting compiled and launched but was killed by the OS with `SIGKILL`, so `cargo check -p merman-lsp --tests` was used as the final compile gate; `npm run check`; `npm test -- --test-reporter=spec`; `cargo fmt --all --check`; `git diff --check`.

# Next Action

Update U7 documentation across LSP capability/protocol docs, ADR 0070, crate READMEs, and the VS Code README so current docs match the implemented ownership model and cleanup results.

# Citations

- [Plan](../../../plans/2026-06-30-004-refactor-editor-diagnostics-architecture-cleanup-plan.md)
