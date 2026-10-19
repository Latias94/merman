# Empty GitGraph palette CSS — macOS, October 20, 2026

The GitGraph palette CSS producer now returns its existing empty result immediately when all
eight palette assignments are absent. It retains the caller's empty output write and checkpoint.
The default primary saves one allocation and 16 bytes; the catalog's no-theme scene saves one
and 19 bytes with its different diagram ID. Active preset palettes stay unchanged. This is
allocation/work removal, not a latency or v0.9.0 convergence claim. All registered checks pass.

The clean baseline is `fd9267eccc6daf7a68a56b9ef549409606cdc527`, following the accepted
[ZenUML checkpoint](zenuml_default_font_owner_macos_2026-10-20.md). The host is Apple M4 Pro,
48 GiB, macOS aarch64, Rust 1.95.0. The ignored registration, frozen sources/probes, build/link
identities and raw results are in `target/bench/experiments/gitgraph-empty-palette-css-20261020/`.
Both sides use the same optimized release opt3 profile and public feature set, with separately
identified libraries/executables. All 645 success controls, 26 errors and 418 catalog cells are
inherited unchanged. Each complete public render has one warmup and five samples, retaining its
rich result at the memory checkpoint. Input loading, theme compilation and result serialization
remain outside it. Library/probe builds use one job; nextest compilation uses four, execution
four test threads, with the same focused/full package/feature graph. Native tasks remain serial.

## Absence proves an empty producer

`has_palette_assignment` checks the same immutable eight optional slot fills that every prepared
paint getter ultimately requires. If all are None, every fill/stroke query is None regardless of
its declaration flags. The previous producer therefore emits no CSS and records no stylesheet
receipt event. It only allocates an escaped diagram identifier and checks 24 slot/surface pairs.
The three-line guard removes that pure internal work.

This is not a theme-None or palette-key heuristic. Some(empty string), partial assignments and
assigned-but-source-owned surfaces all conservatively use the original producer. The extra
predicate on active palettes is small but not claimed as a timing improvement. Ordinary CSS
identifier escaping, active declarations, receipt initialization/observations/completion and all
outer SVG/style structure remain unchanged.

The caller still pushes the empty String into the output. `XmlTextOutput::write_str` performs a
checkpoint even for empty input, so skipping that event would change cancellation behavior. The
guard only bypasses the pure producer's local String work; it cannot bypass meter, byte-limit,
writer-error or cancellation checkpoints. Custom/escaped diagram IDs retain ordinary escaping
and active palette escaping. No palette receipt-mask change is combined with this candidate.

## Complete public-operation observations

The preregistered no-theme GitGraph primary independently requires both count and bytes to
strictly decrease. Counts include allocation/reallocation observations.

| Operation | Allocations before → after | Allocated bytes before → after | Peak growth | Retained growth |
| --- | ---: | ---: | ---: | ---: |
| Default GitGraph medium | 579 → 578 | 90,466 → 90,450 | 41,830 | 18,551 |
| Catalog GitGraph, no theme | 579 → 578 | 93,562 → 93,543 | 43,274 | 18,575 |

The two probes use different diagram IDs and request/result recipes, so their byte totals are
not interchangeable. The removed CSS identifier buffer accounts for the respective 16/19 bytes.

All 645 successful controls preserve exact SVG/report identities and ordered custom-measurer
traces; all 26 errors match exactly. Of 503 jointly stable rows, only the registered GitGraph
primary improves and 502 remain unchanged; none increase any of the four memory metrics. Peak
and retained growth stay unchanged everywhere. The 142 variable rows remain inconclusive without
retry. Each side has 141 variable rows; only the baseline has
`family-flowchart_backedges_subgraphs`, while only the candidate has
`family-flowchart_direction_conflict`. The union adds `treeview-fixture-02` to the preceding
experiment's list and removes none. No observation is filtered or resampled.

All 418 catalog cells are stable and retain exact warmup/sample SVG/report and configuration,
diagnostics, theme status and family metadata. Only GitGraph's no-theme cell improves; 417 cells
retain all four metrics. The ten active GitGraph preset cells retain their original palette path
and all metrics. The catalog still covers 35 logical families and 38 fixture aliases across ten
presets plus no theme. This small default-path result does not establish a general elapsed-time
speedup or explain the remaining release gaps.

## Validation and continuation

Independent final source and raw-evidence reviews find no blocker. The actual one-file guard is byte-identical
to the reviewed draft. The baseline succeeds completely before the production edit, and its
four probes remain frozen. Existing active/partial palette, custom-ID, cancellation, exact-byte
budget and receipt contracts remain applicable without implementation-mirroring tests.

Focused GitGraph all-feature release nextest passes 121 tests. Full public/renderer release
nextest passes 5,612 tests with seven skipped, and strict matching release Clippy passes.
All 37 entries pass structure, parity and parity-root checks with existing browser-text-layout
and parser-diagnostic residuals retained; diagnostic residuals are not DOM equivalence. The full
optimized WASM package builds, including wasm-opt; WASM runtime smoke is not repeated. Formatting,
diff and changed-path checks pass. The driver exits zero with all 13 registered steps successful.
Logs, independent reviews, frozen sources/probes and 37 DOM reports remain in the ignored directory.
The measured patch SHA-256 is
`3335f97d5fee7237916c41af2ef29b530a95c91423535d243aba73f614c5516e`.

The all-family goal remains active. The previous release observation still reports ZenUML,
Packet and Kanban beyond budget and Class tiny, Sankey and Wardley inconclusive. This work does
not reclassify it. Next evaluate the reviewed XY empty paint owner and remaining typed ownership
candidates separately, retaining the full control set and resource policy.
