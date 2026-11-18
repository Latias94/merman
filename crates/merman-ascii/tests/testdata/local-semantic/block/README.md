# Block semantic fixtures

These parser-backed fixtures characterize terminal grid semantics rather than browser pixels.
`small` has a source-ordered two-column grid and one edge; `typical` combines a spanning header,
nested frame, cylinder shapes, a local two-column grid, and two already-expanded spacer records.
`dense` keeps four authored columns across four rows with independent vertical edges. `cjk`
exercises full-width text, Greek letters, and ambiguous middle dots in nested grids.

The test matrix uses viewports 80, 100, and 120 with ASCII/Unicode structural choices and
Unicode/CJK text measurement. Small and typical fixtures must remain spatial. Block uses
Canonical/Strict: dense routing may return a bounded search/capacity diagnostic. Only a
complete valid spatial scene exceeding a Plain viewport can select the existing width
fallback policy and produce a complete semantic document. A route miss does not select a
reading document. Neither projection may reorder records or silently drop edges.

Pinned evidence: `repo-ref/mermaid/packages/mermaid/src/diagrams/block/layout.ts`
(`calculateBlockPosition`, `layoutBlocks`) and `blockDB.ts` (`populateBlockDatabase`). Source
fixtures under `fixtures/block/` cover auto columns, spans overflowing a row, expanded spaces,
composite spans, edge markers, and arrow-shaped blocks.
