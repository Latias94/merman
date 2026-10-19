# Sequence foreground owners — macOS, October 19, 2026

Sequence's terminal text style now borrows its raw or typed foreground from the existing immutable
owner. Two duplicate arrays of owned foregrounds and an unconsumed BoxTitle baseline declaration
are removed. The default public medium fixture saves sixteen allocations and 550 allocated bytes;
the combined typed control saves twenty-one allocations and 563 bytes. This is allocation/work
evidence, without a latency or v0.9.0 convergence claim.

## Experiment and source ownership

The clean baseline is `7f5bb54f2dd66a67fde6cd86028841b97cf9b520`, on Apple M4 Pro, 48 GiB,
macOS aarch64, Rust 1.95.0. Both sides use release opt3 and all public features. The ignored ledger,
frozen patch, input hashes, executables and results are under
`target/bench/experiments/sequence-foreground-owner-20261019/`.

The complete question, independent source design, frozen baseline and gates were registered before
editing. The preceding shared-CSS checkpoint's production source equals this baseline; the
intervening commit changes documentation only. Its frozen probes supply all 584 successful
controls, 26 public errors and the 418-cell catalog. Inputs include 86 Sequence controls for raw
configuration, no theme, empty/paint/font/width/combined themes, three looks and 0/1/16/64 messages.
Each public memory control has one warmup and five samples. Cargo and probes run serially.

The raw compatibility binding already owns each surface's fallback color. Resolved typography
already owns its typed color. Previously the typography plan cloned seven raw foregrounds and
replaced typed winners with more clones; the prepared CSS then cloned all seven winners again.
Both duplicate foreground arrays are removed. The existing seven provenance entries remain,
selected by the same preparation-time ownership checks. Later getters use fixed surface mapping
and the selected source, with no additional set query, parsing, allocation or assertion.

ParticipantLabel and BoxTitle retain separate raw sources despite sharing the Actor role; Loop
keyword and title surfaces retain their separate raw owners as well. Equal raw/typed colors do
not collapse provenance. All seven math/layout/terminal consumers receive the same foreground and
source. The successful stylesheet receipt still records all seven surfaces in its original place
and order, including BoxTitle.

Six actually consumed baseline declarations retain their preparation-time formatting and original
relative order. Their CSS templates, fragments and checkpoints remain unchanged. BoxTitle's
baseline declaration has no stylesheet reader, so its unused formatting is removed; its actual
text inheritance, dynamic text group and terminal receipt remain. No new cache or wrapper is added.

## Exact public measurements

Measurements cover the complete public render with its rich result retained at the checkpoint.
Source loading, theme compilation and serialization are outside the operation. Counts include
allocator allocation/reallocation observations.

| Control | Allocations before → after | Allocated bytes before → after | Peak growth before → after | Retained growth |
| --- | ---: | ---: | ---: | ---: |
| Sequence medium | 4,588 → 4,572 | 745,438 → 744,888 | 288,415 → 287,877 | 60,271 |
| Sequence tiny | 2,338 → 2,322 | 510,304 → 509,754 | 247,786 → 247,248 | 40,493 |
| Rich classic, no theme | 3,410 → 3,394 | 585,786 → 585,236 | 255,113 → 254,575 | 39,025 |
| Rich classic, empty theme | 6,864 → 6,847 | 1,279,542 → 1,278,985 | 470,150 → 470,150 | 42,143 |
| Rich classic, combined theme | 6,749 → 6,728 | 1,285,986 → 1,285,423 | 470,150 → 470,150 | 41,785 |
| Rich classic, typed fonts | 6,617 → 6,600 | 1,269,385 → 1,268,828 | 470,150 → 470,150 | 41,915 |
| 64 messages, no theme | 8,573 → 8,557 | 1,144,033 → 1,143,483 | 359,525 → 358,987 | 80,479 |

All 584 successful controls preserve exact SVG/report identities and ordered custom-measurer
traces; all 26 public errors match. Of 442 jointly stable memory rows, 95 improve and 347 are
unchanged, without increases in any of the four metrics. Forty-four rows save sixteen allocations
and 550 bytes, forty save twenty-one and 563 bytes, and eleven save seventeen and 557 bytes.
Peak growth either falls by 538 bytes or remains unchanged; retained growth never changes.

The 142 variable rows remain inconclusive without retry: the preceding 140 plus
`family-flowchart_direction_conflict` and `treeview-fixture-02`. Their raw samples remain available,
and their medians are not admitted as improvements or regressions. Output contracts still agree.

All 418 catalog cells retain warmup/sample output, SVG/report, configuration, diagnostics, theme
status and family identity. All memory rows are stable. Eleven Sequence cells improve: no theme
saves sixteen allocations and 550 bytes; each typed preset saves twenty-one allocations and
542–614 bytes. The other 407 cells are unchanged. Only the no-theme cell reduces peak growth,
by 538 bytes; retained growth is unchanged everywhere. Catalog and site-control totals differ
because their original request/result recipes differ, and comparisons stay within each lane.

## History, validation and remaining work

`f8f9d83c1` introduces the first foreground array and provenance for math preparation; actual
color/source consumers remain necessary, while duplicate color ownership does not. `b997dc160`
adds the prepared CSS foreground layer and unused declaration. Earlier scalar and empty-group
checkpoints are separate, and `9a297b72f` has already removed a different baseline-fill copy.
These source changes span math preparation and theme binding; the full release gap is not
attributed to this ownership alone.

Independent source review finds no blocking issues. One added semantic test distinguishes equal
Participant raw and BoxTitle typed colors by provenance; existing actor/loop local ownership,
seven-surface, math, failed-sink and exact/one-byte-short budget tests cover the remaining contracts.
The focused release nextest run passes 327 tests. The full renderer release run passes 5,208 tests
with five skipped, and release all-feature Clippy passes. All 37 DOM entries pass structure,
parity and parity-root modes with the existing browser-text-layout receipts. The full optimized
WASM package builds, including wasm-opt; WASM runtime smoke was not repeated. Formatting, diff
and changed-path checks pass. Independent raw-evidence review confirms frozen inputs, library and
probe identities, every comparison lane and the report's numbers.

The all-family goal remains active. Next evaluate Treemap's temporary CSS success values and the
independent Info/Error terminal receipt state. Preserve their actual consumers and completion
uniqueness, rather than treating all evidence work as removable.
