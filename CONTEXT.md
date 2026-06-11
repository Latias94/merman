# Project Context

This file is the current-facing entry point for repository context. ADRs and
workstream documents remain authoritative for detailed decisions; this page
keeps the active baseline, boundaries, and navigation local to the repository
root.

## Domain

`merman` is a Rust implementation of Mermaid-compatible parsing, layout, and
headless rendering. The project favors typed semantic/render models internally
while preserving compatibility JSON and adapter contracts where they are part of
the public surface.

Primary capability areas:

- `merman-core`: detection, preprocessing, configuration merge, parsing,
  sanitization, semantic JSON, and typed render model construction.
- `merman-render`: layout models, SVG parity renderers, root viewport handling,
  theme/config projection, text measurement, and render pipeline internals.
- `merman-ascii`: terminal rendering adapters and ASCII-specific layout/routing.
- `merman`: public Rust facade for parse/layout/render/raster operations.
- `merman-cli`, FFI, UniFFI, WASM, and platform wrappers: adapters over the
  canonical headless operations.
- `xtask`: fixture import, upstream parity comparison, generated data, audit
  reports, and release gates.

## Baseline

Current pinned upstream baseline: `mermaid@11.15.0`.

Authoritative baseline sources:

- `tools/upstreams/REPOS.lock.json`
- `docs/adr/0001-upstream-baseline.md`
- `crates/merman-core/src/baseline.rs`

Generated override filenames and some historical comments may still carry
`11_12_2` or `11.12.x` suffixes. Treat those names as legacy provenance unless a
current-facing document explicitly says otherwise. New code should prefer
`for_pinned_mermaid_baseline`, `pinned_mermaid_baseline_*`, and constants from
`merman_core::baseline` over versioned constructor names.

## Architecture Boundaries

Current direction:

- The canonical headless render flow should be named the **Headless Render Operation**: parse,
  typed render model construction, layout, SVG emission, postprocess metadata, and pipeline
  ordering behind one behavior-bearing module. Public adapters choose input/output shape; they do
  not rebuild that flow independently.
- The core parser flow should be named the **Parse Pipeline**: preprocessing, detection or
  known-type metadata projection, runtime date hooks, parser dispatch, lenient error behavior,
  timing diagnostics, and common DB sanitization behind one internal module. `Engine` remains the
  public facade for metadata, semantic JSON, and typed render-model entrypoints.
- SVG and raster outputs from Mermaid source should route through the Headless Render Operation.
  Raw SVG input may stay adapter-local because it does not have a Mermaid parse/render model.
- **Diagram Family Facts** are the pinned-baseline facts for one Mermaid family: ids, aliases,
  feature profile, detector order, parser adapters, typed render adapters, known-type side effects,
  public metadata, and admission status. Call sites should consume projections from those facts
  instead of duplicating hand-maintained lists.
- **Admission Inventory** records which fixture/family surfaces are parser-only, layout-covered,
  SVG-covered, root-parity-covered, skipped, or deferred for the pinned baseline and why. Parser
  and typed-render capability evidence should be checked against Diagram Family Facts projections.
- Diagram detection and parser registration should derive from pinned-baseline
  registry facts instead of scattering diagram ids across call sites.
- Each diagram family should own semantic construction, compatibility JSON
  projection, typed render model construction, layout, SVG rendering, and
  diagram-specific parity exceptions.
- Public adapters should choose input/output shape; they should not rebuild the
  parse/layout/SVG/postprocess pipeline independently.
- Effective config and presentation theme should be projected into narrow views
  before diagram renderers consume them. Sequence, Class, Flowchart, State, ER,
  Block, Sankey, Event Modeling, TreeView, Packet, Venn, Ishikawa, Treemap,
  QuadrantChart, Radar, Pie, Requirement, and Kanban are the first renderer-
  side family pilots for this: layout and SVG parity settings now flow through
  family-owned config views instead of scattered raw diagram namespace
  lookups.
- Root viewport and emitted SVG bounds logic belongs under the SVG parity layer,
  not under one diagram family.
- Override data is a last resort for pinned-baseline parity and must have
  removal evidence plus no-growth gate coverage.

Current non-goal:

- Do not treat `layout: elk` / `flowchart.defaultRenderer=elk` recognition as a
  complete local ELK implementation. Detection/config side effects are preserved,
  but full ELK layout parity needs a separate spike and design decision.

## Where To Look First

- Architecture issue ledger:
  `docs/quality/ARCHITECTURE_ISSUES_2026-06-01.md`
- Current config/frontmatter support:
  `docs/alignment/CONFIG_FRONTMATTER_SUPPORT.md`
- Upstream baseline policy:
  `docs/adr/0001-upstream-baseline.md`
  and `docs/adr/0014-upstream-parity-policy.md`
- Rendering strategy and pipeline decisions:
  `docs/adr/0042-rendering-strategy.md`,
  `docs/adr/0063-extensible-svg-output-pipeline.md`,
  `docs/adr/0064-host-styling-svg-postprocessors.md`
- SVG root/override policy:
  `docs/adr/0050-svg-viewbox-parity.md`,
  `docs/adr/0062-fixture-derived-overrides.md`,
  `docs/workstreams/fearless-refactor/OVERRIDE_POLICY.md`
- ASCII boundary:
  `docs/adr/0065-ascii-output-boundary.md`,
  `docs/adr/0067-ascii-color-role-api.md`

## Validation Defaults

Prefer focused gates first, then widen only when the touched surface needs it.

- Format Rust changes with `cargo fmt`.
- Prefer `cargo nextest` for Rust tests.
- For renderer changes, start with the touched crate/test target and add parity
  compare commands when DOM/root behavior is involved.
- For release-level confidence, use the documented strict gate in
  `docs/workstreams/fearless-refactor/GATES.md` or the current `xtask verify`
  command set.
