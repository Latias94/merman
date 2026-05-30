# merman Android JNI

Experimental Android wrapper for `merman-ffi`.

The Kotlin layer loads `libmerman_ffi.so`, checks the native ABI version, then exposes blocking
string APIs for SVG, semantic JSON, and layout JSON. Rendering work should be called from a
background dispatcher in app code.

## Kotlin API

```kotlin
import io.merman.MermanEngine

val svg = MermanEngine.renderSvg("flowchart TD\nA[Hello] --> B[World]")
val semanticJson = MermanEngine.parseJson("flowchart TD\nA[Hello] --> B[World]")
val layoutJson = MermanEngine.layoutJson("flowchart TD\nA[Hello] --> B[World]")
```

Native errors are thrown as `MermanException` with the C ABI JSON error payload as the message.

## Local Verification

```powershell
kotlinc src/main/kotlin/io/merman/MermanException.kt src/main/kotlin/io/merman/MermanEngine.kt -d ../../target/platforms/android/merman-android.jar
rustup target add aarch64-linux-android
cargo check -p merman-ffi --target aarch64-linux-android
```

Standalone Gradle verification with native slices and Gradle 9.x:

```powershell
.\platforms\android\build-android.ps1 -Targets aarch64-linux-android,x86_64-linux-android
& "<gradle-install-dir>\bin\gradle.bat" -p platforms/android assembleRelease
# or, when gradle is already on PATH:
gradle -p platforms/android assembleRelease
```

Full platform gate with Android native slices and AAR assembly:

```powershell
.\scripts\verify-platform-bindings.ps1 -BuildAndroidSlices -RunAndroidGradleBuild -GradlePath "<gradle-install-dir>\bin\gradle.bat"
```

## Build Native Slices

```powershell
.\platforms\android\build-android.ps1 -Targets aarch64-linux-android,x86_64-linux-android
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
