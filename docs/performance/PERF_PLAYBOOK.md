# Performance Playbook (How we measure, decide, and record)

This doc is a practical checklist for continuing performance work on `merman` without losing context.
It complements:

- `docs/performance/PERF_PLAN.md` (targets and prioritized backlog)
- `docs/performance/PERF_MILESTONES.md` (what shipped)
- `docs/performance/BENCHMARKING.md` (benchmark mechanics)

## Golden rule

Performance changes are only accepted if:

1) `cargo nextest run -p merman-render` stays green, and
2) we can explain *which stage moved* and *why* (parse/layout/render/end_to_end), and
3) results are recorded in a report under `docs/performance/`.

## Standard canaries

Use these fixture sets unless you have a strong reason to deviate:

- **Throughput (end-to-end)**: `flowchart_medium`, `class_medium`, `mindmap_medium`, `architecture_medium`
- **Attribution (stage)**: the same set as above

These cover:

- large-ish absolute runtime (`flowchart_medium`)
- “already good” signal (`class_medium`) to catch regressions
- the two current gaps (`mindmap_medium`, `architecture_medium`)

## Noise control presets (recommended)

Micro-benchmarks in the ~10–200µs range are inherently noisy. Prefer longer measurement windows when
tracking canaries or making “did it improve?” calls.

### Stage spot-check (triage)

```bash
python tools/bench/stage_spotcheck.py --preset long --fixtures flowchart_medium,class_medium,mindmap_medium,architecture_medium --out docs/performance/spotcheck_YYYY-MM-DD.md
```

Interpretation:

- Use `render/*` to guide “SVG emission” work.
- Use `layout/*` to guide graph/layout work.
- Use `end_to_end/*` as the “real” canary, but only after you’re confident the harness is fair.

### End-to-end comparison vs mmdr

```bash
python tools/bench/compare_mermaid_renderers.py --preset long --skip-mermaid-js --filter "end_to_end/(flowchart_medium|class_medium|mindmap_medium|architecture_medium)" --out docs/performance/COMPARISON.md
```

Notes:

- `--skip-mermaid-js` is recommended by default to avoid extra noise from puppeteer/Chromium.
- Keep `docs/performance/COMPARISON.md` up-to-date at major checkpoints; for ad-hoc experiments
  prefer writing to `target/bench/`.

## Diagnostic toggles (when you need root cause)

These env vars print coarse breakdowns for specific hotspots:

- `MERMAN_RENDER_TIMING=1` (SVG emission sub-timings where available)
- `MERMAN_MINDMAP_LAYOUT_TIMING=1` (mindmap: measure_nodes / manatee / build_edges / bounds)
- `MANATEE_COSE_TIMING=1` (manatee COSE-Bilkent internal timing)

Example:

```bash
$env:MERMAN_MINDMAP_LAYOUT_TIMING="1"; $env:MANATEE_COSE_TIMING="1"
cargo bench -p merman --features render --bench pipeline -- --exact layout/mindmap_medium --noplot --sample-size 30 --warm-up-time 2 --measurement-time 3
```

## Reporting conventions

Create one report per meaningful checkpoint:

- `docs/performance/spotcheck_YYYY-MM-DD.md` (stage attribution)
- `docs/performance/COMPARISON.md` (end-to-end canaries)

When a checkpoint is “in the middle of refactoring” and not ready to become the new baseline, write
to `target/bench/*.md` instead.

## Current focus (as of 2026-02-17)

- Close the end-to-end gaps on:
  - `mindmap_medium`
  - `architecture_medium`
- Reduce cross-diagram SVG emission fixed-cost (`render/*` ratios).

## Decision log (avoid churn)

- Parser framework migration (e.g. `nom`) is not a default perf move; revisit only if `parse/*`
  becomes the dominant canary stage.
- Switching graph crates is not the first lever; prioritize representation/indexing and allocation
  reduction in our own hot loops first.

## Next-step checklist

When starting a new optimization round:

1) Run the correctness gate: `cargo nextest run -p merman-render`.
2) Run attribution: stage spotcheck (`--preset long`) and record under `docs/performance/` if it’s
   a meaningful checkpoint.
3) Run end-to-end canaries vs mmdr (`--preset long --skip-mermaid-js`) and refresh
   `docs/performance/COMPARISON.md` only when you intend to update the baseline.
4) If numbers look contradictory, re-run the same command once (noise happens) and prioritize the
   longer preset results.
