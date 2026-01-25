# Architecture SVG Parity Gaps (Mermaid@11.12.2)

Baseline: Mermaid `@11.12.2` (see `repo-ref/REPOS.lock.json`).

This document tracks what is still missing for Architecture SVG DOM parity beyond the current
minimum fixture set. It complements:

- `docs/alignment/ARCHITECTURE_MINIMUM.md` (Phase 1 scope)
- `docs/alignment/ARCHITECTURE_UPSTREAM_TEST_COVERAGE.md` (which upstream sources are captured as fixtures)

## Current State

- The Architecture SVG compare passes for the non-parser-only fixture set:
  - `cargo run -p xtask -- compare-architecture-svgs --check-dom --dom-mode parity --dom-decimals 3`
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

- `crates/merman-render/src/architecture.rs` currently deserializes a simplified `{ nodes, edges }`
  shape and does not consume the full semantic model (`services`, `junctions`, `groups`, `edges`).
- To promote fixtures with groups/junctions/edges, the layout layer must map:
  - `services[]` and `junctions[]` into layout nodes (with measured sizes)
  - `groups[]` into layout clusters (bounds derived from children + padding + title/icon areas)
  - `edges[]` into routed layout edges (including `{group}` boundary traversal and label placement)

### Headless Layout Parity

Upstream Mermaid Architecture layout uses Cytoscape + FCoSE with alignment and relative placement
constraints. To reach parity, the Rust layout must reproduce the same *effective* placements used by
the pinned upstream SVG baselines.

Missing pieces likely include:

- Deterministic placement strategy for nodes and groups matching the upstream CLI outputs.
- Port-based edge routing (straight vs 90° “segments” behavior) and endpoint calculations.
- Group padding/title height rules affecting group bounds and edge endpoints.

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

Next:

1. `fixtures/architecture/upstream_architecture_docs_example_parser_only_.mmd`
2. Cypress-derived fixtures (groups, junction edges, labels, etc.)

## Notes

- Upstream Cypress rendering specs explicitly skip some cases due to non-deterministic layout.
  For `merman`, the parity target is the pinned upstream SVG baseline generated via the official
  Mermaid CLI at `@11.12.2`, so we should keep promotions incremental and backed by DOM parity checks.

- Some built-in Architecture icons include internal `id` attributes that can differ between Mermaid
  runs (e.g. `IconifyId...`). In parity mode, `xtask` normalizes those icon-internal IDs for DOM
  comparison to avoid flaky fixture updates.
