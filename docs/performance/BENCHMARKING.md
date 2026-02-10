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

- `parse_only_sync`: parsing Mermaid into a semantic model (no layout).
- `parse_only_known_type_sync`: parsing when the diagram type is already known (skips detection).
- `layout_only_sync`: computing layout (geometry + routes) from a parsed diagram.
- `render_svg_sync`: full pipeline (parse + layout + SVG emission).

## Recommendations

- Prefer comparing two git revisions on the same machine.
- Run with a mostly idle system (close background heavy apps).
- Keep the Rust toolchain consistent (e.g. stable vs nightly).

## Future work

- Add larger “stress” fixtures (node-heavy flowcharts, dense edge routing).
- Add timing output to `merman-cli` for ad-hoc benchmarking without Criterion.
- Add a documented “upstream CLI” comparison mode (optional, requires Node.js).
