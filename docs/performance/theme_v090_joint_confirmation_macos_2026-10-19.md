# v0.9.0 joint release confirmation — macOS, October 19, 2026

The complete preregistered same-SVG cohort has **13 confirmed non-regressions, three confirmed
regressions and three inconclusive scenes**, with no evidence-contract failures. This compares
v0.9.0 `f754a3c3077d35d2a372c2fec04f9e9b87db1c3f` with clean
`0ce2aa0bdafd4988b22cf51f9650d1c653aa9d03`. Zenuml, Packet and Kanban remain outside the
joint 10% and 1 us release budget. The all-family convergence goal remains active.

## Frozen scope and measurement

All 19 scenes across 15 logical families come from the earlier output-identity selection, before
these timings. Fixture bytes, benchmark source and corpus are identical across the two runners;
raw SVG bytes, hashes and element counts match for every selected scene. The other 57 changed-SVG
scenes and three preprocessing scenes stay outside this latency cohort. They remain covered by
the [release SVG audit](theme_release_svg_audit_macos_2026-10-19.md), rather than being silently
removed or treated as equivalent workloads. The separate 418-cell theme catalog covers all 35
logical families and 38 fixture aliases, ten compiled presets plus no theme; it is not a
cross-release latency result.

Both executables use the native public `end_to_end` pipeline with a reused Engine, deterministic
measurement, no compiled request theme, default features retained and
`all-diagrams,svg,layout-cytoscape,layout-elk,math`. The host is Apple M4 Pro, 48 GiB, macOS aarch64,
Rust 1.95.0. Criterion uses 30 samples, 2 s warmup and 3 s measurement per observation; each estimate
represents one public render. Tag and head use separate target directories and matching bench
profiles. Builds, calibration and confirmation run serially, without overlapping Cargo, probes
or profiling.

`joint_non_regression_v1` completes eight fresh A/A pairs on each executable for every scene:
608 observations. Before A/B, the exclusive calibration-plan file freezes scope, identities,
qualification, the selected certificate and sample count. Every scene has a qualified relative
certificate requiring eight pairs. The full cohort then completes exactly eight fresh balanced
AB/BA pairs: 304 observations, with no additional pairs or timing-selected exclusions. Every
observation passes its output postflight. Both runner executable/source/manifest/lockfile
post-sampling verifications pass, with the original clean revisions unchanged. The driver finishes
at `2026-10-19T15:42:45.712182+00:00`, exit 1 for confirmed regressions, not a contract failure.

The protocol fixes the 10%/1000 ns boundary, 10,000 bootstrap resamples and 95% family-wise
conditional decision confidence. Its 38-component Bonferroni adjustment applies to the decision
error contract; it does not promise simultaneous 95% coverage of both interval endpoints.
A/A sample sizing is a heuristic, not guaranteed power. The source performs exclusive plan write
and readback before A/B; the progress log has no separate timestamped plan-write event.

## Complete results

Times are microseconds. Base and head are arithmetic means of the eight executable point
estimates. Differences average paired deltas; relative changes are transformed mean paired log
ratios, so they need not equal the ratio of the displayed means. Bounds are the protocol's
one-sided decision bounds. “Within joint budget” means the preselected certificate's upper bound
is at most its threshold, not identical latency or zero overhead.

| Scene | v0.9.0 (us) | Head (us) | Difference (us) | Change | Relative bounds | Decision |
| --- | ---: | ---: | ---: | ---: | ---: | --- |
| `class_tiny` | 128.7975 | 141.7713 | +12.9738 | +10.07% | +9.49% .. +10.67% | Inconclusive |
| `class_medium` | 658.7425 | 674.3475 | +15.6050 | +2.37% | +1.88% .. +2.84% | Within joint budget |
| `class_namespace_dense` | 405.4325 | 420.7437 | +15.3112 | +3.78% | +2.92% .. +4.65% | Within joint budget |
| `class_nested_namespaces` | 1608.0250 | 1635.0875 | +27.0625 | +1.68% | +0.91% .. +2.49% | Within joint budget |
| `class_nested_namespaces_large` | 15507.7500 | 15661.8750 | +154.1250 | +0.99% | +0.40% .. +1.60% | Within joint budget |
| `pie_medium` | 25.5553 | 27.6785 | +2.1233 | +8.31% | +7.80% .. +8.93% | Within joint budget |
| `journey_medium` | 27.7229 | 28.4446 | +0.7218 | +2.61% | +1.63% .. +3.51% | Within joint budget |
| `timeline_medium` | 40.0691 | 43.0603 | +2.9911 | +7.47% | +6.62% .. +8.28% | Within joint budget |
| `gantt_medium` | 36.6370 | 39.2360 | +2.5990 | +7.09% | +6.49% .. +7.72% | Within joint budget |
| `requirement_medium` | 236.7788 | 244.7300 | +7.9512 | +3.36% | +2.70% .. +4.18% | Within joint budget |
| `c4_medium` | 105.6937 | 111.1375 | +5.4437 | +5.15% | +4.56% .. +5.82% | Within joint budget |
| `sankey_medium` | 23.6966 | 26.0841 | +2.3875 | +10.08% | +9.50% .. +10.70% | Inconclusive |
| `quadrant_medium` | 26.6934 | 29.0684 | +2.3750 | +8.90% | +7.94% .. +9.89% | Within joint budget |
| `zenuml_medium` | 19.9956 | 23.5566 | +3.5610 | +17.81% | +16.25% .. +19.17% | Regression |
| `packet_medium` | 8.1185 | 10.3522 | +2.2338 | +27.51% | +25.97% .. +28.76% | Regression |
| `kanban_medium` | 45.1073 | 57.5508 | +12.4435 | +27.58% | +26.81% .. +28.38% | Regression |
| `architecture_medium` | 45.1733 | 47.8610 | +2.6877 | +5.95% | +5.39% .. +6.57% | Within joint budget |
| `wardley_medium` | 40.2563 | 44.3841 | +4.1279 | +10.25% | +9.28% .. +11.22% | Inconclusive |
| `cynefin_medium` | 29.1157 | 30.7264 | +1.6106 | +5.53% | +4.80% .. +6.29% | Within joint budget |

Zenuml's absolute bounds are +3.27..+3.81 us, Packet's +2.12..+2.33 us and Kanban's
+12.12..+12.86 us. Each has both metrics qualified at the fixed eight-pair count and both lower
bounds strictly beyond 10% and 1 us. Class tiny, Sankey and Wardley cross the relative boundary
and therefore remain inconclusive; their point estimates do not settle the decision.

Seven absolute certificates are unqualified: Class medium (178 required pairs), Class namespace
dense (38), Class nested namespaces (1898), Class nested namespaces large (123932), Timeline (8),
Requirement (12) and C4 (48). The uncapped requirements and all precision failures remain in the
frozen plan. Timeline's sample count alone does not qualify its failed precision checks. These
seven scenes can establish non-regression through their preregistered relative certificate; they
cannot use an unqualified absolute bound to establish joint regression. No failed certificate
was truncated to the cap or reselected after A/B.

## Attribution and next work

This release-range observation establishes the current gap, not the cause or speedup of any one
intermediate commit. It does not admit a candidate, evaluate improvement, reclassify the older
[Packet/Kanban/C4 receipt](theme_v090_confirmation_macos_2026-10-19.md), or establish equivalence of
release-range errors, cancellation and resource policies. C4's new relative certificate can
establish the current joint budget without rewriting its old failed absolute calibration.

Source audits already show that ordinary no-theme Packet and Kanban skip compiled-theme
traversal. Their remaining gap cannot be assigned wholesale to theme preparation. Keep the
[stage attribution boundaries](packet_kanban_stages_macos_2026-10-19.md) and the rejected
[bounded-active candidate](bounded_active_rejected_macos_2026-10-19.md); necessary XML/resource work
still needs owner-local evidence before it can explain a quantified share of the gap.

The next separately registered allocation candidate replaces copied Info/Error terminal CSS
values with their existing sticky comparison state and borrows Treemap's expected title fill.
It preserves duplicate/missing-event rejection, completion uniqueness, text geometry and every
emission/checkpoint. Then independently evaluate the source-reviewed Quadrant point-binding,
empty GitGraph palette CSS, empty XY paint owner, common root-font owner and Pie temporary CSS
borrowing candidates. These are proposals, not measured or admitted results. Preserve all
successful outputs/reports/ordered measurer traces, all error controls and the full theme catalog
for each accepted change. A fresh release timing experiment is a later independent decision;
do not append samples to this completed cohort.

## Evidence identities

The [archive manifest](evidence/theme-v090-joint-2026-10-19/manifest.json) records byte counts and
SHA-256 values for the compressed complete report, frozen pre-A/B plan, registration and terminal
execution receipt. Decompression preserves the original bytes, including all 608 A/A and 304 A/B
observations, qualifications, commands, input identities and postflight receipts.

- Frozen calibration plan SHA-256:
  `acaf0dedc392c40bb6163134593cfb148fae8da9d0019e00b0b7e41d55df9c7d`.
- Complete report SHA-256:
  `fda308e40327d9b8e4c43bd26efbc2a5ee817ce6374e3b1391437b711ab78308`.
- Tag executable SHA-256:
  `5562fea88eb11457fdbf459c61076cd4011c476a0fc26828eb39701987b65f57`.
- Head executable SHA-256:
  `86446c874503da5aa7bda50b7066feee5289e980bfdeb89379b37fe067aebc19`.

The ignored directory `target/bench/experiments/theme-v090-joint-all19-20261019/` retains the
progress log, independent pre-A/B and final reviews, original receipts and matching read-only
executable copies. The two immutable copies were verified before any subsequent production build.
