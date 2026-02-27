# Architecture SVG Parity Gaps (Mermaid@11.12.3)

Baseline: Mermaid `@11.12.3` (see `tools/upstreams/REPOS.lock.json`).

This document tracks what is still missing for Architecture SVG DOM parity beyond the current
minimum fixture set. It complements:

- `docs/alignment/ARCHITECTURE_MINIMUM.md` (Phase 1 scope)
- `docs/alignment/ARCHITECTURE_UPSTREAM_TEST_COVERAGE.md` (which upstream sources are captured as fixtures)

## Current State

- The Architecture SVG compare passes for the non-parser-only fixture set:
  - `cargo run -p xtask -- compare-architecture-svgs --check-dom --dom-mode parity --dom-decimals 3`
- Note: this is **DOM parity** in `dom-mode parity`, where geometry numbers are normalized to reduce
  cross-platform drift. Visual/geometry fidelity (edge routing coordinates, exact bboxes) is still
  being tightened incrementally.
- Stage B now derives the root SVG `viewBox` and `max-width` from the rendered element bounds
  (Mermaid-like `setupGraphViewbox()` behavior) and honors `architecture.{padding,iconSize,fontSize}`.
  Header-only diagrams continue to render the `80x80` fallback viewBox.
- Most real-world Architecture inputs are still `*_parser_only_` fixtures, meaning they only assert
  semantic output (`*.golden.json`) and are excluded from:
  - layout snapshots (`*.layout.golden.json`)
  - upstream SVG DOM parity compares

## Promotion Rule (parser-only → full)

A fixture is promoted from `*_parser_only_` to a “full” fixture when:

1. `cargo run -p xtask -- update-layout-snapshots --diagram architecture` produces a stable
   `*.layout.golden.json` for it, and
2. `cargo run -p xtask -- gen-upstream-svgs --diagram architecture --filter <fixture>` produces an upstream baseline, and
3. `cargo run -p xtask -- compare-architecture-svgs --check-dom --dom-mode parity --dom-decimals 3 --filter <fixture>` matches.

## Known Gaps (blocking promotions)

### Semantic → Layout Model Mapping

- `crates/merman-render/src/architecture.rs` now consumes Mermaid's semantic shape more directly:
  - `nodes[]` (including per-node `edges[]` lists, used for BFS adjacency construction)
  - `groups[]` (currently only used as a signal for header/centering heuristics)
  - `edges[]` (global edge list used for deterministic output edge ordering)
  - The `{group}` edge modifier fields `lhsGroup`/`rhsGroup` are boolean in the semantic model and
    are accepted by the deserializer.
- The mapping is still incomplete: layout does not yet model groups as compound nodes/clusters for
  layout purposes (it only places leaf nodes and lets the Stage B renderer derive group rect bounds
  from children).

### Headless Layout Parity

Upstream Mermaid Architecture layout uses Cytoscape + FCoSE with alignment and relative placement
constraints. To reach parity, the Rust layout must reproduce the same *effective* placements used by
the pinned upstream SVG baselines.

Missing pieces likely include:

- A closer match for upstream Cytoscape + FCoSE compound behavior (see `docs/adr/0053-cytoscape-layout-ports.md`).
  `manatee` now contains the FCoSE port and is used as a refinement pass from `merman-render`.
- Architecture currently relies on `manatee` (FCoSE port) for compound/nesting mechanics, but the
  numeric/viewport fidelity is still being tightened incrementally. Avoid baking “one-off” layout
  corrections into Stage B unless they can be expressed as topology/semantics-driven rules.
- The `manatee` FCoSE port now includes a spectral initialization (SVD + power iteration) matching
  the upstream `cytoscape-fcose` pipeline. Randomness is made explicit via a seed to keep headless
  outputs deterministic for tests.
- Deterministic placement strategy for nodes and groups matching the upstream CLI outputs (FCoSE),
  including the exact floating-point behavior and constraint handling.
- Port-based edge routing (straight vs 90° “segments” behavior) and endpoint calculations.
  - Upstream sets Cytoscape edge styles (`curve-style: segments`) and post-adjusts XY edges via
    `segment-distances`/`segment-weights` derived from `edge.sourceEndpoint()`/`edge.targetEndpoint()`
    (see `repo-ref/mermaid/packages/mermaid/src/diagrams/architecture/architectureRenderer.ts`).
  - `merman-render` now mirrors the upstream `getSegmentWeights(...)` math when constructing the
    3-point polyline stored in layout snapshots (so Stage B can render Mermaid-like bends using
    the layout-provided `LayoutEdge.points`): `crates/merman-render/src/architecture.rs`.
  - Remaining work is primarily about *overall geometry fidelity* (FCoSE placements, group bounds,
    root viewBox/max-width), not the “segments vs straight” polyline shape itself.
- Group padding/title height rules affecting group bounds and edge endpoints.
  - Upstream `svgDraw.ts` documents an extra `+18px` on the *bottom* side of group bounds due to
    service label height (used when routing `{group}` edges). Our Stage B bounds estimator must
    account for this label extension when deriving group rects and the root `viewBox`.
- Relative placement constraint semantics:
  - Mermaid uses FCoSE `relativePlacementConstraint` with `gap = 1.5 * iconSize`.
  - In upstream CoSE, this `gap` behaves like a **border-to-border minimum spacing**, not a
    center-to-center distance. Interpreting it incorrectly makes the layout too compact and shifts
    group bounds/root `viewBox` (visible in fixtures like `*_group_edges*`).

### Stage B SVG Parity Renderer

`render_architecture_diagram_svg` must eventually render the full Architecture DOM:

- Services:
  - icon rendering (built-ins + iconify fallback behavior)
  - labels and their positioning
  - service background shape path parity
- Junctions:
  - node shape + sizing and DOM structure
- Groups:
  - group container rectangle, title/icon, nesting, clip paths (if used upstream)
- Edges:
  - markers/arrows and label nodes
  - segment vs straight curve DOM parity
  - `{group}` traversal DOM parity

## Suggested Promotion Order (smallest deltas first)

Completed:

1. `fixtures/architecture/upstream_architecture_docs_service_icon_text.mmd`
2. `fixtures/architecture/upstream_architecture_docs_edge_titles.mmd`
3. `fixtures/architecture/upstream_architecture_docs_edge_arrows.mmd`
4. `fixtures/architecture/upstream_architecture_docs_example.mmd`
5. `fixtures/architecture/upstream_architecture_docs_groups_within_groups.mmd`
6. `fixtures/architecture/upstream_architecture_docs_group_edges.mmd`

Next:

1. Cypress-derived fixtures (groups, junction edges, labels, etc.)

## Notes

- Upstream Cypress rendering specs explicitly skip some cases due to non-deterministic layout.
  For `merman`, the parity target is the pinned upstream SVG baseline generated via the official
  Mermaid CLI at `@11.12.3`, so we should keep promotions incremental and backed by DOM parity checks.

- Wrap sensitivity: upstream Architecture label wrapping is width-driven (`createText()`), so any
  geometry drift (layout placements, edge endpoints, group bounds) can cascade into different line
  breaks. Prefer fixing the underlying geometry/measurement chain (layout + bbox estimation) over
  adding fixture-id keyed renderer special-cases.

- Baseline determinism: Architecture uses Cytoscape `fcose`, whose spectral initialization relies on
  `Math.random()`. `xtask gen-upstream-svgs --diagram architecture` seeds browser-side randomness
  deterministically when generating upstream SVG baselines so they are reproducible across runs.
  See `docs/adr/0055-upstream-svg-determinism-for-cytoscape-layouts.md`.

## Debugging Root Viewport Deltas (parity-root)

When `--dom-mode parity-root` fails due to root `viewBox` / `style max-width` differences, use the
delta debugger to pinpoint which elements drifted (services, junctions, group rects):

```
cargo run -p xtask -- debug-architecture-delta --fixture upstream_architecture_docs_group_edges
```

Outputs:

- `target/debug/architecture-delta/<fixture>.upstream.svg`
- `target/debug/architecture-delta/<fixture>.local.svg`
- `target/debug/architecture-delta/<fixture>.md` (tabular per-element deltas)

## Getting a Global Summary (All Fixtures)

To get a quick, sortable overview of viewport and translation drift across *all* Architecture fixtures:

```
cargo run -p xtask -- summarize-architecture-deltas
```

This writes:

- `target/debug/architecture-delta/architecture-delta-summary.md`

- Some built-in Architecture icons include internal `id` attributes that can differ between Mermaid
  runs (e.g. `IconifyId...`). In parity mode, `xtask` normalizes those icon-internal IDs for DOM
  comparison to avoid flaky fixture updates.

- Some upstream Cypress Architecture fixtures use a shorthand edge syntax like `db L--R server`.
  Mermaid CLI `@11.12.3` renders these as an error SVG ("Syntax error in text"), so we currently keep
  those fixtures in `*_parser_only_` form until we decide whether to align to the CLI parser behavior
  (Langium grammar requires `db:L -- R:server`) or to the browser-only parser behavior.

- Some upstream Cypress Architecture fixtures use parenthesized port pairs like `servC (L--R) servL`.
  Mermaid CLI `@11.12.3` also renders these as an error SVG, so these remain `*_parser_only_` until the
  baseline source is clarified (CLI vs browser/Cypress).

- Some upstream Cypress Architecture fixtures use label shorthands like `servC L-[Label]-R servL`.
  Mermaid CLI `@11.12.3` renders these as an error SVG as well, so these stay `*_parser_only_` for now.

- Some upstream Cypress Architecture fixtures use `{group}` boundary traversal combined with shorthand
  edges like `left_disk{group} (R--L) center_disk{group}`. Mermaid CLI `@11.12.3` renders these as an
  error SVG, so they remain `*_parser_only_` for now.

- To preserve the original Cypress strings while still enabling CLI baselines + DOM parity, we add
  `*_normalized` fixture variants that rewrite shorthand into Mermaid@11.12.3's Langium syntax.
  See `docs/alignment/ARCHITECTURE_UPSTREAM_TEST_COVERAGE.md` for the exact list.
