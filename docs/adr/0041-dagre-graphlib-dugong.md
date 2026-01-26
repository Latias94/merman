# ADR-0041: Dagre/Graphlib Port (dugong, dugong-graphlib)

## Status

Accepted

## Context

Mermaid uses `dagre` + `graphlib` as the core layout engine for several diagrams (most notably
flowchart-v2). In strict SVG XML parity mode, even sub-millipixel numeric differences can break
DOM equality (e.g. `data-points` is `Base64(JSON.stringify(points))` and therefore sensitive to
the exact floating-point results).

To achieve 1:1 parity against `mermaid@11.12.2` we need a Rust implementation that matches Dagre's
algorithms, ordering, and tie-breaking behavior closely enough to reproduce upstream coordinates
bit-for-bit where required.

## Decision

- Add two new workspace crates:
  - `dugong`: a Rust port of `dagre` (layout pipeline + routing output).
  - `dugong-graphlib`: a Rust port of `graphlib` (graph representation + traversal utilities).
- Baseline upstream references (tracked in `repo-ref/REPOS.lock.json`, not git submodules):
  - `dagre@v2.0.2` (`ba9866623`)
  - `graphlib@v2.2.4` (`380d5efa1`)
- Scope and boundaries:
  - Headless-only: no DOM, no CSS, no renderer assumptions.
  - Deterministic: stable iteration order and explicit tie-breaking to match upstream outputs.
  - Numeric parity: use `f64` internally, but mirror JS `Number` semantics where they affect
    observable results (NaN/Infinity handling, ordering, rounding-sensitive comparisons).
- Integration strategy:
  - `merman-render` consumes `dugong` for diagram layouts that are Dagre-backed upstream.
  - Keep the layout boundary at the semantic model layer (`merman-*` diagrams produce a graph
    model; `dugong` returns node positions and edge routes).

## Consequences

- Pros:
  - Enables strict SVG DOM parity for Dagre-backed diagrams.
  - Produces reusable layout crates for the Rust ecosystem (independent of Mermaid rendering).
  - Reduces long-term "ad-hoc rounding hacks" inside diagram renderers.
- Cons:
  - Higher maintenance cost (upstream changes require periodic re-alignment).
  - Care is required to match JS iteration/tie-breaking behavior and floating-point corner cases.

