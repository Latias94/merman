---
type: Current State
status: active
---

# Current State

- Goal: VS Code preview multi-instance parity is implemented on top of the extracted
  `PreviewInstance` boundary.
- Branch: `feat/editor-core-language-intelligence`.
- Last verified: `npm run check`, focused
  `node --test dist/test/preview-manager.test.js --test-reporter=spec` (13 tests),
  `npm test -- --test-reporter=spec` (100 tests), `npm run package`, and `git diff --check`.
- Done: editor diagnostics cleanup is complete; the preview UX follow-up hardens lock/follow,
  explicit target opening, cross-source render failure clearing, Merman-only preview diagnostic
  summaries, stale same-source render failure labeling, stale output-action blocking, and the
  single-preview manager plus per-panel `PreviewInstance` ownership boundary. The next preview
  parity slice adds a manager-owned instance collection, active preview tracking, one unlocked
  follow preview, multiple locked previews, manual refresh commands, toolbar Refresh/Source
  actions, and source-range reveal. Headless smoke-equivalent tests now cover follow preview
  close/reopen, webview-ready state replay, panel-local source reveal, and panel-local Copy/Export
  actions; VSIX packaging also passes.
- In progress: none.
- Blocked: none.
- Next action: no required follow-up for this slice. Optional real VS Code Extension Host/manual
  GUI smoke remains useful for visual confidence; the repo currently has no automated
  extension-host integration harness.

# Citations

- [Editor diagnostics architecture cleanup plan](../../plans/2026-06-30-004-refactor-editor-diagnostics-architecture-cleanup-plan.md)
- [VS Code Preview UX Follow-up](progress/2026-07-01-vscode-preview-ux-follow-up.md)
- [VS Code PreviewInstance Extraction](progress/2026-07-01-vscode-preview-instance-extraction.md)
- [VS Code Preview Multi-Instance Parity](progress/2026-07-01-vscode-preview-multi-instance-parity.md)
