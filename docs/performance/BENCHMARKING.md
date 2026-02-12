# Benchmarking

This document describes how to benchmark `merman` locally in a way that is reproducible and useful
for tracking regressions.

For the optimization backlog (prioritized, correctness-first), see:
`docs/performance/FEARLESS_REFACTORING.md`.

## Goals

- Track performance changes over time (regression detection).
- Compare pipeline stages (parse vs layout vs SVG emission).
- Keep results meaningful across machines and CI.

## Running Criterion benches

`merman` includes Criterion benchmarks for the headless pipeline.

```bash
cargo bench -p merman --features render --bench pipeline
```

Notes:

- Criterion performs multiple warm-up and measurement iterations and reports statistics.
- Results vary across CPUs/OSes. Use relative comparisons on the same machine for regressions.

## What is benchmarked

The `pipeline` bench measures:

- `parse/*`: parsing Mermaid into a semantic model (no layout).
- `parse_known_type/*`: parsing when the diagram type is already known (skips detection).
- `layout/*`: computing layout (geometry + routes) from a parsed diagram.
- `render/*`: SVG emission from an already-laid-out diagram.
- `end_to_end/*`: full pipeline (parse + layout + SVG emission).

Bench fixtures live under `crates/merman/benches/fixtures/` and are intentionally small, focused
inputs designed for regression tracking.

## Comparing with mermaid-rs-renderer (optional)

If you have a local checkout under `repo-ref/mermaid-rs-renderer`, you can generate a comparison
report:

```bash
python tools/bench/compare_mermaid_renderers.py
```

This writes `target/bench/COMPARISON.latest.md` with mid-point `end_to_end/*` estimates and ratios.

## Stage spot-check (recommended for triage)

When you want to attribute a performance change to a specific pipeline stage quickly (parse vs
layout vs SVG emission), use the stage spot-check script:

```bash
python tools/bench/stage_spotcheck.py --fixtures flowchart_medium,class_medium --out target/bench/stage_spotcheck.md
```

This runs a small set of `--exact` Criterion benchmarks for both `merman` and
`repo-ref/mermaid-rs-renderer` and writes a compact stage-by-stage report.

If you also have the pinned Node toolchain under `tools/mermaid-cli` (used for parity SVG
baselines), the script will additionally benchmark upstream Mermaid JS rendering via a single
headless Chromium instance (puppeteer) and include it in the report.

## Recommendations

- Prefer comparing two git revisions on the same machine.
- Run with a mostly idle system (close background heavy apps).
- Keep the Rust toolchain consistent (e.g. stable vs nightly).

## Future work

- Add larger “stress” fixtures (node-heavy flowcharts, dense edge routing).
- Add timing output to `merman-cli` for ad-hoc benchmarking without Criterion.
- Add a documented “upstream CLI” comparison mode (optional, requires Node.js).
