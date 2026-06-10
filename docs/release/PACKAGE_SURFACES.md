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
| Apple | Swift wrapper plus `Merman.xcframework` | `release-apple.yml` | GitHub Release asset | Builds, zips, computes checksum, and uploads assets without moving the release tag. Direct remote SwiftPM consumption still needs a release manifest strategy with URL + checksum committed before tagging. |
| Python | `merman` wheels | `release-python.yml` | GitHub Release + PyPI | Builds Linux, macOS, and Windows wheels, repairs Linux metadata, and publishes through PyPI Trusted Publishing. |
| Flutter | `merman` | `release-flutter.yml` | pub.dev | Builds and injects Android, iOS, macOS, Windows, and Linux native artifacts before publishing. Real pub.dev publication must run from a pushed `v*` tag; manual runs are validation-only. |
| Android | `io.merman:merman-android` Android library module | `release-android.yml` | GitHub Release AAR | Maven publication metadata is declared; Maven Central publishing still needs Central Portal credentials and signing secrets. |
| Web/WASM | `@mermanjs/web` | `release-web.yml` | npm | Browser/JS WASM package built through wasm-bindgen. This is not the Typst/pure-wasm surface. Package metadata and release workflow are present. The npm package exists; subsequent releases use npm Trusted Publishing/provenance once the trusted publisher is configured. |
| Typst WASM | `merman` Typst package | manual `typst/packages` PR | Typst package registry | Uses wasm-minimal-protocol and must stay separate from wasm-bindgen browser glue. Initial package publication is tracked outside npm/crates release automation. |
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

Release preflight is manual and publish-free. Crates and cargo-dist remain tag-driven after
preflight passes. Platform publishing is manual so a fixed workflow on `main` can build and upload
assets for an existing release tag without moving that tag. Registry credentials still need to be
configured per surface before the corresponding workflow can publish.

## WASM Size Matrix

Use the xtask size matrix before changing WASM feature presets:

```bash
cargo run -p xtask -- wasm-size-matrix --surface browser
cargo run -p xtask -- wasm-size-matrix --surface typst
```

The command builds `wasm-size` artifacts and prints raw and stripped bytes for named presets. It
keeps browser/wasm-bindgen and Typst/wasm-minimal-protocol measurements separate so package changes
do not accidentally compare unlike surfaces.

Observed on 2026-06-10:

| Surface | Preset | Default features | Extra features | Raw bytes | Stripped bytes |
| --- | --- | --- | --- | ---: | ---: |
| Browser | `browser-core` | no | none | 1,862,617 | 1,345,196 |
| Browser | `browser-render` | no | `render` | 7,412,346 | 5,610,023 |
| Browser | `browser-ascii` | no | `ascii` | 3,874,343 | 2,929,536 |
| Browser | `browser-full` | yes | none | 8,866,039 | 6,718,352 |
| Browser | `browser-ratex-math` | yes | `ratex-math` | 12,145,965 | 9,446,738 |
| Typst | `typst-bridge` | no | none | 47,287 | 33,412 |
| Typst | `typst-render` | yes | none | 7,025,842 | 5,417,005 |
| Typst | `typst-core-full` | yes | `core-full` | 8,090,998 | 6,263,908 |
| Typst | `typst-ratex-math` | yes | `ratex-math` | 10,544,347 | 8,240,147 |
