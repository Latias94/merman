# merman-render

Headless layout + SVG renderer for Mermaid.

Baseline: Mermaid `@11.12.2` (upstream Mermaid is treated as the spec).

This crate provides:

- Layout (geometry + routes) on top of the semantic model from `merman-core`
- SVG output with parity-oriented DOM comparison against upstream baselines

If you want a single ergonomic API surface, use the `merman` crate with the `render` feature.

Parity dashboards and automation live in `docs/alignment/STATUS.md` in the repository.

