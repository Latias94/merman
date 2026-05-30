# ASCII Flowchart Direction Transform - Handoff

Status: Closed
Last updated: 2026-05-30

## Current State

The flowchart ASCII renderer accepts BT and RL root directions through render-layer output
transforms. Parser-backed BT/RL contracts are green, LR and TD golden outputs remain stable, public
support docs describe the shipped subset, and the lane is closed.

## Active Task

None. This workstream is closed.

## Decisions Since Last Update

- BT and RL are the only root directions in scope.
- Root-direction transforms belong in the ASCII flowchart rendering path, not in `merman-core` or
  the parser.
- Subgraph direction overrides, color/style roles, and state diagrams remain out of scope.
- AFDT-020 added expected ASCII output for `flowchart BT\nA --> B` and `flowchart RL\nA --> B`.
- AFDT-030 added `GraphDirection::BottomTop` and `GraphDirection::RightLeft`, then canonicalized
  layout/routing to TD/LR and mirrored the final graph canvas in the ASCII render layer.
- Node labels, edge labels, and subgraph titles are overlaid after transforms so mirrored text stays
  readable.
- Unsupported direction coverage now uses a direct model direction of `XX` and reports
  `unsupported graph directions`.
- AFDT-040 updated `FLOWCHART_SUPPORT.md`, `crates/merman-ascii/README.md`, and the root `README.md`
  so public docs list BT/RL root directions as shipped.

## Blockers

- None.

## Follow-Ons

- Subgraph direction overrides from `FlowSubgraph.dir`.
- ANSI/HTML color roles and `classDef`/`class`/inline style rendering.
- State diagram graph rendering through a state-to-graph semantic adapter.
- Multiline subgraph labels.
- Additional uncommon flowchart shape families.
