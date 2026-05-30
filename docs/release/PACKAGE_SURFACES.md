# Package Surfaces

Status: draft release planning notes.
Last updated: 2026-05-31

This document records merman package surfaces, current readiness, and the CI gates that should
protect them before any registry publication is enabled.

## Current Surfaces

| Surface | Current package | Release workflow | Channel | Notes |
| --- | --- | --- | --- | --- |
| Rust crates | workspace crates listed in `PUBLISH_ORDER.md` | `release-crates.yml` | crates.io | Publishes in dependency order. `xtask` remains private. |
| CLI | `merman-cli` binary archives | `release.yml` | GitHub Release | Existing cargo-dist workflow. |
| Apple | SwiftPM `Merman` with `Merman.xcframework` | `release-apple.yml` | GitHub Release + SwiftPM tag | Builds, zips, computes checksum, and patches `Package.swift` on the release tag. |
| Python | `merman` wheels | `release-python.yml` | GitHub Release | Builds Linux, macOS, and Windows wheels. PyPI publishing needs a wheel policy decision. |
| Flutter | `merman` | `release-flutter.yml` | pub.dev | Injects Android native libraries before publishing. |
| Android | `io.merman` Android library module | `release-android.yml` | GitHub Release AAR | Maven Central publishing needs Gradle signing/POM metadata and namespace confirmation. |
| Web/WASM | none | none | none | Add only if a WASM API/package is built. |
| React Native | none | none | none | Add only if a React Native API/package is built. |
| JVM | none | none | none | Add only if a JVM-specific wrapper is built. |

## First Release Set

The first release set is:

1. crates.io for Rust crates, using `docs/release/PUBLISH_ORDER.md`.
2. GitHub Release artifacts for `merman-cli`.
3. SwiftPM/GitHub Release packaging for Apple.
4. GitHub Release wheels for Python.
5. pub.dev for Flutter.
6. GitHub Release AAR for Android.

## CI Gates

Merman CI keeps publication separate from validation:

- `platform-script-syntax` checks Python and Apple shell entry points.
- `python-uniffi-wheel` builds and imports a local Python UniFFI wheel.
- `flutter-package-check` runs `flutter pub get`, `flutter analyze`, and Dart formatting.
- `apple-ffi-smoke` builds `Merman.xcframework` and validates the root Swift package.

Release workflows are tag-driven and separate from CI. Registry credentials still need to be
configured per surface before the corresponding workflow can publish.
