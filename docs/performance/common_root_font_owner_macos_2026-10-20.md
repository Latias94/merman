# Shared root-font ownership — macOS, October 20, 2026

The shared prepared CSS stores a separate root-font value only when the configured-root setter
selects one. Resolved root values borrow the existing immutable font owner. This removes the
initial font clone, including branches that immediately overwrite it. The independent Info,
Error and Kanban primaries each save one allocation and 39 bytes. The result is allocation/work
removal, not a latency or v0.9.0 convergence claim. All registered checks pass.

The clean baseline is `32eda1d494c00870dd3840b8e28b3ce8af5d083f`, after the accepted
[Quadrant checkpoint](quadrant_point_binding_macos_2026-10-20.md). The host is Apple M4 Pro,
48 GiB, macOS aarch64, Rust 1.95.0. The registration, source freeze, four read-only baseline
probes, build/link identities and raw results are in
`target/bench/experiments/common-root-font-owner-20261020/`. Both sides use optimized release opt3
and the same all-public-feature recipe, with separately identified baseline/candidate libraries.
The 642 inherited success controls, 26 errors and 418 catalog cells are unchanged. Each complete
public operation uses one warmup and five samples and retains its rich result at the memory
checkpoint. Input loading, theme compilation and result serialization remain outside it.

Library builds use one job and probes run serially. Only non-timing nextest compilation uses four
jobs, with four test threads. Focused and full nextest use the same package/feature graph so the
full run can reuse those artifacts. There is still one native driver at a time, with no probe,
Cargo or profiling overlap. All baseline captures succeed before the production edit.

## Preserve selection, remove the initial duplicate

`PreparedCommonCss::with_resolved_typography` formerly clones its owned font into a second root
field. Some callers retain it; others immediately replace it with the configured-root helper's
result. The new `root_font_family_override: Option<String>` starts as None, meaning the root rule
borrows the existing font. The unchanged setter evaluates the original helper at its original
stage and stores Some(result). The sole root reader selects that override or the immutable font
at the original call after Neo CSS succeeds.

Some(empty) must remain distinct from None: an explicitly empty configured root omits its rule
while the ordinary theme font can still be nonempty. Missing configured root still stores the
helper's default, rather than inheriting an unrelated theme font. Independently configured root
Courier and theme Arial remain separate. Resolved/typed font paths retain their original
precedence. No equality deduplication, delayed helper evaluation, new cache or public API is added.

The font has no later mutation. Derived Clone preserves the selected value and independent owner
semantics; the resolved case avoids cloning its discarded duplicate too. GitGraph's existing
InfoCssWriter clone changes only text/border color, emits prefix/root, then moves font data into
owned parts. The borrowing cannot outlive that emission. The three existing Info CSS receipt
values keep their original font source, including the third; this ownership change does not
reinterpret that separate contract. Empty-root branch behavior, writer calls, byte accounting,
checkpoints, error propagation, completion receipts, resource and cancellation order stay intact.

The unnecessary initial clone entered in theme integration merge
`45790659a122e58f9a0990e02092f06c59fe70a5`. Independent configured-root semantics already existed
before v0.9.0 in `cd6d4895c2647fa941740129cae9b2e1a30a99d2`; the tag constructs its selected
font/root values directly. The candidate removes the redundant temporary owner, not the required
configured-root selection. Packet does not use this shared prepared type, so it is not an affected
family or an explanation for the complete Packet/Kanban release difference.

## Complete public-operation observations

Each primary independently requires strict count and allocated-byte reductions. Counts include
allocation and reallocation observations.

| Primary | Allocations before → after | Allocated bytes before → after | Peak growth before → after | Retained growth |
| --- | ---: | ---: | ---: | ---: |
| Info medium | 124 → 123 | 19,339 → 19,300 | 9,794 → 9,755 | 4,111 |
| Error basic | 132 → 131 | 27,032 → 26,993 | 13,679 → 13,640 | 7,855 |
| Kanban medium | 616 → 615 | 115,454 → 115,415 | 45,838 → 45,838 | 30,319 |

All 642 successful controls retain exact SVG/report identities and ordered custom-measurer
traces; all 26 errors match exactly. Of 502 jointly stable controls, 201 improve, 301 stay
unchanged and none increase any of the four memory metrics. Two hundred improvements save one
allocation and 16–43 bytes; GitGraph medium saves two and 78 bytes because its subsequent clone
also loses the duplicate. Peak never rises and retained growth is unchanged. Empty fonts need
no buffer allocation, so the source-level clone removal is not a claim that every possible input
must save an allocation.

The 140 variable rows remain inconclusive without retry. The baseline has 138 variable rows and
the candidate 140; the two additional candidate rows are `family-flowchart_backedges_subgraphs`
and `treeview-fixture-02`. Relative to the preceding experiment's joint variable list, this one
adds the backedges row and loses `family-flowchart_direction_conflict` and
`family-flowchart_long_edge_labels`. No observation is filtered or resampled.

All 418 catalog cells are stable and retain exact warmup/sample SVG/report, configuration,
diagnostics, theme status and family identity. The 209 improvements are 19 affected families
times eleven theme selections; 209 other cells retain all four metrics. GitGraph's eleven cells
save two allocations and 78–86 bytes each; the other 198 save one and 37–43 bytes. Peak growth is
nonincreasing and retained growth unchanged. The catalog still covers 35 logical families and
38 aliases across ten presets plus no theme. The empty authoring controls in the separate corpus
resolve normal defaults and are compared to their own frozen baseline, not treated as equivalent
to no theme or an empty resolved family view.

## Validation and continuation

Independent final source and raw-evidence reviews find no blocker. The one actual source file is byte-identical
to the reviewed four-hunk draft. The configured helper and all callers/feature gates stay in
place; existing root/theme separation, explicit blank root, resolved typography, precedence,
failed-sink and exact SVG-budget tests cover the changed boundary.

Focused CSS/font all-feature release nextest passes 708 tests. The complete public/renderer
release nextest run passes 5,612 tests with seven skipped, and strict matching release Clippy passes.
All 37 entries pass structure, parity and parity-root checks with existing browser-text-layout
and parser-diagnostic residuals retained; diagnostic residuals are not DOM equivalence. The full
optimized WASM package builds, including wasm-opt; WASM runtime smoke is not repeated. Formatting,
diff and changed-path checks pass. The driver exits zero with all 13 registered steps successful.
Logs, independent reviews, frozen sources/probes and 37 DOM reports remain in the ignored directory.
The measured source patch SHA-256 is
`1ffab6264b060fb5daf18d3adcf933b0fb725c6bb9f24b0e835cf6c2bc28f5ac`.

The all-family goal stays active. The earlier release comparison still has Zenuml, Packet and
Kanban beyond budget and Class tiny, Sankey and Wardley inconclusive. This allocation result does
not reclassify those observations. Next independently evaluate the reviewed default GitGraph,
XY paint and ZenUML default-font owners, retaining exact successful/error/catalog controls and
resource policy; Pie's temporary CSS borrowing remains a separate typed-path candidate.
