# Merman For Flutter

[![pub package](https://img.shields.io/pub/v/merman.svg)](https://pub.dev/packages/merman) [![License: MIT](https://img.shields.io/badge/license-MIT-yellow)](https://github.com/Latias94/merman/blob/main/LICENSE-MIT) [![License: Apache-2.0](https://img.shields.io/badge/license-Apache--2.0-blue)](https://github.com/Latias94/merman/blob/main/LICENSE-APACHE)

Parse, analyze, lay out, and render Mermaid diagrams in Flutter or standalone Dart without a browser or JavaScript runtime. The package is a Dart-friendly facade over Merman's native ABI 3 table and uses Native Assets to bundle size-oriented native libraries for supported targets.

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

- Dart 3.10 or newer; Flutter apps require Flutter 3.38 or newer
- Android API 24 or newer
- iOS 13 or newer
- macOS 11 or newer

This is a `package_ffi`-style Dart package and does not depend on the Flutter SDK. Its build hook selects and bundles the packaged native library for Flutter and Dart consumers. The public API can instead load a host-owned current-contract library with `Merman.openPath(...)`; `mermanPackageVersion` is the exact version expected by `Merman.open()`.

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

Convenience methods are projections over `execute` and cover all 13 generated ABI operations, including `analysisFactsJson` and `svgPlanJson`. `renderPng`, `renderJpeg`, and `renderPdf` retain their simple byte-returning forms; the matching `renderPngResult`, `renderJpegResult`, and `renderPdfResult` methods expose metadata and effective resource-limited output plans. Known raster and PDF plans have typed classes, while a future plan kind becomes `MermanUnknownOutputPlan` with preserved JSON.

The libraries bundled on pub.dev provide SVG, semantic and layout JSON, both native layout engines, ASCII, analysis, validation, and document analysis. They intentionally omit math, PNG, JPEG, PDF, and native runtime adapters to keep the five-platform package small. The corresponding Dart methods remain part of the generated ABI facade for a current-contract custom library loaded with `Merman.openPath(...)` or `Merman.fromDynamicLibrary(...)`; against the bundled library, unavailable outputs raise `MermanMissingCapabilityException` with capability `math`, `png`, `jpeg`, or `pdf` as appropriate.

A native artifact can intentionally omit some outputs. Inspect `merman.runtimeCatalog` before enabling optional UI or export paths; an unavailable operation raises `MermanUnsupportedOperationException` rather than silently falling back. `MermanUnknownOperationException` identifies an ID outside the generated ABI vocabulary; `MermanMissingCapabilityException.capabilityId` identifies the backend absent from a valid native request.

## Deadlines And Cancellation

Create a reusable `MermanOperationControl` from the same `Merman` or `MermanEngine` instance and
attach it to `execute`. Resource and cancellation failures remain structured errors and never
return a partial document.

```dart
final engine = MermanEngine();
final control = engine.createOperationControl(
  timeout: const Duration(milliseconds: 250),
);
try {
  final result = engine.execute(
    MermanOperation.svg,
    source,
    control: control,
  );
  print(result.utf8Text);
} on MermanCancelledException catch (error) {
  print(error.cancellationDetails?.reason);
} finally {
  control.release();
  engine.close();
}
```

Execution is synchronous and the control is isolate-local. A `Timer` or message scheduled on the
same isolate cannot call `cancel()` while `execute` is blocking that isolate. The Dart facade
directly supports relative deadlines, cancellation before execution, and cancellation requested
from a synchronous host callback. Message-driven mid-render cancellation needs a host-owned native
or worker bridge that can call the native control concurrently; use a process boundary when
forceful termination is also required.

Parser and ASCII renderer failures may also expose `MermanException.diagnosticDetails`. Prefer its
stable code, optional byte span, field, and diagram type over parsing the human-facing message:

```dart
try {
  merman.renderAscii(source);
} on MermanException catch (error) {
  print(error.diagnosticDetails?.code);
  print(error.diagnosticDetails?.span?.start);
}
```

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

The packaged library is deterministic and does not include native clock, time-zone, or random adapters. A custom artifact may enable the atomic `native-runtime` feature and then select `"runtime_policy":"native"`. The bundled artifact raises `MermanUnsupportedOperationException` when native policy is requested, and runtime discovery reports concrete adapter IDs only when they are present.

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
- Use `createOperationControl(timeout: ...)` and pass the control to `execute` for cooperative
  deadlines. Do not rely on a same-isolate `Timer` to interrupt a synchronous call.
- Treat runtime operation, resource-limit, option-group, and service IDs as discovered open vocabularies. Unknown runtime operations remain discoverable but require an updated generated SDK before numeric invocation.

## Supported Platforms

| Platform | Packaged native artifact |
| --- | --- |
| Android | `armeabi-v7a`, `arm64-v8a`, and `x86_64` shared libraries |
| iOS | arm64 device plus arm64/x86_64 simulator dylibs |
| macOS | arm64 and x86_64 dylibs |
| Linux | arm64 and x86_64 shared libraries |
| Windows | x86_64 DLL |

Flutter Web is not supported because this package uses `dart:ffi`; use [`@mermanjs/web`](https://www.npmjs.com/package/@mermanjs/web) in browsers. Verify the release archive before deployment because native target availability is release-specific.

The build hook owns platform selection and Flutter owns final app bundling, framework assembly, install-name rewriting, and signing. There are no Flutter plugin registrars, CocoaPods podspecs, Swift packages, Gradle plugin modules, or CMake plugin wrappers. Android package slices use the `flutter-android-native` C ABI recipe from `merman-ffi`; the Kotlin AAR's JNI transport remains separate in `merman-android-jni`.

## Local Development

The checked-in low-level binding is generated with `ffigen` from the public C header. It validates
the size-tagged ABI 3 table and minimum-prefix digest, while release builds require the current
matching header/table contract. Application code uses `merman.dart` and never needs ABI pointers
or record definitions.

```sh
python3 platforms/flutter/build-native.py host
cd platforms/flutter
flutter pub get
dart run ffigen --config ffigen.yaml
flutter analyze
dart run tool/abi3_contract_test.dart
dart run example/main.dart
dart run example/smoke.dart
```

CI regenerates the binding, rejects a stale checked-in result, runs analyzer and the ABI contract, then exercises the default Native Assets entry point against a real host library. `build-native.py all-desktop` assembles the complete Apple, Linux, and Windows release matrix on macOS; Android uses `platforms/android/build-android.py --artifact-profile flutter-android-native`. Native packaging is documented in the [Flutter/Dart FFI guide](https://github.com/Latias94/merman/blob/main/docs/bindings/FLUTTER_DART_FFI.md).

## Documentation And Releases

- [Package changelog](CHANGELOG.md)
- [Flutter/Dart binding guide](https://github.com/Latias94/merman/blob/main/docs/bindings/FLUTTER_DART_FFI.md)
- [Diagram coverage](https://github.com/Latias94/merman/blob/main/docs/alignment/STATUS.md)
- [Issue tracker](https://github.com/Latias94/merman/issues)
- [Source repository](https://github.com/Latias94/merman)

pub.dev is the supported registry channel. Native Assets is the only package-owned Flutter integration path.

## License And Notices

This package is available under MIT or Apache-2.0. See [`LICENSE`](LICENSE), [`THIRD_PARTY_NOTICES.md`](THIRD_PARTY_NOTICES.md), and [`THIRD_PARTY_LICENSES/`](THIRD_PARTY_LICENSES/) in the pub package.
