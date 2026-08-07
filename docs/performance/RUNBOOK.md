# Performance Runbook

This is the operating procedure for performance work in `merman`. The statistical and report
contracts are defined in `docs/performance/BENCHMARKING.md`; the active candidate queue lives in
`docs/performance/PERF_PLAN.md`.

## Evidence classes

Use the narrowest lane that answers the current question:

- Local Criterion, `perf_runner.py`, stage spot-checks, stress benches, and one-to-four-pair
  `compare_self.py` runs are diagnostic. They identify a likely owner or reject weak hypotheses.
- `compare_self.py --evidence-mode confirmation` is the decision-grade same-host regression lane.
- Cross-renderer and browser comparisons provide product context, not causal base/head evidence.
- Allocation, peak-live-heap, RSS, and artifact-size gates remain separate from latency.

Do not carry a diagnostic timing claim into production admission. Confirmation uses fresh samples
under a frozen recipe and fixed maximum budget.

## GitHub Actions lanes

Performance automation is isolated in the `Performance` workflow:

- `perf-contracts` checks Python syntax and benchmark-helper contracts.
- `perf-regression` runs schema-v2 `compare_self.py` evidence for PR or manual base/head checkouts.
  A PR with the `perf` label receives a short diagnostic AB/BA schedule. Timing movement is advisory,
  but any recipe, fixture, executable, runner, or report-contract failure still fails the lane.
  Artifacts and the sticky PR comment are written before the comparison exit code is enforced.
- A manual `perf-regression` dispatch defaults to confirmation mode and can set the corpus suite,
  both thresholds, and explicit revisions. Exit `1`, `2`, and `3` remain distinct in the job result
  and report.
- `perf-frontmatter` owns its preprocessing comparison and uses a separate sticky marker.
- `perf-reference` checks out the pinned `mermaid-rs-renderer` reference and provides cross-renderer
  context. It does not decide a Merman base/head candidate.

Manual confirmation example:

```bash
gh workflow run performance.yml \
  --ref main \
  -f run=regression \
  -f base_ref=main \
  -f head_ref=my-perf-branch \
  -f preset=long \
  -f suite=standard \
  -f evidence_mode=confirmation \
  -f relative_threshold_percent=10 \
  -f absolute_threshold_ns=50000
```

## 1. Freeze the question and revisions

Write down the claim before measuring:

- public operation and lifecycle, such as reused-engine `end_to_end/flowchart_medium`;
- exact base and candidate commits;
- fixture bytes and output/semantic gates;
- relative and absolute minimum detectable effects;
- fixed maximum pair budget;
- relevant control fixtures and memory/resource bounds.

Use adjacent clean committed revisions to prove one optimization. A broad release comparison such as
`v0.8.0-alpha.3` to current head is useful historical context, but unrelated semantic and capability
changes prevent it from proving causality.

Create or identify two non-overlapping checkouts without changing either during the run. Do not use
a dirty working tree as a candidate receipt.

## 2. Declare both runner recipes

Never assume that base and head share a feature vocabulary or benchmark configuration. Declare each
side's checkout, label, package, bench, features, default-feature policy, toolchain, target, target
directory, and corpus.

For the alpha.3 historical context, the essential capability difference is explicit:

```bash
python3 tools/bench/compare_self.py \
  --base-dir ../merman-alpha3 \
  --head-dir . \
  --base-label v0.8.0-alpha.3 \
  --head-label HEAD \
  --base-package merman \
  --head-package merman \
  --base-bench pipeline \
  --head-bench pipeline \
  --base-features render \
  --head-features svg \
  --base-toolchain 1.95.0 \
  --head-toolchain 1.95.0 \
  --base-target-dir target/performance/base \
  --head-target-dir target/performance/head \
  --base-corpus tools/bench/corpus.json \
  --head-corpus tools/bench/corpus.json \
  --suite canary \
  --preset long \
  --evidence-mode confirmation \
  --calibration-pairs 8 \
  --max-pairs 64 \
  --relative-threshold-percent 10 \
  --absolute-threshold-ns 50000 \
  --out target/performance/alpha3_to_head.md \
  --json-out target/performance/alpha3_to_head.json
```

Default features are enabled independently for both sides. Use `--no-base-default-features` or
`--no-head-default-features` only when the selected suite does not require a disabled family;
missing capability for a mandatory fixture is a contract failure. Corpus schema-v2 lane metadata
validates the public operation, lifecycle, selector history, and normalization divisor. An explicit
`--base-logical-operations` or `--head-logical-operations` value must match that metadata. Legacy
schema-v1 confirmation keeps both divisors at one and requires exact benchmark identity. Use the
same recipe shape for an adjacent candidate comparison, changing the labels and checkouts rather
than silently inheriting head settings on base.

## 3. Establish the evidence contract

Before accepting a timing sample, verify that the report proves all of the following:

1. Both Cargo builds were locked and completed before sampling.
2. Each Cargo JSON stream resolved exactly one requested Criterion executable.
3. Executable digests remained unchanged through discovery and sampling.
4. The exact benchmark exists on both sides.
5. Both corpus files were loaded independently.
6. Each required fixture is available on both sides and byte-identical.
7. Both sides measure the same logical operation and each side's normalization divisor is accurate.

The harness invokes the frozen executables directly for discovery and every timed pair. Cargo must
not run in the background while sampling. A missing or mismatched item above is exit `2`; do not
interpret it as a performance outcome.

If both recipes deliberately share one target directory, pass `--freeze-shared-target`. That mode
serially runs `cargo clean --profile bench` before each side, rebuilds it, and freezes the resulting
executable before the next reset. The profile reset is part of the provenance contract and prevents
Cargo from reusing an executable built from the other path-workspace checkout. It does not clean
the debug profile. Ensure no other Cargo process is using the shared target for the whole run.

## 4. Use diagnostic mode for triage

Run a one-to-four-pair alternating AB/BA schedule when the goal is only to decide where to
investigate. Even pair counts are order-balanced:

```bash
python3 tools/bench/compare_self.py \
  --base-dir ../merman-base \
  --head-dir . \
  --base-label base \
  --head-label HEAD \
  --base-package merman \
  --head-package merman \
  --base-bench pipeline \
  --head-bench pipeline \
  --base-features svg \
  --head-features svg \
  --base-toolchain 1.95.0 \
  --head-toolchain 1.95.0 \
  --base-target-dir target/performance/base \
  --head-target-dir target/performance/head \
  --base-corpus tools/bench/corpus.json \
  --head-corpus tools/bench/corpus.json \
  --suite canary \
  --evidence-mode diagnostic \
  --pairs 4 \
  --out target/performance/diagnostic.md \
  --json-out target/performance/diagnostic.json
```

Timing movement in this mode is advisory and returns `0`. Contract failure still returns `2`.
Do not accept, reject, or retain candidate code from diagnostic evidence.

When attribution is needed, run owner-local stages only after the public lane moves:

```bash
python3 tools/bench/stage_spotcheck.py \
  --preset long \
  --fixtures flowchart_medium,class_medium \
  --out target/bench/stage_spotcheck.md
```

Use flamegraphs after stage attribution identifies a CPU owner:

```bash
CARGO_PROFILE_BENCH_DEBUG=true cargo flamegraph --profile bench \
  -p merman --features svg --example profile_render \
  -o target/bench/flamegraphs/profile_render_architecture_medium.svg -- \
  --input crates/merman/benches/fixtures/architecture_medium.mmd \
  --stage render --seconds 20
```

## 5. Run confirmation without adaptive sampling

This section admits latency and throughput claims. For a structural complexity or
input-amplification repair, use the evidence model in `BENCHMARKING.md`: preregister named input
variables, reachable old/new time and space bounds, and an exact work counter or scale curve. Keep
public timing as a non-regression control and do not manufacture a speedup claim from an
asymptotic proof.

Freeze the confidence level, bootstrap seed/resample count, minimum detectable effects, calibration
count, and maximum pair budget before looking at the result. The default contract is:

- at least eight balanced base A/A pairs and eight balanced head A/A pairs;
- two-sided A/A identity and order-effect intervals contained by the preregistered equivalence
  margins;
- a power-derived even AB/BA count, at least eight and no greater than the registered cap;
- fresh confirmation observations that do not reuse diagnostic or calibration data;
- suite-level simultaneous 95% paired bounds for `log(head/base)` and `head - base`, using
  Bonferroni component confidence across both metrics and every comparable row;
- a joint `>10% AND >50 us` regression threshold for ordinary end-to-end work, or the
  preregistered low-latency public-operation formula from `BENCHMARKING.md` when the frozen baseline
  is below 500 us.

Use `--confidence-level`, `--bootstrap-seed`, and `--bootstrap-resamples` when an experiment needs
explicit reproducibility values. Confirmation requires at least 10,000 bootstrap resamples;
smaller values are diagnostic-only, and values above 100,000 are rejected to keep evidence
generation bounded. Schema-v2 confirmation resolves current and registered historical selectors to
one public operation and takes its divisor from lane metadata. Legacy schema-v1 confirmation instead
requires exact benchmark identity and divisors of one. If A/A is unstable, the required count
exceeds `--max-pairs`, or a decision interval crosses the joint boundary, record the result as
inconclusive. Do not add samples after seeing the classification or relax either threshold.
Exploratory owner-local results may motivate selecting the low-latency gate, but they must be
disclosed and cannot be reused in A/A calibration or fresh public confirmation.

## 6. Run semantic and resource gates

Focused owner tests are appropriate during iteration. Before accepting shared-path code or a durable
checkpoint, run the repository's relevant complete gates serially. The broad parity commands are:

```bash
cargo run -p xtask -- verify --strict
cargo run -p xtask -- compare-all-svgs --check-dom --dom-mode parity-root --dom-decimals 3
```

The exact gate set depends on the owner, but must include applicable semantic/model, layout, SVG,
error precedence, resource-limit, security, host-callback, deterministic-output, and control-fixture
contracts. Native allocation count/bytes and peak live growth use the separate instrumented memory
harness; do not place allocator instrumentation in the latency executable.

### Calibrate a layout-work ceiling

Any change to a public layout-work ceiling requires a clean release build and the fail-closed
calibration wrapper:

```bash
CARGO_BUILD_JOBS=1 cargo build --locked --release -p merman \
  --example layout_work_calibration --features complete-svg

python3 tools/bench/run_layout_work_calibration.py \
  --authoritative-date YYYY-MM-DD \
  --out-dir target/bench/layout-work-calibration-YYYY-MM-DD \
  --timeout-seconds 300 \
  --full-repeats 5
```

The source probe owns the closed corpus and registered headroom rule. The wrapper must bind five
byte-identical full reports, isolated semantic/layout/SVG/end-to-end paths, the exact corpus
maximum at `W` and `W-1`, an adjacent deterministic node/edge cardinality boundary, typed failure
payloads, timeout, peak RSS, and output hashes. Commit a compact evidence manifest under
`docs/performance/evidence/` that binds the ignored raw summary by byte length and SHA-256, then
link a dated decision receipt from `PERF_PLAN.md`. Record the result as `accepted-structural` only;
stage elapsed and RSS observations do not create latency, memory, or host-SLO claims.

The current accepted policy example is
[`interactive_layout_work_calibration_2026-08-07.md`](interactive_layout_work_calibration_2026-08-07.md).

Run the native memory harness only when the owner unit registers that gate:

```bash
python3 tools/bench/run_native_memory.py \
  --json-out target/bench/native_memory.json
```

It prebuilds one instrumented executable, then launches 60 fresh subprocesses by default: matched
operation/zero samples across six fixed scales and five repeats. Treat exit `2` as a protocol or
provenance failure, `1` as a failed bound, `3` as inconclusive, and `0` as passing only the declared
owner contract. The checked-in Flowchart contract is explicitly an `infrastructure-smoke` contract
with broad safety limits and cannot admit a candidate. A candidate unit must freeze tighter bounds
before collecting its adjacent A/B evidence. Use `perf_runner.py --include-native-memory` only when
this explicit cost is intended; no normal latency profile enables it automatically.

For a durable baseline, run the full harness from a clean committed checkout, then freeze its JSON
as the authoritative evidence envelope:

```bash
python3 tools/bench/freeze_perf_baseline.py freeze \
  --native-memory-report target/bench/native_memory.json \
  --out docs/performance/baselines/runtime-<source-commit>.json
```

The new manifest makes the checkout dirty only after it has captured the clean source. Commit the
manifest separately. To verify later, use a clean checkout of the manifest's recorded source commit
as `--repo-root` and pass the manifest from the later receipt commit. Verification rehashes the
source, fixtures, report, executable, and any ordered patch stack, then recomputes native-memory
analysis rather than trusting persisted classifications.

## 7. Interpret schema-v2 evidence

Read the JSON artifact before the Markdown summary. Confirm that it retains:

- both runner recipes, revisions, locks, manifests, toolchains, targets, build commands, host state,
  and executable digests;
- two-sided corpus/fixture byte provenance and capability coverage;
- method, thresholds, logical-operation divisor, pair budgets, confidence method, and deterministic
  bootstrap settings;
- every ordered base/head A/A and fresh AB/BA pair;
- stability and order diagnostics, power-derived required count, paired one-sided relative and
  absolute bounds, and both regression and mirrored improvement classifications.

Interpret process exits with fixed precedence:

| Exit | Action |
| ---: | --- |
| `2` | Repair the evidence contract; there is no performance conclusion. |
| `1` | Treat the suite as a confirmed regression and investigate or revert the candidate. |
| `3` | Preserve the receipt and retest conditions; do not retain unproven candidate code. |
| `0` | Diagnostic completed, or confirmation established suite-wide non-regression. Inspect mirrored improvement fields before candidate admission. |

Candidate admission uses the mirrored improvement bounds, not exit `0`. Accept only when both the
relative and absolute improvement thresholds are confirmed and every mandatory non-performance gate
passes. Reject when improvement is disconfirmed or a mandatory gate fails. Keep an explicit queue
entry and next fixed budget only for a genuinely inconclusive hypothesis.

## 8. Store the receipt

- In-flight Markdown and JSON: `target/performance/` or `target/bench/`.
- Durable adjacent-candidate decision: a concise dated Markdown receipt under `docs/performance/`,
  with the schema-v2 JSON retained as a build artifact.
- Durable cross-renderer context: `docs/performance/renderer_comparison_YYYY-MM-DD.md`.

Never rewrite an old report's recipe to current feature names. Historical evidence must preserve the
exact capabilities, fixture hashes, tools, and revisions used when it was collected.

## Harness contract checks

Run these checks after changing the benchmark infrastructure:

```bash
python3 -m py_compile \
  tools/bench/compare_mermaid_renderers.py \
  tools/bench/compare_self.py \
  tools/bench/corpus_utils.py \
  tools/bench/freeze_perf_baseline.py \
  tools/bench/native_memory.py \
  tools/bench/perf_runner.py \
  tools/bench/render_perf_comment.py \
  tools/bench/run_native_memory.py \
  tools/bench/stage_spotcheck.py \
  tools/bench/test_native_memory_contracts.py \
  tools/bench/test_native_memory_driver_contracts.py \
  tools/bench/test_perf_baseline_manifest.py \
  tools/bench/test_perf_contracts.py \
  tools/bench/verify_pipeline_bench_list.py
python3 tools/bench/test_native_memory_contracts.py
python3 tools/bench/test_native_memory_driver_contracts.py
python3 tools/bench/test_perf_baseline_manifest.py
python3 tools/bench/test_perf_contracts.py
python3 tools/bench/verify_pipeline_bench_list.py
python3 tools/bench/run_native_memory.py --smoke \
  --json-out target/bench/native_memory_smoke.json
python3 tools/bench/test_native_memory_driver_contracts.py \
  --verify-smoke-report target/bench/native_memory_smoke.json
```
