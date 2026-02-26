# Changelog

All notable changes to this project will be documented in this file.

The format is based on *Keep a Changelog*, and this project adheres to *Semantic Versioning*.

## [Unreleased]

### Added

- Imported additional upstream fixtures from Cypress and package tests (requirement, gantt, ER, flowchart, sequence, state, class, quadrantchart, xychart, radar, kanban, architecture, block, mindmap, timeline) to expand SVG parity coverage.
- Imported upstream HTML demo fixtures (flowchart, sequence, quadrantchart, sankey, xychart) to expand golden-driven parity coverage.

### Fixed

- Improved `<foreignObject>` readability fallback for raster outputs (PNG/JPG/PDF): remove the white text outline overlay and render a semi-transparent `.labelBkg` background when present (closer to upstream Mermaid defaults).
- Reduced cross-platform SVG DOM drift in `parity-root` compares by snapping root `style` `max-width` and `viewBox` to a stable lattice.
- Further reduced `parity-root` drift by bias-snapping root `max-width` and masking `viewBox` origin (x/y) while still tracking viewport size changes (w/h).
- Block: aligned `doublecircle` SVG structure to match upstream Mermaid DOM output.
- Aligned C4 `sprite` rendering with upstream Mermaid: only `person`/`external_person` emit `<image>` sprites.
- ER: align Markdown formatting in entity labels even when the entity has no attributes.
- Flowchart: preserve cyclic self-loop helper mid-edge labels (fixes missing self-loop label DOM).
- `import-upstream-pkg-tests`: avoid failing the import when all candidates are skipped (still prints a skip summary).
- Reduced churn during `import-upstream-docs --with-baselines` by skipping blank-info code fences that lack an explicit Mermaid diagram directive (e.g. `flowchart` / `graph`).
- Reduced churn during `import-upstream-cypress --with-baselines` by deferring out-of-scope class fixtures (`htmlLabels=false`, `layout=elk`, `look!=classic`) under `fixtures/_deferred/`.
- Improved `import-upstream-pkg-tests` Mermaid source extraction to handle `"..."` / `'...'` literals and template strings with `${...}` interpolation.
- Sequence: render diagram titles from metadata/frontmatter when the semantic model title is empty (aligns upstream HTML demos).
- Sequence: adjusted wrapped note line breaks to match upstream Mermaid `wrapLabel(...)` behavior (11.12.3 baselines).
- QuadrantChart: derive default theme colors from `themeVariables` (including `hsl(...)`/hex parsing) to match upstream theme behavior.

### Changed

- Refreshed README showcase renders after parity updates (architecture/mindmap/sankey/gantt).
- CI: run `parity-root` SVG DOM comparisons as a non-blocking check on Ubuntu (keeps `parity` as the gate).
- Documented that the root viewport override baselines track Mermaid 11.12.3 (override module filenames still use the historical `*_11_12_2.rs` suffix).
- Updated upstream Mermaid baselines to 11.12.3 and refreshed `fixtures/upstream-svgs/**`.
- `import-upstream-html`: flowchart fixtures containing `$$...$$` math labels are imported as `*_parser_only_katex` (kept for parser/layout coverage, excluded from SVG DOM parity gates until KaTeX HTML label parity is implemented).
- Deferred upstream HTML treemap demos that render as upstream error output under `fixtures/_deferred/` (avoid permanently failing parity gates).

### Removed

- Removed `mermaid-rs-renderer` (`mmdr_`) fixtures and baselines from this repository; fixtures are now sourced only from upstream Mermaid.

## [0.1.0] - 2026-02-22

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
