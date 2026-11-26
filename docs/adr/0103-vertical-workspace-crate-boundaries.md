# ADR 0103: Vertical Workspace Crate Boundaries and Internal-vs-Public Packaging

- Accepted for the v0.11 development line on 2026-11-26.
- Amends: package-splitting and family-crate ownership clauses of [ADR-0099](0099-model-and-source-boundaries.md)
- Preserves: single semantic construction, bounded operation execution, and selectable diagram families in [ADR-0073](0073-family-owned-diagram-architecture.md), [ADR-0091](0091-selectable-diagram-families.md), and [ADR-0098](0098-semantic-presentation-capabilities.md)

## Context

[ADR-0099](0099-model-and-source-boundaries.md) established clean top-level separation between `merman-model`, `merman-markup`, `merman-core`, `merman-render`, `merman-ascii`, and `merman-export`. As `merman-core` (~196k LOC) and `merman-render` (~339k LOC) grew across 34 diagram families, incremental extraction introduced 11 additional workspace crates (`merman-core-{preprocess,flow,graph,sequence,structured}` and `merman-render-{core,charts,flow,graph,sequence,structured}`).

Mid-refactor review revealed four structural failure modes when extraction proceeds by moving isolated leaf helpers instead of vertical slices:

1. **Fragmented family ownership ("shallow shell crates")**: Extracting isolated helper fragments leaves a single diagram family split across two crates (for example, opening an SVG `<g>` element in a family crate and closing `</g>` in `merman-render`, or publishing a 300-line single-file crate containing only relationship marker strings).
2. **Callback shim proliferation**: Leaving shared contracts (`TextMeasurer`, Theme v2 rendering primitives, root SVG/scene builders) inside `merman-render` forces extracted families to invent ad-hoc per-family callback traits (`*TextMeasurer`, `*ThemeBridge`) that add boilerplate and indirection.
3. **Defeated feature isolation**: Enabling all heavy optional features of `merman-render-core` unconditionally from `merman-render/Cargo.toml` (alongside unconditional `indexmap`, `cssparser`, `roxmltree`, and `quick-xml`) negates compile-time and binary-size gains for narrow diagram selections.
4. **Public crates.io surface sprawl and lockfile churn**: Marking `#[doc(hidden)]`-only internal shells as publishable without clear public contracts or package metadata complicates SemVer and release orchestration, while hashing raw `Cargo.lock` bytes for third-party license verification turns every internal crate refactor into a 20–30 file license churn commit.

## Decision

### 1. Four-Layer Acyclic Workspace Architecture

The workspace converges to a strict four-layer DAG with a clear distinction between **user-facing entry points** and **internal modularization clusters**:

1. **Layer 1 — User Entry Points (stable user-facing SemVer surfaces)**:
   - Rust facade `merman`, CLI `merman-cli`, and ecosystem bindings (`@mermanjs/node`, `@mermanjs/web`, `merman-ffi`, `merman-uniffi`, `merman-android-jni`, `merman-wasi`, `merman-typst-plugin`, `merman-rustdoc`).
   - Application developers should never need to assemble internal family cluster crates directly.
2. **Layer 2 — Workflow & Host Orchestration**:
   - `merman-analysis` (diagnostics, validation, source mapping, future auto-repair), `merman-editor-core` (LSP, editor intelligence, future streaming sessions), and `merman-bindings-core` (shared transport handles, envelopes, resource admission, and cached execution).
3. **Layer 3 — Domain Engines (strictly orthogonal sub-layers)**:
   - **Data & Safety Foundations**: `merman-model`, `merman-markup`, `merman-theme-contract`.
   - **Source Parsing Clusters**: `merman-core-preprocess`, `merman-core-{flow,graph,sequence,structured}`, orchestrated by `merman-core` (engine, config, detection, registry, presentation).
   - **Pure Math Layout Engines (zero DOM / zero parser dependencies)**: `merman-layout-shoal` (opt-in Shoal), `dugong` / `dugong-graphlib` (optional Dagre), `merman-layout-elk` / `merman-elk-layered` (optional ELK), `manatee` (optional Cytoscape/FCoSE).
   - **Rendering & Output Clusters**: `merman-render-core` (shared render foundations), `merman-render-{flow,graph,sequence,charts,structured}`, orchestrated by `merman-render`, plus independent output backends `merman-ascii` and `merman-export`.
4. **Layer 4 — Developer & Release Tooling**:
   - `xtask` split by capability features (lightweight `contracts` default for schema/ABI/feature-matrix verification; opt-in `render-parity` and `wasm-host` features for heavy parity/WASM tooling).

```mermaid
flowchart LR
    Entry["Facades and transport products"] --> Workflow["Workflow and host orchestration"]
    Entry --> Engines["Source, layout, and output engines"]
    Workflow --> Engines
    Engines --> Foundation["Model, markup, and theme contracts"]
    Tools["Developer and release tools"] --> Entry
    Tools --> Engines
```

### 2. Foundations-First Vertical Extraction Rule

No diagram family may be left permanently split between a cluster crate and `merman-core` / `merman-render`:

- **Hoist shared contracts into `merman-render-core` first**: `TextMeasurer` and text-wrapping contracts, shared Theme v2 rendering primitives, bounded SVG output / root document builders, and the shared `PaintScene` IR (see [ADR-0104](0104-paint-scene-ir-and-dual-output-profiles.md)) live in `merman-render-core` behind fine-grained features.
- **Complete vertical ownership per family**: Once a family moves into a `merman-render-*` cluster, that crate must own the family's complete vertical slice (config view, layout adapter wiring, theme binding, and complete balanced SVG/scene emission). Opening and closing an XML/SVG element across crate boundaries and inventing per-family `*TextMeasurer` adapter shims are forbidden as end-state designs.
- **Complete source vertical ownership**: `merman-core-preprocess` owns unified preprocessing primitives without duplicate split filenames (`directives.rs`, `frontmatter.rs`, `text.rs`) straddling two crates; `merman-core-*` family clusters own lexer, predictive parser, and family semantic model construction together, leaving `merman-core` as the orchestration, config, detection, and compatibility facade.
- **No permanent micro-shells**: Any cluster crate that cannot yet hold a complete vertical slice before a publish cut must either finish its vertical extraction or be folded back into its parent crate prior to release.
- **Separate Shoal ownership**: Keep the existing Shoal solver, bridge, and route-paint paths in their own development track. Family renderers consume the existing layout results without changing Shoal or introducing it to additional transports.

### 3. Strict Feature Gating and Dependency Hygiene

- `merman-render/Cargo.toml` must enable `merman-render-core` features, `indexmap`, `cssparser`, `pulldown-cmark`, and XML parsers **only** under the diagram or capability features that require them.
- `merman-render-core/src/icon_registry` must parse icon XML in a single `quick-xml` pass rather than double-parsing with both `quick-xml` and `roxmltree`.
- Platform/binding crates must not leak optional features unconditionally (for example, `merman-wasm` must gate `merman-core/editor-facts` behind `editor`; vendored graphics crates such as `vendor/krilla-svg` must never unconditionally enable `usvg/system-fonts` or `memmap-fonts`).
- Every crate published to crates.io (including internal cluster crates required by published `merman-core` / `merman-render`) must declare complete package metadata (`description`, `documentation`, `keywords`, `categories`, `license`, `repository`), explicitly document whether its API is workspace-internal (`merman-*-*` SPI with exact workspace version matching) or public SemVer, and pass `xtask` feature-matrix verification.

### 4. Third-Party-Only License Provenance Hashing

- The lock-derived fingerprint in Rust license reports uses a **canonical sorted projection of third-party package identities** (`name`, `version`, `source`, and any `checksum` for entries with `source`) from `Cargo.lock`, not the raw lockfile bytes.
- The fingerprint does not prove the dependency closure of an artifact. Keep per-recipe, per-target license resolution and feature-closure checks, and retain full lockfile identity in build provenance. Vendored path components remain separately covered by [the third-party component inventory](../release/THIRD_PARTY_COMPONENTS.json).
- Internal workspace splits that leave an artifact's external resolution unchanged must not churn its legal files. Land this report change before the remaining crate moves.

## Alternatives considered

- **Continue extracting leaf helpers.** This permits small moves but leaves split lifecycle ownership and introduces local bridge interfaces.
- **Keep both monoliths and use only modules.** This avoids package coordination but does not isolate feature resolution or compilation for independently selected families.

## Consequences

- Independently compiled family clusters make smaller incremental builds possible. Compile time and artifact size still require measurement on a pinned recipe.
- Verify the facade imports, exact semantic and SVG contracts, cancellation, and resource boundaries on the migrated families. Check defaults-off feature closures without expecting tools that a selected family actually uses to disappear.
- Verify that an internal-only lock change leaves legal reports byte-stable and that resolved dependency, feature, or vendored-component changes still reach the appropriate checks. The release artifacts remain the authority for publishable closures.
