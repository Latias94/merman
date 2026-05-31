# merman Flutter/Dart FFI

Flutter package for the canonical `merman-ffi` C ABI. The public package name is `merman`.

Merman renders Mermaid diagrams without a browser. It can parse Mermaid source, return semantic
JSON, compute layout JSON, and render SVG through a headless Rust engine. See the
[project README](https://github.com/Latias94/merman),
[FFI protocol](https://github.com/Latias94/merman/blob/main/docs/bindings/FFI_PROTOCOL.md), and
[diagram coverage status](https://github.com/Latias94/merman/blob/main/docs/alignment/STATUS.md)
for the main library contract.

The package exposes a small Dart API for SVG, semantic JSON, and layout JSON. On Flutter targets,
the plugin also carries the native `merman-ffi` library, so application code normally opens the
engine with `Merman.open()` and does not pass a dynamic library path.

## Supported Flutter Platforms

- Android: bundled `libmerman_ffi.so` slices under `android/src/main/jniLibs`.
- iOS: bundled `Merman.xcframework`, force-loaded so Dart FFI can use
  `DynamicLibrary.process()`.
- macOS: bundled `Libraries/libmerman_ffi.dylib`, linked by CocoaPods.
- Windows: bundled `merman_ffi.dll`, copied beside the plugin DLL by CMake.
- Linux: bundled `linux/lib/<arch>/libmerman_ffi.so`, copied beside the plugin by CMake.

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

try {
  merman.renderSvg(source, optionsJson: '{');
} on MermanException catch (error) {
  print('${error.codeName}: ${error.message}');
}
```

`optionsJson` follows the shared schema in
[`docs/bindings/OPTIONS_JSON.md`](https://github.com/Latias94/merman/blob/main/docs/bindings/OPTIONS_JSON.md).

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

iOS uses the shared Apple XCFramework script:

```bash
bash scripts/build-apple-xcframework.sh --ios
rm -rf platforms/flutter/ios/Merman.xcframework
cp -R platforms/apple/Merman.xcframework platforms/flutter/ios/Merman.xcframework
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
