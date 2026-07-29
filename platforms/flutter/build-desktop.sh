#!/usr/bin/env bash
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
FLUTTER_ROOT="$REPO_ROOT/platforms/flutter"
AUTO_INSTALL_RUST_TARGETS="${MERMAN_AUTO_INSTALL_RUST_TARGETS:-auto}"
MACOS_XCFRAMEWORK_OUT="$FLUTTER_ROOT/macos/MermanFFI.xcframework"
FFI_INCLUDE_DIR="$REPO_ROOT/crates/merman-ffi/include"
RECIPE_PROFILE="flutter-desktop-native"

recipe_field() {
    python3 "$REPO_ROOT/scripts/artifact_profile_recipe.py" "$RECIPE_PROFILE" --field "$1"
}

NATIVE_SDK_PROFILE="$(recipe_field profile)"
NATIVE_SDK_TARGET="$(recipe_field target)"
NATIVE_SDK_LIBRARY_STEM="${NATIVE_SDK_TARGET//-/_}"

MODE="host"

for ARG in "$@"; do
    case "$ARG" in
        --host) MODE="host" ;;
        --all) MODE="all" ;;
        *) echo "unknown argument: $ARG" >&2; exit 2 ;;
    esac
done

if [[ "${MERMAN_CHECK_RECIPE_ONLY:-false}" == "true" ]]; then
    exit 0
fi

require_tool() {
    if ! command -v "$1" >/dev/null 2>&1; then
        echo "required tool not found: $1" >&2
        exit 1
    fi
}

resolve_llvm_nm() {
    local candidate
    local rust_host
    local rust_sysroot

    if [[ -n "${MERMAN_LLVM_NM:-}" ]]; then
        if [[ -f "$MERMAN_LLVM_NM" ]]; then
            echo "$MERMAN_LLVM_NM"
            return
        fi
        if command -v "$MERMAN_LLVM_NM" >/dev/null 2>&1; then
            command -v "$MERMAN_LLVM_NM"
            return
        fi
        echo "MERMAN_LLVM_NM does not resolve to a file: $MERMAN_LLVM_NM" >&2
        exit 1
    fi

    if command -v llvm-nm >/dev/null 2>&1; then
        command -v llvm-nm
        return
    fi

    if command -v xcrun >/dev/null 2>&1; then
        candidate="$(xcrun --find llvm-nm 2>/dev/null || true)"
        if [[ -f "$candidate" ]]; then
            echo "$candidate"
            return
        fi
    fi

    rust_host="$(rustc -vV | sed -n 's/^host: //p')"
    rust_sysroot="$(rustc --print sysroot)"
    for candidate in \
        "$rust_sysroot/lib/rustlib/$rust_host/bin/llvm-nm" \
        "$rust_sysroot/lib/rustlib/$rust_host/bin/llvm-nm.exe"; do
        if [[ -f "$candidate" ]]; then
            echo "$candidate"
            return
        fi
    done

    echo "required tool not found: llvm-nm (set MERMAN_LLVM_NM or install llvm-tools-preview)" >&2
    exit 1
}

verify_dynamic_c_abi() {
    local library="$1"
    python3 "$REPO_ROOT/scripts/native_symbol_contract.py" --contract c-abi \
        --llvm-nm "$LLVM_NM" \
        --label "$RECIPE_PROFILE $library" \
        "$library"
}

verify_macho_c_abi() {
    local library="$1"
    python3 "$REPO_ROOT/scripts/native_symbol_contract.py" --contract c-abi \
        --llvm-nm "$LLVM_NM" \
        --all-macho-architectures \
        --label "$RECIPE_PROFILE $library" \
        "$library"
}

verify_windows_dll_c_abi() {
    local built_library="$1"
    local import_library="$2"
    if [[ ! -f "$built_library" ]]; then
        echo "Windows DLL does not exist: $built_library" >&2
        exit 1
    fi
    # LLVM nm cannot read a PE export table; Cargo's import library lists the same exports.
    python3 "$REPO_ROOT/scripts/native_symbol_contract.py" --contract c-abi \
        --llvm-nm "$LLVM_NM" \
        --external-only \
        --label "$RECIPE_PROFILE $built_library (via $import_library)" \
        "$import_library"
}

ensure_rust_target_installed() {
    local target="$1"
    local installed_targets
    installed_targets="$(rustup target list --installed)"
    if [[ "
$installed_targets
" == *"
$target
"* ]]; then
        return
    fi

    local should_auto_install=false
    case "$AUTO_INSTALL_RUST_TARGETS" in
        true) should_auto_install=true ;;
        false) should_auto_install=false ;;
        auto)
            if [[ -z "${CI-}" ]]; then
                should_auto_install=true
            fi
            ;;
        *)
            echo "unknown MERMAN_AUTO_INSTALL_RUST_TARGETS value: $AUTO_INSTALL_RUST_TARGETS" >&2
            exit 2
            ;;
    esac

    if "$should_auto_install"; then
        echo "==> Installing Rust target $target"
        rustup target add "$target"
        return
    fi

    echo "missing Rust target: $target" >&2
    echo "install it first: rustup target add $target" >&2
    exit 1
}

host_arch() {
    case "$(uname -m)" in
        x86_64|amd64) echo "x86_64" ;;
        arm64|aarch64) echo "aarch64" ;;
        *) echo "unsupported host architecture: $(uname -m)" >&2; exit 1 ;;
    esac
}

host_rust_target() {
    local system
    local arch
    system="$(uname -s)"
    arch="$(host_arch)"
    case "$system:$arch" in
        Darwin:aarch64) echo "aarch64-apple-darwin" ;;
        Darwin:x86_64) echo "x86_64-apple-darwin" ;;
        Linux:aarch64) echo "aarch64-unknown-linux-gnu" ;;
        Linux:x86_64) echo "x86_64-unknown-linux-gnu" ;;
        MINGW*:x86_64|MSYS*:x86_64|CYGWIN*:x86_64) echo "x86_64-pc-windows-gnu" ;;
        *) echo "unsupported host platform: $system ($arch)" >&2; exit 1 ;;
    esac
}

verify_public_headers() {
    local headers_dir="$1"
    local public_header
    for public_header in "$FFI_INCLUDE_DIR"/*.h; do
        test -f "$headers_dir/$(basename "$public_header")"
    done
    printf '#include "merman.h"\n' | xcrun clang -fsyntax-only -x c -I "$headers_dir" -
}

build_host() {
    local system
    local arch
    local target
    local built_library
    local import_library
    local packaged_library
    system="$(uname -s)"
    arch="$(host_arch)"
    target="$(host_rust_target)"

    build_target_with_cargo "$target"

    case "$system" in
        Darwin)
            built_library="$REPO_ROOT/target/$target/$NATIVE_SDK_PROFILE/lib$NATIVE_SDK_LIBRARY_STEM.dylib"
            packaged_library="$FLUTTER_ROOT/macos/Libraries/libmerman_ffi.dylib"
            mkdir -p "$FLUTTER_ROOT/macos/Libraries"
            cp "$built_library" "$packaged_library"
            verify_macho_c_abi "$packaged_library"
            write_macos_xcframework "$packaged_library"
            ;;
        Linux)
            built_library="$REPO_ROOT/target/$target/$NATIVE_SDK_PROFILE/lib$NATIVE_SDK_LIBRARY_STEM.so"
            packaged_library="$FLUTTER_ROOT/linux/lib/$arch/libmerman_ffi.so"
            mkdir -p "$FLUTTER_ROOT/linux/lib/$arch"
            cp "$built_library" "$packaged_library"
            verify_dynamic_c_abi "$packaged_library"
            ;;
        MINGW*|MSYS*|CYGWIN*)
            built_library="$REPO_ROOT/target/$target/$NATIVE_SDK_PROFILE/$NATIVE_SDK_LIBRARY_STEM.dll"
            import_library="$REPO_ROOT/target/$target/$NATIVE_SDK_PROFILE/lib$NATIVE_SDK_LIBRARY_STEM.dll.a"
            packaged_library="$FLUTTER_ROOT/windows/merman_ffi.dll"
            cp "$built_library" "$packaged_library"
            verify_windows_dll_c_abi "$packaged_library" "$import_library"
            ;;
        *)
            echo "unsupported host platform: $system" >&2
            exit 1
            ;;
    esac
}

write_macos_xcframework() {
    local dylib="$1"
    local out_dir="$REPO_ROOT/target/flutter-macos-xcframework"
    local headers_dir="$out_dir/Headers"
    local library

    require_tool xcodebuild
    require_tool xcrun

    rm -rf "$out_dir" "$MACOS_XCFRAMEWORK_OUT"
    mkdir -p "$headers_dir"
    cp "$FFI_INCLUDE_DIR"/*.h "$headers_dir/"

    verify_public_headers "$headers_dir"

    xcodebuild -create-xcframework \
        -library "$dylib" \
        -headers "$headers_dir" \
        -output "$MACOS_XCFRAMEWORK_OUT"

    for HEADER_DIR in "$MACOS_XCFRAMEWORK_OUT"/*/Headers; do
        verify_public_headers "$HEADER_DIR"
        cat > "$HEADER_DIR/module.modulemap" <<'EOF'
module MermanFFI {
    header "merman.h"
    export *
}
EOF
    done

    for library in "$MACOS_XCFRAMEWORK_OUT"/*/"lib$NATIVE_SDK_LIBRARY_STEM.dylib"; do
        verify_macho_c_abi "$library"
    done
}

build_target_with_cargo() {
    local target="$1"
    echo "==> Building $RECIPE_PROFILE for $target"
    ensure_rust_target_installed "$target"
    python3 "$REPO_ROOT/scripts/artifact_profile_recipe.py" "$RECIPE_PROFILE" \
        --build --locked --target-triple "$target"
}

build_target_with_zigbuild() {
    local target="$1"
    echo "==> Building $RECIPE_PROFILE for $target with cargo-zigbuild"
    ensure_rust_target_installed "$target"
    python3 "$REPO_ROOT/scripts/artifact_profile_recipe.py" "$RECIPE_PROFILE" \
        --build --locked --build-tool cargo-zigbuild --target-triple "$target"
}

build_macos_universal() {
    if [[ "$(uname -s)" != "Darwin" ]]; then
        echo "==> Skipping macOS universal dylib; requires a macOS host"
        return
    fi

    require_tool lipo
    build_target_with_cargo aarch64-apple-darwin
    build_target_with_cargo x86_64-apple-darwin

    mkdir -p "$FLUTTER_ROOT/macos/Libraries"
    lipo -create \
        "$REPO_ROOT/target/aarch64-apple-darwin/$NATIVE_SDK_PROFILE/lib$NATIVE_SDK_LIBRARY_STEM.dylib" \
        "$REPO_ROOT/target/x86_64-apple-darwin/$NATIVE_SDK_PROFILE/lib$NATIVE_SDK_LIBRARY_STEM.dylib" \
        -output "$FLUTTER_ROOT/macos/Libraries/libmerman_ffi.dylib"
    write_macos_xcframework "$FLUTTER_ROOT/macos/Libraries/libmerman_ffi.dylib"
}

build_linux() {
    require_tool cargo-zigbuild
    require_tool zig
    build_target_with_zigbuild x86_64-unknown-linux-gnu
    build_target_with_zigbuild aarch64-unknown-linux-gnu

    mkdir -p "$FLUTTER_ROOT/linux/lib/x86_64" "$FLUTTER_ROOT/linux/lib/aarch64"
    cp "$REPO_ROOT/target/x86_64-unknown-linux-gnu/$NATIVE_SDK_PROFILE/lib$NATIVE_SDK_LIBRARY_STEM.so" \
        "$FLUTTER_ROOT/linux/lib/x86_64/libmerman_ffi.so"
    cp "$REPO_ROOT/target/aarch64-unknown-linux-gnu/$NATIVE_SDK_PROFILE/lib$NATIVE_SDK_LIBRARY_STEM.so" \
        "$FLUTTER_ROOT/linux/lib/aarch64/libmerman_ffi.so"
    verify_dynamic_c_abi "$FLUTTER_ROOT/linux/lib/x86_64/libmerman_ffi.so"
    verify_dynamic_c_abi "$FLUTTER_ROOT/linux/lib/aarch64/libmerman_ffi.so"
}

build_windows() {
    require_tool cargo-zigbuild
    require_tool zig
    build_target_with_zigbuild x86_64-pc-windows-gnu

    local built_library="$REPO_ROOT/target/x86_64-pc-windows-gnu/$NATIVE_SDK_PROFILE/$NATIVE_SDK_LIBRARY_STEM.dll"
    local import_library="$REPO_ROOT/target/x86_64-pc-windows-gnu/$NATIVE_SDK_PROFILE/lib$NATIVE_SDK_LIBRARY_STEM.dll.a"
    local packaged_library="$FLUTTER_ROOT/windows/merman_ffi.dll"
    cp "$built_library" "$packaged_library"
    verify_windows_dll_c_abi "$packaged_library" "$import_library"
}

require_tool cargo
require_tool rustup
LLVM_NM="$(resolve_llvm_nm)"

if [[ "$MODE" == "host" ]]; then
    build_host
else
    build_macos_universal
    build_linux
    build_windows
fi

echo "==> Desktop Flutter native artifacts are ready"
