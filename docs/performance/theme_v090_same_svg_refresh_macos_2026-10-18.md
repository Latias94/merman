# v0.9.0 same-SVG E2E refresh — macOS, 2026-10-18

## Scope and method

This refresh compares the frozen native v0.9.0 pipeline executable with candidate
`fb2823f02809bc279fd0f4f58e7f45ea96d11cab` after the
[canonical root-outcome refactor](root_theme_outcomes_macos_2026-10-18.md). It measures all 19
previously identified exact-SVG-matching complete E2E scenarios across 15 logical families.
Class contributes five scenarios but counts as one logical family. Selection follows output
identity, not observed timing.

Each row uses the unchanged pipeline benchmark with a reused Engine, no compiled request theme,
deterministic measurement and the same fixture bytes. Two fixed AB/BA pairs produce four calls per
row, 76 calls total, with 30 Criterion samples, 2 s warm-up and 3 s measurement per call. Table
values are the median of the two per-executable point estimates. All preflight output identities
and postflight checks match; no row is dropped or retried according to its timing. This is an
uncalibrated diagnostic, with no A/A or decision-grade latency admission. Thirty internal Criterion
samples are not 30 independent base/head runs.

The release range includes different error/resource contracts and other implementation changes.
Identical successful SVG output does not establish equivalence for those contracts or isolate a
theme-only cause. The 418-cell current compiled-preset coverage lane has a different scope and
does not enter these release ratios.

## Complete selected results

Positive differences mean the candidate takes longer. The material diagnostic screen requires
both more than 10% and more than 1 us for an individual row; this screen identifies attribution
work and does not replace calibrated confirmation.

| E2E fixture | v0.9.0 (us) | Candidate (us) | Difference (us) | Change |
| --- | ---: | ---: | ---: | ---: |
| `class_tiny` | 125.4500 | 138.9300 | +13.4800 | +10.75% |
| `class_medium` | 658.2350 | 675.7650 | +17.5300 | +2.66% |
| `class_namespace_dense` | 402.5050 | 415.9800 | +13.4750 | +3.35% |
| `class_nested_namespaces` | 1584.5500 | 1602.9000 | +18.3500 | +1.16% |
| `class_nested_namespaces_large` | 15456.0000 | 15446.0000 | -10.0000 | -0.06% |
| `pie_medium` | 25.5635 | 28.9535 | +3.3900 | +13.26% |
| `journey_medium` | 27.2655 | 28.8095 | +1.5440 | +5.66% |
| `timeline_medium` | 39.8985 | 42.9370 | +3.0385 | +7.62% |
| `gantt_medium` | 36.5660 | 39.5310 | +2.9650 | +8.11% |
| `requirement_medium` | 234.6100 | 240.7400 | +6.1300 | +2.61% |
| `c4_medium` | 105.2150 | 115.1500 | +9.9350 | +9.44% |
| `sankey_medium` | 23.3815 | 26.1395 | +2.7580 | +11.80% |
| `quadrant_medium` | 26.3155 | 29.3780 | +3.0625 | +11.64% |
| `zenuml_medium` | 19.5855 | 23.3930 | +3.8075 | +19.44% |
| `packet_medium` | 7.9270 | 11.3225 | +3.3955 | +42.83% |
| `kanban_medium` | 44.4155 | 57.7245 | +13.3090 | +29.96% |
| `architecture_medium` | 44.6710 | 47.8305 | +3.1595 | +7.07% |
| `wardley_medium` | 39.5975 | 44.0770 | +4.4795 | +11.31% |
| `cynefin_medium` | 28.9845 | 30.9905 | +2.0060 | +6.92% |

Eight rows exceed both parts of the screen: Class tiny, Pie, Sankey, Quadrant, Zenuml, Packet,
Kanban and Wardley. The other 11 do not cross that joint screen in this diagnostic; they are not
therefore admitted as equivalent or recovered. There is no all-family recovery average. The
Zenuml estimate here is separate from the earlier root-outcome four-call tag context
(20.1670 → 23.5470 us, +16.76%) and the adjacent base/head lane; do not combine their estimates.

## Separate output lanes and next work

The complete current discovery has 76 renderable E2E fixtures. Comparing current identities with
the frozen tag discovery independently reproduces the 19 equal-output selections and 57 changed
SVG outputs. The changed-output group contains 28 Flowchart, seven Sequence, four Railroad, two
State and one each for ER, Info, Mindmap, Gitgraph, Block, Radar, Treemap, XY Chart, Venn, Swimlane,
Event Modeling, Treeview, Ishikawa, Error, AgentFlow and Usecase. Their identities and complete row
list remain recorded, but they receive no equivalent-output release ratio in this refresh. Retain
the source-backed changed-output audit; an unexplained difference remains unresolved.

The three `frontmatter_preprocess` fixtures are basic, indented and deep-config preprocessing
operations. They are separate from the 76 rendering E2E fixtures and from all ratios in the table.
This is not coverage of every scale, compiled theme, transport, error path or output capability.

Next attribute actual parse/layout/preparation/emission consumers in the eight flagged rows,
prioritizing Packet/Kanban's relative gaps and Class tiny's absolute gap alongside the other
families. Any candidate still needs fresh adjacent allocation/output controls and the registered
calibrated latency procedure before a latency claim. The all-family convergence goal remains active.

## Evidence receipt

Registration, the complete 76 calls, per-call logs and extracted estimates are under
`target/bench/experiments/all-family-v090-refresh-20261018/`; the source of the table is
`tag-diagnostic.json`. Fixture hashes and the 57 changed-output row names are in its
`experiment.yaml`. Compare frozen tag identities in
`target/bench/experiments/xml-fragment-dispatch-20261017/discovery.json` with current identities in
`target/bench/experiments/root-theme-outcomes-20261018/discovery.json`; those receipts also identify
the three preprocessing operations.

Frozen v0.9.0 pipeline SHA-256:
`5562fea88eb11457fdbf459c61076cd4011c476a0fc26828eb39701987b65f57`.
Candidate pipeline SHA-256:
`d1cad7c938b5d9057960f09dd30aa92d91b8eeb36d63c0cbf5d81177d7689be4`.
Both executable digests and the current source receipt are verified. Host: Apple M4 Pro/48 GiB,
macOS aarch64, Rust 1.95.0. Frozen executable calls run serially without concurrent Cargo,
allocation capture or profiling.
