# Performance Milestones (Triage → Targets → Work Items)

This document tracks the performance plan for `merman` with concrete, measurable milestones.
It is intentionally fixture-driven and stage-attributed (parse/layout/render/end-to-end).

## Current Status (2026-02-13)

### Flowchart (`flowchart_medium`)

Stage spot-check (vs `repo-ref/mermaid-rs-renderer`) still shows that **flowchart layout is the main
end-to-end bottleneck**, and the remaining gap is dominated by Dagre-ish ordering sweeps:

- Spotcheck (`tools/bench/stage_spotcheck.py`, 30 samples / 2s warmup / 3s measurement):
  - `parse`: `2.66x` slower (`601.76 µs` vs `226.27 µs`)
  - `layout`: `1.83x` slower (`7.4041 ms` vs `4.0514 ms`)
  - `render`: `3.59x` slower (`678.51 µs` vs `188.77 µs`)
  - `end_to_end`: `2.31x` slower (`9.6824 ms` vs `4.1845 ms`)
- Micro-timing (deterministic text measurer) indicates `dugong::order` is the hotspot:
  - After caching per-rank layer graphs, `order` is split roughly as:
    - `build_layer_graph_cache ~2.7ms`
    - `sweeps ~1.9ms` (dominant part inside sweeps is `sort_subgraph ~1.6ms`)
  - Next hotspots after `order` (varies by run): `position_x ~1ms`, `compound_border ~0.8ms`

Notes:

- With `layout` much faster than before, **`render` is now the biggest ratio gap** on the
  canary fixtures.

Useful debug toggles:

- `MERMAN_RENDER_TIMING=1` (flowchart render stage attribution)
- `MERMAN_FLOWCHART_LAYOUT_TIMING=1` (flowchart layout stage attribution)
- `DUGONG_DAGREISH_TIMING=1` (Dagre-ish pipeline stage attribution; shows `order` as dominant)
- `DUGONG_ORDER_TIMING=1` (ordering stage breakdown inside Dagre-ish pipeline)

### Class diagram (`class_medium`)

This fixture is useful as a counter-example:

- Spotcheck shows `layout` is already faster than `mmdr` (`~0.45x`), and end-to-end is close
  (`~0.93x`), but `render` is still far behind (`~6.29x`).
- Implication: once we fix flowchart layout, **render optimizations will pay off across diagram
  types**, not only flowcharts.
- `MERMAN_RENDER_TIMING=1` now also emits a `[render-timing] diagram=classDiagram ...` line, so we
  can attribute class renderer hotspots without a profiler.

## Milestones

### M0 — Measurement is cheap (Done)

- Keep `tools/bench/stage_spotcheck.py` as the primary “did we move the right stage?” signal.
- Maintain per-diagram micro-timing toggles for fast attribution without a profiler.

### M1 — Flowchart render: avoid sanitizer for common labels (Done)

Goal: reduce `render/flowchart_medium` without changing SVG output.

Work items:

- Fast path for plain text labels in `flowchart_label_html(...)`.
- Skip icon regex expansion when the label cannot contain `:fa-...` syntax.

### M2 — Flowchart layout: make Dagre-ish ordering fast (Mostly done)

Goal: cut `layout/flowchart_medium` substantially.

Primary target: reduce the spotcheck ratio from ~`5.0x` → `< 2.0x` without changing layout output.
Current: `~1.8x` on `flowchart_medium` in a recent spotcheck run.

What we know:

- `MERMAN_FLOWCHART_LAYOUT_TIMING=1` shows almost all layout time inside `dugong::layout_dagreish`.
- `DUGONG_DAGREISH_TIMING=1` shows the **`order`** phase dominates for `flowchart_medium`.
- `DUGONG_ORDER_TIMING=1` shows `sweeps` is the dominant sub-stage inside `order`.

Next work items (ordered by expected ROI):

1. Add micro-timing *inside* `sweeps` to identify the true dominant operations
   (e.g. barycenter evaluation vs conflict resolution vs sorting vs layer graph construction).
2. Reduce allocations / cloning inside `sweeps` (reuse scratch buffers; avoid building temporary
   `Vec<String>` / `HashMap<String, ...>` where a borrowed view works).
3. Deeper refactor (likely required): introduce an index-based internal representation for ordering
   sweeps:
   - map external `NodeKey` → dense `usize` once per `order(...)` call
   - represent adjacency as `Vec<Vec<usize>>` (or a flat CSR-style structure)
   - keep stable output by translating indices back to `NodeKey` at the boundary
4. Algorithmic improvement: early-exit sweeps when crossing count stops improving; avoid “fixed
   number of sweeps” when the order has converged.

Acceptance criteria:

- Spotcheck: `layout/flowchart_medium` improves and end-to-end drops proportionally.
- Layout micro-timing: `order` and especially `sweeps` drop materially (single-digit ms is a
  reasonable medium-term target for the medium fixture).

### M3 — Positioning: reduce `position_x` overhead (Planned)

Goal: after `order` is no longer dominant, reduce the next hotspot(s) without changing layout.

Work items:

- Reduce repeated graph traversals and hashing in Brandes-Koepf positioning.
- Consider an index-based temporary representation for positioning (same strategy as ordering),
  if hashing dominates.

Acceptance criteria:

- `position_x` time drops in `DUGONG_DAGREISH_TIMING=1` output for `flowchart_medium`.

### M4 — Render: close the multi-diagram gap (Planned)

Goal: reduce `render/*` ratios (flowchart + class + state) while preserving SVG output.

Work items (expected ROI order):

- Avoid repeated `String` growth by pre-sizing buffers and using a single `String` builder per SVG.
- Cache per-diagram derived values that are reused many times (e.g. sanitized labels / class names),
  but keep caches scoped to the render call to avoid cross-diagram leaks.
- Keep fast-paths for common label cases (plain text, no HTML entities, no icon syntax).

Acceptance criteria:

- Spotcheck: `render/flowchart_medium` and `render/class_medium` ratios drop materially without
  changing golden fixtures.

### M5 — Parser: only optimize when it matters (Planned)

Guidance:

- Do not switch to a parser combinator crate (e.g. `nom`) as a default move. That trade is mainly
  about maintainability and error reporting; it does not guarantee speed.
- Prioritize parser work only after layout is no longer the dominant end-to-end bottleneck for the
  main canary fixtures.

If parser becomes material:

- Add parse micro-timing (metadata detection vs preprocessing vs AST build vs typed parsing).
- Consider a lightweight lexer (e.g. `logos`) + hand-rolled parser for the hot subset, keeping
  parity requirements explicit.

## Fixture-driven Targets

We treat these fixtures as canaries:

- `flowchart_medium`: layout-heavy + many node labels.
- `state_medium`: render-heavy (shape generation / label handling).
- `class_medium`: end-to-end sanity (already close).

When a milestone lands, record a new spotcheck report under `target/bench/` locally (do not commit)
and update this doc with the latest ratios.

## Non-goals (for now)

- “Switch graph crate” as a primary optimization strategy.
  - The dominant hotspots are algorithmic + representation issues in ordering/positioning; swapping
    a graph crate does not automatically remove the need for dense, index-based hot paths.
  - Prefer keeping the public graph API stable and introducing internal dense representations in
    performance-critical stages.
