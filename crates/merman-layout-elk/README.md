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
for diagnostics and focused parity work. `layout(&Graph)` is the only public
layout interface; the removed compatibility algorithm has no fallback alias.
