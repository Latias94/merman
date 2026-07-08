# Package Surfaces

Status: draft release planning notes.
Last updated: 2026-07-07

This document records merman package surfaces, current readiness, and the CI gates that should
protect them before any registry publication is enabled.

## Current Surfaces

| Surface | Current package | Release workflow | Channel | Notes |
| --- | --- | --- | --- | --- |
| Rust crates | workspace crates listed in `PUBLISH_ORDER.md` | `release-crates.yml` | crates.io | Publishes in dependency order. `xtask` remains private. |
| CLI | `merman-cli` binary archives | `release.yml` | GitHub Release | Existing cargo-dist workflow. |
| CLI (Homebrew) | `merman-cli` formula | `homebrew.yml` | Homebrew/core | Homebrew/core owns the formula and autobump flow; this repo only checks formula metadata, livecheck, install, and smoke behavior. |
| Apple | Swift wrapper plus `Merman.xcframework` | `release-apple.yml` | GitHub Release asset | Builds, zips, computes checksum, and uploads assets without moving the release tag. Direct remote SwiftPM consumption still needs a release manifest strategy with URL + checksum committed before tagging. |
| Python | `merman` wheels | `release-python.yml` | GitHub Release + PyPI | Builds Linux, macOS, and Windows wheels, repairs Linux metadata, and publishes through PyPI Trusted Publishing. |
| Flutter | `merman` | `release-flutter.yml` | pub.dev | Builds and injects Android, iOS, macOS, Windows, and Linux native artifacts before publishing. Real pub.dev publication must run from a pushed `v*` tag; manual runs are validation-only. |
| Android | `io.merman:merman-android` Android library module | `release-android.yml` | GitHub Release AAR | Maven publication metadata is declared; Maven Central publishing still needs Central Portal credentials and signing secrets. |
| Web/WASM | `@mermanjs/web` | `release-web.yml` | npm | Browser/JS WASM package built through wasm-bindgen. The default entry point is full and ELK-bearing; `./core`, `./render`, `./ascii`, and `./full` are opt-in package subpaths. This is not the Typst/pure-wasm surface. |
| VS Code | `merman-vscode` platform VSIX | `vscode-extension.yml` + `release-preflight.yml` | GitHub Actions artifact; Marketplace publish not enabled | The VS Code manifest version is stable SemVer, for example `0.8.0`; workspace prereleases are packaged with the VSIX pre-release marker. |
| Typst WASM | `merman` Typst package backed by `merman-typst-plugin` | manual `typst/packages` PR | Typst package registry | Uses wasm-minimal-protocol and must stay separate from wasm-bindgen browser glue. The publishable package wasm is artifact-owned and ELK-bearing because Typst users import the wasm rather than enabling Cargo features. |
| React Native | none | none | none | Add only if a React Native API/package is built. |
| JVM | none | none | none | Add only if a JVM-specific wrapper is built. |

## First Release Set

The first release set is:

1. crates.io for Rust crates, using `docs/release/PUBLISH_ORDER.md`.
2. GitHub Release artifacts for `merman-cli`.
3. GitHub Release XCFramework packaging for Apple.
4. GitHub Release wheels and PyPI publishing for Python.
5. pub.dev for Flutter.
6. GitHub Release AAR for Android.
7. npm publishing for `@mermanjs/web` through `release-web.yml` after trusted publisher setup.
8. Platform VSIX artifacts for VS Code through `vscode-extension.yml`; Marketplace publishing needs
   an explicit release decision and credentials before it is enabled.

## CI Gates

Merman CI keeps publication separate from validation:

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

WFS-090 decision, updated by PR20 hardening: keep `@mermanjs/web` as one npm package. The default
entry point uses the `browser-full` preset. The package also publishes opt-in subpaths for
`browser-core`, `browser-render`, `browser-render-only`, `browser-ascii`, and explicit
`browser-full` artifacts. Source,
CI, and local package builds can still choose a different browser preset through
`platforms/web/scripts/build-wasm.mjs`; the TypeScript wrapper exposes `bindingCapabilities()` so
callers can discover the active artifact's compiled capabilities after initialization, including
whether `editor_language` is compiled. It also exposes `selectedRegistryProfile()` and
`diagramFamilyCapabilities()` so local slim builds can report the actual full/tiny diagram
parser/render matrix they contain, plus `lintRuleCatalog()` so editor integrations can discover the
governed analyzer rule table and its evidence references without hard-coding them.
The published subpaths are capability-specific TypeScript entry points: they type-re-export the
shared public option/result types and stable helper values, then export only the runtime wrappers
that the subpath supports. Unsupported render, ASCII, or editor wrappers are absent from slim
subpaths instead of being exported as throwing stubs.

| Preset | Default features | Extra features | Intended use |
| --- | ---: | --- | --- |
| `browser-core` | no | `analysis` | Browser wasm-bindgen transport plus metadata, analysis, facts, and validation. Render, parse, layout, ASCII, and editor-language entry points are unavailable. |
| `browser-render` | no | `render`, `analysis` | SVG/parse/layout artifact with metadata, analysis, facts, and validation over the minimal core profile. Editor-language entry points are unavailable. |
| `browser-render-only` | no | `render` | SVG/parse/layout artifact with metadata only. Analysis, validation, lint catalog, ASCII, and editor-language entry points are unavailable. |
| `browser-ascii` | no | `ascii` | ASCII/Unicode artifact with metadata only. Analysis, validation, lint catalog, render, parse, layout, and editor-language entry points are unavailable. |
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
  parser metadata. Native ABI version 2 remains valid for this release line because ABI v2 had not
  been externally released before the structured diagnostic change.
- `@mermanjs/web` keeps the existing default import path and publishes `browser-full` there. Slim
  browser artifacts are available through `@mermanjs/web/core`, `@mermanjs/web/render`,
  `@mermanjs/web/render-only`, and `@mermanjs/web/ascii`; these slim subpaths omit unsupported
  runtime wrapper exports.
  `@mermanjs/web/full` is the explicit full-preset subpath.
- Browser WASM ABI 2 is the first ABI that requires the metadata exports used by the 0.8 wrapper.
  `bindingCapabilities()` reports the active browser artifact's compiled capabilities, including
  whether `analysis` and `editor_language` are available. `selectedRegistryProfile()` and
  `diagramFamilyCapabilities()` report the selected diagram registry profile and registered
  parser/render family facts. `lintRuleCatalog()` is available on analysis-capable artifacts and
  reports analyzer rule ids, evidence references, default profiles, origins, configurability, and
  fixability. Consumers that load custom artifacts must keep the generated wasm-bindgen artifact and
  TypeScript wrapper from the same package
  version/ABI; the 0.8 wrapper does not provide compatibility fallback for pre-ABI-2 browser
  artifacts that lack these metadata exports.
- `merman-wasm` is the browser/wasm-bindgen crate. It should not be used as evidence that an
  artifact is Typst-compatible or pure-WASM compatible.
- `merman-typst-plugin` is the Typst-compatible transport. Its default artifact enables SVG render,
  validation analysis, and ELK. `--no-default-features` builds the protocol bridge only. The Typst
  plugin injects the `typst-package` resource profile when callers omit `resources`.
- A future public browser package, additional npm export path, or changed default artifact needs a
  new migration note and release decision.

## Release Gates By Surface

| Surface | Required local gate before release changes |
| --- | --- |
| Browser npm package | `npm run check:contracts --prefix platforms/web`; `npm run build --prefix platforms/web`; `npm run smoke --prefix platforms/web`; `npm run prepack --prefix platforms/web` |
| VS Code extension | `cargo build --release --locked -p merman-lsp -p merman-cli`; `npm run test --prefix tools/vscode-extension`; `npm run prepare:binaries --prefix tools/vscode-extension`; `npm run package --prefix tools/vscode-extension -- --target <target> --out <file>`; `npm run verify:vsix --prefix tools/vscode-extension -- --vsix <file> --platform <target> --target <target>` |
| Browser preset evidence | `npm run build:wasm:core --prefix platforms/web`; `npm run build:wasm:render --prefix platforms/web`; `npm run build:wasm:render-only --prefix platforms/web`; `npm run build:wasm:ascii --prefix platforms/web`; `MERMAN_WEB_ALLOW_NON_DEFAULT_PRESET=1 npm run prepack --prefix platforms/web` |
| Browser/Typst size evidence | `cargo run -p xtask -- wasm-size-matrix --budget-file docs/release/WASM_SIZE_BUDGETS.json` |
| Typst transport | `cargo build -p merman-typst-plugin --profile wasm-size --target wasm32-unknown-unknown`; `cargo run -p xtask -- profile-budget check-wasm --profile typst-wasm --wasm target/wasm32-unknown-unknown/wasm-size/merman_typst_plugin.wasm`; `cargo run -p xtask -- typst-plugin-smoke --wasm target/wasm32-unknown-unknown/wasm-size/merman_typst_plugin.wasm`; PR CI compiles Typst package examples and a preview import smoke with Typst 0.15.0, and push CI additionally runs `wasm-size-matrix` plus `typst-package-smoke --skip-wasm-build --tests-only` on Typst 0.15.0. |

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

The generated `@mermanjs/web` package also builds through the workspace `wasm-size` profile. Recent
package artifacts measured during local release checks are:

| Package artifact | Preset | Raw bytes | gzip bytes | brotli bytes | Budget source |
| --- | --- | ---: | ---: | ---: | --- |
| `platforms/web/pkg/merman_wasm_bg.wasm` | `browser-full` | 6,936,158 | 2,649,766 | 1,958,841 | `docs/release/WASM_SIZE_BUDGETS.json` |
| `platforms/web/pkg/core/merman_wasm_bg.wasm` | `browser-core` | 1,974,289 | 741,530 | 565,420 | measured |
| `platforms/web/pkg/render/merman_wasm_bg.wasm` | `browser-render` | 4,914,321 | 1,813,940 | 1,340,229 | measured |
| `platforms/web/pkg/render-only/merman_wasm_bg.wasm` | `browser-render-only` | 4,486,446 | 1,653,761 | 1,220,562 | measured |
| `platforms/web/pkg/ascii/merman_wasm_bg.wasm` | `browser-ascii` | 2,974,716 | 1,213,252 | 931,113 | measured |
| `platforms/web/pkg/full/merman_wasm_bg.wasm` | `browser-full` | 6,936,158 | 2,649,766 | 1,958,841 | measured |

For the current Typst render artifact, also run:

```bash
cargo build -p merman-typst-plugin --profile wasm-size --target wasm32-unknown-unknown
cargo run -p xtask -- profile-budget check-wasm --profile typst-wasm --wasm target/wasm32-unknown-unknown/wasm-size/merman_typst_plugin.wasm
cargo run -p xtask -- typst-plugin-smoke --wasm target/wasm32-unknown-unknown/wasm-size/merman_typst_plugin.wasm
```

Recent observed matrix values:

| Surface | Preset | Default features | Extra features | Raw bytes | Stripped bytes | gzip bytes | brotli bytes |
| --- | --- | --- | --- | ---: | ---: | ---: | ---: |
| Browser | `browser-core` | no | `analysis` | 2,314,537 | 1,607,616 | 488,344 | 371,239 |
| Browser | `browser-render` | no | `render`, `analysis` | 7,142,939 | 5,323,303 | 1,567,143 | 1,135,983 |
| Browser | `browser-render-only` | no | `render` | 7,364,323 | 5,475,747 | 1,614,544 | 1,168,221 |
| Browser | `browser-ascii` | no | `ascii` | 4,053,135 | 2,972,267 | 1,000,885 | 745,996 |
| Browser | `browser-full-no-elk` | no | `core-full`, `core-host`, `render`, `analysis`, `ascii`, `editor-language` | 9,139,597 | 6,824,157 | 2,136,333 | 1,536,058 |
| Browser | `browser-full` | yes | none | 10,115,464 | 7,502,959 | 2,335,802 | 1,666,379 |
| Browser | `browser-ratex-math` | yes | `ratex-math` | 13,398,073 | 10,231,577 | 3,277,885 | 2,349,234 |
| Typst | `typst-bridge` | no | none | 48,359 | 34,296 | 13,553 | 11,482 |
| Typst | `typst-render-only-no-elk` | no | `render` | 6,751,372 | 5,201,760 | 1,554,893 | 1,122,068 |
| Typst | `typst-render-analysis-no-elk` | no | `render`, `analysis` | 6,541,087 | 5,056,278 | 1,508,214 | 1,093,218 |
| Typst | `typst-core-full-no-elk` | no | `render`, `analysis`, `core-full` | 8,164,316 | 6,307,341 | 1,989,926 | 1,434,971 |
| Typst | `typst-full-elk` | yes | none | 7,514,778 | 5,735,845 | 1,707,876 | 1,227,566 |
| Typst | `typst-ratex-math` | yes | `ratex-math` | 11,228,422 | 8,620,566 | 2,684,627 | 1,928,257 |
