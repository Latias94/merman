# C4 Terminal Support

Status: supported subset (`Partial`, diagrammatic primary).

The `diagram-c4` feature renders `C4Context`, `C4Container`, `C4Component`, `C4Dynamic`, and
`C4Deployment`. Canonical and full render models share
`merman_model::families::c4::C4DiagramModel`; `AsciiDiagram::C4` borrows that model directly.
C4 owns hierarchy validation and declaration-order placement, then uses the shared positioned
graph router and encoders. It does not translate C4 through Flowchart or reorder it with Dagre.

## Overview and complete records

The overview shows keyed, role-labelled rectangles for people, systems, containers, components,
and their external, database, and queue variants. Boundaries, including deployment nodes, use
nested frames. Shape headings contain the key, role, and name; boundary headings contain only
the key and name. Technology, environment type, and descriptions stay in complete records below
the overview rather than repeating inside nested titles. This keeps deployment frames shorter
and reduces routing work without dropping detail.

Within each scope, direct shapes precede child boundaries. Each collection keeps declaration
order and uses its own `c4ShapeInRow` or `c4BoundaryInRow` capacity. These values count items per
row, not rows. Terminal dimensions follow measured labels and nested frames rather than browser
pixel metrics. Relationships do not repack declarations, and a viewport does not change the
authored row capacities. The synthetic `global` frame is hidden in the overview but retains its
own B record and membership references.

Records use one-based `S1...` shape, `B1...` boundary, and `E1...` relationship keys in each
collection's order. Aliases identify endpoints independently of display names, so equal names
remain distinct. Endpoint lookup checks shapes before boundaries; a shape/boundary alias overlap
retains both declarations and discloses the shadowed boundary. Parent lookup uses boundaries only.
Visible boundaries are real relationship endpoints rather than aliases for one of their members.
Known view discriminators use one-to-one reading names such as `C4 view="Context"`; an unknown
`c4Type` stays verbatim in raw details. Authored labels and metadata strings are not rewritten.

Routes carry short `E1` keys; Dynamic routes carry labels such as `1: E1`. The matching records
retain complete authored labels, technologies, descriptions, endpoint aliases, and arrow ownership.
Dynamic numbers follow the final relationship collection order. Repeating an ordered source/target
pair updates its existing record in place. `RelIndex` discards its authored index, so numbering
follows the final list rather than that argument. Records describe the final typed model rather
than a history of overwritten values.

| Relationship | Arrow ownership |
| --- | --- |
| `Rel_Back` / `rel_b` | Source head only; authored source and target remain unchanged |
| `BiRel` / `birel` | Independent source and target heads |
| `Rel` and direction-suffixed relations | Target head only |

`Rel_U`, `Rel_D`, `Rel_L`, and `Rel_R` retain their relation kinds but do not force cardinal ports:
the pinned C4 renderer computes contacts from element geometry without those side constraints.

Complete records also retain raw layout, wrap, accessibility, source title/configuration, style,
sprite, tag, link, legend, offset, and additional serialized metadata. String and wrapped-string
forms remain distinguishable; other JSON values stay intact in details. Browser silhouettes,
assets, fonts, authored colors, and pixel offsets are not painted by the terminal overview.
Standard terminal encodings do not imply browser-style parity.

## StructuredText and error boundaries

Detached models without an honest spatial interpretation use a complete **StructuredText primary**.
This includes non-text labels or typed text fields, unknown view/role/relation kinds,
empty spatial inventories, empty or duplicate aliases within a collection, unresolved parents or
endpoints, boundary cycles, and relationships to the hidden `global` frame. A shape and boundary
sharing an alias alone is not invalid because shape-first lookup is defined.
The hidden `global` boundary must be a root; a nested transparent global scope uses records so
visible containment and route ownership cannot disagree.

Row capacities must be finite positive integers. Zero, negative, fractional, or non-finite
detached values remain inspectable records rather than invented geometry. Large valid capacities
do not allocate empty column grids; actual geometry still obeys the resource policy.

The report identifies these record-only primaries as `structured_text`, with `outcome: primary`
and no attempted fallback when they fit. They preserve ambiguous or unresolved references instead
of merging declarations or dropping fields. This classification does not turn parse errors or
operation failures into successful output.

C4 admits Canonical layout and Strict graph presentation, both `unicode` and `cjk` width profiles,
and Plain, ANSI16, ANSI256, TrueColor, and HTML primary encodings. ASCII and Unicode structural
charsets share these semantics; the CJK width profile uses the renderer's single-cell structural
fallback. Compact, Auto, and Adaptive are not admitted.

Allow retains a complete wide primary; Error returns `WidthOverflow`. Plain viewport Fallback
can reflow the complete keyed records after a valid primary exceeds the width bound, or report
`FallbackUnavailable` if they cannot fit. Width is measured over the overview and records
together. Styled viewport fallback is rejected before rendering, even for an input that fits.

Strict routing and port-capacity failures remain errors; width fallback cannot bypass them.
Resource/allocation failures, cancellation, and deadlines also remain errors. Hierarchy validation,
placement, routing, records, and encoding share the caller's cumulative operation allowance.
There is no C4-specific budget increase or fresh allowance for a later candidate. Dense fixed-row
scenes remain best effort rather than a universal spatial-coverage guarantee.

## Sources and verification owners

The selected Mermaid revision is owned by
[`tools/upstreams/REPOS.lock.json`](../../../tools/upstreams/REPOS.lock.json). The Mermaid 12.3.0
C4 sources establish the semantic boundary:

- [`c4Db.ts`](https://github.com/mermaid-js/mermaid/blob/2086904e76c692331e34a45d7cc3bc30860d040e/packages/mermaid/src/diagrams/c4/c4Db.ts):
  ordered-pair relationship updates, row capacities, and shape-first boundary-aware lookup.
- [`c4Renderer.ts`](https://github.com/mermaid-js/mermaid/blob/2086904e76c692331e34a45d7cc3bc30860d040e/packages/mermaid/src/diagrams/c4/c4Renderer.ts):
  shapes before child boundaries, hidden global frame, geometry-based contacts, and Dynamic order.
- [`svgDraw.ts`](https://github.com/mermaid-js/mermaid/blob/2086904e76c692331e34a45d7cc3bc30860d040e/packages/mermaid/src/diagrams/c4/svgDraw.ts)
  and [`c4Diagram.jison`](https://github.com/mermaid-js/mermaid/blob/2086904e76c692331e34a45d7cc3bc30860d040e/packages/mermaid/src/diagrams/c4/parser/c4Diagram.jison):
  arrow ownership, relation kinds, and discarded `RelIndex` indices.

Local implementation owners are [`src/c4`](../../../crates/merman-ascii/src/c4/mod.rs) and
[`output/c4.rs`](../../../crates/merman-ascii/src/output/c4.rs).
[`c4_model.rs`](../../../crates/merman-ascii/tests/c4_model.rs) owns parser-backed views, ordering,
nesting, identity, markers, raw records, width/encoding behavior, resource limits, and cancellation;
source specimens live under [`fixtures/c4`](../../../fixtures/c4/).
Neither the copied nor inspected moving `mermaid-ascii` supplies a C4 renderer, byte oracle, or
quality oracle. See the [reference comparison](ASCII_REFERENCE_COMPARISON.md) and
[support matrix](../ASCII_SUPPORT_MATRIX.md) for the shared product boundary.
