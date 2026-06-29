---
type: Work Progress
status: planned
related_plan: ../../../plans/2026-06-30-001-refactor-vscode-preview-lifecycle-plan.md
git_branch: feat/editor-core-language-intelligence
---

# VS Code Preview Lifecycle Refactor Plan

- Date: 2026-06-30
- Branch: `feat/editor-core-language-intelligence`
- Goal: prepare a fearless refactor of the VS Code preview lifecycle so cursor movement,
  diagnostics updates, and source-list changes do not reset the rendered preview.

## Current Finding

`tools/vscode-extension/src/preview.ts` currently funnels active-editor changes, selection changes,
document edits, diagnostics updates, pin/source/theme changes, and panel visibility changes into
`scheduleRefresh()`. `refresh()` then rewrites `panel.webview.html` before rendering and again after
render success or failure. This destroys the webview DOM and the `media/preview.js` state, which is
why moving the cursor can reset pan/zoom and the rendered preview.

The current zoom path still scales `.canvas` with CSS transform. The preview output is SVG, not PNG,
but Electron/Chromium can display a scaled composited layer that looks bitmap-like when enlarged.

## Reference Repos

The relevant reference implementations were cloned under `repo-ref/`:

- `repo-ref/vscode-mermaid-preview`
- `repo-ref/vscode-markdown-preview-enhanced`
- `repo-ref/vscode-markdown-reference`
- `repo-ref/vscode-markdown-mermaid`
- `repo-ref/vscode-vega-viewer`

The shared implementation pattern is a stable webview shell plus `postMessage` updates, with
selection/scroll/viewport state preserved independently from content rendering.

## Plan

The implementation-ready plan is
[`docs/plans/2026-06-30-001-refactor-vscode-preview-lifecycle-plan.md`](../../../plans/2026-06-30-001-refactor-vscode-preview-lifecycle-plan.md).
It starts with `U1. Extract preview snapshot and event policy`, then moves to typed webview
messages, a render queue, vector-aware zoom, incremental diagnostics/source-list updates, and
regression tests.

## Next Action

Start at U1 by adding pure `PreviewSnapshot` / `PreviewEvent` / `PreviewAction` policy tests. Do
not begin by rewriting `media/preview.js`; first lock the event semantics that prevent same-source
cursor movement and diagnostics-only changes from requesting a render.
