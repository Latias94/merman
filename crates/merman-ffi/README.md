# merman-ffi

[![Crates.io](https://img.shields.io/crates/v/merman-ffi.svg)](https://crates.io/crates/merman-ffi)
[![Documentation](https://docs.rs/merman-ffi/badge.svg)](https://docs.rs/merman-ffi)
[![Crates.io Downloads](https://img.shields.io/crates/d/merman-ffi.svg)](https://crates.io/crates/merman-ffi)
[![Made with Rust](https://img.shields.io/badge/made%20with-Rust-orange.svg)](https://www.rust-lang.org)

[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](https://opensource.org/licenses/MIT)
[![License: Apache 2.0](https://img.shields.io/badge/License-Apache%202.0-blue.svg)](https://opensource.org/licenses/Apache-2.0)

C ABI bindings for embedding `merman` in non-Rust hosts.

`merman` is a headless Rust implementation of Mermaid diagram parsing, layout, and rendering. It is
intended for servers, CLIs, mobile apps, desktop apps, and other environments that need Mermaid
output without launching a browser. The main library can produce semantic JSON, layout JSON, SVG,
terminal text, and raster formats depending on enabled features.

Start with the main project README for product scope and diagram coverage:

- Project README: <https://github.com/Latias94/merman>
- Rust library: <https://crates.io/crates/merman>
- CLI: <https://crates.io/crates/merman-cli>
- Coverage status:
  <https://github.com/Latias94/merman/blob/main/docs/alignment/STATUS.md>

This crate exposes the low-level stable boundary described by
[`docs/bindings/FFI_PROTOCOL.md`](https://github.com/Latias94/merman/blob/main/docs/bindings/FFI_PROTOCOL.md).
Higher-level generated bindings such as UniFFI should sit above the same behavior, not replace this
C ABI.

## Build

From the workspace:

```sh
cargo build -p merman-ffi --release
```

The crate builds `cdylib`, `staticlib`, and `rlib` artifacts. Include
[`include/merman.h`](https://github.com/Latias94/merman/blob/main/crates/merman-ffi/include/merman.h)
from C or C-compatible hosts.

Optional features:

```sh
cargo build -p merman-ffi --release --features ratex-math
cargo build -p merman-ffi --release --features raster,ratex-math
```

The first C ABI release candidate exposes SVG, ASCII text, semantic JSON, layout JSON, validation
JSON, and binding metadata. Native raster byte outputs are intentionally split into a later ABI
lane.

## Minimal C Usage

```c
#include "merman.h"

static const uint8_t source[] = "flowchart TD\nA[Hello] --> B[World]";

MermanResult result = merman_render_svg(source, sizeof(source) - 1, NULL, 0);
if (result.code == MERMAN_OK) {
    /* result.data contains UTF-8 SVG bytes. */
}
merman_buffer_free(result.data);
```

Every non-empty `MermanResult.data` buffer must be freed exactly once with `merman_buffer_free`.
Do not use `free`, `delete`, or a host runtime allocator for buffers returned by Rust.

## Example

[`examples/render_svg.c`](https://github.com/Latias94/merman/blob/main/crates/merman-ffi/examples/render_svg.c)
is a small C consumer that renders a flowchart to SVG through the C ABI.

On macOS or Linux:

```sh
cargo build -p merman-ffi --release
cc -I crates/merman-ffi/include \
  crates/merman-ffi/examples/render_svg.c \
  -L target/release -lmerman_ffi \
  -Wl,-rpath,"$PWD/target/release" \
  -o target/merman-ffi-render-svg
target/merman-ffi-render-svg
```

## Entry Points

- `merman_abi_version`
- `merman_package_version`
- `merman_buffer_struct_size`
- `merman_result_struct_size`
- `merman_render_svg`
- `merman_render_ascii`
- `merman_parse_json`
- `merman_layout_json`
- `merman_validate_json`
- `merman_supported_diagrams_json`
- `merman_ascii_supported_diagrams_json`
- `merman_themes_json`
- `merman_buffer_free`

See
[`include/merman.h`](https://github.com/Latias94/merman/blob/main/crates/merman-ffi/include/merman.h)
for declarations and
[`docs/bindings/FFI_PROTOCOL.md`](https://github.com/Latias94/merman/blob/main/docs/bindings/FFI_PROTOCOL.md)
for result codes, options JSON, threading, and compatibility rules.

Higher-level platform wrappers:

- Apple SwiftPM:
  [`docs/bindings/APPLE_SWIFT.md`](https://github.com/Latias94/merman/blob/main/docs/bindings/APPLE_SWIFT.md)
- Android JNI/Kotlin:
  [`docs/bindings/ANDROID_JNI.md`](https://github.com/Latias94/merman/blob/main/docs/bindings/ANDROID_JNI.md)
- Flutter/Dart FFI:
  [`docs/bindings/FLUTTER_DART_FFI.md`](https://github.com/Latias94/merman/blob/main/docs/bindings/FLUTTER_DART_FFI.md)
- Python UniFFI:
  [`docs/bindings/PYTHON_UNIFFI.md`](https://github.com/Latias94/merman/blob/main/docs/bindings/PYTHON_UNIFFI.md)
