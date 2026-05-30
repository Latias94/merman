# Android JNI Wrapper

Status: experimental platform wrapper.

`platforms/android` provides a thin Kotlin wrapper over Android-only JNI exports from
`merman-ffi`.

## Layers

```text
Kotlin MermanEngine
        |
        v
JNI symbols in merman-ffi (target_os = android)
        |
        v
merman-bindings-core
```

The native library name is `merman_ffi`, so Android packages should include ABI-specific
`libmerman_ffi.so` files under `jniLibs`.

## Kotlin Surface

- `MermanEngine.renderSvg(source, optionsJson = null)`
- `MermanEngine.parseJson(source, optionsJson = null)`
- `MermanEngine.layoutJson(source, optionsJson = null)`
- `MermanEngine.packageVersion`
- `MermanException`

The wrapper checks `nativeAbiVersion()` against `MermanEngine.ABI_VERSION` during object
initialization.

## Verification

```powershell
kotlinc platforms/android/src/main/kotlin/io/merman/MermanException.kt platforms/android/src/main/kotlin/io/merman/MermanEngine.kt -d target/platforms/android/merman-android.jar
rustup target add aarch64-linux-android
cargo check -p merman-ffi --target aarch64-linux-android
```

## Follow-On Packaging

- Add Gradle Android library metadata.
- Build Android `.so` slices with cargo-ndk or explicit NDK linker config.
- Package an AAR with `src/main/jniLibs`.
- Add emulator/device smoke once an Android CI target is available.
