#!/usr/bin/env bash
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "$0")/.." && pwd)"
OUT_DIR="$REPO_ROOT/target/apple-xcframework"
INCLUDE_DIR="$OUT_DIR/include"
XCFRAMEWORK_OUT="$REPO_ROOT/platforms/apple/Merman.xcframework"
BUILD_IOS=true
BUILD_MACOS=true
AUTO_INSTALL_RUST_TARGETS="${MERMAN_AUTO_INSTALL_RUST_TARGETS:-auto}"

for ARG in "$@"; do
    case "$ARG" in
        --ios) BUILD_MACOS=false ;;
        --macos) BUILD_IOS=false ;;
        *) echo "unknown argument: $ARG" >&2; exit 2 ;;
    esac
done

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
    echo "==> Building merman-ffi for $target"
    ensure_rust_target_installed "$target"
    cargo build --release -p merman-ffi --target "$target" --manifest-path "$REPO_ROOT/Cargo.toml"
}

copy_staticlib() {
    local target="$1"
    local dest="$2"
    mkdir -p "$(dirname "$dest")"
    cp "$REPO_ROOT/target/$target/release/libmerman_ffi.a" "$dest"
}

require_tool rustup
require_tool cargo
require_tool xcodebuild
require_tool lipo

rm -rf "$OUT_DIR"
mkdir -p "$INCLUDE_DIR"
cp "$REPO_ROOT/crates/merman-ffi/include/merman.h" "$INCLUDE_DIR/merman.h"

XC_ARGS=()

if "$BUILD_IOS"; then
    build_staticlib aarch64-apple-ios
    copy_staticlib aarch64-apple-ios "$OUT_DIR/ios-arm64/libmerman_ffi.a"

    build_staticlib aarch64-apple-ios-sim
    build_staticlib x86_64-apple-ios
    mkdir -p "$OUT_DIR/ios-simulator"
    lipo -create \
        "$REPO_ROOT/target/aarch64-apple-ios-sim/release/libmerman_ffi.a" \
        "$REPO_ROOT/target/x86_64-apple-ios/release/libmerman_ffi.a" \
        -output "$OUT_DIR/ios-simulator/libmerman_ffi.a"

    XC_ARGS+=(
        -library "$OUT_DIR/ios-arm64/libmerman_ffi.a" -headers "$INCLUDE_DIR"
        -library "$OUT_DIR/ios-simulator/libmerman_ffi.a" -headers "$INCLUDE_DIR"
    )
fi

if "$BUILD_MACOS"; then
    build_staticlib aarch64-apple-darwin
    build_staticlib x86_64-apple-darwin
    mkdir -p "$OUT_DIR/macos"
    lipo -create \
        "$REPO_ROOT/target/aarch64-apple-darwin/release/libmerman_ffi.a" \
        "$REPO_ROOT/target/x86_64-apple-darwin/release/libmerman_ffi.a" \
        -output "$OUT_DIR/macos/libmerman_ffi.a"
    XC_ARGS+=(-library "$OUT_DIR/macos/libmerman_ffi.a" -headers "$INCLUDE_DIR")
fi

if [[ "${#XC_ARGS[@]}" -eq 0 ]]; then
    echo "no Apple platforms selected" >&2
    exit 2
fi

rm -rf "$XCFRAMEWORK_OUT"
xcodebuild -create-xcframework "${XC_ARGS[@]}" -output "$XCFRAMEWORK_OUT"

for HEADER_DIR in "$XCFRAMEWORK_OUT"/*/Headers; do
    cat > "$HEADER_DIR/module.modulemap" <<'EOF'
module MermanFFI {
    header "merman.h"
    export *
}
EOF
done

echo "==> Wrote $XCFRAMEWORK_OUT"
