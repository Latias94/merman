# Flowchart ELK Layout - TODO

Status: Active
Last updated: 2026-06-14

## Phase 1 - Baseline And Classification

- [x] Keep the current Flowchart ELK smoke fixtures renderable.
- [x] Classify the upstream `flowchart-elk.spec.js` cases into Tier A, Tier B, and Tier C.
- [x] Record which fixtures are already covered by the current lightweight subset versus which
  ones still need adapter work.

## Phase 2 - Subset Growth

- [x] Carry `Node.direction` through nested subgraphs.
- [x] Improve cluster/hierarchy handling for outgoing links and links between subgraphs.
- [x] Add an explicit ELK probe lane in `xtask` without admitting failing fixtures to the default
  Flowchart parity matrix.
- [ ] Tighten label and multi-edge spacing where the current subset is visibly close.
- [ ] Re-check whether the diamond/intersection cases can be handled without a full port.

Notes:

- 2026-06-14: `merman-layout-elk` now lays out containers recursively. Group nodes inherit the
  parent direction unless they set their own `Node.direction`, so nested Flowchart subgraph
  directions affect final geometry instead of only metadata.
- 2026-06-14: The lightweight backend now ranks direct children per container, routes edges with
  the lowest common container direction, exposes typed `LayoutOptions`, maps Mermaid `elk.*`
  config fields through the Flowchart adapter, and separates parallel edges unless
  `elk.mergeEdges` is set.
- 2026-06-14: `compare-flowchart-svgs --include-elk-probes` can now run known ELK probe
  candidates explicitly. The HTML demo probe is still not admitted by default because it fails on
  Flowchart-ELK DOM shape and layout geometry, so the default parity matrix remains green and
  honest.

## Phase 3 - Port Decision

- [ ] If Tier B converges cleanly, stop at the subset and keep the full port out of the default
  workspace.
- [ ] If Tier C remains large after subset growth, isolate the deeper ELK port as a separate
  dependency and license boundary.
- [ ] Update the compare/import policy so the lane reflects the final decision instead of leaving
  `flowchart-elk` in a permanent limbo.
