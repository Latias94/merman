# Performance Milestones (Triage → Targets → Work Items)

This document tracks the performance plan for `merman` with concrete, measurable milestones.
It is intentionally fixture-driven and stage-attributed (parse/layout/render/end-to-end).

## Current Status (2026-02-13)

### Stage Attribution Snapshot (canaries)

Stage spot-check (vs `repo-ref/mermaid-rs-renderer`) indicates the remaining gap is dominated by
**render + flowchart layout**, with parse now substantially closer after moving the pipeline parse
bench to the typed render semantic models (avoids per-field `serde_json::Value` object-key
allocations for `mindmap`/`stateDiagram`).

- Spotcheck (`tools/bench/stage_spotcheck.py`, 20 samples / 1s warmup / 1s measurement):
  - Latest canary set (`flowchart_medium,class_medium,state_medium,mindmap_medium`):
    - `parse`: `~2.6–2.7x` (run-to-run variance is noticeable)
    - `layout`: `~1.1–1.2x` (gmean hides that `flowchart`/`mindmap` layout can still be large)
    - `render`: `~3.5–3.7x`
    - `end_to_end`: `~1.1–1.2x` (gmean is skewed by `class`/`state` being < 1x)
  - Notable outliers in a recent run:
    - `state_medium`: `render ~5–7x` (RoughJS + leaf node work; typed model still serializes to JSON for renderer)
    - `mindmap_medium`: `layout ~5–6x`, `end_to_end ~2.8–2.9x`
    - `flowchart_medium`: `layout ~1.4–2.4x`, `render ~3.7x`, `end_to_end ~1.2–1.8x`

Root-cause direction:

- `flowchart_medium` is now a multi-stage problem: layout (`dugong::order`) is still expensive, and
  render spends a meaningful slice in DOM building + edge path work + viewport computation.
- Flowchart viewport work had some pure overhead: we were generating an edge path `d` string and
  then re-parsing it to approximate `getBBox()`. We now compute cubic bounds during curve emission
  for the viewBox approximation, avoiding `svg_path_bounds_from_d(...)` in the flowchart viewbox
  prepass (still builds the `d`, but no longer parses it).
- Class diagram viewport work had the same pattern: we were accumulating `path_bounds` by parsing
  the emitted `d` strings. We now compute bounds during path emission for class edges + RoughJS-like
  strokes, and `path_bounds` micro-timing dropped from ~`O(50µs)` to ~`O(1–3µs)` for `class_medium`.
- `state_medium` render is dominated by leaf node work, especially RoughJS path generation and emit.
- `mindmap_medium` overall gap is now mostly layout (COSE port / bbox work) rather than parse.

Useful debug toggles:

- `MERMAN_RENDER_TIMING=1` (flowchart render stage attribution)
- `MERMAN_PARSE_TIMING=1` (parse stage attribution: preprocess/detect/parse/sanitize)
- `MERMAN_FLOWCHART_LAYOUT_TIMING=1` (flowchart layout stage attribution)
- `DUGONG_DAGREISH_TIMING=1` (Dagre-ish pipeline stage attribution; shows `order` as dominant)
- `DUGONG_ORDER_TIMING=1` (ordering stage breakdown inside Dagre-ish pipeline)

### Class diagram (`class_medium`)

This fixture is useful as a counter-example:

- Spotcheck shows `layout` is already faster than `mmdr` (`~0.48x`), and end-to-end can be faster
  (`~0.65–0.75x` in recent runs), but `render` is still far behind (`~4–6x`).
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

### M2 — Flowchart layout: make Dagre-ish ordering fast (In progress)

Goal: cut `layout/flowchart_medium` substantially.

Primary target: reduce the spotcheck ratio from `~5x` → `< 2.0x` without changing layout output.
Current: `~1.4–2.4x` on `flowchart_medium` in the latest canary runs (numbers fluctuate).

What we know:

- `MERMAN_FLOWCHART_LAYOUT_TIMING=1` shows almost all layout time inside `dugong::layout_dagreish`.
- `DUGONG_DAGREISH_TIMING=1` shows the **`order`** phase dominates for `flowchart_medium`.
- `DUGONG_ORDER_TIMING=1` shows `sweeps` is the dominant sub-stage inside `order`.

Next work items (ordered by expected ROI):

1. Add micro-timing *inside* `sweeps` to identify the true dominant operations
   (e.g. barycenter evaluation vs conflict resolution vs sorting vs layer graph construction).
   (Done: `sort_subgraph_*` breakdown is now available in `[dugong-timing] stage=order ...`.)
2. Reduce allocations / cloning inside `sweeps` (reuse scratch buffers; avoid building temporary
   `Vec<String>` / `HashMap<String, ...>` where a borrowed view works).
   (In progress: `sort_subgraph(...)` now runs on node indices end-to-end (movable/barycenter/
   conflict-resolution/subgraph expansion/sort), and the order evaluator (`build_layer_matrix` +
   `cross_count`) is index-based as well. Remaining overhead is now dominated by layer-graph
   materialization + constraint-graph building.)
3. Reduce `build_layer_graph_cache` costs (this is outside `sweeps`, but still inside `order`):
   - Build cached layer graphs using a lightweight node label rather than cloning full `NodeLabel`.
   - On `flowchart_medium`, local timing showed `build_layer_graph_cache` dropping from ~`2.0ms` → ~`0.7ms`,
     and `order total` dropping from ~`4.5ms` → ~`3.3ms`.
4. Deeper refactor (likely required): introduce an index-based internal representation for ordering
   sweeps:
   - map external `NodeKey` → dense `usize` once per `order(...)` call
   - represent adjacency as `Vec<Vec<usize>>` (or a flat CSR-style structure)
   - keep stable output by translating indices back to `NodeKey` at the boundary
   (Partially done: the sweep algorithm now works primarily on dense `usize` ids and only resolves
   back to node ids at the boundary when applying orders.)
5. Algorithmic improvement: early-exit sweeps when crossing count stops improving; avoid “fixed
   number of sweeps” when the order has converged.

Acceptance criteria:

- Spotcheck: `layout/flowchart_medium` improves and end-to-end drops proportionally.
- Layout micro-timing: `order` and especially `sweeps` drop materially (single-digit ms is a
  reasonable medium-term target for the medium fixture).

### M3 — State render: eliminate RoughJS cost (Partially done)

Goal: reduce `render/state_medium` without changing SVG output.

What we did:

- Cache RoughJS-generated path strings across render calls (global cache keyed by rough shape params),
  so Criterion iterations and server-style repeated renders avoid recomputing identical shapes.

Acceptance criteria:

- Spotcheck: `render/state_medium` drops materially and consistently, not only after warm caches.

Status note:

- The cache helps, but `state_medium` is still far behind in `render` stage ratios. The next steps
  are to reduce per-leaf overhead (style resolution, SVG emission) and increase cache hit rate for
  RoughJS shapes.

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
- Avoid cloning `effective_config` JSON in the hot render path; pass `MermaidConfig` (Arc-backed)
  through the render API so diagram renderers can read config without deep-cloning.
- Cache per-diagram derived values that are reused many times (e.g. sanitized labels / class names),
  but keep caches scoped to the render call to avoid cross-diagram leaks.
- Keep fast-paths for common label cases (plain text, no HTML entities, no icon syntax).
- Flowchart: compute edge path geometry once per render (reused for viewbox curve-bounds + edgePaths),
  caching `d` + `data-points` + bounds to avoid paying for D3 curve evaluation twice.
- Viewport bounds: avoid “build SVG path `d` → parse `d`” patterns by computing bounds during path
  generation; extend this beyond flowcharts (e.g. class diagram `path_bounds` hot section).

Acceptance criteria:

- Spotcheck: `render/flowchart_medium` and `render/class_medium` ratios drop materially without
  changing golden fixtures.

### M6 — Parser/IR: stop paying the `serde_json::Value` tax (In progress)

Motivation (from spotcheck):

- Many diagram pipelines pay a large allocation tax by constructing `serde_json::Value` object trees
  with repeated per-field key strings (e.g. `"id"`, `"label"`, `"shape"`) even when the downstream
  renderer only needs typed data.

Work items (ordered by expected ROI):

1. Add parse micro-timing (metadata detection vs preprocessing vs diagram parser vs JSON materialize).
2. Introduce typed parse paths for high-impact diagrams (start with `stateDiagram` and `mindmap`),
   and keep JSON emission as a compatibility layer (only when needed for debugging/tests). (Partially done)
   - `Engine::parse_diagram_for_render_model_sync(...)` returns typed semantic models for `mindmap`/`stateDiagram`.
   - The `parse/*` pipeline bench now measures the typed render parse path (so spotcheck ratios are apples-to-apples).
3. Stop cloning semantic JSON in layout/render decode paths (done for the main `merman-render`
   layout decoders via `T::deserialize(&Value)`).
4. Consider a lightweight lexer + hand-rolled parser for the hot subset where it measurably pays off.

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
