# HPD-050 - Dagre Attribute Case-Insensitivity

Date: 2026-06-04
Task: HPD-050 layout engine source-backed audit

## Context

After graph-dimension writeback and bounding-box source coverage, the remaining direct
`repo-ref/dagre/test/layout-test.js` case was `treats attributes with case-insensitivity`.

The important question was whether this represents a missing Dugong layout behavior or only a JS
input-object compatibility seam.

## Source Finding

Pinned Dagre implements this in `repo-ref/dagre/lib/layout.js::buildLayoutGraph(...)`:

- `buildLayoutGraph(...)` calls `canonicalize(inputGraph.graph())`;
- `canonicalize(attrs)` lowercases object keys before returning a new attribute object;
- only then does Dagre select whitelisted graph attributes such as `nodesep`, `edgesep`,
  `ranksep`, `marginx`, and `marginy`.

That explains why the upstream test can set `g.graph().nodeSep = 200` and still affect
`nodesep`.

## Local API Shape

Local Dugong does not currently accept arbitrary raw JS graph-label objects. Its public layout
surface uses typed Rust labels:

- `dugong::GraphLabel { nodesep, ranksep, edgesep, marginx, marginy, ... }`;
- Mermaid-facing renderers construct those typed fields directly from their own typed
  configuration extraction;
- the active Dagre reference adapter writes lower-case typed graph-label fields.

Therefore there is no present Rust input seam where a mixed-case `nodeSep` key could appear.

## Outcome

- Recorded the upstream case in `docs/dugong/DAGRE_UPSTREAM_TEST_COVERAGE.md` under open Rust/JS
  API-shape differences.
- Did not add a production alias table or a fake Rust test for a non-existent dynamic input path.
- No production Dugong, Graphlib, renderer, xtask, SVG, or root-bounds behavior changed.

## Verification

- `rg -n "nodeSep|nodesep|canonicalize|graphNumAttrs" repo-ref\dagre\lib repo-ref\dagre\test\layout-test.js`
  confirmed the source test and JS canonicalization seam.
- `rg -n "GraphLabel|nodesep|ranksep|edgesep|marginx|marginy|nodeSep|rankSep|edgeSep|layout_dagreish" crates\dugong crates\merman-render crates\xtask -g "*.rs"`
  confirmed the local typed graph-label path and no raw `nodeSep` Dugong input bridge.
- `git diff --check` - passed with the existing `CONTEXT.jsonl` LF-to-CRLF warning only.
- JSON parse gates passed for `CONTEXT.jsonl` (`555` records) and `WORKSTREAM.json`.

## Residual Boundary

This source case is closed as a current Rust API-shape non-target. Reopen it only if Dugong gains a
public JSON/FFI Dagre input bridge that accepts raw graph-label objects with arbitrary key casing.
