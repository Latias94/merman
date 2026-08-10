# Merman For Flutter

[![pub package](https://img.shields.io/pub/v/merman.svg)](https://pub.dev/packages/merman) [![License: MIT](https://img.shields.io/badge/license-MIT-yellow)](https://github.com/Latias94/merman/blob/main/LICENSE-MIT) [![License: Apache-2.0](https://img.shields.io/badge/license-Apache--2.0-blue)](https://github.com/Latias94/merman/blob/main/LICENSE-APACHE)

Render and analyze Mermaid diagrams in Flutter without a browser or JavaScript runtime. The plugin is a Dart-friendly facade over Merman's native ABI 3 table and ships the native library for its supported Flutter targets.

> `Merman.open()` accepts only the exact native package version bundled with this Dart release. `Merman.openPath(...)` may load another package version, but both entry points require the current complete ABI 3 table, runtime catalog fields, metadata and payload schemas, and service constructor. Historical partial ABI 3 producers and ABI 2 are not fallback paths.

## Install

Install the current prerelease from pub.dev:

```sh
flutter pub add 'merman:^0.8.0-alpha.6'
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

The public Dart API can load a host-owned current-contract native library with `Merman.openPath(...)`, but this package itself depends on the Flutter SDK and is not distributed as a standalone Dart package. `mermanPackageVersion` is the exact version expected by `Merman.open()`.

## Render A Diagram

`Merman` is a stateless discovery and one-shot facade. It creates and closes a fresh native engine for each operation, so the facade itself has no `close()` method.

```dart
import 'package:merman/merman.dart';

const source = 'flowchart TD\nA[Hello] --> B[World]';
final merman = Merman.open();
final svg = merman.renderSvg(source);
print(svg.substring(0, 4)); // <svg
```

The generic API is available when an application needs to select an output at runtime. It returns a structured `MermanOperationResult` with the selected operation, media type, copied Dart bytes, and typed `MermanOperationMetadata`:

```dart
final output = merman.execute(MermanOperation.semanticJson, source);
final semantic = output.jsonObject;
print(output.metadata.runtimePolicy);
print(output.metadata.rawJson); // Includes additive fields from newer producers.
```

Convenience methods are projections over `execute` and cover all 13 generated operations, including `analysisFactsJson` and `svgPlanJson`. `renderPng`, `renderJpeg`, and `renderPdf` retain their simple byte-returning forms; the matching `renderPngResult`, `renderJpegResult`, and `renderPdfResult` methods expose metadata and effective resource-limited output plans. Known raster and PDF plans have typed classes, while a future plan kind becomes `MermanUnknownOutputPlan` with preserved JSON.

A native artifact can intentionally omit some outputs. Inspect `merman.runtimeCatalog` before enabling optional UI or export paths; an unavailable operation raises `MermanUnsupportedOperationException` rather than silently falling back. `MermanUnknownOperationException` identifies an ID outside the generated ABI vocabulary; `MermanMissingCapabilityException.capabilityId` identifies the backend absent from a valid native request.

## Inspect Native Metadata

The typed metadata APIs expose the loaded artifact's diagram, ASCII, parser/render, lint, Mermaid theme, and presentation catalogs. Results are copied into Dart-owned immutable values and cached on the `Merman` instance. Decoders require the documented fields while tolerating additive JSON fields from a compatible newer producer. Presentation IDs remain open strings so compatible producers can add presets, profiles, and aspects without requiring a Dart enum update.

```dart
final diagrams = merman.supportedDiagrams();
final ascii = merman.asciiCapabilities();
final families = merman.diagramFamilyCapabilities();
final lintRules = merman.lintRuleCatalog();
final themes = merman.supportedThemes();
final presentation = merman.presentationCatalog();
final rawCatalog = merman.metadataJson('supported-diagrams');
```

All six methods use the ABI 3 `metadata_collect` table slot. Discovery requires the current complete table before any engine or metadata API is exposed, including for `Merman.openPath(...)`.

## Options And Resource Limits

One-shot `Merman` methods accept the complete options document for that operation. `MermanEngine` accepts constructor options as a reusable baseline; method-level `optionsJson` values deeply override that baseline for one call while unspecified nested values remain inherited. Select `runtime_policy` only in the constructor options.

```dart
final engine = MermanEngine(
  optionsJson: '''
    {
      "version": 2,
      "resources": {"profile": "constrained"},
      "svg": {"pipeline": "resvg-safe"}
    }
  ''',
);
try {
  final svg = engine.renderSvg(source);
} finally {
  engine.close();
}
```

Omitting `runtime_policy` keeps the engine deterministic even when the packaged library is built with the atomic `native-runtime` feature. Set `"runtime_policy":"native"` only when the operation should consult the host clock, time-zone rules, and random source. Runtime discovery still reports the concrete `system-clock`, `system-timezone`, and `system-random` adapter IDs. A custom artifact without `native-runtime` raises `MermanUnsupportedOperationException` when native policy is requested.

Use `constrained` for untrusted or multi-tenant input, `interactive` for cooperative local editing, and `trusted-native` for local native automation. `MermanResourceOptionsBuilder` produces the shared resource-options fragment. Its profile is optional: omit it for a reusable request that must inherit its constructor ceiling, and use `MermanResourceOverrideId` for limits. The runtime catalog is the source of truth for compiled capabilities, accepted option groups, constructor services, service limits, and enforced operation limits. See the [options contract](https://github.com/Latias94/merman/blob/main/docs/bindings/OPTIONS_JSON.md) for the complete schema.

`MermanResourceLimitId` describes every catalog limit; `MermanResourceOverrideId` is the narrower option-construction ID set. Inspect `runtimeCatalog.resourceLimits` and `runtimeCatalog.resourceProfiles` for the loaded ABI 3 library's complete phase, description, minimum, hard-cap, applicable operation IDs, purpose, trust, recommendation, and nullable budget metadata. Those typed collections preserve additive IDs reported by a host-owned library; a `null` profile limit means unbounded, while hard caps are always finite.

Every current resource descriptor reports `minimum_value`; omitting it is a runtime-catalog contract error.

## Reusable Engines And Constructor Services

Use `MermanEngine` when options, icon packs, or host text measurement should be reused. `MermanEngineServices` is immutable constructor state; there are no post-construction mutators or callback-specialized constructors.

```dart
final engine = MermanEngine(
  services: MermanEngineServices(
    textMeasurer: (request) {
      if (request.operation == MermanTextMeasurementOperation.measure) {
        return MermanTextMeasureResult.metrics(
          width: 42,
          height: 18,
          lineCount: 1,
        );
      }
      return null; // Use the deterministic fallback for this operation.
    },
  ),
);
try {
  final svg = engine.renderSvg(source);
} finally {
  engine.close();
}
```

The callback is isolate-local and synchronous. Create, render with, and close the measured engine on the same Dart isolate. Do not call back into that engine from the measurer. Precompute or cache WebView, platform-channel, and font results instead of blocking inside the callback. The [host measurement guide](https://github.com/Latias94/merman/blob/main/docs/bindings/HOST_TEXT_MEASUREMENT.md#flutter--dart-ffi) describes result shapes and cache keys.

`MermanIconPack` accepts one in-memory IconifyJSON collection and an optional registration-name override. `MermanIconPackSet.fromPacks` enforces the fixed transport byte/count limits and snapshots packs into immutable UTF-8 buffers, so the source strings need not be retained. Flutter's C ABI has no separate native registry handle: those buffers are borrowed only during each `MermanEngine` constructor call, and the engine owns the parsed registry after construction returns. Native semantic validation is transactional at engine construction; a failure publishes no engine and exposes `MermanException.iconRegistryDetails` when available.

```dart
final iconPackSet = MermanIconPackSet.fromPacks([
  MermanIconPack(
    json: iconifyJson,
    registrationName: 'product',
  ),
]);
final engine = MermanEngine(
  services: MermanEngineServices(iconPackSet: iconPackSet),
);
try {
  final svg = engine.renderSvg(source);
} finally {
  engine.close();
}
```

Merman does not read files, fetch URLs, or trim icon collections. Hosts must acquire packs and pre-trim collections that exceed the fixed limits reported by `runtimeCatalog.constructorServiceContracts`. Icon bodies are XML-scoped and sanitized immediately before embedding under the effective render configuration, but SVG remains active document content: use a trusted WebView policy or an appropriate sanitizer/content-security boundary when displaying untrusted diagrams.

`close()` never waits. `MermanBusyException` means another operation is active, while `MermanReentrantCallException` means the same engine is inside its callback. Both failures retain the complete native engine and service graph so close can be retried. Only a confirmed native close releases the Dart callback registration; later closes are idempotent. Always close callback-owning engines explicitly because a callback may capture its engine and form a cycle.

Outputs are returned as owned Dart byte arrays. Internally, each written native result is released exactly once through its opaque ABI allocation token after the bytes and metadata are copied. The sole zero-token terminal case is native allocation-token exhaustion: `INTERNAL_ERROR` with the result otherwise untouched in its caller-initialized state. This keeps native pointers and allocators out of the public API.

## SVG Display

`renderSvg` returns SVG text; it intentionally does not choose a Flutter widget. Mermaid-compatible SVG can contain `<style>`, `<marker>`, and `<foreignObject>`. A WebView gives the closest browser behavior. Native SVG widgets and rasterizers can omit HTML labels, markers, or CSS features.

Choose the SVG pipeline for the final consumer:

- `parity` is the default Mermaid-compatible output for browser/WebView use.
- `readable` favors text fallbacks and inspectability.
- `resvg-safe` is the explicit choice for stricter SVG consumers, rasterizers, and PDF conversion.

Set the usual reusable pipeline in `MermanEngine(optionsJson: ...)`. Override it for one call with `renderSvg(source, optionsJson: ...)`, or pass the complete options document directly to a one-shot `Merman.renderSvg` call.

## Migrating From The Previous Prerelease API

- Replace the engine-owning `Merman` usage with stateless `Merman` for one-shot calls or direct `MermanEngine(...)` construction for reuse.
- Delete `MermanReusableEngine`, `Merman.reusableEngine(...)`, `dispose()`, and constructor-level `textMeasurer:` arguments. Use `MermanEngineServices` and `close()`.
- Replace map access such as `result.metadata['runtime_policy']` with typed fields such as `result.metadata.runtimePolicy`; use `rawJson` for additive fields.
- Use `renderPngResult`, `renderJpegResult`, or `renderPdfResult` when effective output planning matters; byte-returning methods remain available.
- Treat runtime operation, resource-limit, option-group, and service IDs as discovered open vocabularies. Unknown runtime operations remain discoverable but require an updated generated SDK before numeric invocation.

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

The checked-in low-level binding is generated with `ffigen` from the public C header. It validates
the size-tagged ABI 3 table and minimum-prefix digest, while release builds require the current
matching header/table contract. Application code uses `merman.dart` and never needs ABI pointers
or record definitions.

```sh
cargo build -p merman-ffi --no-default-features --features svg,analysis,ascii,png,jpeg,pdf,layout-cytoscape,layout-elk,math,native-runtime
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
