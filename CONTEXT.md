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
- `@mermanjs/web` and `playground`: browser-only WASM packages plus the local-first authoring,
  Compare, benchmark, and parser-backed editor experience.
- `xtask`: fixture import, upstream parity comparison, generated data, audit
  reports, and release gates.

## Baseline

Current pinned upstream baseline: `mermaid@11.17.2`.

Authoritative baseline sources:

- `tools/upstreams/REPOS.lock.json`
- `docs/adr/0001-upstream-baseline.md`
- `crates/merman-core/src/baseline.rs`

Historical implementation comments may still name the Mermaid release where an algorithm was first
ported. Current assets and APIs must instead use the owning registry's
`pinned_mermaid_baseline()` constructor or constants from `merman_core::baseline`; stale version
suffixes are not an authority for current behavior.

## Architecture Boundaries

Current contract:

- The **Headless Render Operation** is the canonical Mermaid-source render flow: family semantic
  construction, typed layout, SVG emission, postprocess metadata, and pipeline ordering behind one
  behavior-bearing module. Public adapters choose input/output shape; they do not rebuild that flow.
- The **Parse Pipeline** owns preprocessing, detection or known-type metadata projection, runtime
  date hooks, family dispatch, lenient error behavior, timing diagnostics, source remapping, and
  common sanitization. `Engine` remains the public facade; family modules own grammar meaning.
- **Diagram Family Facts** are the single pinned-baseline catalog for ids, aliases, detector order,
  semantic/editor/render adapters, config namespaces, authoring headers, public metadata, and
  admission capability. The complete language catalog is invariant across feature combinations;
  registries and public family metadata are projections.
- Each built-in family owns one successful semantic construction. Compatibility JSON,
  parser-backed editor facts, and typed render models project that construction rather than running
  parallel successful grammars.
- Built-in rendering is typed end to end. `FamilyRenderArtifact` owns the matching semantic/layout
  pair, compatibility layout JSON projects from it, and SVG consumes it.
- `RenderEnvironment` selects text-measurement phases, math/icons, time, randomness, and resource
  limits once per operation. Family renderers do not construct hidden production services or read
  process-global render policy.
- Every built-in SVG root uses the operation-owned Root Viewport protocol for dimension
  normalization, sizing, accessibility chrome, escaping, attribute order, and deferred
  finalization. Families supply source-backed content bounds and family root semantics; they do not
  own root emission or consult fixture-derived root data.
- Editor body semantics are parser-complete, parser-recovered, or unavailable. Generic TextScan
  semantics are deleted; legal source-start headers remain catalog-backed.
- Analysis diagnostics and parser-backed facts use independent schemas: diagnostics `1`, facts `2`.
  Other facts versions are rejected at the version boundary, and the superseded TextScan path is
  not supported in parallel.
- **Capability and Artifact Profiles** are separate contracts. The capability descriptor owns
  additive semantic vocabulary and implications; artifact profiles own exact Cargo build recipes.
  Neither substitutes for the ABI, protocol, package, runtime, or release authority at the surface
  that implements it.
- **Admission Inventory** records which fixture/family surfaces are parser-only, layout-covered,
  SVG-covered, root-parity-covered, skipped, or deferred for the pinned baseline and why. Parser
  and typed-render capability evidence should be checked against Diagram Family Facts projections.
- Effective config and presentation theme should be projected into narrow views
  before diagram renderers consume them.
- ADR-0062 forbids production fixture overrides: fixture ids and complete source or label strings
  must not select root, geometry, or measurement answers. Mermaid config, theme variables, diagram
  directives, and host-owned CSS/output overrides remain supported user inputs; they are general
  configuration and presentation contracts, not fixture answers.
- The **Browser Playground** has one document-owned Merman runtime and one render coordinator.
  Compare and Benchmark own separate realms, the language service owns a bounded Worker mailbox,
  and optional Config, Examples, and Benchmark code is acquired only after user activation. The
  package entry uniquely owns the production WASM URL; realm engines receive explicit resources
  and do not embed another WASM binary.

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
- SVG root and text-measurement policy:
  `docs/adr/0050-svg-viewbox-parity.md`,
  `docs/adr/0057-headless-svg-text-bbox.md`,
  `docs/adr/0062-fixture-derived-overrides.md`
- ASCII boundary:
  `docs/adr/0065-ascii-output-boundary.md`,
  `docs/adr/0067-ascii-color-role-api.md`
- Browser runtime, Playground, benchmark, and editor ownership:
  `docs/adr/0074-browser-runtime-and-benchmark-ownership.md` and
  `docs/workstreams/web-wasm-playground/DESIGN.md`

## Validation Defaults

Prefer focused gates first, then widen only when the touched surface needs it.

- Format Rust changes with `cargo fmt`.
- Prefer `cargo nextest` for Rust tests.
- For renderer changes, start with the touched crate/test target and add parity
  compare commands when DOM/root behavior is involved.
- For Playground changes, run hermetic source/manifest tests first, then prepared unit/build gates.
  Chromium desktop and Firefox/WebKit smoke are mandatory browser lanes; focused Chromium mobile
  interactions are an explicit on-demand lane with a separate real-device residual checklist.
- For release-level confidence, use the documented strict gate in
  `docs/workstreams/fearless-refactor/GATES.md` or the current `xtask verify`
  command set.
