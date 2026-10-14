# Native theme repair: implementation and verification, 2026-10-14

Implementation and statistics are reviewed at frozen candidate `ac4a40437a36e301f6ccf0f0f689230c658d567b` (tree `56c083860416371d5efa17b4e6049ded21c68f09`). Completed measurements confirm corrected-control non-regression within the registered thresholds; they do not admit an overall performance win. The original Binaryen 133 gate failure remains explicit.

The maintainer subsequently selected main's Mermaid 12.2 baseline. Its separate
[main integration report](native_theme_main_integration_2026-10-14.md) records that merge and its
validation. Measurements and platform results below remain evidence for the frozen 12.1 candidate.

## Result

N01–N36 have implemented, independently reviewed fixes or test/tool corrections. Structural tests establish removed copies, discarded evidence and repeated XML work while preserving ownership, terminal evidence, resource accounting, cancellation and error order. They do not establish a latency or allocation improvement.

Main-versus-final confirmation reports 13 comparable cases, 10 confirmed regressions, three inconclusive cases, zero confirmed improvements and zero contract errors. Corrected-versus-final confirms non-regression for all 14 frozen cases, with zero regressions, zero admitted improvements and zero inconclusive regression outcomes. Required validation and ownership contracts are present in the final operation; the remaining main gap has **not** received a final exclusive cost attribution. It cannot all be assigned to required validation.

`OUT` below means `target/bench/experiments/native-theme-repair-20261014/`. Raw historical captures and review receipts remain unchanged. The implementation inventory is `OUT/final-evidence-index-before-timing.json`; its timing-pending fields describe that earlier checkpoint, not the completed main result.

## Implementation and review provenance

| Commit | Scope |
| --- | --- |
| `87cf582bffe47fa425e3b45e75aea1f7ae7e92a5` | U1 theme evaluation and appearance ownership |
| `b994d79d72bf8d273d9b0a6ffdd6b399d04d299c` | N28 pre-freeze compatibility ownership |
| `369c4bd3b025f5de4a9c5de506b33efa26128e86` | U2–U4 XML/CSS, family geometry and retired metadata |
| `d5ae011052a1b9bed75b706b96455991a294f84e` | N32 public publication checkpoint; corrected control |
| `260be8cc0037765168724e743a6856cbfdb815ea` | U5 local theme transactions and COW behavior |
| `2ab878e70cc51d2c31255ff50096e9b96fa3f13b` | U6–U8 compact facts, shared XML/reference analysis; N31/N33–N35 |
| `984b37e9628acf5012ead1b38cdbbf8760f08712` | Strict lint and feature-boundary adaptation |
| `512589484bec1ee797e56942ac8353f5f395380e` | N36 private feature-matrix declaration filtering |
| `ac4a40437a36e301f6ccf0f0f689230c658d567b` | Verified documentation checkpoint; no source change after 512589484 |

The full review starts at `f47abded02b540d7fe563af7bc75f2fa1bb90c51`, with eight Codex personas, merge/validation/report stages and zero retained source defects. Independent local Codex adversarial review follows the maintainer restriction against external coding CLIs; it is not cross-model evidence.

Receipts remain in `/tmp/compound-engineering-501/ce-code-review/20261014-021516-6cf8ffac/`: immutable `review.json`, `N33-delta-review.json`, `late-lint-delta-review.json`, and `N34`/`N35`/`N36-delta-review.json`. The original pending-U9 verdict is preserved. `OUT/final-review-readiness-before-u9.json` reconciles 152 source/test paths with reviewed bytes; inventory SHA-256 is `39160de73acca87ce99a1c14eac190253f223e17a24158c11d599f9379b07ad0`.

## Findings and structural evidence

Each row maps to exact source hashes, regression tests, logs and review receipts in the evidence index. Counts below describe the exercised owner or protocol, not a measured whole-operation allocation total.

| Finding | Unit | Implemented behavior / proof |
| --- | --- | --- |
| N01 | U1 | Evaluate root-directive truthiness and actual selected-theme transitions independently of appearance winners. |
| N02 | U2 | Write logical CSS through one XML text boundary; 152 typed-font/pipeline subcases and 30 host/source cases. |
| N03 | U3 | Restore source-backed Neo non-palette Swimlane border precedence. |
| N04 | U3 | Share authored Block stroke width, including zero, between terminal CSS and viewport bounds. |
| N05 | U3 | Share Neo square padding 16/12 between intrinsic and positioned geometry. |
| N06 | U3 | Preserve Mermaid radius truthiness: missing/null/numeric zero use 5; string zero uses 0. |
| N07 | U4 | Remove unconsumed Ishikawa text bbox attributes and their formatting; retain layout geometry. |
| N08 | U4 | Remove retired TreeView/cluster raster-proof IDs and their exact comparator exemptions. |
| N09 | U5 | Commit local theme variables transactionally; owned/failure paths make zero JSON COW copies. |
| N10 | U5 | Avoid same-value JSON detachment after preserving authority and stamp bookkeeping. |
| N11 | U5 | Skip derived-owner queries only after proving no authority: zero candidates versus the 292-query red witness. |
| N12 | U5 | Exercise actual legacy Default/Extended failure paths, separately from staged Base. |
| N13 | U6 | Use canonical Class slots, seen bits and sticky failures; retain rich facts only when requested. |
| N14 | U6 | Collect Class facts for the union of Text/NodeLabel, palette, effects and typography consumers. |
| N15 | U6 | Prepare Class metrics without discarded XHTML facts; preserve external construction restrictions. |
| N16 | U6 | Keep queried visible-style counters without copied run content; traverse DOM with an iterative stack. |
| N17 | U6 | Gate Kanban facts on TaskLabel/Text and typography demand; preserve measurement and invalid-empty behavior. |
| N18 | U6 | Borrow Mindmap baseline CSS tokens; defer evidence-only identities without dropping source paint/checkpoints. |
| N19 | U8 | Keep an empty root prelude at zero entry/scope capacity while retaining residual obligations. |
| N20 | U7 | Share reference analysis; summary mode retains zero postorder/occurrence slots and occurrence edges. |
| N21 | U7 | Use fully validated no-expansion facts to omit graph work; retain complete XML/error/budget checks. |
| N22 | U8 | Transfer private exact-generation XML facts through admitted no-op stages: XML parses 3 to 1; mutations invalidate. |
| N23 | U8 | Consume reference/resource/font inventories into final reports with identical allocation pointers and contents. |
| N24 | U8 | Move family evidence vectors into completion; borrow facts for temporary strict classification. |
| N25 | U3 | Restore pinned rectangular routing boundaries independently of rounded visual arcs. |
| N26 | U2 | Observe sink refusal/cancellation before scanning and at UTF-8-safe chunks: 2048-byte threshold, maximum 2051. |
| N27 | U2A | Apply XML 1.0 scalar legality to literal text/CDATA/attributes/comments as well as numeric references. |
| N28 | U5A | Preserve surviving pre-freeze compatibility ownership and derived conceptual inputs through the public Engine. |
| N29 | U3A | Gate ELK goldens on layout-elk; validate actual Dagre fallback separately; no production geometry patch. |
| N30 | U2B | Accept legal quoted font metacharacters/escaped commas; retain unsafe CSS and incomplete-grammar rejection. |
| N31 | U6A | Use package-contained roxmltree 0.21.1 with the exact official iterative patch; preserve public codec preflight. |
| N32 | U8A | Check the public operation after artifact identity sealing and before publishing SVG/document outputs. |
| N33 | U2C | Decode emitted Railroad XML text once before strict CSS receipts; commit proof after the final checkpoint. |
| N34 | U9A | Move label background ownership to family; Block-only builds no longer require Flowchart capability. |
| N35 | U9B | Move shared RoughJS paths to roughjs_common; preserve the moved implementation, RNG/work/error ordering and gates. |
| N36 | U9C | Intersect private singleton checks with declared features after unchanged production completeness/forwarding guards. |

N31 retains the exact official 24-addition/12-deletion element-parser patch and package/license closure. Strict and loose accepted 4096-depth labels complete on a 2 MiB thread; upstream 200000-depth and namespace/entity/range/error controls pass. The parser is private; no new public parser API is exposed.

N33 preserves strict root/descendant selectors, counts, important markers, typed/source precedence and duplicate/failed-seal rejection. The actual XML writer seam preserves a literal entity once; illegal typed/host fixtures were removed without broadening font normalization. Its final focused suite passes 46/46.

## Final correctness and platform gates

| Gate | Observed result | Evidence in OUT |
| --- | --- | --- |
| Rust quality | fmt, workspace check, all-targets/all-features Clippy with `-D warnings` pass | `strict-installed-verification.log` |
| Workspace tests | 12,827/12,827 nextest pass; 11 existing skips; doctest and architecture compile-fail tests pass | `strict-installed-verification.log` |
| Feature/artifact matrix | 660/660 pass, including singleton renderer boundaries and artifact profiles | `strict-post-n36-verification.log` |
| Full SVG | structure, parity and parity-root pass; accepted browser/parser residuals remain explicit | `release-full-svg-comparison.log` |
| Root/private acceptance | 7/7 root contracts; 3073/3073 private theme cases, two existing skips | `root-contract-nextest.log`, `private-theme-acceptance.log` |
| Viewport | 3712 fixtures, zero blocking containment failures; diagnostic residuals retained | `root-viewport-diagnostic.json` |
| Generated/legal closure | Reference/generated checks, 13 license reports and 424 synchronized notice projections; legal contracts pass | `final-legal-sync.log`, `final-package-legal-unit.log`, strict log |
| Web with upstream-fixed tool | Build/test/smoke all exit 0; 149 tests, five package smoke artifacts and DOM safety pass | `web-upstream-fixed-optimizer-verification.json` |
| Playground | Eight strict scripts exit 0; 79 non-Chromium and 170 desktop Chromium browser checks pass | `strict-playground-tail.json` |
| VS Code | 255 tests pass | `vscode-final-test.log` |
| C ABI public consumers | Exact CI `c-abi-native` no-default-features header/consumer smoke passes 3/3; ABI unchanged | `exact-c-abi-consumer-smoke.log`, `exact-c-abi-consumer-smoke.json` |

The viewport receipt retains 2399 local containment diagnostics, including 2344 upstream-inherited and one exact residual, plus 2286 indeterminate entries and 54 browser-owned paint diagnostics. Zero blocking failures is not a claim of perfect geometry or browser parity.

| Optimizer gate | Original Binaryen 133 | Temporary Binaryen 133 plus official upstream fix |
| --- | --- | --- |
| Exact Web optimizer input | Official release exits -6 at `constraint.cpp:1223`; Homebrew also aborts | Web build/test/smoke pass with assertions enabled |
| Same raw diagnostic input | Exit -6 | Exit 0; exact input SHA-256 unchanged |
| Overall disposition | Original pinned-tool strict aggregate remains failed | Replacement-tool validation passes; no original-tool green claim |

Exact Web input SHA-256: `b3d43ed41f5b77b006cd4c95e89b912c5c12f6b88a7678fbd223175abb8483da`. Raw A/B input: `79c792a88ce679f1e47b5e734504aa4cc10fe4baf04340a8dc2c629ef288cc50`. Original/fixed receipts are `official-133-exact-web-reproducer.json`, `official-133-raw-diagnostic.json` and `corrected-133-raw-diagnostic.json`.

The temporary optimizer applies official upstream fix `ca1cf60a6b41fbb20c4d0c1bfb98523b9cabe8ee`; executable SHA-256 is `34d4c41bdb500947b3a93d155c59d8538ece794d6e0c2f9b6461641d9b67ce56`. Source preparation/configuration/build receipts retain provenance and assertions. Repository production source, optimizer options and workflow requirements were not weakened.

## Discovery, cohort and statistical contract

Both full discoveries enumerate 38 fixtures and complete runner post-verification without global contract errors. Corrected control (`d5ae011052a1b9bed75b706b96455991a294f84e`) matches all 38 outputs and exits 0 (`diagnostic_advisory`). Main (`3a4b8c393939b69372a8f05a2b971f7cdf6ae529`) matches 19 and retains 19 output-different coverage rows, so broad main discovery exits 2 (`contract_failure`). Coverage mismatches are not silently admitted for latency.

The frozen 14-case cohort in `OUT/confirmation-cohort.json` is class, info, pie, mindmap, journey, timeline, gantt, requirement, c4, kanban, xychart, wardley, cynefin and usecase. Its main same-output intersection contains 13 cases; corrected control contains all 14. The other six main-comparable discovery cases remain diagnostic-only, not confirmed non-regressions.

Usecase differs solely in serialization: U2 replaces 222 `&quot;` entities inside the single stylesheet with literal quotes (2 font-family, 124 data-look, 96 data-color-id quotes). Every other byte is unchanged: 19,178 to 18,068 bytes, with 75 elements on both sides and identical decoded DOM, CSS, model and layout. The exact derived candidate SHA-256 `8f88d7afc944c4723c8598c7ac1eb7f2d7eda8641d41a9ad70be436aaed934c0` matches discovery; main SHA-256 is `0615d1a39fba83c85ff593ad11421f14abcf5331e1388ae8d71c930fdc5bde63`. `OUT/usecase-final-output-disposition.json` records the complete byte proof. It remains coverage-only against main; no normalization changes the frozen contract.

Timings cover warm native Rust source-to-SVG with reused Engine/Renderer, strict parsing, deterministic SVG settings and trusted-native resources. Both recipes use locked `merman`/`pipeline`, default features plus `svg`, Rust 1.95.0, on Apple M4 Pro/macOS arm64. This does not measure raster painting, CLI startup, browser WASM or host font discovery.

Criterion uses 30 samples, two-second warmup and three-second measurement. Each confirmation has eight balanced A/A pairs on each binary and eight fresh balanced AB/BA pairs per case; all 26 main and 28 corrected calibration rows are stable, and discovery/calibration samples are not reused for confirmation. Runner files, executable, fixture and output identities verify after sampling.

Bounds use 10,000 deterministic paired-bootstrap resamples and Bonferroni simultaneous one-sided 95% bounds across 13 main cases and two metrics (26 components), and separately 14 corrected cases and two metrics (28 components). The two control comparisons have separate confidence families; this is not joint 95% coverage across both controls. Pair-count planning does not establish family-wide 80% power. A confirmed regression requires both relative and absolute lower bounds to exceed 10% and 50 us. Inconclusive does not mean unchanged or equivalent.

Reported side timings are arithmetic means of independent Criterion console point estimates, not per-render medians or percentiles. Increase is the ratio of those means; relative confidence bounds use paired log ratios.

## Main versus final confirmation

`OUT/main-final-end-to-end-confirmation.json`: exit 1, `confirmed_regression`; zero contract failures. Command completion is `2026-10-14T08:35:15.378799+00:00`. The helper `generated_at` field records run start, not completion. All rows have eight fresh pairs and matched output identity. Report SHA-256: `2cd862170099078edc25d78cb641626f737f0e70d91ba92994ce1f4a594716ce`.

| Fixture | Main (us) | Final (us) | Increase | Delta (us) | Relative lower | Delta lower (us) | Outcome |
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

Final executable SHA-256: `dcbb2b928b55120476a411791e4b4aca5fc1f11b495fcdf97d2a133024f1cf35`; main executable: `c4f8cc5c7dbcea29e59c582ff5dff8cbb897f2f945723255c16baf0658f17bdd`. Both measured checkouts are clean. Identical benchmark source SHA-256 is `289d59b00ed8eb29615d6cdc4871dffef48b242471e2d60bae32994aa8442c16`; corpus SHA-256 is `9258b9f72c14893a5cb64b2b05a325dc32ac144b827f44717a3030d4a537e076`.

## Corrected control versus final confirmation

`OUT/corrected-final-end-to-end-confirmation.json`: exit 0, `confirmed_non_regression`; 14/14 cases below the registered regression gate, zero contract failures, regressions or admitted improvements. Command completion is `2026-10-14T09:44:26.858066+00:00`. Report SHA-256: `121536d325121dc43165ac256d9eb335e563186cd3ecd7b2c23179daa6005551`.

| Fixture | Corrected (us) | Final (us) | Change | Delta (us) | Relative upper | Delta upper (us) | Outcome |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | --- |
| class_medium | 1259.700 | 1206.963 | -4.19% | -52.737 | -3.78% | -47.657 | confirmed_non_regression |
| info_medium | 23.045 | 22.117 | -4.03% | -0.928 | -3.49% | -0.803 | confirmed_non_regression |
| pie_medium | 61.621 | 60.205 | -2.30% | -1.417 | -1.34% | -0.827 | confirmed_non_regression |
| mindmap_medium | 443.358 | 436.928 | -1.45% | -6.430 | -0.85% | -3.790 | confirmed_non_regression |
| journey_medium | 108.948 | 108.719 | -0.21% | -0.229 | +1.01% | +1.173 | confirmed_non_regression |
| timeline_medium | 111.748 | 109.534 | -1.98% | -2.214 | -1.48% | -1.652 | confirmed_non_regression |
| gantt_medium | 98.953 | 95.849 | -3.14% | -3.103 | -2.58% | -2.558 | confirmed_non_regression |
| requirement_medium | 426.330 | 422.641 | -0.87% | -3.689 | -0.17% | -0.759 | confirmed_non_regression |
| c4_medium | 263.048 | 262.916 | -0.05% | -0.131 | +1.15% | +2.816 | confirmed_non_regression |
| kanban_medium | 163.421 | 152.474 | -6.70% | -10.947 | -6.15% | -10.165 | confirmed_non_regression |
| xychart_medium | 186.554 | 184.070 | -1.33% | -2.484 | -0.20% | -0.345 | confirmed_non_regression |
| wardley_medium | 105.600 | 105.916 | +0.30% | +0.316 | +0.77% | +0.830 | confirmed_non_regression |
| cynefin_medium | 79.488 | 78.634 | -1.07% | -0.854 | -0.90% | -0.712 | confirmed_non_regression |
| usecase_basic | 362.069 | 361.277 | -0.22% | -0.791 | +0.68% | +2.493 | confirmed_non_regression |

Class has a -4.19% arithmetic-mean change and -52.7375 us delta, but `improvement_outcome` is **inconclusive**. Its improvement lower bounds are about 3.93% and 47.6571 us, which fail the joint 10%/50 us admission gate. It is not an admitted speedup. Non-regression under materiality thresholds is not exact equivalence; the 24 corrected diagnostic-only cases are not confirmed non-regressions.

Corrected executable SHA-256: `057fd48f5613f744e26202fbe779990f1129104c855536331d4f6f92d9227793`. Its final executable is identical to the candidate used in the main comparison. Side means belong to their own paired experiment and must not be combined across control runs.

`OUT/final-native-statistics-review.json` independently reconciles the cohort rule, referenced diagnostics/disposition hashes, exact identities, raw-pair/round copies and all 1296 measurement log entries. All 135 bootstrap bound sets and 4266 recursive numeric checks match. Helper SHA-256: `fad3f112d08071a5490076730e6ebe1a6d510f7a5aa9e2ac54ab0200303245b1`; experiment SHA-256: `6d6d38e88f66f82f14af5d36cac4684a8c1b1da747cec1dc13068d216b8cf98b`; cohort SHA-256: `80ad44c048be726b4741428c521608a94cbcd168c6692a29482bbdac9ce74aa6`.

Every measurement retains matched preflight identity and successful output postflight verification. Ordinary independent-target Cargo artifacts have no per-round executable digest fields; actual evidence is preparation identity, post-sampling file/binary/Git verification and independent final digest readback. No per-round executable SHA check is claimed.

## Claim limits and closeout disposition

`OUT/native-memory-smoke.json` is a compiled explicit-binary protocol smoke, with independent build receipt and executable SHA-256 `37cf2e66aa7f0d8e608373181d71277d5d5cff53a4dc9f7fd4b857aca050859c`. One repeat at 1x and 100x omits four registered scales and the required repeats; candidate admission is false and analysis metrics are null, while raw responses retain actual allocation counts. It proves neither allocation slope, peak reduction nor before/after allocation improvement.

Both confirmations and the independent statistics review are complete. The [closure ledger](../workstreams/native-theme-regression-20261014/WORKSTREAM.json) closes all [36 inventory findings](../workstreams/native-theme-regression-20261014/issues.json) as correctness or structural repairs; the remaining main cost stays in [PERF_PLAN.md](PERF_PLAN.md). This closeout is published with [PR178](https://github.com/Latias94/merman/pull/178), preserving both real outcomes and the external original-optimizer failure separately. Source-level work reductions, correctness gates and successful tool-substitution checks do not justify an overall latency/allocation win, a blanket non-regression statement, or exclusive attribution of the remaining main cost.
