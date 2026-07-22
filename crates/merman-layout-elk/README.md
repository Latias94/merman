# merman-layout-elk

`merman-layout-elk` is the optional ELK layout engine integration point for
`merman`.

The crate exists so ELK-specific dependencies and adapters can be developed,
tested, and feature-gated outside `merman-render`. The first supported target is
Mermaid's default ELK behavior for `flowchart-elk` / `layout: elk`, which maps
to the layered ELK algorithm.

This crate ships the sole source-backed layered adapter used by Flowchart ELK,
Class, and ER rendering. It is an integration point for Mermaid's adapter
surface, not a general-purpose complete Eclipse ELK distribution.

Source-backed Eclipse ELK layered work lives in `merman-elk-layered`, an
EPL-2.0 crate that keeps translated ELK algorithm code behind an explicit
license boundary. `merman-layout-elk` re-exports that crate as `source_port`
for diagnostics and focused parity work. The removed compatibility algorithm
has no fallback alias.

## Random seed authority

ELK treats `randomSeed = 0` as an unseeded request. A headless layout must not
turn that into ambient process randomness. `layout(&Graph)` therefore rejects
that sentinel with a typed error. Normal Mermaid graphs use the upstream
nonzero default and continue to use `layout` directly.

An operation owner that intentionally accepts the sentinel must call
`layout_with_random_policy` with an `ElkRandomPolicy` derived from its immutable
operation context. This keeps replayed layouts byte-stable while preserving the
configured `randomSeed` value in the source model.
