# Performance Plan

This is the single current performance backlog for Merman. It records what to optimize and why.
Use [RUNBOOK.md](RUNBOOK.md) for the execution loop and [BENCHMARKING.md](BENCHMARKING.md) for
measurement semantics and tool details. Dated reports are evidence for a particular revision and
host; they are not rolling sources of truth.

## Current evidence

The release-range baseline and latest committed checkpoints were measured on 2026-07-27 and
2026-07-28:

- [Alpha.3 to Alpha.4 Refactoring Report](../release/ALPHA3_TO_ALPHA4_REFACTORING_REPORT.md)
  compares `v0.8.0-alpha.3` with `d2698d0a3`.
- [Three-runner checkpoint](renderer_comparison_2026-07-27.md) compares that historical candidate
  with Mermaid.js 11.16.0 and `mermaid-rs-renderer` at `7ff1196`.
- [Current mmdr checkpoint](renderer_comparison_2026-07-28_75c9fd156_vs_mmdr.md)
  compares measured source `75c9fd156` with the same mmdr revision using the long preset and
  byte-identical input gating.
- [Current Node transport checkpoint](NODE_TRANSPORT_ADMISSION.md) compares N-API and Node-WASM
  from source `5f540c08d` with one capability recipe and the independently validated 4,001-case
  schema-2 corpus. It remains a private, inconclusive macOS arm64 result rather than a selected
  transport.
- [Runtime evidence manifest](baselines/runtime-06616dd71.json) freezes source `06616dd71`, the
  native-memory executable, full six-scale report, recipes, fixtures, and host. Its
  `infrastructure-smoke` owner contract is not candidate-admission evidence.
- [Flowchart work-accounting decision](flowchart_layout_work_accounting_2026-08-04.md) accepts the
  single render-owned Dagre/ELK work meter as a structural repair. Its causal public controls
  confirmed `flowchart_medium` at -2.60% and `flowchart_nested_clusters` at +3.09%, both within the
  registered non-regression gate.
- [Sequence metric-reuse decision](sequence_operation_metric_reuse_2026-08-07.md) accepts the
  private built-in operation sidecar for the two registered high-message lanes, which improved by
  40.00% and 40.88%; its six-scale allocation curves also passed.
- The current-base Requirement and Mindmap U9 hypotheses are closed independently:
  [Requirement](requirement_metric_reuse_2026-08-07.md) is rejected as written and by upper bound,
  while [Mindmap](mindmap_metric_reuse_2026-08-07.md) is rejected as superseded by U8.

The authoritative date for these hardening decisions is 2026-08-07. Automatic later timestamps
from the local machine are known clock drift and are excluded; receipts bind revisions,
executables, fixtures, and raw reports by digest.

| Comparison lane | Shared rows | Median ratio | Geometric mean | Faster / slower |
| --- | ---: | ---: | ---: | ---: |
| `d2698d0a3` / alpha.3, complete SVG product | 34 | 1.10x | 1.09x | 13 / 21 |
| `d2698d0a3` / alpha.3, minimal same-capability SVG | 32 | 1.12x | 1.07x | 10 / 22 |
| `75c9fd156` / `mermaid-rs-renderer`, identical inputs | 30 | 0.664x | 0.297x | 18 / 12 |
| `d2698d0a3` native / warm Mermaid.js browser | 34 | 0.0237x | not reported | 34 / 0 |

The cross-runner rows are not quality-adjusted rankings. Merman and `mermaid-rs-renderer` have
different Mermaid coverage and output goals, and native Rust versus warm Chromium is not a
browser-WASM comparison. The mmdr lane excludes Treemap and XYChart because their same-named
fixtures differ; `flowchart_large` and Info have no shared measurement.

## Triage policy

An ordinary regression enters the active queue when both conditions hold:

- the same-host A/B regression exceeds 10%; and
- the absolute median increase exceeds 50 microseconds.

A fixed 50-microsecond threshold cannot classify a public operation whose complete baseline is
itself below 50 microseconds. For a frozen baseline below 500 microseconds, a candidate may instead
preregister the low-latency gate in [BENCHMARKING.md](BENCHMARKING.md). That gate derives both
thresholds from the baseline and independent A/A noise, still requires fresh public-operation
confirmation, and adds a memory, throughput, or documented high-volume objective when the
implementation adds substantial machinery. It is not an automatic exception for a large ratio on
a tiny private stage.

A workload can also bypass the ordinary threshold when it crosses an interactive frame budget,
materially changes throughput or memory use, or affects a documented high-volume integration. The
exception and its evidence must be frozen before confirmation samples are collected.

An externally reachable complexity or resource-amplification repair uses a different evidence
class. It must name the user-controlled variables, prove the reachable old/new time and added-space
bounds, preserve semantic and resource-accounting behavior, and keep representative public work
within its non-regression budget. Such a result may be accepted as a structural repair without
clearing a latency threshold, but it is not a measured speedup unless a separate end-to-end timing
gate passes.

Before implementation:

1. Select and preregister the admission class.
2. Match capabilities between base and target revisions.
3. Attribute parse, layout, render, and end-to-end stages.
4. For timing or throughput, run at least eight balanced base and head A/A calibration pairs,
   derive the fixed even AB/BA confirmation count from the preregistered MDE, and collect fresh
   order-balanced pairs within the fixed maximum budget. For structural work, freeze the input
   variables, old/new time and space bounds, and exact counter or scale-curve method instead.
5. Record model size, SVG bytes/elements, and the relevant semantic, DOM, or raster parity result.
6. Profile only after the slow stage is known.

## Priorities

| Priority | Fixture | Current latency | Current / alpha.3 | Current / mmdr | User impact |
| --- | --- | ---: | ---: | ---: | --- |
| P1 | `requirement_medium` | 196.96 us | Historical candidate was 2.12-2.25x before the focused fix | 2.77x (+125.88 us) | Operation-scoped label reuse removed 60.6% of SVG emission. Latest gaps are layout (2.81x) and SVG emission (3.85x), while parse is 0.72x mmdr. |
| P1 | `mindmap_medium` | 165.26 us | No confirmed current regression; the ordinary-label fix reduced its pre-fix latency by 75% | 2.23x (+91.20 us) | The layout ratio compares real COSE-Bilkent with mmdr's radial fallback, so only same-algorithm work is actionable. |
| P1 | `flowchart_large` | 23.20 ms | about 1.00x at the historical checkpoint | unavailable | This is not a refactor regression, but it is the only standard fixture beyond a 16.7 ms frame. |
| P1 | `class_medium` | 900.36 us | Historical candidate was 1.10-1.15x (+89-126 us) | 0.38x | A common family worth rechecking against alpha.3; Merman remains about 2.6x faster than mmdr. |

Do not prioritize the current Info, Packet, Radar, or Sankey ratios without new absolute-cost
evidence. Their measured alpha.3 increases are about 2-5 microseconds. Architecture,
`flowchart_small`, Gantt, and `flowchart_ports_heavy` are within the current noise band.

Kanban is no longer a cross-runner P0: it now measures 57.66 us versus mmdr's 28.19 us, a 29.47 us
absolute gap below the active threshold. Its alpha.3 lane must be rerun before declaring the
historical release regression closed. XYChart is not a valid cross-runner priority until both
runners use the same fixture.

## Completed work

### 2026-08-07 headless hardening decisions

| Unit | Decision | Durable boundary |
|---|---|---|
| U5 Flowchart work accounting | `accepted-structural` | One render-owned meter spans Dagre and ELK through neutral lower-crate controls; public causal controls are recorded in [the U5 receipt](flowchart_layout_work_accounting_2026-08-04.md). |
| U6 ELK hierarchy preparation | `accepted-structural` | Stable scope ownership and iterative postorder remove suffix cloning and repeated descendant discovery. A source-equivalent synthetic adjacent pair passed eight-pair public ELK A/A and AB/BA controls with exact SVG identity; see [the U6 receipt](elk_hierarchy_preparation_2026-08-03.md). No latency or memory claim is admitted. |
| U7 Dugong transient retirement | `accepted-structural` | Stable batch retirement preserves node/edge/compound order and exact writeback; public controls matched SVG identity. One self-loop row was stable, while the long-edge base A/A lane was inconclusive, so no latency claim is admitted; see [the U7 receipt](dugong_transient_retirement_2026-08-02.md). |
| U8 rich inline HTML planning | `accepted-structural` | Cumulative five-slice planner closure removes repeated run/style scans and growing transient payloads while preserving Mermaid 11.16.1 wrapping and opaque callback semantics; see [the U8 receipt](rich_inline_html_planning_2026-08-07.md). No latency or peak-memory claim is admitted. |
| Interactive layout-work policy calibration | `accepted-structural` | The 800,000-unit ceiling follows the registered headroom rule for the closed 68-member corpus and has exact `W/W-1`, node/edge cardinality, configuration-amplification, isolated-stage, timeout, RSS, and output evidence; see [the policy receipt](interactive_layout_work_calibration_2026-08-07.md). No latency or memory claim is admitted. |
| U9 Sequence operation metrics | `accepted-latency`, `accepted-memory` | Reuse is private to the exact built-in carrier and semantic owner; host/custom callbacks remain unchanged. |
| U9 Requirement Markdown width | `rejected-as-written`, `rejected-upper-bound` | The old shared change reduced opaque callbacks; a compliant owner-local variant could save at most 13.249 us / 7.379%, below admission. |
| U9 Mindmap inline metrics | `rejected-superseded` | U8 already removed every admissible discarded built-in request; the remaining opaque callbacks are observable behavior. |

No failed U9 production path or global text cache remains. The old Requirement and Mindmap
experiment branches are historical hypotheses only.

### Mindmap ordinary-label rendering

Commit `b2129d9ec` resolved the confirmed ordinary-label regression without a family-local
classifier. Against pre-fix revision `71cb231c8`, three same-host runs measured:

| Metric | Pre-fix median | Fixed median | Change |
| --- | ---: | ---: | ---: |
| Mindmap end-to-end | 654,210 ns | 161,330 ns | -75.34% |
| Mindmap layout | 94,693 ns | 89,608 ns | -5.37% |
| Mindmap SVG | 542,590 ns | 62,383 ns | -88.50% |
| Kanban control | 63,881 ns | 56,272 ns | -7,609 ns |

The control delta remained below the registered 10,000 ns noise threshold. The retained
implementation defines an exact syntax-free ASCII projection in the Markdown interpreter and lets
the sanitizer skip DOM work only when its complete effective tag policy preserves the generated
paragraph. Unicode, entities, Markdown, HTML, icons, math, unknown security levels, and customized
sanitizer policies retain the full path. The faster punctuation-blacklist experiment was rejected
after `#quot;` exposed Mermaid placeholder leakage.

### Requirement operation-scoped label preparation

Commit `8d45b8634` carries Requirement label text, exact measurements, and stable edge identity from
layout into SVG emission. It does not introduce a global cache or syntax heuristic. Markdown
conversion and strict sanitization still happen at render time; only duplicate measurement and
label-plan reconstruction were removed. The public layout JSON remains unchanged.

Three same-host long runs alternated base/head order with 30 samples, two-second warm-up, and
three-second measurement windows:

| Stage | Before median | Fixed median | Change |
| --- | ---: | ---: | ---: |
| Parse | 5,527.6 ns | 5,612.6 ns | +1.54% |
| Layout | 132,050 ns | 131,560 ns | -0.37% |
| SVG emit | 137,430 ns | 54,165 ns | -60.59% |
| End-to-end | 274,840 ns | 198,980 ns | -27.60% |

The latest focused mmdr run at `75c9fd156` measured Merman/mmdr ratios of 0.72x parse, 2.81x
layout, 3.85x SVG emit, and 2.81x end-to-end. All 1,148 `merman-render` tests passed, with one
pre-existing skip; the
focused Requirement selection passed 21/21, and the Look SVG test passed 2/2.

This repair predates the complete evidence rule above: the A/B receipt did not record model size,
SVG byte/element counts, or a raster receipt. Its retained correctness evidence is the unchanged
public layout JSON plus the focused, Look, and full render test suites. Do not use the richer-output
hypothesis to explain the residual until those structural measurements are collected.

The latest full standard run measured 196.96 us versus mmdr's 71.08 us and retained the same
30 comparable rows, 18 Merman leads, and 12 mmdr leads. Its input gate excludes different Treemap
and XYChart sources from every ratio.

## Work queue

### P1.0: Profile residual Requirement layout and SVG construction

The repeated-label cause is closed. Profile Dugong layout separately from path/DOM string
construction on the same fixture. Do not move strict sanitization out of the render phase, and do
not treat mmdr's smaller SVG as semantic equivalence.

Exit: the remaining focused 2.81x layout and 3.85x SVG ratios are attributed to named operations
with checked-in absolute stage receipts, and any retained change preserves Requirement goldens,
sanitization, custom-measurer behavior, and public layout JSON.

Completed on 2026-07-29. Sampling attributed 96.6% of Requirement prepare time to text/label work
and Dugong, both excluded from the residual unit; the dispersed remainder has no qualifying
owner-local term. This 2026-07-29 residual item predates the 2026-08-02 U9 family metric-reuse unit
and closes without production changes; see
[the closure receipt](runtime_hypothesis_closures_2026-07-29.md).

### P1.1: Add fair reusable and strict cross-runner lanes

The current public-path comparison is intentionally asymmetric: Merman's one-shot helper creates a
render environment per call, while mmdr reuses its theme/config; Merman uses strict render-model
parsing, while the mmdr benchmark uses its permissive parser. Keep the public-path lane, then add:

- a reusable environment/config lane for both implementations; and
- a strict parsing lane where both products perform their documented validation.

Exit: reports name the public, reusable, and strict lanes separately and never merge their ratios.

### P1.2: Compare Mindmap with the same layout algorithm

Run a tidy-tree lane only if both implementations expose equivalent configuration. Do not replace
Merman's default COSE-Bilkent layout with mmdr's radial fallback to improve a benchmark.

Exit: the report separates algorithm cost from parser/render cost and retains default-product
geometry.

### P1.3: Prepare Kanban labels once per operation

Kanban layout converts, sanitizes, and measures section/item Markdown, while SVG rendering repeats
the conversion and sanitization. First add a stage benchmark and prove the share of the current
29.47 us cross-runner gap. If material, carry a private operation-scoped label plan as Requirement
does; do not add a global cache or syntax classifier.

Exit: Kanban semantic/layout/SVG goldens and hostile Markdown/HTML behavior remain identical, and a
decision-grade adjacent A/B confirmation shows the exact stage and absolute saving.

Completed on 2026-07-29. The accepted private artifact reuses prepared section/card-title XHTML and
geometry while leaving detail-only measurements in SVG emission. The public medium lane improved
from 51.65 us to 40.71 us (-21.19%, -10.94 us); see
[the U5 receipt](kanban_prepared_labels_candidate_2026-07-29.md).

### P1.4: Avoid editor-only bookkeeping in render-only parsing

Several typed render parsers reuse semantic constructors that also populate editor facts and lexeme
journals, then discard them. Prefer a shared constructor with a semantic-only or no-op facts sink;
do not fork a second parser.

Exit: semantic models, errors, recovery, spans, and editor output remain identical, while
`parse`, `compatibility_json_parse`, and end-to-end measurements show where the fixed cost moved.

Completed on 2026-07-29. Mindmap and Requirement were rejected by whole-parser upper bounds. A
temporary Kanban no-fact candidate saved about 1.0 us (2.49%) on the accepted 40.71 us public path,
below its 4.07 us and 10% low-latency thresholds, and was removed. See
[the closure receipt](runtime_hypothesis_closures_2026-07-29.md).

### P1.5: Measure reporting overhead on the string-only SVG path

Each render session initializes and updates measurement-provenance counters, and
the string-only API constructs a completed report before discarding it. A/B a no-report terminal
path while preserving the report-returning API unchanged. This is a hypothesis, not a proven
cause; reject it if the absolute saving is below the active threshold.

Exit: report APIs retain identical evidence, string APIs retain identical SVG/error behavior, and
the stage benchmark demonstrates any saving.

Completed on 2026-07-29. A minimal raw-string terminal candidate improved the smallest public Info
path by 0.391 us (9.76%), below the low-latency gate's 1 us and 10% minima. It was removed without
expanding the API surface or tests; see
[the closure receipt](runtime_hypothesis_closures_2026-07-29.md).

### P1.6: Optimize large Flowchart scaling

Build a size/density curve rather than tuning against one 420-line fixture. Attribute ordering,
crossing minimization, routing, text measurement, and SVG emission separately. Preserve
source-backed layout semantics and reject magic-number changes made only to improve the benchmark.

The clean `06616dd71` native allocator baseline completed 30 matched operation/zero pairs on an
Apple M4 Pro with Rust 1.95.0. Every repeat was deterministic, so the one-sided bootstrap bounds
collapsed to the estimates:

| Metric | Log-log slope upper bound | `100x` upper bound | Infrastructure cap | Result |
| --- | ---: | ---: | ---: | --- |
| Allocation count | 1.411568 | 4,265,827 | 2.0 / 10,000,000 | Pass |
| Allocated bytes | 2.338849 | 33,701,769,467 B | 2.0 / 8 GiB | Failed bound |
| Peak growth | 1.434359 | 144,199,480 B | 2.0 / 2 GiB | Pass |

This makes cumulative allocation the first qualified scaling target. It does not prove which
Flowchart stage owns the bytes; U4 still requires owner-local attribution and an adjacent public
latency comparison before retaining production code.

Exit: the curve identifies the first superlinear or allocation-heavy stage and a representative
large preview fits the agreed interactive budget.

Completed on 2026-07-29 with a split decision. The indexed Flowchart adapter candidate was rejected
and removed after it failed to produce an admissible public-operation result. The separate Dugong
batch-retirement mechanism remains because it bounds repeated adjacency reconstruction from
worst-case `O(T * (V + E))` to `O(T + V + E)`; its preregistered latency and memory admission was
not completed, so no measured speedup is claimed. Candidate-only memory lanes, contracts,
generators, and tests were deleted. See
[the adapter decision](flowchart_u4_adapter_candidate_2026-07-29.md) and
[the batch preregistration outcome](flowchart_u4_dugong_batch_preregistration_2026-07-28.md).

### P1.7: Bound Resvg finalization and export work

Remove input-amplifiable work from terminal reference and attribute validation independently of
ordinary-fixture latency. Measure one-shot raster export and ResvgSafe finalization on complete
public operations before changing ownership or error-order contracts.

Completed on 2026-07-29. Duplicate parsed-ID reference edges changed from `O(D * R)` to
`O(D + R)`, and duplicate expanded-attribute membership changed from `O(A^2)` to expected `O(A)`.
The one-worker PNG candidate saved only 0.78%; the single-reader XML candidate regressed by 0.57%.
Both latency candidates and their one-fixture benchmark lanes were removed. See [the U8
receipt](resvg_pipeline_candidates_2026-07-29.md).

### P1.8: Linearize rich inline HTML planning

Keep inline HTML segmentation source-backed and owner-local. A candidate may remove repeated
fragment/style discovery and scratch-payload copies, but opaque host measurer calls, request text,
order, and failure behavior remain observable and must not be reduced without a separate contract.
Use fixed-byte `R`, `K`, and orthogonal `R x K` structural controls; do not turn a structural
result into a latency claim without an adjacent public A/B.

Completed on 2026-08-07 with an accepted structural result. One indexed logical source and one
opaque style-group side index reduce Rust planning to `O(B + R + K)` plus explicitly observed
backend work, remove the growing `B * K` scratch-copy term, and preserve host/built-in semantics.
The side index adds `O(R + K)` logical `usize` slots; no peak-memory or latency improvement is
claimed. See [the decision receipt](rich_inline_html_planning_2026-08-07.md).

## Guardrails

- Correctness and Mermaid parity take precedence over a timing ratio.
- Keep native, Node N-API, Node-WASM, and browser-WASM measurements in separate lanes.
- Do not infer a shared cause from unrelated family ratios.
- Do not add family allow-lists, bypass deterministic runtime policy, or remove sanitization.
- Keep exploratory Markdown/JSON under `target/bench`; check in only dated, decision-relevant
  evidence.
