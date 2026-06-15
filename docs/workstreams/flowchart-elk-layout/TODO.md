# Flowchart ELK Layout - TODO

Status: Active
Last updated: 2026-06-15

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
- [x] Align Flowchart ELK root SVG group structure with Mermaid before working on layout geometry.
- [x] Add the source-backed Flowchart ELK lane and keep the default renderer on the stable
  lightweight backend.
- [x] Port the layered end-label lifecycle far enough for source-backed runs to restore head/tail
  labels after routing.
- [x] Port `GreedyModelOrderCycleBreaker` and barycenter sweep-time port distribution so explicit
  source-backed probes reach real P3/P5 geometry differences instead of unsupported processor
  failures.
- [ ] Tighten label and multi-edge spacing where the current subset is visibly close.
- [ ] Finish the remaining P3/P5 ordering and routing semantics needed by the HTML ELK demo probe.

Notes:

- 2026-06-14: `merman-layout-elk` now lays out containers recursively. Group nodes inherit the
  parent direction unless they set their own `Node.direction`, so nested Flowchart subgraph
  directions affect final geometry instead of only metadata.
- 2026-06-14: The lightweight backend now ranks direct children per container, routes edges with
  the lowest common container direction, exposes typed `LayoutOptions`, maps Mermaid `elk.*`
  config fields through the Flowchart adapter, and separates parallel edges unless
  `elk.mergeEdges` is set.
- 2026-06-14: `compare-flowchart-svgs --include-elk-probes` can now run known ELK probe
  candidates explicitly. The HTML demo probe is still not admitted by default; its first remaining
  mismatch is now ELK edge path geometry after the root-level DOM wrapper was aligned, so the
  default parity matrix remains green and honest.
- 2026-06-15: The source-backed lane now follows Eclipse ELK's greedy model-order cycle breaker
  and calls the barycenter port distributor during each layer sweep. The explicit HTML demo probe
  no longer fails on an unsupported P1 processor; it reaches the current geometry mismatch at
  `svg/g[6]/path[1]`.

## Phase 3 - Port Decision

- [x] Decide to keep a deeper source-backed ELK lane for Flowchart parity while preserving the
  default lightweight backend.
- [ ] Keep source-backed ELK behind an explicit backend/feature boundary until probe coverage is
  mature enough for admission.
- [ ] Update the compare/import policy so the lane reflects the final decision instead of leaving
  `flowchart-elk` in a permanent limbo.
