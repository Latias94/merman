# Merman For Android

Parse, analyze, lay out, and render Mermaid diagrams in Android apps without a WebView or a
JavaScript runtime. The Kotlin API uses a direct JNI transport over Merman's shared binding engine.

> **Alpha:** the Android wrapper is not published to Maven Central. Use the repository module or
> an AAR from a matching GitHub release; Kotlin classes and `libmerman_android_jni.so` must come
> from the same release.

## Requirements

- Android API 23 or newer
- Java 17 toolchain
- `arm64-v8a` or `x86_64`
- a release-matched `libmerman_android_jni.so` from the Android native SDK artifact

At load time, the wrapper validates its direct runtime catalog rather than relying on C ABI symbols
or per-method JNI name lookup. A mismatched Kotlin/native pair fails before the first operation.

## Add The Repository Module

Build native slices:

```sh
python3 platforms/android/build-android.py --targets aarch64-linux-android x86_64-linux-android
```

The default `android-native` recipe builds the internal `merman-android-jni` crate. It is reserved
for this Kotlin AAR; Flutter Android packages build `merman-ffi` from a separate C ABI recipe.

Include the module from the host project's `settings.gradle.kts`:

```kotlin
include(":merman-android")
project(":merman-android").projectDir = file("path/to/merman/platforms/android")
```

Then add `implementation(project(":merman-android"))`. Release workflows also attach
`merman-android-<tag>.aar` to GitHub Releases, but no Maven coordinates are currently supported for
remote resolution.

## Render A Diagram

```kotlin
import io.merman.MermanEngine

val source = "flowchart TD\nA[Hello] --> B[World]"
val svg = MermanEngine.renderSvg(source)
check(svg.startsWith("<svg"))
```

Use the generic API when the host chooses output dynamically:

```kotlin
val bytes = MermanEngine.executeBytes("png", source)
```

Convenience methods cover SVG, ASCII, PNG, JPEG, PDF, semantic JSON, layout JSON, validation, and
document analysis. Calls are blocking; invoke substantial work from a background dispatcher.
Native failures throw `MermanException` with a structured Merman error payload.
Use `kind` to distinguish `UNKNOWN_OPERATION` from `MISSING_CAPABILITY`; `capabilityId` is non-null
only for the latter and is the stable descriptor ID.

## Options And Capabilities

`optionsJson` follows the versioned [binding options schema](https://github.com/Latias94/merman/blob/main/docs/bindings/OPTIONS_JSON.md):

```kotlin
val svg = MermanEngine.renderSvg(
    source,
    optionsJson = """{"svg":{"pipeline":"readable"}}""",
)
```

An omitted `runtime_policy` is deterministic even though the normal Android artifact compiles
native adapters. Add `"runtime_policy":"native"` only when an operation should use Android's
clock, time-zone rules, and random source; a custom slim artifact missing an adapter fails with a
typed unsupported-operation error.

Use `MermanEngine.runtimeCatalogJson()` to inspect the loaded artifact's exact capability and
output subset. It returns the flat runtime catalog, including the Android transport API version.
The package's normal release artifact uses the complete native feature set, but hosts should still
query the catalog before exposing optional output choices.

`analyzeJson` and `analyzeDocumentJson` return diagnostics schema `1`; document facts also use
their independently defined schema `1`. These payload schemas are independent of Android transport
version. Pass full Markdown/MDX-like content plus a URI to document analysis:

```kotlin
val factsJson = MermanEngine.analyzeDocumentFactsJson(
    "```mermaid\n$source\n```",
    uri = "file:///workspace/README.md",
)
```

## Reusable Engines And Text Measurement

Use `MermanReusableEngine` when calls share options:

```kotlin
import io.merman.MermanReusableEngine

MermanReusableEngine().use { engine ->
    val svg = engine.renderSvg(source)
}
```

Merman owns a deterministic vendored text measurer by default. Keep it for background jobs, tests,
and content generation. Native Android previews can call `setTextMeasurer` when layout must match
the final `TextPaint`/`StaticLayout` font stack. Return `null` for requests that cannot be answered
faithfully; the engine falls back per request. Reusable calls are serialized and the same engine
must not be called or closed from its measurement callback. Those operations return a typed
reentrant-call error; retry `close()` after the active call returns.

## Verify Locally

```sh
python3 platforms/android/build-android.py --install-missing-ndk --assemble-aar
python3 scripts/verify-platform-bindings.py --build-android-slices
```

The helper uses the checked-in Gradle Wrapper, pinned JDK 17/NDK toolchain, and explicit native SDK
capability profile. Before packaging, NDK `llvm-nm` verifies that the JNI library exports
`JNI_OnLoad` and does not export `merman_get_native_api`. A connected-device JNI smoke is
available through `--only-android-instrumentation-smoke`.

## Documentation And Releases

- [Android JNI transport guide](https://github.com/Latias94/merman/blob/main/docs/bindings/ANDROID_JNI.md)
- [Package changelog](CHANGELOG.md)
- [Diagram coverage](https://github.com/Latias94/merman/blob/main/docs/alignment/STATUS.md)
- [GitHub Releases](https://github.com/Latias94/merman/releases)

The Gradle module defines local Maven publication metadata for release verification; this is not a
claim that `io.merman:merman-android` exists in Maven Central.

## License And Notices

This package is available under MIT or Apache-2.0. See [`LICENSE`](LICENSE),
[`THIRD_PARTY_NOTICES.md`](THIRD_PARTY_NOTICES.md), and [`THIRD_PARTY_LICENSES/`](THIRD_PARTY_LICENSES/).
The AAR also carries these files under `META-INF/`.
