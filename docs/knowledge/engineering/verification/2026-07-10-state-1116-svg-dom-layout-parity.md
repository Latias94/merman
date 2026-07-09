---
type: Verification Evidence
title: State 11.16 SVG DOM and layout parity verification
timestamp: 2026-07-10T05:27:42+08:00
related_plan: docs/plans/2026-07-09-002-refactor-mermaid-11-16-parity-plan.md
git_branch: feat/mermaid-11-16-parity
tags: mermaid-11-16,state,svg-dom,layout,verification
---

# Verification

Commands run after implementing the State 11.16 SVG DOM/layout alignment:

- `cargo run -p xtask -- gen-upstream-svgs --diagram state --out fixtures\upstream-svgs` - passed.
- `cargo run -p xtask -- update-layout-snapshots --diagram state` - passed.
- `cargo run -p xtask -- compare-state-svgs --check-dom --dom-mode parity --dom-decimals 3` -
  passed.
- `cargo run -p xtask -- compare-all-svgs --check-dom --dom-mode parity --dom-decimals 3` -
  passed.
- `cargo run -p xtask -- verify-generated` - passed.
- `cargo run -p xtask -- verify-default-config` - passed.
- `cargo run -p xtask -- check-alignment` - passed.
- `cargo nextest run -p xtask admission --no-fail-fast` - passed, 11 tests.
- `cargo nextest run -p merman-core --no-fail-fast` with low build concurrency - passed, 855 tests.
- `cargo nextest run -p merman-render --no-fail-fast` with low build concurrency - passed, 701
  tests and 2 skipped.
- `cargo nextest run -p xtask --no-fail-fast` with low build concurrency - passed, 206 tests.

# Notes

The first full `merman-render` package run surfaced stale State layout goldens after the SVG DOM
work. Regenerating State layout snapshots against the 11.16 implementation resolved that gate.

The upstream SVG generation emitted Puppeteer `net::ERR_FILE_NOT_FOUND` warnings for fixture-local
asset loads, but it still completed and wrote the 11.16 State SVG baselines.

