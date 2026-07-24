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

NATIVE_SDK_PACKAGE="$(recipe_field package)"
NATIVE_SDK_MANIFEST="$(recipe_field manifest)"
NATIVE_SDK_PROFILE="$(recipe_field profile)"
NATIVE_SDK_DEFAULT_FEATURES="$(recipe_field default-features)"
NATIVE_SDK_BUILD_TARGET="$(recipe_field build-target)"
NATIVE_SDK_TARGET="$(recipe_field target)"
NATIVE_SDK_TARGET_KINDS="$(recipe_field target-kinds)"
NATIVE_SDK_CRATE_TYPES="$(recipe_field crate-types)"
NATIVE_SDK_TRIPLES="$(recipe_field triples)"
NATIVE_SDK_LIBRARY_STEM="${NATIVE_SDK_TARGET//-/_}"

csv_contains() {
    [[ ",$1," == *",$2,"* ]]
}

validate_recipe() {
    if [[ "$NATIVE_SDK_BUILD_TARGET" != "target-set" ]]; then
        echo "$RECIPE_PROFILE must declare a target-set build target" >&2
        exit 2
    fi
    if [[ "$NATIVE_SDK_PACKAGE" != "merman-ffi" ]] ||
        [[ "$NATIVE_SDK_MANIFEST" != "crates/merman-ffi/Cargo.toml" ]] ||
        [[ "$NATIVE_SDK_PROFILE" != "native-sdk" ]] ||
        [[ "$NATIVE_SDK_TARGET" != "merman_ffi" ]] ||
        [[ "$NATIVE_SDK_TARGET_KINDS" != "cdylib,rlib,staticlib" ]] ||
        [[ "$NATIVE_SDK_CRATE_TYPES" != "cdylib,rlib,staticlib" ]] ||
        [[ "$NATIVE_SDK_TRIPLES" != "aarch64-apple-darwin,aarch64-unknown-linux-gnu,x86_64-apple-darwin,x86_64-pc-windows-gnu,x86_64-unknown-linux-gnu" ]]; then
        echo "$RECIPE_PROFILE must select the exact complete merman_ffi cdylib target set" >&2
        exit 2
    fi
    if [[ "$NATIVE_SDK_DEFAULT_FEATURES" != "false" ]]; then
        echo "$RECIPE_PROFILE must disable Cargo defaults" >&2
        exit 2
    fi
    if [[ ! -f "$REPO_ROOT/$NATIVE_SDK_MANIFEST" ]]; then
        echo "$RECIPE_PROFILE manifest does not exist: $NATIVE_SDK_MANIFEST" >&2
        exit 2
    fi
}

assert_target_in_recipe() {
    local target="$1"
    if ! csv_contains "$NATIVE_SDK_TRIPLES" "$target"; then
        echo "$RECIPE_PROFILE does not declare Rust target: $target" >&2
        exit 2
    fi
}

validate_recipe

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
    system="$(uname -s)"
    arch="$(host_arch)"
    target="$(host_rust_target)"

    build_target_with_cargo "$target"

    case "$system" in
        Darwin)
            mkdir -p "$FLUTTER_ROOT/macos/Libraries"
            cp "$REPO_ROOT/target/$target/$NATIVE_SDK_PROFILE/lib$NATIVE_SDK_LIBRARY_STEM.dylib" \
                "$FLUTTER_ROOT/macos/Libraries/libmerman_ffi.dylib"
            write_macos_xcframework "$FLUTTER_ROOT/macos/Libraries/libmerman_ffi.dylib"
            ;;
        Linux)
            mkdir -p "$FLUTTER_ROOT/linux/lib/$arch"
            cp "$REPO_ROOT/target/$target/$NATIVE_SDK_PROFILE/lib$NATIVE_SDK_LIBRARY_STEM.so" \
                "$FLUTTER_ROOT/linux/lib/$arch/libmerman_ffi.so"
            ;;
        MINGW*|MSYS*|CYGWIN*)
            cp "$REPO_ROOT/target/$target/$NATIVE_SDK_PROFILE/$NATIVE_SDK_LIBRARY_STEM.dll" \
                "$FLUTTER_ROOT/windows/merman_ffi.dll"
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
}

build_target_with_cargo() {
    local target="$1"
    assert_target_in_recipe "$target"
    echo "==> Building $NATIVE_SDK_PACKAGE for $target"
    ensure_rust_target_installed "$target"
    python3 "$REPO_ROOT/scripts/artifact_profile_recipe.py" "$RECIPE_PROFILE" \
        --build --locked --target-triple "$target"
}

build_target_with_zigbuild() {
    local target="$1"
    assert_target_in_recipe "$target"
    echo "==> Building $NATIVE_SDK_PACKAGE for $target with cargo-zigbuild"
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
}

build_windows() {
    require_tool cargo-zigbuild
    require_tool zig
    build_target_with_zigbuild x86_64-pc-windows-gnu

    cp "$REPO_ROOT/target/x86_64-pc-windows-gnu/$NATIVE_SDK_PROFILE/$NATIVE_SDK_LIBRARY_STEM.dll" \
        "$FLUTTER_ROOT/windows/merman_ffi.dll"
}

require_tool cargo
require_tool rustup

if [[ "$MODE" == "host" ]]; then
    build_host
else
    build_macos_universal
    build_linux
    build_windows
fi

echo "==> Desktop Flutter native artifacts are ready"
