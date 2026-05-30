# Flutter/Dart FFI Wrapper

Status: experimental platform wrapper.

`platforms/flutter` provides a Dart FFI wrapper over the canonical `merman-ffi` C ABI. It mirrors the
RaTeX platform-wrapper pattern, but uses the merman byte-buffer ABI instead of RaTeX's display-list
ABI.

## What It Does

- Loads `merman_ffi.dll`, `libmerman_ffi.so`, or the process-linked library depending on platform.
- Checks `merman_abi_version`, `merman_buffer_struct_size`, and `merman_result_struct_size` before
  calling render functions.
- Exposes:
  - `Merman.renderSvg`
  - `Merman.parseJson` / `parseJsonRaw`
  - `Merman.layoutJson` / `layoutJsonRaw`
- Converts non-OK C ABI results into `MermanException`.

## Verify Locally

```bash
cargo build -p merman-ffi
cd platforms/flutter
dart pub get
dart analyze
dart run example/smoke.dart ../../target/debug/merman_ffi.dll
```

Use `../../target/debug/libmerman_ffi.so` on Linux and
`../../target/debug/libmerman_ffi.dylib` on macOS.

## Follow-On Packaging

- Bundle native libraries into a real Flutter plugin for Android/iOS/desktop.
- Add Gradle/CocoaPods/CMake packaging.
- Add CI matrix smoke for Android, iOS/macOS, Windows, and Linux.
