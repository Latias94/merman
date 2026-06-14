# Flowchart ELK Layout - TODO

Status: Active
Last updated: 2026-06-14

## Phase 1 - Baseline And Classification

- [x] Keep the current Flowchart ELK smoke fixtures renderable.
- [x] Classify the upstream `flowchart-elk.spec.js` cases into Tier A, Tier B, and Tier C.
- [x] Record which fixtures are already covered by the current lightweight subset versus which
  ones still need adapter work.

## Phase 2 - Subset Growth

- [ ] Carry `Node.direction` through nested subgraphs.
- [ ] Improve cluster/hierarchy handling for outgoing links and links between subgraphs.
- [ ] Tighten label and multi-edge spacing where the current subset is visibly close.
- [ ] Re-check whether the diamond/intersection cases can be handled without a full port.

## Phase 3 - Port Decision

- [ ] If Tier B converges cleanly, stop at the subset and keep the full port out of the default
  workspace.
- [ ] If Tier C remains large after subset growth, isolate the deeper ELK port as a separate
  dependency and license boundary.
- [ ] Update the compare/import policy so the lane reflects the final decision instead of leaving
  `flowchart-elk` in a permanent limbo.
