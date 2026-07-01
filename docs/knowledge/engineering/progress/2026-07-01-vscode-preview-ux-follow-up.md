---
type: Work Progress
status: active
source_session: 019f1389-55d7-7b11-974a-c732f43a4473
git_branch: feat/editor-core-language-intelligence
tags:
  - vscode-preview
  - ux
  - session-recovery
  - subagent-findings
---

# VS Code Preview UX Follow-up

## Summary

Recovered the interrupted preview UX follow-up from session
`019f1389-55d7-7b11-974a-c732f43a4473` after the editor diagnostics cleanup goal had already been
completed. The active follow-up is a smaller preview UX hardening slice, not a reopen of the
completed diagnostics plan.

## Implemented

- Lock/follow state now travels through preview session snapshots, webview messages, update policy,
  and the webview toolbar.
- Different-source render failures clear the old canvas instead of leaving a successful SVG from a
  previous file or Markdown fence.
- Explicit open-preview targets are preferred for one snapshot even when `preserveFocus: true`
  leaves another Mermaid editor active, then normal follow mode resumes.
- Empty previews cannot be locked: the webview lock button starts disabled, empty state resets it
  to Follow, and the controller warns instead of locking without a snapshot.
- The existing kanban editor-fact recovery diagnostics were converted from Chinese to English and
  covered by a focused core test.

## Subagent Findings

- `preview_state`: the highest-risk low-cost issues were wrong-source rendering after explicit
  target opens and empty preview lock dead-end; both were fixed in this slice.
- `lsp_ux`: preview render failures and LSP diagnostics are still separate error sources, and
  preview diagnostics currently collect all VS Code diagnostics in range rather than only Merman
  diagnostics.
- `vscode_parity`: true parity with VS Code Markdown Preview requires a larger
  `PreviewManager + PreviewInstance` model with separate dynamic/locked previews, show-source,
  refresh, and scoped preview commands.

## Verified

- `npm run check` from `tools/vscode-extension`
- `npm test` from `tools/vscode-extension`
- `cargo nextest run -p merman-core kanban_recovered_editor_fact_diagnostics_are_english`
- `git diff --check`

## Remaining UX Work

- Add an explicit stale/last-successful render state for same-source failures, especially because
  Copy SVG may still copy the previous successful DOM while Export uses the current source.
- Filter or group preview diagnostics so external Markdown diagnostics do not look like Mermaid
  render/parse failures.
- Refactor single global preview controller into preview manager/instance ownership before adding
  multiple locked preview panels, show-source, refresh, and command-palette scoping.
