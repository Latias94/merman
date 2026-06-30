---
type: Current State
status: active
---

# Current State

- Goal: Implement `docs/plans/2026-06-30-004-refactor-editor-diagnostics-architecture-cleanup-plan.md` to collapse temporary editor diagnostic ownership layers and finish the preview UX cleanup.
- Branch: `feat/editor-core-language-intelligence`.
- Last verified: U3 second parser-span slice committed as `3345e9cd3 refactor(core): span timeline and c4 parser errors`; focused Timeline/C4 tests, `cargo check -p merman-core --tests`, `cargo fmt --all --check`, and `git diff --check` passed before the commit. The fallback ledger documentation is being verified next.
- Done: U1 core parse diagnostic API cleanup, U2 analysis-owned recovery/duplicate policy, U3 parser spans for XY Chart plot numbers, Gantt weekday/weekend values, GitGraph unknown commands, Timeline event separator insertion points, and C4 relation/style argument insertion points.
- In progress: finish U3 capability documentation for Architecture/Kanban named fallback boundaries, then move to U4 LSP pull/push/workspace/code-action behavior.
- Blocked: none.
- Next action: verify the U3 fallback-ledger documentation and start U4 LSP diagnostic boundary tests/code.

# Citations

- [Editor diagnostics architecture cleanup plan](../../plans/2026-06-30-004-refactor-editor-diagnostics-architecture-cleanup-plan.md)
