# Merman For Android

Parse, analyze, lay out, and render Mermaid diagrams in Android apps without a WebView or a JavaScript runtime. The Kotlin API uses a direct JNI transport over Merman's shared binding engine.

This guide targets the `0.10.0-alpha.1` candidate; see the [host guide](../../docs/release/PREBUILT_PRODUCTS.md) for version and channel availability.

> **Binding contract:** The `0.10.0-alpha.1` AAR uses Options JSON schema `3`. Kotlin classes
> and `libmerman_android_jni.so` must come from the same publication. The candidate Maven
> repository is a qualification artifact; no Maven Central namespace or published coordinate
> is claimed here.

## Requirements

- Android API 23 or newer
- Java 17 toolchain
- `arm64-v8a` or `x86_64`

At load time, the wrapper validates the runtime catalog schema, transport API, native package metadata, Options JSON schema, both binding payload schemas, capability implications, callable operation/output/metadata relations, output policies, resource profiles, and the text-measurement protocol rather than relying on C ABI symbols or per-method JNI name lookup. The default Android AAR carries SVG with math, Cytoscape and ELK layout, ASCII, analysis, validation, and document analysis. Every known ID in that generated artifact contract must be present in stable sorted order. Unknown future IDs and fields remain additive and are preserved. The optional option-group and constructor-service sections may be omitted only for legacy schema-1 producers; when present, their known IDs and contracts must match the generated Android contract. Exact package-version equality is not checked, so always ship the Kotlin classes and native library from the same AAR.

## Resolve A Candidate Maven Publication

Extract `merman-android-0.10.0-alpha.1-maven.zip` from the candidate workflow artifacts into
a local directory. The ZIP is a complete Maven repository for exactly the selected version:
AAR, POM, Gradle module metadata, sources,
API documentation, available detached signatures, and checksum sidecars. Other staged versions
are excluded, and repository metadata lists only the selected version. The POM and Gradle
metadata retain transitive dependencies, so a Maven dependency also resolves the Kotlin runtime.

Add the extracted repository in the application's `settings.gradle.kts`:

```kotlin
dependencyResolutionManagement {
    repositories {
        google()
        mavenCentral()
        exclusiveContent {
            forRepository {
                maven { url = uri(providers.gradleProperty("mermanRepository").get()) }
            }
            filter {
                includeModule(providers.gradleProperty("mermanGroup").get(), "merman-android")
            }
        }
    }
}
```

Declare the exact selected version in `app/build.gradle.kts`:

```kotlin
dependencies {
    val group = providers.gradleProperty("mermanGroup").get()
    val version = providers.gradleProperty("mermanVersion").get()
    implementation("$group:merman-android:$version")
}
```

Pass `-PmermanRepository=/absolute/path/to/extracted/repository`,
`-PmermanGroup=<selected-group>` and `-PmermanVersion=0.10.0-alpha.1` to Gradle. Use the group
recorded by the candidate owner. Central installation requires a separately verified namespace,
publication and public resolution of that exact coordinate. `io.merman` remains a local build
default; it is not evidence of Central ownership.

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

Use `MermanOperationControl` when a host must cancel stale work or apply a relative deadline:

```kotlin
MermanOperationControl(timeoutMs = 250).use { control ->
    val result = Merman.execute(
        operationId = "svg",
        source = source,
        control = control,
    )
}
```

The timeout installs a monotonic deadline relative to control construction. Retain the same
control on another thread and call `cancel()` while `execute` is running; cancellation is
cooperative, so an opaque callback may finish before the next native checkpoint. `isCancelled()`
reports an explicit cancellation request, while deadline expiry is reported by execution.
`close()` and `release()` are equivalent and idempotent. Releasing the Kotlin token does not stop
an operation that already cloned the control, but using the released object for a new operation is
a typed invalid-argument failure.

`Merman` is the discovery and one-shot facade. Convenience methods cover SVG, ASCII, PNG, JPEG, PDF, semantic JSON, layout JSON, analysis facts, SVG planning, validation, and document analysis. The default AAR supports SVG with math, ASCII, semantic/layout operations, analysis, validation, and document analysis. PNG, JPEG and PDF remain source-build opt-ins; the default AAR reports `MISSING_CAPABILITY` with the exact codec capability ID. The `constrained` resource profile continues to reject the native math backend even when the artifact includes math; use the default or `interactive` profile for formulas. `metadataJson(id)` is the generic metadata path for every ID advertised by `runtimeCatalogJson()`. Calls are blocking; invoke substantial work from a background dispatcher. Native failures throw `MermanException` with a structured Merman error payload. Use `kind` to distinguish `UNKNOWN_OPERATION`, `MISSING_CAPABILITY`, `BUSY`, and `REENTRANT_CALL`; `capabilityId` is non-null only for `MISSING_CAPABILITY` and is the stable descriptor ID.

Resource failures always expose lossless `exactResourceDetails`; its `actual` and `max` fields are
canonical unsigned decimal strings covering the complete native `u64` range. The existing
`resourceDetails` API remains a signed-`Long` compatibility projection and is `null` when either
count exceeds `Long.MAX_VALUE`. Existing consumers can keep using `resourceDetails`; consumers that
inspect arithmetic-overflow counts should migrate to `exactResourceDetails`.

Cooperative cancellation and deadline failures expose `cancellationDetails` with the stable
`reason` (`requested` or `deadline_exceeded`) and observed checkpoint `phase`. They publish no
partial `MermanOperationResult` and remain separate from `resourceDetails`.

Resource failures also expose the stable `cause` discriminator (`ceiling` or `arithmetic_overflow`).

When a custom artifact enables binary exports, `renderPng`, `renderJpeg`, and `renderPdf` return bytes while their corresponding `*Result` methods retain the complete envelope and typed effective output plan. Unknown future output-plan kinds are preserved as `MermanUnknownOutputPlan` together with their raw JSON.

## Options And Capabilities

`optionsJson` follows [Options JSON schema `3`](https://github.com/Latias94/merman/blob/main/docs/bindings/OPTIONS_JSON.md):

```kotlin
val svg = Merman.renderSvg(
    source,
    optionsJson = """{"svg":{"pipeline":"readable"}}""",
)
```

`MermanResourceOptionsBuilder` emits Options JSON schema `3`. Its profile is unset by default so reusable request overlays inherit their constructor ceiling; limits accept only `MermanResourceOverrideId`, while `MermanResourceLimitId` describes the full runtime catalog.

The default AAR is deterministic and does not bundle native clock, time-zone, or random adapters. A source build may enable the atomic `native-runtime` feature and then select `"runtime_policy":"native"`; requesting native policy from the default AAR fails with a typed unsupported-operation error. Runtime discovery reports concrete adapter IDs only when the loaded artifact contains them.

Use `Merman.runtimeCatalogJson()` to inspect the loaded artifact's exact options and payload schemas, capability/output/operation/metadata surface, constructor-owned services, and resource-to-operation mappings. Use `Merman.themeCatalogJson()` for open-ended compiled-theme presets, capabilities, font containers, semantic targets, and variants instead of maintaining Kotlin enums. Hosts should query the loaded catalogs before exposing optional choices. A custom Android build that changes the selected SKU must regenerate and package its matching Kotlin artifact contract with the native library.

`analyzeJson` and `analyzeDocumentJson` return diagnostics schema `1`; document facts use their independently defined schema `2` and contain generic parser/editor facts only. These payload schemas are independent of Android transport version. Pass full Markdown/MDX-like content plus a URI to document analysis:

````kotlin
val factsJson = Merman.analyzeDocumentFactsJson(
    "```mermaid\n$source\n```",
    uri = "file:///workspace/README.md",
)
````

## Diagram Themes

Use `Merman.themeCatalogJson()` to discover presets, then select one through schema-3 options:

```kotlin
val pieSource = "pie\n\"A\":35\n\"B\":65"
val themedSvg = Merman.renderSvg(
    pieSource,
    optionsJson = """{"version":3,"theme":{"preset":"ghibli"}}""",
)
```

For a reusable engine, omit `theme` to inherit its selection, use `theme: null` to clear it,
or supply a complete selection or recipe to replace it.

Built-in presets are alpha, with empty public qualification cells. Catalog availability and
family design scope do not certify the result for every diagram or output target. Flowchart
glow and shadow effects require Classic look (`site_config.flowchart.look: "classic"`);
selecting a preset does not change the look or layout engine. Fonts must be supplied by the
host and final SVG viewer; font-stack names do not embed fonts, and caller-provided font
resources in recipes are rejected. This Kotlin AAR does not expose the native C/WASI
memory-font constructor. See the [theme guide](../../docs/rendering/custom-diagram-themes.md)
for version-1 recipes, support discovery, and portability policy.

## Reusable Engines And Constructor Services

Use `MermanEngine` when calls share options. The engine is constructor-configured and explicitly
closeable; no reusable-engine compatibility class is packaged.

```kotlin
import io.merman.MermanEngine

MermanEngine(optionsJson = """{"svg":{"pipeline":"readable"}}""").use { engine ->
    val svg = engine.renderSvg(source)
}
```

`MermanEngine.execute` accepts the same controlled overload as the one-shot facade. The native
transport clones the control before engine admission and never holds the operation-control registry
lock while parsing, analyzing, laying out, rendering, or invoking a host callback.

Merman owns a deterministic, font-agnostic text measurer by default. Keep it for background jobs, tests, and content generation. Supply a `MermanEngineServices` value when Android layout must match the final `TextPaint`/`StaticLayout` font stack:

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

## Add The Repository Module

Building the repository module requires Python 3, Rust 1.95 with `rustup`, Java 17, Android SDK 35, and the NDK revision pinned in `gradle/libs.versions.toml`. Use the matching `0.10.0-alpha.1` source revision and build both native slices:

```sh
python3 platforms/android/build-android.py --targets aarch64-linux-android x86_64-linux-android
```

The default `android-native` recipe builds the internal `merman-android-jni` crate. It is reserved for this Kotlin AAR; Flutter Android packages build `merman-ffi` from a separate C ABI recipe.

Include the module from the host project's `settings.gradle.kts`:

```kotlin
include(":merman-android")
project(":merman-android").projectDir = file("path/to/merman/platforms/android")
```

Then add `implementation(project(":merman-android"))`.

## Verify Locally

```sh
python3 platforms/android/build-android.py --install-missing-ndk --assemble-aar
```

The helper uses the checked-in Gradle Wrapper, pinned JDK 17/NDK toolchain, and exact default native artifact profile. Before packaging, NDK `llvm-nm` verifies that the JNI library exports `JNI_OnLoad` and does not export `merman_get_native_api`. A connected-device JNI smoke is available through `--only-android-instrumentation-smoke`.

<a id="add-a-release-aar"></a>

## Legacy: Add A Release AAR

The published `0.9.2` channel uses `merman-android-v0.9.2.aar` from the matching
[GitHub Release](https://github.com/Latias94/merman/releases/tag/v0.9.2). Place it in the
application's `app/libs/` directory and add it as a local dependency:

```kotlin
dependencies {
    implementation(files("libs/merman-android-v0.9.2.aar"))
}
```

The AAR contains the Kotlin API and `libmerman_android_jni.so` for both supported Android ABIs.
Do not package a second native library beside it. This legacy release uses Options JSON
schema `2` and omits math and binary codecs. Follow its
[tagged guide](https://github.com/Latias94/merman/blob/v0.9.2/platforms/android/README.md) for API
examples; the schema-3 theme examples in this README require the candidate's matching AAR.

## Documentation And Releases

- [Android JNI transport guide](https://github.com/Latias94/merman/blob/main/docs/bindings/ANDROID_JNI.md)
- [Package changelog](CHANGELOG.md)
- [Diagram coverage](https://github.com/Latias94/merman/blob/main/docs/alignment/STATUS.md)
- [GitHub Releases](https://github.com/Latias94/merman/releases)

The Android release owner and qualification commands are documented in
[`RELEASING.md`](RELEASING.md). They prepare local and CI candidates without publishing by
default. Central upload remains a separate, authorized operator action.

## License And Notices

This package is available under MIT or Apache-2.0. See [`LICENSE`](LICENSE), [`THIRD_PARTY_NOTICES.md`](THIRD_PARTY_NOTICES.md), and [`THIRD_PARTY_LICENSES/`](THIRD_PARTY_LICENSES/). The AAR also carries these files under `META-INF/`.
