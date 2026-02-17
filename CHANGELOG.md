# Changelog

All notable changes to this project will be documented in this file.

The format is based on *Keep a Changelog*, and this project adheres to *Semantic Versioning*.

## [0.1.0] - 2026-02-10

### Added

- Headless Mermaid parsing and semantic JSON output (`merman-core`).
- Headless layout + SVG rendering with DOM parity gates against upstream baselines (`merman-render`).
- Ergonomic wrapper crate for UI integrations (`merman`, feature-gated via `render` / `raster`).
- CLI for detection, parsing, layout, and rendering (`merman-cli`).
- Raster outputs (PNG/JPG/PDF) via pure-Rust SVG conversion (`resvg` / `svg2pdf`).
- Golden snapshots and parity tooling (`xtask`, `fixtures/**`, `docs/alignment/STATUS.md`).
- ZenUML headless compatibility mode (subset translated to `sequenceDiagram`; not parity-gated).
- Local performance regression tracking via Criterion (`cargo bench -p merman --features render --bench pipeline`).

### Changed

- SVG renderer implementation is organized under `svg::parity` to reflect the upstream-as-spec intent.
- State diagram root viewport (`viewBox`/`max-width`) defaults to SVG-emitted bounds scanning (closest to browser `getBBox()`); set `MERMAN_STATE_VIEWPORT=layout` to use layout-derived bounds.
