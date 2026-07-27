# Benchmarking

This document describes how to benchmark `merman` locally in a way that is reproducible and useful
for tracking regressions.

For the current optimization backlog, see `docs/performance/PERF_PLAN.md`.
For the execution order, see `docs/performance/RUNBOOK.md`.
For a one-step local workflow, use `python3 tools/bench/perf_runner.py --profile canary`.

## Goals

- Track performance changes over time (regression detection).
- Compare pipeline stages (parse vs layout vs SVG emission).
- Keep results meaningful across machines and CI.

The goal here is not to replace Mermaid.js in the browser. This work is mostly for native apps,
text editors, CI pipelines, preview tools, and doc generators where embedding a JS engine just to
draw a chart is heavy and awkward. Criterion stays in the toolbox for regression tracking, but
parity and predictable headless output remain the top priorities.

## Running Criterion benches

`merman` includes Criterion benchmarks for the headless pipeline.

```bash
cargo bench -p merman --features svg --bench pipeline
```

Notes:

- Criterion performs multiple warm-up and measurement iterations and reports statistics.
- Results vary across CPUs/OSes. Use relative comparisons on the same machine for regressions.
- Dated evidence preserves the feature names that were actually used at measurement time. Use this
  document for current commands instead of mechanically rewriting historical reports.

## What is benchmarked

The `pipeline` bench measures:

- `parse/*`: parsing Mermaid into a semantic model (reused `Engine`, no layout).
- `parse_cold_engine/*`: request-style parsing with a fresh `Engine` per iteration.
- `parse_known_type/*`: parsing when the diagram type is already known (skips detection).
- `layout/*`: computing layout (geometry + routes) from a parsed diagram.
- `render/*`: SVG emission from an already-laid-out diagram.
- `end_to_end/*`: full pipeline (parse + layout + SVG emission).

The dedicated stress benches are separate on purpose:

- `flowchart_stress`: render-only batching for flowchart fixed-cost work.
- `architecture_layout_stress`: layout-only batching for architecture FCoSE/manatee work.
- `architecture_stress`: render-only batching for architecture fixed-cost work.
- `mindmap_layout_stress`: layout-only batching for mindmap COSE fixed-cost work.
- `text_measure_stress`: text measurement batching for label/layout tuning.

Bench fixtures live under `crates/merman/benches/fixtures/` and are intentionally small, focused
inputs designed for regression tracking.

## Comparing with mermaid-rs-renderer (optional)

If you have a local checkout under `repo-ref/mermaid-rs-renderer`, you can generate a renderer
comparison report:

```bash
python3 tools/bench/compare_mermaid_renderers.py
```

By default this runs the `quick` corpus suite, writes a Markdown report to
`target/bench/renderer_comparison.md`, and writes a structured JSON report to
`target/bench/renderer_comparison.json`.

The comparison harness is intentionally corpus-driven. `tools/bench/corpus.json` records which
fixtures belong to each suite, their diagram family, broad feature tags, and the quality gates that
should eventually be paired with the timing result. This keeps benchmark selection out of the
runner implementation and makes coverage differences explicit.

Before computing a Merman/mmdr ratio, the harness compares the exact fixture bytes used by each
native Criterion bench. A non-identical same-named fixture keeps its raw timings for auditability
and remains in measured coverage, but is excluded from ratios and family geomeans. The stage
spot-check is stricter: it exits before starting Cargo if any requested fixture differs or is
missing.

The helper scripts set `MMDR_RUN_CRITERION_BENCHES=1` and enable mmdr's `benchmark` feature
automatically.
If you invoke `cargo bench --bench renderer` there by hand, set that env var yourself or the bench
binary will only run its smoke validation path; also pass `--features benchmark`.

For broader validation, prefer the named suites in `tools/bench/corpus.json`:

- `standard`: routine cross-family validation.
- `cross_family`: one medium fixture per supported family.
- `flowchart`: flowchart-heavy routing and layout stress coverage.
- `stress`: heavy fixtures when validating hotspot work.
- `full`: every fixture in corpus order.

Both comparison helpers add `--locked` when the target repo has `Cargo.lock`, so benchmark runs do
not silently drift to newer registry dependencies. If the local `mermaid-rs-renderer` checkout needs
a newer Rust toolchain than this workspace's `rust-toolchain.toml`, pass it explicitly, for example:

```bash
python3 tools/bench/compare_mermaid_renderers.py --mmdr-toolchain 1.92.0
```

To use a different local artifact name, pass `--out` and `--json-out`, e.g.:

```bash
python3 tools/bench/compare_mermaid_renderers.py \
  --suite standard \
  --out target/bench/renderer_comparison.latest.md \
  --json-out target/bench/renderer_comparison.latest.json
```

For lower-noise results (recommended when tracking canaries), use the `long` preset:

```bash
python3 tools/bench/compare_mermaid_renderers.py --preset long
```

If you want to avoid the optional upstream Mermaid JS (puppeteer) run (which can be noisy and slow),
add `--skip-mermaid-js`:

```bash
python3 tools/bench/compare_mermaid_renderers.py --preset long --skip-mermaid-js
```

To see the available corpus suites:

```bash
python3 tools/bench/compare_mermaid_renderers.py --list-suites
```

The main suites are:

- `quick`: a small smoke set for local iteration; mirrors the historical comparison filter.
- `standard`: a balanced cross-family set for routine comparison reports.
- `cross_family`: one representative medium fixture per supported diagram family.
- `flowchart`: flowchart-heavy routing and layout stress coverage.
- `stress`: heavier fixtures used when validating optimization work.
- `full`: every fixture in `tools/bench/corpus.json`.

The legacy `--filter` path is still available for ad-hoc exact selections. When `--filter` is set,
`--suite` is ignored:

```bash
python3 tools/bench/compare_mermaid_renderers.py \
  --filter 'end_to_end/(flowchart_medium|class_medium)' \
  --out target/bench/filter.md \
  --json-out target/bench/filter.json
```

### How to read comparison reports

The comparison report separates three signals that should not be collapsed into a single speed
number:

- Performance: successful timing samples for each renderer.
- Coverage: requested, available, measured, missing, skipped, and errored fixture counts.
- Quality expectations: fixture-level tags for future SVG sanity, DOM comparison, and raster
  comparison gates.

Ratios are only computed when both renderers successfully measured the same fixture. Missing,
skipped, and errored fixtures reduce coverage rather than being treated as slow or fast results.
This matters when comparing `merman` with renderers that have different goals or partial Mermaid
coverage.

The current harness measures warm steady-state rendering:

- `merman`: Criterion `end_to_end/*` benches.
- `mermaid-rs-renderer`: Criterion `end_to_end/*` benches in the local checkout.
- Mermaid JS: repeated warm `mermaid.render()` calls inside a single Puppeteer/Chromium process.

Cold CLI startup, WASM/browser-hosted `@mermanjs/web`, DOM diff, and raster diff should remain
separate modes or quality gates. Do not mix them into the warm native comparison without making the
mode explicit in the JSON schema.

## Browser comparison with Mermaid JS

Native `merman-cli` results and Mermaid JS browser results should not be treated as the same kind
of benchmark. The CLI path is useful for native pipeline regressions; the browser path is useful
for playground and web embedding decisions.

For web-to-web comparisons, measure after both engines are initialized in the same headless
Chromium session:

- Merman: initialize `@mermanjs/web` once, then measure repeated `renderSvg()` calls.
- Mermaid JS: initialize Mermaid once, then measure repeated `mermaid.render()` calls.
- Use the same fixtures, theme, viewport width, warmup window, and measurement window.
- Keep cold-start numbers separate from steady-state render numbers.

`tools/bench/mermaid_js_bench.cjs` already provides the Mermaid JS side of this comparison through
the pinned `tools/mermaid-cli` dependency set. A future `@mermanjs/web` browser harness should emit
the same JSON shape so `tools/bench/compare_mermaid_renderers.py` can report native, WASM, and
Mermaid JS results without mixing unlike measurements.

For interactive visual comparison rather than timed measurement, see
`docs/workstreams/web-wasm-playground/MERMAID_COMPARE_MODE.md`.

See `docs/performance/RUNBOOK.md` for the default sentinel suite and dated report convention.

## Stage spot-check (recommended for triage)

When you want to attribute a performance change to a specific pipeline stage quickly (parse vs
layout vs SVG emission), use the stage spot-check script:

```bash
python3 tools/bench/stage_spotcheck.py --fixtures flowchart_medium,class_medium --out target/bench/stage_spotcheck.md
```

This runs a small set of `--exact` Criterion benchmarks for both `merman` and
`repo-ref/mermaid-rs-renderer` and writes a compact stage-by-stage report.

The helper script also sets `MMDR_RUN_CRITERION_BENCHES=1` for the local mmdr checkout
automatically.

For lower-noise spotchecks, use the `long` preset:

```bash
python3 tools/bench/stage_spotcheck.py --preset long --fixtures flowchart_medium,class_medium
```

The stage spot-check helper accepts the same `--mmdr-toolchain` option when the reference checkout
needs a newer toolchain.

## Per-request SVG timing diagnostics

When the `render/*` stage is slow, direct `merman-render` callers can set
`SvgDebugOptions::include_timing_diagnostics` and use an explicit `*_with_debug` render entry point.
The resulting per-request breakdown can localize family-specific SVG emission work without a
process-global timing switch.

Use this diagnostic for attribution, then use Criterion A/B measurements for the performance
claim. Keep timing diagnostics disabled in the measured production path. The API boundary and
examples are documented in `crates/merman-render/README.md`.

## Recommendations

- Prefer comparing two git revisions on the same machine.
- Run with a mostly idle system (close background heavy apps).
- Keep the Rust toolchain consistent (e.g. stable vs nightly).

## Stress benches (render-only, lower noise)

Some render paths are fast enough (µs-scale) that small changes get lost in noise. The
`*_stress` benches batch many renders per iteration to amplify fixed-cost improvements and
stabilize A/B comparisons.

```bash
cargo bench -p merman --features svg --bench flowchart_stress -- --noplot --sample-size 50 --warm-up-time 2 --measurement-time 3
cargo bench -p merman --features svg --bench architecture_layout_stress -- --noplot --sample-size 50 --warm-up-time 2 --measurement-time 3
cargo bench -p merman --features svg --bench architecture_stress -- --noplot --sample-size 50 --warm-up-time 2 --measurement-time 3
cargo bench -p merman --features svg --bench mindmap_layout_stress -- --noplot --sample-size 50 --warm-up-time 2 --measurement-time 3
cargo bench -p merman --features svg --bench text_measure_stress -- --noplot --sample-size 50 --warm-up-time 2 --measurement-time 3
```

## Future work

- Add additional “stress” fixtures (node-heavy flowcharts, dense edge routing).
- Add timing output to `merman-cli` for ad-hoc benchmarking without Criterion.
- Add a documented “upstream CLI” comparison mode (optional, requires Node.js).
