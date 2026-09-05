# DrawingList family coverage

`fixtures/drawing-list/v1/family-coverage.json` is the machine-checked inventory for the
renderer-neutral document migration.  It deliberately distinguishes two facts that are easy to
confuse:

- `document_adapter: direct` means the family builds a typed `RenderDocument` without parsing SVG.
- `svg_serializer: canonical` means the SVG target currently serializes that document directly.
  `legacy-bridge` is a temporary, explicit migration status; it does not mean that the family is
  missing DrawingList output.

The coverage test compares the fixture with `RenderFamilyKind::ALL`, so adding a family without an
inventory row fails at test time.  The current inventory is:

| Family | Direct document adapter | SVG serializer |
| --- | --- | --- |
| Error | yes | canonical |
| Mindmap | yes | canonical |
| State | yes | canonical |
| Sequence | yes | legacy bridge |
| ZenUML | yes | canonical |
| Flowchart | yes | legacy bridge |
| Swimlane | yes | legacy bridge |
| Architecture | yes | canonical |
| Class | yes | legacy bridge |
| C4 | yes | canonical |
| Cynefin | yes | canonical |
| Wardley | yes | canonical |
| Railroad | yes | canonical |
| Kanban | yes | canonical |
| Gantt | yes | canonical |
| Pie | yes | canonical |
| Packet | yes | canonical |
| Timeline | yes | canonical |
| Journey | yes | canonical |
| Requirement | yes | canonical |
| Sankey | yes | canonical |
| Radar | yes | canonical |
| Info | yes | canonical |
| Treemap | yes | canonical |
| Block | yes | canonical |
| ER | yes | canonical |
| QuadrantChart | yes | canonical |
| XYChart | yes | canonical |
| GitGraph | yes | canonical |
| TreeView | yes | canonical |
| Ishikawa | yes | canonical |
| EventModeling | yes | canonical |
| Venn | yes | canonical |

This is an honest migration snapshot, not a release claim.  A family may move from `legacy-bridge`
to `canonical` only after a focused SVG parity fixture proves that root geometry, style, DOM/a11y
obligations, and effect disposition remain source-backed.  Unsupported browser-only effects must
remain explicit DrawingList errors or bounded raster fallbacks; they must not be hidden by the
bridge.

The table is a family-level default, not a promise about every request.  The typed SVG result
exposes `SvgSerializationRoute`: `canonical-document` means the result was serialized from the
renderer-neutral document, while `legacy-bridge` records an explicit compatibility path selected
for a legacy family, a browser-only effect, or a diagnostic request.  This distinction is useful
for migration telemetry and prevents an effect-specific fallback from being mistaken for complete
canonical coverage.

## Exercised effect accounting

`fixtures/drawing-list/v1/effect-coverage.json` is the focused effect evidence used by
`drawing_list_effect_accounting`.  It covers the visual constructs currently exercised by admitted
fixtures: portable path paint, host text, gradients, clips, semantic links, opacity, blend modes,
transforms, and expanded marker geometry, plus explicit fail-closed outcomes for browser-wrapped
text, filters, hand-drawn RoughJS output, and external icon registry content.  Each row is executed
against the typed renderer and must produce its declared vector or structured-error disposition;
no row may be an unclassified best effort.  The test also collects the effect kinds actually
observed across successful fixtures and fails when one has no corresponding assertion, so an
adapter cannot start emitting an already-modeled visual command without extending the evidence.

The matrix intentionally does not claim that every protocol resource kind is emitted by a current
family.  Patterns, inline images, glyph/outline text, and raster subtrees remain protocol-level
capabilities with dedicated display-list validation fixtures until a family admits them through a
source-backed renderer slice.
