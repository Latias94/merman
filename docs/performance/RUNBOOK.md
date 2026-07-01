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

## GitHub Actions lanes

Performance automation lives in the separate `Performance` workflow, not the regular `CI` workflow.
This keeps correctness gates and noisy benchmark evidence independent.

- `perf-contracts`: checks benchmark helper syntax and script contracts. It runs for performance
  workflow triggers only.
- `perf-regression`: compares the PR/base checkout against the head checkout with
  `tools/bench/compare_self.py`. Pull requests only run this lane when the PR carries a `perf`
  label; they default to `canary + quick`. Manual runs can select the suite and preset. Reports are
  uploaded as `perf-regression` artifacts. Labeled PR runs also update one sticky performance
  comment with the gate status, threshold crossings, and a link to the run artifact. For manual
  PR-style comparisons, set `base_ref` and `head_ref`; set `base_repository` or `head_repository`
  when comparing across forks.
- `perf-frontmatter`: compares the frontmatter preprocessing lane with
  `frontmatter_basic`, `frontmatter_indented`, and `frontmatter_deep_config`. Pull requests only
  run this lane when the PR carries a `perf-frontmatter` label. Manual runs can use the same
  `base_ref` / `head_ref` inputs. The lane comments on PRs with its own sticky marker so it does
  not collide with the general regression gate.
- `perf-reference`: explicitly checks out the pinned `mermaid-rs-renderer` reference under
  `repo-ref/mermaid-rs-renderer` and runs `compare_mermaid_renderers.py`. It runs on the weekly
  schedule or manual `reference`/`full` dispatch. Mermaid JS is skipped by default; enable it with
  the workflow input, which installs `tools/mermaid-cli` via `npm ci`.

Manual PR-style regression example:

```bash
gh workflow run performance.yml \
  --ref main \
  -f run=regression \
  -f base_ref=main \
  -f head_ref=my-perf-branch \
  -f preset=long \
  -f suite=full
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
- Treat the PR comment as a triage signal. It currently summarizes same-runner mid estimates against
  warn/fail percentage thresholds; use manual long runs before claiming small wins or losses.

## 5. Validation suite choice

- `--suite canary`: the four standard hotspot sentinels.
- `--suite standard`: routine validation across the main cross-family canaries.
- `--suite cross_family`: shared code-path changes that should be checked across families.
- `--suite full`: framework, corpus, or infrastructure changes where broad coverage matters.

## 6. Harness contract tests

- `python3 tools/bench/test_perf_contracts.py` checks the canary suite and `perf_runner` dry-run
  command contract.
