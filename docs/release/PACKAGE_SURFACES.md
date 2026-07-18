# Package Surfaces

Status: draft release planning notes.
Last updated: 2026-07-18

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
| Web/WASM | `@mermanjs/web` | `release-web.yml` | npm | Browser/JS WASM package built through wasm-bindgen. The default entry point is full and ELK-bearing; `./core`, `./render`, `./render-only`, `./ascii`, `./editor`, and `./full` are capability-specific package subpaths. This is not the Typst/pure-wasm surface. |
| VS Code | `merman-vscode` platform VSIX | `vscode-extension.yml` + `release-preflight.yml` | GitHub Actions artifact; Marketplace is `credential-blocked` | The VS Code manifest version is stable SemVer, for example `0.8.0`; workspace prereleases are packaged with the VSIX pre-release marker. |
| Typst WASM | `packages/typst/merman` Typst package backed by `merman-typst-plugin` | manual `typst/packages` PR | Typst package registry | Uses wasm-minimal-protocol and must stay separate from wasm-bindgen browser glue. The publishable package wasm is artifact-owned and ELK-bearing because Typst users import the wasm rather than enabling Cargo features. |
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
  the tiny registry.
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
| `platforms/web/pkg/merman_wasm_bg.wasm` | `browser-full` | 8,005,078 | 3,102,009 | 2,169,792 | `docs/release/WASM_SIZE_BUDGETS.json` |
| `platforms/web/pkg/core/merman_wasm_bg.wasm` | `browser-core` | 2,154,903 | 807,038 | 611,055 | measured |
| `platforms/web/pkg/render/merman_wasm_bg.wasm` | `browser-render` | 6,078,214 | 2,293,073 | 1,571,777 | measured |
| `platforms/web/pkg/render-only/merman_wasm_bg.wasm` | `browser-render-only` | 5,840,419 | 2,190,623 | 1,512,306 | measured |
| `platforms/web/pkg/ascii/merman_wasm_bg.wasm` | `browser-ascii` | 2,283,094 | 853,450 | 656,666 | measured |
| `platforms/web/pkg/editor/merman_wasm_bg.wasm` | `browser-editor` | 2,927,915 | 1,187,134 | 903,197 | measured 2026-07-18 |
| `platforms/web/pkg/full/merman_wasm_bg.wasm` | `browser-full` | 8,005,098 | 3,102,011 | 2,168,495 | measured |

For the current Typst render artifact, also run:

```bash
cargo build -p merman-typst-plugin --profile wasm-size --target wasm32-unknown-unknown
cargo run -p xtask -- profile-budget check-wasm --profile typst-wasm --wasm target/wasm32-unknown-unknown/wasm-size/merman_typst_plugin.wasm
cargo run -p xtask -- typst-plugin-smoke --wasm target/wasm32-unknown-unknown/wasm-size/merman_typst_plugin.wasm
```

Recent observed matrix values:

| Surface | Preset | Default features | Extra features | Raw bytes | Stripped bytes | gzip bytes | brotli bytes |
| --- | --- | --- | --- | ---: | ---: | ---: | ---: |
| Browser | `browser-bridge` | no | none | 2,893,014 | 2,033,390 | 622,835 | 474,317 |
| Browser | `browser-core` | no | `analysis` | 3,784,327 | 2,704,773 | 813,612 | 601,591 |
| Browser | `browser-render` | no | `render`, `analysis` | 9,502,672 | 7,255,388 | 2,262,060 | 1,496,657 |
| Browser | `browser-render-only` | no | `render` | 9,066,926 | 6,960,684 | 2,161,312 | 1,444,596 |
| Browser | `browser-ascii` | no | `ascii` | 3,967,774 | 2,847,511 | 857,787 | 645,191 |
| Browser | `browser-editor` | no | `core-full`, `editor-language` | 4,985,370 | 3,631,466 | 1,192,175 | 883,268 |
| Browser | `browser-full-no-elk` | no | `core-full`, `core-host`, `render`, `analysis`, `ascii`, `editor-language` | 11,794,522 | 8,959,931 | 2,893,161 | 1,944,418 |
| Browser | `browser-full` | yes | none | 12,696,661 | 9,606,018 | 3,081,122 | 2,075,165 |
| Browser | `browser-ratex-math` | yes | `ratex-math` | 15,970,579 | 12,328,550 | 4,026,127 | 2,768,761 |
| Typst | `typst-bridge` | no | none | 51,364 | 36,355 | 14,213 | 12,134 |
| Typst | `typst-render-only-no-elk` | no | `render` | 6,751,372 | 5,201,760 | 1,554,893 | 1,122,068 |
| Typst | `typst-render-analysis-no-elk` | no | `render`, `analysis` | 7,266,306 | 5,586,691 | 1,690,307 | 1,211,638 |
| Typst | `typst-core-full-no-elk` | no | `render`, `analysis`, `core-full` | 8,802,256 | 6,779,460 | 2,146,234 | 1,545,056 |
| Typst | `typst-full-elk` | yes | none | 8,305,206 | 6,313,325 | 1,904,120 | 1,354,361 |
| Typst | `typst-ratex-math` | yes | `ratex-math` | 12,022,147 | 9,203,967 | 2,886,822 | 2,057,657 |
