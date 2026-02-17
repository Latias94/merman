# Performance Plan (Targets → Milestones → Work Items)

This document is the **actionable performance plan** for `merman`.
It is intentionally **fixture-driven** and **stage-attributed** (parse/layout/render/end-to-end).

## Baseline (2026-02-17)

Stage spot-check vs `repo-ref/mermaid-rs-renderer` (mmdr):

- Command:
  - `python tools/bench/stage_spotcheck.py --fixtures flowchart_medium,mindmap_medium,architecture_medium,class_medium,state_medium,sequence_medium --sample-size 50 --warm-up 2 --measurement 3 --out target/bench/stage_spotcheck.after_merge_main_local_2026-02-17.md`
- Report:
  - `target/bench/stage_spotcheck.after_merge_main_local_2026-02-17.md` (local, not committed)
- Stage gmeans (ratios, `merman / mmdr`):
  - `parse`: `1.18x`
  - `layout`: `0.78x`
  - `render`: `1.53x`
  - `end_to_end`: `1.01x`

## Latest Update (2026-02-17)

- Landed:
  - `bench(merman): prevent layout bench elision` (`17e1ebbd`)
    - The `layout/*` Criterion benches now compute a cheap, layout-dependent digest (read a small
      subset of node/edge coordinates) to prevent LLVM from optimizing away expensive layout work.
  - `bench(merman): add architecture layout stress` (`6c708c07`)
    - Adds a layout-only stress bench for Architecture:
      - `cargo bench -p merman --features render --bench architecture_layout_stress`
    - Local baseline saved (not committed): `--save-baseline arch_layout_base`.
  - `perf(manatee): use fxhash in graph+fcose` (`0f5fa791`)
    - Swaps a few small-graph fixed-cost structures (`BTree*`) to `FxHash*` in validate/FCoSE.
  - `perf(manatee): skip root compound map when noop` (`5d728ebf`)
    - Avoids building the root-compound membership map unless we actually observe multiple root
      compounds (compound separation is a no-op for 0/1 roots).
  - `perf(manatee): avoid id clones in layout output` (`22a05bb4`)
    - Moves node ids into the final `positions` map instead of cloning them (COSE + FCoSE).
  - `perf(flowchart): reduce render hot-path overhead` (`d295a53`)
    - Avoids per-label lowercase allocation when detecting `<img` in flowchart labels.
    - Adds a fast-path in `maybe_snap_data_point_to_f32` to skip expensive bit-level checks for
      the common case.
  - `perf(manatee): cut small-graph repulsion overhead` (`7e68646`)
    - Reduces COSE-Bilkent repulsion fixed-cost by reusing per-pair center/half-size deltas and
      inlining the non-overlap force computation in the hot O(n^2) loop.
    - Spotcheck (`mindmap_medium`):
      - Report: `target/bench/stage_spotcheck.mindmap_medium.after_repulsion_inline_2026-02-16.md` (local, not committed)
      - Ratio (`merman / mmdr`): `layout 1.79x` (from `2.02x` baseline)
  - `perf(curve): specialize path emission` (`fe3aa4b`)
    - Splits SVG path emission into `no-bounds` vs `with-bounds` fast paths so the hot render-only
      case (no tight bounds needed) avoids per-command optional bound bookkeeping.
    - Affects `curveBasis` / `curveLinear` emission used by flowchart/class/ER paths.
  - `perf(manatee): cut COSE repulsion loop overhead` (`e24d9eb`)
    - Caches per-node half sizes and reuses `abs(dx/dy)` inside the spring embedder's O(n^2)
      repulsion loop.
    - Local A/B (`layout/mindmap_medium`, `cargo bench` exact, 50 samples / 2s warmup / 3s measurement):
      - `118.43µs` → `112.76µs` (~`-4.8%`)
  - `perf(mindmap): avoid HashMap in edge build` (`17a18aa`)
    - Builds mindmap `LayoutEdge` endpoints via `id -> index -> nodes[idx].(x,y)` instead of
      allocating a `HashMap<&str, (f64,f64)>` each layout call.
    - Spotcheck (`mindmap_medium`):
      - Report: `target/bench/stage_spotcheck.mindmap_medium.after_edge_build_ix_2026-02-16.md` (local, not committed)
      - Ratio (`merman / mmdr`): `layout 1.66x` (from `2.23x` in the prior rerun)
  - `refactor(architecture): borrow model view for layout` (`dea7efb`)
    - Unifies JSON vs typed architecture layout input behind a borrowed view (`&str` ids, `Option<char>` dirs).
    - This is primarily a prerequisite for the next Architecture fixed-cost reductions (dense indices + fewer
      string-keyed maps), not a standalone performance win.
  - `perf(architecture): speed up group separation` (`9439199a`)
    - Speeds up the Architecture post-layout `group_separation` step by reducing fixed-cost overhead:
      - borrow ids (`&str`) instead of cloning `String`s in relations
      - use `FxHash*` instead of `BTree*` for small fixed sets
      - avoid debug-string based sorting/dedup
      - pre-index group members and cache bboxes during separation iterations
    - Architecture stress bench (`cargo bench -p merman --features render --bench architecture_layout_stress -- --baseline arch_layout_base`):
      - `layout_stress/architecture_reasonable_height_layout_x50`: ~`-33%` (local)
  - `perf(class): speed up edge data-points encoding` (`29a5bdaf`)
    - ClassDiagram edges encode `data-points` as Base64(JSON.stringify(points)). Switch from
      `serde_json::to_writer` to our parity-oriented `json_stringify_points_into` helper to reduce
      allocation + formatting overhead and better match upstream `JSON.stringify` number formatting.
  - `perf(class): reduce render fixed-cost` (`6a5bd4c8`)
    - Lazily build the note sanitizer config (only if notes exist), avoid a few per-node allocations,
      and add finer-grained class render timings to guide future render work.
- Latest canary (faster triage parameters):
  - Command:
    - `python tools/bench/stage_spotcheck.py --fixtures flowchart_medium,mindmap_medium,architecture_medium,class_medium,state_medium,sequence_medium --sample-size 20 --warm-up 1 --measurement 2 --out target/bench/stage_spotcheck.canary_after_flowchart_opt_2026-02-16.md`
  - Report:
    - `target/bench/stage_spotcheck.canary_after_flowchart_opt_2026-02-16.md` (local, not committed)
  - Stage gmeans (ratios, `merman / mmdr`):
    - `parse`: `1.56x`
    - `layout`: `0.93x`
    - `render`: `1.62x`
    - `end_to_end`: `0.86x`

Latest spotcheck (local, not committed, 2026-02-17):

- Command:
  - `python tools/bench/stage_spotcheck.py --fixtures flowchart_medium,mindmap_medium,architecture_medium,class_medium,state_medium,sequence_medium --sample-size 25 --warm-up 2 --measurement 2 --out target/bench/stage_spotcheck.after_group_sep_opt_2026-02-17.md`
- Report:
  - `target/bench/stage_spotcheck.after_group_sep_opt_2026-02-17.md`

Outliers worth optimizing (from the spotcheck above):

- `architecture_medium end_to_end`: `1.88x` (`layout 2.08x`, `render 2.46x`)
- `mindmap_medium end_to_end`: `1.42x` (`layout 1.74x`, `render 1.24x`)
- `render/class_medium`: `2.36x`
- `render/state_medium`: `2.42x`
- `render/flowchart_medium`: `2.21x`
- `parse/state_medium`: `3.29x`

## Root Cause Map (what is actually slow)

1. **Manatee (COSE/FCoSE) fixed-cost + iteration cost**
   - Medium fixtures are sensitive to avoidable per-call overhead (allocation + mapping).
   - Compound/group handling can do no-op bookkeeping in the common 0/1-root cases.
2. **SVG emission fixed-cost**
   - Many hot paths still build intermediate `String`s (`format!`, `to_string`, joins) inside loops.
   - Style compilation and attribute escaping are frequently repeated for identical payloads.
   - Some renderers also pay for Mermaid-style metadata (e.g. `data-points` base64 JSON) which is
     correctness-critical for strict parity but can dominate micro-bench render fixed cost.
3. **Parse fixed-cost (targeted)**
   - Still noticeable on a few fixtures (e.g. `state_medium`), but not the global priority.

## Measurement Stack (use the right tool)

- Use `tools/bench/stage_spotcheck.py` for **stage attribution** (parse/layout/render/end-to-end).
- Use `tools/bench/compare_mermaid_renderers.py` for **end-to-end regression tracking** over a
  filtered set of fixtures.
  - These two tools can legitimately disagree on “overall ratio” if the fixture set differs.
  - Prefer `stage_spotcheck` when deciding *where* to optimize next; prefer `compare_*` when
    deciding *whether* we materially improved a user-visible pipeline canary.

## Milestones (ordered by ROI)

### P0 — Reduce SVG emission overhead (multi-diagram)

Targets:

- `render` gmean: `<= 1.50x` (from `1.97x`)
- `render/class_medium`: `<= 2.00x` (from `2.36x`)
- `render/state_medium`: `<= 1.80x` (from `2.42x`)
- `render/architecture_medium`: `<= 2.00x` (from `2.46x`)
- `render/flowchart_medium`: `<= 1.80x` (from `2.21x`)

Work items:

- Introduce per-diagram render scratch (reused `String`s / small `Vec`s) and remove `format!`/temporary `String`s from node/edge loops.
- Cache/derive once per render call:
  - escaped `diagram_id`
  - compiled marker ids
  - class→style compilation results for common class sets

Correctness gate:

- `cargo nextest run -p merman-render` (layout + svg parity tests must remain green).

### P1 — Manatee layout fixed-cost (Architecture + Mindmap)

Targets (spotcheck ratios; validate with stable parameters):

- `end_to_end/architecture_medium`: `<= 1.50x` (from `1.88x`)
- `layout/architecture_medium`: `<= 1.80x` (from `2.08x`)
- `end_to_end/mindmap_medium`: `<= 1.30x` (from `1.42x`)
- `layout/mindmap_medium`: `<= 1.50x` (from `1.74x`)

Work items:

- Keep cutting no-op work in COSE/FCoSE (compound bookkeeping, maps/clones, output conversion).
- Reduce FCoSE fixed-cost in constraints + spectral init (small graphs first; keep deterministic).
- Reduce COSE iteration cost similarly (repulsion/transform/output).
- Move the hottest loops toward dense indices + preallocated scratch where feasible.
- Validate with dedicated benches:
  - `python tools/bench/stage_spotcheck.py ...`
  - `cargo bench -p merman --features render --bench architecture_layout_stress -- --baseline arch_layout_base`

### P2 — Parse fixed-cost (targeted, not a blanket rewrite)

Targets:

- `parse/state_medium <= 2.00x` (from `3.29x`)
- `parse/sequence_medium <= 1.15x` (from `1.17x`)

Work items:

- Add/extend parse micro-timing (`MERMAN_PARSE_TIMING=1`) and optimize the worst sub-stage.
- Prefer fast-path + fallback patterns for a common subset only when it measurably moves spotcheck.

## Design Decisions (avoid premature rewrites)

### Should we adopt a parser crate (e.g. `nom`)?

Not as a default performance move.
`parse` is **not** the dominant global bottleneck in the baseline; most ROI is in **render + specific layouts**.
Adopt a parser crate only if it improves correctness/maintainability and we can show measurable wins on canaries.

### Should we switch graph crates?

Not first.
Most hotspots are about **algorithmic complexity** and **representation** (dense indices, fewer allocations).
We should keep the current public graph surface stable and introduce internal dense representations where needed.

## Operating Rules

- Always keep a local spotcheck report under `target/bench/` for each milestone landing (do not commit).
- Prefer fixture-driven changes: pick a canary, change one thing, re-run spotcheck.
- Sync with local `main` frequently to avoid long-lived divergence.
