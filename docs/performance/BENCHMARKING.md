# Benchmarking

This document defines the measurement contract for `merman`. Use
`docs/performance/RUNBOOK.md` for the operational sequence and
`docs/performance/PERF_PLAN.md` for the active optimization queue.

Performance evidence never overrides semantic parity, deterministic output, resource limits, or
security behavior. A faster result with a different public operation, input, output contract, or
error contract is not an accepted optimization.

## Measurement lanes

Keep these lanes separate because they answer different questions:

- Native Criterion `pipeline` benchmarks measure Merman stages and public end-to-end operations.
- `compare_self.py` compares two Merman revisions on one host. Its confirmation mode is the
  decision-grade regression and candidate-admission lane.
- The stress benches and stage spot-checks attribute work after a public-operation change has been
  observed. They do not independently admit production code.
- `compare_mermaid_renderers.py` and the browser harness provide cross-renderer context. Different
  transports, algorithms, capabilities, fixture bytes, or output-quality contracts must not be
  combined into one aggregate admission number.
- Native allocation, peak-live-heap, Node RSS, and artifact-size evidence use their own harnesses.
  Do not infer memory or size behavior from latency alone.

## Native Criterion benchmarks

The `pipeline` bench can be run directly for local attribution:

```bash
cargo bench -p merman --features svg --bench pipeline
```

It measures:

- `parse/*`: parsing with a reused `Engine`, without layout.
- `parse_cold_engine/*`: request-style parsing with a fresh `Engine` per iteration.
- `compatibility_json_parse/*`: compatibility JSON parsing when the diagram type is already known.
  Historical reports may use its registered `parse_known_type/*` alias.
- `layout/*`: geometry and route computation from a parsed diagram.
- `render/*`: SVG emission from an already laid-out diagram.
- `end_to_end/*`: parse, layout, and SVG emission through the public pipeline.

Bench fixtures live under `crates/merman/benches/fixtures/`. A direct Criterion invocation is useful
for exploration, but its internal samples are not independent base/head observations and cannot by
themselves establish an optimization claim. Use `compare_self.py --evidence-mode confirmation` for
that claim.

### ASCII semantic-depth benchmark

The `ascii_pipeline` bench measures the public synchronous ASCII renderer with strict parsing,
the explicit `UnboundedForTrustedInput` resource profile, Plain 7-bit output, a reused engine, and
one logical render operation per estimate. This is a controlled local batch/stress lane whose
inputs come only from the tracked corpus; the public `Interactive` and `TrustedNative` rejection
boundaries remain covered by exact resource tests rather than benchmark admission:

```bash
cargo bench --locked -p merman --no-default-features --features ascii --bench ascii_pipeline
```

Its schema-v2 metadata is `tools/bench/ascii_corpus.json`. The `closeout` and `large-closeout`
suites cover the five common families selected by U25, while `comparable` contains only fixtures
whose base and head output identities may support a latency comparison. Every registered benchmark
must emit a `plain_ascii` preflight receipt with exact output bytes and SHA-256 before Criterion
timing begins, and an exact invocation must repeat the same identity after timing. Verify the
compiled list and those receipts with:

```bash
python3 tools/bench/verify_pipeline_bench_list.py \
  --bench ascii_pipeline \
  --corpus tools/bench/ascii_corpus.json \
  --features ascii
```

For a decision-grade adjacent-revision comparison, first ensure both checkouts contain the same
benchmark-only harness: `ascii_pipeline.rs`, its `Cargo.toml` bench entry, the selected corpus,
the referenced preflight contract, and every selected fixture must be byte-identical. If the
historical product revision predates that harness, create a clean benchmark-only backport commit
whose parent is the product revision; do not copy only the corpus into an otherwise different
checkout. Then keep the ASCII feature closure and each checkout's corpus path explicit:

```bash
python3 tools/bench/compare_self.py \
  --base-dir ../merman-base \
  --head-dir . \
  --base-label BASE \
  --head-label HEAD \
  --base-package merman \
  --head-package merman \
  --base-bench ascii_pipeline \
  --head-bench ascii_pipeline \
  --base-features ascii \
  --head-features ascii \
  --no-base-default-features \
  --no-head-default-features \
  --base-corpus tools/bench/ascii_corpus.json \
  --head-corpus tools/bench/ascii_corpus.json \
  --suite comparable \
  --group ascii_end_to_end \
  --preset long \
  --evidence-mode confirmation \
  --calibration-pairs 8 \
  --max-pairs 64 \
  --relative-threshold-percent 10 \
  --absolute-threshold-ns 50000 \
  --out target/performance/ascii_comparison.md \
  --json-out target/performance/ascii_comparison.json
```

Both corpus paths are resolved relative to their respective checkout. The benchmark workload,
`UnboundedForTrustedInput` resource profile, bench source, lane contract, and selected fixture
bytes must be identical on both sides. Confirmation mode rejects a different Cargo `[[bench]]`
entry, benchmark source, corpus manifest, or preflight contract before sampling; selected fixture
bytes are checked independently. Do not compare an older checkout's historical harness under the
same operation name. Record the benchmark-only backport revision separately from its product parent
so the final receipt cannot mistake harness provenance for a product change.

The generated Markdown and JSON are machine-local evidence until their exact revision, tree,
environment, corpus and harness digests, output identities, numeric results, and maintainer
disposition are committed in an ASCII closeout scorecard. Until that tracked scorecard exists, a
direct Criterion run or a receipt without those provenance fields is not a U25 closeout claim.

## Decision-grade self-comparison

### Two independent runner recipes

Base and head are separate `RunnerRecipe` values. Each recipe records its own:

- checkout and human-readable label;
- Cargo package and benchmark target;
- feature list and default-feature policy;
- Rust toolchain and optional compilation target;
- Cargo target directory;
- corpus manifest path;
- a corpus schema-v2 lane contract that owns public operation, lifecycle, transport, selector
  history, and the logical-operation divisor.

Do not project the head recipe onto an older revision. Capability names can change across releases.
For example, `v0.8.0-alpha.3` exposes SVG rendering through `render`, while the current candidate
uses `svg`.

This historical comparison makes that difference explicit:

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

The default-feature policy is also side-specific and defaults to enabled. Use
`--no-base-default-features` or `--no-head-default-features` when the corresponding recipe must have
a narrow explicit feature closure and the selected suite does not require a disabled family. Missing
capability for any mandatory fixture is an evidence-contract failure. Use `--base-target` and
`--head-target` for explicit cross-target recipes; otherwise the native target is recorded.

The alpha.3 range is historical context, not causal proof for one optimization. Prove a candidate
against its adjacent clean pre-change commit using the same two-sided recipe contract.

### Build and executable freeze

`compare_self.py` performs build work before any timed pair:

1. It invokes locked Cargo bench compilation independently for each recipe with `--no-run` and
   Cargo JSON diagnostics.
2. It accepts exactly one matching Criterion executable for the requested package and bench.
3. It records and verifies the executable digest and discovers exact benchmark names directly from
   that executable.
4. Every timed sample invokes the frozen executable directly with Criterion's bench mode enabled.

Cargo is therefore outside the timed AB/BA schedule. A missing lockfile, ambiguous executable,
changed digest, absent benchmark, or failed direct invocation is an evidence-contract failure, not
a slow sample.

Base and head should normally use distinct target directories. When disk constraints require
`--freeze-shared-target`, the runner clears only the shared target's `bench` profile before building
each side, then copies that side's executable under `target/perf-frozen`. This reset is required:
Cargo's cache is not a content-addressed boundary between different path-workspace checkouts, so a
plain second `--target-dir` build can otherwise reuse the first checkout's executable. The debug
profile and immutable frozen copies are not removed. Do not run another Cargo process against that
target during either reset, build, or timing schedule. Different Git trees that still resolve to
byte-identical benchmark executables fail the comparison, because they do not provide an
independently built binary delta to measure. The JSON report records the comparison harness digest
as well as both executable digests.

### Corpus and fixture byte gate

The harness loads the corpus independently from both recipes. It compares the selected fixture on
both sides before timing and records each path, byte length, and SHA-256 digest.

Only byte-identical fixtures available to both executables are comparable. Missing or different
bytes remain visible as coverage evidence but cannot produce a timing ratio. A required selection
with no comparable row fails the evidence contract. This prevents source evolution from being
misreported as an engine regression or improvement.

### Diagnostic evidence

Diagnostic mode runs one to four paired observations with alternating AB/BA order, balanced whenever
the requested pair count is even. It is intended for PR triage and hypothesis selection only:

- observed movement is always advisory;
- it cannot accept or reject a performance candidate;
- it exits successfully when timing completes, regardless of movement;
- recipe, byte, capability, runner, and report-contract failures still use exit code `2`.

Do not keep sampling a diagnostic run until it looks favorable. Move to a preregistered confirmation
run when a result will influence production code.

### Confirmation evidence

Confirmation mode separates calibration from the final comparison:

1. Run at least eight order-balanced same-binary A/A pairs for base and for head.
2. Require the two-sided identity and order-effect intervals to remain fully inside the
   preregistered relative and absolute equivalence margins, and estimate dispersion for both
   metrics.
3. Derive the next even confirmation count from each metric using
   `max(8, ceil(((1.645 + 0.842) * sigma / MDE)^2))`.
4. Use the largest required count across every required row and both metrics, bounded by the
   preregistered `--max-pairs` budget.
5. Collect a fresh, even, order-balanced AB/BA confirmation schedule. Calibration and exploratory
   observations are never reused as confirmation evidence.

If A/A is unstable or the derived count exceeds the fixed budget, the result is inconclusive. The
harness does not weaken thresholds, extend the budget after seeing results, or stop when an interval
first becomes favorable.

One normalized Criterion point estimate per side within an AB/BA pair is one independent
observation. Corpus schema-v2 lane metadata owns the public operation and normalization. Current and
registered historical selectors can therefore confirm the same operation even when their Criterion
group names differ. An explicit `--base-logical-operations` or `--head-logical-operations` value must
match the registered divisor. Legacy corpus schema-v1 evidence has no such attestation: confirmation
requires exact benchmark identity and divisors of one, while any override remains diagnostic-only.

### Statistics and decisions

The canonical signed metrics are:

- relative movement: `r = log(head / base)`;
- absolute movement: `d = head - base` in nanoseconds.

Positive values mean regression. The report retains every raw pair and computes deterministic
paired-bootstrap bounds with suite-level 95% simultaneous coverage. For `R` comparable rows, the
confidence family contains `2R` components: relative and absolute movement for every row. A
Bonferroni adjustment gives each component confidence `1 - 0.05 / (2R)`.

The comparator's default screening budget is relative movement greater than 10 percent **and**
absolute movement greater than 50 us. `--absolute-threshold-us 50` is an equivalent convenience
form of `--absolute-threshold-ns 50000`. This default keeps unattended reports comparable; it is
not a universal admission or rejection rule.

Every decision-grade latency or throughput candidate must preregister one materiality contract
before confirmation samples are collected:

- the default joint `10% AND 50 us` budget when it represents the public operation;
- the noise-derived low-latency formula below;
- a documented interactive, throughput, or high-volume integration objective converted into
  operation-level or workload-level bounds; or
- a separate structural or memory claim whose public latency lane is explicitly a non-regression
  control rather than the claimed benefit.

A scenario-specific contract must name the public operation, expected call volume or user-visible
budget, relative and absolute A/A noise floors, and the exact confirmation statistic. Thresholds
must dominate the registered noise floor and cannot be selected or relaxed after looking at
confirmation. A large ratio on a tiny private stage remains attribution evidence, not a public
latency claim.

#### Low-latency public operations

The default absolute threshold is structurally unreachable when the complete public-operation
baseline is below 50 us and can dominate the relative threshold for every baseline below 500 us. A
candidate with a frozen baseline below 500 us may preregister this noise-derived alternative only
before collecting confirmation samples and only for a complete public operation with frozen
base/head revisions and inputs. Private stages remain attribution evidence.

Let `B` be the frozen baseline estimate in nanoseconds per logical operation. Let `e_r` and `e_d` be
the maximum absolute endpoints from the independent base and head A/A simultaneous 95% intervals
for log ratio, absolute delta, and their order effects. A/A is stable only when identity or zero is
inside every interval, `e_r <= -ln(0.95)`, `e_d <= min(1,000 ns, 0.05 * B)`, and the power-derived
pair count does not exceed the preregistered cap. Define:

```text
T_r = max(-ln(0.90), 3 * e_r)
T_d = max(1,000 ns, min(50,000 ns, B * (1 - exp(-T_r))), 3 * e_d)
```

Fresh confirmation accepts the performance claim only when the simultaneous one-sided 95% upper
bounds for `r = ln(head/base)` and `d = head-base` are below `-T_r` and `-T_d`. Derive one fixed even
AB/BA pair count from both thresholds using the normal power procedure, with at least eight pairs
and a preregistered cap no greater than 64. Do not reuse discovery or A/A observations, remove rows,
change the lifecycle, or extend the budget after seeing confirmation.

The low-latency gate handles measurement scale; it does not erase engineering cost. A candidate
that adds persistent state or a large owner implementation must also preregister and pass a concrete
memory, throughput, or documented integration objective, keep cold and unaffected paths within
their noise budgets, and remove the superseded production path and temporary oracle before
acceptance.

Conversely, an owner-local change may be accepted under a structural or memory claim without a
latency label when it removes reachable repeated work, improves a user-controlled complexity bound,
or reduces allocation/retained state with exact counters and semantic parity. The smaller and more
obvious the implementation, the less additional machinery it should require. New caches, protocol
machinery, persistent state, or broad abstractions require correspondingly stronger public
latency, throughput, or memory evidence.

Confirmation fixes `--bootstrap-resamples` at a decision-grade minimum of 10,000. Smaller values
are permitted only for diagnostic exploration and can never produce a confirmation outcome;
values above 100,000 are rejected to keep evidence generation bounded.

Per row:

- `confirmed_regression`: both regression lower bounds clear their positive thresholds;
- `confirmed_non_regression`: either regression upper bound cannot clear its threshold;
- `inconclusive`: the interval crosses the joint decision boundary, A/A is unstable, or the fixed
  measurement budget is insufficient.

Candidate admission is a different question from regression gating. It uses the mirrored
improvement view of the same paired bounds. A candidate is performance-confirmed only when both
mirrored improvement bounds clear the relative and absolute thresholds. It is accepted only when
that improvement and all applicable semantic, error, resource, memory, security, host, and control
gates pass. A suite exit of `0` is not proof of improvement.

Suite exit precedence is fixed:

| Exit | Meaning |
| ---: | --- |
| `2` | Evidence-contract failure, including invalid recipes, byte/capability gaps, runner errors, or no comparable required row. |
| `1` | At least one confirmed regression. |
| `3` | At least one required confirmation row is statistically inconclusive. |
| `0` | Complete diagnostic advisory, or suite-wide conclusive non-regression. |

The precedence is `2 > 1 > 3 > 0`. Candidate accepted/rejected/inconclusive state must be read from
the mirrored per-row fields and mandatory non-performance gates, never inferred from this process
code alone.

### Structural complexity and resource amplification

The latency gates above are not universal admission criteria. A separately preregistered
structural lane may accept a repair without a measurable ordinary-fixture speedup when an
externally reachable input controls repeated work, the old implementation contains a higher-order
or unbounded term, and the candidate removes or caps that term.

Freeze the evidence model before implementation:

- name the independent input variables and the resource limit that bounds each one;
- state the reachable pre-change and post-change time and added-space bounds, distinguishing
  worst-case bounds from expected hash-table behavior;
- identify the exact loop, lookup, rebuild, or expansion that realizes the old term;
- choose code-path proof plus an exact test-only work counter, or a preregistered multi-scale curve
  when the bound cannot be established directly;
- preserve semantic output, structured errors and precedence, security policy, resource charges,
  cancellation, and deterministic ordering;
- run the existing representative public lane as a non-regression control when practical, and add
  a memory bound when the data-structure change adds scale-sensitive state.

Acceptance requires a reviewed causal derivation and passing correctness/resource gates. A noisy
or sub-threshold clock result does not invalidate a proven removal of attacker-controlled
quadratic work, but a confirmed material public-workload regression still requires rejection or an
explicitly approved tradeoff. Describe the result as, for example, `O(A^2)` to expected `O(A)` or
`O(T * (V + E))` to `O(T + V + E)` with its added-space bound. Do not call it a latency speedup
unless a separate public-operation timing confirmation passes.

Prefer existing correctness suites plus one combined adversarial scale test over a permanent
fixture-specific benchmark. Delete candidate-only counters, lanes, contracts, and generators after
rejection; retain infrastructure only when it serves a stable recurring contract.

### Layout-work policy calibration

A public layout-work ceiling is a host-safety policy, not a latency benchmark and not a Mermaid
semantic constant. Calibrate it from a closed, hashed corpus plus a rule registered before the
decision. The rule must state its absolute headroom, relative headroom, and rounding quantum so the
selected ceiling is reproducible rather than chosen after observing the maximum fixture.

The release probe and wrapper must establish all of the following:

- a clean source revision, exact release feature closure, toolchain, executable hash, corpus
  manifest hash, and aggregate member hash;
- multiple fresh full-process runs with byte-identical reports;
- the maximum corpus member, its source/output hashes, and the calculated headroom;
- exact adjacent `W/W-1` admission behavior with a structured rejection payload;
- a preregistered model-cardinality curve whose first rejection is established either by a
  complete sequential accepted-prefix scan or by a reviewed monotonicity proof, scoped only to
  that curve;
- isolated semantic, layout, SVG, end-to-end, and failure probes, each in a fresh timeout-bound
  managed process group with exit status and peak RSS; cleanup must not infer group exit from the
  leader or inherited pipe state, and its regression suite must run in the performance-contract CI;
- a committed evidence manifest that binds ignored raw reports and timing output by byte length and
  SHA-256.

These observations can admit an `accepted-structural` resource-policy decision. They do not admit a
speedup, memory improvement, portable memory limit, or host SLO without a separate candidate-local
A/B or memory contract.

### Report schema

Self-comparison reports use schema version `2`. The JSON is the audit artifact; Markdown and PR
comments are projections of it. It records:

- both full runner recipes and revisions;
- Cargo lock, manifest, toolchain, build-command, target, environment, host, and executable-digest
  provenance;
- corpus and fixture paths, byte lengths, hashes, capability discovery, and coverage gaps;
- evidence mode, thresholds, confidence settings, bootstrap seed/resamples, pair budgets, and the
  declared/effective lane contract and logical-operation divisor;
- base/head A/A calibration, order/stability diagnostics, derived power requirement, and every raw
  pair;
- fresh AB/BA confirmation pairs, one-sided relative and absolute bounds, regression state, mirrored
  improvement state, aggregate outcome, and exit code.

`--out` and `--json-out` must resolve to distinct files, including after resolving `..` components
and symbolic links. The runner rejects aliases before building either candidate.

Consumers must fail closed on an unknown schema. Schema version `1` can be displayed only as legacy
diagnostic evidence and cannot support a current admission decision. The comment renderer always
writes an available failure projection, including for nonstandard signal/OOM exit codes, then exits
`2` when its consumer contract fails. CI combines that status with the producer exit code.

## Native allocation and peak-live evidence

Run the isolated native `System` allocator lane explicitly:

```bash
python3 tools/bench/run_native_memory.py \
  --json-out target/bench/native_memory.json
```

The driver builds a dedicated instrumented executable once, freezes its SHA-256 digest, and then
launches one fresh process for every operation or matched zero-work sample. The renderer is prepared
before the allocator snapshot, so `fresh-process` describes sample isolation while `reused-engine`
describes the measured public operation. This executable is never used for latency evidence.

The registered contract fixes six scales (`1, 2, 4, 10, 32, 100`), one seed across the matrix, at
least five repeats, and alternating operation/zero order. Strict one-line JSON records executable and
invocation identity, generated node/edge counts, deterministic SVG identity and dimensions,
allocation count, allocated bytes, and peak live-byte growth. The analysis subtracts each matched
zero sample, retains per-scale medians, and uses deterministic matched-vector bootstrap bounds for
the OLS slope and scale-100 median.

The Flowchart generator uses three nodes and four edges per scale unit, so its `100x` boundary stays
inside the default layout-work policy while still exercising 100 clusters, 300 nodes, and 400 edges.
Protocol smoke runs matched operation/zero pairs at both `1x` and `100x`; this catches setup leakage
and resource-budget drift before a full 60-process matrix.

An upper bound at or below the cap passes; a lower bound above the cap fails; a crossing interval is
inconclusive. Exit precedence is the same `2 > 1 > 3 > 0` contract used by latency evidence. The
checked-in `infrastructure-smoke` owner contract contains broad harness-safety bounds only and has
`candidate_admission: false`. Every optimization owner must preregister tighter adjacent-candidate
memory bounds and combine them with its semantic and latency gates.

The aggregate runner keeps this lane opt-in:

```bash
python3 tools/bench/perf_runner.py --profile triage --include-native-memory
```

A full native-memory report records the Git commit/tree and dirty disposition, workspace/package
manifest and lock digests, build environment, requested and actual Rust toolchain, executable digest,
and the complete raw schedule. Full evidence rejects a dirty worktree by default. `--allow-dirty`
exists only for investigation, forces `candidate_admission` false, and cannot create a durable
receipt. Protocol smoke is likewise never admission evidence.

After a clean full run, freeze the report, source inputs, every corpus fixture, recipe, host, and
executable into the authoritative receipt envelope:

```bash
python3 tools/bench/freeze_perf_baseline.py freeze \
  --native-memory-report target/bench/native_memory.json \
  --out docs/performance/baselines/runtime-<source-commit>.json
```

The freezer revalidates every persisted protocol response and recomputes paired adjustments,
bootstrap bounds, metric classifications, and aggregate exit precedence before accepting the
report. A report by itself is not a frozen baseline. The manifest describes the measured source
commit, so verify it from a clean checkout of that recorded commit while passing the manifest path
from the receipt commit:

```bash
python3 tools/bench/freeze_perf_baseline.py verify \
  --repo-root ../merman-source-commit \
  --manifest docs/performance/baselines/runtime-<source-commit>.json
```

## Cross-renderer comparison

If `repo-ref/mermaid-rs-renderer` is available, this command produces fixture-level diagnostic
context:

```bash
python3 tools/bench/compare_mermaid_renderers.py \
  --preset long \
  --suite standard \
  --expected-merman-rev <merman-commit> \
  --expected-mmdr-rev <mmdr-commit> \
  --mmdr-toolchain 1.92.0 \
  --out target/bench/renderer_comparison.md \
  --json-out target/bench/renderer_comparison.json
```

The runner rejects dirty Merman or mmdr checkouts before starting Cargo. `--allow-dirty` is available
for explicitly diagnostic work: it fingerprints the tracked diff and every untracked file, then
rechecks both repositories and all benchmark-owned inputs after sampling. A revision, worktree,
fixture, runner, lockfile, config, or browser-bundle change fails the contract with exit `2`.

Use `--skip-mermaid-js` when the optional Puppeteer lane is not required. Available suites can be
listed with `--list-suites`; `standard`, `cross_family`, `flowchart`, `stress`, and `full` broaden the
registered native corpus. They are not the complete supported-family inventory: `full` means every
registered fixture, not every Merman family.

Before a fixture-level ratio is shown, the harness compares the exact bytes used by both native
benches. Missing, skipped, errored, or non-identical fixtures remain coverage facts rather than being
treated as fast or slow. Do not use a family aggregate or a ratio spanning unlike transports as an
optimization gate.

Schema `3` retains repository and input provenance, family-level coverage, contract status, the
Criterion console mid-point estimate kind, and all Mermaid.js per-call samples with p50/p95/p99.
Native Criterion raw samples are not retained, so the report remains diagnostic and explicitly sets
`baseline_eligible` to false. The runner also performs a Mermaid.js SVG preflight, but DOM and raster
parity remain separate gates.

The steady-state modes are also distinct:

- Merman: native Criterion `end_to_end/*`.
- `mermaid-rs-renderer`: its native Criterion `end_to_end/*`.
- Mermaid.js: repeated warm `mermaid.render()` calls in one Puppeteer/Chromium process.

Cold CLI startup, Node N-API, browser WASM, DOM comparison, and raster comparison stay in separate
lanes with explicit lifecycle and output contracts.

## Browser comparison with Mermaid.js

Use the generated family-baseline corpus for the web-to-web comparison. The full command builds the
current Web artifact and measures one baseline from each supported generated-example family:

```bash
npm --prefix playground run benchmark:corpus -- \
  --iterations 6 \
  --warmups 2 \
  --out target/bench/browser-family-corpus.json
```

After an already verified playground build, select fixtures for a diagnostic rerun with:

```bash
npm --prefix playground run benchmark:corpus:built -- \
  --fixtures c4-system-context,mind-map,requirement-trace \
  --iterations 6 \
  --warmups 2 \
  --out target/bench/browser-family-targeted.json
```

The corpus runner derives its inventory from generated examples whose evidence role is
`family-baseline`. It starts a fresh Chromium process for every selected fixture to bound retained
renderer state, and each page accepts exactly one fixture id plus its precomputed cold and warm
seeds. Within that fixture, Merman and Mermaid.js still use the same page, source bytes, theme,
viewport, warmup policy, iteration count, and deterministic AB/BA sample plan. Independent cold and
warm controller runs report first-publishable SVG and reused-engine publishable SVG separately. Both
engines pass through the production browser adapters and the same strict SVG evidence checks.

Corpus schema `2` separates the single-fixture page envelope from the aggregate corpus report. A
page returns exactly one success or structured failure row plus stable generated-catalog identity;
it does not fabricate `pending` or `not-selected` rows for the rest of the catalog. The CLI converts
page crashes, early closes, browser disconnects, timeouts, invalidation, and cancellation into the
same fixture envelope, then one shared linear assembler validates order, seeds, options, catalog and
version identity while deriving coverage, failures, and terminal status. The aggregate embeds raw
schema-`6` controller reports, fixture byte counts and SHA-256 digests, actual execution order, and
fresh-process isolation. It never overwrites an existing output. A whole-corpus budget rejects plans
that would retain more than 4,096 raw samples before the first fixture runs. In each successful
fixture mode's `report.aggregates.ratios`, the numerator is Mermaid.js and the denominator is Merman,
so values greater than `1` favor Merman.

An even `--iterations` value is required by the balanced plan. A two-iteration, zero-warmup run
is useful only to verify coverage and discover large candidates. The default six iterations and two
warmups provide better diagnostic ranking, but they are not a universal admission threshold. Use a
targeted rerun, inspect raw samples, variance, and absolute milliseconds, then confirm any production
change against the same Merman base/head public operation before accepting it.

`tools/bench/mermaid_js_bench.cjs` supplies the Mermaid.js side through the pinned
`tools/mermaid-cli` dependencies for the cross-transport native diagnostic lane above; it is not the
web-to-web admission lane. Interactive visual comparison is documented in
`docs/workstreams/web-wasm-playground/MERMAID_COMPARE_MODE.md`.

## Stage and render diagnostics

Use the stage spot-check only after a public-operation signal needs attribution:

```bash
python3 tools/bench/stage_spotcheck.py \
  --preset long \
  --fixtures flowchart_medium,class_medium \
  --out target/bench/stage_spotcheck.md
```

When `render/*` is slow, direct `merman-render` callers can enable
`SvgDebugOptions::include_timing_diagnostics` through an explicit `*_with_debug` entry point. Keep
timing diagnostics disabled in the measured production path. The API is documented in
`crates/merman-render/README.md`.

Stress benches batch small operations to amplify fixed costs:

```bash
cargo bench -p merman --features svg --bench flowchart_stress -- --noplot --sample-size 50 --warm-up-time 2 --measurement-time 3
cargo bench -p merman --features layout-cytoscape --bench architecture_layout_stress -- --noplot --sample-size 50 --warm-up-time 2 --measurement-time 3
cargo bench -p merman --features layout-cytoscape --bench architecture_stress -- --noplot --sample-size 50 --warm-up-time 2 --measurement-time 3
cargo bench -p merman --features svg --bench mindmap_layout_stress -- --noplot --sample-size 50 --warm-up-time 2 --measurement-time 3
cargo bench -p merman --features svg --bench text_measure_stress -- --noplot --sample-size 50 --warm-up-time 2 --measurement-time 3
```

These results are owner-local diagnostics. Record and normalize their fixed-repeat divisor before
comparing them, and confirm the corresponding public end-to-end operation before accepting code.

## Host discipline

- Compare clean committed revisions on the same host.
- Keep the toolchain, target, power mode, thermal state, and background load stable.
- Prebuild serially and do not run Cargo while the paired schedule is sampling.
- Freeze the experiment configuration before viewing confirmation data.
- Retain the schema-v2 JSON report even when the result is rejected or inconclusive.
