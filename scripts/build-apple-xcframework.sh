#!/usr/bin/env bash
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "$0")/.." && pwd)"
OUT_DIR="$REPO_ROOT/target/apple-xcframework"
INCLUDE_DIR="$OUT_DIR/include"
XCFRAMEWORK_OUT="$REPO_ROOT/platforms/apple/Merman.xcframework"
SWIFT_BINDINGS_DIR="$REPO_ROOT/platforms/apple/Sources/Merman/Generated"
RECIPE_PROFILE="apple-uniffi-native"
BUILD_IOS=true
BUILD_MACOS=true
AUTO_INSTALL_RUST_TARGETS="${MERMAN_AUTO_INSTALL_RUST_TARGETS:-auto}"
METADATA_LIBRARY=""

recipe_field() {
    python3 "$REPO_ROOT/scripts/artifact_profile_recipe.py" "$RECIPE_PROFILE" --field "$1"
}

NATIVE_SDK_PROFILE="$(recipe_field profile)"
NATIVE_SDK_TARGET="$(recipe_field target)"
NATIVE_SDK_LIBRARY_STEM="${NATIVE_SDK_TARGET//-/_}"

for ARG in "$@"; do
    case "$ARG" in
        --ios) BUILD_MACOS=false ;;
        --macos) BUILD_IOS=false ;;
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

build_staticlib() {
    local target="$1"
    echo "==> Building $RECIPE_PROFILE for $target"
    ensure_rust_target_installed "$target"
    python3 "$REPO_ROOT/scripts/artifact_profile_recipe.py" "$RECIPE_PROFILE" \
        --build --locked --target-triple "$target"
    if [[ -z "$METADATA_LIBRARY" ]]; then
        METADATA_LIBRARY="$REPO_ROOT/target/$target/$NATIVE_SDK_PROFILE/lib$NATIVE_SDK_LIBRARY_STEM.a"
    fi
}

copy_staticlib() {
    local target="$1"
    local dest="$2"
    mkdir -p "$(dirname "$dest")"
    cp "$REPO_ROOT/target/$target/$NATIVE_SDK_PROFILE/lib$NATIVE_SDK_LIBRARY_STEM.a" "$dest"
}

generate_swift_bindings() {
    if [[ -z "$METADATA_LIBRARY" || ! -f "$METADATA_LIBRARY" ]]; then
        echo "could not locate a built merman-uniffi static library for Swift binding generation" >&2
        exit 1
    fi

    echo "==> Generating Swift UniFFI bindings"
    python3 "$REPO_ROOT/scripts/artifact_profile_recipe.py" "$RECIPE_PROFILE" \
        --run-example generate_swift_bindings \
        --locked \
        --extra-feature bindgen-smoke \
        --example-argument=--library \
        --example-argument="$METADATA_LIBRARY" \
        --example-argument=--output-dir \
        --example-argument="$SWIFT_BINDINGS_DIR"

    for artifact in Merman.swift MermanFFI.h MermanFFI.modulemap; do
        if [[ ! -f "$SWIFT_BINDINGS_DIR/$artifact" ]]; then
            echo "generated Swift binding artifact is missing: $SWIFT_BINDINGS_DIR/$artifact" >&2
            exit 1
        fi
    done
}

require_tool rustup
require_tool cargo
require_tool xcodebuild
require_tool lipo

rm -rf "$OUT_DIR"
mkdir -p "$INCLUDE_DIR"

XC_ARGS=()

if "$BUILD_IOS"; then
    build_staticlib aarch64-apple-ios
    copy_staticlib aarch64-apple-ios "$OUT_DIR/ios-arm64/lib$NATIVE_SDK_LIBRARY_STEM.a"

    build_staticlib aarch64-apple-ios-sim
    build_staticlib x86_64-apple-ios
    mkdir -p "$OUT_DIR/ios-simulator"
    lipo -create \
        "$REPO_ROOT/target/aarch64-apple-ios-sim/$NATIVE_SDK_PROFILE/lib$NATIVE_SDK_LIBRARY_STEM.a" \
        "$REPO_ROOT/target/x86_64-apple-ios/$NATIVE_SDK_PROFILE/lib$NATIVE_SDK_LIBRARY_STEM.a" \
        -output "$OUT_DIR/ios-simulator/lib$NATIVE_SDK_LIBRARY_STEM.a"

    XC_ARGS+=(
        -library "$OUT_DIR/ios-arm64/lib$NATIVE_SDK_LIBRARY_STEM.a" -headers "$INCLUDE_DIR"
        -library "$OUT_DIR/ios-simulator/lib$NATIVE_SDK_LIBRARY_STEM.a" -headers "$INCLUDE_DIR"
    )
fi

if "$BUILD_MACOS"; then
    build_staticlib aarch64-apple-darwin
    build_staticlib x86_64-apple-darwin
    mkdir -p "$OUT_DIR/macos"
    lipo -create \
        "$REPO_ROOT/target/aarch64-apple-darwin/$NATIVE_SDK_PROFILE/lib$NATIVE_SDK_LIBRARY_STEM.a" \
        "$REPO_ROOT/target/x86_64-apple-darwin/$NATIVE_SDK_PROFILE/lib$NATIVE_SDK_LIBRARY_STEM.a" \
        -output "$OUT_DIR/macos/lib$NATIVE_SDK_LIBRARY_STEM.a"
    XC_ARGS+=(-library "$OUT_DIR/macos/lib$NATIVE_SDK_LIBRARY_STEM.a" -headers "$INCLUDE_DIR")
fi

if [[ "${#XC_ARGS[@]}" -eq 0 ]]; then
    echo "no Apple platforms selected" >&2
    exit 2
fi

generate_swift_bindings
cp "$SWIFT_BINDINGS_DIR/MermanFFI.h" "$INCLUDE_DIR/MermanFFI.h"
cp "$SWIFT_BINDINGS_DIR/MermanFFI.modulemap" "$INCLUDE_DIR/module.modulemap"

rm -rf "$XCFRAMEWORK_OUT"
xcodebuild -create-xcframework "${XC_ARGS[@]}" -output "$XCFRAMEWORK_OUT"

for HEADER_DIR in "$XCFRAMEWORK_OUT"/*/Headers; do
    cp "$SWIFT_BINDINGS_DIR/MermanFFI.h" "$HEADER_DIR/MermanFFI.h"
    cp "$SWIFT_BINDINGS_DIR/MermanFFI.modulemap" "$HEADER_DIR/module.modulemap"
done

echo "==> Wrote $XCFRAMEWORK_OUT"
