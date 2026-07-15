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
  sanitization, compatibility semantic JSON, parser-backed editor facts, and typed render model
  projection.
- `merman-render`: layout models, SVG parity renderers, root viewport handling,
  theme/config projection, text measurement, and render pipeline internals.
- `merman-ascii`: terminal rendering adapters and ASCII-specific layout/routing.
- `merman`: public Rust facade for parse/layout/render/raster operations.
- `merman-cli`, FFI, UniFFI, WASM, and platform wrappers: adapters over the
  canonical headless operations.
- `xtask`: fixture import, upstream parity comparison, generated data, audit
  reports, and release gates.

## Baseline

Current pinned upstream baseline: `mermaid@11.16.0`.

Authoritative baseline sources:

- `tools/upstreams/REPOS.lock.json`
- `docs/adr/0001-upstream-baseline.md`
- `crates/merman-core/src/baseline.rs`

Generated override filenames and some historical comments may still carry
`11_12_2`, `11_15_0`, or `11.12.x`/`11.15.x` suffixes. Treat those names as
legacy provenance unless a current-facing document explicitly says otherwise. New code should prefer
`for_pinned_mermaid_baseline`, `pinned_mermaid_baseline_*`, and constants from
`merman_core::baseline` over versioned constructor names.

## Architecture Boundaries

Current contract:

- The **Headless Render Operation** is the canonical Mermaid-source render flow: family semantic
  construction, typed layout, SVG emission, postprocess metadata, and pipeline ordering behind one
  behavior-bearing module. Public adapters choose input/output shape; they do not rebuild that flow.
- The **Parse Pipeline** owns preprocessing, detection or known-type metadata projection, runtime
  date hooks, family dispatch, lenient error behavior, timing diagnostics, source remapping, and
  common sanitization. `Engine` remains the public facade; family modules own grammar meaning.
- **Diagram Family Facts** are the single pinned-baseline catalog for ids, aliases, feature profile,
  detector order, semantic/editor/render adapters, config namespaces, authoring headers, public
  metadata, and admission capability. Registries and public capability metadata are projections.
- Each built-in family owns one successful semantic construction. Compatibility JSON,
  parser-backed editor facts, and typed render models project that construction rather than running
  parallel successful grammars.
- Built-in rendering is typed end to end. `FamilyRenderArtifact` owns the matching semantic/layout
  pair, compatibility layout JSON projects from it, and SVG consumes it.
- `RenderEnvironment` selects text-measurement phases, math/icons, time, randomness, resource
  limits, and generated root policy once per operation. Family renderers do not construct hidden
  production services or read process-global render policy.
- Every built-in SVG root uses the shared Root Viewport protocol for generated/computed policy,
  sizing, accessibility chrome, escaping, attribute order, and deferred finalization. Families own
  their content bounds, not root emission or generated lookup.
- Editor body semantics are parser-complete, parser-recovered, or unavailable. Generic TextScan
  semantics are deleted; legal source-start headers remain catalog-backed.
- Analysis diagnostics and parser-only facts are independent schema v1 contracts. The
  TextScan-capable alpha facts shape is removed rather than supported in parallel.
- **Admission Inventory** records which fixture/family surfaces are parser-only, layout-covered,
  SVG-covered, root-parity-covered, skipped, or deferred for the pinned baseline and why. Parser
  and typed-render capability evidence should be checked against Diagram Family Facts projections.
- Effective config and presentation theme should be projected into narrow views
  before diagram renderers consume them.
- Override data is a last resort for pinned-baseline parity and must have
  removal evidence plus no-growth and stale-key gate coverage.

Current non-goal:

- Do not infer that one admitted ELK-backed family implies complete support for every Mermaid ELK
  consumer. Flowchart and Class have explicit source-backed paths; other families require their own
  admission evidence.

## Where To Look First

- Architecture issue ledger:
  `docs/quality/ARCHITECTURE_ISSUES_2026-06-01.md`
- Family-owned architecture contract:
  `docs/adr/0073-family-owned-diagram-architecture.md`
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
