# HPD-070 - Unsupported Family Rubric

Date: 2026-06-02

## Context

The implemented Mermaid 11.15 matrix is structurally green, but unsupported upstream families were
still listed as a flat deferred bucket. HPD-070 needed to turn that bucket into an admission policy
and source-backed priority order without starting a new implementation lane.

## Source Checks

Checked locked Mermaid commit `41646dfd43ac83f001b03c70605feb036afae46d`:

- `packages/mermaid/src/diagrams` contains `eventmodeling`, `ishikawa`, `treeView`, `venn`, and
  `wardley`.
- It does not contain `railroad` or `cynefin`.
- `diagram-orchestration.ts` lazy-registers detectors for the five pinned unsupported families.
  `treeView` is the upstream diagram id/config key; its detector accepts `treeView-beta`.
- Syntax docs exist for all five pinned unsupported families under `packages/mermaid/src/docs/syntax`.
- Diagram-local tests exist for `eventmodeling`, `ishikawa`, `venn`, and `wardley`; `treeView` has
  syntax docs but no diagram-local spec file in the pinned source tree.

## Outcome

- Added `docs/alignment/UNSUPPORTED_FAMILY_ADMISSION_RUBRIC.md`.
- Updated `docs/alignment/STATUS.md` so unsupported-family source status is pinned-baseline aware.
- Marked HPD-070 done in the workstream ledger.
- Priority order:
  1. `treeView-beta` header / `treeView` id
  2. `ishikawa` / `ishikawa-beta`
  3. `eventmodeling`
  4. `venn-beta`
  5. `wardley-beta`
- `railroad-*` and `cynefin-beta` are not Mermaid 11.15 backlog items because they are absent from
  the pinned source tree.

## Notes

The rubric deliberately does not add any detector, parser, renderer, fixture, or compare command.
It only defines how a future family workstream becomes eligible. In particular, `venn` needs a
source-backed `@upsetjs/venn.js` layout plan before implementation, and `wardley` should be treated
as a large standalone family lane.
