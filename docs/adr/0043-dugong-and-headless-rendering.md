# ADR 0043: Dugong (Dagre) + Headless Rendering Architecture

## Status

Accepted

## Context

`merman` targets 1:1 behavioral parity with Mermaid `@11.12.2`.

For many Mermaid diagrams (notably flowchart/class/er/state), the final output depends on a layout
pipeline that in upstream is implemented with the `dagre` family of libraries plus Mermaid-specific
wrappers and shape/edge routing utilities. In upstream code this pipeline is DOM-driven: elements
are inserted into SVG, measured, laid out, and then positioned.

We want:

- a **pure Rust**, **headless** solution suitable for integration into other UI frameworks
  (including non-DOM renderers like canvas),
- a parity-oriented workflow similar to `merman` (upstream-baselined tests + coverage docs),
- minimal crate fragmentation while keeping the layout engine reusable for the Rust ecosystem.

## Decision

### 1) Create a reusable Dagre-compatible layout library: `dugong`

- Add a workspace crate `dugong` as the public, general-purpose layout library.
- `dugong` targets API and behavior parity with upstream Dagre (`repo-ref/dagre`, pinned revision).
- `dugong` provides the Dagre layout entrypoints and algorithms (acyclic, ranking, ordering,
  positioning, edge routing, nesting, etc.) to match upstream tests.

### 2) Split the graph data structure as a separate crate: `dugong-graphlib`

- Add `dugong-graphlib` as the graph container crate (Graph API, compound graphs, multigraphs,
  traversal utilities) matching upstream `@dagrejs/graphlib` expectations.
- `dugong` depends on `dugong-graphlib` (not vice versa).
- `dugong` re-exports `dugong_graphlib` types for ergonomics when appropriate.

Rationale:
- Dagre’s upstream tests and algorithms assume a Graphlib-like API.
- Keeping Graphlib separate improves reuse and avoids locking the graph API to layout concerns.

### 3) Do not port `d3` (DOM selection) — port required semantics instead

We explicitly **do not** reimplement `d3.select()/append()` style DOM manipulation in Rust.
Instead, the Rust pipeline is split into:

- **layout**: needs only graph structure + node/edge sizes (no DOM),
- **rendering backends**: convert a layout result into SVG/canvas/etc.

This avoids coupling the core layout engine to any UI framework.

### 4) Introduce a pluggable text measurement interface for parity and portability

Because upstream uses SVG/DOM measurements to size labels, pure Rust must provide sizes without a
DOM. We introduce a runtime-pluggable interface (implemented in `merman` rendering layer, not in
`dugong`):

- `TextMeasurer`: given text + font style + wrapping rules, returns width/height metrics.

This supports:
- deterministic headless rendering (default measurer),
- host-driven measurement for UI frameworks (e.g. gpui) to ensure visual parity.

### 5) Keep `merman` rendering crates minimal

- Keep a single rendering crate in `merman` (e.g. `merman-render`) that contains multiple backends
  (SVG string, and future raster/canvas targets) as modules and/or feature flags.
- Avoid creating many small per-backend crates unless a backend becomes independently valuable.

## Parity and Testing Strategy

### Baselines

- `dugong` baseline: `repo-ref/dagre` (document the pinned revision).
- `dugong-graphlib` baseline: upstream `@dagrejs/graphlib` as required by Dagre tests.

### Tests

- Port upstream Dagre tests from `repo-ref/dagre/test/*.js` into Rust tests.
- Maintain `docs/alignment`-style coverage docs under `docs/dugong/`:
  - `DAGRE_UPSTREAM_TEST_COVERAGE.md` mapping upstream tests → Rust tests
  - `GRAPH_LIB_UPSTREAM_TEST_COVERAGE.md` for graphlib behavior if needed

### Mermaid integration tests

Once `merman-render` exists:
- add parity tests for layout outputs (node coordinates + edge routes) rather than brittle
  full-SVG string equality.
- add optional SVG snapshot tests only when text measurement is deterministic and stable.

## Consequences

- `dugong` becomes a reusable Rust implementation of Dagre-class layout algorithms.
- `merman` can stay headless and UI-framework-agnostic while still enabling high-fidelity output.
- Exact SVG parity depends on deterministic text measurement and faithful porting of Mermaid’s
  rendering semantics (shapes, markers, curves, cluster/subgraph behaviors).
