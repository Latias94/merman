# v0.9.0 cross-family SVG comparison on macOS (2026-10-16)

This report records a broad release-range comparison between `v0.9.0` (`f754a3c3`) and the committed cleanup candidate `9a297b72f`. It is release-range context, not causal proof for the theme cleanup: the range also contains Mermaid-parity, configuration, and output-contract changes.

## Measurement scope

- Host: Apple M4 Pro, 48 GiB; macOS; Rust 1.95.0; optimized bench profile, opt-level 3, no debug/assertions, no LTO, 16 codegen units.
- Surface: native `merman` Criterion `pipeline`, `default + svg`, strict parsing, deterministic SVG environment, trusted-native resources, reused renderer.
- Corpus: 79 registered fixtures; 76 SVG end-to-end inputs across 38 diagram families, plus 3 frontmatter-only inputs.
- Discovery: both executable and fixture bytes were independently verified. 19 SVG rows had identical output bytes; 57 SVG rows had output differences and were measured only as context.
- Same-output diagnostics: 19 rows, two AB/BA pairs, 30 Criterion samples, 2 s warmup and 3 s measurement. These are diagnostic, not confirmation.
- Changed-output context: 57 rows, two ordered pairs with the same recipe and pre/postflight checks. These values must not be used as equivalent-workload performance admission.

Raw JSON, logs, frozen executables and the source audits are preserved under `target/bench/experiments/tag-v0.9.0-and-binaryen-20261016/`.

## Cross-family results

Values are median normalized end-to-end estimates in microseconds. `matched` means byte-identical SVG; `changed` means the input was identical but the SVG identity changed.

| Fixture | Family | Output | v0.9.0 (µs) | cleanup (µs) | Head/base |
| --- | --- | --- | ---: | ---: | ---: |
| `agentflow_basic` | agentflow | changed | 354.30 | 196.58 | -44.5% |
| `architecture_medium` | architecture | matched | 45.04 | 53.52 | +18.8% |
| `block_medium` | block | changed | 22.78 | 37.30 | +63.7% |
| `c4_medium` | c4 | matched | 105.31 | 124.25 | +18.0% |
| `class_medium` | class | matched | 654.45 | 683.57 | +4.4% |
| `cynefin_medium` | cynefin | matched | 29.04 | 36.38 | +25.3% |
| `er_medium` | er | changed | 419.39 | 421.32 | +0.5% |
| `error_basic` | error | changed | 7.36 | 15.61 | +112.0% |
| `eventmodeling_medium` | eventmodeling | changed | 38.69 | 40.12 | +3.7% |
| `flowchart_medium` | flowchart | changed | 986.65 | 1182.15 | +19.8% |
| `gantt_medium` | gantt | matched | 35.96 | 49.89 | +38.7% |
| `gitgraph_medium` | gitgraph | changed | 40.03 | 54.77 | +36.8% |
| `info_medium` | info | changed | 7.53 | 14.43 | +91.6% |
| `ishikawa_medium` | ishikawa | changed | 43.92 | 51.27 | +16.7% |
| `journey_medium` | journey | matched | 27.16 | 36.25 | +33.5% |
| `kanban_medium` | kanban | matched | 44.65 | 79.01 | +77.0% |
| `mindmap_medium` | mindmap | changed | 249.81 | 281.50 | +12.7% |
| `packet_medium` | packet | matched | 8.08 | 15.00 | +85.7% |
| `pie_medium` | pie | matched | 25.41 | 34.13 | +34.4% |
| `quadrant_medium` | quadrantchart | matched | 26.38 | 33.51 | +27.0% |
| `radar_medium` | radar | changed | 20.71 | 33.96 | +64.0% |
| `railroad_abnf_medium` | railroadAbnf | changed | 53.04 | 55.58 | +4.8% |
| `railroad_ebnf_medium` | railroadEbnf | changed | 52.39 | 54.56 | +4.1% |
| `railroad_medium` | railroad | changed | 37.89 | 41.73 | +10.1% |
| `railroad_peg_medium` | railroadPeg | changed | 28.07 | 31.80 | +13.3% |
| `requirement_medium` | requirement | matched | 234.09 | 250.82 | +7.1% |
| `sankey_medium` | sankey | matched | 23.57 | 31.16 | +32.2% |
| `sequence_medium` | sequence | changed | 265.86 | 288.20 | +8.4% |
| `state_medium` | state | changed | 388.94 | 454.34 | +16.8% |
| `swimlane_medium` | swimlane | changed | 313.62 | 383.31 | +22.2% |
| `timeline_medium` | timeline | matched | 39.13 | 60.87 | +55.6% |
| `treemap_medium` | treemap | changed | 33.84 | 42.76 | +26.4% |
| `treeview_medium` | treeView | changed | 14.68 | 17.29 | +17.8% |
| `usecase_basic` | usecase | changed | 195.91 | 201.41 | +2.8% |
| `venn_medium` | venn | changed | 138.59 | 147.74 | +6.6% |
| `wardley_medium` | wardley | matched | 40.40 | 47.51 | +17.6% |
| `xychart_medium` | xychart | changed | 89.02 | 105.88 | +18.9% |
| `zenuml_medium` | zenuml | matched | 19.97 | 33.09 | +65.7% |

The broad matrix is intentionally not reduced to one average: diagram families have different parsers, layout engines, output sizes and theme owners. The most useful matched-output diagnostic movements were Kanban `44.65 → 79.01 µs` (+77.0%), Timeline `39.13 → 60.87 µs` (+55.6%), Packet `8.08 → 15.00 µs` (+85.7%), Journey `27.16 → 36.25 µs` (+33.5%), and Class `654.45 → 683.57 µs` (+4.4%).

Among changed-output context rows, Flowchart medium/large were `986.66 → 1182.15 µs` (+19.8%) and `3985.55 → 4684.30 µs` (+17.5%), Mindmap was `249.82 → 281.51 µs` (+12.7%), while ER was effectively flat at `419.39 → 421.32 µs` (+0.5%). Agentflow was faster in this range (`354.30 → 196.58 µs`, −44.5%), demonstrating why changed-output values are not a single regression score.

## Stage attribution and confirmation

Private stage diagnostics on byte-identical outputs measured render-only changes of Kanban `26.01 → 47.64 µs` (+83.2%), Packet `3.23 → 6.73 µs` (+108.4%), Timeline `18.14 → 31.97 µs` (+76.2%), and Class `229.24 → 251.09 µs` (+9.5%). Parse-only controls were Class `149.55 → 151.32 µs` (+1.2%) and Timeline `4.05 → 4.45 µs` (+10.0%). These isolate a substantial writer/finalization cost but are attribution evidence, not separate public-operation admission.

Kanban then passed a preregistered low-latency calibration (8 A/A pairs per side; stable) and 8-pair AB/BA confirmation. The baseline was `44.36 µs`, so the preregistered thresholds were 11.11% and 4.5 µs. The candidate measured `78.17 µs`: +76.23% (95% paired bootstrap interval +75.64% to +76.79%) and +33.81 µs (interval +33.63 to +33.99 µs). This is a confirmed release-range regression for the Kanban public operation. It does not by itself assign the cause to the theme refactor; the next owner candidates are recorded in the small-diagram hot-path audit.

## Remaining owner candidates

Source review found reachable, unquantified default-path work in State provenance path-history allocation, Architecture typography inventories and CSS receipts, ER per-cell dual-mode visual construction, Mindmap edge fallback preparation, Packet terminal/CSS receipts, the default environment fixed-font/policy construction, and Kanban CSS receipt copies. The detailed boundaries and required semantic controls are in `target/bench/experiments/tag-v0.9.0-and-binaryen-20261016/remaining-default-path-audit.md` and `small-diagram-hotpath-audit.md`.

## Binaryen packaging result

Binaryen 133 `wasm-opt -O` aborted in `constraint-analysis`; the nine-line upstream reproducer fails independently of Merman. The fix is committed in `b44a0f867` as `wasm-opt = ["-O", "--skip-pass=constraint-analysis"]` at the wasm-pack custom profile. Full and all-package Web builds completed, all input manifests and package provenance verified, and the 5-package smoke matrix plus DOM safety smoke passed. The workaround retains the other `-O` passes and should be removed once a Binaryen release containing [PR 9258](https://github.com/WebAssembly/binaryen/pull/9258) is available.

## Reproduction

```console
CARGO_BUILD_JOBS=8 CARGO_INCREMENTAL=0 python3 tools/bench/compare_self.py \
  --base-dir .worktrees/bench-v0.9.0-20261016 --head-dir . \
  --base-label v0.9.0 --head-label cleanup-9a297b72f \
  --suite cross_family --group end_to_end --preset long --discovery-only
```

The diagnostic and context commands, exact frozen executable hashes, discovery contracts and all raw observations are in the experiment directory named above.
