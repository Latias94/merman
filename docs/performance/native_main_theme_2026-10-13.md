# Native SVG: main versus theme branch, 2026-10-13

This checkpoint compares the complete warm native Rust source-to-SVG operation. It diagnoses the existing theme branch; no performance implementation is included.

## Result

Confirmation summary: `{"comparable": 9, "confirmed_improvements": 0, "confirmed_regressions": 9, "contract_failures": 0, "exit_code": 1, "inconclusive": 0, "outcome": "confirmed_regression"}`.

The discovery suite contains 38 inputs. Nineteen produce byte-identical SVGs and were timed; all nineteen are slower in discovery. The other nineteen retain their output differences as coverage findings and are outside same-output latency admission. The broad discovery receipt therefore exits 2 with `contract_failure`; it is not a successful blanket suite. The independently confirmed subset has no output or runner contract failures.

Nine discovery cases exceed the preregistered joint 10% and 50 microsecond thresholds and form the frozen confirmation scope. Both executables receive eight balanced A/A pairs per case. Fresh AB/BA samples use the power-derived even count, at least eight and capped at 32, without reusing discovery or calibration samples. Bounds are simultaneous one-sided 95% paired-bootstrap bounds, Bonferroni-adjusted across the nine cases and two metrics, with 10,000 resamples.

Reported side timings are arithmetic means of the Criterion console point estimates across independent runs, not per-render percentiles or medians. Percentage changes in the tables use the ratio of those means; the confidence bounds use paired log ratios.

| Fixture | Main (us) | Theme (us) | Increase | Delta (us) | Relative lower bound | Delta lower bound (us) | AB/BA pairs | Outcome |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | --- |
| class_medium | 652.900 | 1190.562 | +82.35% | +537.663 | +80.53% | +527.879 | 8 | confirmed_regression |
| journey_medium | 27.525 | 95.371 | +246.49% | +67.847 | +239.69% | +67.110 | 8 | confirmed_regression |
| timeline_medium | 39.534 | 91.676 | +131.89% | +52.142 | +129.98% | +51.565 | 8 | confirmed_regression |
| requirement_medium | 235.775 | 405.965 | +72.18% | +170.190 | +71.25% | +168.005 | 8 | confirmed_regression |
| c4_medium | 104.870 | 232.895 | +122.08% | +128.025 | +120.24% | +126.154 | 8 | confirmed_regression |
| kanban_medium | 44.558 | 126.291 | +183.43% | +81.733 | +177.98% | +80.200 | 8 | confirmed_regression |
| xychart_medium | 88.850 | 175.024 | +96.99% | +86.173 | +93.72% | +83.818 | 8 | confirmed_regression |
| wardley_medium | 39.917 | 100.035 | +150.61% | +60.118 | +148.33% | +59.308 | 8 | confirmed_regression |
| usecase_basic | 198.641 | 368.774 | +85.65% | +170.132 | +83.78% | +168.532 | 8 | confirmed_regression |

## Workload and provenance

- Base: `3a4b8c393939b69372a8f05a2b971f7cdf6ae529`; target: `f47abded02b540d7fe563af7bc75f2fa1bb90c51`. Both measured checkouts were clean.
- Host: Apple M4 Pro, 14 cores, 48 GiB RAM, macOS 27.0 (26A428), ARM64; Rust/Cargo 1.95.0.
- Native `merman` / `pipeline`, locked release/bench build, default features plus `svg` on both sides.
- Strict parser; deterministic SVG environment; `trusted_native` resource policy; reused Engine/Renderer; default Engine configuration including each diagram's scoped theme/look defaults, no Merman preset.
- Criterion: 30 samples, two-second warmup, three-second measurement. Measurements and Cargo builds are serial; `CARGO_BUILD_JOBS=1`.
- This is SVG generation, not PNG/PDF painting, CLI process startup, browser WASM, N-API or host font discovery.
- `parse_cold_engine` constructs a new Engine per parse in an already running process. Global initialization is warmed; it is not process-cold startup.
- Same fixture bytes, corpus and benchmark source; all timed runs have successful preflight and matching postflight output identity.

| Identity | Main | Theme |
| --- | --- | --- |
| bench_source | `289d59b00ed8eb29615d6cdc4871dffef48b242471e2d60bae32994aa8442c16` | `289d59b00ed8eb29615d6cdc4871dffef48b242471e2d60bae32994aa8442c16` |
| corpus | `9258b9f72c14893a5cb64b2b05a325dc32ac144b827f44717a3030d4a537e076` | `9258b9f72c14893a5cb64b2b05a325dc32ac144b827f44717a3030d4a537e076` |
| lockfile | `0ffb737ac1ad0aa9cff246fefe73e3f22e702606fec46df04b579d2467ce2168` | `88b0d0baa92a1b62b915e5d9c7c542219a84dcb1f043a42078e994ef35f2b068` |
| executable | `c4f8cc5c7dbcea29e59c582ff5dff8cbb897f2f945723255c16baf0658f17bdd` | `c4d52cb1cbc4125de6f2d5f9f3d51f8be12b2a36b860519a9e764d6e0980a7e7` |

## Complete discovery results

These four-pair diagnostic values are advisory, including increases below 50 us. A case outside the confirmation subset is not evidence of confirmed non-regression. A large increase below the fallback absolute threshold can still matter for frequent calls; any such objective needs a separately frozen workload and gate.

| Fixture | Main (us) | Theme (us) | Increase | Delta (us) |
| --- | ---: | ---: | ---: | ---: |
| class_medium | 660.120 | 1188.175 | +79.99% | +528.055 |
| info_medium | 7.635 | 18.377 | +140.70% | +10.742 |
| pie_medium | 25.401 | 53.810 | +111.84% | +28.409 |
| journey_medium | 27.198 | 95.929 | +252.71% | +68.731 |
| timeline_medium | 39.636 | 91.405 | +130.61% | +51.769 |
| gantt_medium | 36.621 | 85.096 | +132.37% | +48.475 |
| requirement_medium | 234.475 | 402.692 | +71.74% | +168.218 |
| c4_medium | 104.737 | 230.912 | +120.47% | +126.175 |
| sankey_medium | 23.363 | 54.443 | +133.03% | +31.080 |
| quadrant_medium | 26.290 | 63.327 | +140.88% | +37.037 |
| zenuml_medium | 19.881 | 54.188 | +172.55% | +34.306 |
| packet_medium | 8.077 | 27.326 | +238.33% | +19.249 |
| kanban_medium | 44.277 | 126.808 | +186.40% | +82.531 |
| architecture_medium | 45.251 | 79.070 | +74.74% | +33.819 |
| xychart_medium | 90.135 | 175.350 | +94.54% | +85.215 |
| wardley_medium | 39.410 | 98.847 | +150.82% | +59.437 |
| cynefin_medium | 28.663 | 68.035 | +137.37% | +39.373 |
| error_basic | 7.393 | 26.014 | +251.85% | +18.620 |
| usecase_basic | 198.235 | 364.022 | +83.63% | +165.787 |

### Output-different coverage

None of these cases was admitted for same-output end-to-end timing. Equal byte counts or equal element counts do not establish equivalent SVGs, geometry or paint.

| Fixture | Main bytes | Theme bytes | Main elements | Theme elements |
| --- | ---: | ---: | ---: | ---: |
| flowchart_medium | 77505 | 82430 | 463 | 463 |
| state_medium | 49447 | 49448 | 189 | 189 |
| sequence_medium | 33713 | 34208 | 129 | 129 |
| er_medium | 63431 | 63431 | 400 | 400 |
| mindmap_medium | 37560 | 38480 | 179 | 179 |
| gitgraph_medium | 10682 | 10695 | 44 | 44 |
| block_medium | 8689 | 8693 | 60 | 60 |
| radar_medium | 6889 | 6899 | 24 | 24 |
| treemap_medium | 9584 | 9603 | 56 | 56 |
| venn_medium | 2229 | 2261 | 17 | 17 |
| swimlane_medium | 40716 | 41956 | 132 | 132 |
| eventmodeling_medium | 4542 | 4596 | 61 | 61 |
| treeview_medium | 2158 | 2634 | 22 | 22 |
| ishikawa_medium | 9807 | 11662 | 96 | 96 |
| railroad_medium | 4293 | 4352 | 43 | 43 |
| railroad_abnf_medium | 4506 | 4565 | 37 | 37 |
| railroad_ebnf_medium | 4607 | 4666 | 38 | 38 |
| railroad_peg_medium | 3715 | 3774 | 33 | 33 |
| agentflow_basic | 15610 | 16405 | 66 | 66 |

## Stage attribution

Each stage uses two diagnostic AB/BA pairs. Parse and layout cover eight cases; family SVG emission covers five byte-identical cases. Sequence layout becomes faster while its parse becomes slower, so a universal layout-regression claim is unsupported. Cold-engine construction is measured separately. Layout/paint preparation in the SVG-emission benchmark is untimed batched setup. Stage results use different lifecycles and drop boundaries, and cannot be subtracted from or summed into a measured public-operation decomposition.

### Reused-engine parse

| Fixture | Main (us) | Theme (us) | Increase | Delta (us) |
| --- | ---: | ---: | ---: | ---: |
| class_medium | 148.235 | 224.500 | +51.45% | +76.265 |
| journey_medium | 5.526 | 5.957 | +7.79% | +0.430 |
| kanban_medium | 7.171 | 7.353 | +2.53% | +0.182 |
| requirement_medium | 80.655 | 159.325 | +97.54% | +78.670 |
| xychart_medium | 12.396 | 13.248 | +6.88% | +0.853 |
| flowchart_medium | 246.490 | 329.065 | +33.50% | +82.575 |
| sequence_medium | 102.525 | 180.950 | +76.49% | +78.425 |
| mindmap_medium | 132.535 | 111.730 | -15.70% | -20.805 |

### New-engine parse

| Fixture | Main (us) | Theme (us) | Increase | Delta (us) |
| --- | ---: | ---: | ---: | ---: |
| class_medium | 153.330 | 224.055 | +46.13% | +70.725 |
| journey_medium | 8.848 | 9.373 | +5.94% | +0.525 |
| kanban_medium | 10.417 | 10.922 | +4.85% | +0.505 |
| requirement_medium | 83.387 | 160.430 | +92.39% | +77.043 |
| xychart_medium | 15.656 | 16.407 | +4.79% | +0.750 |
| flowchart_medium | 248.485 | 322.810 | +29.91% | +74.325 |
| sequence_medium | 105.080 | 181.840 | +73.05% | +76.760 |
| mindmap_medium | 134.485 | 113.625 | -15.51% | -20.860 |

### Layout/preparation

| Fixture | Main (us) | Theme (us) | Increase | Delta (us) |
| --- | ---: | ---: | ---: | ---: |
| class_medium | 266.275 | 331.195 | +24.38% | +64.920 |
| journey_medium | 3.736 | 6.389 | +71.02% | +2.653 |
| kanban_medium | 8.353 | 21.991 | +163.26% | +13.637 |
| requirement_medium | 74.779 | 79.113 | +5.80% | +4.333 |
| xychart_medium | 22.018 | 21.622 | -1.80% | -0.396 |
| flowchart_medium | 533.930 | 582.210 | +9.04% | +48.280 |
| sequence_medium | 82.475 | 55.270 | -32.99% | -27.205 |
| mindmap_medium | 53.999 | 64.686 | +19.79% | +10.686 |

### Family SVG emission

| Fixture | Main (us) | Theme (us) | Increase | Delta (us) |
| --- | ---: | ---: | ---: | ---: |
| class_medium | 230.070 | 321.375 | +39.69% | +91.305 |
| journey_medium | 16.590 | 25.235 | +52.11% | +8.645 |
| kanban_medium | 25.780 | 39.800 | +54.38% | +14.020 |
| requirement_medium | 73.129 | 92.368 | +26.31% | +19.239 |
| xychart_medium | 54.160 | 76.449 | +41.15% | +22.288 |

## CPU/source attribution and next owner

Four dedicated Class-medium CPU captures run the exact `parse` or `end_to_end` benchmark on each frozen executable. macOS `sample` collects five seconds at one millisecond intervals. Instrumented timing is excluded from all diagnostic and confirmation observations.

1. Parse: target sampling observes `materialize_selected_theme`, `apply_theme_defaults`, configuration copy-on-write and compatibility JSON cloning. Main also clones configuration, so function presence/counts cannot all be assigned as new overhead. Inspect the ownership and necessity of per-operation selected-theme materialization before choosing reuse keys.
2. Public SVG: target Class sampling observes `StandaloneSvgArtifact::finalize_exact`, XML/reference resource-budget validation and `standalone_svg_evidence`. Source computes the framed SVG resource fingerprint and artifact/admission digests. The finalizer has 719 inclusive samples in a capture with 3,621 main-thread samples. This identifies an owner, not removable time or an exclusive baseline-to-target cost delta.
3. Family preparation and SVG emission also move. Follow family-local evidence after profiling the shared configuration and finalization paths; avoid fixture-specific shortcuts.

Profile summaries retain the maximum inclusive call-tree node per function rather than summing repeated offsets. Counts overlap; inlining also changes symbol visibility. Do not add them into a CPU breakdown, infer nonexecution from missing symbols, or attribute the complete branch difference to one commit. Any optimization must preserve authored theme/config precedence, exact SVG bytes, resource budgets, cancellation, errors and sealed artifact/admission evidence.

The PR Performance workflow currently selects `quick` and `diagnostic` for pull requests (`.github/workflows/performance.yml`). A green diagnostic job does not establish absence of native latency regressions. This checkpoint does not change CI policy.

## Calibration and replay

Suite calibration: `{"exceeds_cap": false, "max_pairs": 32, "required_pairs": 8, "scheduled_pairs": 8}`.

| Fixture | Main A/A | Theme A/A | Required pairs |
| --- | --- | --- | ---: |
| class_medium | stable | stable | 8 |
| journey_medium | stable | stable | 8 |
| timeline_medium | stable | stable | 8 |
| requirement_medium | stable | stable | 8 |
| c4_medium | stable | stable | 8 |
| kanban_medium | stable | stable | 8 |
| xychart_medium | stable | stable | 8 |
| wardley_medium | stable | stable | 8 |
| usecase_basic | stable | stable | 8 |

Fresh confirmation console point estimates in us; each position is one independent paired round.

| Fixture | Main observations (us) | Theme observations (us) |
| --- | --- | --- |
| class_medium | 656.700, 653.000, 659.230, 657.450, 653.660, 649.630, 646.720, 646.810 | 1190.800, 1181.100, 1181.600, 1232.300, 1192.700, 1182.000, 1181.200, 1182.800 |
| journey_medium | 27.137, 27.437, 27.598, 27.727, 27.084, 27.294, 28.612, 27.309 | 94.840, 95.017, 95.164, 97.148, 95.527, 94.990, 95.045, 95.240 |
| timeline_medium | 39.556, 39.331, 39.594, 39.293, 39.340, 39.836, 39.872, 39.453 | 91.793, 92.065, 91.950, 91.426, 91.300, 92.799, 90.867, 91.210 |
| requirement_medium | 236.320, 234.790, 234.120, 235.670, 234.130, 235.870, 236.860, 238.440 | 406.740, 404.100, 402.860, 411.990, 401.110, 404.210, 409.900, 406.810 |
| c4_medium | 105.860, 104.090, 105.230, 103.830, 103.760, 106.280, 105.000, 104.910 | 236.460, 230.790, 235.230, 233.810, 230.500, 234.260, 229.760, 232.350 |
| kanban_medium | 44.622, 44.091, 44.538, 45.918, 43.674, 45.296, 44.038, 44.288 | 129.480, 126.200, 125.320, 125.030, 124.330, 129.450, 124.820, 125.700 |
| xychart_medium | 90.237, 88.180, 89.630, 89.238, 88.582, 88.612, 88.493, 87.831 | 174.050, 172.730, 173.950, 172.710, 172.290, 178.650, 181.450, 174.360 |
| wardley_medium | 39.588, 40.962, 39.361, 40.238, 39.451, 40.219, 39.622, 39.892 | 101.740, 102.000, 99.240, 100.420, 98.582, 99.411, 99.741, 99.145 |
| usecase_basic | 196.890, 201.430, 197.610, 204.930, 196.410, 199.230, 196.210, 196.420 | 364.540, 370.730, 372.420, 373.630, 366.620, 368.640, 365.670, 367.940 |

### Reproduce the confirmation

Use clean checkouts at the base and target revisions above. From the target checkout,
with the base checkout at `../bench-main-native-20261013`, run:

```bash
python3 tools/bench/compare_self.py \
  --base-dir ../bench-main-native-20261013 --head-dir . \
  --base-target-dir target/bench/experiments/native-main-comparison-20261013/build-main \
  --head-target-dir target \
  --base-label main-3a4b8c393 --head-label theme-f47abded0 \
  --base-package merman --head-package merman \
  --base-bench pipeline --head-bench pipeline \
  --base-features svg --head-features svg \
  --base-toolchain 1.95.0 --head-toolchain 1.95.0 \
  --base-corpus tools/bench/corpus.json --head-corpus tools/bench/corpus.json \
  --preset long --pairs 4 --calibration-pairs 8 --max-pairs 32 \
  --relative-threshold-percent 10 --absolute-threshold-ns 50000 \
  --timeout-seconds 3600 --evidence-mode confirmation \
  --filter 'end_to_end/(class_medium|journey_medium|timeline_medium|requirement_medium|c4_medium|kanban_medium|xychart_medium|wardley_medium|usecase_basic)' \
  --out target/bench/experiments/native-main-comparison-20261013/replay-confirmation.md \
  --json-out target/bench/experiments/native-main-comparison-20261013/replay-confirmation.json
```

Replay uses separate output names to retain the original receipt. It is a new experiment;
its host noise and power-derived pair count must be reported independently.

## Retained experiment artifacts

Local ignored evidence directory: `target/bench/experiments/native-main-comparison-20261013/`.

- `experiment.yaml`: frozen policy, environment, input hashes, commands and run outcomes.
- `diagnostic-command.json`, `attribution-commands.json`, `confirmation-command.json`, `cold-engine-command.json`: exact argument vectors; builds use fixed revisions in separate targets.
- Corresponding JSON/Markdown receipts and logs: raw pairs, A/A observations, identities and post-sampling verification.
- `cpu-profile-metadata.json`, `cpu-profile-hotspots.json`, `*-sample.txt`: profiler commands, digests, full captures and attribution limits.
- `artifacts.sha256`: hashes of the completed local top-level evidence files.

| Receipt | SHA-256 |
| --- | --- |
| end-to-end-diagnostic.json | `f46ec8618841c942d8ac87800f0d3935b3d63205060792a667a1b05129b343b8` |
| end-to-end-confirmation.json | `72e9c3a9cb4cbf03165e168402d4c35762f5c5c9a90a3a78ed97772a3b4d8dd8` |
| parse-diagnostic.json | `9a2a8fd0c5793b9047e1bd0a446fd59eaed45fbf58e3835ba96169f77ac0931f` |
| parse-cold-engine-diagnostic.json | `6bd2c63d3999b2799661ae607f8cc98b70be767da3363fad609fb079e74ae851` |
| layout-diagnostic.json | `a13709f43080768bca6876b382766ca240cca18f43aabf4dc987c85a7bf8db39` |
| render-diagnostic.json | `0dc3537ceb7bbf1474954e9083e0b43ee5600e2cd11398c0e890033ee1469279` |

Report generated at 2026-10-13T15:54:59.636869+00:00. No production performance patch is included.
