# Pie CSS facts — macOS, October 20, 2026

Pie CSS success facts now borrow the actual values from the immutable paint binding instead of
copying them into five temporary boxed strings. The existing receipt still owns its observations
and validates the same values. Each of the ten themed Pie catalog cells saves five allocations
and 71 bytes per complete public render; no-theme Pie is unchanged. This is allocation/work
removal, without a latency or v0.9.0 convergence claim. All registered checks pass.

The clean baseline is `1ceb4e8d2bc1034ac9a664a8d2f6b5a39392a650`, after the
[XY owner checkpoint](xy_empty_paint_owner_macos_2026-10-20.md). The host is Apple M4 Pro,
48 GiB, macOS aarch64, Rust 1.95.0. Registration, source freeze, read-only baseline probes,
build/link identities and raw results remain under
`target/bench/experiments/pie-css-facts-20261020/`. Both sides use release opt3 with the same
public feature set. All 645 success controls, 26 errors and 418 catalog cells are inherited
unchanged. Complete public render sampling uses one warmup and five samples, retaining the rich
result at the memory checkpoint; source loading, theme compilation and serialization are outside
it. Library/probe compilation uses one job. Focused and full all-feature nextest share one
feature graph with four compile jobs and four test threads. Native tasks remain serial. The
baseline completes and freezes four probe identities before the production edit.

## Borrow the successful writer values

`PieCssEmission` previously stored five `Option<Box<str>>` fields. Their only consumer immediately
read them into the existing receipt while the owning `PieThemePlan` was still borrowed. They now
use `Option<&str>` with a lifetime tied only to that immutable plan, independent of the sink and
diagram ID. No extra source/configuration owner, cache or validation layer is introduced.

All five selection gates remain: slice and outer stroke retain their own option and
`slices_present` filter, title and section text retain separate options and actual binding
strings, and typography still selects the binding's common font. These are the actual writer
values, never substituted typed expectations. Some empty values and no-slice baseline behavior
are preserved. The prefix, CSS body, checkpoint and root writes remain unchanged; successful
facts are still constructed only after those operations succeed. XML escaping, cancellation,
work accounting, errors, receipt checks and single completion remain unchanged. The earlier
shared root-font changes remain intact. Existing output, ownership, no-slice and byte-budget
contracts are reused; no implementation-mirroring test is added.

## Complete public-operation observations

The source-selected primary is the existing `pie_medium/editor-light` catalog cell. Both
allocation count and allocated bytes must independently decrease. Counts include allocation and
reallocation observations. The 645-control Pie row has no theme and is an unchanged secondary.

| Operation | Allocations before → after | Allocated bytes before → after | Peak growth | Retained growth |
| --- | ---: | ---: | ---: | ---: |
| Pie, editor-light catalog | 3,349 → 3,344 | 656,462 → 656,391 | 311,625 | 8,185 |
| Pie, no-theme control | 368 → 368 | 38,263 → 38,263 | 17,180 | 7,855 |

All 645 success controls preserve exact SVG/report identities and ordered custom-measurer
traces. All 504 jointly stable rows retain all four memory metrics; all 26 errors match exactly.
The 141 variable rows remain inconclusive without retry. Baseline has 139 and candidate 141;
direction-conflict Flowchart and TreeView-02 vary only in the candidate. These are the only two
additions to the preceding experiment's variable union. No input or observation is filtered or
resampled.

All 418 catalog cells are stable and retain exact warmup result metadata and all five saved
sample SVG/report identities, together with configuration, diagnostic, theme-status and family
metadata. The probe does not save warmup SVG bytes, so their byte identity is not separately
observed. The ten themed Pie cells each save five allocations
and 71 bytes. The other 408 cells retain all four metrics, including no-theme Pie. All peak and
retained-growth metrics are unchanged. The catalog covers 35 logical families and 38 fixture
aliases across ten presets plus no theme.

## Validation and continuation

Independent final source and raw-evidence reviews find no blocker. Focused Pie all-feature
release nextest passes 75 tests. Full public/renderer release nextest passes 5,613 tests with
seven skipped, and strict matching release Clippy passes. All 37 entries pass structure, parity
and parity-root checks with existing browser-text-layout and parser-diagnostic residuals
retained; diagnostic residuals are not DOM equivalence. The full optimized WASM package builds,
including wasm-opt; WASM runtime smoke is not repeated. Formatting, diff and changed-path checks
pass. The driver exits zero with all 13 registered steps successful. Logs, reviews, source/probe
identities, the warmup evidence clarification and 37 current DOM reports remain in the ignored
directory. No extra sample or retry is added to resolve the wording clarification.
The measured source patch SHA-256 is
`546e297d5b769ff6c45b734084daf3e11273066f9e3882ccf965f50ecf05454d`.

The all-family goal remains active. The earlier release observation still has ZenUML, Packet and
Kanban beyond budget and Class tiny, Sankey and Wardley inconclusive. This allocation result does
not reclassify that observation. Next independently evaluate Kanban's duplicated bound-paint
owner against its active typed catalog primary, retaining no-theme controls unchanged. Keep
receipt-state and finite-geometry candidates separate.
