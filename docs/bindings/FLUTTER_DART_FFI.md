# Flutter/Dart FFI Binding

Status: experimental, publishable Flutter package.

`platforms/flutter` provides the `merman` Dart/Flutter package directly over the canonical Merman native ABI 3. It is a Dart facade, not a second operation protocol: the package discovers one size-tagged C function table, validates the runtime catalog, and routes every output through the same generic operation path.

The generated [`merman.h`](../../crates/merman-ffi/include/merman.h) header is the authoritative native wire definition. ABI 2 and pre-freeze ABI 3 layouts are removed. The Dart transport does not pass through JNI or UniFFI, including on Android; those transports have separate owners and lifecycles.

Start with the [package README](../../platforms/flutter/README.md) for normal use, the [native ABI protocol](FFI_PROTOCOL.md) for C-level details, the [ABI 3 migration guide](ABI3_MIGRATION.md) for host changes, and the [options contract](OPTIONS_JSON.md) for resource policy.

## Public Dart API

Application code imports `package:merman/merman.dart`. Generated raw FFI declarations, native pointers, callback context, engine tokens, result allocation tokens, and native result buffers remain private.

```dart
final merman = Merman.open();
final catalog = merman.runtimeCatalog;
if (catalog.supportsOutput('svg')) {
  final svg = merman.renderSvg(source);
}
```

`Merman` is a stateless discovery and one-shot facade; it owns no native engine token and therefore
has no `close()` method. Each operation constructs a fresh deterministic engine and closes it before
returning. `Merman.runtimeCatalog` is the primary runtime capability API. The facade validates flat
runtime-catalog schema `1`, ABI transport version `3`, supported options and payload schema
versions, named metadata IDs, sorted capability/output/operation/adapter IDs, text measurement
availability, complete resource-to-operation relations, and agreement between the table and catalog
package versions before it becomes usable.

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

Native failures use `MermanException` and machine-readable `MermanErrorKind`. Unknown operation codes throw `MermanUnknownOperationException`; valid operations whose artifact lacks a backend throw `MermanMissingCapabilityException` with the exact `capabilityId`. `MermanBusyException` and `MermanReentrantCallException` preserve the two nonblocking engine-admission failures. Resource failures expose optional typed `resourceDetails` with the stable cause (`ceiling` or `arithmetic_overflow`), limit ID, phase, actual value, effective maximum, and selected profile.

## ABI Discovery

`ffigen` generates `lib/src/generated/native_abi.dart` from `merman.h` and `merman_text_measurement_abi.h`. The generated binding performs exactly one dynamic lookup: `merman_get_native_api`. It never reconstructs per-operation symbol names.

Discovery sends:

- `MERMAN_NATIVE_ABI_VERSION`;
- `MERMAN_NATIVE_ABI_MINIMUM_PREFIX_LAYOUT_DIGEST`;
- the Dart consumer's `MermanNativeApi` capacity.

The ABI 3 table is size-tagged. The caller passes its real table capacity and the producer returns
only complete fields safely initialized within that capacity, so a host never reads a partial
function pointer. Release builds use the matching generated header and current complete table;
historical five- or six-slot producers are not a supported compatibility target.

The returned digests have separate roles:

- `minimum_prefix_layout_digest` is the compatibility key and must equal the generated consumer value;
- `full_descriptor_digest` records producer descriptor provenance and is not a compatibility rejection key;
- `capability_catalog_digest` identifies the loaded artifact and is not a compatibility rejection key.

`package_version` is also provenance unless the caller deliberately pins an exact artifact:

- `Merman.open()` loads the package-owned library, requires the exact generated `mermanPackageVersion`, and treats a missing current `metadata_collect` slot as a damaged bundled artifact;
- `Merman.openPath(...)` accepts another package version only when it implements the current complete
  ABI table and runtime schemas;
- `Merman.fromDynamicLibrary(...)` applies the same current-contract requirement and accepts
  `expectedPackageVersion:` when an embedding application wants an explicit exact pin.

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

The native engine is an opaque nonzero `uint64_t` token stored privately as a Dart `int` by
`MermanEngine`. Constructor options and services are immutable for that engine. `Merman` does not
retain a token.

An optional `MermanTextMeasurer` is installed through `MermanEngineServices`:

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
      return null;
    },
  ),
);
try {
  engine.renderSvg(source);
} finally {
  engine.close();
}
```

Create another engine to use another callback. Construction validates the service contract but does
not invoke the callback.

`MermanEngine.close()` calls `engine_try_close` and never waits:

- `MERMAN_NATIVE_STATUS_OK` retires the token, clears the Dart handle, and releases callback state;
- `MERMAN_NATIVE_STATUS_BUSY` throws `MermanBusyException` and retains the token and callback for retry;
- `MERMAN_NATIVE_STATUS_REENTRANT_CALL` throws `MermanReentrantCallException` and retains the token and callback for retry;
- a second close after success is idempotent.

A close or execution attempted from the engine's active callback is rejected locally with the same
REENTRANT classification. This mirrors the native admission contract without blocking the Dart
isolate. There is no `dispose()` compatibility alias.

## Options And Output

Pass SVG, resource, layout, environment, and theme options to engine construction:

```dart
final engine = MermanEngine(
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

## Immutable Icon Registries

`MermanIconPack` accepts one in-memory IconifyJSON collection and an optional registration-name
override. `MermanIconPackSet.fromPacks(...)` validates fixed byte/count ceilings and snapshots
immutable UTF-8 buffers. Those buffers are borrowed only during `MermanEngine` construction; the
engine owns the parsed registry when construction returns. The same Dart pack set can therefore be
reused for multiple engine constructions without exposing a native mutable handle.

Merman performs no filesystem, package, or network acquisition and does not trim collections.
Hosts must acquire and pre-trim packs that exceed the fixed limits reported by the runtime catalog.
Icon fragments are XML-validated, deterministically ID-scoped, and sanitized under the effective
Mermaid configuration before embedding. This does not make parity/readable SVG safe for direct
browser DOM insertion; use `SafeInlineSvg`, CSP, or a sandbox at that boundary.

## Text Measurement

The callback runs synchronously through `NativeCallable.isolateLocal`. Create, use, and close a measured engine on the same isolate. Do not wait for WebView JavaScript, a platform channel, another isolate, or font loading inside the callback. Precompute or cache a measurement service and return `null` when it has no faithful answer so Merman can use its deterministic fallback.

The callback bridge catches every Dart exception and returns `MERMAN_NATIVE_STATUS_CALLBACK_ERROR`; no Dart exception may unwind across the C boundary. Request and result operation codes come from the generated text-measurement header rather than handwritten Dart numbers.

See [host text measurement](HOST_TEXT_MEASUREMENT.md#flutter--dart-ffi) for cache keys and display-surface guidance.

## Migrating From The Previous Prerelease API

- Keep `Merman` only for stateless discovery and one-shot calls; it no longer owns an engine or
  exposes `close()`/`dispose()`.
- Delete `MermanReusableEngine` and `Merman.reusableEngine(...)`; construct `MermanEngine(...)`
  directly.
- Delete constructor-level `textMeasurer:` parameters and use `MermanEngineServices`.
- Call `MermanEngine.close()` deterministically. Busy and re-entrant failures preserve the complete
  engine and callback for retry.
- Use typed `MermanOperationMetadata`; its raw JSON preserves additive fields and unknown future
  output-plan kinds.
- Treat runtime operation and resource-limit IDs as open value objects. A newly appended numeric C
  operation still requires an updated generated Dart SDK before invocation.

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
cargo build -p merman-ffi --release --no-default-features --features 'svg,analysis,ascii,png,jpeg,pdf,layout-cytoscape,layout-elk,math,native-runtime'
```

`native-runtime` is the C binding's atomic clock, time-zone, and random adapter feature. The
loaded runtime catalog still reports those adapters by their concrete `system-clock`,
`system-timezone`, and `system-random` IDs; Flutter hosts should not treat the Cargo aggregate as a
runtime capability ID.

Android slices are assembled by `platforms/android/build-android.py --artifact-profile flutter-android-native`; iOS and desktop helpers are `platforms/flutter/build-ios.sh` and `platforms/flutter/build-desktop.sh`. Flutter Web is not supported because this package uses `dart:ffi`; use `@mermanjs/web` in browsers.

## Local Verification

```sh
cargo build -p merman-ffi --no-default-features --features 'svg,analysis,ascii,png,jpeg,pdf,layout-cytoscape,layout-elk,math,native-runtime'
cd platforms/flutter
flutter pub get
dart run ffigen --config ffigen.yaml
dart format --set-exit-if-changed lib example tool
flutter analyze
dart run tool/abi3_contract_test.dart
dart run example/smoke.dart ../../target/debug/libmerman_ffi.dylib
```

Use `.so` on Linux and `.dll` on Windows. The local contract test validates the current complete ABI
3 table boundary, runtime-catalog and typed metadata relations, package-version projection,
BUSY/REENTRANT decoding, and malformed native error payloads. The real-library smoke intentionally
does one service-backed SVG render through icon packs and host text measurement, then closes the
engine. Owner-local Rust and Dart contract tests carry the exhaustive operation and lifecycle cases.

The repository-wide gate checks generated declaration freshness and the desktop smoke. Android
packaging has its own direct entry point:

```sh
python3 scripts/verify-platform-bindings.py
python3 platforms/flutter/tool/android-smoke.py --targets aarch64-linux-android
```
