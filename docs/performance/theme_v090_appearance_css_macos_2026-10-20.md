# Public SVG release comparison after appearance/CSS repairs — October 20, 2026

## Admitted result

The fresh identity-repaired confirmation at production `b9a58731c` has **14 scenes within the
joint budget, two confirmed regressions and three inconclusive scenes**, with zero consumer
contract failures. The actual process exits 1 because confirmed regressions remain. Independent
terminal review admits this run as a formal release checkpoint; it does not complete the
all-family convergence goal or establish the isolated latency benefit of either repair.

The first attempt is explicitly rejected: Cargo reused the raw-capture head executable from the
shared target in a clean no-hook checkout. Its complete report, plan and review remain archived
with `rejected-` names; none of its samples or nominal 16/3 classifications enters this result.
The replacement was triggered by invalid executable provenance, with no favorable-result
selection. It uses entirely new A/A and A/B observations and the original production revisions,
19-scene order, thresholds and selection rules.

## Revisions and timed operation

Production baseline: v0.9.0, `f754a3c3077d35d2a372c2fec04f9e9b87db1c3f`.
Production candidate: `b9a58731c95cd457e04864e0dc737f6c2774cc4c`, including current main and the
independently admitted scoped appearance/CSS identifier repairs in PR #226.
PR #226 subsequently merges as `a933b02c1` with the exact same full tree
`073080433a69fb3038bf4459784dc9989f895eb9`; the historical timing identity stays `b9a58731c`.

The public operation remains `Renderer::render(RenderRequest::svg(...))`, including the rich
output and its reports at the operation boundary. No SVG-bytes API replaces it. Both sides use
Rust 1.95.0, macOS / Apple M4 Pro, default features enabled plus
`all-diagrams,svg,layout-cytoscape,layout-elk,math`, locked dependencies and optimized bench
profile. Their different release lockfiles remain part of this comparison.

An upstream prepared-redraw change alters the private layout/render benchmark API. Isolated
bench-only snapshots remove those two private microbenchmarks and their unused imports on both
sides. All public E2E code, fixtures and preflight/postflight contracts remain byte-identical;
production benchmark source and the comparator identity gate stay unchanged. The snapshots are
base `2a1ebf70af98c01ca74a64cb4567af4c1bc8880f` and head
`70b7cf6f6e1b59aa30faebf4ce021596921f3081`. Both full `pipeline.rs` files have SHA-256
`cff13a13474a46c15645014ef1d850b3dec4043783bc64ff8ab048f442607d92` and actually discover
307 benchmarks in five groups, rather than the 459-entry production list. Both reproducible
patches and the complete common source are archived.

## Input, output and executable gates

Fresh all-76 inventory verifies identical inputs, available capabilities and valid complete SVG
receipts. There are 19 exact-output scenes and 57 changed-output scenes; the latter intentionally
make discovery-only inventory exit 2. This phase contains no A/A or A/B observations.

Separately registered raw capture and strict independent replay verify all 76 physical SVGs on
each side against the no-hook inventory: full SHA-256, byte length, XML element count and every
receipt field. Both capture and replay runs succeed. All 152 files also exactly match the
October 19 captures, preserving its source-backed 57-row difference classification without
normalization or same-output admission.

The repair clears only package `merman`'s bench cache, leaving source and Git work intact. An
explicit head rebuild reports a unique pipeline compiler artifact with `fresh=false` and the
registered common checkout's `manifest_path` and `src_path`. Independent admission verifies
both actual executables, the read-only head copy and fresh inventory before sampling. The head
matches the original pre-capture inventory executable and differs from the rejected hook binary.
All three capture-hook markers are absent on both sides. External identity guards verify the
same physical SHA values after the formal tool's own prepare step and at terminal. Both actual
sampled binaries are copied read-only before any later native work. No other native build,
probe, profile or test runs during sampling.

This failure demonstrates that a clean checkout, Cargo success and unchanged-file postflights
alone cannot bind a shared cached artifact to the intended source. Capture-only and timing
artifacts require separate physical identities; after a source variant shares a target, force
an appropriate bounded rebuild or isolate its target before admitting timing.

## Frozen confirmation contract

The original 19-scene order is fixed before calibration. Eight fresh A/A pairs on each runner
produce 608 observations. All 19 select their qualified relative certificate. The newly written
exclusive plan is read back before A/B and remains unchanged. It requires eight balanced pairs
per fixture, four AB and four BA, producing 304 new confirmation observations. Every observation
passes its strict output receipt and postflight. Both runners pass final source, executable,
manifest, input and lockfile checks; independent physical-source/executable admission is an
additional requirement, rather than an inference from the comparator's internal success.

Criterion uses 30 samples, two-second warmup and three-second measurement. The pair cap is 32;
confidence is conditional joint 95% decision error across 38 Bonferroni components, with the
existing deterministic 10,000-resample bootstrap and seed zero. This does not claim simultaneous
coverage of both interval endpoints, and A/A sizing does not guarantee power. The per-scene
joint budget remains 10% and 1 microsecond. No old observation or discovery injection is reused,
and no sampling rule changes after seeing results.

Nine absolute A/A certificates remain unqualified: all five Class scenes, Gantt, Requirement,
C4 and Kanban. The archived plan retains their exact identity/order failures and uncapped pair
requirements, including 133,010 for the largest Class fixture; the configured cap is not raised.
Qualified relative certificates can establish non-regression independently. Confirmed joint
regression requires both applicable certificates.

## Complete results

Times are microseconds and arithmetic means of eight point estimates; differences average paired
deltas. Percent changes transform the mean paired log ratio and need not equal the ratio of the
displayed means. Bounds are the protocol's one-sided decision bounds. Within budget means the
preselected certificate meets its threshold, rather than zero overhead.

| Scene | v0.9.0 (us) | Candidate (us) | Difference (us) | Change | Relative bounds | Decision |
| --- | ---: | ---: | ---: | ---: | ---: | --- |
| `class_tiny` | 128.7650 | 139.7075 | +10.9425 | +8.50% | +8.00% .. +8.98% | Within joint budget |
| `class_medium` | 657.5825 | 652.2888 | -5.2938 | -0.81% | -1.46% .. +0.03% | Within joint budget |
| `class_namespace_dense` | 406.0063 | 415.1087 | +9.1025 | +2.24% | +1.61% .. +2.59% | Within joint budget |
| `class_nested_namespaces` | 1605.0250 | 1652.0375 | +47.0125 | +2.92% | +1.99% .. +4.51% | Within joint budget |
| `class_nested_namespaces_large` | 15563.3750 | 15640.6250 | +77.2500 | +0.50% | +0.10% .. +1.05% | Within joint budget |
| `pie_medium` | 25.5631 | 27.6626 | +2.0995 | +8.21% | +7.53% .. +8.95% | Within joint budget |
| `journey_medium` | 27.4612 | 28.4159 | +0.9546 | +3.48% | +2.88% .. +3.97% | Within joint budget |
| `timeline_medium` | 39.8676 | 42.5315 | +2.6639 | +6.68% | +5.98% .. +7.46% | Within joint budget |
| `gantt_medium` | 36.6936 | 39.1855 | +2.4919 | +6.79% | +6.31% .. +7.36% | Within joint budget |
| `requirement_medium` | 238.2388 | 244.2113 | +5.9725 | +2.51% | +1.90% .. +3.25% | Within joint budget |
| `c4_medium` | 105.8662 | 111.6462 | +5.7800 | +5.46% | +4.59% .. +6.42% | Within joint budget |
| `sankey_medium` | 23.6685 | 26.0093 | +2.3407 | +9.89% | +9.02% .. +10.71% | Inconclusive |
| `quadrant_medium` | 26.6546 | 28.3954 | +1.7408 | +6.53% | +5.58% .. +7.71% | Within joint budget |
| `zenuml_medium` | 19.9846 | 23.4836 | +3.4990 | +17.51% | +16.88% .. +18.36% | Regression |
| `packet_medium` | 8.0875 | 10.4196 | +2.3322 | +28.83% | +26.39% .. +31.10% | Regression |
| `kanban_medium` | 44.9776 | 53.4982 | +8.5206 | +18.95% | +18.48% .. +19.60% | Inconclusive |
| `architecture_medium` | 45.4489 | 47.3895 | +1.9406 | +4.27% | +3.77% .. +4.82% | Within joint budget |
| `wardley_medium` | 40.3980 | 44.2062 | +3.8083 | +9.43% | +8.51% .. +10.22% | Inconclusive |
| `cynefin_medium` | 29.1309 | 30.7967 | +1.6659 | +5.72% | +4.91% .. +6.64% | Within joint budget |

ZenUML and Packet remain confirmed regressions. Sankey and Wardley relative bounds cross 10%.
Kanban's point estimate is +18.95% / +8.521 us and both displayed confirmation lower bounds
exceed the nominal thresholds, but its absolute A/A order certificate is unqualified. The existing
joint classifier therefore returns inconclusive. This is a precision/qualification boundary,
not recovery or budget compliance; Kanban remains an optimization priority.

Earlier `a6742779c` and rejected-attempt classifications retain their own revisions, samples and
qualification. Their counts cannot establish an improvement or deterioration caused by an
individual repair, and no favorable rerun is used to change this checkpoint.

## Coverage and next candidate

The 57 changed-output scenes retain their complete records and the
[source-backed release SVG audit](theme_release_svg_audit_macos_2026-10-19.md). They cannot prove
same-output release convergence. Separately, all 35 logical families / 38 aliases across ten
typed presets plus None have 418-cell adjacent allocation/behavior coverage. v0.9.0 lacks the
current typed request API, so that lane is not silently substituted into this release comparison.
The all-family convergence goal remains incomplete.

The next source-backed candidate removes the intermediate owned `ParseMetadata` copied for the
default facade SVG path and then discarded without a consumer. This is a string/Arc clone/drop
cost, not a configuration JSON deep copy. A separate adjacent registration must preserve public
raw metadata ownership, actual pipelines, evidence, cancellation/resource errors and the parsed
model release boundary. A fresh-render `PreparedSource` Arc with no shared consumer is a second,
independent ownership candidate. Neither has a measured benefit in this checkpoint.

## Frozen evidence

The [evidence archive](evidence/theme-v090-appearance-css-2026-10-20/manifest.json) preserves the
complete accepted report, immutable plan, registration, identity repair, raw gates, common bench
source and independent receipts, together with the clearly rejected first attempt. Read-only
sampled executables stay in the ignored repair directory.

- Accepted report SHA-256: `79bd111960b9a5d69e8b959195cd83db33ef3b4bdbcf1f1c7dff4ab03bb9d1b3`.
- Pre-A/B plan SHA-256: `64d7040842ad9b721c018280e11d2d7e618482297bb05ce91c4a6b864868dda6`.
- Independent final review SHA-256: `5be5e05309e4cd327e3e37582188eb692ac4493feda167653bd3cca4752bf7db`.
- Baseline sampled executable SHA-256: `8712a3bbd5c054fdb2c9ad5b923d867818b5ec916c33326d2f28bac67d456042`.
- Candidate sampled executable SHA-256: `6da05572985bb212236a5c3d57dfd95ceb11e32eb229b69f9366566b2a2960a5`.

Ignored execution record:
`target/bench/experiments/theme-current-v090-confirmation-repair-20261020/`.
