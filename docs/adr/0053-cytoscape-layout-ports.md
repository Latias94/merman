# ADR-0053: Cytoscape Layout Ports (FCoSE, COSE-Bilkent)

## Status

Accepted

## Context

Mermaid `@11.12.2` relies on Cytoscape-based layout engines for multiple diagrams:

- Mindmap: `cose-bilkent` (via `cytoscape-cose-bilkent`)
- Architecture: `fcose` (via `cytoscape-fcose`) with additional alignment/relative constraints derived
  from the Architecture grammar (services, groups, junctions, edges).

In `merman`, these diagrams currently do not reproduce upstream placements. This directly affects:

- Root `<svg>` viewport attributes (`viewBox`, `style="max-width: ..."`), which dominate `parity-root`
  mismatches for Architecture and Mindmap.
- Downstream edge routing (ports/segments), group bounds, and label placement, which depend on the
  layout engine’s coordinates and tie-breaking.

The project goal is full parity with Mermaid’s pinned baseline. Therefore, we need a headless,
deterministic Rust implementation of the same effective layout behavior.

## Decision

- Port the Cytoscape layout engines used by Mermaid into Rust (headless-only):
  - `cytoscape@3.33.1`
  - `cytoscape-fcose@2.2.0`
  - `cytoscape-cose-bilkent@4.1.0`
  - Versions are taken from `repo-ref/mermaid/pnpm-lock.yaml` under the pinned Mermaid baseline.
  - Upstream source repositories are tracked in `repo-ref/REPOS.lock.json` (no git submodules).
  - Note: `cytoscape-cose-bilkent`/`cytoscape-fcose` delegate most of their algorithmic core to
    `cose-base` + `layout-base`, which are also tracked in `repo-ref/REPOS.lock.json`.
- Implement them as a dedicated workspace crate:
  - `manatee`: reusable, headless graph layout engines (initially `fcose` and `cose-bilkent`).
- Keep `merman-render` responsible for diagram-to-layout-data mapping:
  - Diagram renderers produce a layout graph (nodes with measured sizes, edges with weights/labels,
    groups/clusters if applicable).
  - `manatee` consumes the graph and returns node positions (and any algorithm-provided routing hints).
- Determinism and numeric policy:
  - Stable iteration order (explicit node/edge ordering by ID).
  - No ambient randomness: all randomness must be explicit and seeded; default seed pinned to match
    Mermaid CLI baselines.
  - Prefer `f64` internally, but mirror JS `Number` corner cases where they impact observable output
    (e.g. comparisons near-equality, tie-breaking).
- Testing policy:
  - Continue to treat official upstream SVGs (`fixtures/upstream-svgs/**`) as the source of truth.
  - Add or expand fixtures to cover Cytoscape-driven placements as we promote more Architecture and
    Mindmap fixtures to “full” (layout + upstream DOM parity).
  - If needed for debugging (not runtime), add an `xtask` script to extract/reference upstream layout
    positions from Mermaid to validate the port (tests/dev-only).

## Alternatives Considered

1. Embed a JS engine (Boa/QuickJS) and run the upstream layout code at runtime.
   - Pros: fastest path to parity.
   - Cons: heavy dependency surface, harder security story, runtime non-determinism risks, and not
     aligned with the “pure Rust headless library” direction.

2. Implement a simplified deterministic layout that merely matches the current fixture set.
   - Pros: short-term mismatch reduction.
   - Cons: conflicts with the “full Mermaid parity” goal; likely to break as fixtures expand or as
     more real-world inputs are promoted from parser-only to full.

3. Use an existing Rust graph layout library as a substitute (no port).
   - Pros: lower initial effort.
   - Cons: unlikely to match Mermaid’s pinned outputs closely enough; different heuristics and
     tie-breaking cause persistent SVG parity drift.

## Consequences

- Pros:
  - Unblocks Architecture and Mindmap parity-root viewport alignment by matching upstream placements.
  - Enables incremental promotion of currently parser-only Architecture fixtures into full DOM parity.
  - Produces a reusable Rust layout crate for the ecosystem (similar to `dugong` for Dagre).
- Cons:
  - Non-trivial engineering effort: force-directed layouts are complex and sensitive to ordering and
    numerics.
  - Requires ongoing maintenance when Mermaid updates its Cytoscape dependencies.
