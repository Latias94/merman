# Flutter/Dart FFI Binding

Status: experimental, publishable Flutter package.

`platforms/flutter` provides the `merman` Dart/Flutter package directly over the canonical Merman native ABI 3. It follows Dart's `package_ffi` structure and uses Native Assets for platform bundling. It is a Dart facade, not a second operation protocol: the package discovers one size-tagged C function table, validates the runtime catalog, and routes every output through the same generic operation path.

The generated [`merman.h`](../../crates/merman-ffi/include/merman.h) header is the authoritative native wire definition. ABI 2 and pre-freeze ABI 3 layouts are removed. The Dart transport does not pass through JNI or UniFFI, including on Android; those transports have separate owners and lifecycles.

This guide owns Dart transport and packaging details. Start with the
[package README](../../platforms/flutter/README.md) for application use, the
[native ABI protocol](FFI_PROTOCOL.md) for C-level rules and contract evolution, and the
[options contract](OPTIONS_JSON.md) for resource policy.

The transport owner is the [Dart facade](../../platforms/flutter/lib/src/merman_ffi.dart), with raw
declarations generated through [ffigen](../../platforms/flutter/ffigen.yaml) and packaging owned by
the [Flutter release workflow](../../.github/workflows/release-flutter.yml). Keep one Flutter
transport. A replacement must preserve request, capability, resource, runtime-policy, structured
error, ownership, and synchronous callback semantics; demonstrate a material application benefit;
and own the generator, package, and CI path across Android, iOS, macOS, Linux, and Windows. The
removed Flutter Rust Bridge experiment did not meet those requirements. Scheduling alone does not
provide synchronous text measurement, bounded queues, preemptive cancellation, or incremental core
output.

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

Detailed registries remain separate from that flat catalog. `supportedDiagrams()`, `asciiCapabilities()`, `diagramFamilyCapabilities()`, `lintRuleCatalog()`, `supportedThemes()`, and `themeCatalog()` call the generic appended ABI 3 `metadata_collect` slot and return immutable typed Dart values. The theme catalog describes the selected artifact's typed semantic-theme vocabulary and carries an explicit open-string maturity for every built-in preset. The initial inventory is `alpha`; catalog presence does not imply a stable preset or cross-target portability claim. This keeps the metadata surface extensible without adding per-catalog native symbols or copying detailed catalogs into the runtime catalog.

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

Typed `MermanOperationMetadata` retains raw JSON for additive fields and unknown future output-plan
kinds. Runtime operation and resource-limit IDs remain open values; invoking an appended numeric C
operation still requires an updated generated Dart SDK.

The published `0.9.2` package's prebuilt libraries select the historical default native SKU:
SVG, semantic JSON, layout JSON, both native layout engines, ASCII, analysis, validation, and
document analysis. It omits `math`, `png`, `jpeg`, `pdf`, and `native-runtime`. The next
main-line target assets include `math` while retaining the absent exporters and adapters, so one
thin facade can carry the Android, Apple, Linux, and Windows target matrix without bundling all
native bytes. The generated Dart operation vocabulary remains unchanged for custom
current-contract libraries; unavailable bundled operations use the existing typed
`missing-capability` result.

Native failures use `MermanException` and machine-readable `MermanErrorKind`. Unknown operation codes throw `MermanUnknownOperationException`; valid operations whose artifact lacks a backend throw `MermanMissingCapabilityException` with the exact `capabilityId`. `MermanBusyException` and `MermanReentrantCallException` preserve the two nonblocking engine-admission failures. Resource failures expose `exactResourceDetails` with the stable cause (`ceiling` or `arithmetic_overflow`), limit ID, phase, canonical unsigned-decimal actual value, effective maximum, and selected profile. The compatibility `resourceDetails` view remains available only when both counts fit a signed 64-bit Dart `int`.

Cooperative cancellation is a separate failure path. Native status
`MERMAN_NATIVE_STATUS_CANCELLED` (`17`) becomes `MermanCancelledException`, whose
`cancellationDetails` preserves the stable `reason` (`requested` or `deadline_exceeded`) and the
phase that observed it. It is never decoded as a resource-limit exception.

## ABI Discovery

`ffigen` generates `lib/src/generated/native_abi.dart` from `merman.h` and `merman_text_measurement_abi.h`. The generated `@Native` declaration resolves exactly one entry point through the Native Assets ID: `merman_get_native_api`. It never reconstructs per-operation symbol names.

Discovery sends:

- `MERMAN_NATIVE_ABI_VERSION`;
- `MERMAN_NATIVE_ABI_MINIMUM_PREFIX_LAYOUT_DIGEST`;
- the Dart consumer's `MermanNativeApi` capacity.

The ABI 3 table is size-tagged. The caller passes its real table capacity and the producer returns
only complete fields safely initialized within that capacity, so a host never reads a partial
function pointer. Release builds use the matching generated header and current complete table;
historical five- or six-slot producers are not a supported compatibility target. The minimum ABI 3
prefix still ends at `engine_new_with_services` (slot `6`), while the current Dart package requires
the complete appended table through `execute_collect_controlled` (slot `10`). The generated prefix
constants for slots `7`-`10` make those complete boundaries explicit.

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

## Cooperative Operation Control

`Merman.createOperationControl(...)` and `MermanEngine.createOperationControl(...)` create an
opaque `MermanOperationControl`. An omitted timeout creates a control with no deadline; a
non-negative `Duration` creates a relative monotonic deadline, and `Duration.zero` is immediate.
Attach the control through the typed `execute(..., control: control)` path:

```dart
final engine = MermanEngine();
final control = engine.createOperationControl(
  timeout: const Duration(milliseconds: 250),
);
try {
  final result = engine.execute(
    MermanOperation.svg,
    source,
    control: control,
  );
  print(result.utf8Text);
} on MermanCancelledException catch (error) {
  print('${error.cancellationDetails?.reason} '
      '${error.cancellationDetails?.phase}');
} finally {
  control.release();
  engine.close();
}
```

At the C boundary, an omitted Dart control calls `execute_collect`; a supplied control calls the
append-only `execute_collect_controlled` entry point without changing the ABI 3 request record. The
nonzero token is borrowed, may be reused across operations, and must be released explicitly.
`release()` is idempotent in Dart and retires the token only; an operation that already cloned the
control remains safe until it returns.

Cancellation is cooperative. The synchronous operation stops at its next parser, layout, adapter,
post-processing, or export checkpoint and publishes no partial output. Opaque synchronous native
backends and the isolate-local text-measurement callback cannot be forcefully interrupted; a
request made while either is running is observed after it returns to a checkpoint.

The public Dart calls are synchronous and `MermanOperationControl` is isolate-local. Therefore a
timer or message on the same blocked isolate cannot invoke `cancel()` during `execute`; the Dart
facade directly supports deadlines, cancellation before execution, and cancellation requested from
inside a synchronous host callback. The raw C ABI permits another native thread to cancel a shared
token, but the Dart facade intentionally keeps that token private. An application that needs
message-driven mid-render cancellation must own a worker/native bridge that can call the control
slot concurrently; use a process boundary when it also requires forceful termination.

## Options And Output

Pass SVG, resource, layout, environment, and theme options to engine construction:

```dart
final engine = MermanEngine(
  optionsJson: '''
    {
      "version": 3,
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

The callback bridge catches every Dart exception and returns `MERMAN_NATIVE_STATUS_CALLBACK_ERROR`; no Dart exception may unwind across the C boundary. An operation control does not preempt the callback body, so the callback must still return normally before cancellation can be observed. Request and result operation codes come from the generated text-measurement header rather than handwritten Dart numbers.

See [host text measurement](HOST_TEXT_MEASUREMENT.md#flutter--dart-ffi) for cache keys and display-surface guidance.

## Platform Packaging

| Platform | Native artifact |
| --- | --- |
| Android | `libmerman_ffi.so` for `armeabi-v7a`, `arm64-v8a`, and `x86_64` |
| iOS | arm64 device plus arm64/x86_64 simulator dylibs |
| macOS | arm64 and x86_64 dylibs |
| Linux | arm64 and x86_64 `libmerman_ffi.so` |
| Windows | `merman_ffi.dll` |

Flutter uses owner-specific C ABI recipes. Android selects `flutter-android-native`; iOS and desktop select their corresponding target-set recipes. These recipes package `merman-ffi` directly with the size-oriented `native-distribution` Cargo profile, including math for ordinary SVG rendering. PNG, JPEG, PDF and native runtime adapters remain absent. This recipe targets `0.10.0-alpha.1`; published `0.9.2` retains its original package and capability contract. The Kotlin AAR's JNI transport remains structurally isolated in `merman-android-jni`, and Python's UniFFI transport is not part of the Dart call path.

`hook/build.dart` maps the requested OS, architecture, and iOS SDK to one target, acquires its
prebuilt library, then emits a bundled dynamic `CodeAsset` with the same asset ID used by the
generated `@Native` declaration. Flutter performs final copying, multi-architecture Apple
framework assembly, install-name rewriting, and signing. The package therefore owns no Flutter
plugin registrars, CocoaPods podspecs, Swift packages, Gradle plugin modules, or CMake plugin
wrappers. Android applications must target API 24 or newer; the hook rejects a lower target
before acquisition.

Release staging generates `hook/native-assets.json` from the frozen native output, containing
one versioned `.gz` asset for each table entry above, its exact download URL, SHA-256 and size,
and the uncompressed library digest and size. iOS device and simulator slices have distinct
asset names. The manifest also binds the Dart version and immutable source commit/tree.
`scripts/flutter_release_archive.py pack --native-manifest ...` inserts it into the frozen facade
and physically excludes `native/` and repository-only `tool/`. The collector enforces this
independently of `.pubignore`; `extract --require-thin` verifies the manifest identity again.
The source checkout has no hand-maintained second manifest.

The hook downloads during build only and verifies both compressed and materialized bytes on
each invocation. It stores these files under hooks' `outputDirectoryShared` and registers the
manifest, cached archive and selected library as dependencies so hooks invalidates stale outputs.
The runner owns invocation caching and serialization. Failed downloads leave no partial
accepted file. A corrupt cache fails explicitly, with the cache path to remove before retrying.
The hook never invokes Cargo or downloads at runtime.

Consumers configure `hooks.user_defines.merman` in their root/workspace `pubspec.yaml`:

| Define | Behavior |
| --- | --- |
| `native_assets: vendor/merman-native` | Explicit native root resolved through `input.userDefines.path`; contains the selected subtree such as `ios/arm64-simulator/libmerman_ffi.dylib`. Tracked as a dependency and used without network or manifest digest checks. Runtime ABI/version admission still applies. |
| `offline: true` | Require already verified hook cache bytes. First build online and preserve `.dart_tool` or supply `native_assets`; a cold cache explains the missing input. |
| `mirror: https://artifacts.example.org/flutter-v0.10.0-alpha.1/` | Replace only the asset URL prefix; use the same immutable filenames and package-pinned hashes. |

All downloads require HTTPS, with HTTP permitted only for loopback fixtures. Signed CDN
redirect URLs are supported. A local runtime `Merman.openPath(...)` does not configure the build
hook; use `native_assets` to override acquisition and bundling. During migration, a checkout
without a manifest still accepts explicitly built self-contained `native/` files.

```sh
cargo build -p merman-ffi --profile native-distribution --no-default-features --features 'svg,analysis,ascii,layout-cytoscape,layout-elk,math'
```

The bundled artifacts are deterministic and contain no system adapters. Custom source builds may
add the C binding's atomic `native-runtime` feature for clock, time-zone, and random adapters. The
loaded runtime catalog reports those adapters by their concrete `system-clock`, `system-timezone`,
and `system-random` IDs only when present; Flutter hosts should not treat the Cargo aggregate as a
runtime capability ID.

Android slices are assembled by `platforms/android/build-android.py --artifact-profile flutter-android-native`; `platforms/flutter/build-native.py` builds Apple and desktop slices. Flutter Web is not supported because this package uses `dart:ffi`; use `@mermanjs/web` in browsers.

### Publication And Consumer Qualification

The release build job is read-only. It freezes native target assets, builds the thin facade,
runs the existing Dart compressed-size budget against the extracted facade and qualifies a
relocated app. `platforms/flutter/tool/qualify_packaged_app.py` creates a temporary Flutter macOS
consumer of that exact package, downloads exactly the current host target from a local HTTP
fixture and builds a release `.app`. Its generated Xcode config excludes the other CPU, so this
lane qualifies the executed host slice. It removes temporary inputs/cache, stops HTTP, checks
the bundled native framework, relocates the app and executes it. `example/smoke.dart` checks real math vector glyphs, SVG/services,
ASCII/analysis, and typed absence of PNG/JPEG/PDF. Its uploaded JSON receipt records the source,
host and selected-target compressed/library sizes. Hosted macOS release CI owns this real binary
qualification; the small hook test covers target selection for other OS/CPU/Apple SDKs without
claiming to execute their binaries.

Only tag-push releases can publish. `publish-native-assets` is a separate job protected by the
`github-release` environment and scoped `contents: write`. It verifies the thin package receipt,
the exact expected GitHub URL prefix and frozen target hashes before uploading. The matching
non-draft GitHub Release must already exist and the remote tag must still resolve to the selected
source. An existing asset is reused only when its bytes match; accepted assets are never replaced.
The job then anonymously downloads every package-pinned URL and checks exact size/digest.
The pub.dev publisher depends on that successful readback and has only `contents: read` and
OIDC `id-token: write` permissions. It retains the existing pub reconciler before publication and
requires an exact public package reconciliation afterward. Manual candidate runs build and
qualify artifacts without either publication step.

Public tag/asset hosting and pub.dev resolution are release rollout proofs. A candidate archive
or workflow definition alone does not establish that a new public version can be installed.

## Local Verification

```sh
python3 platforms/flutter/build-native.py host
cd platforms/flutter
flutter pub get
dart run ffigen --config ffigen.yaml
dart format --set-exit-if-changed \
  lib/merman.dart lib/src/merman_ffi.dart lib/src/operation_metadata.dart \
  example tool hook
flutter analyze
dart run tool/abi3_contract_test.dart
dart run tool/native_assets_contract_test.dart
dart run example/main.dart
dart run example/smoke.dart
```

The local contract test validates the current complete ABI
3 table boundary through controlled-execution slot `10`, runtime-catalog and typed metadata relations,
package-version projection, BUSY/REENTRANT/CANCELLED decoding, and malformed native error payloads. The real-library smoke intentionally
exercises service-backed SVG, ASCII, and analysis, verifies typed absence for the three binary
exporters, then closes the engine. Owner-local Rust and Dart contract tests carry the exhaustive
operation and lifecycle cases. The small hook contract uses real loopback HTTP and supported
hooks input/output to exercise all target/SDK mappings, bad downloads, compressed/materialized
cache tampering, warm/cold offline behavior, mirror redirects and a local override; it needs no
native compilation or large SDK download.

The repository-wide gate checks generated declaration freshness and the current host Native Assets smoke. Complete release packaging has two direct entry points:

```sh
python3 scripts/verify-platform-bindings.py
python3 platforms/android/build-android.py --artifact-profile flutter-android-native
python3 platforms/flutter/build-native.py all-desktop
```
