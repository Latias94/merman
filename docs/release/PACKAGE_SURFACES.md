# Package Surfaces

Status: draft release planning notes.
Last updated: 2026-06-10

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
| Web/WASM | `@mermanjs/web` | `release-web.yml` | npm | Browser/JS WASM package built through wasm-bindgen. This is not the Typst/pure-wasm surface. Package metadata and release workflow are present. The npm package exists; subsequent releases use npm Trusted Publishing/provenance once the trusted publisher is configured. |
| Typst WASM | `merman` Typst package backed by `merman-typst-plugin` | manual `typst/packages` PR | Typst package registry | Uses wasm-minimal-protocol and must stay separate from wasm-bindgen browser glue. The current plugin wasm passes the Typst import/export gate and wasmi smoke; initial package publication is tracked outside npm/crates release automation. |
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

## CI Gates

Merman CI keeps publication separate from validation:

- `platform-script-syntax` checks Python, Apple, and Flutter shell entry points.
- `python-uniffi-wheel` builds and imports a local Python UniFFI wheel.
- `flutter-package-check` runs `flutter pub get`, `flutter analyze`, and Dart formatting.
- `apple-ffi-smoke` builds `Merman.xcframework` and validates the root Swift package.
- `web-npm-dry-run` builds the TypeScript/WASM package and runs `npm pack --dry-run`.
- `homebrew.yml` checks the published Homebrew formula, runs `brew livecheck`, installs
  `merman-cli`, and renders a smoke diagram from the installed binary.

Release preflight is manual and publish-free. Crates and cargo-dist remain tag-driven after
preflight passes. Platform publishing is manual so a fixed workflow on `main` can build and upload
assets for an existing release tag without moving that tag. Registry credentials still need to be
configured per surface before the corresponding workflow can publish.

## Browser WASM Presets

WFS-090 decision: keep `@mermanjs/web` as one npm package and one published artifact per version for
now. The published package uses the `browser-full` preset. Source, CI, and local package builds can
choose a different browser preset through `platforms/web/scripts/build-wasm.mjs`; the TypeScript
wrapper exposes `bindingCapabilities()` so callers can discover the active artifact's compiled
capabilities after initialization.

| Preset | Default features | Extra features | Intended use |
| --- | ---: | --- | --- |
| `browser-core` | no | none | Browser wasm-bindgen transport and metadata only. Render, parse, layout, validation, and ASCII entry points report unsupported capability errors. |
| `browser-render` | no | `render` | SVG/parse/layout/validation artifact over the minimal core profile. |
| `browser-ascii` | no | `ascii` | ASCII/Unicode artifact. It still carries the full core registry because the browser ASCII crate depends on the full core/host profile. |
| `browser-full` | yes | none | Default npm artifact: full core profile, browser host capabilities, SVG/layout/parse/validate, and ASCII. |
| `browser-ratex-math` | yes | `ratex-math` | Full browser artifact plus RaTeX math rendering support. |

`npm run prepack --prefix platforms/web` requires `browser-full` unless
`MERMAN_WEB_ALLOW_NON_DEFAULT_PRESET=1` is set for an intentional local slim package. This protects
the public npm package from accidentally publishing a slim artifact under the default import path.

## Compatibility And Migration Notes

Current release semantics are intentionally conservative:

- Rust, CLI, native bindings, and browser package defaults remain compatibility-oriented. No default
  feature behavior changed for normal Rust or browser consumers.
- `@mermanjs/web` keeps the existing default import path and publishes `browser-full`. Slim browser
  presets are source-build presets only; they are not npm subpackages or package export paths.
- `bindingCapabilities()` reports the active browser artifact's compiled capabilities. Consumers
  that load an older artifact without this export should treat it as the historical full browser
  artifact.
- `merman-wasm` is the browser/wasm-bindgen crate. It should not be used as evidence that an
  artifact is Typst-compatible or pure-WASM compatible.
- `merman-typst-plugin` is the Typst-compatible transport. Its default artifact enables SVG render;
  `--no-default-features` builds the protocol bridge only. Typst package builds should keep
  `core-host` disabled.
- A future public slim browser package, npm export path, or changed default artifact needs a new
  migration note and release decision.

## Release Gates By Surface

| Surface | Required local gate before release changes |
| --- | --- |
| Browser full npm default | `npm run build --prefix platforms/web`; `npm run smoke --prefix platforms/web`; `npm run prepack --prefix platforms/web` |
| Browser preset evidence | `npm run build:wasm:core --prefix platforms/web`; `npm run build:wasm:render --prefix platforms/web`; `npm run build:wasm:ascii --prefix platforms/web`; `MERMAN_WEB_ALLOW_NON_DEFAULT_PRESET=1 npm run prepack --prefix platforms/web` |
| Browser/Typst size evidence | `cargo run -p xtask -- wasm-size-matrix --budget-file docs/release/WASM_SIZE_BUDGETS.json` |
| Typst transport | `cargo build -p merman-typst-plugin --profile wasm-size --target wasm32-unknown-unknown`; `cargo run -p xtask -- profile-budget check-wasm --profile typst-wasm --wasm target/wasm32-unknown-unknown/wasm-size/merman_typst_plugin.wasm`; `cargo run -p xtask -- typst-plugin-smoke --wasm target/wasm32-unknown-unknown/wasm-size/merman_typst_plugin.wasm` |

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
current default `browser-full` package artifact is:

| Package artifact | Raw bytes | gzip bytes | brotli bytes | Budget source |
| --- | ---: | ---: | ---: | --- |
| `platforms/web/pkg/merman_wasm_bg.wasm` | 5,580,151 | 2,135,543 | 1,589,052 | `docs/release/WASM_SIZE_BUDGETS.json` |

For the current Typst render artifact, also run:

```bash
cargo build -p merman-typst-plugin --profile wasm-size --target wasm32-unknown-unknown
cargo run -p xtask -- profile-budget check-wasm --profile typst-wasm --wasm target/wasm32-unknown-unknown/wasm-size/merman_typst_plugin.wasm
cargo run -p xtask -- typst-plugin-smoke --wasm target/wasm32-unknown-unknown/wasm-size/merman_typst_plugin.wasm
```

Observed on 2026-06-10:

| Surface | Preset | Default features | Extra features | Raw bytes | Stripped bytes | gzip bytes | brotli bytes |
| --- | --- | --- | --- | ---: | ---: | ---: | ---: |
| Browser | `browser-core` | no | none | 1,863,948 | 1,346,079 | 433,339 | 332,192 |
| Browser | `browser-render` | no | `render` | 7,413,955 | 5,611,115 | 1,653,063 | 1,196,259 |
| Browser | `browser-ascii` | no | `ascii` | 3,875,744 | 2,930,489 | 994,799 | 736,899 |
| Browser | `browser-full` | yes | none | 8,867,749 | 6,719,540 | 2,107,768 | 1,512,064 |
| Browser | `browser-ratex-math` | yes | `ratex-math` | 12,147,547 | 9,447,798 | 3,052,089 | 2,188,356 |
| Typst | `typst-bridge` | no | none | 47,287 | 33,412 | 13,388 | 11,361 |
| Typst | `typst-render` | yes | none | 6,445,189 | 4,990,451 | 1,486,500 | 1,078,513 |
| Typst | `typst-core-full` | yes | `core-full` | 8,091,296 | 6,264,136 | 1,973,465 | 1,422,134 |
| Typst | `typst-ratex-math` | yes | `ratex-math` | 10,544,626 | 8,240,356 | 2,575,773 | 1,857,459 |
