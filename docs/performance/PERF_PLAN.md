# Performance Plan (Targets → Milestones → Work Items)

This document is the **actionable performance plan** for `merman`.
It is intentionally **fixture-driven** and **stage-attributed** (parse/layout/render/end-to-end).

## Baseline (2026-02-16)

Stage spot-check vs `repo-ref/mermaid-rs-renderer` (mmdr):

- Command:
  - `python tools/bench/stage_spotcheck.py --fixtures flowchart_medium,mindmap_medium,architecture_medium,class_medium,state_medium,sequence_medium --sample-size 50 --warm-up 2 --measurement 3 --out target/bench/stage_spotcheck.baseline_2026-02-16.md`
- Report:
  - `target/bench/stage_spotcheck.after_merge_main_2026-02-16.md` (local, not committed)
- Stage gmeans (ratios, `merman / mmdr`):
  - `parse`: `1.32x`
  - `layout`: `0.87x`
  - `render`: `1.61x`
  - `end_to_end`: `0.90x`

## Latest Update (2026-02-16)

- Landed:
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

Outliers worth optimizing:

- `flowchart_medium end_to_end`: `1.42x` (`render 1.62x`, `layout 1.05x`)
- `mindmap_medium end_to_end`: `1.60x` (`layout 2.02x`, `render 1.25x`)
- `architecture_medium end_to_end`: `2.01x` (`layout 3.35x`, `render 1.97x`)
- Render fixed-cost remains a consistent theme (even when end-to-end is competitive on other canaries).

## Root Cause Map (what is actually slow)

1. **SVG emission fixed-cost**
   - Many hot paths still build intermediate `String`s (`format!`, `to_string`, joins) inside loops.
   - Style compilation and attribute escaping are frequently repeated for identical payloads.
2. **Mindmap + Architecture layout fixed-cost**
   - COSE/FCoSE are sensitive to representation (dense indices vs string-key maps) and per-iteration allocations.
   - Even when absolute times are small, ratio outliers usually indicate avoidable per-call overhead.
3. **Parse fixed-cost (mostly tiny/medium canaries)**
   - Some diagrams still pay noticeable preprocessing/scanning overhead even for short inputs.

## Milestones (ordered by ROI)

### P0 — Flowchart end-to-end (highest visibility)

Targets (spotcheck ratios):

- `end_to_end/flowchart_medium`: `<= 1.15x` (from `1.42x`)
- `render/flowchart_medium`: `<= 1.35x` (from `1.62x`)

Work items:

- Prioritize flowchart SVG emission: remove `format!`/temporary `String`s in node/edge hot loops, reuse per-diagram scratch buffers, and avoid repeated escaping/style compilation.
- Re-run the canary after each landed change:
  - `python tools/bench/stage_spotcheck.py --fixtures flowchart_medium --sample-size 50 --warm-up 2 --measurement 3`

Correctness gate:

- `cargo nextest run -p merman-render` (layout + svg parity tests must remain green).

### P1 — Reduce SVG emission overhead (multi-diagram)

Targets:

- `render` gmean: `<= 1.35x` (from `1.61x`)
- `render/class_medium`: `<= 2.00x` (from `2.35x`)
- `render/state_medium`: `<= 2.00x` (from `2.64x`)
- `render/architecture_medium`: `<= 1.60x` (from `1.97x`)

Work items:

- Introduce per-diagram render scratch (reused `String`s / small `Vec`s) and remove `format!`/temporary `String`s from node/edge loops.
- Cache/derive once per render call:
  - escaped `diagram_id`
  - compiled marker ids
  - class→style compilation results for common class sets

### P2 — Mindmap layout (COSE) cost reduction (keep deterministic)

Targets:

- `layout/mindmap_medium <= 1.60x` (from `2.02x`)
- `end_to_end/mindmap_medium <= 1.35x` (from `1.60x`)

Work items:

- Use `MANATEE_COSE_TIMING=1` to identify the dominant COSE sub-stages on the canary fixture.
- Keep the public API stable, but move hot loops to dense indices + preallocated scratch buffers.
- Reduce per-iteration allocations in spring/repulsion updates (reuse `Vec`s, avoid rebuilding maps).

### P3 — Architecture fixed-cost (layout + render)

Targets:

- `end_to_end/architecture_medium <= 1.50x` (from `2.01x`)
- `layout/architecture_medium <= 2.50x` (from `3.35x`)

Work items:

- Convert typed layout passes to index-based queues/adjacency (avoid `HashMap<String, ...>` in hot paths).
- Apply the same render fixed-cost reductions as P1 (architecture is still render-heavy in ratio terms).

### P4 — Parse fixed-cost (targeted, not a blanket rewrite)

Targets:

- `parse/state_medium <= 1.80x` (from `2.09x`)
- `parse/sequence_medium <= 1.40x` (from `1.66x`)

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
