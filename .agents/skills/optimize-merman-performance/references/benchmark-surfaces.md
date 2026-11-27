# Merman Performance Surface Map

Read this file before adding a benchmark script. Prefer the narrowest existing harness that answers
the registered question.

## Contents

- Rust pipeline and stress benches
- Same-implementation revision A/B
- Ordered public workloads and corpus coverage
- External renderer reference
- CPU attribution
- Node N-API and Node-WASM
- Browser and WASM
- Native size and dependency closure
- Correctness and harness contracts

## Rust Pipeline

Use `crates/merman/benches/pipeline.rs` for stable stage boundaries. The
[benchmark contract](../../../../docs/performance/BENCHMARKING.md#native-criterion-benchmarks)
owns current groups and recipes; the executable's `required-features` in
`crates/merman/Cargo.toml` apply even to one filtered group:

```console
cargo bench --locked -p merman --features all-diagrams,svg,compat-json,editor-facts --bench pipeline -- \
  --noplot --sample-size 30 --warm-up-time 2 --measurement-time 3 \
  --exact render/mindmap_medium
```

Fixtures live in `crates/merman/benches/fixtures/`. Current compatibility JSON timing uses
`compatibility_json_parse`; `parse_known_type` is only a registered history alias in
`tools/bench/corpus.json`. The `editor_snapshot` group measures requested snapshot capture.

Select a Criterion stress lane for fixed-cost work. For public operation attribution, run `bench.py
stages` with a frozen workload plan. See the [stage diagnostics guide](../../../../docs/performance/BENCHMARKING.md#stage-and-render-diagnostics).

## Same-Implementation Revision A/B

Use `bench.py ab` for base/head regressions. Follow the
[per-revision runner recipes](../../../../docs/performance/RUNBOOK.md#2-declare-both-runner-recipes)
for an executable command; explicitly select each side's features from its own revision.

Run separate parse, layout, render, and end-to-end filters when attributing a regression. Three
alternating repeats are exploratory evidence; use `BENCHMARKING.md`'s calibrated, power-sized
confirmation schedule for an acceptance decision. Preserve historical feature vocabularies and
compare equivalent capabilities. The helper's legacy base default is `svg`; current pipeline bases
also require compatibility JSON and editor facts.

Use exactly one stage group per invocation. The helper expands `group/(fixture_a|fixture_b)`, but it
does not expand `(parse|layout|render)/(fixture_a|fixture_b)`. Criterion 0.8 can treat the latter as
a literal substring and measure no rows. Run four commands when all four stages are required.

## Ordered Public Workloads and Corpus Coverage

Use `bench.py plan` for a sequence of public Rust, Node or CLI calls. For fixture
growth, inspect `bench.py coverage` for distinct sources, repeated occurrences and label facts.
For a headless Node product comparison, run `bench.py node` from the
[ordered workload runbook](../../../../docs/performance/RUNBOOK.md#run-an-ordered-workload).
It executes the installed Merman, Pele and Mermaid v13 public APIs in a locked consumer and writes
raw results and coverage beside the summary. `--pele-fixtures` imports upstream input text only.
For a focused comparison across independent Node processes, add `--process-rounds 8` and inspect
the eligible pairs, order and spread. `--samples` counts warm batches per worker. Check the
recorded product files before attributing a result to a local build or published package.

When a renderer appears to miss a label, inspect its SVG before changing its expected facts.
For a Flowchart arrow, sibling membership, horizontal label overflow or a repeated Sequence message, declare the source fact
and run `bench.py node --check-drawings` or replay the retained CLI SVGs. Inspect the occurrence's
`drawing-audit.json` result before drawing timing conclusions. The [drawing contract](../../../../docs/performance/BENCHMARKING.md#svg-drawing-checks)
identifies the supported geometry and the boundary of the evidence.
For family breadth use `bench.py plan --suite families` and inspect the per-product gaps in
`coverage.json`. For input growth use the Sequence loop message counts or the topology series.
For an unstable SVG, preserve the original hashes and compare the retained files in a fixed
browser with a same-file control before attributing a visual difference.
For startup, use a fresh CLI or the Node initialization and first-render observations. For a batch
use the matching complete warm batch. For topology growth, use the `work` metric and record the
first failing limit. Always identify the host, lifecycle, input population and operation in the finding.
Use `bench.py plan --suite topology-orders` to vary declaration or edge order with the same
labeled directed graph. Inspect both the geometry boundary and the actual default SVG outcome.

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

For Web artifact size, use the [upgrade playbook's artifact budget
commands](../../../../docs/release/MERMAID_UPGRADE_PLAYBOOK.md#7-refresh-generated-surfaces-and-evidence)
against the built package root and its exact artifact profiles.

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
cargo run --locked -p xtask -- verify --strict
python3 -m unittest discover -s tools/bench
node --test tools/bench/collectors/node_runner.test.mjs
```

Run strict once at the settled source for final integration; the
[verification schedule](../../../../docs/release/MERMAID_UPGRADE_PLAYBOOK.md#7-refresh-generated-surfaces-and-evidence)
owns its coverage and additional checks. Rerun covered checks for failures or subsequent changes;
keep performance harness contracts and measurements as distinct evidence.

Consult `docs/performance/BENCHMARKING.md` for semantics, `docs/performance/RUNBOOK.md` for
execution order, and `docs/performance/PERF_PLAN.md` for current priorities and thresholds.

Keep in-flight Markdown and JSON under `target/bench`. Use the dedicated Performance workflow only
for explicitly requested regression or reference runs; do not add noisy timing lanes to regular CI.
