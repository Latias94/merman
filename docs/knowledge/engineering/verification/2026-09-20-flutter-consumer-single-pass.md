---
type: Verification Record
title: Flutter Native Assets consumer after XML reference single-pass integration
timestamp: 2026-09-20
source_commit: bdb209e1166af960121ce0e2b3231d9929d3bca1
---

# Result

The Flutter macOS ARM64 Native Assets library was rebuilt from clean integrated source
`bdb209e1166af960121ce0e2b3231d9929d3bca1` with the `flutter-desktop-native` recipe and passed
its exported ABI symbol contract. The package owner then ran the real bundled Native Assets
entrypoint.

The current source lane passed the ABI 3 contract test, public Dart example smoke and theme
authoring smoke. One-shot and reusable consumers each passed two materializations, 22 support
queries, five structured errors and three budgeted authoring operations. The dry-run package
validation reported an 8 MB compressed archive with zero warnings and listed the native asset and
legal payloads.

| Artifact | Identity |
| --- | --- |
| `libmerman_ffi.dylib` | 21,564,512 bytes; SHA-256 `371fc03c92f88c3ee87e29e7246616d6782d96655147d53c97754d98accf956e` |

# Evidence and limits

Logs are retained under `target/bench/experiments/flutter-bdb-20260920/`, including the native
profile build, package-native output, ABI contract, example smoke, theme authoring smoke and
`dart pub publish --dry-run`. This is local macOS ARM64 Native Assets evidence. It does not prove
Android/iOS/Linux/Windows execution, an installed Flutter application, pub.dev publication or the
full hosted release matrix. No publication or package upload occurred.
