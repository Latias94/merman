---
type: "Work Log"
title: "U3 swimlane parser/editor facts slice complete"
description: "Swimlane now reuses Flowchart semantics and editor facts while render admission remains explicitly deferred."
timestamp: 2026-07-09T12:15:06Z
producer_id: "codex-root"
related_plan: "docs/plans/2026-07-09-002-refactor-mermaid-11-16-parity-plan.md"
git_branch: "feat/mermaid-11-16-parity"
---

# Summary

U3 admitted `swimlane` into the semantic parser surface by preserving Mermaid 11.16's source shape:
`swimlane-beta` is a Flowchart grammar entry point with a `swimlane` default layout, not a forked
diagram language. The Flowchart lexer, LALRPOP grammar, parser registration, metadata pipeline,
JSON model path, and editor semantic facts now all accept `swimlane`.

Render admission intentionally remains closed. `swimlane` has `has_semantic_parser=true` and
`has_render_parser=false`, and render-model parsing returns a diagnostic about the missing typed
render parser rather than silently relying on JSON render fallback.

# LSP and Parser Notes

LSP-facing value depends on spans, recovery, partial facts, expected syntax, and completion context.
For `swimlane`, the correct answer is reuse of the existing Flowchart parser/editor-fact pipeline.
LALRPOP remains appropriate here because Flowchart already has a recovering lexer and editor-fact
projection around it.

This does not make LALRPOP the default for the other Mermaid 11.16 families. `cynefin` should use a
small line-oriented parser unless upstream semantics prove otherwise. The railroad dialects should
start from a shared spanned lexer plus recursive-descent or Pratt-style expression parser; LALRPOP is
only worth keeping after a spike proves equivalent span and recovery behavior for editor tooling.

# Verification

- `cargo fmt`
- `cargo nextest run -p merman-core flowchart registry detect --no-fail-fast` passed: 168/168.
- `cargo nextest run -p xtask admission --no-fail-fast` passed: 11/11.
- `cargo nextest run -p merman-bindings-core diagram_family_capabilities --no-fail-fast` passed:
  1/1.
- `cargo run -p xtask -- check-alignment` passed.
- `git diff --check` passed with only the existing LF-to-CRLF warning for
  `crates/merman-core/src/diagrams/flowchart_grammar.lalrpop`.

# Next

The next U3 slice should implement `cynefin` semantic parsing and editor facts before starting the
larger railroad parser spike. Keep `wardley` deferred unless the plan is explicitly expanded for its
larger model semantics.
