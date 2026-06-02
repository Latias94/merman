# merman Android JNI

Experimental Android wrapper for `merman-ffi`.

The Kotlin layer loads `libmerman_ffi.so`, checks the native ABI version, then exposes blocking
string APIs for SVG, ASCII text, semantic JSON, layout JSON, validation JSON, and metadata.
Rendering work should be called from a background dispatcher in app code.

## Kotlin API

```kotlin
import io.merman.MermanEngine
import io.merman.MermanException

val source = "flowchart TD\nA[Hello] --> B[World]"
val version = MermanEngine.packageVersion

val svg = MermanEngine.renderSvg(
    source,
    optionsJson = """{"svg":{"pipeline":"readable"}}""",
)
val semanticJson = MermanEngine.parseJson(source)
val layoutJson = MermanEngine.layoutJson(source)
val ascii = MermanEngine.renderAscii(source)
val validationJson = MermanEngine.validateJson(source)
val diagramsJson = MermanEngine.supportedDiagramsJson()
val themesJson = MermanEngine.themesJson()

try {
    MermanEngine.renderSvg(source, optionsJson = "{")
} catch (error: MermanException) {
    println(error.message)
}
```

Native errors are thrown as `MermanException` with the C ABI JSON error payload as the message.
`MermanEngine` checks the loaded native ABI before the first call and exposes the linked native
package version through `packageVersion`.
`optionsJson` follows the shared schema in
[`docs/bindings/OPTIONS_JSON.md`](../../docs/bindings/OPTIONS_JSON.md).

## Example

[`examples/MermanSmoke.kt`](examples/MermanSmoke.kt) shows the smallest Android-side smoke call
sequence. Use it from an Android app or instrumentation test after packaging
`libmerman_ffi.so` into the app.

## Local Verification

```bash
kotlinc src/main/kotlin/io/merman/MermanException.kt src/main/kotlin/io/merman/MermanEngine.kt -d ../../target/platforms/android/merman-android.jar
rustup target add aarch64-linux-android
cargo check -p merman-ffi --target aarch64-linux-android
```

Standalone Gradle verification with native slices and Gradle 9.x:

```bash
python3 platforms/android/build-android.py --targets aarch64-linux-android x86_64-linux-android
gradle -p platforms/android assembleRelease
# or, with an explicit Gradle install:
"<gradle-install-dir>/bin/gradle" -p platforms/android assembleRelease
```

On Windows, the existing PowerShell entry point remains available:

```powershell
.\platforms\android\build-android.ps1 -Targets aarch64-linux-android,x86_64-linux-android
gradle -p platforms/android assembleRelease
```

Full platform gate with Android native slices and AAR assembly:

```bash
python3 scripts/verify-platform-bindings.py --build-android-slices --run-android-gradle-build --gradle-path "<gradle-install-dir>/bin/gradle"
```

## Build Native Slices

```bash
python3 platforms/android/build-android.py --targets aarch64-linux-android x86_64-linux-android
```

This copies libraries into:

```text
platforms/android/src/main/jniLibs/{arm64-v8a,x86_64}/libmerman_ffi.so
```

`jniLibs` is generated output and is ignored by git.

## Gradle Module

`platforms/android` is an Android library module. In a host app:

```kotlin
include(":merman-android")
project(":merman-android").projectDir = file("path/to/merman/platforms/android")
```

Then depend on `implementation(project(":merman-android"))`.

The release workflow currently uploads an AAR to GitHub Releases. Maven Central publishing still
needs final coordinates, signing, and POM metadata.
