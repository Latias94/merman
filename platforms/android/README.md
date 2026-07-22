# Merman For Android

[![License: MIT](https://img.shields.io/badge/license-MIT-yellow)](https://github.com/Latias94/merman/blob/main/LICENSE-MIT)
[![License: Apache-2.0](https://img.shields.io/badge/license-Apache--2.0-blue)](https://github.com/Latias94/merman/blob/main/LICENSE-APACHE)

Parse, analyze, lay out, and render Mermaid diagrams in Android apps without a WebView or JavaScript runtime. The Kotlin API calls Merman's Rust engine through JNI and returns SVG, Unicode terminal output, or structured JSON.

> **Alpha:** the Android wrapper is not published to Maven Central. Use the repository module or an AAR attached to a matching GitHub release, and never mix Kotlin classes with a native library from another release.

## Requirements

- Android API 23 or newer
- Java 17 toolchain
- `arm64-v8a` or `x86_64`
- C ABI `2` native library packaged as `libmerman_ffi.so`

The wrapper checks the ABI and exposed struct sizes before the first call. ABI 2 records can still be replaced in place during alpha development, so those checks do not make mixed prerelease artifacts supported.

## Add The Repository Module

Build native slices:

```sh
python3 platforms/android/build-android.py --targets aarch64-linux-android x86_64-linux-android
```

Include the module from the host project's `settings.gradle.kts`:

```kotlin
include(":merman-android")
project(":merman-android").projectDir = file("path/to/merman/platforms/android")
```

Then add `implementation(project(":merman-android"))`. Release workflows also attach `merman-android-<tag>.aar` to GitHub Releases, but no Maven coordinates are currently supported for remote resolution.

## Render A Diagram

```kotlin
import io.merman.MermanEngine

val source = "flowchart TD\nA[Hello] --> B[World]"
val svg = MermanEngine.renderSvg(source)
check(svg.startsWith("<svg"))
```

`MermanEngine` also exposes terminal rendering, semantic and layout JSON, validation, diagram/document analysis, parser facts, themes, lint metadata, ASCII support grades, and diagram-family capability JSON.

Calls are blocking. Invoke non-trivial rendering from a background dispatcher rather than the main thread. Native failures throw `MermanException` with the structured C ABI error payload as the message.

## Options And Analysis

`optionsJson` follows the versioned [binding options schema](https://github.com/Latias94/merman/blob/main/docs/bindings/OPTIONS_JSON.md):

```kotlin
val svg = MermanEngine.renderSvg(
    source,
    optionsJson = """{"svg":{"pipeline":"readable"}}""",
)
```

`analyzeJson` and `analyzeDocumentJson` return diagnostics schema `1`; `analyzeDocumentFactsJson` returns parser-backed facts schema `2` and rejects facts v1 at its version boundary. These schema versions are independent of native ABI `2`. The removed TextScan alpha facts shape is not retained.

Pass full Markdown/MDX-like content and a URI to document analysis:

```kotlin
val factsJson = MermanEngine.analyzeDocumentFactsJson(
    "```mermaid\n$source\n```",
    uri = "file:///workspace/README.md",
)
```

Use `diagramFamilyCapabilitiesJson()` and `asciiCapabilitiesJson()` instead of hard-coding support for a build profile or output format.
Use `runtimeContractJson()` for the loaded ABI/package/options versions, feature set, registry
facts, and exact resource profile values. The returned JSON uses runtime-contract schema `3`.
Choose a profile from the shared [resource decision table](https://github.com/Latias94/merman/blob/main/docs/bindings/OPTIONS_JSON.md#resource-options), then pass the generated options JSON:

```kotlin
val resourceOptions = MermanResourceOptionsBuilder()
    .profile(MermanResourceProfile.CONSTRAINED)
    .build()
val svg = MermanEngine.renderSvg(source, resourceOptions.toOptionsJson())
```

Use `CONSTRAINED` for untrusted, public, or multi-tenant input; `INTERACTIVE` is for cooperative
local editing. The native CLI's default is intentionally separate (`trusted-native`).

## Reusable Engines And Text Measurement

Use `MermanReusableEngine` for calls that share options. It is `AutoCloseable`:

```kotlin
import io.merman.MermanReusableEngine

MermanReusableEngine().use { engine ->
    val svg = engine.renderSvg(source)
}
```

Merman owns a deterministic vendored text measurer by default. Keep it for background jobs, tests, and content generation. Native Android previews can call `setTextMeasurer` when layout must match the final `TextPaint`/`StaticLayout` font stack; WebView previews should use synchronously cached DOM/canvas measurements.

ABI 2 exposes 19 exact operations (`0..18`). Construct handled results with
`MermanTextMeasureResult.metrics`, `.length`, `.horizontalExtents`, or
`.wrappedWithRawWidth`; each factory requires and validates the fields used by that result shape.
Return `null` when an operation cannot be answered immediately and faithfully. Wrong-kind results
and callback exceptions fall back for that operation.

Reusable engine calls are serialized. Do not call the same engine from its measurement callback. Calling `close()` during a callback safely defers native release until the call returns. See the [host measurement guide](https://github.com/Latias94/merman/blob/main/docs/bindings/HOST_TEXT_MEASUREMENT.md#android-jni) for operation shapes and lifecycle rules.

## Verify Locally

```sh
python3 platforms/android/build-android.py --install-missing-ndk --assemble-aar
```

This command installs only the pinned NDK when needed, builds both published native ABIs, uses the
checked-in Gradle Wrapper, and verifies the completed AAR. Existing Android projects that consume a
release AAR do not need Rust, the NDK, or a separate Gradle installation.
The helper discovers an installed JDK 17 without changing the shell's active Java; pass
`--java-home <path>` only when automatic discovery cannot find a nonstandard installation.

The connected-device JNI smoke is available through `--only-android-instrumentation-smoke`. [`examples/MermanSmoke.kt`](examples/MermanSmoke.kt) exercises the complete binding contract.

## Documentation And Releases

- [Android JNI guide](https://github.com/Latias94/merman/blob/main/docs/bindings/ANDROID_JNI.md)
- [Package changelog](CHANGELOG.md)
- [Diagram coverage](https://github.com/Latias94/merman/blob/main/docs/alignment/STATUS.md)
- [GitHub Releases](https://github.com/Latias94/merman/releases)
- [Issue tracker](https://github.com/Latias94/merman/issues)

The Gradle module defines local Maven publication metadata for release verification, but this is not a claim that `io.merman:merman-android` exists in Maven Central.

## License And Notices

This package is available under MIT or Apache-2.0. See [`LICENSE`](LICENSE),
[`THIRD_PARTY_NOTICES.md`](THIRD_PARTY_NOTICES.md), and
[`THIRD_PARTY_LICENSES/`](THIRD_PARTY_LICENSES/). The AAR also carries these files under
`META-INF/`.
