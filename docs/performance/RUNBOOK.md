# Performance Runbook

This is the default operating procedure for performance work in `merman`.
Use it whenever you want to decide whether a change is actually faster, and at what stage.
The current optimization queue lives in `docs/performance/PERF_PLAN.md`; benchmark definitions and
comparison semantics live in `docs/performance/BENCHMARKING.md`.

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
  -p merman --features svg --example profile_render \
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

- **Did this change get faster?** Use `compare_self.py` against two same-host checkouts.
- **Which stage moved between those revisions?** Repeat the self-comparison for parse, layout,
  render, and end-to-end groups.
- **How does the current product compare with another renderer?** Use the stage spotcheck or
  cross-runner comparison; do not use either as a base/head regression result.
- **Did request-style parsing improve too?** Use `parse_cold_engine/*`.
- **Did SVG emission or layout fixed-cost move?** Use the relevant stress bench plus timing toggles.
- **Did the change hold across more of Mermaid?** Use the `standard`, `cross_family`, or `full`
  comparison suites.

## 2. Measure in this order

1. Fast iteration gate:
   - `cargo nextest run -p merman-render`
2. Same-host base/head A/B:

   ```bash
   python3 tools/bench/compare_self.py \
     --base-dir ../merman-base \
     --head-dir . \
     --base-label base \
     --head-label HEAD \
     --preset long \
     --filter 'end_to_end/(mindmap_medium|requirement_medium|kanban_medium|class_medium)' \
     --out target/bench/self_comparison.md \
     --json-out target/bench/self_comparison.json
   ```

   Repeat the command with `parse`, `layout`, and `render` in place of `end_to_end` when stage
   attribution is required. The helper assumes both revisions use the current `svg` feature
   vocabulary; release ranges that renamed capabilities need explicit per-revision feature lanes.
3. Current Merman versus mmdr stage reference:
   - `python3 tools/bench/stage_spotcheck.py --preset long --fixtures mindmap_medium,requirement_medium,kanban_medium,class_medium --out target/bench/perf-runner/stage_regression_sentinels_latest.md`
4. Cross-runner end-to-end reference when the checkpoint matters:
   - `python3 tools/bench/compare_mermaid_renderers.py --preset long --skip-mermaid-js --suite standard --out target/bench/renderer_comparison.md`
5. Hot vs cold parse sanity:
   - `parse/*` measures a reused `Engine`
   - `parse_cold_engine/*` measures a fresh `Engine` per iteration
6. Micro-hotspot validation:
   - `cargo bench -p merman --features svg --bench flowchart_stress -- --noplot --sample-size 50 --warm-up-time 2 --measurement-time 3`
   - `cargo bench -p merman --features svg --bench architecture_layout_stress -- --noplot --sample-size 50 --warm-up-time 2 --measurement-time 3`
   - `cargo bench -p merman --features svg --bench architecture_stress -- --noplot --sample-size 50 --warm-up-time 2 --measurement-time 3`
   - `cargo bench -p merman --features svg --bench mindmap_layout_stress -- --noplot --sample-size 50 --warm-up-time 2 --measurement-time 3`
   - `cargo bench -p merman --features svg --bench text_measure_stress -- --noplot --sample-size 50 --warm-up-time 2 --measurement-time 3`

## 3. Run acceptance gates

Focused family tests are sufficient during iteration. Before accepting a shared-path optimization
or recording a release checkpoint, run the complete repository and DOM parity gates:

```bash
cargo run -p xtask -- verify --strict
cargo run -p xtask -- compare-all-svgs --check-dom --dom-mode parity-root --dom-decimals 3
```

Family-local changes should also retain their semantic, layout, SVG, sanitization, and custom
text-measurer tests. A faster benchmark with a changed output contract is not an accepted result.

## 4. Use the right report location

- In-flight work: `target/bench/perf-runner/*.md`
- Meaningful checkpoint: `docs/performance/spotcheck_YYYY-MM-DD*.md`
- Durable cross-runner checkpoint: `docs/performance/renderer_comparison_YYYY-MM-DD.md`

## 5. Interpretation rules

- Prefer the long preset for decisions.
- For a decision-grade base/head result, run at least three repetitions and alternate which
  checkout runs first, normalizing every result back to the same head/base direction.
- Accept a performance change only when correctness is green, the moved stage and cause are
  explained, and the relevant evidence is recorded.
- For microsecond-scale work, prefer batched stress benches over single-shot micro changes.
- Treat ratio-only changes below the active plan's absolute-cost threshold as evidence, not an
  automatic optimization priority.
- Treat the PR comment as a triage signal. It currently summarizes same-runner mid estimates against
  warn/fail percentage thresholds; use manual long runs before claiming small wins or losses.

## 6. Validation suite choice

- `--suite canary`: the four stable cross-release sentinels.
- `--suite standard`: routine validation across the main cross-family canaries.
- `--suite cross_family`: shared code-path changes that should be checked across families.
- `--suite full`: framework, corpus, or infrastructure changes where broad coverage matters.

## 7. Harness contract tests

- `python3 tools/bench/test_perf_contracts.py` checks the canary suite and `perf_runner` dry-run
  command contract.
