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
- Active unit: U1, replacing the dual core parse-error surface with one structured `ParseDiagnostic` path and explicit named fallbacks for remaining imprecise parser errors.
- Verification pending: focused core diagnostics tests first, then analysis/editor/LSP and VS Code gates as later units land.

# Next Action

Migrate `Error::DiagramParse` construction sites to structured diagnostics, remove `DiagramParseDiagnostic`, minimize parse diagnostic public mutation, and run focused `merman-core` checks before the first implementation commit.

# Citations

- [Plan](../../../plans/2026-06-30-004-refactor-editor-diagnostics-architecture-cleanup-plan.md)
