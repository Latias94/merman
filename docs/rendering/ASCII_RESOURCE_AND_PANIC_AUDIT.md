# ASCII Resource and Panic-Path Audit

Status: tracked closeout audit for production source
`861fc0ba33ac6f0a724263b3a6f303a3f26eee15`; package verification recorded in the canonical receipt
Last updated: 2026-08-22

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
| StructuredText families | Family validators charge authored records and text before section/group indices and output rows; shared encoders enforce document/output limits. | Gantt, sectioned Journey/Timeline, Mindmap, Kanban, TreeView, Packet, and GitGraph use fallible reserves for authored-size indexes and rows on their audited paths. | `new_family_models`, `sectioned_structured_text`, terminal safety, and per-family resource boundary tests in the ASCII package gate. |
| Facade and bindings | Source/model limits are applied before parse/model handoff; typed ASCII errors preserve the limit id, phase, actual, maximum, and profile through public adapters. | The facade does not replace a renderer resource error with an unbounded compatibility allocation. | `headless_ascii_renderer_proves_every_ascii_limit_at_exact_boundary`, binding metadata/resource tests, UniFFI/WASM tests, and CLI diagnostic tests. |

The audit explicitly permits bounded or input-independent ordinary allocations. It rejects
unchecked allocations whose size is directly amplified by authored nodes, edges, labels, rows, or
grid extents. Parser-owned allocations before the typed ASCII model are governed by the shared
source/model resource policy rather than reimplemented in this crate.

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
