# Performance Plan (Targets -> Milestones -> Work Items)

This document is the actionable performance plan for `merman`.
It is fixture-driven and stage-attributed (parse/layout/render/end-to-end).

For the day-to-day workflow (commands, presets, and reporting conventions), see:
`docs/performance/PERF_PLAYBOOK.md`.

## Baseline (2026-02-17)

We track two complementary views against `repo-ref/mermaid-rs-renderer` (mmdr):

1) **End-to-end canaries** (throughput view)
- Report: `docs/performance/COMPARISON.md`
- Filter used in the latest report: `end_to_end/(flowchart_medium|class_medium|mindmap_medium|architecture_medium)`
- Observed ratios (`merman / mmdr`, mid estimate):
  - `end_to_end/flowchart_medium`: `0.9x` (faster)
  - `end_to_end/class_medium`: `0.4x` (faster)
  - `end_to_end/mindmap_medium`: `2.0x` (slower)
  - `end_to_end/architecture_medium`: `2.5x` (slower)

2) **Stage attribution** (what moved view)
- Report: `docs/performance/spotcheck_2026-02-17_after_bench_fix.md`
- Stage gmeans (`merman / mmdr`, geometric mean over the fixture set):
  - `parse`: `1.07x`
  - `layout`: `1.11x`
  - `render`: `1.46x`
  - `end_to_end`: `1.48x`

### What is actually slow (root-cause map)

- **Architecture + mindmap end-to-end are the main gaps** (canaries show ~`2.0–2.5x`).
- **Render fixed-cost is still behind** on the medium canaries (spotcheck `render` gmean `~1.46x`).
- **Layout is not uniformly slow**:
  - `class_medium layout` is already faster than mmdr (large margin).
  - `mindmap_medium` and `architecture_medium` remain the main layout gaps.
- **Tiny fixtures are dominated by fixed costs** (allocations + per-render setup); keep an eye on
  deep JSON/config clones and benchmark harness overhead.

## Operating constraints

- Correctness gate: `cargo nextest run -p merman-render`
- `manatee` forbids unsafe code (`#![forbid(unsafe_code)]`), so hot-loop wins must be safe Rust or
  algorithmic/representation changes.
- Prefer deterministic changes: benchmark stability and golden fixtures matter.

## Milestones (prioritized)

### M0: Keep measurement honest (guardrails)

Goal: fast feedback without chasing noise.

- Use `tools/bench/stage_spotcheck.py` to decide *where* to optimize.
- Use `tools/bench/compare_mermaid_renderers.py` to decide *whether* we improved end-to-end canaries.
- Prefer `--preset long` for canary decisions (reduces noise at µs-scale).
- For micro-scale work, prefer stress benches (batching work per iteration) and stable parameters:
  - `--sample-size 50 --warm-up-time 2 --measurement-time 3`

Deliverables:
- At least one committed spotcheck per major performance checkpoint (under `docs/performance/`).
- Keep `end_to_end/*` benches lightweight (avoid Criterion harness overhead that dominates µs-scale fixtures).

### M1: Reduce SVG emission overhead (multi-diagram, highest ROI)

Targets (spotcheck ratios, medium fixtures):
- `render` gmean: `<= 1.50x` (from `1.94x`)
- Canaries: `render/flowchart_medium`, `render/class_medium`, `render/mindmap_medium`, `render/architecture_medium`

Work items:
- Remove per-node/per-edge temporary `String` creation in hot loops (`format!`, `to_string`, joins).
- Introduce per-render scratch buffers (reused `String`s and small `Vec`s) for renderers with many loops.
- Cache per-render derived values (escaped IDs, compiled style fragments, marker IDs).
- Prefer write-into-buffer helpers over building intermediate strings.

Correctness gate:
- `cargo nextest run -p merman-render`

### M2: Mindmap layout (manatee COSE) - reduce the “small graph + many iters” cost

Targets (stage spotcheck):
- `layout/mindmap_medium`: `<= 2.0x` (from `2.93x` in `docs/performance/spotcheck_2026-02-17.md`)
- `end_to_end/mindmap_medium`: `<= 1.3x` (from `2.16x` in the same report)

Work items (safe + deterministic):
- Reduce fixed-cost around COSE calls (allocation and mapping) in `merman-render/src/mindmap.rs`.
- Add/keep fine-grained timing toggles (`MANATEE_COSE_TIMING=1`, `MERMAN_MINDMAP_LAYOUT_TIMING=1`) to
  confirm that changes hit repulsion/spring rather than shifting overhead.
- Consider algorithmic changes only with strict gates:
  - early-exit criteria, iteration caps, or specialized tree layout
  - must preserve golden fixtures and deterministic placements

### M3: Architecture layout - reduce fixed-cost and post-layout passes

Targets (stage spotcheck):
- `layout/architecture_medium`: `<= 2.5x` (from `3.86x`)
- `render/architecture_medium`: `<= 2.0x` (from `2.45x`)

Work items:
- Keep pushing “dense indices + borrowed ids + fewer maps” through the layout pipeline.
- Treat SVG emission as a first-class part of the architecture gap (stage spotcheck shows both layout and render).
- Expand stress benches when a sub-step is too fast/noisy to move reliably.

### M4: Parse (targeted) - only when it wins canaries

Targets:
- Focus on fixtures where parse dominates at the µs scale (tiny fixtures, or known outliers).

Work items:
- Add micro-timing inside parsing only after the spotcheck shows parse as the limiting stage.
- Prefer fast-path + fallback patterns rather than a full parser rewrite.

## Design decisions (avoid premature rewrites)

### Should we adopt a parser crate (e.g. `nom`)?

Not as a default performance move.
Today’s biggest, cross-diagram gap is `render`, and the largest layout gaps are `mindmap` and
`architecture`.

We should consider a parser crate only if:
- correctness/maintainability is a problem in the current parser, and
- a prototype shows measurable wins on parse-heavy canaries.

### Should we switch graph crates?

Not first.
The hot problems are dominated by algorithmic and representation choices (dense indices, fewer
allocations, fewer string-keyed maps), and we can usually get the same wins without a public crate swap.
