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

Additional ELK algorithm translations stay in this existing EPL-2.0 crate so they share its
license and caller-owned work-control boundary. They do not introduce a second feature or
budget system. A kernel entry point alone does not advertise Mermaid renderer support;
adapter dispatch and compound integration are admitted separately.

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

## Packing kernels

`algorithms::box_layout` translates the SIMPLE mode of Eclipse ELK's
[`BoxLayoutProvider`](https://github.com/eclipse-elk/elk/blob/62d5909f96fad541bc101ad52dabaece6b7eab7e/plugins/org.eclipse.elk.core/src/org/eclipse/elk/core/util/BoxLayoutProvider.java).
It packs measured rectangles, supports the target container expansion behavior, and returns
content translations separately. It does not route edges or consume randomness. Grouped
packing modes are outside Mermaid 12's exposed configuration and are not advertised.

## Force and Stress kernels

`algorithms::force` translates the Fruchterman-Reingold and Eades models from the pinned
`org.eclipse.elk.alg.force` sources. `algorithms::stress` translates Stress majorization,
including the source's Force initialization, fixed nodes, dimension selection, and desired
edge lengths. Both share source import, component ordering/packing, label placement and
rectangle endpoint clipping. The measured flat-graph entry points do not yet enable Mermaid
root or container dispatch; that integration remains separate.

The kernels consume the caller's work control and resolved Java seed. Stress admits its
quadratic distance/weight storage before allocation. Force rejects coincident coordinates
when floating-point precision prevents the source jitter from making progress, and charges
repeated edge/label work as well as particle pairs. Numerical and interruption errors leave
caller inputs unchanged. Self loops are omitted as in the source importer. Force bend
particles are not exposed by Mermaid 12 and are outside these entry points.

Tests in `src/algorithms/{force,stress}/tests.rs` compare with actual elkjs 0.9.3 outputs,
including components, cycles, interactive inputs, labels, and clipped endpoints. Stress
callers supply effective label options (its upstream default is inline placement).

## Tree, packing, and overlap-removal kernels

The same source boundary now contains the remaining Mermaid 12 algorithm kernels:

- `algorithms::mrtree`: source DFS treeification, Walker placement, component packing,
  and AvoidOverlap routing, including long edges and cycle channels in all four directions.
- `algorithms::radial`: the default Eades radial placement, node-size wedges, radius extension,
  and rectangle clipping. It retains the source's first-root and disconnected-node behavior;
  reachable cycles fail explicitly instead of exhausting the call stack. Mermaid selects this
  algorithm only for containers, not through a root `elk.radial` registration.
- `algorithms::rectpacking`: greedy width approximation, block/stack compaction, repeated
  compaction and equal whitespace expansion. It preserves the provider's explicit `trybox`
  branch. Mermaid's `SCANLINE` option is not an ELK 0.9.1 enum value and resolves to GREEDY in
  the selected elkjs 0.9.3 runtime. Container dimensions and content translations are returned.
- `algorithms::spore_overlap`: scanline detection, source Bowyer-Watson triangulation,
  minimum spanning tree growth and straight-edge clipping. Source hash-bucket traversal is
  preserved for equal-cost edge choices. Duplicate centers use an explicitly supplied random
  stream; the kernel never reads process randomness. Unrepresentable jitter is a typed error.

These entry points consume measured geometry and work control. Source-specific sorting,
optimization or compaction options that the Mermaid adapter does not emit are not advertised.
Node micro layout, compound scheduling, label translation and renderer dispatch still belong
at the adapter boundary. Tests compare actual elkjs 0.9.3 node coordinates, dimensions and
routes, alongside scoped cancellation and numeric-boundary cases.

## Random seed authority

Eclipse ELK uses `randomSeed = 0` as an unseeded `new Random()` request. This source port does not read time or process randomness for that branch. A graph must either retain a nonzero source seed or be imported with an `OperationSeed` before a configurator or pipeline entry point executes.

`import_graph_with_operation_seed` derives a Java seed from the owning operation seed, stable graph path, and configuration invocation without rewriting the source option. Raw callers use `import_graph`; every public execution entry rejects the sentinel at the configuration boundary. Individual translated phase helpers are crate-private, so no caller can bypass that boundary with a raw graph.

See [LICENSES/EPL-2.0.txt](LICENSES/EPL-2.0.txt) for the governing license text.
