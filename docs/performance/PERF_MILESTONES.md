# Performance Milestones (Triage → Targets → Work Items)

This document tracks the performance plan for `merman` with concrete, measurable milestones.
It is intentionally fixture-driven and stage-attributed (parse/layout/render/end-to-end).

## Current Status (2026-02-13)

### Flowchart (`flowchart_medium`)

After recent work, the remaining hot stage is **layout**, specifically the Dagre-ish ordering step:

- `render` moved from multi-ms to sub-ms by avoiding the HTML sanitizer path for plain labels.
- `layout` is still ~4x slower than `mermaid-rs-renderer` for this fixture.

Useful debug toggles:

- `MERMAN_RENDER_TIMING=1` (flowchart render stage attribution)
- `MERMAN_FLOWCHART_LAYOUT_TIMING=1` (flowchart layout stage attribution)
- `DUGONG_DAGREISH_TIMING=1` (Dagre-ish pipeline stage attribution; shows `order` as dominant)
- `DUGONG_ORDER_TIMING=1` (ordering stage breakdown inside Dagre-ish pipeline)

## Milestones

### M0 — Measurement is cheap (Done)

- Keep `tools/bench/stage_spotcheck.py` as the primary “did we move the right stage?” signal.
- Maintain per-diagram micro-timing toggles for fast attribution without a profiler.

### M1 — Flowchart render: avoid sanitizer for common labels (Done)

Goal: reduce `render/flowchart_medium` without changing SVG output.

Work items:

- Fast path for plain text labels in `flowchart_label_html(...)`.
- Skip icon regex expansion when the label cannot contain `:fa-...` syntax.

### M2 — Flowchart layout: make Dagre-ish ordering fast (In progress)

Goal: cut `layout/flowchart_medium` substantially (target: < ~2x vs `mmdr` on spotcheck).

What we know:

- `MERMAN_FLOWCHART_LAYOUT_TIMING=1` shows almost all layout time inside `dugong::layout_dagreish`.
- `DUGONG_DAGREISH_TIMING=1` shows the **`order`** phase dominates for `flowchart_medium`.

Next work items (ordered by expected ROI):

1. Reduce allocations / cloning inside `dugong::order` (especially repeated `Vec<String>` builds).
2. Use index-based iteration where possible (`for_each_node_mut` / `for_each_edge_*`) instead of
   “collect ids then hash-lookup per id”.
3. If still slow: consider a deeper refactor to replace string-keyed graphs in hot ordering code
   with numeric node indices (arena / interner), keeping the external API stable.

Acceptance criteria:

- Spotcheck: `layout/flowchart_medium` improves and end-to-end drops proportionally.
- Layout micro-timing: `order` time drops materially (ideally single-digit ms on the medium fixture).

### M3 — Render (state/flowchart): cache deterministic shape generation (Planned)

Scope:

- Only for deterministic generators (e.g. RoughJS with `roughness=0`, fixed seed).
- Cache per-render-context (never global) to avoid cross-diagram leaks.

Acceptance criteria:

- Render-stage ratio drops without changing golden fixtures.

### M4 — Parser: only optimize when it matters (Planned)

Guidance:

- Do not “switch to `nom`” as a default move; it is a correctness/maintainability trade-off and does
  not automatically guarantee speed.
- Prioritize parser work only after layout/render are no longer dominating end-to-end.

If parser becomes material:

- Add parse micro-timing (tokenization vs AST build vs normalization).
- Consider a lightweight lexer (e.g. `logos`) + hand-rolled parser for the hot subset, but keep
  error reporting and parity requirements explicit.

## Fixture-driven Targets

We treat these fixtures as canaries:

- `flowchart_medium`: layout-heavy + many node labels.
- `state_medium`: render-heavy (shape generation / label handling).
- `class_medium`: end-to-end sanity (already close).

When a milestone lands, record a new spotcheck report under `target/bench/` locally (do not commit)
and update this doc with the latest ratios.
