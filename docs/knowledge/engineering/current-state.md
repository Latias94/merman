---
type: Current State
status: complete
---

# Current State

- Goal: Finish the recovered VS Code preview UX follow-up after completing `docs/plans/2026-06-30-004-refactor-editor-diagnostics-architecture-cleanup-plan.md`.
- Branch: `feat/editor-core-language-intelligence`.
- Last verified: `npm run check`, `npm test`, `cargo nextest run -p merman-core kanban_recovered_editor_fact_diagnostics_are_english`, and `git diff --check`.
- Done: editor diagnostics cleanup plan is complete; the preview UX follow-up hardens preview lock/follow, explicit target opening, cross-source render failure clearing, and kanban English recovery diagnostics.
- In progress: none.
- Blocked: none.
- Next action: continue separate UX research around multi-preview manager parity, stale render labeling, and diagnostic-source filtering.

# Citations

- [Editor diagnostics architecture cleanup plan](../../plans/2026-06-30-004-refactor-editor-diagnostics-architecture-cleanup-plan.md)
- [VS Code Preview UX Follow-up](progress/2026-07-01-vscode-preview-ux-follow-up.md)
