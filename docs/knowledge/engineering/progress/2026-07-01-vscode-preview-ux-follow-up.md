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
- Preview diagnostics now collect only Merman-sourced VS Code diagnostics, so Markdown linting and
  spelling extensions do not pollute the Mermaid preview summary.
- Same-source render failures now mark the visible diagram as stale and label it as the last
  successful preview instead of silently showing old output as if it were current.

## Subagent Findings

- `preview_state`: the highest-risk low-cost issues were wrong-source rendering after explicit
  target opens and empty preview lock dead-end; both were fixed in this slice.
- `lsp_ux`: preview render failures and LSP diagnostics are still separate error sources. The
  diagnostic-source pollution finding was fixed by filtering preview diagnostics to Merman.
- `vscode_parity`: true parity with VS Code Markdown Preview requires a larger
  `PreviewManager + PreviewInstance` model with separate dynamic/locked previews, show-source,
  refresh, and scoped preview commands.

## Verified

- `npm run check` from `tools/vscode-extension`
- `npm test` from `tools/vscode-extension`
- `cargo nextest run -p merman-core kanban_recovered_editor_fact_diagnostics_are_english`
- `git diff --check`

## Remaining UX Work

- Consider whether Copy SVG should be disabled or retitled while the preview is stale, because it
  still copies the previous successful DOM while Export uses the current source.
- Refactor single global preview controller into preview manager/instance ownership before adding
  multiple locked preview panels, show-source, refresh, and command-palette scoping.
