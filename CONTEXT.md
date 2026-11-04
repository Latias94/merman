# Project Context

This file is the current-facing entry point for repository context. ADRs remain
authoritative for detailed decisions; this page
keeps the active baseline, boundaries, and navigation local to the repository
root.

## Domain

`merman` is a Rust implementation of Mermaid-compatible parsing, layout, and
headless rendering. The project favors typed semantic/render models internally
while preserving compatibility JSON and adapter contracts where they are part of
the public surface.

Primary capability areas:

- `merman-model`: canonical family data, configuration values, frozen presentation
  data, and shared lifecycle, operation, runtime, and resource contracts.
- `merman-markup`: shared HTML/URL safety, CSS structural predicates, and color
  operations used by source ingestion and graphical output.
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

The selected upstream baseline is owned by the [alignment authority map](docs/alignment/README.md#machine-authorities).

Authoritative baseline sources:

- `tools/upstreams/REPOS.lock.json`
- `docs/adr/0001-upstream-baseline.md`
- `crates/merman-model/src/baseline.rs`

Historical implementation comments may still name the Mermaid release where an algorithm was first
ported. Current assets and APIs must instead use the owning registry's
`pinned_mermaid_baseline()` constructor or constants from `merman_model::baseline`; stale version
suffixes are not an authority for current behavior.

## Architecture Boundaries

Current contract:

- The **Headless Render Operation** is the canonical Mermaid-source render flow: family semantic
  construction, typed layout, SVG emission, postprocess metadata, and pipeline ordering behind one
  behavior-bearing module. Public adapters choose input/output shape; they do not rebuild that flow.
- The **Parse Pipeline** owns preprocessing, detection or known-type metadata projection, runtime
  date hooks, family dispatch, lenient error behavior, timing diagnostics, source remapping, and
  sanitization orchestration. Family construction owns source sanitization where its model
  requires it; custom JSON admission still sanitizes at the pipeline boundary. `SemanticParser`
  owns lean semantic requests; `MermaidEngine` owns presentation-aware parsing. Family modules
  own grammar meaning.
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
- Analysis diagnostics and parser-backed facts use independent schemas. Current development uses
  diagnostics `1` and facts `3`, with other facts versions rejected at the version boundary; see
  the [payload owner](crates/merman-analysis/src/payload.rs) and
  [current facts contract](docs/lsp/CAPABILITIES.md#typed-snapshot-and-analysis-facts-wire-contracts).
  Published `0.10.0-alpha.1` retains facts `2`; the
  [development migration](docs/release/UNRELEASED_CONTRACT_VERSIONS.md#source-appearance-and-render-evidence-follow-up)
  applies to matching source-built wrappers and artifacts.
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

- Task navigation: [documentation index](docs/README.md).
- Current package/data ownership: [ADR-0099](docs/adr/0099-model-and-source-boundaries.md);
  family semantics and operation ownership: [ADR-0073](docs/adr/0073-family-owned-diagram-architecture.md).
- Selected upstream and parity evidence: [alignment authority map](docs/alignment/README.md) and
  [dashboard](docs/alignment/STATUS.md).
- Config/frontmatter support: [support guide](docs/alignment/CONFIG_FRONTMATTER_SUPPORT.md).
- Rendering and prepared-source APIs: [Rust embedding guide](docs/rendering/RUST_EMBEDDING.md),
  [SVG output pipeline](docs/rendering/SVG_OUTPUT_PIPELINE.md), and
  [ADR-0096](docs/adr/0096-prepared-source-output-ownership.md).
- SVG root and text measurement: [ADR-0073 root protocol](docs/adr/0073-family-owned-diagram-architecture.md#root-viewport-owns-every-root-svg),
  [ADR-0057](docs/adr/0057-headless-svg-text-bbox.md), and
  [fixture override policy](docs/adr/0062-fixture-derived-overrides.md).
- ASCII boundary: [ADR-0065](docs/adr/0065-ascii-output-boundary.md) and
  [color roles](docs/adr/0067-ascii-color-role-api.md).
- Browser runtime, Playground, benchmark, and editor ownership:
  [ADR-0074](docs/adr/0074-browser-runtime-and-benchmark-ownership.md) and
  [LSP/editor contracts](docs/lsp/README.md).

## Validation Defaults

Use focused owner checks during iteration; widen when the touched surface needs it.

- Format Rust changes with `cargo fmt`.
- Prefer `cargo nextest` for Rust tests.
- For renderer changes, start with the touched crate/test target and add parity
  compare commands when DOM/root behavior is involved.
- For Playground changes, run hermetic source/manifest tests first, then prepared unit/build gates.
  Chromium desktop and Firefox/WebKit smoke are mandatory browser lanes; focused Chromium mobile
  interactions are an explicit on-demand lane with a separate real-device residual checklist.
- For verification ownership and current feature recipes, use
  [the CI guide](docs/development/CI.md), the affected package manifests, and
  [the verifier](crates/xtask/src/cmd/verify.rs). When strict verification is applicable, settle
  the source and run `cargo run --locked -p xtask -- verify --strict` once for final acceptance.
  The [verification schedule](docs/release/MERMAID_UPGRADE_PLAYBOOK.md#7-refresh-generated-surfaces-and-evidence)
  identifies its coverage and the distinct reference-trust, artifact, and platform owners.
  Reuse covered results; rerun for failures or subsequent changes.
