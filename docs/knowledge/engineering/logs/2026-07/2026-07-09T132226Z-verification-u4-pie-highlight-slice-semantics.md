---
type: "Work Log"
title: "U4 pie highlightSlice semantics complete"
description: "Pie now consumes Mermaid 11.16 highlightSlice config in SVG class and CSS output."
timestamp: 2026-07-09T13:22:26Z
producer_id: "codex-root"
related_plan: "docs/plans/2026-07-09-002-refactor-mermaid-11-16-parity-plan.md"
git_branch: "feat/mermaid-11-16-parity"
---

# Summary

U4 started with a small existing-family delta in Pie. The renderer already consumed 11.16
`donutHole` and `legendPosition`; the stale 11.15 behavior was `highlightSlice`, where tests
asserted that highlight classes and CSS should not be emitted. The slice now mirrors Mermaid
11.16: `highlightSlice: "A"` marks matching slices with `pieCircle highlighted`, while
`highlightSlice: "hover"` marks slices with `pieCircle highlightedOnHover` and emits the matching
style rules.

Active Pie coverage docs and touched test names now refer to Mermaid 11.16 rather than treating
highlighting as an unsupported 11.15 residual.

# Verification

- `cargo fmt`
- `cargo nextest run -p merman-core pie --no-fail-fast` passed: 9/9.
- `cargo nextest run -p merman-render pie --no-fail-fast` passed: 22/22.
- `cargo run -p xtask -- check-alignment` passed.

# Next

Continue U4 with ER/State using the subagent finding: ER should keep LALRPOP for the grammar
composition while adding lexer support for backticks, comma-containing types, and nullable `?`;
State should add the multi-word composite state error in the hand-written lexer.
