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
Kotlin MermanEngine / MermanReusableEngine / MermanOperationResult
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
val result = MermanEngine.execute(
    operationId = "svg",
    source = "flowchart TD\nA --> B",
)
val svg = result.data.toString(Charsets.UTF_8)
```

`MermanOperationResult` contains `operationId`, `mediaType`, `data`, and `metadataJson`. `MermanEngine` also exposes convenience methods for SVG, ASCII, PNG, JPEG, PDF, semantic JSON, layout JSON, analysis JSON, document analysis, facts, and validation. Binary methods return `ByteArray`; JSON/SVG/ASCII methods decode the output as UTF-8.

Use `MermanReusableEngine(optionsJson, textMeasurer)` when calls share base options. Its `execute` method accepts per-call `optionsJson` overlays and uses the same operation IDs while retaining a native engine for its lifetime. `textMeasurer` is immutable after construction. A callback-free reusable engine accepts concurrent calls. An engine with a callback rejects a competing operation immediately with `BUSY`; a call or close from its callback returns `REENTRANT_CALL`. `close()` is nonblocking and retains the Kotlin handle when it fails, so callers can retry after the active operation returns.

## Runtime Catalog

During initialization, Kotlin loads `libmerman_android_jni.so`, requests the direct catalog, and
validates:

- flat runtime-catalog schema `1`;
- Android transport API version `1`;
- sorted, unique capability, output, operation, and adapter IDs;
- each runtime ID against that vocabulary; and
- text-measurement/provider consistency and resource descriptors.

Read the validated catalog with `MermanEngine.runtimeCatalogJson()`:

```json
{
  "schema_version": 1,
  "transport_api_version": 1,
  "package_version": "0.8.0-alpha.4",
  "capabilities": {
    "capability_ids": ["svg"],
    "operation_ids": ["svg"],
    "output_ids": ["svg"],
    "system_adapter_ids": [],
    "text_measurement": { "protocol_version": 1, "provider_ids": ["vendored"] }
  },
  "registry": { "diagram_family_count": 35 },
  "resources": { "general_binding_default_profile": "interactive" }
}
```

This is the boundary used to decide whether an installed AAR can render a requested output. Do not
infer support from Kotlin method presence or Cargo feature names. A missing output reports a typed
`MermanException` instead of silently selecting a fallback format.

`MermanException.kind` is `UNKNOWN_OPERATION`, `MISSING_CAPABILITY`, `BUSY`, `REENTRANT_CALL`, or `GENERIC`.
`capabilityId` is non-null only for `MISSING_CAPABILITY` and preserves the descriptor ID emitted by
bindings-core. Local wrapper and lifecycle failures remain `GENERIC`; consumers should branch on
these fields rather than parse `message`.

## Text Measurement

Pass a synchronous `MermanTextMeasurer` to `MermanReusableEngine` at construction only when Android text geometry must match the final surface. Native previews should measure with the same `TextPaint`/`StaticLayout` configuration used for display; WebView previews should use a prepared DOM/canvas cache rather than block a render thread on UI work.

The independent text-measurement protocol has 19 operations (`0..18`). Construct handled results
with `MermanTextMeasureResult.metrics`, `.length`, `.horizontalExtents`, or
`.wrappedWithRawWidth`; each factory validates its required fields. Return `null` when a request
cannot be answered accurately and Merman falls back per request. Callback exceptions are cleared at
the JNI boundary and likewise fall back for that request, but hosts should log them because repeated
fallback can change geometry.

## Verification

The Android release artifact is built from the descriptor-owned `android-native` profile:

```bash
python3 platforms/android/build-android.py --targets aarch64-linux-android x86_64-linux-android
python3 scripts/verify-platform-bindings.py --build-android-slices
```

The platform verifier compiles the Kotlin declarations against an Android SDK `android.jar` and
cross-checks both Android target recipes with their complete feature sets. The build and AAR gates
inspect the actual shared objects with the pinned NDK's `llvm-nm`: the Kotlin library must export
`JNI_OnLoad` and must not export `merman_get_native_api`; Flutter's C ABI library enforces the
inverse contract. A connected device or emulator is still required for runtime JNI registration
and callback-exception cleanup:

```bash
python3 scripts/verify-platform-bindings.py --only-android-instrumentation-smoke
```

`platforms/android/examples/MermanSmoke.kt` covers runtime catalog validation, generic output
calls, reusable-engine lifecycle, and the text-measurement operation contract.
