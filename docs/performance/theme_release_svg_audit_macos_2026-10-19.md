# Release SVG evidence audit — macOS, October 19, 2026

All seventy-six native public rendering fixtures now have actual paired v0.9.0/current SVG files.
Every file matches its previously frozen side-specific SHA-256, byte count and XML element count;
independent replay reproduces every byte. The matrix still contains nineteen exact-SVG rows and
fifty-seven changed-SVG rows. No changed row was promoted to the strict same-output timing lane.

This checkpoint repairs an evidence gap, rather than claiming a new speedup or release convergence.
The previous source audit covered only twenty-three cross-family changed scenes and had no paired
raw files. The new inventory includes every stress/variant scene and all 35 logical families
(38 corpus family identifiers because four Railroad syntaxes share one logical family).

## Capture contract

Tag production is `f754a3c3077d35d2a372c2fec04f9e9b87db1c3f`; current production is `f40f1c5f7`,
with documentation-only checkpoint `c3ba56c0e`. Their original locked dependency recipes, public
`Renderer.render` request, fixture bytes, strict parse, deterministic measurement, diagram ID and
trusted-native resource policy remain unchanged. Both enable default features plus
`all-diagrams,svg,layout-cytoscape,layout-elk,math`, built serially with Rust 1.95.0.

The original byte-identical pipeline harness receives the same eleven-line, environment-enabled
capture hook on both sides. It writes `svg.svg()` in the existing end-to-end preflight closure.
It does not change the timed loop. Capture programs run only `--bench --list --format terse`;
they are never used for latency. Because this command skips exact-selector postflight, each
frozen capture executable is independently replayed into the same directory. The hook compares
existing bytes, and all identities are checked again.

Both capture-only commits remain isolated: `db9cd6b7c` on tag and `383d1336b` on current. Removing
only the identical benchmark change recovers the respective original production trees. Neither
capture branch is integrated. Frozen timing executables remain intact. Three preprocessing-only
fixtures are kept separate; no missing SVG is measured as a faster render.

The ignored evidence directory is `target/bench/experiments/theme-release-raw-svg-20261019/`:
`experiment.yaml`, `registered-corpus.json`, `execution.json`, `replay-execution.json`, `pairs.json`,
the `base/` and `head/` raw SVG directories, capture logs and four scoped analysis JSON/Markdown
pairs. Independent review rechecks all 152 files and the two benchmark-only tree differences.

## Every changed rendering fixture

The following is a difference inventory. A localized difference does not prove its implementation
cost is necessary, nor authorize broad comparator normalization. DOM correspondence is used only
to explain output order separately from geometry, with original order/IDs/styles retained.

| Fixture | Bytes: tag → current | Observed changes / remaining attribution |
| --- | ---: | --- |
| `flowchart_nested_clusters_dagre_classic` | 40,064 → 42,118 | Owned IDs, references, semantic metadata; recorded node/cluster order. Matched node/cluster/edge/label geometry agrees. |
| `flowchart_medium_dagre_classic` | 80,726 → 85,486 | Owned IDs, references, semantic metadata; recorded node/cluster order. Matched node/cluster/edge/label geometry agrees. |
| `flowchart_tiny` | 31,396 → 32,082 | Owned IDs, references, semantic metadata; recorded node/cluster order. Matched node/cluster/edge/label geometry agrees. |
| `flowchart_small` | 39,610 → 41,053 | Owned IDs, references, semantic metadata; recorded node/cluster order. Matched node/cluster/edge/label geometry agrees. |
| `flowchart_medium` | 77,505 → 82,430 | Owned IDs, references, semantic metadata; recorded node/cluster order. Matched node/cluster/edge/label geometry agrees. |
| `flowchart_large` | 240,109 → 259,951 | Owned IDs, references, semantic metadata; recorded node/cluster order. Matched node/cluster/edge/label geometry agrees. |
| `flowchart_ports_heavy` | 71,312 → 74,529 | Owned IDs, references, semantic metadata; recorded node/cluster order. Matched node/cluster/edge/label geometry agrees. |
| `flowchart_weave` | 66,422 → 69,215 | Owned IDs, references, semantic metadata; recorded node/cluster order. Real edge/path/label geometry remains to be attributed. |
| `flowchart_backedges_subgraphs` | 60,359 → 62,536 | Owned IDs, references, semantic metadata; recorded node/cluster order. Matched node/cluster/edge/label geometry agrees. |
| `flowchart_sparse_components` | 49,623 → 51,308 | Owned IDs, references, semantic metadata; recorded node/cluster order. Matched node/cluster/edge/label geometry agrees. |
| `flowchart_lanes_crossfeed` | 58,651 → 61,313 | Owned IDs, references, semantic metadata; recorded node/cluster order. Real edge/path/label geometry remains to be attributed. |
| `flowchart_grid_feedback` | 68,768 → 71,477 | Owned IDs, references, semantic metadata; recorded node/cluster order. Matched node/cluster/edge/label geometry agrees. |
| `flowchart_fanout_returns` | 64,926 → 67,433 | Owned IDs, references, semantic metadata; recorded node/cluster order. Real edge/path/label geometry remains to be attributed. |
| `flowchart_label_collision` | 56,964 → 58,599 | Owned IDs, references, semantic metadata; recorded node/cluster order. Matched node/cluster/edge/label geometry agrees. |
| `flowchart_nested_clusters` | 59,407 → 61,626 | Owned IDs, references, semantic metadata; recorded node/cluster order. Matched node/cluster/edge/label geometry agrees. |
| `flowchart_elk_nested_directions` | 41,902 → 43,158 | Owned IDs, references, semantic metadata; recorded node/cluster order. Matched node/cluster/edge/label geometry agrees. |
| `flowchart_asymmetric_components` | 52,029 → 53,746 | Owned IDs, references, semantic metadata; recorded node/cluster order. Matched node/cluster/edge/label geometry agrees. |
| `flowchart_parallel_merges` | 56,460 → 58,797 | Owned IDs, references, semantic metadata; recorded node/cluster order. Matched node/cluster/edge/label geometry agrees. |
| `flowchart_long_edge_labels` | 63,623 → 65,935 | Owned IDs, references, semantic metadata; recorded node/cluster order. Real edge/path/label geometry remains to be attributed. |
| `flowchart_svg_label_reuse` | 187,748 → 191,689 | Owned IDs, references, semantic metadata; recorded node/cluster order. Matched node/cluster/edge/label geometry agrees. |
| `flowchart_selfloop_bidi` | 59,681 → 61,472 | Owned IDs, references, semantic metadata; recorded node/cluster order. Matched node/cluster/edge/label geometry agrees. |
| `flowchart_component_packing` | 55,715 → 58,164 | Owned IDs, references, semantic metadata; recorded node/cluster order. Real edge/path/label geometry remains to be attributed. |
| `flowchart_direction_conflict` | 58,007 → 59,892 | Owned IDs, references, semantic metadata; recorded node/cluster order. Matched node/cluster/edge/label geometry agrees. |
| `flowchart_parallel_label_stack` | 53,071 → 54,504 | Owned IDs, references, semantic metadata; recorded node/cluster order. Matched node/cluster/edge/label geometry agrees. |
| `state_tiny` | 17,329 → 17,330 | Start-circle style gains a leading semicolon; no geometry change. |
| `state_medium` | 49,447 → 49,448 | Start-circle style gains a leading semicolon; no geometry change. |
| `sequence_tiny` | 22,101 → 22,556 | Only stylesheet changes: scoped filter references, native marker selectors and final paint terminals; other raw bytes agree. |
| `sequence_medium` | 33,713 → 34,208 | Only stylesheet changes: scoped filter references, native marker selectors and final paint terminals; other raw bytes agree. |
| `sequence_actor_only_control` | 22,567 → 23,302 | Only stylesheet changes: scoped filter references, native marker selectors and final paint terminals; other raw bytes agree. |
| `sequence_block_repeat_low` | 23,672 → 24,367 | Only stylesheet changes: scoped filter references, native marker selectors and final paint terminals; other raw bytes agree. |
| `sequence_block_repeat_medium` | 36,966 → 37,721 | Only stylesheet changes: scoped filter references, native marker selectors and final paint terminals; other raw bytes agree. |
| `sequence_block_repeat_high` | 131,594 → 132,309 | Only stylesheet changes: scoped filter references, native marker selectors and final paint terminals; other raw bytes agree. |
| `sequence_block_unique_high` | 131,594 → 132,309 | Only stylesheet changes: scoped filter references, native marker selectors and final paint terminals; other raw bytes agree. |
| `er_medium` | 63,431 → 63,431 | Marker-circle attribute order changes; values and DOM otherwise agree. |
| `info_medium` | 2,518 → 2,518 | Visible pinned Mermaid version changes from 12.1.0 to 12.2.0. |
| `mindmap_medium` | 37,560 → 38,537 | HTML color rules and root circle enclosure formula change; downstream geometry attribution remains partial. |
| `gitgraph_medium` | 10,682 → 10,695 | Concrete prepared font replaces a CSS variable. |
| `block_medium` | 8,689 → 8,693 | Root width/viewBox and three rectangle bounds change. |
| `radar_medium` | 6,889 → 6,899 | Title CSS gains SVG fill beside HTML color. |
| `treemap_medium` | 9,584 → 9,603 | Leaf CSSOM spelling and equivalent color serialization change. |
| `xychart_medium` | 12,841 → 13,056 | Five Neo rules gain configured drop shadows; no font or geometry difference in this fixture. |
| `venn_medium` | 2,229 → 2,221 | Four font stacks change from Trebuchet to Recursive Variable. |
| `swimlane_medium` | 40,716 → 41,215 | Owned IDs/metadata, lane selector and real label/layout dimensions change; source decomposition remains partial. |
| `eventmodeling_medium` | 4,542 → 4,596 | Configured font size and inherited root paint change. |
| `treeview_medium` | 2,158 → 2,266 | Explicit font inheritance is added. |
| `ishikawa_medium` | 9,807 → 9,805 | Font stack/semicolon spelling and unit-bearing size change. |
| `railroad_medium` | 4,293 → 4,292 | Root selector becomes a matching compound ID/class selector. |
| `railroad_abnf_medium` | 4,506 → 4,505 | Same root selector correction, independently verified. |
| `railroad_ebnf_medium` | 4,607 → 4,606 | Same root selector correction, independently verified. |
| `railroad_peg_medium` | 3,715 → 3,714 | Same root selector correction, independently verified. |
| `error_basic` | 4,236 → 4,236 | Visible pinned Mermaid version changes from 12.1.0 to 12.2.0. |
| `agentflow_basic` | 15,610 → 16,405 | Owned document IDs/references and semantic metadata; geometry and text agree. |
| `usecase_basic` | 19,178 → 18,068 | 222 quote entities become literal quotes inside CSS; decoded DOM agrees. |
| `flowchart_weave_dagre_classic` | 46,862 → 49,194 | Owned IDs, references, semantic metadata; recorded node/cluster order. Matched node/cluster/edge/label geometry agrees. |
| `flowchart_ports_heavy_dagre_classic` | 64,239 → 67,291 | Owned IDs, references, semantic metadata; recorded node/cluster order. Matched node/cluster/edge/label geometry agrees. |
| `flowchart_fanout_returns_dagre_classic` | 46,639 → 48,785 | Owned IDs, references, semantic metadata; recorded node/cluster order. Matched node/cluster/edge/label geometry agrees. |
| `flowchart_long_edge_labels_dagre_classic` | 36,563 → 38,213 | Owned IDs, references, semantic metadata; recorded node/cluster order. Matched node/cluster/edge/label geometry agrees. |

The nineteen exact rows are five Class fixtures plus Pie, Journey, Timeline, Gantt, Requirement,
C4, Sankey, Quadrant, Zenuml, Packet, Kanban, Architecture, Wardley and Cynefin. Their identities
remain available for formal release timing; this audit adds no timing observations.

## Findings and limits

- Flowchart: every semantic node/cluster correspondence is unique and complete across all 28
  rows. Their matched subtrees retain text, geometry and styles; generated IDs/data attributes
  and output order differ. All stylesheet differences are exact owned-filter references
  (`201212e6a`, `5fe6ce92d`). Five fixtures retain real edge/label geometry changes: Weave, Lanes
  Crossfeed, Fanout Returns, Long Edge Labels and Component Packing. Those source causes remain open.
- Sequence: each of seven complete raw edits independently reconstructs current output; all
  non-style bytes agree. Native-compatible exact marker selectors (`2069e5d85`), terminal paint
  ownership (`e6d7de3d4`) and scoped filters (`aeb9fe2f3`) explain the stylesheet changes. Not every
  selector has a consumer in every fixture, so this does not prove all emitted CSS work necessary.
- Usecase: exactly 222 `&quot;` sequences become literal quotes inside XML character data, saving
  1,110 bytes (`369c4bd3b`). Decoded stylesheet and complete DOM agree, while raw SVG remains different.
- XYChart: the full 215-byte change is five configured drop-shadow rules, with no font/geometry
  delta in this input. The older typography explanation was too broad. Those selectors have no
  matching elements in this scene; removal requires a separate stylesheet contract decision.
- State, ER, Block, Treemap, Venn and Ishikawa: all seven rows are individually reconstructed
  from twenty exact raw substitutions, with no unexplained bytes. State's semicolon and ER's
  attribute ordering are serialization differences. Block's padding/stroke viewport, Venn's font
  precedence and Ishikawa's unit-bearing size are observable behavior. Treemap's CSSOM spelling
  follows an actual fitting mutation; equal before/after font size alone would not prove that
  the mutation can be skipped.
- Mindmap: root radius now encloses the label diagonal (`e82da0fa8`). Remaining downstream
  coordinates are recorded, but their exclusive attribution is unproven. Swimlane has a source-
  backed 120-pixel minimum label width plus Neo padding; other geometry still needs decomposition.

The remaining seven geometry cases stay unresolved. Equivalent XML/CSS spelling, new native
selectors, ID collision prevention, upstream layout corrections and configured ownership must
not be grouped into one supposed theme regression or erased to produce a same-output result.

## Next optimization experiments

The consumer audit identifies separate, source-backed ownership candidates:

1. Sequence creates empty declaration groups and surface vectors before unconditionally dropping
   them. Skip an already empty declaration before grouping, keeping all nonempty ordering.
2. Shared Info/Error/Quadrant/XY CSS success facts copy the same prepared font three times. Borrow
   the immutable owner after successful writes, preserving the current receipt values and error order.
3. Treemap title/typography success facts copy four already owned scalar strings even without
   receipt consumers. Borrow those values; keep typed receipt retention as a separate experiment.

Each candidate still needs preregistered exact allocation/output/report/measurement/resource
evidence and owner checks. The empty math lease is another separate candidate, with a corrected
premise: `Arc<[T]>::default` uses static empty storage on Rust 1.95. Only reachable `Vec → Arc`
empty conversions allocate; default Packet/Kanban leases do not account for two allocations.

Full new typed presets remain a separate function contract because v0.9.0 lacks that API. The
418 adjacent preset cells do not substitute for historical legacy-function timing. Exact common
legacy configuration, a strictly proven exported Mermaid subset, and complete new preset behavior
need distinct evidence. The all-family convergence goal remains active.
