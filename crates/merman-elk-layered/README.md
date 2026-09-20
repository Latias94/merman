# merman-elk-layered

`merman-elk-layered` is the source-backed Eclipse ELK layered layout port used by `merman-layout-elk`.

> **Implementation and license boundary:** Mermaid applications should enable `layout-elk` on [`merman`](https://crates.io/crates/merman), not depend on this crate directly. This crate is isolated because its translated Eclipse ELK source is licensed under EPL-2.0.

This crate is intentionally separate from the rest of the workspace because the Eclipse ELK sources are licensed under EPL-2.0. Source-port work in this crate must preserve upstream source references and keep algorithm translations inside this EPL-2.0 boundary.

Port provenance baselines:

- Mermaid adapter used when this port was admitted: https://github.com/mermaid-js/mermaid/blob/41646dfd43ac83f001b03c70605feb036afae46d/packages/mermaid-layout-elk/src/render.ts
- elkjs: https://github.com/kieler/elkjs/tree/a8304cf79fde75bc2ab1a89d28320f53f8637436
- Eclipse ELK: https://github.com/eclipse-elk/elk/tree/62d5909f96fad541bc101ad52dabaece6b7eab7e

These are the translated port's historical derivation points, not the workspace's current Mermaid baseline. The current adapter revision is pinned by [`merman-layout-elk`](../merman-layout-elk/src/lib.rs) and [`REPOS.lock.json`](../../tools/upstreams/REPOS.lock.json).

The crate contains the production layered graph, option model, processor assembly, and layout phases used by `merman-layout-elk`. Corrections and new behavior must continue to follow the pinned Eclipse ELK sources rather than approximating fixture output.

## Layer assignment

The layered pipeline supports `NETWORK_SIMPLEX`, `LONGEST_PATH`, `LONGEST_PATH_SOURCE`,
`COFFMAN_GRAHAM`, `MIN_WIDTH`, `STRETCH_WIDTH`, and `INTERACTIVE`. The six additional
strategies are translated from the same Eclipse ELK revision above and checked against
elkjs 0.9.3 phase outputs. Their source references live beside the implementations in
`src/p2layers/`.

Adaptive layering searches honor the caller's work control. Stretch-width rejects mixed
zero and positive normal-node heights with a typed error because the upstream normalization
can make its retry loop nonterminating. An interrupted assignment does not commit partial layers.

## Node placement

NetworkSimplex supports the node-level `PORT_POSITION` flexibility used for Mermaid 12
containers. It preserves fixed or crowded ports, builds separate corner and port constraints
for flexible nodes, and applies source path-straightening rules. This property belongs to
the container node in its parent graph; it is not inherited by the container's children.
The implementation and elkjs 0.9.3 geometry evidence live in `src/p4nodes/network_simplex.rs`.

## Random seed authority

Eclipse ELK uses `randomSeed = 0` as an unseeded `new Random()` request. This source port does not read time or process randomness for that branch. A graph must either retain a nonzero source seed or be imported with an `OperationSeed` before a configurator or pipeline entry point executes.

`import_graph_with_operation_seed` derives a Java seed from the owning operation seed, stable graph path, and configuration invocation without rewriting the source option. Raw callers use `import_graph`; every public execution entry rejects the sentinel at the configuration boundary. Individual translated phase helpers are crate-private, so no caller can bypass that boundary with a raw graph.

See [LICENSES/EPL-2.0.txt](LICENSES/EPL-2.0.txt) for the governing license text.
