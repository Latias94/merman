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
- Verification so far: `cargo nextest run -p merman-core error::tests editor::tests --no-fail-fast`; `cargo check -p merman-core -p merman-analysis --tests`; `cargo check -p merman-core -p merman-analysis -p merman-editor-core --tests`; `cargo test -p merman-analysis --lib deduplicates_matching_recovery_diagnostic -- --nocapture`; `cargo test -p merman-editor-core --test diagnostics -- --nocapture`; `cargo test -p merman-lsp --test diagnostics -- --nocapture`; `cargo fmt --all --check`; `git diff --check`.

# Next Action

Migrate XY Chart, Gantt, GitGraph, Timeline, and C4 handwritten parser errors to span-aware diagnostics where local offsets are available, while leaving remaining imprecision as visible named fallback constructors.

# Citations

- [Plan](../../../plans/2026-06-30-004-refactor-editor-diagnostics-architecture-cleanup-plan.md)
