---
type: "Work Log"
title: "U3 railroad parser/editor facts slice complete"
description: "Railroad IR, EBNF, ABNF, and PEG dialects now have source-backed semantic parsing and editor facts."
timestamp: 2026-07-09T12:45:04Z
producer_id: "codex-root"
related_plan: "docs/plans/2026-07-09-002-refactor-mermaid-11-16-parity-plan.md"
git_branch: "feat/mermaid-11-16-parity"
---

# Summary

U3 added hand-written parsers for Mermaid 11.16's railroad family: `railroad-beta`,
`railroad-ebnf-beta`, `railroad-abnf-beta`, and `railroad-peg-beta`. All four dialects map into the
same AST JSON model used by upstream render transforms: `terminal`, `nonterminal`, `sequence`,
`choice`, `optional`, `repetition`, and `special`.

The IR parser accepts upstream function forms and collapses single-element `sequence` and `choice`.
The EBNF parser handles choice, sequence, grouping, optionals, repetitions, postfixes, exceptions,
and special sequences. The ABNF parser handles alternation, concatenation, repeats, exact repeats,
numeric values, quoted strings, rule names, and optional groups. The PEG parser handles ordered
choice, sequence, lookahead prefixes, suffix operators, grouped expressions, identifiers, literals,
and the any-character dot.

All railroad detector ids are now parser-only in the capability matrix: `has_semantic_parser=true`
and `has_render_parser=false`. SVG admission remains closed until the renderer/layout path and
fixtures are ported.

# LSP and Parser Notes

LALRPOP was not used. The shared tokenizer plus recursive parsers keep source spans on rules and AST
nodes, allowing editor facts to expose rule symbols, nonterminal references, string/special payloads,
directive prefixes, and expected syntax.

For parse errors, the editor path falls back to a lossy token scan so partially written diagrams can
still expose rule names and literal payloads. This is the behavior to preserve if future refactors
try a parser generator.

# Verification

- `cargo fmt`
- `cargo nextest run -p merman-core railroad registry detect --no-fail-fast` passed: 47/47.
- `cargo nextest run -p xtask admission --no-fail-fast` passed: 11/11.
- `cargo nextest run -p merman-bindings-core diagram_family_capabilities --no-fail-fast` passed:
  1/1.
- `cargo run -p xtask -- check-alignment` passed.
- `git diff --check` passed.

# Next

Renderer work should stay deferred until the railroad renderer/layout path, style config, semantic
snapshots, and SVG fixtures are ported for representative examples across all four dialects.
