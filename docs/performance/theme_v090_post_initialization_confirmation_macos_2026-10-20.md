# v0.9.0 post-initialization release confirmation — macOS, October 20, 2026

The complete preregistered same-SVG cohort has **15 confirmed non-regressions, three confirmed
regressions and one inconclusive scene**, with no evidence-contract failures. This compares
v0.9.0 `f754a3c3077d35d2a372c2fec04f9e9b87db1c3f` with clean
`a6742779c80252e40f535a3de087f842a61ba94c`, including the accepted literal XHTML paragraph
and prepared compiled-theme initialization changes. ZenUML, Packet and Kanban remain beyond
both release thresholds; Class tiny remains inconclusive. The all-family goal is not complete.
This is a new release-distance observation, not an isolated optimization admission.

## Frozen workload and protocol

All 19 previously selected scenes across 15 logical families remain in scope. Benchmark source,
corpus and selected input bytes match across runners; every selected raw SVG hash, byte count
and element count matches. The 57 changed-SVG scenes and three preprocessing scenes stay separate;
see the [release SVG audit](theme_release_svg_audit_macos_2026-10-19.md). The separate 418-cell
compiled-theme catalog covers 35 logical families, 38 fixture aliases and ten presets plus None.
The typed compiled-theme request API has no matching v0.9.0 surface and is not part of this timing.

Both native public `end_to_end` runners reuse an Engine, retain default features and enable
`all-diagrams,svg,layout-cytoscape,layout-elk,math`, with deterministic measurement and
`trusted_native` resource policy. The Apple M4 Pro host uses Rust 1.95.0 and matching bench
profiles in distinct target directories. Every observation uses 30 Criterion samples, 2 s
warmup and 3 s measurement, with one render per estimate. Builds, probes, profiling and native
measurements do not overlap.

Fresh calibration completes eight A/A pairs per runner per scene: **608 observations**. The tool
writes and reads back its immutable plan before starting the complete fixed eight-pair A/B
schedule: **304 observations**, four AB and four BA pairs per scene. All 19 rows preselect the
qualified relative certificate. There is no filtering, resampling, metric switching or truncation
of uncapped sample requirements. The independent plan review is a post-freeze review after A/B
started; it is not a manual pre-A/B approval. All observation postflights and both final runner
source/executable/input/manifest/lockfile checks pass.

The unchanged `joint_non_regression_v1` contract fixes 10% and 1000 ns boundaries, 10,000
bootstrap resamples and 95% conditional decision confidence with 38 Bonferroni components.
This is not simultaneous 95% coverage of both endpoints, and A/A sizing does not guarantee power.
The actual driver terminal exit is 1 for confirmed regressions, with zero contract errors.
The first observed terminal poll is `2026-10-20T05:53:55.003741+00:00`; the report's
`generated_at` records tool initialization rather than sampling completion. Both actual chosen
executables were copied and SHA-verified before subsequent native work.

## Complete results

Times are microseconds. Times are arithmetic means of eight point estimates; differences average
paired deltas. Changes transform mean paired log ratios and need not equal the ratio of displayed
means. Bounds are the protocol's one-sided decision bounds. Within the joint budget means the
preselected certificate's upper bound meets its threshold, not zero overhead.

| Scene | v0.9.0 (us) | Head (us) | Difference (us) | Change | Relative bounds | Decision |
| --- | ---: | ---: | ---: | ---: | ---: | --- |
| `class_tiny` | 129.0175 | 141.5788 | +12.5613 | +9.74% | +9.03% .. +10.29% | Inconclusive |
| `class_medium` | 662.5462 | 657.1800 | -5.3662 | -0.81% | -1.27% .. -0.33% | Within joint budget |
| `class_namespace_dense` | 408.2125 | 416.8763 | +8.6638 | +2.12% | +1.81% .. +2.43% | Within joint budget |
| `class_nested_namespaces` | 1619.7375 | 1652.0500 | +32.3125 | +2.00% | +1.27% .. +2.63% | Within joint budget |
| `class_nested_namespaces_large` | 15597.1250 | 15685.1250 | +88.0000 | +0.57% | +0.08% .. +1.04% | Within joint budget |
| `pie_medium` | 25.6893 | 27.7639 | +2.0746 | +8.08% | +7.15% .. +8.88% | Within joint budget |
| `journey_medium` | 27.6995 | 28.4062 | +0.7067 | +2.55% | +1.74% .. +3.81% | Within joint budget |
| `timeline_medium` | 40.2025 | 42.5349 | +2.3324 | +5.80% | +5.04% .. +6.58% | Within joint budget |
| `gantt_medium` | 37.0609 | 39.1659 | +2.1050 | +5.68% | +5.01% .. +6.73% | Within joint budget |
| `requirement_medium` | 239.1138 | 245.6887 | +6.5750 | +2.75% | +2.16% .. +3.23% | Within joint budget |
| `c4_medium` | 106.5125 | 113.1262 | +6.6137 | +6.20% | +5.08% .. +7.38% | Within joint budget |
| `sankey_medium` | 23.9676 | 26.0087 | +2.0411 | +8.52% | +7.47% .. +9.75% | Within joint budget |
| `quadrant_medium` | 26.7775 | 28.2424 | +1.4649 | +5.47% | +4.85% .. +6.15% | Within joint budget |
| `zenuml_medium` | 20.0508 | 23.4941 | +3.4434 | +17.18% | +15.63% .. +18.14% | Regression |
| `packet_medium` | 8.1045 | 10.3079 | +2.2034 | +27.18% | +25.86% .. +28.48% | Regression |
| `kanban_medium` | 45.3274 | 53.5252 | +8.1979 | +18.09% | +16.82% .. +18.97% | Regression |
| `architecture_medium` | 45.3326 | 47.5796 | +2.2470 | +4.96% | +4.05% .. +5.94% | Within joint budget |
| `wardley_medium` | 40.3921 | 44.0546 | +3.6625 | +9.06% | +8.64% .. +9.60% | Within joint budget |
| `cynefin_medium` | 29.2586 | 30.7664 | +1.5077 | +5.15% | +4.07% .. +6.40% | Within joint budget |

ZenUML, Packet and Kanban qualify both metrics at eight pairs and have both lower bounds
strictly beyond the thresholds. Class tiny qualifies both metrics in this fresh calibration,
unlike the previous post-ownership run. Its relative bounds cross 10%, so it establishes neither
joint regression nor non-regression. Its absolute lower bound alone cannot decide the joint gate.

Six absolute certificates are unqualified: C4 (10 required pairs), Class medium (332),
Class namespace dense (36), Class nested namespaces (1596), Class nested namespaces large
(36704) and Requirement (8). All failed precision reasons and uncapped requirements remain in
the frozen plan. A requirement within 32 pairs does not override failed precision checks.
Each of these six rows has a qualified relative certificate and establishes non-regression.

## Attribution and next work

The [previous post-ownership confirmation](theme_v090_post_ownership_confirmation_macos_2026-10-20.md)
remains 12 non-regressions, three regressions and four inconclusive scenes at its own frozen
`9f8143a06fb73390571f72489aa89098315e34e2`. The present classifications do not establish
an adjacent-commit speedup or prove which change caused any historical difference. In particular,
prepared compiled-theme initialization is not invoked by this no-request-theme cohort.
The [compiled initialization allocation result](compiled_theme_initialization_macos_2026-10-20.md)
and [literal XHTML work removal](xhtml_plain_paragraph_macos_2026-10-20.md) retain their separate
admission lanes and memory boundaries.

Continue source-proven ownership/query work removal independently: an empty explicit-owner set
cannot own any queried path; absent scoped appearance keys do not consume constructed scoped
paths; already normalized CSS IDs need no duplicate owned escape result. Each candidate needs a
fresh adjacent registration and its own output/error/allocation controls. These are not explanations
of the entire remaining release gap. The bounded ZenUML CSS assembly and sink-dispatch ideas
remain unmeasured hypotheses; no allocation or latency win is inferred from sample counts.
Missing coverage, inconclusive timing and unexplained residuals cannot close the all-family goal.

## Evidence identities

The [archive manifest](evidence/theme-v090-post-initialization-2026-10-20/manifest.json) records
losslessly compressed original bytes for the complete report, immutable plan, registration,
execution, reviews and executable archive manifest. Read-only copies of the actual measured
executables remain under the ignored experiment directory.

- Frozen plan SHA-256: `8e688ad6ef4891cb14938c70ad70255bbf33d0266296741e90d335dac542280d`.
- Complete report SHA-256: `715e0cd1227e93cf757bfdc2dc3fea4db5fc3f3027d2e8bb848bcf9db7fcfdf2`.
- Tag executable SHA-256: `5562fea88eb11457fdbf459c61076cd4011c476a0fc26828eb39701987b65f57`.
- Head executable SHA-256: `9fc99ad7942dd8d103a8ccf1040a19fe3c8d56a2fffe91fb000ccc8bce0673cb`.

The ignored directory `target/bench/experiments/theme-v090-post-initialization-20261020/`
retains original reports, progress log, registration, reviews and matching executable copies.
