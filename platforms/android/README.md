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

Producing Android `.so` artifacts and an AAR with Gradle/cargo-ndk is follow-on work.
