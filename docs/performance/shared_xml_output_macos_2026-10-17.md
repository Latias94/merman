# Shared XML output ownership — macOS, 2026-10-17

The adjacent baseline is `719ec4f3102e0faa2752cbd926985008db33caac`. The XML runtime change is
`f68f63c8b`; the fallback feature correction is `05012a28d`, and the profiling boundary is
`acc58b2e0`. This change optimizes logical stylesheet serialization at the shared XML owner.
It removes duplicate observation of unchanged text, preserves byte admission, and uses bounded
bulk search where the complete
chunk proves that scalar filtering is unnecessary. No public API, cache, dependency, unsafe
code, production profiler, or new persistent output state is introduced.

## Necessary responsibilities and removed work

XML character data must still escape `&` and `<`, suppress forbidden XML scalars, and prevent
`]]>` across formatter fragments and chunk boundaries. Quotes remain literal. Theme resolution
cannot promise these properties for arbitrary font strings or other authored values. The
serializer therefore retains these responsibilities rather than accepting a caller blacklist.
The bounded document sink still owns retained-byte admission before allocation, output growth,
operation cancellation, and sticky detected failures.

For B logical input bytes, P unchanged output bytes, and X replacement bytes, cancellation work
observation previously included B + P + X. It now includes B + X: unchanged fragments carry an
internal already-observed append contract, while entities continue normal generated-byte
observation. This removes a repeated linear term, not a claimed asymptotic rendering improvement
or a reduction of charged resource-budget units. Total work remains O(B + X), with O(1) added
space. There is no scan-credit field or lifetime-spanning assumption.

Every chunk remains at most 2048 bytes plus up to three bytes to finish its UTF-8 scalar.
The complete chunk proof rejects controls and EF-prefixed scalars from bulk searching. An
initial special byte directly selects the scalar writer to avoid the rejected dense-entity
preflight overhead. Both paths normalize identically; the branch changes the algorithm, never
which characters are accepted. Existing `memchr` searches delimiter positions in proven chunks.

Exact entrypoint counters extracted from the real serializer cover 20 CSS inputs. Append counts,
normalized bytes and serialized lengths match on every input. Timeline medium CSS removes
11,405 repeated observation bytes, Kanban 15,302, Class 8,516 and large Flowchart 25,996.
The counter sink characterizes entrypoint bytes, not the private operation meter runtime;
independent source review and actual bounded-sink tests establish that connection.

## Contracts and allocation controls

All 67 public-render inputs produce byte-identical SVG and identical public theme summaries,
diagnostics and capabilities. Five repeated allocator observations per input match baseline
allocation count, cumulative bytes, peak live-byte growth and retained bytes exactly. They cover
all three Timeline looks and theme modes, event scales to 256, special/long/Unicode font values,
Packet, Kanban, Class, large Flowchart, ER, Mindmap and Architecture. Heap counters do not measure RSS.
All 76 pipeline SVG inputs match in both render and end-to-end lanes: 152 identities.

Detected first failures remain sticky. Exact `MaxSvgBytes` values and append boundaries remain
unchanged when byte admission wins. Cancellation remains bounded during plain scanning, discarded
scalars and dense entity expansion; completion observes cancellation unconditionally. Removing
redundant observations intentionally changes internal polling counts. A synthetic cancellation
at checkpoint 1 racing a 2047-byte ceiling on a 2048-byte append can now select the byte-limit
error rather than cancellation. No exact scheduling of concurrent errors or compatibility with
private injected checkpoint numbers is claimed.

## Rejected alternatives and attribution

The initial 32-byte bulk classifier reduced ordinary CSS elapsed time but added about 20% on
dense entity input. Guarded and nested variants retained that tradeoff and were rejected. A per-byte search
choice was also rejected; the final implementation selects scalar versus searchable once per
bounded chunk. No abandoned variant remains in production.

A String-sink microbenchmark compiles the extracted real serializer with one unchanged harness.
Two same-input paired observations over 20 inputs observe ordinary family CSS scanner elapsed-time
reductions of 84%–92%; dense entities range from -1.2% to +2.6%. Six additional hostile shapes
cover ordinary prefixes followed by dense entities, alternating entities, late forbidden scalars, and allowed
newlines. The dense variants improve by 19%–52%; late exceptional bytes add approximately 2%–6%.
These isolate the scanner and String allocation, not bounded public render latency. No claim of
universal speedup follows from these private-stage observations.

The existing profiling example now exposes non-inlined `profile_render::emit_prepared_svg`.
It contains emission and output consumption, matching Criterion's render boundary. Preparation
still occurs freshly outside that frame; whole-process durations still include it. A final macOS
sample confirms the frame is visible. No tracing or profiling crate was added.

## Measurements and validation

The unchanged pipeline benchmark runs 30 Criterion samples, two-second warmup and three-second
measurement. Each row has four observations per side in two ABBA blocks, with exact preflight
SVG identity and successful postflight verification. The table lists medians, not independent
Criterion subsamples. One diagnostic run paused after the first three complete render rows for
hostile-shape attribution, then resumed only the remaining registered observations; no row was
extended or cherry-picked. These observations do not constitute calibrated latency admission.

| Operation | Baseline, microseconds | Candidate, microseconds | Observed change |
| --- | ---: | ---: | ---: |
| render/timeline_medium | 24.320 | 14.560 | -40.13% |
| render/packet_medium | 5.405 | 4.605 | -14.79% |
| render/kanban_medium | 38.537 | 31.937 | -17.13% |
| render/class_medium | 238.475 | 232.200 | -2.63% |
| render/flowchart_large | 1104.950 | 1077.950 | -2.44% |
| end_to_end/timeline_medium | 51.725 | 42.943 | -16.98% |
| end_to_end/packet_medium | 12.141 | 11.479 | -5.46% |
| end_to_end/kanban_medium | 67.302 | 61.471 | -8.66% |
| end_to_end/class_medium | 673.055 | 667.820 | -0.78% |
| end_to_end/flowchart_large | 4589.700 | 4539.900 | -1.09% |

A separate confirmation attempt preregisters the complete public operations Timeline, Packet
and Kanban; baseline estimates are frozen at 52,771 / 12,357 / 70,182.5 ns. The unchanged
`compare_self.py` rebuilds clean baseline `719ec4f31` and candidate `acc58b2e0`, serially freezes
the executables, verifies matching SVG identities, and collects eight balanced A/A pairs per
executable per row: **96 invocations**. Calibration uses 10,000 bootstrap resamples, seed 0,
a simultaneous 95% confidence family of 24 components, and a fixed cap of 32 confirmation pairs.

| Public operation | Absolute A/A noise, ns | Allowed noise, ns | Calibration |
| --- | ---: | ---: | --- |
| Timeline | 856.66 | 1,000.00 | Pass |
| Packet | 202.44 | 617.85 | Pass |
| Kanban | 1,482.73 | 1,000.00 | Fail |

All identity-zero, order, relative-noise and power-cap checks pass. Kanban fails only its
absolute-noise ceiling. As preregistered, this blocks fresh A/B for the **entire three-row set**;
no row is removed, no sample budget is extended, and no calibration data is reused as A/B.
The result is `inconclusive` (exit 3), with zero fresh confirmation pairs and no confirmed latency
improvement or regression. The earlier table remains diagnostic. The implementation is retained
for exact repeated-work removal at the correct owner with unchanged outputs and memory controls;
it does not claim a new asymptotic bound or calibrated latency admission.

The first invocation used an unsupported anchored filter and exited during CLI/corpus contract
validation before building or collecting a sample. Correcting it to the supported
`end_to_end/(timeline_medium|packet_medium|kanban_medium)` selects the same three preregistered
rows. Both the zero-sample failure and the complete calibration evidence remain in the ledger.

A reproducer for the confirmation attempt, after checking out the two clean revisions, is:

```console
python3 tools/bench/compare_self.py \
  --base-dir .worktrees/bench-xml-base-20261017 --head-dir . \
  --base-target-dir target --head-target-dir target --freeze-shared-target \
  --group end_to_end --filter 'end_to_end/(timeline_medium|packet_medium|kanban_medium)' \
  --preset long --evidence-mode confirmation --calibration-pairs 8 --max-pairs 32 \
  --bootstrap-resamples 10000 --bootstrap-seed 0 \
  --low-latency-baseline-ns timeline_medium=52771 \
  --low-latency-baseline-ns packet_medium=12357 \
  --low-latency-baseline-ns kanban_medium=70182.5 \
  --out target/bench/shared-xml-confirmation.md \
  --json-out target/bench/shared-xml-confirmation.json
```

Final scoped validation:

- Native all-feature nextest across six affected packages: 8,242 passed, 13 skipped, including
  all 17 output-owner tests and the independent all-Unicode normalization oracle.
- Private theme qualification: 34 passed. WASM host tests: 47 passed.
- Fresh optimized full Web WASM builds and runtime smoke pass for 37 diagram families, deterministic
  and host-callback measurement routes, and SVG/ASCII outputs.
- All-family SVG DOM checks pass in structure, parity and parity-root modes with the repository's
  existing, explicitly reported browser-text-layout residual handling.
- Workspace all-feature check and all-target/all-feature Clippy pass, including after the auxiliary
  feature fix. Formatting and whitespace checks pass.
- Pure SVG, Timeline, State and Railroad facade-only feature checks pass after the orphan gate fix.
- Eight independent static review lenses complete without actionable findings; the single-line
  feature fix receives a separate light review. Simplification review retains the measured
  scalar/searchable separation and all protected proof tests.

The six-package/native, private and real-WASM/DOM results above use the final XML algorithm before
only the reviewed orphan gate deletion. Default/all-diagram features already select the helper,
so that deletion changes no selected runtime item in these lanes. The final clean candidate
benchmark SHA-256 is identical to the earlier measured candidate executable. All 714 renderer
source hashes are recorded; only `svg.rs` changes between those snapshots.

Raw ledger, rejected implementations, executable/library/source hashes, counter samples,
allocation/report corpus, profiler capture and validation commands are under
`target/bench/experiments/xml-output-owner-20261017/`. Host: Apple M4 Pro, 48 GiB,
macOS aarch64, Rust 1.95.0. Cargo runs serially: focused builds/checks use up to six jobs; the
unchanged comparison tool freezes clean benchmark builds with one job. Timing excludes concurrent
Cargo, profiler and allocator runs. No benchmark work was added to regular CI.

## Separate v0.9.0 context

A frozen tag executable traces to `f754a3c3077d35d2a372c2fec04f9e9b87db1c3f`, with the same
SHA-256 as the October 16/17 receipts. A fresh two-ABBA diagnostic uses the unchanged benchmark
and exact SVG identities on each invocation. These are same-output timing observations;
the tag does not offer the current complete XML/resource/error contract, so they do not prove
security-equivalent performance or full release recovery.

| Operation | v0.9.0, microseconds | Candidate, microseconds | Observed difference |
| --- | ---: | ---: | ---: |
| render/timeline_medium | 18.157 | 14.697 | -19.06% |
| end_to_end/timeline_medium | 39.621 | 43.021 | +8.58% |
| end_to_end/packet_medium | 7.938 | 11.463 | +44.41% |
| end_to_end/kanban_medium | 44.168 | 61.026 | +38.17% |

Timeline emission is now below the tag in this diagnostic; its complete operation still has a
remaining gap. Packet and Kanban retain larger end-to-end gaps and require fresh attribution of
request/theme/preparation/output work. Class and Flowchart remain separate layout/rendering
frontiers. The remaining differences cannot be attributed from percentages alone.

## Feature-boundary defect found by strict verification

The initial `verify --strict` passed workspace all-feature check and Clippy, then failed its
isolated `facade-empty` consumer: the facade imported the public fallback helper while the
renderer incorrectly gated its reexport on Railroad. A separate checkout of baseline `719ec4f31`
reproduces the same E0432 with `--no-default-features --features svg`.

History at `9a297b72f` shows that the gate belonged to a deleted private CSS decoder and was
accidentally left before the previously unconditional public fallback reexport. The helper is
generic, the module always compiles, and SVG-only typography and State tests import it.
Deleting that single orphan attribute restores the original API contract. Adding another family
gate to the facade would instead preserve the defect. Pure SVG, Timeline, State and Railroad
facade combinations each pass a fresh check. The default/all-diagram configuration already enables
Railroad, so this auxiliary deletion changes no code selected in the timing and prior validation
lanes. It receives its own commit and supplemental static review.

The full strict command is not reported as passing. The rerun passes workspace all-feature check,
workspace all-target/all-feature Clippy and isolated consumers, then enters a 661-build release
and transport feature sweep. That overbroad sweep was deliberately interrupted; its receipt and
completed scope are preserved. Android/UniFFI and full release packaging are not certified by this
checkpoint. Existing focused and cross-platform output checks are the applicable change gates.

## Next bounded experiments

These are source-backed hypotheses, not profiler-proven explanations of the remaining tag gap:

1. Packet's `ResolvedFamilyStylePlan::observe_compatibility` counts residuals without cloning,
   then still freezes parse evidence before a no-theme reconciliation returns empty. Attribute
   preparation allocations and public E2E before removing that requestless copy; preserve explicit
   theme error ordering and actual writer outcomes.
2. Packet `packet_style` formats declarations, runs generic safe-declaration parsing and copies
   values even while resolving derived defaults. Attribute style/theme preparation for defaults,
   derived colors and authored values separately. Safe CSS handling remains an input boundary;
   unconditionally deleting its validation would not be a valid fix.
3. Kanban typography resolves a normalized font stack and its CSS binding can resolve the same
   configuration again through `PreparedCommonCss::new`. Attribute preparation before sharing one
   operation-owned resolved value. Preserve distinct layout/CSS size semantics and budget-first
   traversal; ticket/assignee text measurement and XHTML preparation perform real upstream layout
   work and are not redundant merely because their stage changed since the tag.
