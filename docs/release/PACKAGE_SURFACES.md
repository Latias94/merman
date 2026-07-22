# Package Surfaces

Status: maintained release surface contract.
Last updated: 2026-07-21

This document records merman package surfaces, current readiness, and the CI gates that protect
each publication or artifact build.

## Current Surfaces

<!-- BEGIN GENERATED RELEASE SURFACES -->
| Contract ID | Surface | Entry point | Support | Channels |
| --- | --- | --- | --- | --- |
| `rust-core` | Rust core crate | `merman-core` | `published` | `crates.io` (`published`) |
| `rust-analysis` | Rust analysis crate | `merman-analysis` | `published` | `crates.io` (`published`) |
| `rust-render` | Rust render facade | `merman` | `published` | `crates.io` (`published`) |
| `rust-editor-lsp` | Rust editor and LSP crates | `merman-lsp` | `published` | `github-release` (`published`), `crates.io` (`published`) |
| `rustdoc` | Rustdoc integration | `merman-rustdoc` | `published` | `crates.io` (`published`) |
| `cli` | Command line interface | `merman-cli` | `published` | `github-release` (`published`), `crates.io` (`published`) |
| `homebrew` | Homebrew CLI formula | `merman-cli` | `stable-only` | `homebrew-core` (`published`) |
| `web-wasm` | Browser WebAssembly package | `@mermanjs/web` | `published` | `npm` (`published`), `crates.io` (`published`) |
| `vscode` | VS Code extension | `merman-vscode` | `artifact-only` | `github-actions-vsix` (`artifact-only`), `vs-marketplace` (`credential-blocked`) |
| `c-abi` | Native C ABI | `merman-ffi` | `published` | `crates.io` (`published`) |
| `python` | Python UniFFI package | `merman` | `published` | `pypi` (`published`), `github-release-wheels` (`published`), `crates.io` (`published`) |
| `flutter` | Flutter and Dart FFI package | `merman` | `published` | `pub.dev` (`published`) |
| `android` | Android JNI package | `io.merman:merman-android` | `artifact-only` | `github-release-aar` (`artifact-only`), `maven-central` (`credential-blocked`) |
| `apple` | Apple Swift package and XCFramework | `Merman` | `artifact-only` | `github-release-xcframework` (`artifact-only`), `swiftpm-remote-binary` (`registry-blocked`) |
| `typst` | Typst package and WASM plugin | `packages/typst/merman` | `manual-registry` | `typst-registry` (`manual-registry`), `crates.io` (`published`) |
<!-- END GENERATED RELEASE SURFACES -->

The generated table is the public view of `SURFACES.json`; refresh it with
`python scripts/verify-release-surfaces.py --write-docs`. Foundational Rust crates are intentionally
hidden from this user-choice table while remaining part of the maintainer and crates.io contract.
Homebrew/core owns formula publication. Merman verifies the external stable formula rather than
claiming to publish it from this repository. Registry-blocked and credential-blocked channels are
not silently presented as available install paths.

## Release Surface Set

The repository-owned release surface set is:

1. crates.io for Rust crates, using `docs/release/PUBLISH_ORDER.md`.
2. GitHub Release artifacts for `merman-cli` and `merman-lsp`.
3. GitHub Release XCFramework packaging for Apple.
4. GitHub Release wheels and PyPI publishing for Python.
5. pub.dev for Flutter.
6. GitHub Release AAR for Android.
7. npm publishing for `@mermanjs/web` through `release-web.yml` after trusted publisher setup.
8. Platform VSIX artifacts for the independently versioned VS Code extension through
   `vscode-extension.yml`; Marketplace publishing needs an explicit release decision and credentials
   before it is enabled.

## Release Status States

`docs/release/SURFACES.json` is the machine-readable source of truth for public package surfaces and
release channels. The status terms are intentionally user-facing:

| State | Meaning |
| --- | --- |
| `published` | The registry or install channel is expected to publish for the selected release kind. |
| `artifact-only` | CI produces or uploads an artifact, but no registry package is published from this repo yet. |
| `credential-blocked` | The registry path is designed but blocked on credentials, signing, or marketplace setup. |
| `registry-blocked` | The registry package contract needs more release-manifest design before publication. |
| `manual-registry` | Publication happens through a manual registry PR or external review flow. |
| `not-built` | The surface is documented but not produced by current automation. |
| `not-applicable` | The channel does not apply to the selected release kind, such as Homebrew for prereleases. |

For a user-facing package-choice table:

```bash
python scripts/release-status.py --view public
```

For maintainer readiness against a candidate version:

```bash
VERSION="<version>"
python scripts/release-status.py --version "$VERSION" --view maintainer
python scripts/release-status.py --version "$VERSION" --probe --format json
```

`--probe` is best-effort and should be used after publication when network and registry tools are
available. It reports observed status separately from the declared release state.

## CI Gates

Merman CI keeps publication separate from validation:

- `python scripts/verify-release-surfaces.py` checks this document, `SURFACES.json`, package
  manifests, Web source subpaths, feature/preset names, and release workflow paths.
- `cargo run -p xtask -- verify-mermaid-reference` checks that the selected Mermaid and companion
  behavior graph, package locks, generated runtime labels, and provenance agree.
- `cargo run -p xtask -- verify-editor-token-descriptor` checks the single editor-language token
  descriptor, its Rust/Web/VS Code projections, the generated VS Code token/theme contributions,
  and the exact 35-family plus recovery packed-token evidence before LSP or browser packages build.
- `cargo run -p xtask -- verify-web-diagram-catalog` and
  `verify-playground-example-catalog` keep the published full/editor family set and source-backed
  examples aligned.
- `platform-script-syntax` checks Python, Apple, and Flutter shell entry points.
- `python-uniffi-wheel` builds and imports a local Python UniFFI wheel.
- `flutter-package-check` runs `flutter pub get`, `flutter analyze`, and Dart formatting.
- `apple-ffi-smoke` builds `Merman.xcframework` and validates the root Swift package.
- `web-npm-dry-run` builds the TypeScript/WASM package and runs `npm pack --dry-run`.
- `vscode-extension.yml` and the VS Code preflight job build platform runtime binaries, package a
  VSIX, and verify package contents, target platform, stable manifest version, and pre-release
  marker.
- `homebrew.yml` checks the published Homebrew formula, runs `brew livecheck`, installs
  `merman-cli`, and renders a smoke diagram from the installed binary.

Release preflight is manual and publish-free. Crates and cargo-dist remain tag-driven after
preflight passes. Platform publishing is manual so a fixed workflow on `main` can build and upload
assets for an existing release tag without moving that tag. Registry credentials still need to be
configured per surface before the corresponding workflow can publish.

## Browser WASM Presets

ADR-0069 keeps `@mermanjs/web` as one npm package. The default
entry point uses the `browser-full` preset. The package also publishes opt-in subpaths for
`browser-core`, `browser-render`, `browser-render-only`, `browser-ascii`, `browser-editor`, and
explicit `browser-full` artifacts. Source,
CI, and local package builds can still choose a different browser preset through
`platforms/web/scripts/build-wasm.mjs`; the TypeScript wrapper exposes `bindingCapabilities()` so
callers can discover the active artifact's compiled capabilities after initialization, including
whether `editor_language` is compiled. It also exposes `selectedRegistryProfile()` and
`diagramFamilyCapabilities()` so local slim builds can report the actual full/tiny diagram
parser/render matrix they contain, plus `lintRuleCatalog()` so editor integrations can discover the
governed analyzer rule table and its evidence references without hard-coding them.
The editor-bearing presets expose `editorSemanticTokenDescriptor()` and
`editorSemanticTokens()`. The descriptor is generated from
`editor-language/token-descriptor-v1.json`; the token query returns the descriptor's validated
five-word LSP-relative UTF-16 sequence as a `Uint32Array`. Token codes, modifier bits, precedence,
legend indices, sorting, and overlap resolution are not redefined by the TypeScript wrapper.
The published subpaths are capability-specific TypeScript entry points: they type-re-export the
shared public option/result types and stable helper values, then export only the runtime wrappers
that the subpath supports. Unsupported render, ASCII, or editor wrappers are absent from slim
subpaths instead of being exported as throwing stubs.
`platforms/web/web-surface-descriptor.json` is the single machine-readable preset/subpath mapping;
the WASM builder, surface generator, Python release verifier, package checks, and tests consume it
without extracting private JavaScript names or source formatting.

| Preset | Default features | Extra features | Intended use |
| --- | ---: | --- | --- |
| `browser-core` | no | `analysis` | Browser wasm-bindgen transport plus metadata, analysis, facts, and validation. Render, parse, layout, ASCII, and editor-language entry points are unavailable. |
| `browser-render` | no | `render`, `analysis` | SVG/parse/layout artifact with metadata, analysis, facts, and validation over the minimal core profile. Editor-language entry points are unavailable. |
| `browser-render-only` | no | `render` | SVG/parse/layout artifact with metadata only. Analysis, validation, lint catalog, ASCII, and editor-language entry points are unavailable. |
| `browser-ascii` | no | `ascii` | ASCII/Unicode artifact with metadata only. Analysis, validation, lint catalog, render, parse, layout, and editor-language entry points are unavailable. |
| `browser-editor` | no | `core-full`, `editor-language` | Full 35-family catalog, analysis, validation, facts, and parser-backed editor APIs for a dedicated Worker. Render, parse/layout JSON, ASCII, host, and ELK entry points are unavailable. |
| `browser-full` | yes | none | Default npm artifact: full core profile, browser host capabilities, SVG/layout/parse/validate, ASCII, editor-language APIs, and ELK layout. Includes EPL-backed `merman-elk-layered`. |
| `browser-full-no-elk` | no | `core-full`, `core-host`, `render`, `analysis`, `ascii`, `editor-language` | Evidence preset for the same browser surface without ELK. Keeps editor-language enabled. Not the npm default. |
| `browser-ratex-math` | yes | `ratex-math` | Full browser artifact plus RaTeX math rendering support and ELK layout. Keeps editor-language enabled. Includes EPL-backed `merman-elk-layered`. |

`npm run check:contracts --prefix platforms/web` compares the wasm-bindgen full declarations with
the hand-written TypeScript wrapper, `MermanWasmModule`, `bindSurfaceRuntime()`, and the generated
capability-specific subpath entry templates. It also rejects value star re-exports and unsupported
runtime wrapper exports in slim subpaths. `npm run prepack --prefix platforms/web` runs that
contract check and
requires `browser-full` unless `MERMAN_WEB_ALLOW_NON_DEFAULT_PRESET=1` is set for an intentional
local slim package. This protects the public npm package from accidentally publishing a slim artifact
under the default import path. It also checks that every package subpath has matching TypeScript,
wasm-bindgen, WASM, and preset manifest artifacts.

There is intentionally no `@mermanjs/web/analysis` subpath. `@mermanjs/web/core` is already the
smallest analysis-capable browser artifact because analysis, validation, registry metadata, and
document facts all share the same minimal core bindings. A separate analysis alias would expand the
public API without reducing the WASM payload.

## Compatibility And Migration Notes

Current release semantics are intentionally explicit:

- Low-level Rust `merman/render` enables SVG/layout support only. `merman/elk-layout` is the
  explicit feature that pulls `merman-layout-elk` and the EPL-2.0 `merman-elk-layered` source port.
- CLI defaults remain compatibility-oriented and enable `elk-layout` through the CLI crate's own
  default feature set.
- Native FFI defaults stay conservative: `render` does not imply ELK. Downstream native artifacts
  that want ELK must enable `elk-layout` or publish a distinct full artifact.
- Rust source callers that match `merman_core::Error::DiagramParse` must migrate from the old raw
  message field to `diagnostic: ParseDiagnostic`. The displayed error message remains compatible,
  and callers can use `diagnostic.message()`, `span()`, `span_kind()`, and `code()` for structured
  parser metadata. The current native ABI version remains 2 during alpha development. Its exact
  host text-measurement contract has 19 operations with contiguous codes 0 through 18 and tagged
  result kinds. Operation 17 (`create-text-middle-bbox-y-offset`, signed length) measures
  Architecture createText under inherited `dominant-baseline="middle"`; it is not interchangeable
  with operation 14's ordinary createText bbox y. Operation 18 (`raw-bbox-height`, length) measures
  direct raw SVG `<text>.getBBox().height`. Callbacks and generated bindings must implement the
  current ABI 2 shape and handle the complete operation range.
- `@mermanjs/web` keeps the existing default import path and publishes `browser-full` there. Slim
  browser artifacts are available through `@mermanjs/web/core`, `@mermanjs/web/render`,
  `@mermanjs/web/render-only`, `@mermanjs/web/ascii`, and `@mermanjs/web/editor`; these slim
  subpaths omit unsupported runtime wrapper exports. The editor subpath retains `core-full` so its
  Worker covers the same 35 logical families as the Playground and LSP rather than silently using
  the tiny registry. Its diagnostics, detection, code actions, completion, structure, navigation,
  rename, and packed semantic tokens are projections of one analyzed document snapshot in the
  dedicated Worker; there is no Monarch or regex fallback.
  `@mermanjs/web/full` is the explicit full-preset subpath.
- Browser WASM ABI 2 is required by the current 0.8 wrapper and render-environment contract.
  `bindingCapabilities()` reports the active browser artifact's compiled capabilities, including
  whether `analysis` and `editor_language` are available. `selectedRegistryProfile()` and
  `diagramFamilyCapabilities()` report the selected diagram registry profile and registered
  parser/render family facts. `lintRuleCatalog()` is available on analysis-capable artifacts and
  reports analyzer rule ids, evidence references, default profiles, origins, configurability, and
  fixability. Consumers that load custom artifacts must keep the generated wasm-bindgen artifact and
  TypeScript wrapper from the same package
  version/ABI; the 0.8 wrapper does not provide compatibility fallback for custom browser artifacts
  with a different ABI or without these metadata exports.
- `merman-wasm` is the browser/wasm-bindgen crate. It should not be used as evidence that an
  artifact is Typst-compatible or pure-WASM compatible.
- `merman-typst-plugin` is the Typst-compatible transport. Its Cargo default and public package
  profile `publish` both resolve to canonical profile `typst-full-elk`, built with exactly
  `render`, `analysis`, `core-full`, and `elk-layout`. The closed ABI 2 surface exports
  `abi_version`, `package_version`, `capabilities_json`, `render_svg_json`, and `analyze_json`.
  `--no-default-features` builds the internal protocol bridge only; it is not a public package
  profile. The Typst plugin replaces caller-provided `resources` with the fixed `constrained`
  policy at every call, so document input cannot select a trusted or unbounded host profile.
- Publishable Typst artifacts live under
  `target/typst-wasm-artifacts/<canonical-profile>/`. The profile directory contains only the
  stripped WASM and `manifest.json`; the manifest binds the canonical profile and features, ABI,
  package and Mermaid versions, input tree, toolchain, effective Rust flags, and artifact digest.
  `build-typst-package --skip-wasm-build` verifies that provenance before reuse and copies it into
  the package as `merman_typst_plugin.manifest.json`. The package also contains
  `merman_package.manifest.json`, which binds that artifact to a frozen, fully enumerated wrapper
  self-contained wrapper and legal-material snapshot. Staging validates exact shape and bytes, then
  rechecks live source identity before atomic installation. Raw Cargo output under
  `target/wasm-build/` is private build input and must not be consumed by CI or release packaging.
- A future public browser package, additional npm export path, or changed default artifact needs a
  new migration note and release decision.

## Release Gates By Surface

| Surface | Required local gate before release changes |
| --- | --- |
| Browser npm package | `cargo run -p xtask -- verify-mermaid-reference`; `cargo run -p xtask -- verify-editor-token-descriptor`; `cargo run -p xtask -- verify-web-diagram-catalog`; `npm run check:contracts --prefix platforms/web`; `npm run build --prefix platforms/web`; `npm run smoke --prefix platforms/web`; `npm run prepack --prefix platforms/web` |
| VS Code extension | `cargo build --release --locked -p merman-lsp -p merman-cli`; `npm run test --prefix tools/vscode-extension`; `npm run prepare:binaries --prefix tools/vscode-extension`; `npm run package --prefix tools/vscode-extension -- --target <target> --out <file>`; `npm run verify:vsix --prefix tools/vscode-extension -- --vsix <file> --platform <target> --target <target>` |
| Browser preset evidence | `npm run build:wasm:core --prefix platforms/web`; `npm run build:wasm:render --prefix platforms/web`; `npm run build:wasm:render-only --prefix platforms/web`; `npm run build:wasm:ascii --prefix platforms/web`; `MERMAN_WEB_ALLOW_NON_DEFAULT_PRESET=1 npm run prepack --prefix platforms/web` |
| Browser/Typst size evidence | `cargo run -p xtask -- wasm-size-matrix --budget-file docs/release/WASM_SIZE_BUDGETS.json` |
| Typst transport | `cargo run --locked -p xtask -- build-typst-package --profile publish`; `cargo run --locked -p xtask -- profile-budget check-wasm --profile typst-wasm --wasm target/typst-wasm-artifacts/typst-full-elk/merman_typst_plugin.wasm`; `cargo run --locked -p xtask -- typst-plugin-smoke --profile publish --wasm target/typst-wasm-artifacts/typst-full-elk/merman_typst_plugin.wasm`; `cargo run --locked -p xtask -- typst-package-smoke --profile publish --skip-wasm-build`; PR CI compiles package examples and a preview import smoke with Typst 0.15.0, and push CI additionally runs `wasm-size-matrix` plus the tests-only package smoke on Typst 0.15.0. The skipped build path remains a gate because it verifies the profile-owned provenance manifest before copying the artifact. |

## WASM Size Matrix

Use the xtask size matrix before changing WASM feature presets:

```bash
cargo run -p xtask -- wasm-size-matrix --surface browser
cargo run -p xtask -- wasm-size-matrix --surface typst
cargo run -p xtask -- wasm-size-matrix --budget-file docs/release/WASM_SIZE_BUDGETS.json
```

The command builds `wasm-size` artifacts and prints raw, stripped, gzip, and brotli bytes for named
presets. gzip and brotli are measured from the stripped artifact unless `--no-strip` is used. The
budget file is intentionally a regression guard with headroom, not a product target. It keeps
browser/wasm-bindgen and Typst/wasm-minimal-protocol measurements separate so package changes do
not accidentally compare unlike surfaces.

The generated `@mermanjs/web` package also builds through the workspace `wasm-size` profile. The
2026-07-22 measurements include the ICU4X collation data used to reproduce Mermaid's Swimlane
`localeCompare` ordering rather than substituting Rust byte ordering:

| Package artifact | Preset | Raw bytes | gzip bytes | brotli bytes | Budget source |
| --- | --- | ---: | ---: | ---: | --- |
| `platforms/web/pkg/merman_wasm_bg.wasm` | `browser-full` | 10,199,393 | 3,840,060 | 2,707,333 | `docs/release/WASM_SIZE_BUDGETS.json` |
| `platforms/web/pkg/core/merman_wasm_bg.wasm` | `browser-core` | 2,652,820 | 925,486 | 690,626 | measured |
| `platforms/web/pkg/render/merman_wasm_bg.wasm` | `browser-render` | 8,000,016 | 2,930,737 | 2,063,928 | measured |
| `platforms/web/pkg/render-only/merman_wasm_bg.wasm` | `browser-render-only` | 7,754,567 | 2,824,643 | 1,995,754 | measured |
| `platforms/web/pkg/ascii/merman_wasm_bg.wasm` | `browser-ascii` | 2,775,409 | 970,159 | 735,690 | measured |
| `platforms/web/pkg/editor/merman_wasm_bg.wasm` | `browser-editor` | 3,450,375 | 1,315,950 | 992,116 | measured |
| `platforms/web/pkg/full/merman_wasm_bg.wasm` | `browser-full` | 10,199,393 | 3,840,060 | 2,707,333 | measured |

For the current Typst publish artifact, also run:

```bash
cargo run --locked -p xtask -- build-typst-package --profile publish
cargo run --locked -p xtask -- profile-budget check-wasm --profile typst-wasm --wasm target/typst-wasm-artifacts/typst-full-elk/merman_typst_plugin.wasm
cargo run --locked -p xtask -- typst-plugin-smoke --profile publish --wasm target/typst-wasm-artifacts/typst-full-elk/merman_typst_plugin.wasm
cargo run --locked -p xtask -- typst-package-smoke --profile publish --skip-wasm-build
```

Observed 2026-07-19 matrix values, measured from the exact profile descriptors with compression
applied to `wasm-tools strip --all` output:

| Surface | Preset | Default features | Extra features | Raw bytes | Stripped bytes | gzip bytes | brotli bytes |
| --- | --- | --- | --- | ---: | ---: | ---: | ---: |
| Browser | `browser-bridge` | no | none | 3,132,879 | 2,232,230 | 681,924 | 521,464 |
| Browser | `browser-core` | no | `analysis` | 4,088,849 | 2,956,873 | 886,906 | 662,096 |
| Browser | `browser-render` | no | `render`, `analysis` | 10,036,036 | 7,688,279 | 2,389,785 | 1,598,148 |
| Browser | `browser-render-only` | no | `render` | 9,593,092 | 7,389,779 | 2,285,872 | 1,536,742 |
| Browser | `browser-ascii` | no | `ascii` | 4,273,421 | 3,100,695 | 931,323 | 707,493 |
| Browser | `browser-editor` | no | `core-full`, `editor-language` | 5,313,319 | 3,901,365 | 1,270,356 | 941,247 |
| Browser | `browser-full-no-elk` | no | `core-full`, `core-host`, `render`, `analysis`, `ascii`, `editor-language` | 12,670,877 | 9,702,444 | 3,119,444 | 2,111,940 |
| Browser | `browser-full` | yes | none | 13,573,233 | 10,348,509 | 3,307,400 | 2,248,122 |
| Browser | `browser-ratex-math` | yes | `ratex-math` | 16,842,133 | 13,069,360 | 4,252,461 | 2,927,677 |
| Typst | `typst-bridge` | no | none | 62,063 | 46,146 | 19,500 | 16,574 |
| Typst | `typst-render-only-no-elk` | no | `render` | 8,796,764 | 7,009,094 | 2,212,276 | 1,484,121 |
| Typst | `typst-render-analysis-no-elk` | no | `render`, `analysis` | 9,166,305 | 7,259,433 | 2,297,231 | 1,535,192 |
| Typst | `typst-core-full-no-elk` | no | `render`, `analysis`, `core-full` | 10,543,235 | 8,341,671 | 2,732,327 | 1,855,750 |
| Typst | `typst-full-elk` (`publish`) | no | `render`, `analysis`, `core-full`, `elk-layout` | 11,445,996 | 8,987,989 | 2,919,785 | 1,986,128 |
