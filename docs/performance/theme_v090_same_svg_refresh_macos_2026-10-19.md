# v0.9.0 same-SVG E2E refresh (macOS, 2026-10-19)

This refresh measures candidate `546ccb4acec03467ea0085fc81b9dc949834403c` after the staged-empty,
family-preparation and default-font changes against the previously frozen native v0.9.0 pipeline.
All 19 previously selected same-SVG E2E scenes across 15 logical families are retained. Selection
uses output identity, never timing.

The unchanged pipeline uses a reused Engine, deterministic measurement, no compiled request theme
and identical fixture bytes. Two fixed AB/BA pairs per row give 76 calls, each with 30 Criterion
samples, 2 s warm-up and 3 s measurement. Values below are medians of two point estimates per
executable. Every preflight SVG identity and postflight check matches; no timing-based retry or
row deletion occurred. This is diagnostic only: there is no A/A calibration or formal latency
admission, and internal Criterion samples are not independent executable runs.

| E2E fixture | v0.9.0 (us) | Candidate (us) | Difference (us) | Change |
| --- | ---: | ---: | ---: | ---: |
| `class_tiny` | 122.7450 | 135.0550 | +12.3100 | +10.03% |
| `class_medium` | 639.6250 | 650.0150 | +10.3900 | +1.62% |
| `class_namespace_dense` | 397.5950 | 408.4000 | +10.8050 | +2.72% |
| `class_nested_namespaces` | 1589.2500 | 1606.7500 | +17.5000 | +1.10% |
| `class_nested_namespaces_large` | 15407.5000 | 15461.0000 | +53.5000 | +0.35% |
| `pie_medium` | 25.3585 | 27.7250 | +2.3665 | +9.33% |
| `journey_medium` | 27.3305 | 28.3860 | +1.0555 | +3.86% |
| `timeline_medium` | 39.6480 | 42.5330 | +2.8850 | +7.28% |
| `gantt_medium` | 36.2985 | 38.8025 | +2.5040 | +6.90% |
| `requirement_medium` | 234.4100 | 239.6050 | +5.1950 | +2.22% |
| `c4_medium` | 104.4650 | 115.3400 | +10.8750 | +10.41% |
| `sankey_medium` | 23.3620 | 25.7925 | +2.4305 | +10.40% |
| `quadrant_medium` | 26.1580 | 28.7335 | +2.5755 | +9.85% |
| `zenuml_medium` | 19.5535 | 23.1655 | +3.6120 | +18.47% |
| `packet_medium` | 7.8914 | 10.4780 | +2.5866 | +32.78% |
| `kanban_medium` | 44.5085 | 57.1735 | +12.6650 | +28.46% |
| `architecture_medium` | 46.9540 | 47.2640 | +0.3100 | +0.66% |
| `wardley_medium` | 39.6945 | 43.8370 | +4.1425 | +10.44% |
| `cynefin_medium` | 28.9350 | 30.6885 | +1.7535 | +6.06% |

Seven rows exceed both 10% and 1 us: Class tiny, C4, Sankey, Zenuml, Packet, Kanban and Wardley.
Class tiny, C4, Sankey and Wardley are close to the relative screen; this small diagnostic does not
establish either recovery or a regression against previous checkpoints. The other 12 rows are not
formally admitted. No average or theme-only attribution is inferred.

The 57 previously identified changed-SVG rendering rows and three preprocessing-only rows remain
separate, with the full names and fixture digests in the ledger. Current compiled-preset coverage
has 418 cells across 35 logical families and 10 presets plus no-theme; it is separate from these
release ratios. Successful SVG equality also does not establish equality of release-range error,
cancellation and resource contracts. See the [preceding refresh](theme_v090_same_svg_refresh_macos_2026-10-18.md)
for that coverage boundary and the unchanged-output selection method.

Kanban stage attribution needs care: current preparation resolves its 12 section CSS colors,
while v0.9.0 resolves them in emission. The larger layout-stage estimate is therefore partly a
stage-boundary move, not proof of added work. The current profile identifies component String
allocation in the shared color serializer as a distinct candidate. Retain full public-operation
controls when evaluating it; do not infer total improvement by subtracting private stages.

Raw observations, all 76 logs, executable/build receipts and fixture digests are under
`target/bench/experiments/theme-v090-refresh-20261019/`. Source status was clean and all input,
lockfile and executable digests were verified before execution. Frozen executables ran serially
without Cargo, profiling or allocation capture on Apple M4 Pro/48 GiB, macOS aarch64, Rust 1.95.0.

- Tag SHA-256: `5562fea88eb11457fdbf459c61076cd4011c476a0fc26828eb39701987b65f57`.
- Candidate SHA-256: `0521ef1ea3e88ef5af0dfb5c05cc89fc9a8cd32b39ce7be66ba8fc306522bcea`.

The all-family convergence goal remains active.
