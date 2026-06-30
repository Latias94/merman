---
type: Current State
status: active
---

# Current State

- Goal: Implement `docs/plans/2026-06-30-004-refactor-editor-diagnostics-architecture-cleanup-plan.md` to collapse temporary editor diagnostic ownership layers and finish the preview UX cleanup.
- Branch: `feat/editor-core-language-intelligence`.
- Last verified: U5 preview UX slice passed `npm run check`, `npm test -- --test-reporter=spec` from `tools/vscode-extension`, and `git diff --check`.
- Done: U1 core parse diagnostic API cleanup, U2 analysis-owned recovery/duplicate policy, U3 parser spans/fallback ledger, U4 LSP document-pull/push/workspace/code-action boundary cleanup, and U5 preview/source-action UX polish.
- In progress: U6 redundant compatibility path cleanup.
- Blocked: none.
- Next action: search for legacy parse diagnostics, projection dedup, pin terminology, unadvertised workspace diagnostic leftovers, and obsolete preview diagnostic scaffolding; delete remaining compatibility paths with tests.

# Citations

- [Editor diagnostics architecture cleanup plan](../../plans/2026-06-30-004-refactor-editor-diagnostics-architecture-cleanup-plan.md)
