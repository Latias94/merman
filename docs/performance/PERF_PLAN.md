# Performance Plan (Targets → Milestones → Work Items)

This document is the **actionable performance plan** for `merman`.
It is intentionally **fixture-driven** and **stage-attributed** (parse/layout/render/end-to-end).

## Baseline (2026-02-16)

Stage spot-check vs `repo-ref/mermaid-rs-renderer` (mmdr):

- Command:
  - `python tools/bench/stage_spotcheck.py --fixtures flowchart_medium,mindmap_medium,architecture_medium,class_medium,state_medium,sequence_medium --sample-size 50 --warm-up 2 --measurement 3 --out target/bench/stage_spotcheck.baseline_2026-02-16.md`
- Report:
  - `target/bench/stage_spotcheck.baseline_2026-02-16.md` (local, not committed)
- Stage gmeans (ratios, `merman / mmdr`):
  - `parse`: `1.37x`
  - `layout`: `0.88x`
  - `render`: `1.73x`
  - `end_to_end`: `0.92x`

Outliers worth optimizing:

- `mindmap_medium end_to_end`: `1.87x` (`layout 2.65x`, `render 1.35x`)
- `architecture_medium end_to_end`: `2.17x` (`layout 3.27x`, `render 2.32x`)
- Render fixed-cost is consistently behind:
  - `render/flowchart_medium`: `1.70x`
  - `render/class_medium`: `2.91x`
  - `render/state_medium`: `2.30x`

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

### P0 — Reduce SVG emission overhead (highest ROI)

Targets (spotcheck ratios):

- `render` gmean: `<= 1.35x` (from `1.73x`)
- `render/flowchart_medium`: `<= 1.35x` (from `1.70x`)
- `render/class_medium`: `<= 2.00x` (from `2.91x`)
- `render/state_medium`: `<= 1.80x` (from `2.30x`)

Work items:

- Introduce per-diagram render scratch (reused `String`s / small `Vec`s) and remove `format!`/temporary `String`s from node/edge loops.
- Cache/derive once per render call:
  - escaped `diagram_id`
  - compiled marker ids
  - class→style compilation results for common class sets
- Convert “build then escape” patterns into “write escaped pieces” patterns (`write!` + `escape_*_display`).

Correctness gate:

- `cargo nextest run -p merman-render` (layout + svg parity tests must remain green).

### P1 — Mindmap layout (COSE) cost reduction (keep deterministic)

Targets:

- `layout/mindmap_medium <= 1.80x` (from `2.65x`)
- `end_to_end/mindmap_medium <= 1.30x` (from `1.87x`)

Work items:

- Use `MANATEE_COSE_TIMING=1` to identify the dominant COSE sub-stages on the canary fixture.
- Keep the public API stable, but move hot loops to dense indices + preallocated scratch buffers.
- Reduce per-iteration allocations in spring/repulsion updates (reuse `Vec`s, avoid rebuilding maps).

### P2 — Architecture layout + render fixed-cost

Targets:

- `end_to_end/architecture_medium <= 1.50x` (from `2.17x`)
- `layout/architecture_medium <= 2.50x` (from `3.27x`)
- `render/architecture_medium <= 1.70x` (from `2.32x`)

Work items:

- Convert typed layout passes to index-based queues/adjacency (avoid `HashMap<String, ...>` in hot paths).
- Apply the same render fixed-cost reductions as P0 (architecture is currently render-heavy in ratio terms).

### P3 — Parse fixed-cost (targeted, not a blanket rewrite)

Targets:

- `parse/state_medium <= 1.60x` (from `2.33x`)
- `parse/sequence_medium <= 1.50x` (from `1.90x`)

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

