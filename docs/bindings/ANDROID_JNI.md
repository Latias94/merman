# Android JNI Transport

Status: experimental Android transport.

`platforms/android` exposes a Kotlin API backed directly by `merman-bindings-core`. It is not a
wrapper around the C ABI: `JNI_OnLoad` registers a small, exact native method set with
`RegisterNatives`, so Android does not depend on Java-name-derived exported symbols.
The Android AAR builds the internal, non-published `merman-android-jni` crate through the
`android-native` artifact recipe. The public `merman-ffi` crate remains a C ABI transport and is
not linked into the Kotlin AAR.

## Layers

```text
Kotlin Merman / MermanEngine / MermanEngineServices / MermanOperationResult
                 |
                 v
JNI_OnLoad + RegisterNatives (libmerman_android_jni.so)
                 |
                 v
merman-bindings-core BindingEngine::execute
```

The Android transport API version is `1`. It is independent of native ABI 3, UniFFI, or browser
WASM transport versions.

## Kotlin Surface

The primary API is generic and returns a complete operation envelope:

```kotlin
val result = Merman.execute(
    operationId = "svg",
    source = "flowchart TD\nA --> B",
)
val svg = result.data.toString(Charsets.UTF_8)
```

`MermanOperationResult` contains `operationId`, `mediaType`, `data`, and typed operation metadata
that retains its original JSON. `Merman` also exposes convenience methods for SVG, ASCII, PNG,
JPEG, PDF, semantic JSON, layout JSON, analysis facts, SVG planning, document analysis, and
validation. The default AAR supports SVG, ASCII, semantic/layout operations, analysis, validation,
and document analysis. Binary methods remain in the generated API for custom artifacts and return
typed missing-capability errors against the default AAR.

Use the direct `MermanEngine(optionsJson, services)` constructor when calls share base options or
host services. Its `execute` method accepts per-call `optionsJson` overlays and uses the same
operation IDs while retaining a native engine for its lifetime. `MermanEngineServices` is immutable
and can contain an optional text measurer and immutable icon-pack snapshot. A callback-free engine accepts
concurrent calls. An engine with a callback rejects a competing operation immediately with `BUSY`;
a call or close from its callback returns `REENTRANT_CALL`. `close()` is nonblocking and idempotent,
and retains the Kotlin handle when it fails so callers can retry after the active operation returns.

## Runtime Catalog

During initialization, Kotlin loads `libmerman_android_jni.so`, requests the direct catalog, and
validates:

- flat runtime-catalog schema `1`;
- Android transport API version `1`;
- non-empty native package metadata;
- Options JSON schema `2` and both binding payload schemas;
- the generated default-native Android capability/output/operation/metadata identity and capability
  implication closure;
- output media/resource policies, constructor-service ownership, and resource profile relations;
  and
- the independent text-measurement protocol version and provider ownership.

The published Android AAR uses the default native SKU, so known IDs must match the generated
Android artifact contract in sorted order. This prevents package documentation from drifting from
the native library that it redistributes. Unknown future IDs and fields are accepted only as
additive values and must retain the catalog's sorted-ID rules. Legacy schema-1 producers may omit
the option-group and constructor-service sections; if either section is present, its known entries
are validated. Exact package-version equality is intentionally not required. A custom SKU must
regenerate and package the matching Kotlin contract.

The default AAR does not compile native runtime adapters. A custom `merman-android-jni` build may
enable the atomic `native-runtime` feature; it does not expose separate clock, time-zone, or random
Cargo switches. The catalog reports the concrete adapter IDs only when those callable services are
present rather than exposing the artifact assembly feature.

Read the validated catalog with `Merman.runtimeCatalogJson()`:

```json
{
  "schema_version": 1,
  "transport_api_version": 1,
  "package_version": "<loaded artifact version>",
  "options_schema_versions": [2],
  "payload_schemas": [
    { "id": "binding-result", "version": 1 },
    { "id": "operation-metadata", "version": 1 }
  ],
  "metadata_ids": ["ascii-capabilities", "..."],
  "capabilities": {
    "capability_ids": ["analysis", "ascii", "...", "svg"],
    "operation_ids": ["analysis-json", "...", "svg"],
    "output_ids": ["ascii", "svg"],
    "system_adapter_ids": [],
    "text_measurement": { "protocol_version": 1, "provider_ids": ["host-callback", "vendored"] }
  },
  "output_contracts": [{ "id": "ascii", "media_type": "text/plain; charset=utf-8" }, "..."],
  "registry": { "diagram_family_count": 35 },
  "resources": { "general_binding_default_profile": "interactive" }
}
```

Hosts use this catalog to decide whether the installed AAR can render a requested output. Do not
infer support from Kotlin method presence or Cargo feature names. The native operation path remains
the final authority and reports a typed `MermanException` instead of silently selecting a fallback
format.

`MermanException.kind` is `UNKNOWN_OPERATION`, `MISSING_CAPABILITY`, `BUSY`, `REENTRANT_CALL`, or `GENERIC`.
Resource failures populate `MermanException.resourceDetails` with the stable cause (`ceiling` or
`arithmetic_overflow`), limit ID, phase, actual value, effective maximum, and selected profile;
other failures leave it `null`.
`capabilityId` is non-null only for `MISSING_CAPABILITY` and preserves the descriptor ID emitted by
bindings-core. Local wrapper and lifecycle failures remain `GENERIC`; consumers should branch on
these fields rather than parse `message`.

## Text Measurement

Pass a synchronous `MermanTextMeasurer` through `MermanEngineServices` only when Android text
geometry must match the final surface:

```kotlin
val services = MermanEngineServices(
    textMeasurer = MermanTextMeasurer { request ->
        // Return null when the host cannot reproduce this operation faithfully.
        null
    },
)
MermanEngine(services = services).use { engine ->
    engine.renderSvg("flowchart TD\nA --> B")
}
```

Native previews should measure with the same `TextPaint`/`StaticLayout` configuration used for
display; WebView previews should use a prepared DOM/canvas cache rather than block a render thread
on UI work. Construction never invokes the callback.

The independent text-measurement protocol has 19 operations (`0..18`). Construct handled results
with `MermanTextMeasureResult.metrics`, `.length`, `.horizontalExtents`, or
`.wrappedWithRawWidth`; each factory validates its required fields. Return `null` when a request
cannot be answered accurately and Merman falls back per request. Callback exceptions are cleared at
the JNI boundary and likewise fall back for that request, but hosts should log them because repeated
fallback can change geometry.

## Immutable Icon Pack Snapshots

`MermanIconPackSet.fromPacks(...)` snapshots complete IconifyJSON collections or host-curated
subsets as one immutable Kotlin value. Install it as `MermanEngineServices.iconPackSet`; the same
pack set can be shared across multiple engine constructors and exposes neither mutation nor
lifecycle methods. Each constructor borrows fresh string arrays for one synchronous native call,
validates and parses the packs transactionally within the fixed limits reported by the runtime
catalog, and returns only after that engine owns the parsed registry. JNI therefore publishes no
separate registry handle. Merman performs no filesystem, package, or network acquisition, so hosts
must acquire and pre-trim packs themselves.

Pack input is untrusted: parsing, retained data, aliases, XML structure, identifier scoping, and
per-operation expansion are bounded. Icon fragments are sanitized under the effective Mermaid
configuration immediately before embedding. This does not make parity/readable SVG safe for direct
browser DOM insertion; use `SafeInlineSvg`, an appropriate CSP, or a sandbox at that boundary.

## Migrating From The Previous Prerelease API

- Replace static one-shot calls on the old `MermanEngine` with `Merman`.
- Delete `MermanReusableEngine` and callback-specialized constructors; construct
  `MermanEngine(optionsJson, services)` directly.
- Put text measurement and icon registries in `MermanEngineServices`; there are no service
  mutators.
- Use typed result metadata and `renderPngResult`, `renderJpegResult`, or `renderPdfResult` when the
  effective output plan matters.
- Treat operation, metadata, resource-limit, option-group, and service IDs from runtime discovery
  as open values. Generated known constants are conveniences, not exhaustive runtime enums.

## Verification

The Android release artifact is built from the descriptor-owned `android-native` profile:

```bash
python3 platforms/android/build-android.py --targets aarch64-linux-android x86_64-linux-android
```

The platform verifier compiles the Kotlin declarations against an Android SDK `android.jar` and
cross-checks both Android target recipes with their exact feature sets. The build and AAR gates
inspect the actual shared objects with the pinned NDK's `llvm-nm`: the Kotlin library must export
`JNI_OnLoad` and must not export `merman_get_native_api`; Flutter's C ABI library enforces the
inverse contract. A connected device or emulator is still required for runtime JNI registration
and callback-exception cleanup:

```bash
python3 scripts/verify-platform-bindings.py --only-android-instrumentation-smoke
```

`platforms/android/examples/MermanSmoke.kt` is intentionally narrow: it proves that the packaged
AAR loads, renders SVG, installs immutable icon and text-measurement services, and closes cleanly.
Owner-local Rust and Kotlin tests carry exhaustive catalog, error, output, protocol, and lifecycle
contracts.
