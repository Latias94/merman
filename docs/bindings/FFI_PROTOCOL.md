# Merman FFI Protocol

Status: Draft
Last updated: 2026-05-30

This document defines the first C ABI protocol for `merman-ffi`.

Authoritative code:

- `crates/merman-ffi/include/merman.h`
- `crates/merman-ffi/src/lib.rs`
- `docs/adr/0066-ffi-binding-strategy.md`
- `docs/workstreams/ffi-api/DESIGN.md`

## Build And Link

Build the C ABI artifacts from the workspace:

```sh
cargo build -p merman-ffi --release
```

`merman-ffi` is configured as `cdylib`, `staticlib`, and `rlib`. C and C-compatible hosts should
include `crates/merman-ffi/include/merman.h` and link the platform-specific artifact from
`target/release`.

Feature examples:

```sh
cargo build -p merman-ffi --release --features ratex-math
cargo build -p merman-ffi --release --features raster,ratex-math
```

The current C ABI exposes SVG, semantic JSON, and layout JSON. Raster byte outputs are not part of
this protocol version even though the Rust crate has a reserved `raster` feature gate.

## Stability

The protocol is pre-1.0. Hosts should still treat these rules as the compatibility baseline for the
first FFI release candidate:

- ignore unknown JSON fields
- tolerate missing optional fields
- do not assume JSON field ordering
- release every non-empty result buffer exactly once

## Types

```c
typedef struct MermanBuffer {
    uint8_t* data;
    size_t len;
} MermanBuffer;

typedef struct MermanResult {
    int32_t code;
    MermanBuffer data;
} MermanResult;
```

`MermanBuffer.data == NULL` means there is no payload. `len == 0` means the payload is empty.

## Result Codes

| Code | Name | Meaning |
| ---: | --- | --- |
| 0 | `MERMAN_OK` | Success. |
| 1 | `MERMAN_INVALID_ARGUMENT` | Pointer/length combination or option value is invalid. |
| 2 | `MERMAN_UTF8_ERROR` | Source or options bytes are not valid UTF-8. |
| 3 | `MERMAN_OPTIONS_JSON_ERROR` | Options JSON could not be parsed. |
| 4 | `MERMAN_NO_DIAGRAM` | No Mermaid diagram was detected. |
| 5 | `MERMAN_PARSE_ERROR` | Mermaid parsing failed. |
| 6 | `MERMAN_RENDER_ERROR` | Layout, SVG, or postprocessing failed. |
| 7 | `MERMAN_UNSUPPORTED_FORMAT` | Requested output is not enabled or not implemented. |
| 8 | `MERMAN_PANIC` | Rust panic was caught at the ABI boundary. |
| 9 | `MERMAN_INTERNAL_ERROR` | Serialization, allocation, or other unexpected internal failure. |

## Memory Ownership

Every non-empty `MermanResult.data` returned by Rust must be freed with:

```c
void merman_buffer_free(MermanBuffer buffer);
```

Passing `{ NULL, 0 }` is a no-op. Double-free is caller misuse.

## Input Rules

- `source == NULL && source_len == 0` is accepted as empty source.
- `source == NULL && source_len > 0` returns `MERMAN_INVALID_ARGUMENT`.
- `options_json == NULL && options_len == 0` means defaults.
- `options_json == NULL && options_len > 0` returns `MERMAN_INVALID_ARGUMENT`.
- Non-empty source/options buffers must be UTF-8.

## SVG Rendering

```c
MermanResult merman_render_svg(
    const uint8_t* source,
    size_t source_len,
    const uint8_t* options_json,
    size_t options_len
);
```

On success, `data` contains UTF-8 SVG bytes.

On error, `data` contains UTF-8 JSON:

```json
{
  "version": 1,
  "ok": false,
  "code": 6,
  "code_name": "MERMAN_RENDER_ERROR",
  "message": "layout failed: ..."
}
```

## Options JSON

Pass `NULL/0` for defaults. Non-empty options use a tolerant JSON object:

```json
{
  "version": 1,
  "parse": {
    "suppress_errors": false
  },
  "layout": {
    "viewport_width": 800.0,
    "viewport_height": 600.0,
    "text_measurer": "vendored",
    "math_renderer": "none"
  },
  "svg": {
    "diagram_id": null,
    "pipeline": "parity"
  }
}
```

Supported `layout.text_measurer` values:

- `vendored`
- `deterministic`

Supported `layout.math_renderer` values:

- `none`
- `ratex` when the `ratex-math` feature is enabled

Supported `svg.pipeline` values:

- `parity`
- `readable`
- `resvg_safe`
- `resvg-safe`

## Parse JSON

```c
MermanResult merman_parse_json(
    const uint8_t* source,
    size_t source_len,
    const uint8_t* options_json,
    size_t options_len
);
```

On success, `data` contains UTF-8 semantic model JSON. The current payload mirrors
`merman-cli parse` without `--meta`.

## Layout JSON

```c
MermanResult merman_layout_json(
    const uint8_t* source,
    size_t source_len,
    const uint8_t* options_json,
    size_t options_len
);
```

On success, `data` contains UTF-8 layout JSON using the same `LayoutedDiagram` shape as
`merman-cli layout`.

## Threading

The first ABI is stateless. Calls may be made concurrently as long as callers obey buffer ownership
rules.
