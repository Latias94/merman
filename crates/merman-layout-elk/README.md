# merman-layout-elk

`merman-layout-elk` adapts Merman graph models to the source-backed ELK providers.

> **Implementation dependency:** applications should enable `layout-elk` on the [`merman`](https://crates.io/crates/merman) facade. This crate is the internal adapter and does not promise the complete Eclipse ELK API.

> **Distribution boundary:** this adapter depends on `merman-elk-layered`, which contains
> translated Eclipse ELK source under EPL-2.0. Any binary or bundled artifact that links this
> adapter must ship the EPL license text, attribution/provenance, and the applicable source-
> availability information. The adapter's own Cargo `MIT OR Apache-2.0` declaration does not
> describe that dependency closure.

Keeping this boundary separate lets `merman` feature-gate ELK-specific code and the EPL-2.0 source port outside the base SVG renderer. This crate ships the sole source-backed layered adapter used by Flowchart ELK, Class, and ER rendering; it is an integration point for Mermaid's adapter surface, not a general-purpose Eclipse ELK distribution.

Source-backed Eclipse ELK layered work lives in `merman-elk-layered`, an EPL-2.0 crate that keeps translated algorithm code behind an explicit license boundary. Phase implementations are not re-exported: diagnostics use `SourcePhaseDiagnostics`, preserving the same guarded execution boundary as production layout. There is no compatibility fallback under the same API.

## Random seed authority

ELK treats `randomSeed = 0` as an unseeded request. A headless layout must not turn that into ambient process randomness. `layout(&Graph)` rejects that sentinel when executing a provider that uses it, including Layered, Force and Stress. Deterministic packing providers do not require random authority for an unused option. Normal Mermaid graphs use the upstream nonzero default and continue to use `layout` directly.

An operation owner that intentionally accepts the sentinel must call `layout_with_operation_seed` with an `ElkOperationSeed` created from the immutable, nonzero seed captured for that operation. This keeps replayed layouts byte-stable while preserving the configured `randomSeed` in the source model.

The workspace license is MIT OR Apache-2.0; the translated ELK implementation remains EPL-2.0 in its own crate.

## Additional provider integration

`LayoutOptions::algorithm` selects Box, Rectpacking, Force, Stress, Mr. Tree, Radial, or SPOrE
for measured flat graphs. The adapter preserves provider errors and caller work interruption;
it does not substitute Layered when a selected provider fails. Radial's availability in this
internal API does not imply a Mermaid root loader named `elk.radial`.

This is an intermediate integration boundary: non-layered graphs with parent relationships
return `NonLayeredHierarchy` until mixed hierarchy dispatch is complete. The renderer still
selects Layered and does not advertise these additional names yet. Packing results retain empty
edge routes so the renderer can distinguish absent sections from routed two-point sections.

SPOrE's source uses ambient `Math.random` only to separate coincident centers. The adapter uses
a separate operation-owned deterministic stream when an operation seed is supplied, or the
configured nonzero seed for raw calls. This preserves replayability, not the browser's random
sample sequence.
