# Shared CSS borrowed receipt — macOS, October 19, 2026

The shared Info/Error/Quadrant/XY stylesheet writer now borrows its three font success facts from
the existing immutable `PreparedCommonCss`. The default public controls each save three allocations
and 117 allocated bytes. The actual writes, recorded values and terminal consumers remain intact.
This checkpoint measures allocation/work removal, without a latency or v0.9.0 convergence claim.

## Experiment and ownership

The clean baseline is `58388dca5`, on Apple M4 Pro, 48 GiB, macOS aarch64, Rust 1.95.0. Both sides
use release opt3 and all public features. The ignored question, preregistration, frozen executables,
patch, inputs and results are under `target/bench/experiments/common-css-borrowed-receipt-20261019/`.

The preceding empty-group checkpoint's frozen production probes and 488 successful controls supply
the inherited baseline. Before editing, another 96 controls complete public preflight, one warmup,
five allocation samples and ordered custom-measurer recording. They cover each of the four families
with no theme, an empty compiled theme and typed fonts, across default, distinct root/theme fonts,
empty, whitespace, null, numeric, XML-sensitive and Neo configurations. All succeed. Twenty-six
inherited public errors and the 418-cell catalog remain separate lanes. Cargo and probes run serially.

`InheritedFontStackCssWrite` keeps three fields and their existing getters, replacing `Box<str>`
with `&str`. The function's lifetime binds only `PreparedCommonCss`; it does not bind the output
stream or diagram ID. Two facts still follow the successful prefix write and the third still
follows the successful root write. Errors return before a complete receipt. No formatter, XML
wrapper, fragment, scope, checkpoint or cancellation call changes.

All three values deliberately remain `values.font_family`, including when the root CSS uses a
different font or omits its variable. This ownership change does not redefine the existing receipt.
Info/Error synchronously compare the borrowed facts and retain their own terminal copies; those
long-lived owners remain. Quadrant compares and stores results, while XY consumes the root getter
and stores results. No caller stores the intermediate borrowed receipt, and no public API changes.

## Exact public measurements

The complete public render retains its rich result at the memory checkpoint. Source loading,
theme compilation and serialization stay outside the measured operation. Counts include allocator
allocation/reallocation observations; every row has one warmup and five samples.

| Control | Allocations before → after | Allocated bytes before → after | Peak growth before → after | Retained growth |
| --- | ---: | ---: | ---: | ---: |
| Info medium | 134 → 131 | 19,861 → 19,744 | 10,076 → 9,959 | 4,111 |
| Error basic | 142 → 139 | 27,554 → 27,437 | 13,961 → 13,844 | 7,855 |
| Quadrant medium | 403 → 400 | 38,276 → 38,159 | 19,280 → 19,163 | 7,599 |
| XY medium | 1,898 → 1,895 | 178,696 → 178,579 | 104,083 → 103,966 | 15,343 |

All 584 successful controls preserve exact SVG/report identities and ordered measurement traces;
all 26 errors match exactly. Of 444 jointly stable rows, 102 improve and 342 remain unchanged,
without increases in any of the four metrics. Every added control improves, alongside the four
inherited family controls and two Info canvas controls. Each improvement saves three allocations
and 48–129 bytes. Peak growth either falls by the same bytes or remains unchanged; retained growth
never changes. The 140 variable rows remain inconclusive without retry. Relative to the preceding
comparison, `flowchart_direction_conflict` becomes jointly stable; no variable name is added.

The catalog covers 38 scenes across 35 logical families, ten presets and no theme: 418 cells.
Warmup result metadata, all five saved sample SVG/report identities, configuration,
diagnostics, theme status and family identity agree.
All memory rows are stable. Each of the 44 cells in the four affected families saves three
allocations: 117 bytes for no theme and 129 for each typed preset. All other 374 cells are unchanged.
Peak growth falls by 117 bytes for the four default cells and by 129 bytes for XY's ten typed cells;
other cells retain their peak. Retained growth is unchanged everywhere. Catalog and site-control
totals stay within their own request/result lanes.

## Historical attribution and validation

`6209fb12c` introduces the three owned success facts, originally cloning twice and moving the
third font String. `3f60081e6` retains that ownership. `6ff15c63d` makes CSS preparation long-lived,
so the third move becomes another clone. v0.9.0 has no corresponding intermediate receipt: its
Info/Error/Quadrant path moves the original font into `InfoCssParts`, while XY uses its existing
default-font CSS path. Real configuration and CSS owners already present in the tag are not counted
as these added copies. This removes specific theme-refactor ownership, not the whole release gap.

Independent source review finds no blocking issues. Existing root/theme separation, empty fonts,
XML and exact/one-byte-short SVG budget tests, supplemented by the public controls, cover this
private ownership change without implementation-mirroring tests. The focused library nextest run
passes 106 tests. The full renderer release run passes 5,207 tests with five skipped, and release
all-feature Clippy passes. All 37 DOM entries pass structure, parity and parity-root modes with the
existing browser-text-layout receipts. The full optimized WASM package builds, including wasm-opt;
WASM runtime smoke was not repeated. Formatting, diff and changed-path checks pass. Independent
raw-evidence review confirms the source provenance, frozen inputs, samples and report numbers.

The all-family goal remains active. Treemap's four temporary success values and Sequence's duplicate
foreground owners are independent next experiments; neither has a measured saving in this report.
