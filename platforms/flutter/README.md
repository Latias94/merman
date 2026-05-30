# merman Flutter/Dart FFI

Experimental Dart FFI wrapper for the `merman-ffi` C ABI.

This package is the first platform-wrapper layer in the same spirit as the local RaTeX reference:
it loads the native library, checks the ABI version and struct sizes, then exposes a small Dart API
for SVG, semantic JSON, and layout JSON.

## Local Smoke

Build the native library:

```bash
cargo build -p merman-ffi
```

Run the Dart smoke with an explicit native library path:

```bash
cd platforms/flutter
dart pub get
dart run example/smoke.dart ../../target/debug/merman_ffi.dll
```

Use `../../target/debug/libmerman_ffi.so` on Linux and
`../../target/debug/libmerman_ffi.dylib` on macOS.

## API

```dart
import 'package:merman_flutter/merman_flutter.dart';

final merman = Merman.open('path/to/merman_ffi.dll');
final svg = merman.renderSvg('flowchart TD\nA[Hello] --> B[World]');
final semantic = merman.parseJson('flowchart TD\nA[Hello] --> B[World]');
final layout = merman.layoutJson('flowchart TD\nA[Hello] --> B[World]');
```

Packaging native libraries into Android, iOS, macOS, Windows, and Linux app/plugin bundles remains
follow-on work.
