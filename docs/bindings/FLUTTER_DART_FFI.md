# Flutter/Dart FFI Wrapper

Status: experimental publishable Flutter package.

`platforms/flutter` provides the `merman` Dart/Flutter package over the canonical `merman-ffi`
C ABI. The Dart wrapper uses the byte-buffer ABI and exposes SVG, ASCII text, semantic JSON,
layout JSON, validation, and metadata as Dart strings/maps/lists.

Merman itself is a browserless Rust engine for Mermaid diagrams. Start from the
[project README](https://github.com/Latias94/merman) for product scope, the
[FFI protocol](https://github.com/Latias94/merman/blob/main/docs/bindings/FFI_PROTOCOL.md) for ABI
details, and
[diagram coverage status](https://github.com/Latias94/merman/blob/main/docs/alignment/STATUS.md)
for current Mermaid parity.

## What It Does

- Opens the bundled native library for each Flutter platform:
  - Android: `libmerman_ffi.so`
  - iOS/macOS: process-linked symbols via `DynamicLibrary.process()`
  - Windows: `merman_ffi.dll`
  - Linux: `libmerman_ffi.so`
- Checks `merman_abi_version`, `merman_buffer_struct_size`, and `merman_result_struct_size` before
  calling render functions.
- Exposes:
  - `Merman.renderSvg`
  - `Merman.renderAscii`
  - `Merman.parseJson` / `parseJsonRaw`
  - `Merman.layoutJson` / `layoutJsonRaw`
  - `Merman.validate` / `validateJsonRaw`
  - `Merman.supportedDiagrams`
  - `Merman.asciiCapabilities`
  - `Merman.diagramFamilyCapabilities`
  - `Merman.lintRuleCatalog`
  - `Merman.supportedThemes`
  - `Merman.supportedHostThemePresets`
- Converts non-OK C ABI results into `MermanException`.
- Provides `Merman.openPath(path)` only for local Dart CLI smoke tests and other development
  diagnostics where Flutter platform packaging is not running.
- Exposes `MermanReusableEngine` and `MermanTextMeasurer` for repeated calls and host text
  measurement through the C reusable-engine API.

`Merman.lintRuleCatalog()` returns typed rule metadata with `id`, `description`,
`evidence`, `defaultSeverity`, `category`, `defaultEnabled`, `defaultProfile`, `origin`,
`configurable`, and `fixable`. Hosts should use this catalog to build rule pickers or LSP
configuration UI instead of shipping their own hard-coded rule table.

## Platform Packaging

- Android copies generated native slices into `platforms/flutter/android/src/main/jniLibs`.
- iOS publishes `platforms/flutter/ios/MermanFFI.xcframework` as a dynamic framework so Dart FFI can
  resolve C symbols through `DynamicLibrary.process()` without bundling the larger static archive.
- macOS publishes `platforms/flutter/macos/Libraries/libmerman_ffi.dylib`.
- Windows publishes `platforms/flutter/windows/merman_ffi.dll`.
- Linux publishes `platforms/flutter/linux/lib/x86_64/libmerman_ffi.so` and
  `platforms/flutter/linux/lib/aarch64/libmerman_ffi.so`.

Generated native artifacts are ignored by git and re-included for pub packages through
`platforms/flutter/.pubignore`.

## SVG Rendering Guidance

The Flutter wrapper returns SVG strings. It intentionally does not wrap them in a UI component,
because the right display surface depends on the host app's fidelity and portability needs.

For browser-like visual fidelity, use a WebView or another renderer that supports Mermaid-style SVG.
Mermaid-like output can contain `<style>`, `<marker>`, and `<foreignObject>`:

- `<style>` carries theme and class styling.
- `<marker>` draws arrowheads.
- `<foreignObject>` carries HTML labels.

Native Flutter SVG renderers and some rasterizers may ignore or partially support those elements.
For example, a renderer that ignores `<marker>` can drop arrowheads, and a renderer that ignores
`<foreignObject>` can lose label content.

Application code should not blindly remove these tags. Choose an explicit SVG pipeline through
`optionsJson` instead:

```dart
final paritySvg = merman.renderSvg(source);
final readableSvg = merman.renderSvg(
  source,
  optionsJson: '{"svg":{"pipeline":"readable"}}',
);
final resvgSafeSvg = merman.renderSvg(
  source,
  optionsJson: '{"svg":{"pipeline":"resvg-safe"}}',
);
```

- Use the default `parity` output for WebView/browser display and Mermaid-like comparison.
- Use `readable` when the target renderer needs best-effort label text fallbacks.
- Use `resvg-safe` for stricter SVG consumers and raster/PDF export flows.

## Text Measurement Guidance

Use `MermanReusableEngine.setTextMeasurer(...)` when Flutter needs label geometry to match its final
preview surface. The current wrapper uses `NativeCallable.isolateLocal`, so create the reusable
engine, install the measurer, render, and close the engine on the same Dart isolate.

For WebView display, measure with a DOM/canvas service from that WebView after fonts are loaded and
feed cached values into the synchronous measurer. For Flutter-native display, measure with the same
paragraph/text layout stack and font registration used by the preview. Return `null` for
unsupported requests so merman can fall back per request. Measure natural HTML-like label width
before constraining to `maxWidth`; otherwise short labels can be overestimated and make the diagram
wider than the final Flutter/WebView surface. See
[`HOST_TEXT_MEASUREMENT.md`](HOST_TEXT_MEASUREMENT.md#flutter--dart-ffi) for the full platform
checklist.

## Verify Locally

```bash
cargo build -p merman-ffi
cd platforms/flutter
flutter pub get
flutter analyze
dart run example/smoke.dart ../../target/debug/libmerman_ffi.dylib
```

Use `../../target/debug/libmerman_ffi.so` on Linux and `../../target/debug/merman_ffi.dll` on
Windows.

Android packaging smoke:

```bash
python3 platforms/flutter/tool/android-smoke.py
```

Combined platform gate:

```bash
python3 scripts/verify-platform-bindings.py --build-android-slices
python3 scripts/verify-platform-bindings.py --build-android-slices --run-flutter-android-smoke
```
