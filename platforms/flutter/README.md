# merman Flutter/Dart FFI

Flutter package for the canonical `merman-ffi` C ABI. The public package name is `merman`.

Merman renders Mermaid diagrams without a browser. It can parse Mermaid source, return semantic
JSON, compute layout JSON, and render SVG through a headless Rust engine. See the
[project README](https://github.com/Latias94/merman),
[FFI protocol](https://github.com/Latias94/merman/blob/main/docs/bindings/FFI_PROTOCOL.md), and
[diagram coverage status](https://github.com/Latias94/merman/blob/main/docs/alignment/STATUS.md)
for the main library contract.

The package exposes a small Dart API for SVG, ASCII text, semantic JSON, layout JSON, validation,
and metadata. On Flutter targets, the plugin also carries the native `merman-ffi` library, so
application code normally opens the engine with `Merman.open()` and does not pass a dynamic library
path.

## Supported Flutter Platforms

- Android: bundled `libmerman_ffi.so` slices under `android/src/main/jniLibs`.
- iOS: bundled `MermanFFI.xcframework`, linked as a dynamic framework so Dart FFI can use
  `DynamicLibrary.process()` without shipping the much larger Rust static archive.
- macOS: bundled `Libraries/libmerman_ffi.dylib`, linked by CocoaPods.
- Windows: bundled `merman_ffi.dll`, installed into the Flutter app bundle by CMake.
- Linux: bundled `linux/lib/<arch>/libmerman_ffi.so`, installed into the Flutter app bundle by CMake.

## API

```dart
import 'package:merman/merman.dart';

final merman = Merman.open();
final source = 'flowchart TD\nA[Hello] --> B[World]';
final version = merman.packageVersion;

final svg = merman.renderSvg(
  source,
  optionsJson: '{"svg":{"pipeline":"readable"}}',
);
final semantic = merman.parseJson(source);
final layout = merman.layoutJson(source);
final ascii = merman.renderAscii(source);
final validation = merman.validate(source);
final diagrams = merman.supportedDiagrams();
final themes = merman.themes();

try {
  merman.renderSvg(source, optionsJson: '{');
} on MermanException catch (error) {
  print('${error.codeName}: ${error.message}');
}
```

`optionsJson` follows the shared schema in
[`docs/bindings/OPTIONS_JSON.md`](https://github.com/Latias94/merman/blob/main/docs/bindings/OPTIONS_JSON.md).

## Rendering SVG In Flutter

`Merman.renderSvg` returns SVG text; this package does not prescribe a Flutter widget. For highest
visual fidelity, render the SVG in a browser-capable surface such as `webview_flutter`:

```dart
final svg = Merman.open().renderSvg(source);
final controller = WebViewController()
  ..setJavaScriptMode(JavaScriptMode.unrestricted)
  ..loadHtmlString('''
<!doctype html>
<html>
<body style="margin:0;background:white">
$svg
</body>
</html>
''');
```

Mermaid-like SVG can include `<style>`, `<marker>`, and `<foreignObject>`. Those elements are valid
parts of the output: styles preserve theme behavior, markers draw arrowheads, and foreign objects
carry HTML labels. Native Flutter SVG widgets and some rasterizers may ignore or partially support
those elements, so diagrams can lose arrowheads, labels, or styling outside a WebView.

Do not blindly strip those tags in application code. Instead, select an explicit SVG pipeline for
the target renderer:

```dart
final browserSvg = merman.renderSvg(source); // default parity output
final readableSvg = merman.renderSvg(
  source,
  optionsJson: '{"svg":{"pipeline":"readable"}}',
);
final resvgSafeSvg = merman.renderSvg(
  source,
  optionsJson: '{"svg":{"pipeline":"resvg-safe"}}',
);
```

Use the default parity output for WebView/browser display, `readable` when a renderer needs text
fallbacks for labels, and `resvg-safe` for stricter SVG consumers or raster/PDF export paths.

## Local Dart Smoke

Raw `dart run` does not execute Flutter's platform packaging step, so the smoke example accepts an
explicit native library path for local development:

```bash
cargo build -p merman-ffi
cd platforms/flutter
dart pub get
dart run example/smoke.dart ../../target/debug/libmerman_ffi.dylib
```

Use `../../target/debug/libmerman_ffi.so` on Linux and `../../target/debug/merman_ffi.dll` on
Windows. In Flutter applications, use `Merman.open()` without a path.

## Building Native Artifacts For The Flutter Package

Android slices are built from the shared Android script and copied into the Flutter package:

```bash
python3 platforms/android/build-android.py --targets aarch64-linux-android x86_64-linux-android
mkdir -p platforms/flutter/android/src/main/jniLibs
cp -R platforms/android/src/main/jniLibs/* platforms/flutter/android/src/main/jniLibs/
```

iOS uses a Flutter-specific dynamic framework XCFramework:

```bash
bash platforms/flutter/build-ios.sh
```

Desktop artifacts are built with:

```bash
bash platforms/flutter/build-desktop.sh --host
```

For release packaging, use `--all` on macOS with `cargo-zigbuild` and `zig` installed. This creates
the macOS universal dylib plus Linux x86_64/aarch64 and Windows x86_64 artifacts.

## Packaging Smoke

To verify Android plugin packaging through a temporary Flutter app:

```bash
python3 platforms/flutter/tool/android-smoke.py
```
