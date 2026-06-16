# Performance Runbook

This is the default operating procedure for performance work in `merman`.
Use it whenever you want to decide whether a change is actually faster, and at what stage.

## One-step entrypoints

- `python3 tools/bench/perf_runner.py --profile canary` for the default hot-path workflow.
- `python3 tools/bench/perf_runner.py --profile full` for broader validation plus stress benches.

By default the one-step runner writes local Markdown and JSON artifacts under
`target/bench/perf-runner/`. Add `--write-docs` when a checkpoint should be durable in
`docs/performance/`; Markdown reports move to the docs tree while structured JSON artifacts stay
under `target/bench/perf-runner/`.

Use flamegraphs after a benchmark identifies a suspicious stage, not as the first measurement.
Broad Criterion harnesses such as `pipeline` can include fixture setup and prechecks in the
profiler output. For CPU attribution, prefer the dedicated single-stage runner:

```bash
CARGO_PROFILE_BENCH_DEBUG=true cargo flamegraph --profile bench \
  -p merman --features render --example profile_render \
  -o target/bench/flamegraphs/profile_render_architecture_medium.svg -- \
  --input crates/merman/benches/fixtures/architecture_medium.mmd \
  --stage render --seconds 20
```

## 1. Choose the question

- **Did the reused hot path get faster?** Use the standard stage spotcheck.
- **Did request-style parsing improve too?** Use `parse_cold_engine/*`.
- **Did SVG emission or layout fixed-cost move?** Use the relevant stress bench plus timing toggles.
- **Did the change hold across more of Mermaid?** Use the `standard`, `cross_family`, or `full`
  comparison suites.

## 2. Measure in this order

1. Correctness gate:
   - `cargo nextest run -p merman-render`
2. Stage attribution for the standard canaries:
   - `python3 tools/bench/stage_spotcheck.py --preset long --fixtures flowchart_medium,class_medium,mindmap_medium,architecture_medium --out target/bench/perf-runner/stage_standard_canaries_latest.md`
3. Cross-repo throughput comparison when the checkpoint matters:
   - `python3 tools/bench/compare_mermaid_renderers.py --preset long --skip-mermaid-js --suite canary --out docs/performance/COMPARISON.md`
4. Hot vs cold parse sanity:
   - `parse/*` measures a reused `Engine`
   - `parse_cold_engine/*` measures a fresh `Engine` per iteration
5. Micro-hotspot validation:
   - `cargo bench -p merman --features render --bench flowchart_stress -- --noplot --sample-size 50 --warm-up-time 2 --measurement-time 3`
   - `cargo bench -p merman --features render --bench architecture_layout_stress -- --noplot --sample-size 50 --warm-up-time 2 --measurement-time 3`
   - `cargo bench -p merman --features render --bench architecture_stress -- --noplot --sample-size 50 --warm-up-time 2 --measurement-time 3`
   - `cargo bench -p merman --features render --bench mindmap_layout_stress -- --noplot --sample-size 50 --warm-up-time 2 --measurement-time 3`
   - `cargo bench -p merman --features render --bench text_measure_stress -- --noplot --sample-size 50 --warm-up-time 2 --measurement-time 3`

## 3. Use the right report location

- In-flight work: `target/bench/perf-runner/*.md`
- Meaningful checkpoint: `docs/performance/spotcheck_YYYY-MM-DD*.md`
- End-to-end baseline refresh: `docs/performance/COMPARISON.md`

## 4. Interpretation rules

- Prefer the long preset for decisions.
- Re-run once if results conflict; prioritize the longer run.
- Do not accept a change unless the stage movement is clear and parity still holds.
- For microsecond-scale work, prefer batched stress benches over single-shot micro changes.

## 5. Validation suite choice

- `--suite canary`: the four standard hotspot sentinels.
- `--suite standard`: routine validation across the main cross-family canaries.
- `--suite cross_family`: shared code-path changes that should be checked across families.
- `--suite full`: framework, corpus, or infrastructure changes where broad coverage matters.

## 6. Harness contract tests

- `python3 tools/bench/test_perf_contracts.py` checks the canary suite and `perf_runner` dry-run
  command contract.
