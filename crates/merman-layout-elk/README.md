# merman-layout-elk

`merman-layout-elk` is the optional ELK layout engine integration point for
`merman`.

The crate exists so ELK-specific dependencies and algorithms can be developed,
tested, and feature-gated outside `merman-render`. The first supported target is
Mermaid's default ELK behavior for `flowchart-elk` / `layout: elk`, which maps
to the layered ELK algorithm.

This crate currently ships a lightweight, deterministic layered backend that
keeps `flowchart-elk` renderable without adding heavy runtime dependencies. It is
an integration boundary, not a complete Eclipse ELK port. More precise ELK
parity work can replace or extend the backend behind this API without widening
the dependency surface of `merman-render`.
