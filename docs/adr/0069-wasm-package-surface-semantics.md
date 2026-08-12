# ADR 0069: WASM Package Surface Semantics

- Status: accepted for transport separation; package and capability mapping superseded by ADR-0076
- Date: 2026-06-10
- Last amended: 2026-07-22

ADR-0076 now owns Web artifact profiles, package mappings, Typst artifact mappings, and capability
semantic IDs through `capabilities/feature-surface-v1.json`. The one-package/subpath and legacy
profile ownership decisions below are historical. Browser wasm-bindgen and Typst
wasm-minimal-protocol remain separate transports. The old Web and Typst descriptors stay live only
as migration-ledger entries until U8 consumes the canonical projections and deletes them.

## Context

The WASM feature-surface slimming lane split concerns that were previously easy to conflate:

- browser WebAssembly delivered through `wasm-bindgen` and the `@mermanjs/web` TypeScript package;
- Typst/pure WebAssembly delivered through `wasm-minimal-protocol` and a `wasmi` host;
- Cargo feature profiles for full Mermaid compatibility, host capabilities, and output capabilities.

`merman-wasm` remains a browser/JavaScript transport crate. It is not a generic "runs in every
WASM host" artifact, because wasm-bindgen glue, browser imports, and TypeScript package helpers are
part of its intended contract.

At the same time, `merman-typst-plugin` proves the Typst transport boundary through an exact
profile descriptor, a closed ABI, and a profile-owned artifact. The publish artifact may import
only the two `typst_env` wasm-minimal-protocol functions. Its callable surface is exactly the five
Typst ABI-2 operations for version, capabilities, SVG render, and canonical analysis. Its non-callable
support exports are exactly `memory` plus the immutable `i32` linker globals `__data_end` and
`__heap_base` emitted by Rust's WebAssembly linker.

## Decision

Keep the browser, Rust/native, and Typst WASM surfaces separate for release semantics.

```mermaid
flowchart LR
    Rust["Rust/native crates"] --> Core["merman / merman-core"]
    Browser["Browser / TypeScript"] --> Web["@mermanjs/web"]
    Web --> WasmBindgen["merman-wasm<br/>wasm-bindgen"]
    Typst["Typst / wasmi"] --> TypstPkg["Typst package"]
    TypstPkg --> TypstPlugin["merman-typst-plugin<br/>wasm-minimal-protocol"]
    Core --> Render["merman-render"]
    WasmBindgen --> Core
    TypstPlugin --> Core
    TypstPlugin --> Render
```

1. `@mermanjs/web` remains one npm package with capability-specific public subpaths.
   - `platforms/web/web-surface-descriptor.json` is the machine-readable owner of browser artifact
     features/capabilities and public package/profile mappings.
     Build and release tools consume it as structured data rather than parsing JavaScript source.
   - The default entry point publishes `browser-full`.
   - `./core`, `./render`, `./render-only`, `./ascii`, `./editor`, and `./full` each bind to the
     matching generated WASM artifact and omit unsupported runtime wrappers from their TypeScript
     surface.
   - `browser-editor` contains the full diagram catalog, analysis, and `merman-editor-core`
     language intelligence without SVG, ASCII, host, or ELK dependencies. It is the browser Worker
     surface described by ADR-0074.
   - Browser package variants are represented by the owner-specific entries in
     `platforms/web/web-surface-descriptor.json` and `capabilities/artifact-profiles-v1.json`;
     there is no repository-wide Cargo preset vocabulary.
   - Pure `./catalog`, `./svg-safety`, and `./text-measurement-abi` helpers do not initialize WASM.
   - `bindingCapabilities()` is the runtime discovery API for the active artifact. Package names
     are not a substitute for capability checks when consumers supply a custom generated artifact.

2. `merman-wasm` remains explicitly browser/JS WASM.
   - It may use wasm-bindgen, serde-wasm-bindgen, and browser-compatible glue.
   - It must not be documented as the Typst or pure-WASM surface.

3. `merman-typst-plugin` owns Typst-compatible WASM.
   - `crates/merman-typst-plugin/wasm-profiles.json` owns the plugin ABI number plus exact feature
     sets and expected capabilities for every measured profile. The crate build generates the numeric
     ABI constant and wire bytes from it; package assembly, runtime smoke, and the size matrix
     consume the same descriptor.
   - Package builds must pass the exact `typst-wasm` import/export gate and invoke every ABI
     operation through the wasmi smoke gate before assembly.
   - The export gate distinguishes the five callable ABI operations from linker support metadata;
     only `memory` and the immutable `i32` globals `__data_end` and `__heap_base` may accompany
     those operations.
  - The publish/default plugin artifact enables `svg`, `analysis`, `layout-cytoscape`, and `layout-elk`; the public
    package API exposes canonical diagnostics analysis schema 1 rather than the old
     validation projection.
   - `--no-default-features` is the bridge-only protocol artifact.
   - Typst package builds never enable `core-host`.
   - RaTeX is not a Typst capability while its dependency closure imports browser system-font
     discovery. A future math profile needs a separate zero-browser-import admission.

4. Rust/native feature ownership stays explicit.
   - Low-level crates and transport crates use empty defaults. The `merman` facade defaults to the
     result-named `complete-svg` aggregate; it does not compile system adapters merely because a
     caller wants deterministic SVG.
   - CLI, Web, Typst, Node, and native packages use owner-specific artifact profiles with direct
     leaf features. A profile is the exact build recipe; it is not exposed as a repository-wide
     Cargo preset.
   - Constrained hosts choose `default-features = false` recipes intentionally and query the
     generated runtime catalog before using optional operations.

## Success Metrics

| Metric | Target | Measurement |
| --- | --- | --- |
| Browser publication default | Default npm entry uses `browser-full` | `npm run prepack --prefix platforms/web` rejects a non-full default artifact unless an explicit local override is supplied |
| Public browser packages | Every wrapper has matching TS, wasm-bindgen, WASM, profile metadata, exports, and size evidence | package contract/smoke checks, the Web descriptor, and `xtask wasm-size-matrix` |
| Editor Worker surface | `./editor` exposes parser-backed language APIs on Web transport API 3/runtime-catalog schema 1 without render/ASCII/ELK | Web contract tests plus Playground Worker browser tests |
| Browser profile evidence | All named browser profiles build and report accurate capabilities | `npm run build:surfaces --prefix platforms/web`, package smoke, and profile manifests |
| Runtime capability discovery | Active artifact reports compiled capabilities | `bindingCapabilities()` returns booleans and legacy artifacts fall back to full capabilities |
| Typst import boundary | Only the two `typst_env` protocol imports are present | `cargo run -p xtask -- profile-budget check-wasm --profile typst-wasm --wasm <plugin.wasm>` |
| Typst export boundary | Exactly five callable ABI operations, `memory`, and immutable `i32` `__data_end`/`__heap_base` linker globals are present; every other export is rejected | The shared Wasmi module-surface validator used by `profile-budget check-wasm` and the Typst package builder |
| Typst execution boundary | Plugin can be loaded by a Typst-compatible host and every Typst ABI-2 operation matches the selected profile | `cargo run -p xtask -- build-typst-package --profile publish`, followed by `cargo run -p xtask -- typst-package-smoke --profile publish --skip-wasm-build` |
| Surface documentation | Browser and Typst/pure-WASM surfaces are not conflated | `docs/release/PACKAGE_SURFACES.md`, `docs/FEATURES.md`, and README surface sections |

## Alternatives Considered

### Option A: Multiple npm packages

Publish `@mermanjs/web-core`, `@mermanjs/web-render`, and `@mermanjs/web-editor` as independent
packages.

- Pros: package installation makes the selected dependency posture explicit.
- Cons: multiplies versions, provenance, release workflows, and shared TypeScript type ownership.
- Decision: rejected. Capability-specific subpaths provide small artifacts while one npm package
  keeps versioning and shared contract generation atomic.

### Option B: Make the slim browser artifact the npm default

Change `@mermanjs/web` to publish `browser-render` or `browser-core` by default.

- Pros: smaller default download.
- Cons: breaking behavior for callers using ASCII or full-core behavior; surprises users who expect
  existing default APIs to work.
- Decision: rejected. `browser-full` remains the default until a versioned migration is designed.

### Option C: Treat `merman-wasm` as the generic WASM crate

Make `merman-wasm` cover browser, pure-WASM, and Typst by feature switches.

- Pros: fewer crates and fewer names.
- Cons: conflates wasm-bindgen browser glue with Typst's import contract; makes import regressions
  harder to reason about; weakens package documentation.
- Decision: rejected. `merman-wasm` is browser-specific; `merman-typst-plugin` is Typst-specific.

### Option D: Delay Typst surface documentation until full package publication

Keep `merman-typst-plugin` as an internal probe until the Typst package wrapper is complete.

- Pros: avoids documenting an experimental package surface too early.
- Cons: hides an important compatibility boundary; makes release gates less visible; risks future
  browser-WASM changes regressing Typst import compatibility.
- Decision: rejected. Document the transport as experimental and gate it now, while keeping Typst
  registry publication separate.

## Risks And Mitigations

| Risk | Severity | Likelihood | Mitigation |
| --- | --- | --- | --- |
| Users assume every named artifact profile is a public npm package | Medium | Medium | Generate the exact package list from release/package manifests; document evidence-only profiles separately |
| A package ships the wrong WASM capability set | High | Low | Bind wrapper, profile metadata, generated declarations, smoke, ABI, and size budget in one release gate |
| Editor helpers drift from LSP/parser semantics | High | Low | Keep behavior in `merman-editor-core`; run it through the dedicated Web transport API 3/runtime-catalog schema 1 Worker rather than TypeScript heuristics |
| Typst docs imply full package readiness from transport smoke | Medium | Medium | Label Typst package publication as manual/future; document smoke as transport validation only |
| Browser changes reintroduce JS imports into Typst builds | High | Low | Keep `profile-budget check-wasm --profile typst-wasm` and `typst-package-smoke` as release gates; package assembly runs the shared plugin validator internally |
| A stale or different-profile WASM is assembled into the Typst package | High | Low | Use a profile-owned artifact manifest, validate it on `--skip-wasm-build`, and replace package directories transactionally |
| Feature defaults become unclear across crates | Medium | Medium | Record defaults in `docs/FEATURES.md`, README, and package surface docs |
| Source-build profile sizes are compared across unlike surfaces | Low | Medium | Use `xtask wasm-size-matrix` with explicit `browser` and `typst` surfaces |

## Consequences

- Browser package and Typst transport boundaries remain separate, while Rust/native defaults now
  make the complete SVG result explicit and low-level crates no longer inherit unrelated adapters.
- Browser consumers can choose a capability-specific public subpath without splitting package
  versioning across multiple npm names.
- Playground language intelligence can load the editor-only Worker artifact instead of duplicating
  the full renderer artifact.
- Typst work has a concrete, testable transport gate before registry packaging.
- Future independent browser packages, a changed default artifact, or a reusable public engine/session
  API require a new ADR or migration plan.
- Playground runtime, BFCache, Compare realms, and benchmark ownership remain application concerns
  defined by ADR-0074; they are not exported from `@mermanjs/web`.
