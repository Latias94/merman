# Flutter/Dart FFI Binding

Status: experimental, publishable Flutter package.

`platforms/flutter` provides the `merman` Dart/Flutter package over the
canonical Merman native ABI 3. It is a Dart facade, not a second operation
protocol: the package discovers one size-tagged native function table, validates
the runtime catalog, and routes every output through the same generic operation
path.

The generated [`merman.h`](../../crates/merman-ffi/include/merman.h) header is
the authoritative native wire definition. ABI 2 is removed. A Dart package and
native library from different releases must fail discovery rather than attempt a
legacy symbol fallback.

Start with the [package README](../../platforms/flutter/README.md) for normal
use, the [native ABI protocol](FFI_PROTOCOL.md) for C-level details, and the
[options contract](OPTIONS_JSON.md) for resource policy.

## Public Dart API

Application code imports `package:merman/merman.dart`. It deliberately does
not expose generated raw FFI declarations, pointers, callback context, or
native result buffers.

```dart
final merman = Merman.open();
try {
  final catalog = merman.runtimeCatalog;
  if (catalog.supportsOutput('svg')) {
    final svg = merman.renderSvg(source);
  }
} finally {
  merman.dispose();
}
```

`Merman.runtimeCatalog` is the primary runtime capability API. The facade
validates flat runtime-catalog schema `1`, ABI transport version `3`, sorted
capability/output/operation/adapter IDs, and text measurement availability before
it creates a usable engine. The typed catalog
is the only public capability surface; it does not expose a second raw
`runtime_contract` view.

The facade exposes:

- `execute(MermanOperation, ...)` for generic operation selection and
  `MermanOperationResult.bytes`, `.utf8Text`, or `.jsonObject` consumption. Each call accepts optional
  `optionsJson`.
- Typed conveniences for SVG, PNG, JPEG, PDF, ASCII, semantic/layout/analysis
  JSON, document analysis, and validation.
- `MermanReusableEngine` for a separate engine lifecycle, engine-level options,
  and host text measurement.
- `MermanResourceOptionsBuilder` and `MermanResourceOptions` for the shared
  resource-options JSON contract.
- `MermanException` and the typed
  `MermanUnsupportedOperationException` for native failures.

The facade owns `result_free` calls and copies every output into Dart-owned
memory before returning. A public API never borrows a native buffer or exposes a
native pointer.

## Native Discovery And Generated Declarations

`ffigen` generates `lib/src/generated/native_abi.dart` from `merman.h` and the
text-measurement header. The generated binding performs exactly one dynamic
lookup: `merman_get_native_api`. It then verifies the ABI version, layout digest,
required function-table entries, and generated record sizes. The package's C
consumer smoke test is the compile-run layout fingerprint; the Dart facade does
not duplicate a runtime offset probe.

Do not hand-maintain Dart copies of C structs, status values, output values, or
function names. Regenerate after a header change:

```sh
cd platforms/flutter
dart run ffigen --config ffigen.yaml
git diff --exit-code -- lib/src/generated/native_abi.dart
```

Only the handwritten facade is public. This keeps raw ABI evolution localized to
the generated file and guarantees that stale package/library combinations are
rejected at discovery.

Native failure JSON is projected as `MermanErrorKind`. Unknown operation codes throw
`MermanUnknownOperationException`; valid requests whose artifact lacks a backend throw
`MermanMissingCapabilityException` and expose the exact `capabilityId`. Both remain subclasses of
`MermanUnsupportedOperationException` for status-oriented callers. The facade reads the kind and
failure-schema values from the generated header projection and rejects a mismatched schema as a
native contract error.

## Lifecycle, Options, And Output

`Merman` owns one default native engine. Pass SVG, resource, layout,
environment, and theme options to `Merman.open`, `Merman.openPath`,
`Merman.fromDynamicLibrary`, or `reusableEngine`.

```dart
final merman = Merman.open(
  optionsJson: '''
    {
      "version": 1,
      "resources": {"profile": "constrained"},
      "svg": {"pipeline": "resvg-safe"}
    }
  ''',
);
```

Use `MermanReusableEngine` for repeated work with one constructor-owned options
baseline or a host callback. `dispose()` and `close()` are idempotent after a
successful disposal. Calling either method from an active callback is rejected
with `MERMAN_NATIVE_STATUS_REENTRANT_CALL` and `MermanErrorKind.reentrantCall`;
retry after the enclosing native call returns. Re-entering the same engine from
a callback is rejected the same way.

The ABI 3 request contains the operation code, source, optional URI, and generic
`optionsJson`. Request objects are deeply merged over constructor options for that call, with
request values taking precedence while unspecified nested values remain inherited. A request
cannot set `runtime_policy`; select it when constructing the engine. Every current backend
materializes its output, so the transport returns one owned result buffer and does not expose a
chunk-sink API.

## Text Measurement

Register an optional text measurer when constructing a reusable engine:

```dart
final engine = merman.reusableEngine(
  textMeasurer: (request) {
    if (request.operation == MermanTextMeasurementOperation.measure) {
      return MermanTextMeasureResult.metrics(
        width: 42,
        height: 18,
        lineCount: 1,
      );
    }
    return null;
  },
);
```

The protocol's operation and result-kind codes are generated from the header;
the Dart API uses typed operations and result factories instead of raw codes.
The callback runs synchronously through `NativeCallable.isolateLocal`. Create,
use, and dispose this engine on the same isolate, and do not wait for WebView
JavaScript, a platform channel, another isolate, or font loading in the
callback. Precompute/cache a measurement service and return `null` when it has
no faithful answer so Merman can use its deterministic fallback. See
[host text measurement](HOST_TEXT_MEASUREMENT.md#flutter--dart-ffi) for cache
keys and display-surface guidance.

## Platform Packaging

| Platform | Native artifact |
| --- | --- |
| Android | `libmerman_ffi.so` for `arm64-v8a` and `x86_64` |
| iOS | Dynamic `MermanFFI.xcframework` |
| macOS | Universal dylib and SwiftPM XCFramework |
| Linux | target-specific `libmerman_ffi.so` |
| Windows | `merman_ffi.dll` |

All Flutter native helpers resolve the descriptor-owned `c-abi-native` profile and build the
native SDK with its exact feature recipe:

```sh
cargo build -p merman-ffi --release --no-default-features --features 'svg,analysis,ascii,png,jpeg,pdf,layout-cytoscape,layout-elk,math,system-clock,system-timezone,system-random'
```

Android slices are assembled by `platforms/android/build-android.py`; iOS and
desktop helpers are `platforms/flutter/build-ios.sh` and
`platforms/flutter/build-desktop.sh`. Flutter Web is not supported by this
package because it uses `dart:ffi`; use `@mermanjs/web` in browsers.

## Local Verification

```sh
cargo build -p merman-ffi --no-default-features --features 'svg,analysis,ascii,png,jpeg,pdf,layout-cytoscape,layout-elk,math,system-clock,system-timezone,system-random'
cd platforms/flutter
flutter pub get
dart run ffigen --config ffigen.yaml
dart format --set-exit-if-changed lib example tool
flutter analyze
dart run tool/abi3_contract_test.dart
dart run example/smoke.dart ../../target/debug/libmerman_ffi.dylib
```

Use `.so` on Linux and `.dll` on Windows. The repository-wide gate also checks
the generated declaration freshness and can build Android slices:

```sh
python3 scripts/verify-platform-bindings.py --build-android-slices
python3 scripts/verify-platform-bindings.py --build-android-slices --run-flutter-android-smoke
```

The local contract test rejects malformed catalog relations before an engine is
used. The smoke test exercises ABI discovery, the runtime catalog, generic
output, SVG/ASCII/PNG/JPEG/PDF output, document analysis, host measurement, and
per-request option merging against a real native SDK library.
