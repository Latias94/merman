# Performance Milestones (Triage → Targets → Work Items)

This document tracks the performance plan for `merman` with concrete, measurable milestones.
It is intentionally fixture-driven and stage-attributed (parse/layout/render/end-to-end).

## Current Status (2026-02-13)

### Stage Attribution Snapshot (canaries)

Stage spot-check (vs `repo-ref/mermaid-rs-renderer`) indicates the remaining gap is now dominated by
**parse + render**, with layout mostly competitive except for `flowchart_medium` and `mindmap_medium`.

- Spotcheck (`tools/bench/stage_spotcheck.py`, 20 samples / 1s warmup / 1s measurement):
  - Geomean ratios across canaries (`flowchart_medium,class_medium,state_medium,sequence_medium,mindmap_medium`):
    - `parse`: `~5.91x`
    - `layout`: `~1.36x`
    - `render`: `~4.40x`
    - `end_to_end`: `~1.73x`
  - Notable outliers:
    - `mindmap_medium`: `parse ~18x`, `layout ~9x`, `end_to_end ~7–8x`
    - `state_medium`: `parse ~16x`, `render ~11x`
    - `flowchart_medium`: `layout ~2–3x`, `render ~6x`, `end_to_end ~2–3x`

Root-cause direction:

- `flowchart_medium` still needs more `dugong::order` work, but **layout is no longer the only big
  lever**.
- `state_medium` render is dominated by RoughJS path generation; caching across renders improves it
  materially, but it remains a large ratio gap.
- `mindmap_medium` is currently the biggest overall gap (parse + layout), and needs a dedicated plan
  (likely typed parsing + fewer `serde_json::Value` allocations, and/or a faster internal model).

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

### M3 — State render: eliminate RoughJS cold-start cost (Done)

Goal: reduce `render/state_medium` without changing SVG output.

What we did:

- Cache RoughJS-generated path strings across render calls (global cache keyed by rough shape params),
  so Criterion iterations and server-style repeated renders avoid recomputing identical shapes.

Acceptance criteria:

- Spotcheck: `render/state_medium` drops materially (e.g. ~`1.5ms` → ~`0.45–0.80ms` in typical runs),
  and end-to-end ratio improves accordingly.

### M4 — Positioning: reduce `position_x` overhead (Planned)

Goal: after `order` is no longer dominant, reduce the next hotspot(s) without changing layout.

Work items:

- Reduce repeated graph traversals and hashing in Brandes-Koepf positioning.
- Consider an index-based temporary representation for positioning (same strategy as ordering),
  if hashing dominates.

Acceptance criteria:

- `position_x` time drops in `DUGONG_DAGREISH_TIMING=1` output for `flowchart_medium`.

### M5 — Render: close the multi-diagram gap (Planned)

Goal: reduce `render/*` ratios (flowchart + class + state) while preserving SVG output.

Work items (expected ROI order):

- Avoid repeated `String` growth by pre-sizing buffers and using a single `String` builder per SVG.
- Cache per-diagram derived values that are reused many times (e.g. sanitized labels / class names),
  but keep caches scoped to the render call to avoid cross-diagram leaks.
- Keep fast-paths for common label cases (plain text, no HTML entities, no icon syntax).

Acceptance criteria:

- Spotcheck: `render/flowchart_medium` and `render/class_medium` ratios drop materially without
  changing golden fixtures.

### M6 — Parser/IR: stop paying the `serde_json::Value` tax (Planned)

Motivation (from spotcheck):

- `parse/state_medium` and `parse/mindmap_medium` are extreme ratio outliers.
- Many diagram pipelines parse into intermediate JSON-like structures and then deserialize again for
  layout/render. That doubles work and allocates heavily.

Work items (ordered by expected ROI):

1. Add parse micro-timing (metadata detection vs preprocessing vs diagram parser vs JSON materialize).
2. Introduce typed parse paths for high-impact diagrams (start with `stateDiagram` and `mindmap`),
   and keep JSON emission as a compatibility layer (only when needed for debugging/tests).
3. Consider a lightweight lexer + hand-rolled parser for the hot subset where it measurably pays off.

Guidance:

- Do not switch to a parser combinator crate (e.g. `nom`) as a default move. That trade is mainly
  about maintainability and error reporting; it does not guarantee speed.

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
