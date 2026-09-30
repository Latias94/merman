---
type: "Work Progress"
title: "Mermaid 12 Usecase Dagre self-loop checkpoint"
description: "Usecase Dagre self-loop segments and helper labels now follow Mermaid 12 source behavior; broader family admission remains open."
timestamp: 2026-09-20T13:57:53Z
record_id: "66d3a2208d804515a7ab701e0c69f399"
producer_id: "codex-mermaid12"
run_id: "session-01a0bd10"
related_plan: "docs/plans/2026-09-20-1250-refactor-mermaid-12-alignment-plan.md"
git_branch: "refactor/mermaid-12-alignment"
git_commit: "85f599be8"
supersedes: "c775359c111547128eee5e925c22e572"
---

# Status

GOAL U1–U15 remains active and incomplete. Alpha.7 is unpublished main plus Mermaid 12 alignment;
package versions remain alpha.6 and the selected reference remains 11.17.2 until U15 promotion.
FFI, built Web WASM, Playground and genuine golden refresh remain in scope. No push, PR, release or merge.

# Implemented

Commit `85f599be8` ports Mermaid 12's Usecase Dagre self-loop preparation and output semantics.
Usecase is excluded from the upstream compact self-loop merge allowlist, so each self-loop expands to
three named segments: `A-cyclic-special-1`, `A-cyclic-special-mid`, and `A-cyclic-special-2`.
Two 0.1x0.1 labelRect helper nodes use source IDs `A---A---1/2`, inherit the original parent, and
are painted. Only the middle segment retains the original label; all three segments emit their
helper/edge-label records. Duplicate self-loops reuse the same Graphlib keys and therefore overwrite
the first triple while retaining insertion order, as upstream does.

Top-level segment normalization intentionally retains the original self-loop node as the shape
endpoint, while an extracted boundary keeps real helper endpoints. Top-level synthetic edges receive
`data-usecase-*` and role metadata, with synthetic ID accessibility fallback; extracted edges remain
inside recursive content without top-level annotation. Styles, marker ownership and original
relationship mapping follow the source segment rules.

Earlier commits remain part of this behavior graph: ELK node port alignment (`795fdbd75`), Dagre
boundary extraction and text/root style propagation (`14d2ca55`), and the prior Usecase pipeline.

# Verification

- `cargo nextest run -p merman-render --features layout-elk --test usecase_svg_test -E
  'test(usecase_dagre_keeps_self_loop_segments) | test(usecase)'`: 17 passed, run
  `6a05ba3f-33ac-4481-b888-7ea5d3ba261b` (the self-loop test itself passed separately as
  `015b3702-60bd-4a0b-b700-7c5f6c6f3ffd`).
- `cargo check -p merman-render --no-default-features`: passed.
- `git diff --check` and scoped rustfmt passed before commit.
- Source evidence is retained in `target/mermaid-12-admission/usecase-dagre-loops-probe.cjs/.json`
  and the SVG variant probe. The pinned Mermaid 12 runtime confirmed helper dimensions, synthetic
  IDs, duplicate-loop overwrite, top-level versus extracted metadata, and HTML/SVG empty-label DOM.
  These are exploratory probes, not accepted goldens.

# Remaining Work / Next Action

1. Usecase still lacks full nested SVG DOM admission, exact boundary package/long-title geometry,
   handDrawn shapes, complete marker/style structure and source family catalog registration.
2. ELK and Dagre route edge cases beyond the focused Usecase samples need source-backed family tests;
   browser text and bbox residuals remain bounded artifact contracts.
3. Complete parser/resource review, including ordered JSON recursion and external model validation.
4. Continue U9–U15: Agentflow, existing family/theme deltas, editor facts, FFI, actual Web WASM,
   Playground, license/feature closure, upstream goldens, bundle promotion and alpha.7 versioning.
5. Do not claim full Usecase parity or refresh its goldens until the remaining DOM and source geometry
   contracts are closed.

No Cargo process is active. Continue serial Cargo usage, protect unrelated edits, and use explicit
UTF-8 Python file operations.
