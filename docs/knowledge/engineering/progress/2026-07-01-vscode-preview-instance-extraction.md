---
type: Work Progress
status: complete
source_session: 019f1b8e-45ea-70b1-abd1-33e080b748ec
related_plan: docs/plans/2026-07-01-001-refactor-vscode-preview-instance-extraction-plan.md
git_branch: feat/editor-core-language-intelligence
tags:
  - vscode-preview
  - preview-instance
  - ce-work
  - refactor
---

# VS Code PreviewInstance Extraction

## Summary

Implemented the no-user-visible-behavior-change `PreviewInstance` ownership extraction from
`docs/plans/2026-07-01-001-refactor-vscode-preview-instance-extraction-plan.md`.

The VS Code preview still exposes exactly one preview panel. `preview.ts` now acts as a
single-instance manager/router for commands and global editor/workspace events, while
`preview-instance.ts` owns panel-local state and behavior: the `WebviewPanel`, `PreviewSession`,
`PreviewRenderQueue`, `PreviewWebviewClient`, render debounce timer, webview messages, panel-origin
export/copy, and instance disposal cleanup.

## Implemented

- Added the implementation plan as commit `665460560`.
- Added characterization coverage for targeted Markdown source opening and follow-mode restoration
  as commit `b2e66f458`.
- Extracted `PreviewInstance` and reduced `preview.ts` to manager routing as commit `0ace6869a`.
- Tidied the instance lifecycle as commit `b5bbc1894`: removed the panel guard helper, shared the
  empty-lock warning string, aborted pending render work on dispose, and guarded dispose-during-open.
- Added a narrow fake-`vscode` manager behavior harness as commit `0831d41c0`, covering command
  registration, single-panel reuse, explicit target opening with `preserveFocus`, and no-instance
  lock warning.

## Review Findings

- Simplification review found and fixed two lifecycle issues before final review: closing or
  disposing an instance now cancels pending render work, and `open()` rechecks disposal after
  `openResource()`.
- Code review found no confirmed correctness, reliability, maintainability, project-standards, or
  adversarial defects after those fixes.
- The main remaining test gap is extension-host-level smoke coverage for full VS Code panel
  lifecycle behavior, especially webview reload/replay, panel close/reopen, panel-origin Copy SVG,
  and Export SVG/PNG after retargeting or stale render recovery.

## Verified

- `npm run check` from `tools/vscode-extension`
- `npm test -- --test-reporter=spec` from `tools/vscode-extension` (87 tests)
- `git diff --check` from the repository root

Rust verification was not required because the implementation touched only VS Code extension
TypeScript files, tests, plan documentation, and this engineering memory.

## Remaining Work

- Manual VS Code extension-host smoke is still useful before shipping: open from active `.mmd` or
  Markdown fence, open from source action while another editor is active, lock/unlock, select
  another source, same-source failure, different-source failure, webview reload/reveal, Copy SVG,
  and Export SVG/PNG.
- True multi-preview behavior remains deferred. Future work can now build on the explicit manager
  plus per-instance boundary.
