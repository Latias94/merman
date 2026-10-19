# Timeline writer outcomes — macOS, 2026-10-17

This checkpoint removes Timeline's redundant theme certification while preserving drawing and
reporting semantics. The adjacent baseline is `3c087bf76c2a6cb9cfd9e59047937ecccc1d3e75`; the runtime change is
`51f8dd61453b7e539809993da20648af485791d6`.
The accepted result is exact allocation reduction with identical SVG and public theme evidence.
Timing is diagnostic; no calibrated latency or complete release-recovery claim is made.

## Which work remains necessary

Theme resolution still selects actual winning properties, respects source ownership, and supplies
paint, canonical opacity tokens and clamped radii to the renderer. Layout still measures text.
XML serialization, retained-byte admission, operation cancellation and real sink failures remain
at their owning boundaries.

The renderer no longer clones expected event/palette records, walks layout to certify text and
stroke counts, rereads emitted SVG attributes, rebuilds expected path commands, or compares its
own selected font/fill strings against copies. Successful write sites collect consumed properties;
outcomes are published only after the complete SVG succeeds. One family owner holds the requested
mechanisms, and all three Timeline consumers contribute outcome deltas.

Used palette slots still require discovery. Their inverse colors are now computed once per slot,
not once per node. Event counts remain because effect-only requests have an independent empty
versus nonempty applicability contract. Necessary event decisions remain; the change does not
claim to remove all traversal or make complete rendering sublinear.

For event count E, node count N and text-run count L, the removed work includes an additional
O(E) path reconstruction and O(E + N + L) expected-record preparation/cloning. These are redundant
linear terms, not a claimed end-to-end asymptotic improvement. Actual rule outcomes retain only
winning rule identifiers; live geometry and paint preparation remain operation-owned.

## Controlled allocation evidence

Both optimized native probes use the repository's unchanged `CountingSystemAllocator`, the same
public `Renderer::render` operation, deterministic environment, strict parsing, trusted-native
resources, fixed SVG id and fresh request construction. Theme compilation occurs outside counting.
Each input runs in a fresh process, warms once, verifies a zero-allocation empty region, and repeats
five times with output retained at the measurement boundary. Counts and the first four allocator
metrics are identical across each row's five repeats; counters never overflow or underflow.

The 57 inputs include five event counts (0, 1, 16, 64, 256), three looks (classic, Redux, Neo),
three theme modes (none, text, complete event/typography theme), the original medium fixture,
empty/title-only/source-color/XML-font cases, and Kanban/Class/large Flowchart controls.
All 57 SVGs match baseline bytes, and their public theme summaries, diagnostics and required
capabilities match. These are 54 Timeline inputs and three other-family controls.

| Input | Baseline allocations | Candidate allocations | Removed allocations | Removed cumulative bytes |
| --- | ---: | ---: | ---: | ---: |
| Timeline medium | 844 | 795 | 49 | 4,493 |
| Classic, 1 event | 1,339 | 1,305 | 34 | 3,765 |
| Classic, 16 events | 6,550 | 6,201 | 349 | 16,395 |
| Classic, 64 events | 23,477 | 22,120 | 1,357 | 56,811 |
| Classic, 256 events | 91,149 | 85,760 | 5,389 | 218,475 |
| Classic, 256 events, complete theme | 104,228 | 94,698 | 9,530 | 417,001 |
| Redux, 256 events, complete theme | 106,119 | 96,588 | 9,531 | 417,008 |
| Kanban medium | 671 | 671 | 0 | 0 |
| Class medium | 11,973 | 11,973 | 0 | 0 |
| Flowchart large | 102,587 | 102,587 | 0 | 0 |

For the five generated classic/no-theme scales, removed allocations follow `21 * E + 13`
exactly. This is a measured property of this fixed corpus, not a universal per-event allocation
contract; the deleted path-token reconstruction explains its event-dependent component.

Every Timeline row has fewer allocations and fewer cumulative allocated bytes. Peak live-byte
growth decreases on 31 inputs and is unchanged on the others; no row increases. Retained bytes
are identical on every input. The three controls match all four metrics exactly. Peak/live-heap
metrics do not measure process RSS or allocator-resident pages.

## Rejected direct CSS streaming

The first candidate wrote formatted CSS fragments directly through `XmlTextOutput`. It removed
more allocations but increased XML sink/checkpoint traffic. Four paired diagnostic estimates
observed Timeline's render stage at 27.446 -> 30.333 microseconds (+10.5%) and public end-to-end at
55.689 -> 58.448 microseconds (+5.0%). These are diagnostic observations without A/A calibration.

It also changed output-buffer growth thresholds. At 256 unthemed Redux events, cumulative bytes
increased from 5,934,123 to 6,163,487, peak growth from 971,404 to 1,022,754, and retained bytes from
295,151 to 479,599. SVG bytes and actual resource ceilings were unchanged, but this was a worse
tradeoff for the investigated workflow. The complete rejected patch, binaries and raw results
remain in the experiment's `rejected-direct-stream` directory.

The final candidate retains one transient CSS batch, then serializes it through one XML text
writer call and releases it before drawing nodes. It removes the separate root-rule buffer,
copied font/color metadata and certification state. No output-size heuristic, global cache,
extra prepass, blanket shrinking step or new dependency was introduced. CSS batching has a
measured purpose under the current output interface; removing an allocation alone is insufficient
reason to remove that step.

## Final adjacent-revision diagnostics

The unchanged `pipeline` benchmark ran two ABBA blocks: four estimates per executable per row,
30 Criterion samples, two-second warmup and three-second measurement. The table compares medians
of those estimates. Each invocation verifies matching preflight SVG identity and successful
postflight identity. Builds, tests, allocation counting and profiling do not overlap timing.

| Operation | Baseline, microseconds | Candidate, microseconds | Observed change |
| --- | ---: | ---: | ---: |
| Timeline render | 28.213 | 25.353 | -10.14% |
| Timeline public end-to-end | 56.546 | 52.771 | -6.68% |
| Kanban public end-to-end control | 72.927 | 70.183 | -3.76% |
| Class public end-to-end control | 689.750 | 689.970 | +0.03% |

The render stage excludes preparation through Criterion's `iter_batched`; the public end-to-end
row includes fresh request construction, parsing, preparation, rendering and output destruction.
No A/A calibration or fixed-power confirmation was run, so these observations do not establish
confirmed improvements or blanket latency non-regression. Allocation reduction is the admitted
result. Unrelated controls are not attributed to Timeline changes.

## Validation and reproducibility

- Final native nextest: 8,239 passed, 13 skipped, across the six selected core/render/facade/export/
  bindings/theme-contract packages with all features.
- Final private theme qualification: 34 passed.
- All-family DOM structure/parity/parity-root passed before the CSS batching revision; the final
  batching revision passed fresh Timeline checks in all three modes.
- Final native benchmark discovery: all 76 SVG inputs match baseline bytes in both render and
  end-to-end lanes (152 identities).
- Final 57-input public allocation/report comparison matches exactly; no failed render is counted
  as a performance result.
- Final Clippy with `-D warnings`, formatting check, and Timeline-only feature compilation pass.
- Eight independent review lenses, including focused review of the final CSS batch revision,
  report no actionable findings.
- Existing tests retain source ownership, title-only typography, referenced versus unused markers,
  palette rings, mixed/unsupported rules, canonical f32 tokens, negative zero, horizontal/vertical
  radius clamps, exact SVG byte ceilings and cancellation coverage. The shared XML font matrix
  now includes Timeline.

An intermediate no-event-route outcome returned a requestless full ledger and dropped typography
results during merging. The existing source-owned font-size integration test caught it. All three
consumer plans now return outcome deltas, including the event baseline. The final measurements
and checks above postdate that fix.

Raw ledgers, frozen executable/library/probe digests, five-repeat allocation samples, SVG/report
identities, validation commands and rejected variants are under
`target/bench/experiments/timeline-consumer-outcomes-20261017/`. The baseline library digest also
matches the previous checkpoint's independently frozen head library. The identical probe source
links against different, explicitly recorded baseline and candidate libraries. The production
source fingerprint covers all 714 renderer Rust files and matches the final candidate.

Host: Apple M4 Pro, 48 GiB, macOS aarch64, Rust 1.95.0. Cargo runs serially with up to six jobs.
No benchmark or new diagnostic dependency was added to production or regular CI.

Optimized macOS `sample` captures use the existing `profile_render --stage render` example,
12-second process with a five-second/one-millisecond sample. That example includes repeated
preparation/layout and is not the Criterion render-only contract. Its remaining prominent stacks
include `XmlTextOutput::write_str`, memory copying, allocation and text measurement. It identifies
the shared XML serialization boundary as a next attribution target; the sample is not a timing
acceptance test or a reason to remove XML/security/resource checks.

Source inspection also distinguishes the tag's work: v0.9.0 Timeline interpolates its CSS batch
straight into `<style>`, while the current writer serializes logical CSS as XML character data
and admits retained bytes through the bounded sink. Ordinary safe-fixture SVG bytes match, but
that does not make their complete security/resource/error contracts equivalent. The remaining
XML work is a real responsibility to optimize at its owner, not another theme receipt to delete;
this source difference alone does not quantify its share of the remaining release gap.

## v0.9.0 context

The immutable tag executable is traced to `v0.9.0` / `f754a3c3077d35d2a372c2fec04f9e9b87db1c3f`
through the October 16 comparison receipt. Its SHA-256 matches that original receipt. A fresh
separate two-ABBA-block run compares only identical-output Timeline rows against the final
candidate, using the same original benchmark, feature profile and sample parameters.

| Operation | v0.9.0, microseconds | Candidate, microseconds | Observed difference |
| --- | ---: | ---: | ---: |
| Timeline render | 18.444 | 24.351 | +32.02% |
| Timeline public end-to-end | 40.331 | 53.038 | +31.51% |

These are 16 fresh diagnostic invocations with pre/postflight identities, not calibrated latency
admission or security/error-contract equivalence. The cleanup narrows the observed Timeline gap
but does not restore tag latency or prove a complete cross-family release recovery. The shared
XML serialization boundary is the next measured target; Class and Flowchart optimization remain
separate work.
