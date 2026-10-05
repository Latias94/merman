# Expanded native library rendering observation

Measured on 2026-10-05 (Asia/Shanghai). This extends the [initial native library observation](library_rendering_v070_to_01cc4562f_2026-10-05.md) to the remaining fixtures in `tools/bench/corpus.json`. The target is reused-engine `merman` crate SVG rendering, not CLI process startup, argument parsing, file output, or network icon fetching.

**Status: completed descriptive observation.** All 72 registered SVG fixtures have an accepted current-version observation. 59 have an accepted old/new pair (including two configuration controls and one error-diagram fixture); 48 have a lower paired median and 11 a higher one. Nine historical inputs are unsupported and four fail historical output identity. Three preprocessing controls are separate. 6 paired medians exceed the exploratory joint >10% AND >50-us gate; two pairs do not confirm a regression. No production renderer change is part of this work. See the [machine-readable receipt](baselines/library-v070-to-01cc4562f-expanded-2026-10-05.json).

## Scope and comparison contract

The registered corpus contains 72 SVG fixtures, including two explicit Dagre/classic controls and an intentional error-diagram fixture, plus three frontmatter preprocessing controls. Nine accepted SVG observations are reused from the initial report with their original executable and sample identities. The remaining 63 SVG fixtures and three preprocessing controls receive two independent current-version observations. Historical timings are admitted only when the exact input succeeds and output identity remains stable.

The product revisions remain v0.7.0 (`00a2f291025913d612b6ca1ce9b6df6daf5bef94`) and merged PR #165 (`01cc4562f74abaa1c7d31a44881db69296cacb3f`). The expanded benchmark source commits are `834d6a331988a481f528be6f9044a2557d23dc5a` for the historical adapter and `bfcc2db686c0e9799f792058c8d9367f7082d25b` for the current harness. The historical adapter receives benchmark-only fixture backports; production sources and both original Cargo.lock files are unchanged. Current AgentFlow and Usecase registrations are restored to match the existing corpus.

Both executables use Rust 1.95.0, the optimized `bench` profile, and x86_64 Windows MSVC. Base features are `--no-default-features --features render`; current features are `--no-default-features --features all-diagrams,svg,layout-cytoscape,layout-elk,math`. Builds are serial with one Cargo job and use the shared target directory. Frozen executables are hashed before invocation. Each admitted operation has two independent preflights, exact pre/post output controls, 30 internal Criterion samples, a 2-second warmup, and 3-second measurement. The pair order is base/head followed by head/base; current-only lanes run twice. No compilation runs alongside the timed campaign.

These are descriptive release-product comparisons. Old/new layout defaults, themes, text policy, APIs, SVG output, and Criterion versions differ. Byte-identical input does not imply byte-identical work. Two A/B pairs identify follow-up candidates; they are not statistically powered confirmation. An unsupported parser or unstable output is excluded, never recorded as zero time. The existing strict same-output comparator and its acceptance checks remain unchanged.

## Newly supported and excluded historical inputs

The local upstream Mermaid changelog places AgentFlow (`agentflow-beta`, PR #8073) and Usecase (PR #8048) in **Mermaid 12.0.0**. They are current-version baselines, not regressions against an implementation that predates them. The source is `repo-ref/mermaid/packages/mermaid/CHANGELOG.md`, under `## 12.0.0`; its source revision and digest accompany the receipt.

Historical preflight rejects nine fixture inputs with `Parse(DetectType(...))`: Swimlane, four Railroad dialect fixtures, Wardley, Cynefin, AgentFlow, and Usecase. This reports the measured v0.7.0 recipe's actual support; it does not infer support from a fixture's creation date. Eventmodeling, Treeview, Ishikawa, and Venn do pass the historical preflight.

Four additional historical fixtures are excluded for output identity failures: `state_tiny` and `state_medium` differ between processes; `requirement_medium` and `gitgraph_medium` fail their pre/post SVG equality checks within a process. These are evidence blockers, not proof of visible visual defects. The initial report's current `parse/state_medium` compatibility-projection instability remains a separate open issue; it does not imply that the current complete SVG operation fails.

## Results

All 72 registered SVG fixtures have an accepted current-version observation. 59 have an accepted old/new pair (including two configuration controls and one error-diagram fixture); 48 have a lower paired median and 11 a higher one. Nine historical inputs are unsupported and four fail historical output identity. Three preprocessing controls are separate. 6 paired medians exceed the exploratory joint >10% AND >50-us gate; two pairs do not confirm a regression.

Times are microseconds; each column is the median of independent estimates. Change and delta use paired ratios/differences, which need not equal ratios/differences of the displayed medians. `Mixed` means the two rounds disagree on direction. `Prior` retains the earlier receipt (eight pairs for Mindmap/Sequence, two for other prior rows); new rows have two pairs. Different rows were not measured simultaneously. Neither corpus counts nor an average across families describe a user's workload.

### Comparable complete library operations

| Fixture | v0.7.0 us | Current us | Paired change | Paired delta us | Rounds / origin |
| --- | ---: | ---: | ---: | ---: | --- |
| `flowchart_nested_clusters_dagre_classic` | 2625.40 | 1781.95 | -32.1% | -843.45 | 2 / Prior |
| `flowchart_medium_dagre_classic` | 4407.60 | 3004.90 | -31.8% | -1402.70 | 2 / Prior |
| `flowchart_tiny` | 389.13 | 432.04 | +11.0% | +42.90 | 2 / New |
| `flowchart_small` | 685.53 | 437.02 | -36.2% | -248.50 | 2 / Prior |
| `flowchart_medium` | 4640.45 | 1677.75 | -63.8% | -2962.70 | 2 / Prior |
| `flowchart_large` | 51935.00 | 11459.00 | -77.9% | -40476.00 | 2 / New |
| `flowchart_ports_heavy` | 3423.95 | 4952.40 | +44.6% | +1528.45 | 2 / New |
| `flowchart_weave` | 3720.25 | 6402.40 | +72.1% | +2682.15 | 2 / New |
| `flowchart_backedges_subgraphs` | 2717.70 | 3076.60 | +20.3% | +358.90 | 2 / New; mixed |
| `flowchart_sparse_components` | 2783.60 | 1397.90 | -49.8% | -1385.70 | 2 / New |
| `flowchart_lanes_crossfeed` | 3313.55 | 2015.00 | -39.2% | -1298.55 | 2 / New |
| `flowchart_grid_feedback` | 5144.00 | 5307.70 | +3.2% | +163.70 | 2 / New |
| `flowchart_fanout_returns` | 2419.30 | 3441.90 | +42.3% | +1022.60 | 2 / New |
| `flowchart_label_collision` | 1438.65 | 1393.90 | -3.1% | -44.75 | 2 / New |
| `flowchart_nested_clusters` | 2693.25 | 1279.95 | -52.5% | -1413.30 | 2 / Prior |
| `flowchart_elk_nested_directions` | 1654.80 | 825.22 | -50.1% | -829.58 | 2 / New |
| `flowchart_asymmetric_components` | 2996.25 | 1379.30 | -54.0% | -1616.95 | 2 / New |
| `flowchart_parallel_merges` | 2228.90 | 2762.80 | +24.0% | +533.90 | 2 / New |
| `flowchart_long_edge_labels` | 3194.90 | 4153.25 | +32.1% | +958.35 | 2 / New |
| `flowchart_svg_label_reuse` | 14854.50 | 5724.20 | -60.6% | -9130.30 | 2 / New |
| `flowchart_selfloop_bidi` | 2451.40 | 2688.50 | +8.3% | +237.10 | 2 / New |
| `flowchart_component_packing` | 2802.10 | 2229.60 | -20.5% | -572.50 | 2 / New |
| `flowchart_direction_conflict` | 2864.05 | 1842.05 | -34.4% | -1022.00 | 2 / New |
| `flowchart_parallel_label_stack` | 1274.25 | 1246.73 | -2.9% | -27.52 | 2 / New; mixed |
| `class_tiny` | 344.23 | 334.87 | -0.3% | -9.36 | 2 / New; mixed |
| `class_medium` | 1302.80 | 1019.25 | -21.7% | -283.55 | 2 / Prior |
| `class_namespace_dense` | 1764.65 | 1110.76 | -36.0% | -653.89 | 2 / New |
| `class_nested_namespaces` | 5928.75 | 4086.05 | -30.2% | -1842.70 | 2 / New |
| `class_nested_namespaces_large` | 96911.00 | 30491.50 | -68.6% | -66419.50 | 2 / New |
| `sequence_tiny` | 407.50 | 426.08 | +10.5% | +18.59 | 2 / New; mixed |
| `sequence_medium` | 460.12 | 419.88 | -8.6% | -39.55 | 8 / Prior |
| `sequence_actor_only_control` | 421.89 | 323.94 | -23.0% | -97.95 | 2 / New |
| `sequence_block_repeat_low` | 507.22 | 458.38 | -8.7% | -48.84 | 2 / New |
| `sequence_block_repeat_medium` | 1020.85 | 829.04 | -19.2% | -191.81 | 2 / New |
| `sequence_block_repeat_high` | 3515.60 | 3321.20 | -5.2% | -194.40 | 2 / New; mixed |
| `sequence_block_unique_high` | 3877.35 | 3217.95 | -16.9% | -659.40 | 2 / New |
| `er_medium` | 957.47 | 947.42 | -0.9% | -10.04 | 2 / New; mixed |
| `info_medium` | 196.43 | 17.20 | -91.3% | -179.23 | 2 / New |
| `pie_medium` | 252.01 | 53.20 | -78.8% | -198.81 | 2 / New |
| `mindmap_medium` | 352.76 | 389.82 | +11.1% | +38.83 | 8 / Prior |
| `journey_medium` | 265.19 | 43.27 | -83.1% | -221.92 | 2 / New |
| `timeline_medium` | 242.01 | 55.77 | -76.8% | -186.24 | 2 / New |
| `gantt_medium` | 198.96 | 51.82 | -74.0% | -147.14 | 2 / Prior |
| `c4_medium` | 292.01 | 173.26 | -40.7% | -118.75 | 2 / New |
| `sankey_medium` | 171.93 | 33.23 | -80.7% | -138.69 | 2 / New |
| `quadrant_medium` | 245.59 | 50.48 | -79.7% | -195.11 | 2 / New |
| `zenuml_medium` | 495.33 | 46.24 | -90.9% | -449.10 | 2 / New |
| `block_medium` | 261.98 | 62.40 | -76.1% | -199.58 | 2 / New |
| `packet_medium` | 171.06 | 18.78 | -88.6% | -152.28 | 2 / New |
| `kanban_medium` | 283.36 | 93.56 | -67.0% | -189.80 | 2 / New |
| `architecture_medium` | 396.02 | 107.74 | -73.2% | -288.29 | 2 / New |
| `radar_medium` | 238.74 | 45.35 | -81.1% | -193.39 | 2 / New |
| `treemap_medium` | 300.99 | 76.39 | -75.2% | -224.60 | 2 / New |
| `xychart_medium` | 428.17 | 192.70 | -54.7% | -235.47 | 2 / New |
| `venn_medium` | 358.25 | 330.19 | -8.4% | -28.05 | 2 / New |
| `eventmodeling_medium` | 317.02 | 84.17 | -73.9% | -232.85 | 2 / New |
| `treeview_medium` | 221.01 | 33.63 | -85.0% | -187.39 | 2 / New |
| `ishikawa_medium` | 340.30 | 90.69 | -73.3% | -249.61 | 2 / New |
| `error_basic` | 200.61 | 16.65 | -91.7% | -183.96 | 2 / New |

### Current baselines: unsupported historical inputs

| Fixture | Current median us | Independent estimates us |
| --- | ---: | --- |
| `swimlane_medium` | 858.97 | 1019.90, 698.03 |
| `railroad_medium` | 81.15 | 104.44, 57.86 |
| `railroad_abnf_medium` | 101.21 | 130.01, 72.41 |
| `railroad_ebnf_medium` | 103.02 | 130.95, 75.09 |
| `railroad_peg_medium` | 49.43 | 58.66, 40.20 |
| `wardley_medium` | 58.84 | 54.48, 63.20 |
| `cynefin_medium` | 44.89 | 48.37, 41.41 |
| `agentflow_basic` | 871.43 | 1064.70, 678.16 |
| `usecase_basic` | 530.28 | 656.42, 404.15 |

### Current baselines: historical identity blockers

| Fixture | Current median us | Independent estimates us |
| --- | ---: | --- |
| `state_tiny` | 320.58 | 383.06, 258.11 |
| `state_medium` | 1003.11 | 1155.90, 850.32 |
| `requirement_medium` | 475.88 | 424.77, 526.99 |
| `gitgraph_medium` | 58.38 | 54.85, 61.91 |

### Preprocessing controls (not rendering)

| Fixture | Current median us |
| --- | ---: |
| `frontmatter_basic` | 13.28 |
| `frontmatter_indented` | 14.46 |
| `frontmatter_deep_config` | 22.61 |

### Bounded stage localization

The four largest absolute paired increases from the newly comparable rows select this follow-up. Ratios list both rounds, not a confidence interval. Their ordering does not override the different ownership, defaults, and output contracts.

| Fixture | Parse head/base | Layout head/base | SVG emission head/base |
| --- | --- | --- | --- |
| `flowchart_weave` | 1.07x / 1.03x | 1.51x / 1.49x | 4.57x / 4.77x |
| `flowchart_ports_heavy` | 1.04x / 1.03x | 1.33x / 1.38x | 2.56x / 2.63x |
| `flowchart_fanout_returns` | 0.96x / 0.93x | 1.27x / 1.12x | 2.70x / 3.01x |
| `flowchart_long_edge_labels` | 0.92x / 1.06x | 1.03x / 1.06x | 2.55x / 2.52x |

The first three fixtures show higher layout cost in both rounds; `long_edge_labels` has only a modest layout increase. SVG emission is higher for all four, while parse is close to unchanged or inconsistent. These patterns prioritize profiling layout and SVG preparation/serialization; they do not prove a common implementation cause. The SVG payload grows even though element counts are equal or lower:

| Fixture | SVG bytes, base/current | Elements, base/current |
| --- | ---: | ---: |
| `flowchart_weave` | 42,511 / 66,447 | 318 / 238 |
| `flowchart_ports_heavy` | 59,172 / 71,337 | 423 / 263 |
| `flowchart_fanout_returns` | 42,344 / 64,951 | 296 / 196 |
| `flowchart_long_edge_labels` | 33,034 / 63,648 | 222 / 222 |

Output growth, backend selection, preparation, and destruction boundaries are competing explanations to separate next. Source-code volume by itself is not an attribution result. The earlier medium/nested Dagre controls do not settle these different stress fixtures; reproduce the four named inputs with explicit common backend/theme/text settings before assigning their slowdown to a refactor.

## Follow-up register

| ID | Priority | Observation and handoff |
| --- | --- | --- |
| LIBPERF-01 | P1 | Retain the initial eight-pair Mindmap observation (+11.1%, +38.83 us). Profile its parse/configuration work under a fixed operation contract; this campaign reuses those measurements. |
| LIBPERF-02 | P2 | Retain private SVG-emission signals. Align artifact ownership/drop boundaries before treating them as implementation regressions. |
| LIBPERF-03 | P1 evidence blocker | Retain State compatibility-projection/SVG identity issues; add historical Requirement and Gitgraph pre/post instability. Reproduce and determine whether differences are semantic before any normalization. Current complete SVG lanes pass this campaign. |
| LIBPERF-04 | Resolved | AgentFlow and Usecase are registered in all six expected groups. The frozen current executable passes the complete compiled-list/preflight contract. |
| LIBPERF-05 | Native corpus complete | Every current registered native SVG fixture is observed. Memory, other input scales, cold-engine cost, Node, browser/WASM, and mobile remain unmeasured. New-only families have no historical ratio. |
| LIBPERF-06 | P1 diagnostic candidates | `flowchart_weave` +72.1% (+2682.1 us; slower); `flowchart_ports_heavy` +44.6% (+1528.5 us; slower); `flowchart_fanout_returns` +42.3% (+1022.6 us; slower); `flowchart_long_edge_labels` +32.1% (+958.4 us; slower); `flowchart_parallel_merges` +24.0% (+533.9 us; slower); `flowchart_backedges_subgraphs` +20.3% (+358.9 us; mixed). Review output/default differences and collect pinned-runner A/A plus fresh alternating confirmation before bisecting alpha checkpoints. Use the bounded stage observations to select profiles. |
| LIBPERF-07 | P2 smaller signals | `flowchart_selfloop_bidi` +8.3% (+237.1 us; slower); `flowchart_grid_feedback` +3.2% (+163.7 us; slower); `flowchart_tiny` +11.0% (+42.9 us; slower); `sequence_tiny` +10.5% (+18.6 us; mixed). Preserve these observations; a below-gate result is not a regression pass. |

No rendering fixes or causal claims are made. The follow-up can be delegated later without rerunning discovery: start from the named fixture, exact source/recipe, individual pair, raw output and stage result in the receipt. Shared host speed changed between rounds, and some pairs reverse direction. Adjacent pairing helps but does not remove that limitation.

## Reproduction and validation

The local experiment is `target/bench/experiments/stable-library-expanded-2026-10-05/` in the main workspace. It contains the preregistration, expanded benchmark-only patches, build receipts, frozen executables, source backport inventory, exact commands, output SVGs, every Criterion sample, failed attempts, and a resumable bounded runner. The companion JSON retains independent paired estimates and a hashed artifact inventory; the initial nine rows keep their original receipt provenance.

Private parse/layout/SVG-emission timings, where collected, only locate later investigation. Current SVG emission consumes an owned prepared artifact through batched setup; historical emission borrows a persistent layout. Those ownership/destruction boundaries differ, and independently timed stages must not be added together. No alpha bisect, causal profile, full DOM/image parity matrix, memory, browser/WASM, Node, or mobile performance claim is made here.

Both locked benchmark builds passed. The frozen current executable exposes 435 expected selectors with 435 preflight receipts across 7 groups. All 217 Python owner-contract tests passed (121 performance, 16 baseline-manifest, 51 native-memory, 29 native-memory-driver). Both benchmark checkouts pass `cargo fmt --all -- --check` and `git diff --check`. The expanded campaign retains 280 valid timed invocations, each with 30 raw internal samples; reused observations remain in the original receipt. All failed attempts are retained. Full Rust production tests were not rerun because production sources and manifests were not changed.
