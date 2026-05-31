#!/usr/bin/env bash
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
FLUTTER_ROOT="$REPO_ROOT/platforms/flutter"
AUTO_INSTALL_RUST_TARGETS="${MERMAN_AUTO_INSTALL_RUST_TARGETS:-auto}"

MODE="host"

for ARG in "$@"; do
    case "$ARG" in
        --host) MODE="host" ;;
        --all) MODE="all" ;;
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

host_arch() {
    case "$(uname -m)" in
        x86_64|amd64) echo "x86_64" ;;
        arm64|aarch64) echo "aarch64" ;;
        *) echo "unsupported host architecture: $(uname -m)" >&2; exit 1 ;;
    esac
}

build_host() {
    local system
    local arch
    system="$(uname -s)"
    arch="$(host_arch)"

    echo "==> Building host merman-ffi library"
    cargo build --release -p merman-ffi --manifest-path "$REPO_ROOT/Cargo.toml"

    case "$system" in
        Darwin)
            mkdir -p "$FLUTTER_ROOT/macos/Libraries"
            cp "$REPO_ROOT/target/release/libmerman_ffi.dylib" \
                "$FLUTTER_ROOT/macos/Libraries/libmerman_ffi.dylib"
            ;;
        Linux)
            mkdir -p "$FLUTTER_ROOT/linux/lib/$arch"
            cp "$REPO_ROOT/target/release/libmerman_ffi.so" \
                "$FLUTTER_ROOT/linux/lib/$arch/libmerman_ffi.so"
            ;;
        MINGW*|MSYS*|CYGWIN*)
            cp "$REPO_ROOT/target/release/merman_ffi.dll" \
                "$FLUTTER_ROOT/windows/merman_ffi.dll"
            ;;
        *)
            echo "unsupported host platform: $system" >&2
            exit 1
            ;;
    esac
}

build_target_with_cargo() {
    local target="$1"
    echo "==> Building merman-ffi for $target"
    ensure_rust_target_installed "$target"
    cargo build --release -p merman-ffi --target "$target" --manifest-path "$REPO_ROOT/Cargo.toml"
}

build_target_with_zigbuild() {
    local target="$1"
    echo "==> Building merman-ffi for $target with cargo-zigbuild"
    ensure_rust_target_installed "$target"
    cargo zigbuild --release -p merman-ffi --target "$target" --manifest-path "$REPO_ROOT/Cargo.toml"
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
        "$REPO_ROOT/target/aarch64-apple-darwin/release/libmerman_ffi.dylib" \
        "$REPO_ROOT/target/x86_64-apple-darwin/release/libmerman_ffi.dylib" \
        -output "$FLUTTER_ROOT/macos/Libraries/libmerman_ffi.dylib"
}

build_linux() {
    require_tool cargo-zigbuild
    require_tool zig
    build_target_with_zigbuild x86_64-unknown-linux-gnu
    build_target_with_zigbuild aarch64-unknown-linux-gnu

    mkdir -p "$FLUTTER_ROOT/linux/lib/x86_64" "$FLUTTER_ROOT/linux/lib/aarch64"
    cp "$REPO_ROOT/target/x86_64-unknown-linux-gnu/release/libmerman_ffi.so" \
        "$FLUTTER_ROOT/linux/lib/x86_64/libmerman_ffi.so"
    cp "$REPO_ROOT/target/aarch64-unknown-linux-gnu/release/libmerman_ffi.so" \
        "$FLUTTER_ROOT/linux/lib/aarch64/libmerman_ffi.so"
}

build_windows() {
    require_tool cargo-zigbuild
    require_tool zig
    build_target_with_zigbuild x86_64-pc-windows-gnu

    cp "$REPO_ROOT/target/x86_64-pc-windows-gnu/release/merman_ffi.dll" \
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
