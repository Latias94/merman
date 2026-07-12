---
type: "Work Log"
title: "U3 cynefin parser/editor facts slice complete"
description: "Cynefin now has source-backed semantic parsing and editor facts while render admission remains explicitly deferred."
timestamp: 2026-07-09T12:29:31Z
producer_id: "codex-root"
related_plan: "docs/plans/2026-07-09-002-refactor-mermaid-11-16-parity-plan.md"
git_branch: "feat/mermaid-11-16-parity"
---

# Summary

U3 added a hand-written `cynefin` parser based on Mermaid 11.16's Langium grammar and DB behavior.
The parser accepts `cynefin-beta` and `cynefin-beta:`, fixed domain names, quoted domain items,
transitions with optional quoted labels, title/accessibility fields, quote-aware `%%` comments,
duplicate-domain replacement, and upstream self-loop filtering.

`cynefin` is now a parser-only family in the capability matrix: `has_semantic_parser=true` and
`has_render_parser=false`. Render-model parsing remains explicitly rejected with the existing
missing typed render parser diagnostic until the renderer/layout slice is implemented.

# LSP and Parser Notes

LALRPOP was not used. The upstream grammar is small and line-oriented, while the local editor
contract needs direct source spans, partial recovery, expected syntax, directive prefixes, and
useful diagnostics. Domain symbols are exposed as outline symbols with `NodeIdentifier` expected
syntax, avoiding user-defined-node completion pollution for a fixed-domain language.

Railroad remains the only new 11.16 family where a parser-generator spike may still be worth
evaluating. The acceptance bar is unchanged: span and recovery quality must be at least as useful
as the current hand-written parser surfaces.

# Verification

- `cargo fmt`
- `cargo nextest run -p merman-core cynefin registry detect --no-fail-fast` passed: 47/47.
- `cargo nextest run -p xtask admission --no-fail-fast` passed: 11/11.
- `cargo nextest run -p merman-bindings-core diagram_family_capabilities --no-fail-fast` passed:
  1/1.
- `cargo run -p xtask -- check-alignment` passed.
- `git diff --check` passed.

# Next

The next new-family slice is railroad parser research/implementation. Keep `cynefin` out of render
admission until the fixed layout, wavy boundary path generation, item badge measurement strategy,
and SVG fixtures are ported or explicitly documented as bounded residuals.
