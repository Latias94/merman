# Flutter/Dart FFI Wrapper

Status: experimental platform wrapper.

`platforms/flutter` provides a Dart FFI wrapper over the canonical `merman-ffi` C ABI. It uses the
merman byte-buffer ABI and exposes SVG, semantic JSON, and layout JSON as Dart strings/maps.

## What It Does

- Loads `merman_ffi.dll`, `libmerman_ffi.so`, or the process-linked library depending on platform.
- Checks `merman_abi_version`, `merman_buffer_struct_size`, and `merman_result_struct_size` before
  calling render functions.
- Exposes:
  - `Merman.renderSvg`
  - `Merman.parseJson` / `parseJsonRaw`
  - `Merman.layoutJson` / `layoutJsonRaw`
- Converts non-OK C ABI results into `MermanException`.
- Declares an Android Flutter plugin shim that packages generated `libmerman_ffi.so` slices from
  `platforms/android/src/main/jniLibs`.

## Verify Locally

```bash
cargo build -p merman-ffi
cd platforms/flutter
flutter pub get
flutter analyze
dart run example/smoke.dart ../../target/debug/libmerman_ffi.dylib
```

The smoke example source is `platforms/flutter/example/smoke.dart`.

Android packaging smoke:

```bash
python3 platforms/flutter/tool/android-smoke.py
```

Combined platform gate:

```bash
python3 scripts/verify-platform-bindings.py --build-android-slices
python3 scripts/verify-platform-bindings.py --build-android-slices --run-flutter-android-smoke
```

Use `../../target/debug/libmerman_ffi.so` on Linux and `../../target/debug/merman_ffi.dll` on
Windows.

## Follow-On Packaging

- Add iOS/macOS CocoaPods and desktop CMake packaging.
- Add CI matrix smoke for Android, iOS/macOS, Windows, and Linux.
