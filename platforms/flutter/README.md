# Merman For Flutter And Dart

[![pub package](https://img.shields.io/pub/v/merman.svg)](https://pub.dev/packages/merman)
[![License: MIT](https://img.shields.io/badge/license-MIT-yellow)](https://github.com/Latias94/merman/blob/main/LICENSE-MIT)
[![License: Apache-2.0](https://img.shields.io/badge/license-Apache--2.0-blue)](https://github.com/Latias94/merman/blob/main/LICENSE-APACHE)

Render and analyze Mermaid diagrams in Flutter without a browser or JavaScript runtime. The plugin wraps Merman's native Rust engine with Dart FFI and bundles the required libraries for supported Flutter platforms.

> **Alpha:** Dart and native APIs may break before the stable release. The package currently targets C ABI version `2`; install the Dart wrapper and bundled native artifacts from the same package release. The source-tree README describes `Unreleased`, while each pub.dev archive preserves the documentation for that published version.

## Install

Use the current 0.8 prerelease line from pub.dev:

```yaml
dependencies:
  merman: ">=0.8.0-alpha.3 <0.9.0"
```

Then fetch packages with `flutter pub get`.

## Render A Diagram

```dart
import 'package:merman/merman.dart';

final engine = Merman.open();
final source = 'flowchart TD\nA[Hello] --> B[World]';
final svg = engine.renderSvg(source);
print(svg.substring(0, 4)); // <svg
```

The same engine exposes SVG and terminal output, semantic and layout maps, validation, diagram/document diagnostics, parser facts, themes, lint metadata, ASCII support grades, and diagram-family capabilities.

`optionsJson` follows the versioned [binding options schema](https://github.com/Latias94/merman/blob/main/docs/bindings/OPTIONS_JSON.md):

```dart
final svg = engine.renderSvg(
  source,
  optionsJson: '{"svg":{"pipeline":"readable"}}',
);
```

## Supported Platforms

| Platform | Packaged native artifact |
| --- | --- |
| Android | `arm64-v8a` and `x86_64` shared libraries |
| iOS | Dynamic `MermanFFI.xcframework` |
| macOS | Universal dylib for CocoaPods and XCFramework for SwiftPM |
| Linux | x86_64 or aarch64 shared library, depending on the release artifact |
| Windows | x86_64 DLL |

Flutter Web is not supported because this package uses `dart:ffi`; use [`@mermanjs/web`](https://www.npmjs.com/package/@mermanjs/web) for browsers. Native artifact availability is release-specific, so verify the package archive for the target architecture before deployment.

## SVG Display

`renderSvg` returns SVG text and does not prescribe a widget. Mermaid-compatible output may contain `<style>`, `<marker>`, and `<foreignObject>` elements. A WebView provides the closest browser behavior; native SVG widgets can omit HTML labels, markers, or CSS features.

Select the output contract deliberately:

```dart
final paritySvg = engine.renderSvg(source);
final readableSvg = engine.renderSvg(
  source,
  optionsJson: '{"svg":{"pipeline":"readable"}}',
);
final strictSvg = engine.renderSvg(
  source,
  optionsJson: '{"svg":{"pipeline":"resvg-safe"}}',
);
```

Use the default for browser/WebView parity, `readable` for text fallbacks, and `resvg-safe` for stricter SVG or raster consumers.

## Reusable Engines And Text Measurement

Use `MermanReusableEngine` for repeated calls with shared options. It owns a native handle and must be closed:

```dart
final reusable = engine.reusableEngine();
try {
  final svg = reusable.renderSvg(source);
} finally {
  reusable.close();
}
```

Merman owns a deterministic vendored text measurer by default. Server-like Dart workloads, tests, and documentation builds should normally keep it. Flutter previews can call `setTextMeasurer` when layout must match the final Flutter or WebView font stack.

ABI 2 exposes 19 exact operations (`0..18`). Construct handled results with
`MermanTextMeasureResult.metrics`, `.length`, `.horizontalExtents`, or
`.wrappedWithRawWidth`; each factory requires and validates the fields used by that result shape.
Return `null` when the current isolate cannot answer an operation synchronously and faithfully.
Wrong-kind results and callback exceptions fall back for that operation.

The callback is isolate-local. Create, measure, render, and close the reusable engine on the same isolate; do not call back into that engine from its measurer. Calling `close()` during a callback defers native disposal until the call returns. See the [host measurement guide](https://github.com/Latias94/merman/blob/main/docs/bindings/HOST_TEXT_MEASUREMENT.md#flutter--dart-ffi) for operation shapes and cache guidance.

## Analysis Contract

`analyzeJson` and `analyzeDocumentJson` return diagnostics schema `1`; `analyzeDocumentFactsJson` returns parser-backed facts schema `2` and rejects facts v1 at its version boundary. These schema versions are independent of native ABI `2`. The removed TextScan alpha facts shape is not retained.

## Runtime Contract

Call `runtimeContract()` to inspect runtime-contract schema `3` from the loaded native library. It
reports the loaded ABI, package, and options versions together with compiled features, registry
facts, and the exact values of every resource profile. Use this contract to choose an explicit
profile for the host workload instead of duplicating limits in Dart or inferring capabilities from
package versions.

Choose a profile from the shared [resource decision table](https://github.com/Latias94/merman/blob/main/docs/bindings/OPTIONS_JSON.md#resource-options), then pass the generated builder output:

```dart
final resourceOptions = MermanResourceOptionsBuilder()
  ..setProfile(MermanResourceProfile.constrained);
final svg = engine.renderSvg(
  source,
  optionsJson: resourceOptions.build().toOptionsJson(),
);
```

Use `constrained` for untrusted, public, or multi-tenant input; `interactive` is for cooperative
local editing. The native CLI's default is intentionally separate (`trusted-native`).

Document analysis accepts the full Markdown/MDX-like source and a URI:

```dart
final facts = engine.analyzeDocumentFactsJson(
  '```mermaid\n$source\n```',
  uri: 'file:///workspace/README.md',
);
```

All native calls are synchronous. Run rendering outside latency-sensitive UI work, and query `diagramFamilyCapabilities()` or `asciiCapabilities()` rather than assuming every output supports every family.

## Local Development

Run the Dart smoke against a locally built native library:

```sh
cargo build -p merman-ffi
cd platforms/flutter
dart pub get
dart run example/smoke.dart ../../target/debug/libmerman_ffi.dylib
```

Use `.so` on Linux and `.dll` on Windows. Flutter applications normally call `Merman.open()` without a path. Native artifact assembly and packaging commands are documented in the [Flutter/Dart FFI guide](https://github.com/Latias94/merman/blob/main/docs/bindings/FLUTTER_DART_FFI.md).

## Documentation And Releases

- [Package changelog](CHANGELOG.md)
- [Flutter/Dart binding guide](https://github.com/Latias94/merman/blob/main/docs/bindings/FLUTTER_DART_FFI.md)
- [Diagram coverage](https://github.com/Latias94/merman/blob/main/docs/alignment/STATUS.md)
- [Issue tracker](https://github.com/Latias94/merman/issues)
- [Source repository](https://github.com/Latias94/merman)

pub.dev is the supported registry channel for this package. CocoaPods and SwiftPM files inside the plugin are Flutter integration details, not a separately published Dart package.

## License And Notices

This package is available under MIT or Apache-2.0. See [`LICENSE`](LICENSE),
[`THIRD_PARTY_NOTICES.md`](THIRD_PARTY_NOTICES.md), and
[`THIRD_PARTY_LICENSES/`](THIRD_PARTY_LICENSES/) in the pub package.
