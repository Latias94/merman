---
type: Work Progress
status: completed
related_plan: ../../../plans/2026-06-30-001-refactor-vscode-preview-lifecycle-plan.md
git_branch: feat/editor-core-language-intelligence
git_commit: 850489aa8
verified_by: npm test; npm run check; npm run package
---

# VS Code Preview Lifecycle Refactor

- Date: 2026-06-30
- Branch: `feat/editor-core-language-intelligence`
- Goal: refactor the VS Code preview lifecycle so cursor movement, diagnostics updates, and
  source-list changes do not reset the rendered preview.

## Result

Implemented the plan and follow-up hardening as seven commits:

- `c22080782 docs(vscode): plan preview lifecycle refactor`
- `d240a1f78 refactor(vscode): stabilize preview lifecycle`
- `ea7196c50 docs(vscode): document preview lifecycle behavior`
- `e0c19476f fix(vscode): scope preview viewport state by source`
- `ca439818d refactor(vscode): deepen preview lifecycle modules`
- `d7ec55817 fix(vscode): abort superseded preview renders`
- `850489aa8 fix(vscode): guard preview svg injection`

The preview now creates a stable webview shell and updates it with typed messages instead of using
ordinary `panel.webview.html` rewrites as the transport. Cursor movement inside the same resolved
source produces `selectionChanged` only; diagnostics-only changes produce `diagnosticsUpdated` only;
source text or render-setting changes request a render through the stale-safe queue.

The webview keeps the previous SVG visible during render start and render failure, persists
pan/zoom/background with `getState()/setState()`, invalidates viewport state when the document/source
identity changes, and uses SVG dimension changes from `viewBox` for vector-aware zoom instead of
whole-canvas CSS scale.

Follow-up hardening added a Node `vm` webview behavior harness for the real `media/preview.js`,
split `PreviewSession` and `PreviewWebviewClient` out of the controller, aborts superseded preview
render child processes through `AbortSignal`, and rejects unsafe preview SVG before posting it to the
webview.

## Reference Repos

The relevant reference implementations were cloned under `repo-ref/`:

- `repo-ref/vscode-mermaid-preview`
- `repo-ref/vscode-markdown-preview-enhanced`
- `repo-ref/vscode-markdown-reference`
- `repo-ref/vscode-markdown-mermaid`
- `repo-ref/vscode-vega-viewer`

The shared implementation pattern is a stable webview shell plus `postMessage` updates, with
selection/scroll/viewport state preserved independently from content rendering.

## Key Files

- `tools/vscode-extension/src/preview-model.ts`
- `tools/vscode-extension/src/preview-policy.ts`
- `tools/vscode-extension/src/preview-messages.ts`
- `tools/vscode-extension/src/preview-render.ts`
- `tools/vscode-extension/src/preview-session.ts`
- `tools/vscode-extension/src/preview-webview-client.ts`
- `tools/vscode-extension/src/preview-svg-safety.ts`
- `tools/vscode-extension/src/render-process.ts`
- `tools/vscode-extension/src/preview.ts`
- `tools/vscode-extension/media/preview.js`
- `tools/vscode-extension/media/preview.css`
- `tools/vscode-extension/src/test/preview-policy.test.ts`
- `tools/vscode-extension/src/test/preview-render.test.ts`
- `tools/vscode-extension/src/test/preview-messages.test.ts`
- `tools/vscode-extension/src/test/preview-webview.test.ts`
- `tools/vscode-extension/src/test/preview-svg-safety.test.ts`
- `tools/vscode-extension/src/test/render-process.test.ts`
- `tools/vscode-extension/src/test/preview.test.ts`
- `tools/vscode-extension/README.md`

## Verification

Automated verification passed from `tools/vscode-extension`:

- `npm run check`
- `npm test -- --test-reporter=spec`
- `npm run package`

`npm run package` produced
`tools/vscode-extension/merman-vscode-0.1.0.vsix`.

Manual VS Code extension-host smoke was not run in this headless session; use the checklist in the
plan if interactive validation is needed.
