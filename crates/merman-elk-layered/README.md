# merman-elk-layered

`merman-elk-layered` is the source-backed Eclipse ELK layered layout port used by
`merman-layout-elk`.

This crate is intentionally separate from the rest of the workspace because the
Eclipse ELK sources are licensed under EPL-2.0. Source-port work in this crate
must preserve upstream source references and keep algorithm translations inside
this EPL-2.0 boundary.

Current source baseline:

- Mermaid adapter: https://github.com/mermaid-js/mermaid/blob/41646dfd43ac83f001b03c70605feb036afae46d/packages/mermaid-layout-elk/src/render.ts
- elkjs: https://github.com/kieler/elkjs/tree/a8304cf79fde75bc2ab1a89d28320f53f8637436
- Eclipse ELK: https://github.com/eclipse-elk/elk/tree/62d5909f96fad541bc101ad52dabaece6b7eab7e

The crate contains the production layered graph, option model, processor assembly, and layout
phases used by `merman-layout-elk`. Corrections and new behavior must continue to follow the pinned
Eclipse ELK sources rather than approximating fixture output.

## Random seed authority

Eclipse ELK uses `randomSeed = 0` as an unseeded `new Random()` request. This
source port does not read time or process randomness for that branch. A graph
must either retain a nonzero source seed or carry an explicit
`RandomSeedPolicy` before a configurator or pipeline entry point executes.

`RandomSeedPolicy::DeterministicFallback` derives a Java seed from the owning
operation key, stable graph path, and configuration invocation without
rewriting the source option. Low-level callers can instead use
`RequireExplicit` to reject the sentinel at the execution boundary.
