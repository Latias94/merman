# Performance Plan

This page owns durable priorities and unresolved performance findings. Use the
[runbook](RUNBOOK.md) to execute experiments and [benchmarking](BENCHMARKING.md) for measurement
contracts. Dated results apply only to their recorded revisions, capabilities, inputs and hosts;
none of the historical values below measures current `main`.

## Triage and admission

Preregister the public operation, lifecycle, capabilities, controls, metric, noise floor and
materiality budget before confirmation. The comparator's fallback is jointly **>10% and >50 us**;
it is a reporting default, not a universal engineering cutoff.

| Evidence class | Required basis for the claim |
| --- | --- |
| Latency or throughput | Same-input, same-capability public operations; independent base/head A/A calibration and fresh, fixed, balanced AB/BA confirmation. |
| Low-latency operation | Below 500 us, preregister the [noise-derived gate](BENCHMARKING.md#low-latency-public-operations), or a justified frame, tail-latency or call-volume budget. |
| Memory | Measure allocation traffic, operation peak growth and retained state separately; include cold construction and cache retention when work moves out of the warm path. |
| Complexity or resource amplification | Name user-controlled variables, reachable old/new time and added-space bounds, and a proof, exact counter or scale curve; preserve resource accounting and public non-regression controls. |
| Artifact size or dependency closure | Freeze the actual package/profile, capabilities, tools and compression; measure the delivered artifact or resolved closure. |

Timing confirmation starts with at least eight balanced A/A pairs per runner and a power-sized,
fixed even AB/BA schedule of at least eight pairs. Follow the selected protocol's qualification
rules; diagnostic runs, profiler shares and private-stage ratios cannot admit a public speedup.
Structural or memory repairs can stand on their own evidence without closing a latency finding.
Caches, persistent state and broad machinery need evidence proportional to their maintenance cost.

## Unresolved historical observations

These findings remain open for revalidation under their original comparison scope. A newer local
optimization, changed runtime contract or different release baseline does not close them.

- **v0.9.0 release distance:** the [October 20 confirmation](theme_v090_appearance_css_macos_2026-10-20.md)
  at `b9a58731c` records 14 within budget, two regressions and three inconclusive scenes under the
  joint 10%/1 us release protocol. ZenUML is +17.51%/+3.499 us; Packet +28.83%/+2.332 us.
  Kanban is +18.95%/+8.521 us but fails absolute A/A qualification; Sankey and Wardley cross the
  relative boundary. The 19 same-SVG scenes exclude 57 changed-output scenes. New typed-preset
  coverage does not establish historical equivalence. The [October 21 diagnostic](theme_v090_current_fontstack_diagnostic_macos_2026-10-21.md)
  and later allocation work do not supply a new calibrated release classification.
- **Alpha.6 to alpha.7 default SVG:** the September 16 observation at `a5e3cd2d6` confirms
  Class tiny +270–274%, Class medium +125–127% and XY Chart medium +162–166%, with identical SVG.
  The September 21 post-SHA renewal at `aa35110ee` still measures Class tiny +209.94%
  (50.65 to 156.99 us) and medium +102.32% (600.34 to 1,214.61 us), clearing 10%/50 us.
  These are release-range findings, including dependency changes; they do not isolate themes.
  No closing matched release receipt is recorded here for Class or XY Chart. The
  [September 21 contract](#class-renewal--2026-09-21) retains the two Class results for revalidation.
- **Native theme-branch comparison:** the October 14 repair at `ac4a40437` still records ten
  material regressions and three inconclusive outcomes in its thirteen-row same-output intersection
  against original main; Class is about +85.7%/+554.3 us. Non-regression against corrected control
  `d5ae01105` does not close that baseline. Later lifecycle changes and the separate v0.9.0 comparison
  require their own comparable evidence rather than retroactively closing this record. The
  [October 14 contract](#native-theme-comparison--2026-10-14) retains the original main comparison.
- **Earlier family signals:** the [alpha.3 checkpoint](../release/ALPHA3_TO_ALPHA5_REFACTORING_REPORT.md)
  leaves Class and Kanban release comparisons needing a closing rerun. Requirement label reuse and
  the ordinary-label Mindmap repair have scoped wins; residual cross-renderer ratios need matched
  validation, output and layout algorithms before they justify more implementation work.
- **v0.7.0 library findings:** the [expanded observation](library_rendering_expanded_2026-10-05.md)
  retains Mindmap +11.1%/+38.83 us, private SVG/drop-boundary signals, State projection and historical
  State/Requirement/Gitgraph identity blockers, and smaller/noisy signals. Four Flowchart slowdowns
  do not reproduce with [matched Dagre](library_dagre_v070_to_01cc4562f_2026-10-05.md), whose A/A
  calibration fails; parallel-merges and backedges/subgraphs still lack matched-backend confirmation.
- **Large Flowchart allocation:** the [six-scale baseline](baselines/runtime-06616dd71.json) at
  `06616dd71` records cumulative-byte slope 2.338849 and 33,701,769,467 B at `100x`, exceeding its
  2.0/8 GiB infrastructure bounds. This identifies a scaling target, not its owning stage or a
  current measurement. Retained structural repairs have no closing allocation-curve receipt here.

## Maintenance priorities

1. Renew the unresolved public SVG comparisons, then attribute parse, layout, emission and E2E
   separately. Revalidate the generated CSS/XML and bounded append leads from the
   [October 21 investigation](theme_regression_root_cause_macos_2026-10-21.md) against current owners.
   ZenUML already has an [exact default CSS fast path](../../crates/merman-render/src/svg/parity/zenuml.rs);
   its diagnostics and other allocation repairs do not close the release comparison.
   Contract-bypassing interventions are not production candidates or recoveries.
2. Resolve identity/capability blockers before timing; use size/density curves for Flowchart and
   separate native PDF filter/export allocation, cold startup and explicit system-font costs.
   [CLI font policy](../../crates/merman-cli/README.md#explicit-fonts) distinguishes deterministic
   layout from native measurement and codec painting. Keep native, N-API, Node-WASM and browser lanes separate.
3. Preserve semantic output, sanitization, source ownership, callback order, errors, cancellation
   and resource limits. Record model size, SVG bytes/elements and relevant DOM/raster controls.
   Keep in-flight ledgers under `target/bench`; retain only concise decision-relevant evidence.

## Playground frontend

Keep browser UI workloads separate from native renderer latency. Local source edits stay in Monaco
and publish to the workspace without a controlled-value echo; external replacements reconcile
through the mounted editor binding. Preview subscribes to visible/current batches and input
freshness, preserving immediate stale-action invalidation without observing each debounce request
identity. Preserve these ownership boundaries when extending editor features.

Remaining attribution leads, not admitted speedups:

- **First syntax highlighting:** `createMermaidSyntaxEngine()` waits for Tree-sitter runtime
  initialization before starting the language WASM download. Investigate overlapping download and
  compilation with initialization while retaining streaming fallback and explicit worker failures.
- **Large-diagram fitting:** the infinite-canvas fit walks the mounted SVG for connected visual
  bounds on each fit/automatic resize. Attribute geometry reads and layout separately from rendering.
  A reuse proposal must cover responsive sizing, fonts, DOM changes, presentation mode and mount
  lifetime; an artifact-identity-only cache does not establish equivalent geometry.

## Current artifact budget owners

[WASM_SIZE_BUDGETS.json](../release/WASM_SIZE_BUDGETS.json) owns Typst's raw, stripped, gzip and
Brotli ceilings. [Web size measurement](../../crates/xtask/src/cmd/wasm_size_matrix.rs) reports
final assembled npm WASM without byte ceilings; `build-typst-package --profile publish --budget-file ...` owns the
[Typst package](../../crates/xtask/src/cmd/typst_package.rs) and its verified artifact. Retain functional
smoke and installed-consumer checks alongside size reporting. The [semantic baseline](semantic_wasm_budget_2026-10-25.md)
and [binding refresh](binding_wasm_budget_2026-11-01.md) retain historical size decisions; restoring math to
Web full/render changes capabilities and is not an equal-capability size optimization.

## Retained revalidation contracts

These are condensed historical receipts from `alpha6_current_class_2026-09-21.md` and
`native_theme_repair_2026-10-14.md`, available in Git at `11d86d16dfff36d7f2788e63b12485c1d6ef04d2`.
They preserve open comparisons, not implementation or review journals. Both measure the warm native
`merman` / `pipeline` public `end_to_end` SVG operation with reused Engine/Renderer and strict
parsing, not process startup, first render, raster painting, browser/Node-WASM or installed packages.
Each invocation uses 30 Criterion samples, 2 s warmup and 3 s measurement. Confirmation uses
eight independent A/A pairs per executable per case, then eight fresh,
balanced AB/BA pairs per case, with 10,000 paired-bootstrap resamples. Regression requires both
relative and absolute bounds to exceed 10% and 50 us. Neither comparison has contract failures;
exit 1 records regression. New runs require fresh calibration, not reuse of these observations.

### Current Class diagnostic — 2026-11-04

Frozen source `3adcb9206ce8934cee5a50da5fddb1ea70cd34fa`, its unmodified `pipeline` harness and
locked dependencies were measured on Apple M4 Pro / macOS 27.0 arm64, Rust 1.95.0, bench profile.
Defaults remain enabled with `all-diagrams,svg,compat-json,editor-facts,layout-cytoscape,layout-elk,math`.
Builds used one job, no incremental/debug information; sampling used two Rayon threads and no
concurrent build, browser test, or theme render. Three serial repetitions selected each exact
`{end_to_end,parse,layout,render}/class_{tiny,medium}` case, with 30 samples, 2 s warmup and 3 s
measurement per call. All 24 calls succeeded and retained identical per-case pre/postflight output.

Values below are the **median of three invocation means**, in microseconds; parentheses show
the observed minimum–maximum, not confidence bounds. Stages are independently scoped attribution
lanes, not an additive decomposition of the public operation.

| Fixture | Public rich SVG E2E | Reused-engine parse | Layout | Prepared SVG emission |
| --- | ---: | ---: | ---: | ---: |
| Class tiny | 142.28 (135.94–143.68) | 88.03 (86.37–88.07) | 17.20 (16.58–17.29) | 31.57 (31.14–31.80) |
| Class medium | 606.29 (596.77–615.42) | 151.04 (148.99–153.19) | 215.76 (212.12–215.86) | 233.84 (231.66–240.58) |

The unchanged 43/995-byte fixtures now produce 16,466/77,724-byte SVGs with 95/576 elements and
SHA-256 `d28d3c2056c65eaaf0bcc23dbd5cc7ac19b818ecfd81e15bc454da3a3a8a63f9` /
`5382f40ca4f7dd6a0813387d0133d6bd2d1f9222d09556b49ff57c61fad04be7` respectively. Both differ
from the retained alpha.6 and v0.9.0 full SVG identities: those release comparisons are
**non-comparable**, and the September regression remains open. No A/A, A/B, recovery, or
optimization admission is claimed. Investigate the tiny parse boundary and medium layout/emission
owners before selecting a production hypothesis. Preregistration, executable/source/input hashes,
commands, logs and raw Criterion samples remain under ignored
`target/bench/experiments/class-current-20261104/` (`diagnostic-metadata.json`, `criterion/`).

### Class renewal — 2026-09-21

Production base is alpha.6 `d529f858ea3d337a1bdc8fe12e44e1403ededf2e`; production head is
`aa35110eeb236a1b0906437cf1b2f28e9da915d2`. Actual measurement commits are base
`cdbc77075d82c894109f2f39eff77bc338385c14` and head
`e4db6ff9c6e9764132116ce10a6d011af1a24107`. Only the untimed preflight SHA formatter changes;
production crates, manifests and lockfiles retain their respective source. Both complete benchmark
files have SHA-256 `c37c5bdf2029305b6102885e5c204bdeb95c242e6a6141f59ff396352ea2889e`.
Use these measurement commits when reproducing the recorded benchmark, rather than substituting
the production commits' different preflight formatter.

Both sides use Rust 1.95.0, bench profile, no default features, and
`svg,layout-elk,layout-cytoscape` on Apple M4 Pro / macOS 26.6.2 arm64, with default headless SVG
settings; builds and measurements are serial. Dependencies remain part of this release comparison;
the delta is not attributed solely
to themes. Complete SVG bytes match for these two cases; changed-output Flowchart, Sequence and
State cases are excluded. All four case/executable A/A calibrations are stable and select eight
confirmation pairs. Bounds below have simultaneous 95% coverage over four relative/absolute
components, with Bonferroni-adjusted 98.75% component confidence. Both outcomes are
`confirmed_regression`; independent invocation pairs, not Criterion samples, are the inference unit.

| Fixture | alpha.6 us | Head us | Added us (95% bound) | Relative change (95% bound) |
| --- | ---: | ---: | ---: | ---: |
| Class tiny | 50.65 | 156.99 | 106.34 (105.77–106.95) | +209.94% (+208.27–211.56%) |
| Class medium | 600.34 | 1,214.61 | 614.28 (612.53–615.69) | +102.32% (+101.93–102.65%) |

| SVG identity | Bytes | Elements | SHA-256 |
| --- | ---: | ---: | --- |
| `class_tiny` | 14,421 | 95 | `297d969418ccf35d7fbf63f126e29c3d9a6712e6e265952fdf7c27bdb9012669` |
| `class_medium` | 74,203 | 576 | `4f6b471d69d3bd5119e6e198887d8e5173bb11f0823155cdfd461e4407a88865` |

Recorded raw path: `target/bench/experiments/alpha6-current-u10-20260920/confirmation.json`.
Its SHA-256 is `82e0b0a681d4db8fb5de5fd3e2736e30d9af4f76ad9d671d2d1388515910afe6`;
base executable SHA-256 is `5690698b499f1071e6af1e1eb397b0e67af92e6a726c1bb975792fcfef8947d8`,
head executable SHA-256 is `b49a02b83234e63296c2afa6deef570e720899e0528a5db7c0b8f24ee60fe208`.

### Native theme comparison — 2026-10-14

Original main is `3a4b8c393939b69372a8f05a2b971f7cdf6ae529`; frozen candidate is
`ac4a40437a36e301f6ccf0f0f689230c658d567b` (tree `56c083860416371d5efa17b4e6049ded21c68f09`).
This is the Mermaid 12.1 candidate, not the later 12.2 integration. Both locked bench recipes use
Rust 1.95.0, default features plus `svg`, deterministic SVG settings and `trusted_native` resources
on Apple M4 Pro / macOS arm64. Identical benchmark source SHA-256 is
`289d59b00ed8eb29615d6cdc4871dffef48b242471e2d60bae32994aa8442c16`; corpus SHA-256 is
`9258b9f72c14893a5cb64b2b05a325dc32ac144b827f44717a3030d4a537e076`.

The 38-case discovery has 19 identical SVGs and 19 changed outputs. A frozen 14-case cohort has
13 same-output main cases below; Usecase remains coverage-only because its serialization changed.
The other six main-comparable discovery cases are diagnostic-only. All 26 main case/executable
A/A rows are stable. Bounds are Bonferroni simultaneous one-sided 95% bounds over 13 cases and two
metrics; historical pair planning does not establish family-wide 80% power. Side timings are
arithmetic means of independent Criterion invocation point estimates;
percentage changes are ratios of those means, while relative bounds use paired log ratios.

| Fixture | Main us | Candidate us | Increase | Delta us | Relative lower | Delta lower us | Outcome |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | --- |
| class_medium | 646.626 | 1200.938 | +85.72% | +554.311 | +84.74% | +547.707 | confirmed_regression |
| info_medium | 7.484 | 22.051 | +194.64% | +14.567 | +192.55% | +14.434 | inconclusive |
| pie_medium | 25.185 | 60.241 | +139.20% | +35.057 | +137.67% | +34.628 | inconclusive |
| mindmap_medium | 250.310 | 439.039 | +75.40% | +188.729 | +74.91% | +186.877 | confirmed_regression |
| journey_medium | 27.195 | 109.104 | +301.20% | +81.909 | +292.34% | +80.128 | confirmed_regression |
| timeline_medium | 39.370 | 110.101 | +179.66% | +70.731 | +176.64% | +69.424 | confirmed_regression |
| gantt_medium | 36.232 | 96.318 | +165.84% | +60.086 | +164.48% | +59.164 | confirmed_regression |
| requirement_medium | 234.459 | 421.129 | +79.62% | +186.670 | +78.69% | +184.291 | confirmed_regression |
| c4_medium | 104.614 | 262.848 | +151.26% | +158.234 | +148.03% | +156.229 | confirmed_regression |
| kanban_medium | 44.235 | 152.655 | +245.10% | +108.420 | +242.42% | +106.994 | confirmed_regression |
| xychart_medium | 88.895 | 184.062 | +107.06% | +95.168 | +106.09% | +94.267 | confirmed_regression |
| wardley_medium | 39.611 | 105.767 | +167.02% | +66.157 | +164.79% | +65.467 | confirmed_regression |
| cynefin_medium | 28.724 | 78.176 | +172.16% | +49.452 | +169.80% | +48.878 | inconclusive |

Recorded raw directory: `target/bench/experiments/native-theme-repair-20261014/`.
`main-final-end-to-end-confirmation.json` SHA-256 is
`2cd862170099078edc25d78cb641626f737f0e70d91ba92994ce1f4a594716ce`;
main executable SHA-256 is `c4f8cc5c7dbcea29e59c582ff5dff8cbb897f2f945723255c16baf0658f17bdd`,
candidate executable SHA-256 is `dcbb2b928b55120476a411791e4b4aca5fc1f11b495fcdf97d2a133024f1cf35`.
The separate corrected-control comparison against `d5ae011052a1b9bed75b706b96455991a294f84e`
confirms 14/14 non-regressions under its own 28-component confidence family, without an admitted
speedup. Its `corrected-final-end-to-end-confirmation.json` SHA-256 is
`121536d325121dc43165ac256d9eb335e563186cd3ecd7b2c23179daa6005551`. Do not combine side means
or confidence families between controls; this result does not close the ten original-main
regressions, and inconclusive is not equivalence.

Raw availability checked on 2026-11-04: the three recorded JSON paths above and both experiment
directories are absent from the reviewed worktree and are ignored by Git. Their historical hashes
identify the original reports; they do not constitute a distributed raw dataset or prove that
copies remain available on another host. Revalidation requires new runs at the recorded scope.
