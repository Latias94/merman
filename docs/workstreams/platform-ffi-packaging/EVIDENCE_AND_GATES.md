# Platform FFI Packaging - Evidence And Gates

Status: Closed
Last updated: 2026-05-30

## Current Evidence

- 2026-05-30: `PFP-010` opened the platform FFI packaging lane after confirming the core C ABI,
  FFI release hardening, UniFFI, and Python UniFFI package lanes are closed.
- 2026-05-30: `PFP-020` added:
  - `scripts/verify-platform-bindings.py`
  - `scripts/build-python-uniffi-wheel.py`
  - `platforms/android/build-android.py`
  - `platforms/flutter/tool/android-smoke.py`
- 2026-05-30: `python3 -m py_compile scripts/verify-platform-bindings.py scripts/build-python-uniffi-wheel.py platforms/android/build-android.py platforms/flutter/tool/android-smoke.py` passed.
- 2026-05-30: `python3 scripts/verify-platform-bindings.py --help`, `python3 platforms/android/build-android.py --help`, `python3 scripts/build-python-uniffi-wheel.py --help`, and `python3 platforms/flutter/tool/android-smoke.py --help` passed.
- 2026-05-30: `bash scripts/build-apple-xcframework.sh` passed on macOS arm64 and wrote
  `platforms/apple/Merman.xcframework`.
- 2026-05-30: Apple XCFramework inspection found module maps for all three slices:
  `ios-arm64`, `ios-arm64_x86_64-simulator`, and `macos-arm64_x86_64`.
- 2026-05-30: `file platforms/apple/Merman.xcframework/*/*.a` confirmed universal simulator and
  macOS static libraries with `arm64` and `x86_64` slices.
- 2026-05-30: `plutil -p platforms/apple/Merman.xcframework/Info.plist` confirmed iOS device,
  iOS simulator, and macOS libraries.
- 2026-05-30: `swift package describe` passed and reported `MermanFFI` as a binary target and
  `Merman` as the Swift target.
- 2026-05-30: `swift build` passed, compiling `platforms/apple/Sources/Merman/MermanEngine.swift`
  against the generated XCFramework.
- 2026-05-30: `cargo nextest run -p merman-ffi` passed (`15` tests).
- 2026-05-30: `cargo nextest run -p merman-uniffi --features bindgen-smoke --test bindgen_smoke`
  passed (`2` tests).
- 2026-05-30:
  `git diff --check -- .gitignore scripts platforms bindings docs README.md Package.swift` passed
  after closeout doc updates.

## Gate Set

### Script Syntax Gate

```bash
python3 -m py_compile \
  scripts/verify-platform-bindings.py \
  scripts/build-python-uniffi-wheel.py \
  platforms/android/build-android.py \
  platforms/flutter/tool/android-smoke.py
```

### Apple Local Gate

```bash
bash scripts/build-apple-xcframework.sh
swift package describe
find platforms/apple/Merman.xcframework -name module.modulemap -print
```

### Focused Binding Gate

```bash
cargo nextest run -p merman-ffi
cargo nextest run -p merman-uniffi --features bindgen-smoke --test bindgen_smoke
```

### Diff Hygiene Gate

```bash
git diff --check -- scripts platforms bindings docs README.md Package.swift
```

## Evidence Anchors

- `scripts/verify-platform-bindings.py`
- `scripts/build-python-uniffi-wheel.py`
- `platforms/android/build-android.py`
- `platforms/flutter/tool/android-smoke.py`
- `scripts/build-apple-xcframework.sh`
- `Package.swift`
- `platforms/apple/Sources/Merman/MermanEngine.swift`

## Notes

Full Android/Flutter package gates require external tools (`ANDROID_HOME` or `ANDROID_NDK_HOME`,
`kotlinc`, Gradle, Flutter). When those tools are absent locally, record the skipped command and the
missing prerequisite instead of weakening the scripts.

Closeout note: full Android/Flutter packaging was not run in this lane because the deliverable was
cross-platform script parity plus Apple local verification, and this host did not have `kotlinc` or
`gradle` on `PATH` during closeout. The Python scripts preserve those gates for hosts with the full
Android/Flutter toolchain installed.
