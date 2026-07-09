---
type: Verification Evidence
title: Mermaid 11.16 TreeView SVG DOM order parity verification
timestamp: 2026-07-10T03:26:18+08:00
related_plan: docs/plans/2026-07-09-002-refactor-mermaid-11-16-parity-plan.md
git_branch: feat/mermaid-11-16-parity
git_commit: cca2ce09e562
tags: mermaid-11-16,treeview,svg-dom,verification
---

# Verification

Commands run after the TreeView DOM serialization fix:

- `cargo fmt --check` - passed.
- `cargo nextest run -p merman-render tree_view --no-fail-fast` - passed, 11 tests.
- `cargo run -p xtask -- compare-all-svgs --check-dom --dom-mode parity --dom-decimals 3` - passed.
- `git diff --check` - passed.

# Upstream Risk

GitHub API check for `https://github.com/mermaid-js/mermaid/issues/7954` confirmed issue 7954 is
open with labels `Type: Bug / Error` and `Status: Triage`. Its title is `Arrows between subgraphs
are broken since 11.16.0`.

# Residual Gate Notes

- `cargo run -p xtask -- verify-generated` passed earlier in this plan-level audit.
- `cargo run -p xtask -- verify-default-config` passed earlier in this plan-level audit.
- `cargo run -p xtask -- check-upstream-svgs --diagram all --check-dom --dom-mode parity --dom-decimals 3`
  was unblocked by installing Puppeteer `chrome-headless-shell` 131.0.6778.204, but the full all-family
  run timed out after one hour before a final verdict. Completed families did not surface a reported
  mismatch before timeout.
- `cargo run -p xtask -- compare-all-svgs --check-dom --dom-mode parity-root --dom-decimals 3` still
  reports broad root viewport residuals. Do not hide those by expanding accepted residual policy
  without a source-backed root sizing decision.
