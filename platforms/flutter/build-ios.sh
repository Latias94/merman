#!/usr/bin/env bash
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
FLUTTER_ROOT="$REPO_ROOT/platforms/flutter"
OUT_DIR="$REPO_ROOT/target/flutter-ios-xcframework"
FRAMEWORK_NAME="MermanFFI"
FRAMEWORK_OUT="$FLUTTER_ROOT/ios/$FRAMEWORK_NAME.xcframework"
FFI_INCLUDE_DIR="$REPO_ROOT/crates/merman-ffi/include"
AUTO_INSTALL_RUST_TARGETS="${MERMAN_AUTO_INSTALL_RUST_TARGETS:-auto}"
RECIPE_PROFILE="flutter-ios-native"

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
        [[ "$NATIVE_SDK_TRIPLES" != "aarch64-apple-ios,aarch64-apple-ios-sim,x86_64-apple-ios" ]]; then
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

build_cdylib() {
    local target="$1"
    assert_target_in_recipe "$target"
    echo "==> Building $NATIVE_SDK_PACKAGE for $target"
    ensure_rust_target_installed "$target"
    python3 "$REPO_ROOT/scripts/artifact_profile_recipe.py" "$RECIPE_PROFILE" \
        --build --locked --target-triple "$target"
}

verify_public_headers() {
    local headers_dir="$1"
    local public_header
    for public_header in "$FFI_INCLUDE_DIR"/*.h; do
        test -f "$headers_dir/$(basename "$public_header")"
    done
    printf '#include "merman.h"\n' | xcrun clang -fsyntax-only -x c -I "$headers_dir" -
}

write_framework_metadata() {
    local framework_dir="$1"
    mkdir -p "$framework_dir/Headers" "$framework_dir/Modules"
    cp "$FFI_INCLUDE_DIR"/*.h "$framework_dir/Headers/"

    verify_public_headers "$framework_dir/Headers"

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
    "$REPO_ROOT/target/aarch64-apple-ios/$NATIVE_SDK_PROFILE/lib$NATIVE_SDK_LIBRARY_STEM.dylib" \
    "$OUT_DIR/ios-arm64/$FRAMEWORK_NAME.framework"

mkdir -p "$OUT_DIR/ios-simulator"
lipo -create \
    "$REPO_ROOT/target/aarch64-apple-ios-sim/$NATIVE_SDK_PROFILE/lib$NATIVE_SDK_LIBRARY_STEM.dylib" \
    "$REPO_ROOT/target/x86_64-apple-ios/$NATIVE_SDK_PROFILE/lib$NATIVE_SDK_LIBRARY_STEM.dylib" \
    -output "$OUT_DIR/ios-simulator/$FRAMEWORK_NAME"

make_framework \
    "$OUT_DIR/ios-simulator/$FRAMEWORK_NAME" \
    "$OUT_DIR/ios-simulator/$FRAMEWORK_NAME.framework"

rm -rf "$FRAMEWORK_OUT"
xcodebuild -create-xcframework \
    -framework "$OUT_DIR/ios-arm64/$FRAMEWORK_NAME.framework" \
    -framework "$OUT_DIR/ios-simulator/$FRAMEWORK_NAME.framework" \
    -output "$FRAMEWORK_OUT"

for HEADERS_DIR in "$FRAMEWORK_OUT"/*/"$FRAMEWORK_NAME.framework"/Headers; do
    verify_public_headers "$HEADERS_DIR"
done

echo "==> Wrote $FRAMEWORK_OUT"
