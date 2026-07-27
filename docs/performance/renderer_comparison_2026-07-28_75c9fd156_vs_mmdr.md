# Current Merman and mmdr Comparison

This is the decision checkpoint after Requirement label preparation. It compares Merman's warm
native public SVG path with `mermaid-rs-renderer` (mmdr) on the same Apple M4 Pro host.

## Decision summary

Only byte-identical fixture inputs contribute ratios. Treemap and XYChart retain measured timings
but are excluded because the same-named fixtures differ; Info and `flowchart_large` are unavailable
in mmdr.

| Metric | Result |
| --- | ---: |
| Requested standard rows | 34 |
| Byte-identical, jointly measured rows | 30 |
| Merman faster / slower | 18 / 12 |
| Median Merman / mmdr ratio | 0.664x |
| Geometric-mean Merman / mmdr ratio | 0.297x |
| Rows above both 1.10x and 50 us | 2 |

The aggregate is not a quality-adjusted score. Complex Flowchart cases dominate the geometric
mean, while the two implementations differ in supported families, layout algorithms, validation,
sanitization, and SVG DOM output.

## Provenance

| Field | Value |
| --- | --- |
| Measured at | `2026-07-28 00:55:59 +0800` |
| Merman measurement commit | `75c9fd1560b67496a5cf0ec65f201f1d1f295f09` |
| Requirement optimization | Isolated patch `4264f2aad`; equivalent branch commit `8d45b8634` |
| mmdr commit | `7ff1196ed297c32a65a6b3cdc28f3ca3787fb65e` |
| Host | macOS 26.5.1, arm64, Apple M4 Pro |
| Toolchain | Rust 1.95.0, Python 3.14.6 |
| Protocol | 30 samples, 2 s warm-up, 3 s measurement, Criterion exact benches |
| Raw JSON | `target/bench/renderer_comparison_75c9fd156_vs_mmdr.json` (ignored local evidence) |
| Raw JSON SHA-256 | `7a4099daa933c964267367e27fc162dd1cdf47a4f95f1bde55e587f28928e000` |
| Requirement stage spot-check SHA-256 | `19652b77dffe34a9b3c155087f21c3b6bc466c7f5d7078fbb177c1cbafc5e4c5` |
| Comparable-input receipt SHA-256 | `536120273f095ad418f6f6fe347dbfa7ad70e96f2e857f84bfbe4bc9226159eb` |

The measurement includes the later source-map indexing and editor-coordinate cleanup commits.
`4264f2aad` is the isolated Requirement repair and `8d45b8634` is its equivalent branch commit.
The comparable-input receipt hashes the canonical sorted JSON array of fixture name, byte length,
and source SHA-256 for the 30 ratio rows.

## Remaining mmdr leads

| Fixture | Merman | mmdr | Ratio | Absolute gap | Interpretation |
| --- | ---: | ---: | ---: | ---: | --- |
| `requirement_medium` | 196.96 us | 71.08 us | 2.77x | +125.88 us | Actionable residual; layout and SVG construction remain slower. |
| `mindmap_medium` | 165.26 us | 74.06 us | 2.23x | +91.20 us | Different default layout algorithm; not a valid shortcut target. |
| `sequence_tiny` | 83.53 us | 42.89 us | 1.95x | +40.64 us | Tiny fixed-cost and richer-output candidate. |
| `c4_medium` | 100.69 us | 61.56 us | 1.64x | +39.13 us | Below the absolute gate; stage cause is not yet measured. |
| `class_tiny` | 52.42 us | 18.35 us | 2.86x | +34.08 us | Tiny fixed-cost and richer-output candidate. |
| `kanban_medium` | 57.66 us | 28.19 us | 2.05x | +29.47 us | Duplicate label work is visible in source, but not yet stage-attributed. |
| `architecture_medium` | 40.42 us | 18.28 us | 2.21x | +22.14 us | Different layout capability and algorithm. |
| `state_tiny` | 43.52 us | 27.68 us | 1.57x | +15.83 us | Below the absolute gate. |
| `timeline_medium` | 52.14 us | 39.11 us | 1.33x | +13.03 us | Below the absolute gate. |
| `gitgraph_medium` | 42.50 us | 29.88 us | 1.42x | +12.62 us | Below the absolute gate. |
| `block_medium` | 24.67 us | 15.06 us | 1.64x | +9.61 us | Below the absolute gate. |
| `radar_medium` | 20.78 us | 18.30 us | 1.14x | +2.48 us | Below the absolute gate. |

Only Requirement and Mindmap cross the current triage rule of both 10% and 50 microseconds. A
large ratio on a tiny absolute operation is retained as evidence, not automatically promoted to a
refactor.

## Representative Merman leads

| Fixture | Merman | mmdr | Merman / mmdr |
| --- | ---: | ---: | ---: |
| `flowchart_medium` | 3.52 ms | 101.33 ms | 0.035x |
| `flowchart_ports_heavy` | 1.11 ms | 1.24 s | 0.0009x |
| `flowchart_nested_clusters` | 1.29 ms | 132.84 ms | 0.0097x |
| `class_medium` | 900.36 us | 2.34 ms | 0.38x |
| `state_medium` | 467.95 us | 1.78 ms | 0.26x |
| `sequence_medium` | 199.33 us | 1.17 ms | 0.17x |
| `er_medium` | 447.85 us | 3.72 ms | 0.12x |
| `zenuml_medium` | 19.54 us | 131.56 us | 0.15x |

These results show workload scaling, not universal superiority. In particular, the Flowchart rows
dominate every aggregate that weights fixture ratios equally.

## Root-cause classification

### Requirement

Commit `8d45b8634` removed duplicate label measurement by carrying an operation-scoped private
prepared artifact from layout to SVG rendering. The three-run A/B reduced SVG emission by 60.59%
and end-to-end latency by 27.60%. The latest full-suite row is 196.96 us versus mmdr's 71.08 us.

The latest focused stage ratios are 0.72x parse, 2.81x layout, 3.85x SVG emission, and 2.81x
end-to-end. Merman
uses Dugong/Dagre geometry and emits Mermaid-compatible Markdown labels, strict sanitization,
`foreignObject`, full CSS/DOM metadata, `data-points`, and Rough-compatible paths. mmdr uses a
smaller generic layout/render path. The next valid work is to profile Dugong separately from path
and DOM-string construction, not bypass sanitization or reduce output semantics.

### Mindmap and Architecture

Merman's Mindmap default runs COSE-Bilkent. mmdr's current dispatch does not recognize its
configured `cose-bilkent` value and falls back to radial placement. Architecture similarly
compares Merman's compound FCoSE graph, constraints, groups, and iterative layout with mmdr's
deterministic BFS grid and orthogonal routing. These are capability and algorithm differences.
Replacing the defaults only to improve the benchmark would change the product.

### Kanban and tiny diagrams

Kanban still performs Markdown conversion, sanitization, and HTML measurement during layout and
again during SVG emission. That is a plausible semantics-preserving preparation opportunity, but
the 29.47 us end-to-end gap is below the ordinary gate and has no stage A/B yet.

Sequence, Class, and State tiny are slower while their medium counterparts are substantially
faster. This is consistent with fixed public-path or richer-DOM cost rather than a scaling
regression, but it is not stage-attributed.
Merman creates a render environment/session and completes a report on the string-only path; mmdr
reuses theme and layout configuration outside the timed loop. A reusable-environment lane and a
no-report terminal-path experiment are required before assigning a shared cause.

## Complete comparable-result receipt

These are all 30 byte-identical, jointly measured rows used for the aggregates above. Keeping the
complete medians in this report makes the counts, median, and geometric mean independently
recomputable without the ignored raw sample file.

| Fixture | Merman | mmdr | Merman / mmdr |
| --- | ---: | ---: | ---: |
| `flowchart_tiny` | 36.89 us | 56.78 us | 0.650x |
| `flowchart_small` | 288.86 us | 19.49 ms | 0.0148x |
| `flowchart_medium` | 3.52 ms | 101.33 ms | 0.0348x |
| `flowchart_ports_heavy` | 1.11 ms | 1.24 s | 0.0009x |
| `flowchart_weave` | 890.63 us | 522.64 ms | 0.0017x |
| `flowchart_nested_clusters` | 1.29 ms | 132.84 ms | 0.0097x |
| `flowchart_long_edge_labels` | 1.28 ms | 379.86 ms | 0.0034x |
| `class_tiny` | 52.42 us | 18.35 us | 2.857x |
| `class_medium` | 900.36 us | 2.34 ms | 0.384x |
| `state_tiny` | 43.52 us | 27.68 us | 1.572x |
| `state_medium` | 467.95 us | 1.78 ms | 0.262x |
| `sequence_tiny` | 83.53 us | 42.89 us | 1.948x |
| `sequence_medium` | 199.33 us | 1.17 ms | 0.170x |
| `er_medium` | 447.85 us | 3.72 ms | 0.121x |
| `pie_medium` | 23.94 us | 41.70 us | 0.574x |
| `mindmap_medium` | 165.26 us | 74.06 us | 2.231x |
| `journey_medium` | 26.17 us | 38.61 us | 0.678x |
| `timeline_medium` | 52.14 us | 39.11 us | 1.333x |
| `gantt_medium` | 35.16 us | 62.73 us | 0.561x |
| `requirement_medium` | 196.96 us | 71.08 us | 2.771x |
| `gitgraph_medium` | 42.50 us | 29.88 us | 1.422x |
| `c4_medium` | 100.69 us | 61.56 us | 1.636x |
| `sankey_medium` | 22.68 us | 26.00 us | 0.872x |
| `quadrant_medium` | 22.63 us | 31.88 us | 0.710x |
| `zenuml_medium` | 19.54 us | 131.56 us | 0.149x |
| `block_medium` | 24.67 us | 15.06 us | 1.638x |
| `packet_medium` | 6.79 us | 45.37 us | 0.150x |
| `kanban_medium` | 57.66 us | 28.19 us | 2.046x |
| `architecture_medium` | 40.42 us | 18.28 us | 2.211x |
| `radar_medium` | 20.78 us | 18.30 us | 1.136x |

## Comparability limits

- Merman's public one-shot helper performs strict render-model parsing and creates operation state.
  mmdr's benchmark reuses theme/config and uses its permissive parser.
- Both runners received identical source bytes for every ratio row, but they do not promise
  equivalent layouts, SVG bytes, DOM structure, sanitization, or Mermaid release parity.
- Timings include only successful renders. Missing and failed work reduces coverage instead of
  becoming an artificial latency sample.
- Native Merman, Node N-API, Node-WASM, browser-WASM, and Mermaid.js are separate lanes.

## Reproduction

```console
python3 tools/bench/compare_mermaid_renderers.py \
  --preset long \
  --suite standard \
  --skip-mermaid-js \
  --mmdr-dir repo-ref/mermaid-rs-renderer \
  --out target/bench/renderer_comparison_75c9fd156_vs_mmdr.md \
  --json-out target/bench/renderer_comparison_75c9fd156_vs_mmdr.json
```

The rejected pre-gate run remains local evidence only. It measured the same operations but allowed
different Treemap and XYChart fixture bytes into ratios, so none of its aggregates are used here.
