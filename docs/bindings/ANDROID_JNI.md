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
- `MermanEngine.renderAscii(source, optionsJson = null)`
- `MermanEngine.parseJson(source, optionsJson = null)`
- `MermanEngine.layoutJson(source, optionsJson = null)`
- `MermanEngine.validateJson(source, optionsJson = null)`
- `MermanEngine.supportedDiagramsJson()`
- `MermanEngine.asciiSupportedDiagramsJson()`
- `MermanEngine.diagramFamilyCapabilitiesJson()`
- `MermanEngine.supportedThemesJson()`
- `MermanEngine.supportedHostThemePresetsJson()`
- `MermanEngine.packageVersion`
- `MermanException`

The wrapper checks `nativeAbiVersion()` against `MermanEngine.ABI_VERSION` during object
initialization.

## Example

`platforms/android/examples/MermanSmoke.kt` shows the smallest smoke sequence for SVG, ASCII,
semantic JSON, layout JSON, validation JSON, and metadata from Android/Kotlin.

## Verification

```bash
kotlinc platforms/android/src/main/kotlin/io/merman/MermanException.kt platforms/android/src/main/kotlin/io/merman/MermanEngine.kt -d target/platforms/android/merman-android.jar
rustup target add aarch64-linux-android
cargo check -p merman-ffi --target aarch64-linux-android
cargo clippy -p merman-ffi --target aarch64-linux-android -- -D warnings
python3 platforms/android/build-android.py --targets aarch64-linux-android
```

Combined platform gate:

```bash
python3 scripts/verify-platform-bindings.py --build-android-slices
```

To verify the standalone Android library module with native slices and Gradle 9.x:

```bash
python3 platforms/android/build-android.py --targets aarch64-linux-android x86_64-linux-android
"<gradle-install-dir>/bin/gradle" -p platforms/android assembleRelease
```

The platform gate can run the same AAR packaging check after building native slices:

```bash
python3 scripts/verify-platform-bindings.py --build-android-slices --run-android-gradle-build --gradle-path "<gradle-install-dir>/bin/gradle"
```

`--gradle-path` accepts either the Gradle executable path or the Gradle `bin` directory. You can
also set `MERMAN_GRADLE` instead of passing the parameter. Windows users can still run the existing
PowerShell scripts if that is more convenient.

## Follow-On Packaging

- Build every supported Android ABI in CI.
- Add AAR publishing metadata once the release repository target is chosen.
- Add emulator/device smoke once an Android CI target is available.
