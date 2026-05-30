# merman-ffi

[![Crates.io](https://img.shields.io/crates/v/merman-ffi.svg)](https://crates.io/crates/merman-ffi)
[![Documentation](https://docs.rs/merman-ffi/badge.svg)](https://docs.rs/merman-ffi)
[![Crates.io Downloads](https://img.shields.io/crates/d/merman-ffi.svg)](https://crates.io/crates/merman-ffi)
[![Made with Rust](https://img.shields.io/badge/made%20with-Rust-orange.svg)](https://www.rust-lang.org)

[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](https://opensource.org/licenses/MIT)
[![License: Apache 2.0](https://img.shields.io/badge/License-Apache%202.0-blue.svg)](https://opensource.org/licenses/Apache-2.0)

C ABI bindings for embedding `merman` in non-Rust hosts.

This crate exposes the low-level stable boundary described by
[`docs/bindings/FFI_PROTOCOL.md`](../../docs/bindings/FFI_PROTOCOL.md). Higher-level generated
bindings such as UniFFI should sit above the same behavior, not replace this C ABI.

## Build

From the workspace:

```sh
cargo build -p merman-ffi --release
```

The crate builds `cdylib`, `staticlib`, and `rlib` artifacts. Include
[`include/merman.h`](include/merman.h) from C or C-compatible hosts.

Optional features:

```sh
cargo build -p merman-ffi --release --features ratex-math
cargo build -p merman-ffi --release --features raster,ratex-math
```

The first C ABI release candidate exposes SVG, semantic JSON, and layout JSON. Native raster byte
outputs are intentionally split into a later ABI lane.

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

## Entry Points

- `merman_abi_version`
- `merman_package_version`
- `merman_buffer_struct_size`
- `merman_result_struct_size`
- `merman_render_svg`
- `merman_parse_json`
- `merman_layout_json`
- `merman_buffer_free`

See [`include/merman.h`](include/merman.h) for declarations and
[`docs/bindings/FFI_PROTOCOL.md`](../../docs/bindings/FFI_PROTOCOL.md) for result codes, options
JSON, threading, and compatibility rules.
