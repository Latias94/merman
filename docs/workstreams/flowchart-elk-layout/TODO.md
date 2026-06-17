# Flowchart ELK Layout - TODO

Status: Active
Last updated: 2026-06-17

## Phase 1 - Baseline And Classification

- [x] Keep the current Flowchart ELK smoke fixtures renderable.
- [x] Classify the upstream `flowchart-elk.spec.js` cases into Tier A, Tier B, and Tier C.
- [x] Record which exact-call fixtures are covered by the source-backed probe lane and which
  duplicate layout bodies share semantic coverage.

## Phase 2 - Subset Growth

- [x] Carry `Node.direction` through nested subgraphs.
- [x] Improve cluster/hierarchy handling for outgoing links and links between subgraphs.
- [x] Add an explicit ELK probe lane in `xtask` without admitting failing fixtures to the default
  Flowchart parity matrix.
- [x] Align Flowchart ELK root SVG group structure with Mermaid before working on layout geometry.
- [x] Add the source-backed Flowchart ELK lane while keeping the compatibility backend available as
  an explicit fallback.
- [x] Port the layered end-label lifecycle far enough for source-backed runs to restore head/tail
  labels after routing.
- [x] Port `GreedyModelOrderCycleBreaker` and barycenter sweep-time port distribution so explicit
  source-backed probes reach real P3/P5 geometry differences instead of unsupported processor
  failures.
- [x] Cover the current Mermaid ELK spec body set with source-backed probes.
- [x] Finish the remaining P3/P5 ordering and routing semantics needed by the HTML ELK demo probe.

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
- 2026-06-15: The source-backed P5 router now matches Eclipse ELK's actual `PortType.OUTPUT`
  semantics by treating ports with outgoing edges as output ports, independent of the stored
  imported port marker. The explicit HTML ELK demo probe now matches in the source-backed lane;
  explicit admission still keeps probe fixtures out of the broad parity matrix by policy.
- 2026-06-17: Public render paths and xtask diagnostics now default Flowchart ELK to the
  source-backed backend.
- 2026-06-18: The dedicated probe gate now covers all 63 upstream exact render calls from 57 unique
  layout bodies. Duplicate layout bodies remain visible as separate exact-call fixtures for
  upstream traceability.

## Phase 3 - Port Decision

- [x] Decide to keep a deeper source-backed ELK lane for Flowchart parity while preserving an
  explicit compatibility fallback.
- [x] Make source-backed ELK the default backend once the dedicated probe lane covers the current
  upstream body set.
- [x] Update the compare/import policy so the source-backed probe lane reflects the current
  decision instead of leaving the HTML ELK demo only as an ad hoc command.
- [x] Import and probe the upstream ELK body set against the source-backed lane before broad parity
  admission.
- [x] Import the six duplicate exact-call fixtures for traceability.
- [ ] Decide broad Flowchart matrix admission for the 63 source-backed probe fixtures.

Notes:

- 2026-06-15: `--include-elk-probes` now only admits registered source-backed ELK probes when the
  compare run uses the source-backed backend, while `check-flowchart-elk-source-backed-probes` is
  the fixed gate for matching source-backed fixtures.
- 2026-06-17: xtask compare/XML/debug defaults now match the public source-backed render default.
  `--flowchart-elk-backend compat` remains available when diagnosing the previous alpha fallback.
- 2026-06-18: Duplicate exact-call fixtures are imported and admitted to the source-backed probe
  lane; coverage audit reports 63 dedicated fixtures, 63 upstream SVGs, and 0 duplicate-covered
  gaps.
