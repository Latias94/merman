# ZenUML default CSS font owner — macOS, October 20, 2026

ZenUML's default CSS font now borrows its static fallback instead of allocating a temporary
String that is immediately formatted and dropped. Both independently required default primaries
save one allocation and 23 bytes. All authored Some-font normalization paths remain owned and
unchanged. This is allocation/work removal, not a latency claim or an explanation for the
complete +3.561 us release gap. All registered checks pass.

The clean baseline is `2bee8f77544a22a3fdb56974e8df9db6a70bd4a6`, after the accepted
[shared root-font checkpoint](common_root_font_owner_macos_2026-10-20.md). The host is Apple
M4 Pro, 48 GiB, macOS aarch64, Rust 1.95.0. The ignored registration, source freeze, read-only
baseline probes, independent reviews and raw results are in
`target/bench/experiments/zenuml-default-font-owner-20261020/`. Both sides use the same optimized
release opt3 profile and public feature set, with separately identified libraries/executables.
Library/probe builds use one job, nextest compilation four jobs and execution four test threads;
all native tasks and captures remain serial. Focused/full nextest share their package/feature graph.

The existing 642 success controls are unchanged, including 45 Zen-specific entries. Three frozen
public recipes add a minimal default-font scene, explicit site namespace empty font and a spaced,
quoted frontmatter font. All 645 baseline operations succeed before the production edit. Each
complete public render uses one warmup and five samples, retaining its rich result at the memory
checkpoint; input loading, theme compilation and result serialization remain outside it.

## Borrow the default, retain authored normalization

`ZenumlTypography::css_family` returns `Cow<'_, str>`. None borrows the exact existing CSS spelling
`Helvetica,Verdana,serif`. Some still calls the same `normalize_css_font_family` and owns its
result, including empty output. Borrowing a raw authored font would bypass required normalization
and is excluded. Some(empty) remains distinct from None and does not acquire the fallback.

The type and getter are crate-private. The sole production consumer evaluates the getter once
as a named format argument, then uses its Display value at twelve positions in the same CSS
format string. The temporary survives through formatting as before. There is no CSS order,
XML encoding, color, title receipt, geometry, work charge, byte-limit, cancellation or error-order
change. The full stylesheet String remains; this candidate removes only the default font buffer.
The separate finite-geometry traversal fusion is not part of this patch.

The default String entered in `ef9595dab8cbbb57d1a1acff51e0b7aad195bdc4`; v0.9.0 emits the same
CSS font as a literal. The registration has a source-length typo of 22; the fallback is actually
23 UTF-8 bytes. A separate clarification records that fact before candidate compilation/sampling,
retaining the original registration, inputs and strict count/byte reduction criteria unchanged.

## Complete public-operation observations

Each primary independently requires both allocation count and allocated bytes to strictly
decrease. Counts include allocation/reallocation observations.

| Primary | Allocations before → after | Allocated bytes before → after | Peak growth | Retained growth |
| --- | ---: | ---: | ---: | ---: |
| ZenUML medium, no theme | 462 → 461 | 152,550 → 152,527 | 95,920 | 9,555 |
| Minimal default font | 237 → 236 | 134,722 → 134,699 | 92,483 | 9,531 |

All 645 successful controls preserve exact SVG/report identities and ordered custom-measurer
traces, and all 26 public errors match. Of 504 jointly stable rows, 54 improve by exactly one
allocation and 23 bytes, 450 stay unchanged and none increase any of the four memory metrics.
Peak and retained growth are unchanged for every stable row. The 141 variable rows remain
inconclusive without retries. Baseline and candidate each have 140 variable rows; only the
baseline has `family-flowchart_direction_conflict`, while only the candidate has
`family-flowchart_backedges_subgraphs`. Relative to the preceding experiment's union, this one
adds direction-conflict and long-edge-label rows and loses `treeview-fixture-02`. No row is
removed or resampled because of its result.

The explicit site empty-font and spaced/quoted frontmatter controls retain all four metrics.
Saved baseline SVG shows an empty `font-family:` value for the former, with no fallback; the
latter preserves normalization of its quoted internal commas and escaped quotes. Recipe names
alone do not establish that a custom font is selected: some older configuration-named rows use
paths not consumed by ZenUML's font namespace. Only actual configuration binding, final CSS and
matching measurements establish the branch. The unchanged baseline and raw outputs are retained.

All 418 catalog cells are stable and preserve exact warmup/sample SVG/report and configuration,
diagnostics, theme status and family metadata. Eleven ZenUML cells save one allocation and
23 bytes each, across no theme and ten presets; 407 other cells retain all four metrics. Peak
and retained growth are unchanged throughout. The catalog still covers 35 logical families and
38 fixture aliases. These catalog presets do not project a new font into ZenUML's source namespace.

## Validation and continuation

Independent final source and raw-evidence reviews find no blocker. The actual single-file
one-hunk source change is byte-identical to the reviewed draft, and all baseline probes/source
identities remain frozen. Existing authored-empty, normalization, XML-once escaping, forbidden
scalar, exact byte-ceiling and title-proof controls cover the unchanged branch boundaries.

Focused ZenUML all-feature release nextest passes 49 tests. Complete public/renderer release
nextest passes 5,612 tests with seven skipped, and strict matching release Clippy passes.
All 37 entries pass structure, parity and parity-root checks with the existing browser-text-layout
and parser-diagnostic residuals retained; diagnostic residuals are not DOM equivalence. The full
optimized WASM package builds, including wasm-opt; WASM runtime smoke is not repeated. Formatting,
diff and changed-path checks pass. The driver exits zero with all 13 registered steps successful.
Logs, independent reviews, frozen sources/probes and 37 DOM reports remain in the ignored directory.
The frozen measured source patch is
`434b0c41550dc1ee4301a145151663bb0416630a06cbce1a738ce52b6e1d896a` (SHA-256).

The all-family goal stays active. The earlier release comparison still reports ZenUML, Packet
and Kanban beyond budget and Class tiny, Sankey and Wardley inconclusive. This work does not
reclassify those observations. Next evaluate empty GitGraph palette CSS and the other reviewed
ownership candidates separately; any future ZenUML finite-check fusion must retain the complete
geometry/error contract, original verdict position and all measurer calls.
