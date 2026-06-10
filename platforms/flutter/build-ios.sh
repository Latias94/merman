#!/usr/bin/env bash
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
FLUTTER_ROOT="$REPO_ROOT/platforms/flutter"
OUT_DIR="$REPO_ROOT/target/flutter-ios-xcframework"
FRAMEWORK_NAME="MermanFFI"
FRAMEWORK_OUT="$FLUTTER_ROOT/ios/$FRAMEWORK_NAME.xcframework"
INCLUDE_HEADER="$REPO_ROOT/crates/merman-ffi/include/merman.h"
AUTO_INSTALL_RUST_TARGETS="${MERMAN_AUTO_INSTALL_RUST_TARGETS:-auto}"

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

build_cdylib() {
    local target="$1"
    echo "==> Building merman-ffi for $target"
    ensure_rust_target_installed "$target"
    cargo build --release -p merman-ffi --target "$target" --manifest-path "$REPO_ROOT/Cargo.toml"
}

write_framework_metadata() {
    local framework_dir="$1"
    mkdir -p "$framework_dir/Headers" "$framework_dir/Modules"
    cp "$INCLUDE_HEADER" "$framework_dir/Headers/merman.h"

    cat > "$framework_dir/Modules/module.modulemap" <<'EOF'
framework module MermanFFI {
  umbrella header "merman.h"
  export *
  module * { export * }
}
EOF

    cat > "$framework_dir/Info.plist" <<'EOF'
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
  <key>CFBundleDevelopmentRegion</key>
  <string>en</string>
  <key>CFBundleExecutable</key>
  <string>MermanFFI</string>
  <key>CFBundleIdentifier</key>
  <string>io.merman.flutter.MermanFFI</string>
  <key>CFBundleInfoDictionaryVersion</key>
  <string>6.0</string>
  <key>CFBundleName</key>
  <string>MermanFFI</string>
  <key>CFBundlePackageType</key>
  <string>FMWK</string>
  <key>CFBundleShortVersionString</key>
  <string>0.8.0</string>
  <key>CFBundleVersion</key>
  <string>0.8.0</string>
  <key>MinimumOSVersion</key>
  <string>13.0</string>
</dict>
</plist>
EOF
}

make_framework() {
    local binary="$1"
    local framework_dir="$2"
    mkdir -p "$framework_dir"
    cp "$binary" "$framework_dir/$FRAMEWORK_NAME"
    install_name_tool -id "@rpath/$FRAMEWORK_NAME.framework/$FRAMEWORK_NAME" "$framework_dir/$FRAMEWORK_NAME"
    xcrun strip -x "$framework_dir/$FRAMEWORK_NAME" 2>/dev/null || true
    write_framework_metadata "$framework_dir"
}

require_tool rustup
require_tool cargo
require_tool xcodebuild
require_tool lipo
require_tool install_name_tool
require_tool xcrun

rm -rf "$OUT_DIR"
mkdir -p "$OUT_DIR"

build_cdylib aarch64-apple-ios
build_cdylib aarch64-apple-ios-sim
build_cdylib x86_64-apple-ios

make_framework \
    "$REPO_ROOT/target/aarch64-apple-ios/release/libmerman_ffi.dylib" \
    "$OUT_DIR/ios-arm64/$FRAMEWORK_NAME.framework"

mkdir -p "$OUT_DIR/ios-simulator"
lipo -create \
    "$REPO_ROOT/target/aarch64-apple-ios-sim/release/libmerman_ffi.dylib" \
    "$REPO_ROOT/target/x86_64-apple-ios/release/libmerman_ffi.dylib" \
    -output "$OUT_DIR/ios-simulator/$FRAMEWORK_NAME"

make_framework \
    "$OUT_DIR/ios-simulator/$FRAMEWORK_NAME" \
    "$OUT_DIR/ios-simulator/$FRAMEWORK_NAME.framework"

rm -rf "$FRAMEWORK_OUT"
xcodebuild -create-xcframework \
    -framework "$OUT_DIR/ios-arm64/$FRAMEWORK_NAME.framework" \
    -framework "$OUT_DIR/ios-simulator/$FRAMEWORK_NAME.framework" \
    -output "$FRAMEWORK_OUT"

echo "==> Wrote $FRAMEWORK_OUT"
