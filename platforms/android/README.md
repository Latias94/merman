# Merman For Android

Parse, analyze, lay out, and render Mermaid diagrams in Android apps without a WebView or a JavaScript runtime. The Kotlin API uses a direct JNI transport over Merman's shared binding engine.

> **Alpha:** the Android wrapper is not published to Maven Central. Use the repository module or an AAR from a matching GitHub release; Kotlin classes and `libmerman_android_jni.so` must come from the same release.

## Requirements

- Android API 23 or newer
- Java 17 toolchain
- `arm64-v8a` or `x86_64`

At load time, the wrapper validates the runtime catalog schema, transport API, native package metadata, Options JSON schema, both binding payload schemas, capability implications, callable operation/output/metadata relations, output policies, resource profiles, and the text-measurement protocol rather than relying on C ABI symbols or per-method JNI name lookup. The published Android AAR carries the default native prebuilt SKU: SVG, both layout engines, ASCII, analysis, validation, and document analysis. Every known ID in that generated artifact contract must be present in stable sorted order. Unknown future IDs and fields remain additive and are preserved. The optional option-group and constructor-service sections may be omitted only for legacy schema-1 producers; when present, their known IDs and contracts must match the generated Android contract. Exact package-version equality is not checked, so always ship the Kotlin classes and native library from the same AAR.

## Add A Release AAR

Download `merman-android-<tag>.aar` from the matching [GitHub Release](https://github.com/Latias94/merman/releases), place it in the application's `app/libs/` directory, and add it as a local dependency:

```kotlin
dependencies {
    implementation(files("libs/merman-android-<tag>.aar"))
}
```

The AAR contains the Kotlin API and `libmerman_android_jni.so` for both supported Android ABIs. Do not download or package a second native library beside it.

## Add The Repository Module

Building the repository module requires Python 3, Rust 1.95 with `rustup`, Java 17, Android SDK 35, and the NDK revision pinned in `gradle/libs.versions.toml`. Build both native slices:

```sh
python3 platforms/android/build-android.py --targets aarch64-linux-android x86_64-linux-android
```

The default `android-native` recipe builds the internal `merman-android-jni` crate. It is reserved for this Kotlin AAR; Flutter Android packages build `merman-ffi` from a separate C ABI recipe.

Include the module from the host project's `settings.gradle.kts`:

```kotlin
include(":merman-android")
project(":merman-android").projectDir = file("path/to/merman/platforms/android")
```

Then add `implementation(project(":merman-android"))`. No Maven coordinates are currently supported for remote resolution.

## Render A Diagram

```kotlin
import io.merman.Merman

val source = "flowchart TD\nA[Hello] --> B[World]"
val svg = Merman.renderSvg(source)
check(svg.startsWith("<svg"))
```

Use the generic API when the host chooses an available operation dynamically:

```kotlin
val result = Merman.execute("ascii", source)
check(result.operationId == "ascii")
check(result.mediaType == "text/plain; charset=utf-8")
val text = result.data.toString(Charsets.UTF_8)
```

`Merman` is the discovery and one-shot facade. Convenience methods cover SVG, ASCII, PNG, JPEG, PDF, semantic JSON, layout JSON, analysis facts, SVG planning, validation, and document analysis. The default AAR supports SVG, ASCII, semantic/layout operations, analysis, validation, and document analysis. Math-bearing SVG and PNG, JPEG, or PDF helpers remain available for source-built artifacts that enable those capabilities; the default AAR reports `MISSING_CAPABILITY` with the exact capability ID. `metadataJson(id)` is the generic metadata path for every ID advertised by `runtimeCatalogJson()`. Calls are blocking; invoke substantial work from a background dispatcher. Native failures throw `MermanException` with a structured Merman error payload. Use `kind` to distinguish `UNKNOWN_OPERATION`, `MISSING_CAPABILITY`, `BUSY`, and `REENTRANT_CALL`; `capabilityId` is non-null only for `MISSING_CAPABILITY` and is the stable descriptor ID. Resource failures expose optional typed `resourceDetails` with the stable limit ID, phase, actual value, effective maximum, and selected profile.

Resource failures also expose the stable `cause` discriminator (`ceiling` or `arithmetic_overflow`).

When a custom artifact enables binary exports, `renderPng`, `renderJpeg`, and `renderPdf` return bytes while their corresponding `*Result` methods retain the complete envelope and typed effective output plan. Unknown future output-plan kinds are preserved as `MermanUnknownOutputPlan` together with their raw JSON.

## Options And Capabilities

`optionsJson` follows the versioned [binding options schema](https://github.com/Latias94/merman/blob/main/docs/bindings/OPTIONS_JSON.md):

```kotlin
val svg = Merman.renderSvg(
    source,
    optionsJson = """{"svg":{"pipeline":"readable"}}""",
)
```

`MermanResourceOptionsBuilder` emits Options JSON schema `2`. Its profile is unset by default so reusable request overlays inherit their constructor ceiling; limits accept only `MermanResourceOverrideId`, while `MermanResourceLimitId` describes the full runtime catalog.

The default AAR is deterministic and does not bundle native clock, time-zone, or random adapters. A source build may enable the atomic `native-runtime` feature and then select `"runtime_policy":"native"`; requesting native policy from the default AAR fails with a typed unsupported-operation error. Runtime discovery reports concrete adapter IDs only when the loaded artifact contains them.

Use `Merman.runtimeCatalogJson()` to inspect the loaded artifact's exact options and payload schemas, capability/output/operation/metadata surface, constructor-owned services, and resource-to-operation mappings. Use `Merman.presentationCatalogJson()` for the open-ended theme preset, presentation profile, aspect, and capability-availability catalog instead of maintaining a Kotlin enum. Hosts should query the loaded catalogs before exposing optional choices. A custom Android build that changes the selected SKU must regenerate and package its matching Kotlin artifact contract with the native library.

`analyzeJson` and `analyzeDocumentJson` return diagnostics schema `1`; document facts also use their independently defined schema `1`. These payload schemas are independent of Android transport version. Pass full Markdown/MDX-like content plus a URI to document analysis:

````kotlin
val factsJson = Merman.analyzeDocumentFactsJson(
    "```mermaid\n$source\n```",
    uri = "file:///workspace/README.md",
)
````

## Reusable Engines And Constructor Services

Use `MermanEngine` when calls share options. The engine is constructor-configured and explicitly
closeable; no reusable-engine compatibility class is packaged.

```kotlin
import io.merman.MermanEngine

MermanEngine(optionsJson = """{"svg":{"pipeline":"readable"}}""").use { engine ->
    val svg = engine.renderSvg(source)
}
```

Merman owns a deterministic vendored text measurer by default. Keep it for background jobs, tests, and content generation. Supply a `MermanEngineServices` value when Android layout must match the final `TextPaint`/`StaticLayout` font stack:

```kotlin
val services = MermanEngineServices(
    textMeasurer = MermanTextMeasurer { request ->
        // Return null when this host cannot answer the request faithfully.
        null
    },
)

MermanEngine(services = services).use { engine ->
    engine.renderSvg(source)
}
```

A callback-free engine permits concurrent calls. An engine with a callback rejects a competing call immediately with `BUSY`, and calls or close attempts from its measurement callback return `REENTRANT_CALL`. Construction never invokes the callback. `close()` is idempotent, including concurrent callers. A `BUSY` or `REENTRANT_CALL` close preserves the complete engine for retry after the active call returns.

## Immutable Iconify Pack Inputs

Snapshot IconifyJSON once, then reuse that immutable input across engine constructors:

```kotlin
val iconPackSet = MermanIconPackSet.fromPacks(
    listOf(
        MermanIconPack(
            json = iconifyJson,
            registrationName = "product",
        ),
    ),
)

val services = MermanEngineServices(iconPackSet = iconPackSet)
MermanEngine(services = services).use { first ->
    MermanEngine(services = services).use { second ->
        first.renderSvg("flowchart TD\nA@{ icon: \"product:rocket\" }")
        second.renderSvg("flowchart TD\nB@{ icon: \"product:rocket\" }")
    }
}
```

`MermanIconPackSet.fromPacks` snapshots complete Iconify collections or host-curated subsets and exposes no mutation or lifecycle API. Each `MermanEngine` constructor borrows fresh string arrays, validates and parses them transactionally within the fixed native constructor limits, and returns only after that engine owns the parsed registry. The same Kotlin snapshot can therefore construct multiple independent engines; no native registry handle is shared between them. Merman performs no filesystem, package, or network acquisition—the host must load and, when necessary, pre-trim packs.

Icon bodies are treated as untrusted input and are bounded, XML-validated, deterministically ID-scoped, and sanitized under the effective Mermaid configuration before embedding. This does not make parity/readable SVG safe for direct browser DOM insertion. Use `SafeInlineSvg`, an appropriate CSP, or a sandbox at that boundary, and remember that policy-allowed external references may still cause downstream loaders to perform I/O.

## Verify Locally

```sh
python3 platforms/android/build-android.py --install-missing-ndk --assemble-aar
```

The helper uses the checked-in Gradle Wrapper, pinned JDK 17/NDK toolchain, and exact default native artifact profile. Before packaging, NDK `llvm-nm` verifies that the JNI library exports `JNI_OnLoad` and does not export `merman_get_native_api`. A connected-device JNI smoke is available through `--only-android-instrumentation-smoke`.

## Documentation And Releases

- [Android JNI transport guide](https://github.com/Latias94/merman/blob/main/docs/bindings/ANDROID_JNI.md)
- [Package changelog](CHANGELOG.md)
- [Diagram coverage](https://github.com/Latias94/merman/blob/main/docs/alignment/STATUS.md)
- [GitHub Releases](https://github.com/Latias94/merman/releases)

The Gradle module defines local Maven publication metadata for release verification; this is not a claim that `io.merman:merman-android` exists in Maven Central.

## License And Notices

This package is available under MIT or Apache-2.0. See [`LICENSE`](LICENSE), [`THIRD_PARTY_NOTICES.md`](THIRD_PARTY_NOTICES.md), and [`THIRD_PARTY_LICENSES/`](THIRD_PARTY_LICENSES/). The AAR also carries these files under `META-INF/`.
