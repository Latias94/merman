# Performance Milestones (Triage → Targets → Work Items)

This document tracks the performance plan for `merman` with concrete, measurable milestones.
It is intentionally fixture-driven and stage-attributed (parse/layout/render/end-to-end).

## Current Status (2026-02-16)

### Baseline (post-merge local `main`)

Stage spot-check vs `repo-ref/mermaid-rs-renderer` (mmdr):

- Command:
  - `python tools/bench/stage_spotcheck.py --fixtures flowchart_medium,class_medium,state_medium,sequence_medium,mindmap_medium,architecture_medium --sample-size 20 --warm-up 1 --measurement 1 --out target/bench/stage_spotcheck.latest.md`
- Report:
  - `target/bench/stage_spotcheck.latest.md` (not committed)
- Stage gmeans (ratios, `merman / mmdr`):
  - `parse`: `1.38x`
  - `layout`: `0.88x`
  - `render`: `1.99x`
  - `end_to_end`: `0.82x`
- Interpretation:
  - **End-to-end is already competitive overall**, but we have **concentrated gaps**:
    - `flowchart_medium end_to_end`: `1.21x` (render-heavy; `render 1.91x`, `parse 1.40x`)
    - `mindmap_medium end_to_end`: `1.69x` (layout+render; `layout 2.51x`, `render 2.15x`)
    - `architecture_medium end_to_end`: `2.24x` (layout+render; `render 3.59x`)
  - Layout is **not** the main global bottleneck right now; **SVG emission fixed-cost** is.

### Latest (after syncing local `main`)

- Report:
  - `target/bench/stage_spotcheck.latest_after_mindmap_layout_md_fast.md` (not committed)
- Stage gmeans (ratios, `merman / mmdr`):
  - `parse`: `1.48x`
  - `layout`: `1.07x`
  - `render`: `1.66x`
  - `end_to_end`: `0.80x`
- Remaining end-to-end outliers (ratios):
  - `mindmap_medium end_to_end`: `1.94x` (`layout 2.60x`, `render 1.51x`)
  - `architecture_medium end_to_end`: `2.47x` (`layout 3.88x`, `render 2.32x`)

### Stage Attribution Snapshot (canaries)

Stage spot-check (vs `repo-ref/mermaid-rs-renderer`) shows the remaining gap is *multi-source*:

- `render` is still behind on several diagram types (flowchart/class/mindmap/architecture),
- `parse` fixed-cost dominates `*_tiny` canaries (state/sequence),
- `layout` is still a dominant ratio gap for `architecture` and `mindmap` (absolute times are small).

Latest local spotcheck (2026-02-16):

- Command:
  - `python tools/bench/stage_spotcheck.py --fixtures flowchart_medium,class_medium,architecture_medium,mindmap_medium,flowchart_tiny,state_tiny,sequence_tiny --sample-size 20 --warm-up 1 --measurement 1`
- Report:
  - `docs/performance/spotcheck_2026-02-16.md`
- Stage gmeans (ratios, `merman / mmdr`):
  - `parse`: `1.81x`
  - `layout`: `1.15x`
  - `render`: `2.10x`
  - `end_to_end`: `1.21x`
- Notable outliers (ratios):
  - `render/class_medium`: `3.57x` (SVG emission fixed overhead)
  - `render/flowchart_medium`: `2.20x` (SVG emission fixed overhead)
  - `parse/state_tiny`: `4.04x` (tiny parse fixed overhead)
  - `parse/sequence_tiny`: `3.79x` (tiny parse fixed overhead)
  - `layout/architecture_medium`: `2.91x` (fixed-cost; absolute time is µs-scale)
  - `layout/mindmap_medium`: `2.42x` (COSE still expensive)
- Notes:
  - Mindmap layout: we now use an indexed COSE entrypoint (avoid building `BTreeMap<String, Point>`).
  - Architecture render: we avoid cloning `effective_config` when only the sanitize config is needed.
  - Mindmap render (label emission): we reduced per-label `String` churn and added a conservative
    markdown fast-path for plain-text labels.
    - Spotcheck: `target/bench/stage_spotcheck.mindmap_after_iconfast.long.md` (not committed)
    - Ratios (`mindmap_medium`): `parse 1.57x`, `layout 2.49x`, `render 1.31x`, `end_to_end 2.00x`
    - Interpretation: mindmap is still **layout-dominated** (COSE). Render improved materially
      vs the original ~2x gap, but correctness fixes that apply Markdown parsing to more labels
      increased render fixed-cost again.
  - Mindmap layout (label measurement): add a conservative fast-path for "plain text" markdown so
    layout doesn't pay pulldown-cmark twice per node (wrapped + unwrapped).
    - Spotcheck: `target/bench/stage_spotcheck.mindmap_after_layout_md_fast.long.md` (not committed)
    - Ratios (`mindmap_medium`): `parse 1.34x`, `layout 2.27x`, `render 1.09x`, `end_to_end 1.67x`
    - Interpretation: on the canary fixture, the remaining mindmap gap is now mostly COSE iterations
      (repulsion-heavy spring), not markdown parsing overhead.

- Tiny canaries (after Dagre-ish tiny fast-path):
  - `docs/performance/spotcheck_2026-02-15_tiny.md`
  - gmeans: `parse 2.33x`, `layout 1.09x`, `render 2.41x`, `end_to_end 1.54x`
  - Interpretation: **tiny layout fixed-cost is now mostly solved**; the remaining tiny gap is
    dominated by **parse + SVG render fixed overhead**.
  - Update (after class typed model routing):
    - `docs/performance/spotcheck_2026-02-15_tiny_after_typed.md`
    - gmeans: `parse 1.89x`, `layout 0.88x`, `render 2.13x`, `end_to_end 1.21x`
    - Interpretation: tiny `end_to_end` is now close to parity; remaining outliers are
      `parse/sequence_tiny` and `render/class_tiny`.

- Latest combined spotcheck report:
  - `docs/performance/spotcheck_2026-02-15.md` (`tools/bench/stage_spotcheck.py`, 10 samples / 1s warmup / 5s measurement)
  - Canary set (`flowchart_medium,class_medium,sequence_medium,mindmap_medium,architecture_medium`):
    - `parse` gmean: `1.49x`
    - `layout` gmean: `1.14x`
    - `render` gmean: `1.77x`
    - `end_to_end` gmean: `1.26x`
  - Notable outliers:
    - `architecture_medium`: `layout 4.26x`, `render 2.51x`, `end_to_end 4.16x` (absolute times are small; ratio is large)
    - `mindmap_medium`: `layout 2.67x`, `end_to_end 1.93x`
    - `class_medium`: `render 3.70x` (despite `layout 0.38x` and `end_to_end 0.62x`)
    - `flowchart_medium`: `render 1.75x` (despite `layout 1.04x` and `end_to_end 0.87x`)
  - Update (after class typed model routing):
    - `docs/performance/spotcheck_2026-02-15_class_medium_after_typed.md`
    - `class_medium` now shows `parse 1.44x`, `render 2.57x`, `end_to_end 0.46x` in that spotcheck.

Local update (same date, after merging local `main` + architecture pipeline refactors):

- `docs/performance/spotcheck_2026-02-15_after_arch_refactors.md`
- Canary set (`flowchart_medium,class_medium,sequence_tiny,mindmap_medium,architecture_medium,class_tiny`):
  - `parse` gmean: `1.35x`
  - `layout` gmean: `1.46x`
  - `render` gmean: `1.88x`
  - `end_to_end` gmean: `1.44x`

Local re-run notes (same date, after merging local `main` + small renderer refactors):

- Stage spot-check expanded to include `state_medium`:
  - `target/bench/stage_spotcheck.latest.md` (10 samples / 1s warmup / 5s measurement; includes `state_medium`)
  - Stage gmeans (6 fixtures): `parse ~1.50x`, `layout ~1.12x`, `render ~1.89x`, `end_to_end ~1.06x`.
- End-to-end canary comparison (8 fixtures: `*_tiny` + `*_medium` for flowchart/class/state/sequence):
  - `target/bench/COMPARISON.latest.after_state_and_sequence_fxhash.md` (10 samples / 1s warmup / 1s measurement; noisier)
  - Interpretation: **medium fixtures are already competitive** (gmean < `1.0x`), while **tiny fixtures still pay fixed overhead** (now ~`1.5x`, mostly parse+render).

Near-term priorities (updated plan):

1. **SVG emission fixed costs**: reduce class/flowchart/architecture render overhead (allocations + fmt).
   - Target: `render/class_medium <= 2.0x`, `render/flowchart_medium <= 1.5x`, `render/architecture_medium <= 2.0x`.
2. **Tiny parse fixed cost**: reduce `parse/state_tiny` and `parse/sequence_tiny`.
   - Target: `parse/state_tiny <= 2.5x`, `parse/sequence_tiny <= 2.5x`.
3. **Architecture layout fixed cost**: continue cutting `layout/architecture_*` without changing output.
   - Target: `layout/architecture_medium <= 2.5x` and `end_to_end <= 2.0x` in spotchecks.
4. **Mindmap layout**: continue reducing COSE cost with deterministic behavior.
   - Target: `layout/mindmap_medium <= 2.0x`.
5. **Flowchart medium canary (guardrail)**: keep `end_to_end/flowchart_medium` at `~parity` while we optimize other stages.

Root-cause direction:

- `sequence_tiny` is still primarily a parse fixed-overhead problem (detection helped, but parsing/lexing
  overhead dominates at the µs scale).
- `architecture` remains layout-heavy in ratio terms; the typed pipeline now avoids per-node edge cloning,
  but the remaining layout fixed-cost is still dominated by string-key maps and conservative BFS/component logic.
- `class` (and often `flowchart`) remain render-heavy: once layout is “good enough”, SVG emission and style
  resolution become the dominant opportunities.
- BK x-positioning (`dugong::position::bk::position_x`) was a measurable secondary hotspot after
  ordering. We now reuse the already-computed `layering` matrix from the Dagre-ish pipeline and use
  `&str`-based temporary maps plus an index-based block-graph pass to reduce hashing + allocation.
  On this machine, `DUGONG_DAGREISH_TIMING=1` for `flowchart_medium` dropped `position_x` from
  ~`1.0ms` → ~`0.66ms` (single-run signal; spotcheck variance still applies).
- Compound border segments (`compound_border` / `add_border_segments`) had an accidental O(n^2)
  implementation detail: border dummy ids were generated by scanning `"_bl1"`, `"_bl2"`, ... from
  scratch for *every* new node. Switching to a per-prefix monotonic counter keeps naming identical
  but makes id allocation amortized O(1). On `flowchart_medium`, `compound_border` dropped from
  ~`0.8–0.9ms` to ~`0.28–0.35ms` in local `DUGONG_DAGREISH_TIMING=1` spot runs.
- The nesting graph pass (`nesting_run`) had the same O(n^2) dummy id scan pattern for `_root` /
  `_bt` / `_bb`. We applied the same monotonic id strategy to keep it from scaling badly on large
  compound graphs.
- Flowchart viewport work had some pure overhead: we were generating an edge path `d` string and
  then re-parsing it to approximate `getBBox()`. We now compute cubic bounds during curve emission
  for the viewBox approximation, avoiding `svg_path_bounds_from_d(...)` in the flowchart viewbox
  prepass (still builds the `d`, but no longer parses it).
- Additional fast path: for default flowchart edge curves (`basis`), if the edge polyline bbox is
  already fully contained in the current viewBox bbox, skip the expensive cubic-extrema bounds solve.
- SVG emission still has measurable fixed overhead. Two recent low-risk wins:
  - Skip XML-escape scanning for `data-points` base64 payloads (and other known-safe path payloads).
  - Avoid repeated full-SVG rebuild passes for placeholder replacement:
    - patch initial `<svg ...>` attribute placeholders in-place when we can record their ranges (class),
    - otherwise do a single rebuild pass (state slow viewport finalize).
  - Avoid building the flowchart `<svg ...>` open tag via nested `format!(...)` + intermediate
    strings; write directly into the output buffer to reduce allocations.
- Flowchart edge path emission was allocating aggressively per edge (style joins + marker attribute
  formatting). We now write the style attribute and marker attrs directly into the output buffer to
  cut per-edge allocations (golden fixtures unchanged).
- Class diagram viewport work had the same pattern: we were accumulating `path_bounds` by parsing
  the emitted `d` strings. We now compute bounds during path emission for class edges + RoughJS-like
  strokes, and `path_bounds` micro-timing dropped from ~`O(50µs)` to ~`O(1–3µs)` for `class_medium`.
- `state_medium` render is dominated by leaf node work, especially RoughJS path generation and emit.
- `mindmap_medium` overall gap is now mostly layout (COSE port / bbox work) rather than parse.
- `architecture_medium` remaining gap is layout + SVG emission.
- Flowchart label metrics are now carried on `LayoutNode` for reuse in render, but are intentionally
  not serialized in layout golden snapshots (runtime-only fields).

Useful debug toggles:

- `MERMAN_RENDER_TIMING=1` (flowchart render stage attribution)
- `MERMAN_RENDER_TIMING=1` (mindmap + architecture coarse attribution)
- `MERMAN_PARSE_TIMING=1` (parse stage attribution: preprocess/detect/parse/sanitize)
- `MERMAN_FLOWCHART_LAYOUT_TIMING=1` (flowchart layout stage attribution)
- `MERMAN_MINDMAP_LAYOUT_TIMING=1` (mindmap layout coarse attribution: measure/manatee/edges/bounds)
- `MERMAN_ARCHITECTURE_LAYOUT_TIMING=1` (architecture layout coarse attribution: bfs/manatee/edges/bounds)
- `MANATEE_COSE_TIMING=1` (COSE-Bilkent internal breakdown: from_graph/flat_forest/radial/spring/transform/output + spring embedder)
- `DUGONG_DAGREISH_TIMING=1` (Dagre-ish pipeline stage attribution; shows `order` as dominant)
- `DUGONG_ORDER_TIMING=1` (ordering stage breakdown inside Dagre-ish pipeline)

### Class diagram (`class_medium`)

This fixture is useful as a counter-example:

- Spotcheck shows `layout` is already faster than `mmdr` (`~0.32x`), and end-to-end can be faster
  (`~0.48x` in the latest canary run), but `render` is still far behind (`~4x`).
- Implication: once we fix flowchart layout, **render optimizations will pay off across diagram
  types**, not only flowcharts.
- `MERMAN_RENDER_TIMING=1` now also emits a `[render-timing] diagram=classDiagram ...` line, so we
  can attribute class renderer hotspots without a profiler.

## Milestones

### M4 — Architecture layout: cut typed fixed-cost (Planned)

Goal: reduce `layout/architecture_*` fixed overhead without changing layout output.

Why: architecture is currently a large ratio outlier even though absolute times are small; this is a
good indicator of avoidable per-call overhead (allocation + hashing + string-key maps).

Work items (ordered by expected ROI):

1. Replace `HashMap<String, ...>` / `HashSet<String>` hot paths with dense indices (`usize`) and a
   single `id -> idx` map built once per call.
2. Represent adjacency with dense structures:
   - `Vec<IndexMap<&'static str, usize>>` (preserve insertion semantics) or
   - `Vec<[Option<usize>; 12]>` (dense direction-pair slots) if we can prove stable behavior.
3. Avoid cloning IDs in BFS queues and component maps; keep `usize` in the queue and store positions
   in `Vec<(i32, i32)>`.

Acceptance criteria:

- Spotcheck: `architecture_medium layout <= 3.0x` and `end_to_end <= 2.0x` (expect variance).
- Stress fixtures: measurable reduction in layout time and peak allocations (use `--features render`
  golden tests as correctness gate).

### M5 — Sequence parse: fast path + fallback (Planned)

Goal: reduce `parse/sequence_tiny` fixed overhead while preserving full parser correctness.

Approach:

- Implement a fast parser that covers the common subset (header + actor + message + note), and
  falls back to the existing parser on any unrecognized syntax.

Acceptance criteria:

- Spotcheck: `parse/sequence_tiny <= 2.5x` and `end_to_end/sequence_tiny <= 1.1x`.
- Golden fixtures: no SVG/JSON parity regressions.

### M6 — SVG emission: fewer allocations (Planned)

Goal: reduce render fixed overhead across diagrams, especially class/flowchart.

Work items:

- Continue migrating from `format!/String` churn to `write!` into a single output buffer.
- Reuse scratch `Vec<LayoutPoint>` / small `String` buffers per render call where safe.
- Cache style compilation / marker strings at the diagram scope.

Acceptance criteria:

- Spotcheck: `render/class_tiny <= 2.0x`, `render/class_medium <= 2.0x`, `render/flowchart_medium <= 1.5x`.

### M7 — Mindmap layout: COSE cost reduction (Planned)

Goal: reduce `layout/mindmap_medium` while keeping deterministic output.

Work items:

- (Done) Use an indexed COSE entrypoint to avoid string-key graph build and `BTreeMap<String, Point>` output.
- Keep pushing COSE repulsion costs down (grid-based neighbor filtering, stable iteration order).
- Reduce per-iteration allocations in the spring embedder; reuse scratch buffers.

Acceptance criteria:

- Spotcheck: `layout/mindmap_medium <= 2.0x` and `end_to_end/mindmap_medium` improves proportionally.

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

Primary target: keep `layout/flowchart_medium` at `<= 1.0x` vs `mmdr` without changing layout output.
Current: `~1.34x` on `flowchart_medium` in the latest canary run (spotcheck variance applies).

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
   - Recent `DUGONG_ORDER_TIMING=1` single-run timings for `flowchart_medium` are in the
     `build_layer_graph_cache ~0.7ms` and `order total ~2.3ms` range (variance applies).
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

- Spotcheck: `layout/flowchart_medium` stays at `<= 1.0x` and end-to-end drops proportionally.
- Layout micro-timing: `order` and especially `sweeps` drop materially (single-digit ms is a
  reasonable medium-term target for the medium fixture).

### M3 — State render: eliminate RoughJS cost (Partially done)

Goal: reduce `render/state_medium` without changing SVG output.

What we did:

- Cache RoughJS-generated path strings across render calls (global cache keyed by rough shape params),
  so Criterion iterations and server-style repeated renders avoid recomputing identical shapes.
- Render state diagrams directly from the typed `StateDiagramRenderModel` to avoid a
  `serde_json::Value` roundtrip (previously `to_value` + `from_value_ref`).

Acceptance criteria:

- Spotcheck: `render/state_medium` drops materially and consistently, not only after warm caches.

Status note:

- The cache helps, but `state_medium` is still far behind in `render` stage ratios. The next steps
  are to reduce per-leaf overhead (style resolution, SVG emission) and increase cache hit rate for
  RoughJS shapes.

### M4 — Positioning: reduce `position_x` overhead (Done)

Goal: after `order` is no longer dominant, reduce the next hotspot(s) without changing layout.

Work items:

- Reduce repeated graph traversals and hashing in Brandes-Koepf positioning.
- Consider an index-based temporary representation for positioning (same strategy as ordering),
  if hashing dominates.

Acceptance criteria:

- `position_x` time drops in `DUGONG_DAGREISH_TIMING=1` output for `flowchart_medium`.

Status note:

- Landed: `position_x_with_layering(...)` fast path that:
  - reuses pipeline `layering` (no duplicate `build_layer_matrix`),
  - keeps conflicts/alignment maps keyed by `&str` (no per-iteration `String` cloning),
  - replaces the block-graph `Graph<(), f64, ()>` construction with an index-based edge list.

### M5 — Render: close the multi-diagram gap (In progress)

Goal: reduce `render/*` ratios (flowchart + class + state) while preserving SVG output.

Work items (expected ROI order):

- (Done) Avoid “build SVG path `d` → parse `d`” viewport bounds patterns by computing bounds during
  path generation (flowchart + class edges).
- (Done) Reduce SVG finalize fixed overhead:
  - skip XML-escape scanning for known-safe `data-points` base64 payloads
  - reduce placeholder replacement overhead (in-place patching for class; state fast viewport skips the pass)
- (Done) Avoid allocating temporary `String` for common attribute escaping (Display-based attr escape
  in flowchart tooltip emission).
- (Done) Reduce per-edge allocations in flowchart edge path emission (style attribute + marker attrs).
- (In progress) Avoid repeated `String` growth by pre-sizing buffers and using a single `String`
  builder per SVG (especially for flowchart node emission).
- (In progress) Reduce per-node overhead for the hot path:
  - avoid cloning the base `TextStyle` when a node has no class/style overrides
  - pre-parse class text overrides once per render call (so we don't re-split decl strings per node)
- (Done) Reduce HTML label style overhead by extracting `color/font-*` fields during style compilation
  (avoid rescanning `label_style` strings per node/edge label).
- (Done) Reuse flowchart node label metrics computed during layout (avoid re-measuring HTML/markdown
  labels during render).
  - includes viewBox approximation for `delay`/`curv-trap` shapes (avoid re-measuring just to
    recover the "theoretical" label width)
- (Done) Avoid cloning `effective_config` JSON in hot render paths where the sanitize config is needed
  (pass `MermaidConfig` through the render API so diagram renderers can read config without deep-cloning).
- (Done) Reduce mindmap label render allocations:
  - write SVG label markup directly into the output buffer (avoid `format!`/temporary strings)
  - conservative markdown fast-path for plain-text labels (avoid pulldown + sanitize)
- (Planned) Cache per-diagram derived values that are reused many times (e.g. sanitized labels /
  class names), scoped to the render call to avoid cross-diagram leaks.

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

### M7 — Architecture: cut parse fixed-costs (Done)

Motivation (from spotcheck):

- `architecture_medium` was *dominated* by parse stage fixed costs (orders of magnitude vs `mmdr`),
  even on tiny inputs.

Work items (ordered by expected ROI):

1. Add a typed semantic model / typed render-model parse path for architecture (similar to flowchart). (Done)
2. Reduce preprocess overhead for short diagrams (avoid unnecessary allocations/scans). (Deferred; only if needed)
3. Audit the architecture parser for avoidable `String` cloning and map churn (prefer `&str`/interning). (Deferred; parse is no longer dominant)

Acceptance criteria:

- Spotcheck: `parse/architecture_medium` ratio drops by an order of magnitude without changing goldens.
  - Status: achieved (`parse` now < `1.0x` in local runs; layout/render remain behind).

## Fixture-driven Targets

We treat these fixtures as canaries:

- `flowchart_medium`: layout-heavy + many node labels.
- `state_medium`: render-heavy (shape generation / label handling).
- `class_medium`: end-to-end sanity (already close).
- `mindmap_medium`: layout-heavy (COSE port).
- `architecture_medium`: parse fixed-cost canary (tiny input).

When a milestone lands, record a new spotcheck report under `target/bench/` locally (do not commit)
and update this doc with the latest ratios.

## Non-goals (for now)

- “Switch graph crate” as a primary optimization strategy.
  - The dominant hotspots are algorithmic + representation issues in ordering/positioning; swapping
    a graph crate does not automatically remove the need for dense, index-based hot paths.
  - Prefer keeping the public graph API stable and introducing internal dense representations in
    performance-critical stages.
