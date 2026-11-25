# Merman For Flutter

[![pub package](https://img.shields.io/pub/v/merman.svg)](https://pub.dev/packages/merman) [![License: MIT](https://img.shields.io/badge/license-MIT-yellow)](https://github.com/Latias94/merman/blob/main/LICENSE-MIT) [![License: Apache-2.0](https://img.shields.io/badge/license-Apache--2.0-blue)](https://github.com/Latias94/merman/blob/main/LICENSE-APACHE)

Parse, analyze, lay out, and render Mermaid diagrams in Flutter or standalone Dart without a browser or JavaScript runtime. The package uses Merman's size-tagged native function table and Native Assets to bundle libraries for supported targets.

The installation commands below pin `0.11.0`. The Theme v2 examples target the current source checkout. Check the [host installation guide](https://github.com/Latias94/merman/blob/main/docs/release/PREBUILT_PRODUCTS.md#published-versions) for verified versions and channel availability. Use the [stable upgrade guide](https://github.com/Latias94/merman/blob/main/docs/release/V093_TO_V0100_UPGRADE_GUIDE.md) when migrating from `0.9.3` or a `0.10.0` alpha.

> The current source requires the complete native ABI 4 table and Options JSON schema `4`. Use a native library built from the same source to run the Theme v2 examples. `Merman.open()` requires the exact native package version recorded by the Dart package. `Merman.openPath(...)` accepts a different package version when it provides the complete matching ABI and runtime schemas.

Version `0.11.0` uses native ABI `3`, Options JSON schema `3`, facts schema `3`, and ASCII output schema `4`. In `MermanAsciiOutputPlan`, read nullable `primaryOverflowed` for the primary layout and `emittedOverflowed` for the output text. Upgrade the Dart facade, manifest, and native assets together, and update saved options and JSON consumers as described in the [stable upgrade guide](https://github.com/Latias94/merman/blob/main/docs/release/V093_TO_V0100_UPGRADE_GUIDE.md).

## Install

Pin the exact package version when pairing the Dart facade with its Native Assets libraries:

```sh
flutter pub add 'merman:0.11.0'
```

For source development, use the matching `0.11.0` source revision and a local path dependency:

```yaml
dependencies:
  merman:
    path: /path/to/merman/platforms/flutter
```

Run `flutter pub get` after adding the dependency.

For the previous stable `0.9.3` installation, use the [tagged guide](https://github.com/Latias94/merman/blob/v0.9.3/platforms/flutter/README.md).
That release uses Options JSON schema `2` and math-free native libraries; keep its facade and
artifacts together rather than using them with the theme API documented here.

## Requirements

- Dart 3.10 or newer; Flutter apps require Flutter 3.47.7 or newer
- Android API 24 or newer
- iOS 13 or newer
- macOS 11 or newer

The Flutter floor follows the current native-hooks dependency graph and its SDK-pinned
`meta` compatibility. This package also supports native Dart without the Flutter SDK.

This is a `package_ffi`-style Dart package and does not depend on the Flutter SDK. Its build hook selects and bundles the native library for Flutter and Dart consumers. The public API can instead load a host-owned current-contract library with `Merman.openPath(...)`; `mermanPackageVersion` is the exact version expected by `Merman.open()`.

<a id="target-native-assets-unreleased"></a>

## Target Native Assets

The package ships a thin Dart facade. During a build, the hook downloads only the selected
OS, CPU and Apple SDK asset. `hook/native-assets.json` is generated from frozen release libraries
and included in that facade; it pins the package version, source commit/tree, compressed bytes
and library SHA-256 digests. Downloads and existing cache files are verified before bundling.
There is no runtime download or automatic Cargo build.

The `0.10.0-alpha.2` native assets belong to the workspace Merman Release `v0.10.0-alpha.2`, under `https://github.com/Latias94/merman/releases/download/v0.10.0-alpha.2/`, with filenames such as `merman-flutter-native-0.10.0-alpha.2-macos-arm64.gz`. Keep the facade, manifest, and target assets from the same release together. Check the [host installation guide](https://github.com/Latias94/merman/blob/main/docs/release/PREBUILT_PRODUCTS.md#published-versions) for verified download availability.

The `0.10.0-alpha.1` package retains its original download base `https://github.com/Latias94/merman/releases/download/flutter-v0.10.0-alpha.1/`. The hook always follows the manifest packaged with the installed version; historical packages keep their original URLs.

The cache lives in the hooks runner's `outputDirectoryShared` under `.dart_tool`. Preserve that
directory between online and offline builds of the same app and package version. Configure
acquisition in the **consuming app's** `pubspec.yaml`:

```yaml
hooks:
  user_defines:
    merman:
      offline: true
```

A cold offline cache fails with preparation instructions. To build without a network or a warm
cache, explicitly supply a prepared native directory:

```yaml
hooks:
  user_defines:
    merman:
      native_assets: vendor/merman-native
      offline: true
```

The path is relative to the pubspec that declares it and contains the selected target subtree,
for example `macos/arm64/libmerman_ffi.dylib` or `android/arm64-v8a/libmerman_ffi.so`.
This override participates in the hook's bundling and dependency tracking. It deliberately
selects host-owned bytes; package/ABI checks still run when the library is opened.
`Merman.openPath(...)` alone does not override build-hook acquisition.

An HTTPS mirror can host the same versioned asset filenames:

```yaml
hooks:
  user_defines:
    merman:
      mirror: https://artifacts.example.org/merman/v0.11.0/
```

The mirror changes the download location only; package-pinned digests remain mandatory.
HTTP is allowed solely for loopback qualification fixtures. Corrupt downloaded or cached bytes
fail visibly; remove the named invalid cache file before retrying. A source checkout without a
manifest can use explicitly built `native/` libraries as described in [Local Development](#local-development).

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

Convenience methods use `execute` to expose operations such as `analysisFactsJson`, `svgPlanJson`, and Theme v2 authoring. `renderPng`, `renderJpeg`, and `renderPdf` retain their simple byte-returning forms; the matching `renderPngResult`, `renderJpegResult`, and `renderPdfResult` methods expose metadata and effective resource-limited output plans. Known raster and PDF plans have typed classes, while a future plan kind becomes `MermanUnknownOutputPlan` with preserved JSON.

The target assets provide SVG including math, semantic and layout JSON, Cytoscape and ELK layout,
ASCII, analysis, validation, and document analysis. PNG, JPEG, PDF and native runtime adapters
remain absent. The corresponding Dart methods remain part of the generated ABI facade
for a current-contract custom library loaded with `Merman.openPath(...)` or
`Merman.fromDynamicLibrary(...)`; unavailable exporters raise `MermanMissingCapabilityException`
with capability `png`, `jpeg`, or `pdf`.

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

The typed metadata APIs expose the loaded artifact's diagram, ASCII, parser/render, lint, and theme catalogs. Decoded values are immutable and cached on the `Merman` instance. The `jsonObject` fields preserve additive metadata.

```dart
final diagrams = merman.supportedDiagrams();
final ascii = merman.asciiCapabilities();
final families = merman.diagramFamilyCapabilities();
final lintRules = merman.lintRuleCatalog();
final themes = merman.supportedThemes();
final themeCatalog = merman.themeCatalog();
final rawCatalog = merman.metadataJson('theme-catalog');
```

These methods use the `metadata_collect` table slot. Both `Merman.open()` and `Merman.openPath(...)` validate the native table before exposing metadata or engine operations.

## Diagram Themes

`MermanThemeCatalog` decodes schema `2`. Each preset has an `id`, `name`, `description`, `category`, `appearances`, `defaultAppearance`, and six `swatches` for its default appearance. `mermaidThemes` lists the classic Mermaid IDs, and `families` contains the native theme capability bitmask for each diagram family. For classic Mermaid styling, set `site_config.theme`.

Set `theme` to a preset ID, a selector with `preset` or `document`, or an inline Theme v2 document. A selector can also set `appearance` (`light` or `dark`) and `precedence` (`host` or `source`). Resolve the app's appearance preference in Dart before passing it to Merman. For a reusable engine, omit `theme` to inherit the configured selection or use `"none"` to clear it for one request.

```dart
import 'dart:convert';

final exported = merman.execute(MermanOperation.exportThemePreset, 'studio');
final themeJson = exported.utf8Text;
final checked = merman.execute(
  MermanOperation.resolveTheme,
  jsonEncode({'document': themeJson, 'appearance': 'dark'}),
).jsonObject;
if (checked['ok'] != true) {
  throw FormatException(checked['message'] as String);
}
final rendered = merman.execute(
  MermanOperation.svg,
  'flowchart LR\nA[Review] --> B[Publish]',
  optionsJson: jsonEncode({
    'theme': {'document': exported.jsonObject, 'appearance': 'dark'},
    'background': 'transparent',
  }),
);
print(rendered.metadata.themeOutcome);
```

`exportThemePreset` takes the raw preset ID and returns editable JSON. `resolveTheme` accepts the document as a JSON object or original JSON text, and supports `auto_fix_contrast` in the request envelope. Send the original text to retain document-relative `line`, `column`, and `path` in errors. SVG metadata includes the render's `theme_outcome`, exposed as `themeOutcome` in Dart.

`background` overrides the theme canvas with a color or `"transparent"`. On custom artifacts with PNG, JPEG, or PDF export, use `export_matte` for an opaque export backing color. Native themes supply palettes for truecolor and 256-color ASCII. Set `ascii.theme` to provide an explicit terminal palette. [Theme authoring smoke](https://github.com/Latias94/merman/blob/main/platforms/flutter/tool/theme_authoring_smoke.dart) · [Theme v2 schema](https://github.com/Latias94/merman/blob/main/schemas/theme-v2.schema.json)

## Options And Resource Limits

One-shot `Merman` methods accept the complete schema-3 options document for that operation. `MermanEngine` accepts constructor options as a reusable baseline; method-level `optionsJson` values deeply override that baseline for one call while unspecified nested values remain inherited. Select `runtime_policy` only in the constructor options.

```dart
final engine = MermanEngine(
  optionsJson: '''
    {
      "version": 4,
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

The default interactive profile admits SVG math. Explicit `constrained` policy rejects the
native math backend with `MermanMissingCapabilityException` and capability `math`; compiled
math does not remove the need for host execution and memory limits.

`MermanResourceLimitId` describes every catalog limit; `MermanResourceOverrideId` is the narrower option-construction ID set. Inspect `runtimeCatalog.resourceLimits` and `runtimeCatalog.resourceProfiles` for the loaded library's complete phase, description, minimum, hard-cap, applicable operation IDs, purpose, trust, recommendation, and nullable budget metadata. Those typed collections preserve additive IDs reported by a host-owned library; a `null` profile limit means unbounded, while hard caps are always finite.

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

<a id="migrating-from-the-previous-prerelease-api"></a>

## Migrating From Older Prerelease APIs

These steps apply to older engine-owning prerelease APIs. Version `0.10.0-alpha.1` already uses the stateless facade and reusable-engine split; its alpha.2 migration requires matching assets and the JSON payload updates described above.

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
the size-tagged ABI 4 table and minimum-prefix digest, while release builds require the current
matching header/table contract. Application code uses `merman.dart` and never needs ABI pointers
or record definitions.

```sh
python3 platforms/flutter/build-native.py host
cd platforms/flutter
flutter pub get
dart run ffigen --config ffigen.yaml
flutter analyze
dart run tool/native_abi_contract_test.dart
dart run tool/native_assets_contract_test.dart
dart run example/main.dart
dart run example/smoke.dart
dart run tool/theme_authoring_smoke.dart
```

CI regenerates the binding, rejects a stale checked-in result, runs analyzer and the ABI contract, then exercises the default Native Assets entry point against a real host library. `build-native.py all-desktop` assembles the complete Apple, Linux, and Windows release matrix on macOS; Android uses `platforms/android/build-android.py --artifact-profile flutter-android-native`. Native packaging is documented in the [Flutter/Dart FFI guide](https://github.com/Latias94/merman/blob/main/docs/bindings/FLUTTER_DART_FFI.md).

Flutter release CI also qualifies the exact frozen thin package with
`tool/qualify_packaged_app.py`. It creates a Flutter macOS app outside the checkout, fetches only
the executing host's target through a local HTTP mirror, builds a release `.app`, removes the
temporary package and hook cache, and runs the relocated app with the server stopped. That app renders SVG math
and verifies that the three binary exporters remain unavailable. The uploaded qualification
receipt records the executed host; it does not establish execution on every target.

The repository-only theme authoring smoke compares light/dark materialization, full support
discovery responses, and structured authoring/resource errors against shared cross-transport
fixtures. It runs through the bundled Native Assets library using both one-shot and reusable
engines in platform verification and Flutter release/preflight CI. It does not qualify final
application bundles or visual output.

## Documentation And Releases

- [Package changelog](CHANGELOG.md)
- [Flutter/Dart binding guide](https://github.com/Latias94/merman/blob/main/docs/bindings/FLUTTER_DART_FFI.md)
- [Diagram coverage](https://github.com/Latias94/merman/blob/main/docs/alignment/STATUS.md)
- [Issue tracker](https://github.com/Latias94/merman/issues)
- [Source repository](https://github.com/Latias94/merman)

pub.dev is the supported registry channel. Native Assets is the only package-owned Flutter integration path.

## License And Notices

This package is available under MIT or Apache-2.0. See [`LICENSE`](LICENSE), [`THIRD_PARTY_NOTICES.md`](THIRD_PARTY_NOTICES.md), and [`THIRD_PARTY_LICENSES/`](THIRD_PARTY_LICENSES/) in the pub package.
