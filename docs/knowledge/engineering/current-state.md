---
type: Current State
status: active
---

# Current State

- Goal: VS Code preview architecture is ready for the next parity slice after completing the
  no-behavior-change `PreviewInstance` extraction.
- Branch: `feat/editor-core-language-intelligence`.
- Last verified: `npm run check`, `npm test -- --test-reporter=spec` (87 tests), and
  `git diff --check`.
- Done: editor diagnostics cleanup is complete; the preview UX follow-up hardens lock/follow,
  explicit target opening, cross-source render failure clearing, Merman-only preview diagnostic
  summaries, stale same-source render failure labeling, stale output-action blocking, and the
  single-preview manager plus per-panel `PreviewInstance` ownership boundary.
- In progress: none.
- Blocked: none.
- Next action: run optional VS Code extension-host smoke for preview lifecycle/output actions before
  shipping, then plan true multi-preview parity work on top of the manager/instance boundary.

# Citations

- [Editor diagnostics architecture cleanup plan](../../plans/2026-06-30-004-refactor-editor-diagnostics-architecture-cleanup-plan.md)
- [VS Code Preview UX Follow-up](progress/2026-07-01-vscode-preview-ux-follow-up.md)
- [VS Code PreviewInstance Extraction](progress/2026-07-01-vscode-preview-instance-extraction.md)
