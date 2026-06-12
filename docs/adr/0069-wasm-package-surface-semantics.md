# ADR 0069: WASM Package Surface Semantics

- Status: accepted
- Date: 2026-06-10

## Context

The WASM feature-surface slimming lane split concerns that were previously easy to conflate:

- browser WebAssembly delivered through `wasm-bindgen` and the `@mermanjs/web` TypeScript package;
- Typst/pure WebAssembly delivered through `wasm-minimal-protocol` and a `wasmi` host;
- Cargo feature profiles for full Mermaid compatibility, host capabilities, and output capabilities.

`merman-wasm` remains a browser/JavaScript transport crate. It is not a generic "runs in every
WASM host" artifact, because wasm-bindgen glue, browser imports, and TypeScript package helpers are
part of its intended contract.

At the same time, `merman-typst-plugin` now proves the Typst transport boundary: the default render
artifact imports only the two `typst_env` wasm-minimal-protocol functions, exports `memory`, and
passes a wasmi smoke call returning SVG JSON.

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

1. `@mermanjs/web` remains one npm package with one published WASM artifact per version.
   - The published artifact is `browser-full`.
   - `browser-core`, `browser-render`, `browser-ascii`, and `browser-ratex-math` are source-build
     presets for local builds and CI evidence, not public npm entry points yet.
   - `bindingCapabilities()` is the runtime discovery API for the active artifact.

2. `merman-wasm` remains explicitly browser/JS WASM.
   - It may use wasm-bindgen, serde-wasm-bindgen, and browser-compatible glue.
   - It must not be documented as the Typst or pure-WASM surface.

3. `merman-typst-plugin` owns Typst-compatible WASM.
   - Package builds must pass the `typst-wasm` import/export gate and the wasmi smoke gate.
   - The default plugin artifact enables `render`.
   - `--no-default-features` is the bridge-only protocol artifact.
   - `core-full`, `core-host`, and `ratex-math` are opt-ins; Typst package builds should not enable
     `core-host`.

4. Rust/native defaults stay compatibility-oriented.
   - This ADR does not change default feature behavior for normal Rust, CLI, browser, or native
     binding consumers.
   - Constrained hosts must opt into no-default feature profiles intentionally.

## Success Metrics

| Metric | Target | Measurement |
| --- | --- | --- |
| Browser publication default | npm package uses `browser-full` | `npm run prepack --prefix platforms/web` rejects non-full artifacts unless `MERMAN_WEB_ALLOW_NON_DEFAULT_PRESET=1` |
| Browser preset evidence | All named browser presets build and smoke according to capability metadata | `npm run build:wasm:* --prefix platforms/web` plus `npm run smoke --prefix platforms/web` |
| Runtime capability discovery | Active artifact reports compiled capabilities | `bindingCapabilities()` returns booleans and legacy artifacts fall back to full capabilities |
| Typst import boundary | Only the two `typst_env` protocol imports are present | `cargo run -p xtask -- profile-budget check-wasm --profile typst-wasm --wasm <plugin.wasm>` |
| Typst execution boundary | Plugin can be loaded by a Typst-compatible host and return SVG JSON | `cargo run -p xtask -- typst-plugin-smoke --wasm <plugin.wasm>` |
| Surface documentation | Browser and Typst/pure-WASM surfaces are not conflated | `docs/release/PACKAGE_SURFACES.md`, `docs/FEATURES.md`, and README surface sections |

## Alternatives Considered

### Option A: Multiple npm packages or export paths now

Publish `@mermanjs/web-core`, `@mermanjs/web-render`, or package export variants immediately.

- Pros: consumers can install smaller browser artifacts directly.
- Cons: locks public API and semver commitments before enough usage evidence; increases release
  workflow and documentation surface; risks users importing unsupported slim artifacts as if they
  were the default full package.
- Decision: rejected for this lane. Source-build presets give evidence without freezing the public
  npm surface.

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
| Users assume browser slim presets are stable public npm entry points | Medium | Medium | Document presets as source-build only; keep package default `browser-full`; prepack rejects non-full artifacts by default |
| Typst docs imply full package readiness from transport smoke | Medium | Medium | Label Typst package publication as manual/future; document smoke as transport validation only |
| Browser changes reintroduce JS imports into Typst builds | High | Low | Keep `profile-budget check-wasm --profile typst-wasm` and `typst-plugin-smoke` as release gates |
| Feature defaults become unclear across crates | Medium | Medium | Record defaults in `docs/FEATURES.md`, README, and package surface docs |
| Source-build preset sizes are compared across unlike surfaces | Low | Medium | Use `xtask wasm-size-matrix` with explicit `browser` and `typst` surfaces |

## Consequences

- Existing browser and Rust/native users keep compatible defaults.
- Browser size work can continue behind named source-build presets without multiplying npm package
  contracts.
- Typst work has a concrete, testable transport gate before registry packaging.
- Future public slim browser packages or export paths require a new ADR or migration plan.
