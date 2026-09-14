# Flutter theme authoring budget matrix — 2026-09-14

## Scope

Native source: `44d81d033fa5c9f1305b36142551d8989d6c2738`.
The Flutter wrapper already bypasses generic engine-constructor resource admission for all
three theme authoring operations. The old shared smoke only exercised resource errors through
materialization, leaving support discovery and preset export uncovered.

The existing Native Assets smoke now runs both one-shot and reusable consumers through:

- materialize-theme-json, describe-theme-support-json and export-theme-preset-json with a valid
  explicit max_theme_encoded_bytes ceiling, comparing media type and JSON with unbounded calls;
- the same three operations with max_theme_encoded_bytes=1 and input `{}`, requiring
  resource-limit-exceeded and the exact resource envelope;
- the existing authoring diagnostic and support golden vectors.

Input admission for support and preset ID export returns the resource envelope without an
extra authoring diagnostic envelope. Materialization retains the shared authoring diagnostics.
Preset export also admits its expanded definition, so its successful budget covers the exported
recipe; it is not limited to the preset ID's byte length.

## Actual artifact and results

The existing package-owned macOS library was stale (support claim revision 79 versus current
fixture revision 86). Rebuilt it using the existing `python3 platforms/flutter/build-native.py host`
recipe: `flutter-desktop-native`, native-distribution, aarch64-apple-darwin. The build validates
its C ABI symbols, sets the package install name and signs the packaged dynamic library.

- Packaged library SHA-256: `b559666a07b974ddb1a9ac01195b49b48d87508dbedc2d938547c45917ed50dd`.
- Packaged library size: 21827456 bytes.
- Smoke source SHA-256: `df51341400653af705514f33a6520fdf2676d7b6e40986e3b0eef2ea6260ccdd`.
- `dart analyze tool/theme_authoring_smoke.dart`: no issues.
- `dart run tool/abi3_contract_test.dart`: passed.
- Actual `dart run tool/theme_authoring_smoke.dart`: passed through Native Assets; per consumer,
  2 materializations, 15 support queries, 5 errors and 3 budgeted operation comparisons.
- In the disposable verification clone, temporarily reducing the one-shot predicate to only
  materialization made the smoke fail: expected resource-limit-exceeded, got invalid-argument.
  Reinstating the current predicate and rerunning the same smoke passed. The main wrapper was
  never modified for this mutation. Each support/export bypass was also removed individually;
  both independent mutations were detected with the same status mismatch.

The smoke is already invoked by scripts/verify-platform-bindings.py, which builds the host
native asset before execution. No new verification script or CI lane was added. Repository
fixtures and this smoke remain excluded from the published Dart package.

This is macOS arm64 host evidence only. It does not claim Android/iOS/Linux/Windows completion,
public catalog promotion, or C7a contract freeze. The package dynamic library is an ignored build
artifact, not a tracked binary change.
