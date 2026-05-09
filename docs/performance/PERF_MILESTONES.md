# Performance Milestones

This document records completed performance work and the current prioritized backlog.
For the actively maintained plan and targets, see `docs/performance/PERF_PLAN.md`.

## Current snapshot (2026-05-10)

Latest committed reports:
- Cross-repo stage attribution:
  `docs/performance/spotcheck_2026-05-09_mindmap_architecture_c4_stage_mmdr.md`
- Same-machine canary pipeline run:
  `docs/performance/spotcheck_2026-05-10_mindmap_architecture_canary_pipeline_long.md`
- End-to-end comparison: `docs/performance/COMPARISON.md`

Key takeaways:
- `architecture_medium` and `mindmap_medium` remain the main canaries.
- The latest local canary run shows strong layout-stage improvement for both canaries, and the
  longer sample is now the default local checkpoint.
- The latest cross-repo stage attribution still leaves Architecture layout as the largest observed
  gap; re-run it after the next Architecture layout cleanup.
- `parse/mindmap_medium` showed a small local regression band in the short canary run, so validate
  it with a longer run before treating parse as the limiting stage.

## Completed (selected)

### 2026-02-16 .. 2026-02-17

- `perf(architecture): speed up group separation` (`9439199a`)
- `perf(class): speed up edge data-points encoding` (`29a5bdaf`)
- `perf(class): reduce render fixed-cost` (`6a5bd4c8`)
- `bench(merman): add mindmap layout stress` (`7cc41c8b`)
- `perf(sequence): avoid config deep clone in render` (`cb1d0a67`)
- `bench(merman): reduce end_to_end harness overhead` (`4d17f31d`)
- `perf(mindmap): reuse id map for edges` (`96343662`)
- `perf(architecture): avoid config clone in json render` (`e216928c`)
- `bench(tools): add long preset and skip mermaid-js` (`88053edf`)

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
- Re-run `spotcheck_2026-05-10_mindmap_architecture_canary_pipeline.md` with a longer Criterion
  preset before claiming durable layout movement.

### P2: Architecture layout + render

Goal:
- Reduce both `layout/architecture_medium` and `render/architecture_medium`.

Approach:
- Keep cutting string-keyed maps/clones in layout.
- Treat render emission as part of the canary, not an afterthought.
- Re-run the merman-vs-mmdr stage spotcheck after layout changes, because the local canary only
  proves same-machine movement.

### P3: Parse (targeted)

Goal:
- Only optimize parse when it is the limiting stage for chosen canaries (especially tiny fixtures).
