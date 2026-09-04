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
| Mindmap | yes | legacy bridge |
| State | yes | legacy bridge |
| Sequence | yes | legacy bridge |
| ZenUML | yes | legacy bridge |
| Flowchart | yes | legacy bridge |
| Swimlane | yes | legacy bridge |
| Architecture | yes | legacy bridge |
| Class | yes | legacy bridge |
| C4 | yes | legacy bridge |
| Cynefin | yes | legacy bridge |
| Wardley | yes | legacy bridge |
| Railroad | yes | legacy bridge |
| Kanban | yes | legacy bridge |
| Gantt | yes | legacy bridge |
| Pie | yes | legacy bridge |
| Packet | yes | canonical |
| Timeline | yes | legacy bridge |
| Journey | yes | legacy bridge |
| Requirement | yes | legacy bridge |
| Sankey | yes | legacy bridge |
| Radar | yes | legacy bridge |
| Info | yes | canonical |
| Treemap | yes | legacy bridge |
| Block | yes | legacy bridge |
| ER | yes | legacy bridge |
| QuadrantChart | yes | legacy bridge |
| XYChart | yes | legacy bridge |
| GitGraph | yes | legacy bridge |
| TreeView | yes | legacy bridge |
| Ishikawa | yes | legacy bridge |
| EventModeling | yes | legacy bridge |
| Venn | yes | legacy bridge |

This is an honest migration snapshot, not a release claim.  A family may move from `legacy-bridge`
to `canonical` only after a focused SVG parity fixture proves that root geometry, style, DOM/a11y
obligations, and effect disposition remain source-backed.  Unsupported browser-only effects must
remain explicit DrawingList errors or bounded raster fallbacks; they must not be hidden by the
bridge.
