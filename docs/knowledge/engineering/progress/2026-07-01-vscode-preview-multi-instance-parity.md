---
type: Work Progress
status: complete
related_plan: docs/plans/2026-07-01-002-feat-vscode-preview-multi-instance-parity-plan.md
git_branch: feat/editor-core-language-intelligence
tags:
  - vscode-preview
  - multi-preview
  - preview-instance
  - ce-work
  - feat
---

# VS Code Preview Multi-Instance Parity

## Summary

Implemented the next VS Code preview parity slice from
`docs/plans/2026-07-01-002-feat-vscode-preview-multi-instance-parity-plan.md`.

`MermanPreviewManager` now manages a preview instance collection with an active preview and one
unlocked follow preview. Locked previews stay bound to their source, and opening another source
creates or reuses a follow preview instead of retargeting the locked panel.

## Implemented

- Added the multi-instance parity plan as commit `c48c6f296`.
- Replaced the single `currentInstance` manager model with `instances`, `activeInstance`, and
  `followInstance`.
- Added active panel tracking through `PreviewInstance` view-state callbacks.
- Kept lock-state routing consistent for both command-palette toggles and webview toolbar lock
  messages.
- Added manual refresh behavior: command-palette refresh forces all managed previews, while the
  webview Refresh button forces only the sending instance.
- Added `merman.refreshPreview` and `merman.showPreviewSource` command contributions.
- Added preview toolbar Refresh and Source buttons through typed webview messages.
- Added show-source behavior that opens the preview's source document and selects the current
  Mermaid source range.
- Extended manager, policy, message, HTML shell, and webview tests for multi-preview routing,
  forced refresh, active-preview source reveal, and panel-origin commands.
- Added headless smoke-equivalent manager tests for follow preview close/reopen, webview-ready
  state replay, panel-origin source reveal, and panel-local Copy SVG plus Export SVG/PNG routing.

## Verified

- `npm run check` from `tools/vscode-extension`
- focused `node --test dist/test/preview-manager.test.js --test-reporter=spec` from
  `tools/vscode-extension` (13 tests)
- `npm test -- --test-reporter=spec` from `tools/vscode-extension` (100 tests)
- `npm run package` from `tools/vscode-extension`
- `git diff --check` from the repository root

Rust verification was not required because this slice touched only VS Code extension TypeScript,
webview media, package metadata, tests, plan documentation, and engineering memory.

## Remaining Work

- Optional real VS Code Extension Host/manual GUI smoke is still useful for visual confidence:
  open two previews by locking one source and opening another, toggle lock from both command
  palette and toolbar, run Refresh globally and from one panel, run Show Preview Source from
  command palette and toolbar, close and reopen panels, reload the webview, and verify Copy SVG
  plus Export SVG/PNG remain panel-local. No automated extension-host integration harness exists
  in the current repo; the headless manager/webview tests and package smoke cover the same routing
  and packaging risks that can be exercised without a GUI session.
- Full Markdown Preview parity remains deferred: custom editor restoration, serializer support,
  scroll synchronization, view-column matching, preview links, image copy, security selector, and
  plugin reload behavior are intentionally out of scope.
