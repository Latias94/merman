# merman-layout-elk

`merman-layout-elk` is the optional ELK layout engine integration point for
`merman`.

The crate exists so ELK-specific dependencies and adapters can be developed,
tested, and feature-gated outside `merman-render`. The first supported target is
Mermaid's default ELK behavior for `flowchart-elk` / `layout: elk`, which maps
to the layered ELK algorithm.

This crate ships the source-backed layered adapter used by Flowchart ELK in
`merman-render`, plus the previous lightweight compatibility backend for
explicit alpha fallback. It is an integration boundary for the Mermaid adapter
surface, not a general-purpose complete Eclipse ELK distribution.

Source-backed Eclipse ELK layered work lives in `merman-elk-layered`, an
EPL-2.0 crate that keeps translated ELK algorithm code behind an explicit
license boundary. `merman-layout-elk` re-exports that crate as `source_port`
for diagnostics and focused parity work. The low-level `layout` API still
delegates to the compatibility backend for callers that explicitly need the
pre-port behavior; the public Flowchart render path calls the source-backed
adapter.
