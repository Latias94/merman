# merman-layout-elk

`merman-layout-elk` is the optional ELK layout engine integration point for
`merman`.

The crate exists so ELK-specific dependencies and adapters can be developed,
tested, and feature-gated outside `merman-render`. The first supported target is
Mermaid's default ELK behavior for `flowchart-elk` / `layout: elk`, which maps
to the layered ELK algorithm.

This crate currently ships a lightweight, deterministic layered backend that
keeps `flowchart-elk` renderable without adding heavy runtime dependencies. It is
an integration boundary, not a complete Eclipse ELK port.

Source-backed Eclipse ELK layered work lives in `merman-elk-layered`, an
EPL-2.0 crate that keeps translated ELK algorithm code behind an explicit
license boundary. `merman-layout-elk` re-exports that crate as `source_port`
while the public layout API continues to delegate to the compatibility backend.
