# Merman Performance Surface Map

Read this file before adding a benchmark script. Prefer the narrowest existing harness that answers
the registered question.

## Contents

- Rust pipeline and stress benches
- Same-implementation revision A/B
- External renderer reference
- CPU attribution
- Node N-API and Node-WASM
- Browser and WASM
- Native size and dependency closure
- Correctness and harness contracts

## Rust Pipeline

Use `crates/merman/benches/pipeline.rs` for stable stage boundaries:

```console
cargo bench --locked -p merman --features svg --bench pipeline -- \
  --noplot --sample-size 30 --warm-up-time 2 --measurement-time 3 \
  --exact render/mindmap_medium
```

Available groups include `parse`, `parse_cold_engine`, `parse_known_type`, `layout`, `render`, and
`end_to_end`. Fixtures live in `crates/merman/benches/fixtures/`.

Use batched benches when fixed costs are too small for stable single-operation measurements:

- `flowchart_stress`
- `architecture_layout_stress`
- `architecture_stress`
- `mindmap_layout_stress`
- `text_measure_stress`

`python3 tools/bench/perf_runner.py --profile canary` runs the documented local workflow.
Use `--dry-run` to inspect it and `--profile full` only for broad validation.
With `--write-docs`, measurement artifacts remain at the validated report root until the complete
profile succeeds; in-repository roots are restricted to `target/bench`, while external absolute
paths are also accepted. Markdown reports are published to `docs/performance` only after that clean
measurement phase finishes.

## Same-Implementation Revision A/B

Use `compare_self.py` for base/head regressions:

```console
python3 tools/bench/compare_self.py \
  --base-dir ../merman-base \
  --head-dir . \
  --base-label base \
  --head-label HEAD \
  --preset long \
  --filter 'end_to_end/(mindmap_medium|kanban_medium)' \
  --out target/bench/self-comparison.md \
  --json-out target/bench/self-comparison.json
```

Run separate parse, layout, render, and end-to-end filters when attributing a regression. Repeat
three times and alternate checkout order. Release ranges with renamed features require explicit
same-capability lanes; the helper assumes the current `svg` vocabulary.

Use exactly one stage group per invocation. The helper expands `group/(fixture_a|fixture_b)`, but it
does not expand `(parse|layout|render)/(fixture_a|fixture_b)`. Criterion 0.8 can treat the latter as
a literal substring and measure no rows. Run four commands when all four stages are required.

## External Renderer Reference

Use these only after the Merman base/head result is understood:

```console
python3 tools/bench/stage_spotcheck.py \
  --preset long \
  --fixtures mindmap_medium,kanban_medium

python3 tools/bench/compare_mermaid_renderers.py \
  --preset long \
  --suite standard \
  --skip-mermaid-js \
  --out target/bench/renderer-comparison.md \
  --json-out target/bench/renderer-comparison.json
```

The local reference checkout defaults to `repo-ref/mermaid-rs-renderer`. Pin its revision and
toolchain. Keep warm native Merman, mmdr, and warm browser Mermaid.js as separate implementations;
coverage or failed renders are not latency samples.

## CPU Attribution

Profile only after a stage benchmark identifies the hotspot:

```console
CARGO_PROFILE_BENCH_DEBUG=true cargo flamegraph --profile bench \
  -p merman --features svg --example profile_render \
  -o target/bench/flamegraphs/mindmap-render.svg -- \
  --input crates/merman/benches/fixtures/mindmap_medium.mmd \
  --stage render --seconds 20
```

On macOS, Instruments Time Profiler and Allocations are suitable alternatives. Preserve the input,
stage, profile, and batch size in the experiment ledger.

## Node N-API and Node-WASM

Use the package-owned admission harness:

```console
npm test --prefix platforms/node
npm run build:candidate --prefix platforms/node -- --candidate node-wasm
npm run build:candidate --prefix platforms/node -- --candidate napi --target darwin-arm64
npm run benchmark --prefix platforms/node -- \
  --native artifacts/napi/darwin-arm64/merman.node \
  --wasm artifacts/node-wasm/merman_node.js
```

It owns cold process, warm SVG, concurrency, RSS, installed footprint, queue lifecycle, errors, and
transport parity. Use the same installed product facade and `semantic-json` for parse-only timing.
Do not call SVG latency parse latency. Do not claim admission from one host.

## Browser and WASM

For Web artifact size, use exact artifact profiles:

```console
cargo run -p xtask -- wasm-size-matrix \
  --surface web \
  --artifact-profile web-full \
  --budget-file docs/release/WASM_SIZE_BUDGETS.json
```

Record raw, stripped, gzip, and Brotli bytes and the profile capabilities. Compare browser
renderers with the checked-in family corpus:

```console
npm --prefix playground run benchmark:corpus -- \
  --iterations 6 \
  --warmups 2 \
  --out target/bench/browser-family-corpus.json
```

The runner uses a fresh Chromium process per fixture to bound retained renderer state, while both
engines share one page, source, theme, viewport, warmup policy, and deterministic AB/BA schedule
inside that fixture. Keep cold first-publishable SVG separate from reused-engine warm publishable
SVG. Each fixture mode reports its ratio as Mermaid.js divided by Merman, so values above one favor
Merman. The runner rejects plans above its 4,096 retained-sample whole-corpus budget before timing.
Treat two-iteration coverage runs as discovery only; target and repeat candidates before changing
production code. Keep browser-WASM separate from Node-WASM and native Rust.

## Native Size and Dependency Closure

Build both revisions with the same target, profile, lockfile, and feature set. Record raw, stripped,
and packaged bytes when applicable.

Count unique resolved normal dependencies with:

```console
cargo tree --locked --edges normal --prefix none --format '{p}'
```

Keep direct manifest dependencies separate from the resolved closure. A default product and a slim
same-capability build are separate lanes.

## Correctness and Harness Contracts

Use focused `cargo nextest` during iteration. For shared behavior, use:

```console
cargo run -p xtask -- verify --strict
cargo run -p xtask -- compare-all-svgs --check-dom --dom-mode parity-root --dom-decimals 3 --diagnostic-browser-text-layout
python3 tools/bench/test_perf_contracts.py
```

Consult `docs/performance/BENCHMARKING.md` for semantics, `docs/performance/RUNBOOK.md` for
execution order, and `docs/performance/PERF_PLAN.md` for current priorities and thresholds.

Keep in-flight Markdown and JSON under `target/bench`. Use the dedicated Performance workflow only
for explicitly requested regression or reference runs; do not add noisy timing lanes to regular CI.
