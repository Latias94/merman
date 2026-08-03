# Flutter/Dart FFI Binding

Status: experimental, publishable Flutter package.

`platforms/flutter` provides the `merman` Dart/Flutter package directly over the canonical Merman native ABI 3. It is a Dart facade, not a second operation protocol: the package discovers one size-tagged C function table, validates the runtime catalog, and routes every output through the same generic operation path.

The generated [`merman.h`](../../crates/merman-ffi/include/merman.h) header is the authoritative native wire definition. ABI 2 and pre-freeze ABI 3 layouts are removed. The Dart transport does not pass through JNI or UniFFI, including on Android; those transports have separate owners and lifecycles.

Start with the [package README](../../platforms/flutter/README.md) for normal use, the [native ABI protocol](FFI_PROTOCOL.md) for C-level details, the [ABI 3 migration guide](ABI3_MIGRATION.md) for host changes, and the [options contract](OPTIONS_JSON.md) for resource policy.

## Public Dart API

Application code imports `package:merman/merman.dart`. Generated raw FFI declarations, native pointers, callback context, engine tokens, result allocation tokens, and native result buffers remain private.

```dart
final merman = Merman.open();
try {
  final catalog = merman.runtimeCatalog;
  if (catalog.supportsOutput('svg')) {
    final svg = merman.renderSvg(source);
  }
} finally {
  merman.close();
}
```

`Merman.runtimeCatalog` is the primary runtime capability API. The facade validates flat runtime-catalog schema `1`, ABI transport version `3`, supported options and payload schema versions, named metadata IDs, sorted capability/output/operation/adapter IDs, text measurement availability, complete resource-to-operation relations, and agreement between the table and catalog package versions before it creates a usable engine.

Detailed registries remain separate from that flat catalog. `supportedDiagrams()`, `asciiCapabilities()`, `diagramFamilyCapabilities()`, `lintRuleCatalog()`, `supportedThemes()`, and `presentationCatalog()` call the generic appended ABI 3 `metadata_collect` slot and return immutable typed Dart values. This keeps the metadata surface extensible without adding per-catalog native symbols or copying detailed catalogs into the runtime catalog.

The generated `MermanResourceLimitId` describes the complete catalog vocabulary. `MermanResourceOverrideId` is the narrower set accepted by `MermanResourceOptionsBuilder`, whose profile is optional so reusable requests can inherit their constructor ceiling. The generated values intentionally do not duplicate the loaded artifact's descriptive metadata or budget table.

The loaded artifact remains authoritative at runtime. `resourceLimits`, `resourceProfiles`, `resourceLimitsById`, and `resourceProfilesById` expose the same typed descriptor shapes from the runtime catalog, including each limit's applicable operation IDs. `generalBindingDefaultResourceProfile` and `cliDefaultResourceProfile` resolve its default IDs. Schema-1 parsing accepts additive declared IDs for ABI-compatible `openPath` and `fromDynamicLibrary` loads, but rejects missing or duplicate descriptors, undeclared or missing profile-limit references, coerced scalar types, values below the declared minimum, unbounded hard caps, and inconsistent default recommendations.

The generic operation entry point returns structured data:

```dart
final result = merman.execute(
  MermanOperation.semanticJson,
  source,
  optionsJson: '{"resources":{"profile":"constrained"}}',
);

print(result.operation);
print(result.mediaType);
print(result.metadata);
final semantic = result.jsonObject;
```

`MermanOperationResult` contains the requested `operation`, returned `mediaType`, copied Dart-owned `bytes`, and decoded operation `metadata`. `utf8Text` and `jsonObject` are decoding conveniences. Named methods for SVG, PNG, JPEG, PDF, ASCII, semantic/layout/analysis JSON, document analysis, and validation are projections over the same `execute` path.

Native failures use `MermanException` and machine-readable `MermanErrorKind`. Unknown operation codes throw `MermanUnknownOperationException`; valid operations whose artifact lacks a backend throw `MermanMissingCapabilityException` with the exact `capabilityId`. `MermanBusyException` and `MermanReentrantCallException` preserve the two nonblocking engine-admission failures. Resource failures expose optional typed `resourceDetails` with the stable limit ID, phase, actual value, effective maximum, and selected profile.

## ABI Discovery

`ffigen` generates `lib/src/generated/native_abi.dart` from `merman.h` and `merman_text_measurement_abi.h`. The generated binding performs exactly one dynamic lookup: `merman_get_native_api`. It never reconstructs per-operation symbol names.

Discovery sends:

- `MERMAN_NATIVE_ABI_VERSION`;
- `MERMAN_NATIVE_ABI_MINIMUM_PREFIX_LAYOUT_DIGEST`;
- the Dart consumer's `MermanNativeApi` capacity.

The frozen ABI 3 prefix has five ordered slots: `runtime_catalog`, `engine_new`, `engine_try_close`, `execute_collect`, and `result_free`. The current table appends `metadata_collect` at code `5`. The caller passes its real table capacity and the producer returns the largest complete prefix safely initialized within that capacity. The Dart consumer accepts a prefix at least as large as `MERMAN_NATIVE_API_MINIMUM_PREFIX_SIZE`, verifies the returned ABI version and minimum-prefix digest, then reads an appended slot only when the returned initialized size reaches that field and its pointer is nonzero. It does not require the initialized prefix size to equal the current `ffigen` struct size, so an append-only ABI 3 producer remains discoverable without exposing a partial function pointer.

The returned digests have separate roles:

- `minimum_prefix_layout_digest` is the compatibility key and must equal the generated consumer value;
- `full_descriptor_digest` records producer descriptor provenance and is not a compatibility rejection key;
- `capability_catalog_digest` identifies the loaded artifact and is not a compatibility rejection key.

`package_version` is also provenance unless the caller deliberately pins an exact artifact:

- `Merman.open()` loads the package-owned library, requires the exact generated `mermanPackageVersion`, and treats a missing current `metadata_collect` slot as a damaged bundled artifact;
- `Merman.openPath(...)` requires only compatible ABI 3 discovery and accepts another package version; a five-slot producer remains usable for runtime catalog and operations, while named metadata methods report that the optional slot is unavailable;
- `Merman.fromDynamicLibrary(...)` is ABI-compatible by default and accepts `expectedPackageVersion:` when an embedding application wants an explicit exact pin.

The exact package version lives in `lib/src/generated/package_version.dart`, is exported as `mermanPackageVersion`, and is updated from the workspace release authority alongside `pubspec.yaml`. The contract test rejects projection drift.

Do not hand-maintain Dart copies of C records, status values, operation values, function slots, or digest literals. Regenerate after a header change:

```sh
cd platforms/flutter
dart run ffigen --config ffigen.yaml
git diff --exit-code -- lib/src/generated/native_abi.dart
```

## Native Result Ownership

Every producing call receives a freshly `calloc`-initialized `MermanNativeResult` with only the exact `struct_size` set. The complete record is therefore zeroed before native code inspects its ownership state.

After a producing call, every written result must carry a nonzero opaque `allocation_token`. The only valid zero-token terminal state is `MERMAN_NATIVE_STATUS_INTERNAL_ERROR` with every field except the caller-set `struct_size` still zero, which preserves the ABI-defined token-issuance exhaustion failure. Any other zero-token or partially written result fails as a native contract violation.

For a written result, the Dart transport validates the result status, copies data and metadata into Dart-owned memory, calls the discovered `result_free` function exactly once, and only then releases the caller-owned result record. `result_free` owns native buffer cleanup through the token; Dart never frees a result buffer pointer through `calloc` and never exposes the token publicly. The zero-token exhaustion record is also passed to `result_free`, whose ABI contract safely clears it without releasing an allocation.

This ownership path applies to runtime catalog loading, named metadata loading, engine construction results, successful operations, and structured operation failures. Moving or duplicating native result records is unnecessary in Dart.

## Engine Lifecycle

The native engine is an opaque nonzero `uint64_t` token stored privately as a Dart `int`. `Merman` owns one default engine, and `MermanReusableEngine` owns an independent token.

An optional `MermanTextMeasurer` is immutable constructor state:

```dart
final merman = Merman.open(
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

Pass the callback to `Merman.open`, `Merman.openPath`, `Merman.fromDynamicLibrary`, or `Merman.reusableEngine`. Create another engine to use another callback.

`close()` and its compatibility alias `dispose()` call `engine_try_close` and never wait:

- `MERMAN_NATIVE_STATUS_OK` retires the token, clears the Dart handle, and releases callback state;
- `MERMAN_NATIVE_STATUS_BUSY` throws `MermanBusyException` and retains the token and callback for retry;
- `MERMAN_NATIVE_STATUS_REENTRANT_CALL` throws `MermanReentrantCallException` and retains the token and callback for retry;
- a second close after success is idempotent.

`Merman` marks itself closed only after its default engine closes successfully. Closing or re-entering a reusable engine from its active callback is rejected locally with the same REENTRANT classification. This mirrors the native admission contract without blocking the Dart isolate.

## Options And Output

Pass SVG, resource, layout, environment, and theme options to engine construction:

```dart
final merman = Merman.open(
  optionsJson: '''
    {
      "version": 2,
      "resources": {"profile": "constrained"},
      "svg": {"pipeline": "resvg-safe"}
    }
  ''',
);
```

Each `execute` call accepts the operation, source, optional URI, and generic `optionsJson`. Request options are deeply merged over immutable constructor options for that call, with request values taking precedence while unspecified nested values remain inherited. A request cannot set `runtime_policy`; select it when constructing the engine.

Every current backend materializes its output, so the transport returns one owned result instead of exposing a chunk sink. The public facade copies bytes before native cleanup, making SVG, bitmap, PDF, and JSON lifetimes identical.

## Text Measurement

The callback runs synchronously through `NativeCallable.isolateLocal`. Create, use, and close a measured engine on the same isolate. Do not wait for WebView JavaScript, a platform channel, another isolate, or font loading inside the callback. Precompute or cache a measurement service and return `null` when it has no faithful answer so Merman can use its deterministic fallback.

The callback bridge catches every Dart exception and returns `MERMAN_NATIVE_STATUS_CALLBACK_ERROR`; no Dart exception may unwind across the C boundary. Request and result operation codes come from the generated text-measurement header rather than handwritten Dart numbers.

See [host text measurement](HOST_TEXT_MEASUREMENT.md#flutter--dart-ffi) for cache keys and display-surface guidance.

## Platform Packaging

| Platform | Native artifact |
| --- | --- |
| Android | `libmerman_ffi.so` for `arm64-v8a` and `x86_64` |
| iOS | Dynamic `MermanFFI.xcframework` |
| macOS | Universal dylib and SwiftPM XCFramework |
| Linux | Target-specific `libmerman_ffi.so` |
| Windows | `merman_ffi.dll` |

Flutter uses owner-specific C ABI recipes. Android selects `flutter-android-native`; iOS and desktop select their corresponding target-set recipes. These recipes package `merman-ffi` directly. The Kotlin AAR's JNI transport remains structurally isolated in `merman-android-jni`, and Python's UniFFI transport is not part of the Dart call path.

```sh
cargo build -p merman-ffi --release --no-default-features --features 'svg,analysis,ascii,png,jpeg,pdf,layout-cytoscape,layout-elk,math,system-clock,system-timezone,system-random'
```

Android slices are assembled by `platforms/android/build-android.py --artifact-profile flutter-android-native`; iOS and desktop helpers are `platforms/flutter/build-ios.sh` and `platforms/flutter/build-desktop.sh`. Flutter Web is not supported because this package uses `dart:ffi`; use `@mermanjs/web` in browsers.

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

Use `.so` on Linux and `.dll` on Windows. The local contract test statically projects the frozen minimum prefix, optional metadata slot, and allocation token; validates runtime-catalog and typed metadata relations; checks package-version projection; verifies BUSY/REENTRANT decoding; and deterministically fuzzes malformed native error payloads. The real-library smoke loads all six metadata catalogs, repeatedly executes generic operations, exercises result cleanup, verifies exact-version and ABI-compatible loading, uses constructor-owned text measurement, proves a callback-time REENTRANT close retains the engine, and closes successfully afterward.

The repository-wide gate also checks generated declaration freshness and can build Android slices:

```sh
python3 scripts/verify-platform-bindings.py --build-android-slices
python3 scripts/verify-platform-bindings.py --build-android-slices --run-flutter-android-smoke
```
