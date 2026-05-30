# ASCII Flowchart Direction Transform - Handoff

Status: Active
Last updated: 2026-05-30

## Current State

The flowchart ASCII renderer now accepts BT and RL root directions through render-layer output
transforms. Parser-backed BT/RL contracts are green, LR and TD golden outputs remain stable, and the
old unsupported-direction coverage now uses a hand-built model direction outside Mermaid's supported
set. Support docs still need to be updated before closeout.

## Active Task

- Task ID: AFDT-040
- Owner: planner
- Files:
  - `crates/merman-ascii/FLOWCHART_SUPPORT.md`
  - `README.md`
  - `docs/workstreams/ascii-flowchart-direction-transform`
- Validation: `cargo nextest run -p merman-ascii`; `cargo fmt --all --check`; `git diff --check`
- Status: READY
- Review: Support docs should describe shipped BT/RL root-direction behavior and explicitly keep
  subgraph direction overrides, color/style roles, state diagrams, and uncommon shapes out of scope
  unless split into follow-ons.
- Evidence: `EVIDENCE_AND_GATES.md`

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

## Blockers

- None.

## Next Recommended Action

- Run AFDT-040: update support docs/readme references, run final gates, then close or split any
  remaining flowchart direction follow-ons.
