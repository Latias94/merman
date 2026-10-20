# v0.9.0 post-ownership release confirmation — macOS, October 20, 2026

The complete preregistered same-SVG cohort has **12 confirmed non-regressions, three confirmed
regressions and four inconclusive scenes**, with no evidence-contract failures. This compares
v0.9.0 `f754a3c3077d35d2a372c2fec04f9e9b87db1c3f` with clean
`9f8143a06fb73390571f72489aa89098315e34e2`, after the accepted ownership changes through
Kanban. ZenUML, Packet and Kanban remain beyond both release thresholds. The all-family goal
remains active. This is an independent release-distance observation, not an adjacent-commit
optimization admission or a reclassification of earlier results.

## Frozen workload and protocol

All 19 previously selected scenes across 15 logical families remain in scope. Input bytes,
benchmark source and corpus match across runners, and every selected raw SVG hash, byte count
and element count matches. The 57 changed-SVG scenes and three preprocessing scenes stay separate;
see the [release SVG audit](theme_release_svg_audit_macos_2026-10-19.md). The 418-cell compiled
theme catalog covers 35 logical families and 38 fixture aliases, ten presets plus no theme;
it remains separate from this no-request-theme release timing.

Both native public `end_to_end` runners reuse an Engine and use deterministic measurement,
`trusted_native` resource policy, retained default features and
`all-diagrams,svg,layout-cytoscape,layout-elk,math`. The Apple M4 Pro host uses Rust 1.95.0
and matching bench profiles with distinct target directories. Every observation uses 30 Criterion
samples, 2 s warmup and 3 s measurement, with one render per estimate. Builds and measurements
are serial, without overlapping Cargo, probes or profiling.

Fresh calibration completes eight A/A pairs per runner per scene: 608 observations. The tool
exclusively writes and reads back the immutable pre-A/B plan before starting the complete
fixed eight-pair AB/BA schedule: 304 further observations, four AB and four BA pairs per scene.
Every row selects the qualified relative certificate before A/B. There is no row filtering,
metric switching, additional sampling or truncation of uncapped sample requirements. All
observation postflights and both runner source/executable/manifest/lockfile verifications pass.
The terminal driver exit is 1 for confirmed regressions, with zero contract errors. Its actual
completion time was not separately observed; the first terminal poll in this continuation was
`2026-10-20T01:09:15+00:00`.

The unchanged `joint_non_regression_v1` contract fixes 10% and 1000 ns boundaries, 10,000
bootstrap resamples and 95% conditional decision confidence with 38 Bonferroni components.
This is not simultaneous 95% coverage of both endpoints, and A/A sizing does not guarantee power.
The plan write is enforced by source and readback; no separate timestamped human pre-A/B audit
or pause is claimed.

## Complete results

Times are microseconds. Times are arithmetic means of the eight point estimates; differences
average paired deltas. Changes transform the mean paired log ratios and need not equal the ratio
of displayed means. Bounds are the protocol's one-sided decision bounds. Within the joint budget
means the preselected certificate's upper bound meets its threshold, not zero overhead.

| Scene | v0.9.0 (us) | Head (us) | Difference (us) | Change | Relative bounds | Decision |
| --- | ---: | ---: | ---: | ---: | ---: | --- |
| `class_tiny` | 129.8450 | 143.7138 | +13.8687 | +10.68% | +10.11% .. +11.01% | Inconclusive |
| `class_medium` | 662.6425 | 686.2162 | +23.5738 | +3.55% | +2.52% .. +4.54% | Within joint budget |
| `class_namespace_dense` | 407.6012 | 429.8550 | +22.2538 | +5.45% | +5.01% .. +6.40% | Within joint budget |
| `class_nested_namespaces` | 1608.8125 | 1646.2625 | +37.4500 | +2.33% | +1.92% .. +2.62% | Within joint budget |
| `class_nested_namespaces_large` | 15584.2500 | 15788.3750 | +204.1250 | +1.31% | +0.81% .. +1.82% | Within joint budget |
| `pie_medium` | 25.6310 | 28.0448 | +2.4137 | +9.42% | +8.44% .. +10.18% | Inconclusive |
| `journey_medium` | 27.6879 | 28.4095 | +0.7216 | +2.60% | +2.11% .. +3.50% | Within joint budget |
| `timeline_medium` | 40.2049 | 42.5284 | +2.3235 | +5.77% | +4.70% .. +6.63% | Within joint budget |
| `gantt_medium` | 37.0045 | 39.3530 | +2.3485 | +6.34% | +5.20% .. +7.51% | Within joint budget |
| `requirement_medium` | 238.8862 | 245.6900 | +6.8037 | +2.85% | +2.41% .. +3.43% | Within joint budget |
| `c4_medium` | 105.9963 | 112.0825 | +6.0862 | +5.74% | +5.13% .. +6.34% | Within joint budget |
| `sankey_medium` | 23.7024 | 26.1965 | +2.4941 | +10.52% | +9.74% .. +11.35% | Inconclusive |
| `quadrant_medium` | 26.7259 | 28.6914 | +1.9655 | +7.34% | +5.74% .. +8.82% | Within joint budget |
| `zenuml_medium` | 19.9710 | 23.5549 | +3.5839 | +17.95% | +17.05% .. +18.67% | Regression |
| `packet_medium` | 8.0632 | 10.3730 | +2.3098 | +28.66% | +27.59% .. +29.64% | Regression |
| `kanban_medium` | 45.1191 | 56.8564 | +11.7372 | +26.02% | +25.36% .. +26.64% | Regression |
| `architecture_medium` | 45.1187 | 47.9928 | +2.8740 | +6.37% | +5.77% .. +7.15% | Within joint budget |
| `wardley_medium` | 40.2590 | 44.1061 | +3.8471 | +9.56% | +8.78% .. +10.38% | Inconclusive |
| `cynefin_medium` | 29.2906 | 31.0066 | +1.7160 | +5.86% | +4.89% .. +6.68% | Within joint budget |

ZenUML, Packet and Kanban qualify both metrics at eight pairs and have both lower bounds
strictly beyond the thresholds. Pie, Sankey and Wardley cross the relative boundary and remain
inconclusive. Class tiny has both displayed lower bounds above the thresholds but its absolute
A/A precision checks failed. Its unqualified absolute bound cannot establish joint regression;
its selected relative upper bound also cannot establish non-regression. It remains inconclusive.

Eight absolute certificates are unqualified: Class tiny (8 required pairs), Class medium (758),
Class namespace dense (96), Class nested namespaces (6630), Class nested namespaces large
(210376), Requirement (24), C4 (8) and Architecture (10). Counts within the cap do not override
failed precision checks; requirements above the cap remain uncapped in the frozen plan. All eight
have qualified relative certificates. Class tiny is inconclusive; the other seven establish
non-regression through the preselected relative certificate.

## Attribution and next work

The [previous full confirmation](theme_v090_joint_confirmation_macos_2026-10-19.md) remains
13 non-regressions, three regressions and three inconclusive scenes at its own revision.
The change in classifications does not prove any individual later commit caused a speedup or
slowdown. Improvement is not assessed by this protocol. Historical candidate receipts, including
C4 precision failures, remain unchanged. This workload does not establish cross-release error,
callback, cancellation or resource-policy equivalence.

Next use the exact archived executables for bounded stage diagnostics on ZenUML, Packet and
Kanban with the previously selected Timeline control, followed by system CPU sampling. Parse
and layout projections may differ across releases, and preparation work has moved between stages;
do not sum stage differences or turn mismatched stages into equivalent-work latency claims.
No profiling crate or tracing instrumentation is required. Keep the rejected
[bounded-active candidate](bounded_active_rejected_macos_2026-10-19.md) separate.

A source-only audit also finds repeated compiled-theme compatibility installation and unnecessary
copy-on-write before checking whether a root font exists. Those paths are absent from this
default public cohort and cannot explain its regressions. A future prepared immutable config
owner must preserve per-operation detector/Tracking state, host overrides and parse-stage errors;
it needs a separate registration and full catalog validation.

## Evidence identities

The [archive manifest](evidence/theme-v090-post-kanban-2026-10-20/manifest.json) records
compressed original bytes for the full report, immutable pre-A/B plan, registration, execution
receipt, reviews and executable archive manifest. Read-only copies
of the actual sampled executables were verified before any subsequent native task.

- Frozen plan SHA-256: `31e63c5839437beac9f603ac33e9240e623d4cfc68f78d61f355983161014aa0`.
- Complete report SHA-256: `5f390ad98d08fa25b4f2026f4a174fea6ef4bde00ec37a4829696b281af336a0`.
- Tag executable SHA-256: `5562fea88eb11457fdbf459c61076cd4011c476a0fc26828eb39701987b65f57`.
- Head executable SHA-256: `d2009a7af7ad0496d5966334cdaf9dc7e808aa3d1052ffced9175d620efff317`.

The ignored directory `target/bench/experiments/theme-v090-post-kanban-20261020/` retains
original reports, progress log, registration, reviews and matching read-only executable copies.
