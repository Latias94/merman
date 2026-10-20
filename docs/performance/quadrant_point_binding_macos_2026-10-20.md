# Quadrant point binding ownership — macOS, October 20, 2026

Quadrant point styles now select borrowed values in the required layout loop and copy each final
string directly into the public layout once. The prepared theme plan no longer retains an owned
point-binding array or builds it in an independent traversal. Default medium saves twelve
allocations and 384 allocated bytes. The independently required empty-theme and typed controls
also save twelve allocations, with 332 and 319 bytes respectively. This is allocation/work
removal, not a latency or v0.9.0 convergence claim. All registered checks pass.

The clean baseline is `d0c792f135c4578408a5189dd30ebbfd65ac53b4`, on Apple M4 Pro, 48 GiB,
macOS aarch64, Rust 1.95.0. The ignored registration, frozen inputs/probes, source patch and raw
results are in `target/bench/experiments/quadrant-point-binding-20261020/`. Both sides use the same
optimized release opt3 profile and public feature set, with separately identified baseline and
candidate libraries. The existing complete public render
probes use one warmup and five samples, retaining rich results at the memory checkpoint. Input
loading, theme compilation and report serialization stay outside the measured operation. Native
builds, probes and checks run serially; no new timing or benchmarking framework is introduced.

## One final owner, one necessary loop

The old plan prepares three owned strings per point, stores them in a boxed binding array, then
the layout clones all three into its own final point data. Terminal SVG reads the completed
layout and the plan's independent proof expectations; it never consumes the retained bindings.
The extra `bind_points` pass also projects all layout settings just to read the default radius,
allocating otherwise discarded x/y axis-position strings.

The plan retains an explicit semantic point count. Its existing point expectations, CSS,
common-font data and terminal proofs remain intact. The required layout loop looks up the source
class, selects a short-lived borrowed binding and owns the final strings once. It reuses the
already projected `cfg.point_radius`. Both layout and render keep their original first count
checks. The binding pass contains no fallible operation, meter charge, checkpoint, sanitizer,
parser or user callback, so removing it does not reorder those boundaries.

Radius precedence stays source point, class, prepared expectation and configured default. Fill
stays source point, class, prepared source/config/typed expectation and CSS fallback. Stroke color
and width keep their independent original fallbacks. Empty authored values and exact raw spelling
stay significant. Typed radius Clear keeps its exact token and viewport expectation. Resolve's
entry `charge(point_count)`, all fallible theme preparation and unsupported-theme reconciliation,
all receipt cardinality/identity checks and completion seals stay unchanged.

Public family preparation always passes the same immutable model/config to its local plan and
layout. Private pairing of a plan prepared from model A with an unrelated equal-length model B
is not a public contract and does not justify a general provenance framework. Two former
helper-storage tests now exercise completed layouts, including a real compiled typed theme and
raw direct-model values. The public parser rejects authored `var(...)`, `currentColor`, empty
colors and `0.25em` widths; those direct-model cases remain private controls rather than being
misreported as successful public text inputs.

The duplicate prepared owner entered in `6ff15c63d74f6b6cc06f172aefacc80a33dbf6f1`.
v0.9.0 selected source/config values in layout and already duplicated one local fill. Thus not
all removed copies can be attributed as new work relative to the tag.

## Frozen public controls and observations

The 632 inherited rows are unchanged, with ten separately frozen Quadrant inputs appended before
any measurement. All ten first succeed on the baseline. Their saved SVGs establish source/class/
inline radius and paint priority, combined typed radius/fill, zero/one/multiple points, explicit
site radius/fill ownership, Clear radius `5`, and configured raw `var(--point)`, `currentColor`
and empty fill/stroke. None and Some(empty) source/class scenes each match their own frozen
baseline; equality between those two different operations is not assumed. Raw legacy fallback
tokens also stay unchanged; this candidate does not normalize paint output.
The probe resolves its empty authoring specification with normal defaults, including font and
canvas values, so that recipe is not an empty resolved family view or equivalent to no theme.

Each of the following three primaries independently requires strict allocation-count and byte
reductions. Counts include allocation and reallocation observations.

| Independent primary | Allocations before → after | Allocated bytes before → after | Peak growth before → after | Retained growth |
| --- | ---: | ---: | ---: | ---: |
| Default medium | 396 → 384 | 37,919 → 37,535 | 19,163 → 18,786 | 7,599 |
| Source/class, empty theme | 2,205 → 2,193 | 432,174 → 431,842 | 202,283 → 202,283 | 7,648 |
| Source/class, typed radius/fill | 2,232 → 2,220 | 434,998 → 434,679 | 202,283 → 202,283 | 7,648 |

For three points with nonempty selected strings, twelve removed allocations comprise nine
intermediate string buffers, one binding array and two discarded axis-position strings. Empty
strings need no buffer allocation, so counts vary across the other controls. The zero-point typed
control still saves two allocations and 15 bytes by removing the unused settings projection.

All 642 successful controls preserve exact SVG/report identities and ordered custom-measurer
traces. All 26 public errors match. Of 501 jointly stable rows, 35 Quadrant controls improve and
466 remain unchanged, with no increase in allocation count, bytes, peak or retained growth.
Savings range from two to twelve allocations and 15 to 384 bytes. Retained growth never changes.
The 141 variable rows stay inconclusive without retries. The baseline has 140 variable rows and
the candidate has 141; only `treeview-fixture-02` differs between them. Relative to the preceding
experiment, the union adds that TreeView row and `family-flowchart_direction_conflict`, while
`family-flowchart_backedges_subgraphs` becomes stable. No timing or memory row is dropped.

All 418 catalog cells are stable and preserve exact warmup result metadata and all five saved
sample SVG/report identities, together with configuration, diagnostics, theme status and
family identity.
The eleven Quadrant cells improve by twelve
allocations each: no theme saves 384 bytes, presets save 345–375 bytes. Peak growth never rises
and retained growth is unchanged; the other 407 cells retain all four memory metrics. The catalog
continues to cover 35 logical families and 38 aliases across ten presets plus no theme. Its
request/result recipe differs from the site-control probe, so absolute byte totals are not
interchanged between those tables.

## Validation and continuation

Independent final source and raw-evidence reviews find no blocker. The two actual source files match the frozen
measured patch; the only difference from the reviewed draft is formatting in an adapted test.
The baseline is captured successfully before production edit and its four probes are frozen.

Focused Quadrant all-feature release nextest passes 45 tests.
The complete public/renderer all-feature release nextest run passes 5,612 tests with seven
skipped, and strict matching release Clippy passes.
All 37 entries pass structure, parity and parity-root checks with the existing browser-text-layout
and parser-diagnostic residuals retained; parser-diagnostic residuals are not DOM equivalence.
The optimized full WASM package builds, including wasm-opt; WASM runtime smoke is not repeated.
Formatting, diff and changed-path checks pass. The driver exits zero with all 13 registered steps
successful. Its logs, frozen sources/probes, independent reviews and 37 DOM reports remain in the
ignored experiment directory. The frozen candidate patch SHA-256 is
`4ce88578fc4d36fe1b1d15d1392397e3793014ca91f1793db28f062ce772b606`.

The all-family goal stays active. The earlier release comparison still has Zenuml, Packet and
Kanban above budget and Class tiny, Sankey and Wardley inconclusive; this local allocation
result does not reclassify it. Next independently evaluate the reviewed default/typed owner
candidates, including the common root-font owner and ZenUML default CSS font, while preserving
all successful/error/catalog controls and resource policy.
