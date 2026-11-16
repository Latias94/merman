# ASCII Resource and Panic-Path Audit

Status: tracked closeout audit for production source
`861fc0ba33ac6f0a724263b3a6f303a3f26eee15`; package verification recorded in the canonical receipt
Last updated: 2026-08-22

Packet spatial admission extension: 2026-11-16. Its row below and current family tests supplement
the historical closeout; they do not change the source identity of that receipt.

Static graph protection extension: 2026-11-16. The spatial query bounds and current tests below
supplement the same historical receipt.

Indexed channel adjacency extension: 2026-11-16. Its search-state and work bounds below also
supplement the historical receipt.

Scene planning extension: 2026-11-16. Conflict-directed repair, measured reflow, and explicit
adaptive presentation use the same public operation ledger and resource ceilings. The scene
planning section below supplements the historical receipt.

This audit records the input-amplification and panic-path boundary reviewed for the ASCII
semantic-depth closeout. It is not a claim that every Rust allocation is fallible or that the crate
contains no invariant assertions. It identifies where authored-size work is admitted, where large
materialization starts, and how remaining invariants are constrained. The source review in this
document does not replace the exact package and Clippy gates recorded in `ASCII_U25_CLOSEOUT_RECEIPT.md`.

## Resource Matrix

| Owner | Admission before large materialization | Fallible allocation boundary | Executable evidence |
| --- | --- | --- | --- |
| Shared terminal text and encoders | `AsciiResourcePolicy` checks source/model/layout/grid/document/output limits; `LogicalExtent` uses checked arithmetic before canvas and encoded output construction. | Authored grapheme arenas, styled rows, canvases, and output buffers use checked sizes and `try_reserve` on the audited paths. Small fixed metadata values may still use ordinary stack or bounded collection construction. | `resource::tests`, `text::tests`, `canvas::tests`, `terminal_text_safety`, facade exact-boundary tests. |
| Flowchart and State graph projection | `GraphGroupTopology::try_new` charges topology work before maps; layout/routing charge rank, candidate, occupancy, and route work; `graph::draw` checks the final grid before canvas allocation. State charges typed projection work before using the shared graph path. | Graph nodes, edges, groups, topology maps, route candidates, and occupancy owners use `try_reserve` helpers after admission. | `nested_topology_construction_accepts_exact_work_and_rejects_max_minus_one`, `graph_grid_limit_accepts_exact_extent_and_rejects_max_minus_one`, scene/route exact-minus-one tests, and State projection exact-minus-one tests. |
| Sequence | Typed-model validation precedes layout; `SequenceBatchExtent` and `SequenceExtentLedger` admit row, grid, document, and output totals before control, box, note, and message row batches are retained. | Participant/message/box indices and retained row batches reserve fallibly; event and control materializers validate their planned extents before painting. | `extent_ledger_accepts_exact_grid_and_document_limits`, `extent_ledger_rejects_limit_minus_one_with_exact_aggregate_counts`, control/box aggregate tests, self-message exact-geometry tests, and facade resource tests. |
| Class and ER | Family adapters measure boxes and relation descriptors first. `RelationStackPlan`, `RelationParallelPlan`, `RelationSelfLoopPlan`, and the document plan check aggregate extents before invoking row materializers. | Entity/class boxes, relation components, lane descriptors, summaries, and final rows use `try_reserve`; materializer-closure tests prove N-1 rejection occurs before painting. | Relation stack/parallel/self-loop/document exact and N-1 tests, `horizontal_class_strip_checks_grid_and_layout_work_before_allocating_rows`, and the ER counterpart. |
| XYChart | Axis/series planning derives a complete plot/document extent before allocating topology cells or final rows; final document grid and encoded output limits are checked independently. | Plot samples, labels, cells, rows, and disclosures use fallible reserves tied to checked counts. | Complete vertical/horizontal grid exact-minus-one tests, output-byte exact-minus-one tests, and `xychart_layout_work_budget_covers_series_values_categories_and_paint`. |
| Packet | Numeric admission precedes coordinate-to-count conversion. Complete physical row grid/work bounds are checked before the row plan; exact labels and metadata use shared budgeted document writers. Spatially ineligible values remain records, while resource/cancellation errors propagate. | The complete row plan, per-row spans, and label descriptors reserve fallibly. Field geometry is independent of label length; numeric records use bounded number spelling. | `packet_grid`, `packet_numeric_parity_test`, `packet_grid_work_and_document_usage_accept_exact_and_reject_one_below`, and Packet field-framing compatibility tests. |
| StructuredText families | Family validators charge authored records and text before section/group indices and output rows; shared encoders enforce document/output limits. | Gantt, sectioned Journey/Timeline, Mindmap, Kanban, TreeView, and GitGraph use fallible reserves for authored-size indexes and rows on their audited paths. | `new_family_models`, `sectioned_structured_text`, terminal safety, and per-family resource boundary tests in the ASCII package gate. |
| Facade and bindings | Source/model limits are applied before parse/model handoff; typed ASCII errors preserve the limit id, phase, actual, maximum, and profile through public adapters. | The facade does not replace a renderer resource error with an unbounded compatibility allocation. | Facade exact-boundary tests such as `ascii_request_honors_target_local_grid_policy_at_exact_and_minus_one`, binding metadata/resource tests, UniFFI/WASM tests, and CLI diagnostic tests. |

The audit explicitly permits bounded or input-independent ordinary allocations. It rejects
unchecked allocations whose size is directly amplified by authored nodes, edges, labels, rows, or
grid extents. Parser-owned allocations before the typed ASCII model are governed by the shared
source/model resource policy rather than reimplemented in this crate.

## Static Graph Protection

The routing scene compiles node rectangles (including their reading margin), group border spans,
and title regions into a centroid-split bounding hierarchy. Congestion trials share this immutable
protection while rebuilding their dynamic route, marker, port, and label occupancy independently.
Point and reservation queries retain the original exact semantic predicates after conservative
spatial filtering. No terminal-cell raster of the protected area is allocated.

For `P` protected regions, median construction takes `O(P log P)` work and retains `2P - 1`
hierarchy nodes for nonempty input, plus the original geometry. Temporary partition entries and
retained storage are `O(P)` and use checked counts and fallible reserves. Construction and query
visits charge the shared layout-work ledger and observe cancellation. Reordering shares protection
without rebuilding or charging its already completed construction again.
The preparation has a fixed cost even for tiny scenes: the unchanged two-node Binding Core work
fixture intentionally moves from 901 to 950 units; 950 succeeds and 949 rejects with actual 950.
This is accounting for changed work, without increasing any profile ceiling.

Queries visit intersecting hierarchy branches with no per-query allocation. This removes the
previous unconditional scan of all `P` regions for every route-cell query; a query's worst case
remains `O(P)`. Overlapping or poorly separable bounds can still require a complete traversal.
In the registered separated-row regression, 8/64/512/4096 protected regions require 7/13/19/25
tree-node visits for one point query. Fully overlapping inputs visit all `2P - 1` nodes and retain
every original contributor. These are deterministic work observations, not latency measurements.

`graph::routing::occupancy::protected::tests` compares indexed queries with full scans across
rectangles, borders, title rows, reading margins, overlaps, and large coordinates. It also checks
long-region partitioning, exact/N-1 query work, construction admission, and cancellation.
Congestion rebuild tests verify that trials share the same immutable protection object while
preserving independent dynamic occupancy. Remaining channel-coordinate and group-crossing scans
are outside this optimization.

## Indexed Channel Adjacency

Terminal search retains an implicit orthogonal graph of sorted critical-coordinate axes.
An admitted port-pair search resolves its initial berth into axis indices once, after terminal
preflight. Frontier entries, costs, and parent links retain those indices and the arrival direction.
Adjacent channels use checked index increments/decrements and bounds-checked coordinate lookup.
Physical coordinates still determine distances, collision predicates, frame crossings, and costs;
heap priority and insertion-sequence ties are unchanged. Winning parent links are expanded into
the same terminal-cell path before route admission.

For maximum axis length `C`, `A` port-pair searches reaching the frontier, and `S` expanded states,
adjacency lookup changes from `O(S log C)` to `O(A log C + S)`. This bound covers coordinate
resolution and neighbor selection only. Segment inspection still visits traversed terminal cells,
and frontier ordering, port-pair enumeration, labels, and congestion repair have their own costs.
It does not establish a complete-render latency improvement or routing completeness.

The index position uses two `usize` components and a direction, matching the physical-coordinate
position's retained size. No Cartesian grid, coordinate-to-index map, or per-query allocation is
added. The initial binary searches charge each comparison; each neighbor request charges one work
unit, including a boundary miss. Both observe cancellation. Existing cumulative work ceilings,
search-expansion allowances, and error policy remain unchanged.

`graph::routing::terminal::channels::tests` compares indexed neighbors with geometric neighbors
on sparse and near-maximum axes, rejects missing coordinates, and covers empty/singleton axes,
constant adjacency work, exact/N-1 limits, cancellation, and state storage size. For axis lengths
8/64/512/4096, 512 neighbor queries each charge 512 units; initial coordinate resolution is measured
separately. Existing compressed/unit-grid oracle tests retain path quality and segment semantics,
including markers, labels, compound frames, long gaps, and winner materialization.

## Graph Scene Planning

`graph::planning` prepares stable semantic demand and route identities once for each admitted
layout profile. It owns the initial geometry and at most two measured capacity rebuilds. Every
generation recreates coordinate-dependent contacts, channels, and occupancy; immutable protection
is shared only between trials within one generation. Retired geometry is dropped before the next
generation is built.

Feasibility, displaced-route replacement, prospective allocation, complete-scene quality, and
reflow have separate finite algorithmic allowances. They all charge the cumulative public work
ledger and observe the caller's cancellation/deadline. These scheduling allowances are not public
resource ceilings and do not make a difficult graph unlimited under the trusted-unbounded policy.
Optional quality begins with a validated complete incumbent; exhausting that allowance retains
the incumbent. The planner never raises a public ceiling to make a candidate succeed.

Strict feasibility retains the finite channel-state search and the caller's public work ceiling.
Adaptive feasibility additionally stops speculative search early enough to leave room for complete
document recovery. Deterministic candidate construction and route commits always charge the public
ledger; they do not consume that adaptive search allowance. A long, directly routable chain is not
rejected merely because constructing its earlier routes used more than a fixed search-work slice.
The existing 200-node compound-chain benchmark remains a strict admission control.

Rejected route, port, marker, and label reservations record at most 64 distinct canonical blockers.
The witness records static/capacity conflicts and truncation separately; it does not certify global
impossibility. Repair expands to all semantic members of affected shared channels and validates the
entire replacement before committing. Failed trials restore the incumbent without allocating or
replaying cancellation-sensitive work. Work already spent remains charged. Reflow uses a private
work boundary checked before each charge; public resource errors in all dimensions and cancellation
take precedence over that boundary. Discarded reflow geometry restores speculative document cells
while retaining the work spent to build it.

The unchanged two-node Binding Core fixture now needs 954 work units (953 rejects with actual
954), following the earlier static-protection checkpoint of 950. The structured resource-boundary
fixture and exact/N-1 transport tests own this count; no profile ceiling changed. Core tests cover
private-boundary precedence, failed transactions, useful replacement credit for wide contacts,
four-contributor repair, and a 97-marker capacity deficit corrected through reflow.

Adaptive presentation catches only admitted `RoutingSearchExhausted` and `PortCapacityExceeded`
misses after semantic construction. It uses the existing complete Plain graph-document writer;
allocation failures, public resource exhaustion, cancellation, and unsupported semantics still
return errors. Its text construction and encoding retain the same document/output admission.
Both fixed dense sources in `tests/scene_planning_corpus.rs` complete at 80 cells under the
unchanged 8,000,000-unit Trusted Native work ceiling. This is a deterministic admission result,
not a latency or arbitrary-graph routing guarantee.

## Panic and Invariant Review

The closeout search was:

```text
rg -n --glob '*.rs' 'panic!|todo!|unimplemented!|unreachable!|\.unwrap\(|\.expect\(' \
  crates/merman-ascii/src
```

Disposition:

- `panic!` calls found in ASCII source are in `#[cfg(test)]` modules and fixture helpers. They are
  assertions about the test harness, not public authored-input paths.
- Production `unreachable!` sites are limited to locally established invariants: directions have
  already been canonicalized before one-axis layout, routing helpers receive only their declared
  axis, and normalized safe-text line breaks are handled before segment matching. Direction,
  compound-route, mirror, and safe-text tests exercise those transitions.
- `SequenceParticipantLabel::try_from_raw` contains an `expect` after requesting non-trimmed
  normalized lines. The normalizer contract always returns at least one row in that mode, including
  for empty input; the expectation is not derived from an unchecked model index.
- Test-only convenience constructors retain `unwrap`/`expect` so a violated fixture invariant fails
  loudly. They are excluded from production builds by their enclosing `#[cfg(test)]` owner.
- No `todo!` or `unimplemented!` remains on a production ASCII dispatch path. Unsupported families
  and unsupported semantics return typed errors instead.

The audit did not execute an allocator-failure injector and does not claim recovery from operating
system abort-on-OOM behavior. It establishes checked authored extents, fallible collection growth
on audited amplification paths, and absence of known authored-input panic branches. The current
package tests and strict Clippy results are recorded in the canonical receipt; they must be rerun
after any later production change.

## Residuals

- Small temporary strings and fixed-size descriptors can be materialized before a final document
  extent check when their size has already been admitted by source/text work.
- Third-party parser, Unicode segmentation, and standard-library internals are outside this
  crate-local panic audit; their inputs remain bounded by the public source/model policies.
- New families must add a row to this matrix and exact/N-1 evidence before admission.
