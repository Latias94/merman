# Performance Milestones

This document records completed performance work and the current prioritized backlog.
For the actively maintained plan and targets, see `docs/performance/PERF_PLAN.md`.

## Current snapshot (2026-02-17)

Latest committed reports:
- Stage attribution: `docs/performance/spotcheck_2026-02-17.md`
- End-to-end comparison: `docs/performance/COMPARISON.md`

Key takeaways:
- `render` remains the most consistent cross-diagram gap (spotcheck gmean `~1.94x`).
- `mindmap` and `architecture` are the main layout gaps.
- `class` is already faster end-to-end than mmdr on the medium canary.

## Completed (selected)

### 2026-02-16 .. 2026-02-17

- `perf(architecture): speed up group separation` (`9439199a`)
- `perf(class): speed up edge data-points encoding` (`29a5bdaf`)
- `perf(class): reduce render fixed-cost` (`6a5bd4c8`)
- `bench(merman): add mindmap layout stress` (`7cc41c8b`)

## Next (prioritized)

### P0: SVG emission fixed-cost (multi-diagram)

Goal:
- Bring `render` ratios down materially on medium fixtures without changing output.

Canaries:
- `render/flowchart_medium`
- `render/class_medium`
- `render/mindmap_medium`
- `render/architecture_medium`

Gate:
- `cargo nextest run -p merman-render`

### P1: Mindmap layout (manatee COSE)

Goal:
- Reduce `layout/mindmap_medium` ratio and improve end-to-end `mindmap_medium`.

Approach:
- Focus on safe Rust + representation changes first (no unsafe in `manatee`).
- Use timing toggles (`MANATEE_COSE_TIMING=1`, `MERMAN_MINDMAP_LAYOUT_TIMING=1`) when validating hypotheses.

### P2: Architecture layout + render

Goal:
- Reduce both `layout/architecture_medium` and `render/architecture_medium`.

Approach:
- Keep cutting string-keyed maps/clones in layout.
- Treat render emission as part of the canary, not an afterthought.

### P3: Parse (targeted)

Goal:
- Only optimize parse when it is the limiting stage for chosen canaries (especially tiny fixtures).

