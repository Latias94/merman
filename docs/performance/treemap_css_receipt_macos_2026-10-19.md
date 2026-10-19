# Treemap CSS receipt ownership — macOS, October 19, 2026

Treemap's stylesheet success facts now borrow four immutable prepared values instead of copying
them into intermediate Boxes. Default public medium saves four allocations and 51 allocated bytes;
all SVG, report and measurement contracts agree. This is allocation/work evidence, without a
latency or v0.9.0 convergence claim.

The clean baseline is `cd7db81a6`, on Apple M4 Pro, 48 GiB, macOS aarch64, Rust 1.95.0. Both sides
use release opt3 and all public features. The preregistration, frozen probes, patch, input hashes,
logs and raw results are in `target/bench/experiments/treemap-borrowed-css-receipts-20261019/`.
The accepted preceding checkpoint supplies 584 inherited controls. Before production editing,
48 new controls complete one warmup, five allocation samples and ordered custom-measurer recording:
title/no title, six raw configurations and no theme/empty/title-fill/text-plus-font themes.
Configurations cover distinct fonts, independent colors, empty values, hidden values and small
fitting dimensions. All succeed. Cargo and probes run serially.

Only two source files change. `TreemapTitleCssEmission` borrows `title_color`;
`TreemapTypographyCssEmission` borrows `common.font_family()`, `label_color` and `value_color`.
Lifetimes bind only the prepared typography plan. The sole production caller compares these facts
immediately; no long-lived consumer acquires a borrow. The owned CSS String, title class, prefix
and root booleans, formatting, checkpoints, failure order and separate receipt gates remain intact.
Treemap's resolved font also supplies its root custom property; this does not import Info's
different root-font behavior. The title terminal receipt's three later copies remain separate.

| Public control | Allocations before → after | Allocated bytes before → after | Peak growth before → after | Retained growth |
| --- | ---: | ---: | ---: | ---: |
| Default medium | 754 → 750 | 92,539 → 92,488 | 33,709 → 33,658 | 18,807 |
| Typed font stack | 2,515 → 2,511 | 467,748 → 467,714 | 204,913 → 204,913 | 11,128 |
| Title with typed title fill | 2,597 → 2,593 | 502,001 → 501,926 | 213,024 → 213,024 | 19,008 |
| Empty raw fonts/colors, no theme | 744 → 743 | 92,407 → 92,368 | 33,685 → 33,646 | 18,807 |

The complete public render retains its rich result at the memory checkpoint; loading, theme
compilation and serialization are outside it. Counts include allocation/reallocation observations.
All 632 successful controls preserve exact SVG/report identities and ordered measurement traces;
26 public errors match. Of 490 jointly stable rows, all 55 Treemap controls improve and 435 are
unchanged; none increase any memory metric. Nonempty facts save four allocations, while eight
empty-color controls save one. Bytes saved range from 28 to 81, with no peak increase and unchanged
retained growth. The preceding 142 variable rows remain inconclusive without retry or name changes.

All 418 catalog cells retain warmup result metadata and all five saved sample SVG/report
identities, together with configuration, diagnostics, theme status and family identity.
All memory rows are stable. Eleven Treemap cells each save four allocations:
51 bytes for no theme, and 61–91 bytes for typed presets. The other 407 cells are unchanged.
Only the no-theme cell lowers peak growth, by 51 bytes; retained growth is unchanged everywhere.
Catalog and site-control totals stay within their own request/result recipes.

History identifies the new intermediate owners: `f46172bedb` adds title fill, `8871438c62` adds
typography font, `a8e23990a` adds shared text fill, and `9578a1ac0f` splits it into label/value.
`01cb9ca2d` moves actual values into the prepared plan, enabling direct borrowing. v0.9.0 has no
corresponding emission owners. This does not attribute the whole release gap to these copies.

Independent source review finds no blocking issues. Existing independent label/value ownership,
transparent titles, typed measurement, missing/wrong title facts and exact/one-byte-short SVG
tests cover the change; no implementation-mirroring test is added. The focused release nextest
run passes 61 tests. The full renderer release run passes 5,208 tests with five skipped; release
all-feature Clippy passes. All 37 DOM entries pass structure, parity and parity-root modes with
the existing browser-text-layout receipts. The full optimized WASM package builds, including
wasm-opt; WASM runtime smoke was not repeated. Formatting, diff and changed-path checks pass.
Independent raw-evidence review verifies the frozen inputs, linked probes and report numbers.

The all-family goal remains active. Next test reuse of the shared empty source-config owner, then
simplify copied terminal receipt values in Info/Error and Treemap title, retaining each state
machine, successful completion and proof semantics.
