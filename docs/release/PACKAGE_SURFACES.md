# Package Surfaces

Status: draft release planning notes.
Last updated: 2026-06-06

This document records merman package surfaces, current readiness, and the CI gates that should
protect them before any registry publication is enabled.

## Current Surfaces

| Surface | Current package | Release workflow | Channel | Notes |
| --- | --- | --- | --- | --- |
| Rust crates | workspace crates listed in `PUBLISH_ORDER.md` | `release-crates.yml` | crates.io | Publishes in dependency order. `xtask` remains private. |
| CLI | `merman-cli` binary archives | `release.yml` | GitHub Release | Existing cargo-dist workflow. |
| Apple | SwiftPM `Merman` with `Merman.xcframework` | `release-apple.yml` | GitHub Release asset | Builds, zips, computes checksum, and uploads assets without moving the release tag. |
| Python | `merman` wheels | `release-python.yml` | GitHub Release + PyPI | Builds Linux, macOS, and Windows wheels, repairs Linux metadata, and publishes through PyPI Trusted Publishing. |
| Flutter | `merman` | `release-flutter.yml` | pub.dev | Builds and injects Android, iOS, macOS, Windows, and Linux native artifacts before publishing. Real pub.dev publication must run from a pushed `v*` tag; manual runs are validation-only. |
| Android | `io.merman:merman-android` Android library module | `release-android.yml` | GitHub Release AAR | Maven publication metadata is declared; Maven Central publishing still needs Central Portal credentials and signing secrets. |
| Web/WASM | `@merman/web` | none | npm planned | Package metadata is present; npm Trusted Publishing/provenance workflow still needs to be added before registry publication. |
| React Native | none | none | none | Add only if a React Native API/package is built. |
| JVM | none | none | none | Add only if a JVM-specific wrapper is built. |

## First Release Set

The first release set is:

1. crates.io for Rust crates, using `docs/release/PUBLISH_ORDER.md`.
2. GitHub Release artifacts for `merman-cli`.
3. SwiftPM/GitHub Release packaging for Apple.
4. GitHub Release wheels and PyPI publishing for Python.
5. pub.dev for Flutter.
6. GitHub Release AAR for Android.

## CI Gates

Merman CI keeps publication separate from validation:

- `platform-script-syntax` checks Python, Apple, and Flutter shell entry points.
- `python-uniffi-wheel` builds and imports a local Python UniFFI wheel.
- `flutter-package-check` runs `flutter pub get`, `flutter analyze`, and Dart formatting.
- `apple-ffi-smoke` builds `Merman.xcframework` and validates the root Swift package.

Release preflight is manual and publish-free. Crates and cargo-dist remain tag-driven after
preflight passes. Platform publishing is manual so a fixed workflow on `main` can build and upload
assets for an existing release tag without moving that tag. Registry credentials still need to be
configured per surface before the corresponding workflow can publish.
