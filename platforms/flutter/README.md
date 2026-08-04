# Merman For Flutter

[![pub package](https://img.shields.io/pub/v/merman.svg)](https://pub.dev/packages/merman) [![License: MIT](https://img.shields.io/badge/license-MIT-yellow)](https://github.com/Latias94/merman/blob/main/LICENSE-MIT) [![License: Apache-2.0](https://img.shields.io/badge/license-Apache--2.0-blue)](https://github.com/Latias94/merman/blob/main/LICENSE-APACHE)

Render and analyze Mermaid diagrams in Flutter without a browser or JavaScript runtime. The plugin is a Dart-friendly facade over Merman's native ABI 3 table and ships the native library for its supported Flutter targets.

> `Merman.open()` accepts only the exact native package version bundled with this Dart release and requires the current ABI 3 metadata slot. `Merman.openPath(...)` accepts any library that passes ABI 3 minimum-prefix discovery. A compatible five-slot producer can still render and analyze, but metadata queries report `DART_NATIVE_CONTRACT_ERROR`. ABI 2 symbols and pre-freeze ABI 3 layouts are not fallback paths.

## Install

Install the current prerelease from pub.dev:

```sh
flutter pub add 'merman:^0.8.0-alpha.4'
```

For local source development, depend on a matching checkout instead:

```yaml
dependencies:
  merman:
    path: /path/to/merman/platforms/flutter
```

Run `flutter pub get` after adding the dependency.

## Requirements

- Dart 3.4 or newer and Flutter 3.10 or newer
- Android API 23 or newer with Java 17
- iOS 13 or newer
- macOS 11 or newer

The public Dart API can load a host-owned compatible native library with `Merman.openPath(...)`, but this package itself depends on the Flutter SDK and is not distributed as a standalone Dart package. `mermanPackageVersion` is the exact version expected by `Merman.open()`.

## Render A Diagram

`Merman` owns a default native engine. Close it when the application no longer needs it.

```dart
import 'package:merman/merman.dart';

const source = 'flowchart TD\nA[Hello] --> B[World]';
final merman = Merman.open();
try {
  final svg = merman.renderSvg(source);
  print(svg.substring(0, 4)); // <svg
} finally {
  merman.close();
}
```

The generic API is available when an application needs to select an output at runtime. It returns a structured `MermanOperationResult` with the selected operation, media type, copied Dart bytes, and decoded metadata:

```dart
final output = merman.execute(MermanOperation.semanticJson, source);
final semantic = output.jsonObject;
```

Convenience methods are projections over `execute` and cover SVG, PNG, JPEG, PDF, ASCII, semantic/layout/analysis JSON, document analysis, and validation. A native artifact can intentionally omit some outputs. Inspect `merman.runtimeCatalog` before enabling optional UI or export paths; an unavailable operation raises `MermanUnsupportedOperationException` rather than silently falling back. `MermanUnknownOperationException` identifies an ID outside the ABI vocabulary; `MermanMissingCapabilityException.capabilityId` identifies the backend absent from a valid native request.

## Inspect Native Metadata

The typed metadata APIs expose the loaded artifact's diagram, ASCII, parser/render, lint, Mermaid theme, and presentation catalogs. Results are copied into Dart-owned immutable values and cached on the `Merman` instance. Decoders require the documented fields while tolerating additive JSON fields from a compatible newer producer. Presentation IDs remain open strings so compatible producers can add presets, profiles, and aspects without requiring a Dart enum update.

```dart
final diagrams = merman.supportedDiagrams();
final ascii = merman.asciiCapabilities();
final families = merman.diagramFamilyCapabilities();
final lintRules = merman.lintRuleCatalog();
final themes = merman.supportedThemes();
final presentation = merman.presentationCatalog();
```

All six methods use the appended ABI 3 `metadata_collect` table slot. `Merman.open()` requires that slot because the bundled Dart and native package versions are exact peers. `Merman.openPath(...)` deliberately keeps minimum-prefix compatibility with older ABI 3 producers and reports a contract error only if one of these metadata methods is called without the appended slot.

## Options And Resource Limits

Options passed when opening `Merman` or creating a reusable engine form the baseline for later calls. `execute` and every convenience method also accept `optionsJson`; request values deeply override that baseline for one call while unspecified nested values remain inherited. Select `runtime_policy` only at engine construction.

```dart
final merman = Merman.open(
  optionsJson: '''
    {
      "version": 2,
      "resources": {"profile": "constrained"},
      "svg": {"pipeline": "resvg-safe"}
    }
  ''',
);
```

Omitting `runtime_policy` keeps the engine deterministic even when the packaged library includes native adapters. Set `"runtime_policy":"native"` only when the operation should consult the host clock, time-zone rules, and random source. Generic operation metadata reports the selected policy, and a custom slim artifact missing a requested adapter raises `MermanUnsupportedOperationException`.

Use `constrained` for untrusted or multi-tenant input, `interactive` for cooperative local editing, and `trusted-native` for local native automation. `MermanResourceOptionsBuilder` is available when an application only needs to produce the shared resource-options fragment. Its profile is optional: omit it for a reusable request that must inherit the constructor ceiling, and use `MermanResourceOverrideId` for limits. The generic native operation envelope has no Dart-facing fields in schema 1, so the Flutter facade keeps it internal until a real per-output contract exists. The runtime catalog is the source of truth for the compiled capabilities and enforced resource limits. See the [options contract](https://github.com/Latias94/merman/blob/main/docs/bindings/OPTIONS_JSON.md) for profile selection and the complete option schema.

`MermanResourceLimitId` describes every catalog limit; `MermanResourceOverrideId` is the narrower option-construction ID set. Inspect `runtimeCatalog.resourceLimits` and `runtimeCatalog.resourceProfiles` for the loaded ABI 3 library's complete phase, description, minimum, hard-cap, applicable operation IDs, purpose, trust, recommendation, and nullable budget metadata. Those typed collections preserve additive IDs reported by a host-owned library; a `null` profile limit means unbounded, while hard caps are always finite.

For an older ABI 3 producer opened through `openPath()`, a schema 1 resource descriptor that predates `minimum_value` is interpreted with the historical positive-integer minimum of `1`. Current producers always report the field explicitly; the only zero-minimum limit, `max_document_diagrams`, was introduced together with that field.

## Reusable Engines And Text Measurement

Pass `textMeasurer:` to `Merman.open(...)` when the default engine needs host measurement, or use an independent `MermanReusableEngine` when a workflow needs its own options, lifecycle, or measurer. The optional synchronous callback is immutable constructor state.

```dart
final engine = merman.reusableEngine(
  textMeasurer: (request) {
    if (request.operation == MermanTextMeasurementOperation.measure) {
      return MermanTextMeasureResult.metrics(
        width: 42,
        height: 18,
        lineCount: 1,
      );
    }
    return null; // Use Merman's deterministic fallback for this operation.
  },
);
try {
  final svg = engine.renderSvg(source);
} finally {
  engine.close();
}
```

The callback is isolate-local and synchronous. Create, render with, and close the measured engine on the same Dart isolate. Do not call back into that engine from the measurer. Precompute or cache WebView, platform-channel, and font results instead of blocking inside the callback. The [host measurement guide](https://github.com/Latias94/merman/blob/main/docs/bindings/HOST_TEXT_MEASUREMENT.md#flutter--dart-ffi) describes result shapes and cache keys.

`close()` never waits. `MermanBusyException` means another operation is active, while `MermanReentrantCallException` means the same engine is inside its callback. Both failures retain the native engine token and callback registration so close can be retried. Only a successful close clears the Dart handle; later closes are idempotent.

Outputs are returned as owned Dart byte arrays. Internally, each written native result is released exactly once through its opaque ABI allocation token after the bytes and metadata are copied. The sole zero-token terminal case is native allocation-token exhaustion: `INTERNAL_ERROR` with the result otherwise untouched in its caller-initialized state. This keeps native pointers and allocators out of the public API.

## SVG Display

`renderSvg` returns SVG text; it intentionally does not choose a Flutter widget. Mermaid-compatible SVG can contain `<style>`, `<marker>`, and `<foreignObject>`. A WebView gives the closest browser behavior. Native SVG widgets and rasterizers can omit HTML labels, markers, or CSS features.

Choose the SVG pipeline for the final consumer:

- `parity` is the default Mermaid-compatible output for browser/WebView use.
- `readable` favors text fallbacks and inspectability.
- `resvg-safe` is the explicit choice for stricter SVG consumers, rasterizers, and PDF conversion.

Set the usual pipeline in `Merman.open(optionsJson: ...)` or `reusableEngine(optionsJson: ...)`. Override it for one call with `renderSvg(source, optionsJson: ...)` when required.

## Supported Platforms

| Platform | Packaged native artifact |
| --- | --- |
| Android | `arm64-v8a` and `x86_64` shared libraries |
| iOS | Dynamic `MermanFFI.xcframework` |
| macOS | Universal dylib for CocoaPods and XCFramework for SwiftPM |
| Linux | x86_64 or aarch64 shared library, depending on the release artifact |
| Windows | x86_64 DLL |

Flutter Web is not supported because this package uses `dart:ffi`; use [`@mermanjs/web`](https://www.npmjs.com/package/@mermanjs/web) in browsers. Verify the release archive before deployment because native target availability is release-specific.

Android package slices use the `flutter-android-native` C ABI recipe from `merman-ffi`. The Kotlin AAR's JNI transport lives in the separate internal `merman-android-jni` crate.

## Local Development

The checked-in low-level binding is generated with `ffigen` from the public C header. It discovers the frozen five-slot ABI 3 prefix with the minimum-prefix digest and performs no JNI, UniFFI, or per-operation symbol lookup. Application code uses `merman.dart` and never needs ABI pointers or record definitions.

```sh
cargo build -p merman-ffi --no-default-features --features svg,analysis,ascii,png,jpeg,pdf,layout-cytoscape,layout-elk,math,system-clock,system-timezone,system-random
cd platforms/flutter
flutter pub get
dart run ffigen --config ffigen.yaml
flutter analyze
dart run tool/abi3_contract_test.dart
dart run example/smoke.dart ../../target/debug/libmerman_ffi.dylib
```

Use `.so` on Linux and `.dll` on Windows. CI regenerates the binding, rejects a stale checked-in result, runs analyzer and deterministic malformed-error fuzz coverage, then exercises the facade against a real native library. Native artifact assembly and platform packaging are documented in the [Flutter/Dart FFI guide](https://github.com/Latias94/merman/blob/main/docs/bindings/FLUTTER_DART_FFI.md).

## Documentation And Releases

- [Package changelog](CHANGELOG.md)
- [Flutter/Dart binding guide](https://github.com/Latias94/merman/blob/main/docs/bindings/FLUTTER_DART_FFI.md)
- [Diagram coverage](https://github.com/Latias94/merman/blob/main/docs/alignment/STATUS.md)
- [Issue tracker](https://github.com/Latias94/merman/issues)
- [Source repository](https://github.com/Latias94/merman)

pub.dev is the supported registry channel for this package. CocoaPods and SwiftPM files inside the plugin are Flutter integration details, not separately published Dart packages.

## License And Notices

This package is available under MIT or Apache-2.0. See [`LICENSE`](LICENSE), [`THIRD_PARTY_NOTICES.md`](THIRD_PARTY_NOTICES.md), and [`THIRD_PARTY_LICENSES/`](THIRD_PARTY_LICENSES/) in the pub package.
