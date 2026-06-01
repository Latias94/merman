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

By default this writes `docs/performance/COMPARISON.md` with mid-point `end_to_end/*` estimates and ratios.

The helper script sets `MMDR_RUN_CRITERION_BENCHES=1` for the local mmdr checkout automatically.
If you invoke `cargo bench --bench renderer` there by hand, set that env var yourself or the bench
binary will only run its smoke validation path.

Both comparison helpers add `--locked` when the target repo has `Cargo.lock`, so benchmark runs do
not silently drift to newer registry dependencies. If the local `mermaid-rs-renderer` checkout needs
a newer Rust toolchain than this workspace's `rust-toolchain.toml`, pass it explicitly, for example:

```bash
python tools/bench/compare_mermaid_renderers.py --mmdr-toolchain 1.92.0
```

If you prefer keeping comparison artifacts out of the docs tree, pass `--out` explicitly, e.g.:

```bash
python tools/bench/compare_mermaid_renderers.py --out target/bench/COMPARISON.latest.md
```

For lower-noise results (recommended when tracking canaries), use the `long` preset:

```bash
python tools/bench/compare_mermaid_renderers.py --preset long
```

If you want to avoid the optional upstream Mermaid JS (puppeteer) run (which can be noisy and slow),
add `--skip-mermaid-js`:

```bash
python tools/bench/compare_mermaid_renderers.py --preset long --skip-mermaid-js
```

## Browser comparison with Mermaid JS

Native `merman-cli` results and Mermaid JS browser results should not be treated as the same kind
of benchmark. The CLI path is useful for native pipeline regressions; the browser path is useful
for playground and web embedding decisions.

For web-to-web comparisons, measure after both engines are initialized in the same headless
Chromium session:

- Merman: initialize `@merman/web` once, then measure repeated `renderSvg()` calls.
- Mermaid JS: initialize Mermaid once, then measure repeated `mermaid.render()` calls.
- Use the same fixtures, theme, viewport width, warmup window, and measurement window.
- Keep cold-start numbers separate from steady-state render numbers.

`tools/bench/mermaid_js_bench.cjs` already provides the Mermaid JS side of this comparison through
the pinned `tools/mermaid-cli` dependency set. A future `@merman/web` browser harness should emit
the same JSON shape so `tools/bench/compare_mermaid_renderers.py` can report native, WASM, and
Mermaid JS results without mixing unlike measurements.

For interactive visual comparison rather than timed measurement, see
`docs/workstreams/web-wasm-playground/MERMAID_COMPARE_MODE.md`.

See `docs/performance/PERF_PLAYBOOK.md` for the recommended default canary filter and report naming.

## Stage spot-check (recommended for triage)

When you want to attribute a performance change to a specific pipeline stage quickly (parse vs
layout vs SVG emission), use the stage spot-check script:

```bash
python tools/bench/stage_spotcheck.py --fixtures flowchart_medium,class_medium --out target/bench/stage_spotcheck.md
```

This runs a small set of `--exact` Criterion benchmarks for both `merman` and
`repo-ref/mermaid-rs-renderer` and writes a compact stage-by-stage report.

The helper script also sets `MMDR_RUN_CRITERION_BENCHES=1` for the local mmdr checkout
automatically.

For lower-noise spotchecks, use the `long` preset:

```bash
python tools/bench/stage_spotcheck.py --preset long --fixtures flowchart_medium,class_medium
```

The stage spot-check helper accepts the same `--mmdr-toolchain` option when the reference checkout
needs a newer toolchain.

## Recommendations

- Prefer comparing two git revisions on the same machine.
- Run with a mostly idle system (close background heavy apps).
- Keep the Rust toolchain consistent (e.g. stable vs nightly).

## Stress benches (render-only, lower noise)

Some render paths are fast enough (µs-scale) that small changes get lost in noise. The
`*_stress` benches batch many renders per iteration to amplify fixed-cost improvements and
stabilize A/B comparisons.

```bash
cargo bench -p merman --features render --bench flowchart_stress -- --noplot --sample-size 50 --warm-up-time 2 --measurement-time 3
cargo bench -p merman --features render --bench architecture_stress -- --noplot --sample-size 50 --warm-up-time 2 --measurement-time 3
```

## Future work

- Add additional “stress” fixtures (node-heavy flowcharts, dense edge routing).
- Add timing output to `merman-cli` for ad-hoc benchmarking without Criterion.
- Add a documented “upstream CLI” comparison mode (optional, requires Node.js).
