# Block ASCII Support

Status: supported subset (`Partial`, diagrammatic primary).

The `diagram-block` feature renders the typed
`merman_model::families::block::BlockDiagramModel` through the canonical parsed or
borrowed-model renderer. Block owns the flat hierarchy validation, grid sizing,
and placement. It never sends Block geometry through Dagre or graph reflow.
`graph::positioned` keeps the shared graph model and layout private and passes
fixed terminal geometry to the existing validated route, viewport admission,
canvas, and encoder path.

## Grid semantics

The semantic root index identifies an unlabelled composite. Each composite uses
its own authored `columns`; an omitted count or `-1` means one automatic row.
Positive integral counts advance through columns in child insertion order.
Source line breaks do not create rows. A child's `width_in_columns` reserves a
positive integral span, defaulting to one. Spans wider than the remaining row
retain their full visual width. They consume the remaining logical positions,
so the next child begins on the next row. Blocks are never reordered or wrapped
to fit a requested terminal width.

Sibling terminal columns share an integer base width derived from their measured
labels and nested frames. Multi-column blocks include the intervening gutters.
Rows reserve the height of their tallest child. Composite frames contain the
complete child grid, with a separate title band when the model supplies a label.
Terminal widths and padding are discrete; SVG text metrics and pixel equality
are outside this projection.

The parser already expands `space:N` into N distinct hierarchy records. Each
record reserves one default column; its retained `width` is metadata and is not
expanded again. Unattached space templates kept by Mermaid's database are
accepted and remain present in the complete reading document.

These dispositions follow pinned Mermaid 12.3.0 source:

- `repo-ref/mermaid/packages/mermaid/src/diagrams/block/layout.ts`, particularly
  `calculateBlockPosition` and `layoutBlocks`, defines automatic columns,
  authored ordering, and the capped logical consumption of overflowing spans.
- `repo-ref/mermaid/packages/mermaid/src/diagrams/block/blockDB.ts`, particularly
  `populateBlockDatabase`, defines spacer expansion and repeated declaration
  updates. `setCssClass` can replace a database binding while leaving the
  previously attached hierarchy object unchanged.
- `fixtures/block/upstream_cypress_block_spec_bl30_block_should_overflow_if_too_wide_for_columns_030.mmd`
  is a direct regression probe for row overflow. Parser-backed tests also cover
  ordinary redeclarations and the ambiguous spaced-class rebinding case.

## Shapes and relationships

The shared terminal shapes cover square/default blocks, rounded rectangles,
circles, double circles, stadiums, subroutines, cylinders, diamonds, hexagons,
leaning rectangles, and both trapezoids. The inverse-arrow shape uses an explicit
`[rect_left_inv_arrow]` role tag. Block arrows use `[block-arrow:...]` with every
authored direction, so their rectangular approximation is visible.

Edges preserve source and target, both endpoint markers independently, labels,
and normal, thick, or dotted strokes. Open or absent markers remain unmarked.
The fixed scene uses shared route validation and marker/label ownership checks.
Block admits Canonical layout and Strict graph presentation. If bounded routing
cannot produce a truthful scene, the routing or capacity diagnostic is returned.
Only a complete valid spatial scene that exceeds a Plain viewport may select
the existing width fallback policy and emit a complete reading document. There
is no adaptive route fallback or geometry-relaxing retry. Simultaneously thick and
dotted strokes, unknown shapes, unknown directions, and unknown markers receive
explicit unsupported-feature diagnostics.

## Invalid models and resource admission

Missing or invalid roots, invalid child indices, non-composite parents, cycles,
repeated child membership, multiple parents, duplicate database IDs, unreachable
non-spacer records, missing edge endpoints, and edges to invisible root/spacer
geometry fail before shared routing. Database IDs are not silently merged: a
later binding can differ from the object retained by the hierarchy. That
ambiguity requires an explicit diagnostic rather than selecting an arbitrary
visible object.

Zero, fractional, negative, or non-finite spans are rejected. Columns accept
`-1` or a positive integer; zero, other negative numbers, fractions, and
non-finite values are rejected. The pinned layout divides by spans and requires
nonzero integral column counts, so these inputs have no stable finite terminal
grid interpretation. Huge finite counts fail the existing grid resource policy
before conversion or allocation. Float-to-integer conversion never relies on
Rust's saturation behavior.

The complete borrowed raw-text batch is admitted before projection copies.
Borrowed deferred label plans are retained through aggregate admission of normalized
display cells, raw/normalized label bytes, identity copies, and source-copy work.
The source-copy work is charged once before the complete graph source batch is
materialized. Neither repeated label scans nor normalized-row replay runs between
those copies. Nodes and frame titles retain their plans; terminal rows are replayed
after source copies and fixed packing, before routing and painting. A source-copy
allowance does not promise that later row replay, layout, routing, or painting will fit.

Same-row shape annotations first use a separately admitted intermediate assembly.
Its exact aggregate byte/copy-work allowance is checked before assembling any
annotation. Final plans borrow those complete annotated labels, and the still-live
intermediate bytes are included in the final source batch allowance. The visible
annotation text, label breaks, shape metrics, and authored geometry are unchanged.
IDs, metadata collections, empty strings, child links, depth,
integer dimensions, and packing passes are metered. Packing uses O(records +
child links) storage and arithmetic spans; it never allocates an occupancy array
whose length comes from a column/span value. Cancellation and resource failures
retain the caller's operation. Recoverable viewport and route misses preserve
spent work without committing speculative document cells.

## Complete reading document

Plain width fallback uses `B0...` keys for every flat record, `E0...` for authored
edges, `C0...` for class definitions in insertion order, and `W0...` for warning
facts. Root and children reference flat keys, preserving the exact hierarchy
without recursively duplicating descendants. Records retain IDs, types, labels,
columns, spans, spacer expansion metadata, class/style/direction arrays,
relationship IDs and endpoints, both marker fields, thickness and pattern.
Source configuration/title and future envelope fields remain complete borrowed
JSON details. No generic `blocks_flat[i].children[j]` field-flattening expansion
is used. The shared writer quotes terminal text safely and wraps complete
records using the requested Unicode or CJK width profile.

## Evidence

`tests/block_model.rs` covers small, typical, dense and CJK parser-backed fixtures
at 80/100/120 columns in ASCII/Unicode and both width profiles; source order,
spans, spacers, nesting, marker/stroke distinctions, visible shape roles,
malformed direct models, exact output-byte admission, resource limits,
cancellation, and complete narrow fallback. Family unit tests cover aggregate
raw-label rejection, exact versus one-below normalized display-cell quotas,
and exact source-copy work admission for plain and annotated/nested/edge-label
batches before row replay. A one-below work allowance rejects before the first
projected source; an exact source-copy allowance may still exhaust during the
distinct row replay phase. Annotation assembly also has an exact/one-below
aggregate byte boundary. Positioned-graph unit tests compare all admitted shape
sizes and replayed labels with the existing eager text/shape rules, including
both annotation forms, hard breaks, CJK/emoji, empty frame titles, and unchanged
fixed coordinates. Fixtures live in
`tests/testdata/local-semantic/block/`.

Main integration runs these checks serially with the workspace's normal target:

```text
cargo nextest run -p merman-ascii --features diagram-block --test block_model
cargo nextest run -p merman-ascii --features diagram-block --lib -E 'test(block::)'
```
