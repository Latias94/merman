# HPD-050 - Dagreish Layout Source Coverage

Date: 2026-06-04
Task: HPD-050 layout engine source-backed audit

## Context

After the Architecture Cytoscape canvas-width audit rejected font-family, exact lookup, and
group-padding production fixes, HPD-050 continued through source-backed Dugong/Graphlib/Dagre
coverage. The previous Graphlib public API slice strengthened an `Option<T>` label seam without a
production change. This pass moved closer to Mermaid layout consumers by targeting the full
`layout_dagreish(...)` path used by State, Class, Flowchart, ER, and Requirement renderers.

## Source Finding

Pinned Dagre `repo-ref/dagre/test/layout-test.js` still had layout-output cases not represented in
the Dagre coverage ledger. Four of those cases map directly to behavior consumed by renderer layout
pipelines:

- `can layout a long edge with a label`
- `can layout out a short cycle`
- `minimizes separation between nodes not adjacent to subgraphs`
- `can layout subgraphs with different rankdirs`

These are higher-value HPD-050 candidates than unused Graphlib shortest-path algorithms or JS-only
Graphlib chainability because they exercise edge-label coordinates, acyclic undo, and compound
subgraph geometry after the real Dagreish pipeline.

## Outcome

- Added direct `layout_dagreish(...)` tests for the four source cases in
  `crates/dugong/tests/layout_test.rs`.
- The tests intentionally use `layout_dagreish(...)`, not the default minimal `dugong::layout(...)`,
  because Mermaid-facing renderers consume the full Dagreish path.
- Updated `docs/dugong/DAGRE_UPSTREAM_TEST_COVERAGE.md` to map those upstream cases to the new
  Rust tests.
- No production Dagre, Graphlib, renderer, SVG, or root-bounds behavior changed.

## Verification

- `cargo nextest run -p dugong layout_dagreish_can_layout_a_long_edge_with_a_label`
- `cargo nextest run -p dugong layout_dagreish_can_layout_a_short_cycle`
- `cargo nextest run -p dugong layout_dagreish_minimizes_separation_between_nodes_not_adjacent_to_subgraphs`
- `cargo nextest run -p dugong layout_dagreish_can_layout_subgraphs_with_different_rankdirs`
- `cargo nextest run -p dugong --test layout_test` - passed, `15` tests run.
- `cargo nextest run -p dugong` - passed, `271` tests run.
- `cargo fmt --check -p dugong -p dugong-graphlib` - passed.
- `git diff --check` - passed with the existing `CONTEXT.jsonl` LF-to-CRLF warning only.
- Line-by-line JSON parse for `docs/workstreams/headless-parity-deepening/CONTEXT.jsonl` - passed,
  `542` JSONL records parsed.
- `docs/workstreams/headless-parity-deepening/WORKSTREAM.json` parse - passed.

## Residual Boundary

This slice does not claim the remaining upstream `layout-test.js` cases are covered. In particular,
GraphLabel `width` / `height` writeback and default minimal `dugong::layout(...)` equivalence remain
separate API/product decisions. Continue HPD-050 by choosing source-backed Dagre/Graphlib cases that
real Mermaid-facing consumers use.
