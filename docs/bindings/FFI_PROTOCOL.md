# Merman FFI Protocol

Status: Draft
Last updated: 2026-06-02

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

The current C ABI exposes SVG, ASCII text, semantic JSON, layout JSON, validation JSON, and binding
metadata. Raster byte outputs are not part of this protocol version even though the Rust crate has a
reserved `raster` feature gate. All source-processing functions accept the shared `options_json`
contract documented in
`docs/bindings/OPTIONS_JSON.md`.

## Stability

The protocol is pre-1.0. Hosts should still treat these rules as the compatibility baseline for the
first FFI release candidate:

- ignore unknown JSON fields
- tolerate missing optional fields
- do not assume JSON field ordering
- release every non-empty result buffer exactly once

## Types

The current ABI protocol version is:

```c
#define MERMAN_ABI_VERSION 2
```

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

## ABI Introspection

Platform wrappers should check ABI compatibility before making render calls:

```c
uint32_t merman_abi_version(void);
const char* merman_package_version(void);
size_t merman_buffer_struct_size(void);
size_t merman_result_struct_size(void);
```

- `merman_abi_version()` returns `MERMAN_ABI_VERSION`.
- `merman_package_version()` returns a static null-terminated string owned by Rust. Do not free it.
- `merman_buffer_struct_size()` and `merman_result_struct_size()` return Rust-side struct sizes so
  hosts can catch packing or header/library mismatches at startup.

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

## ASCII Rendering

```c
MermanResult merman_render_ascii(
    const uint8_t* source,
    size_t source_len,
    const uint8_t* options_json,
    size_t options_len
);
```

On success, `data` contains UTF-8 terminal text. If the native library is built without the `ascii`
feature, this function returns `MERMAN_UNSUPPORTED_FORMAT`.

## Options JSON

Pass `NULL/0` for defaults. Non-empty options use the shared tolerant JSON object documented in
`docs/bindings/OPTIONS_JSON.md`.

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

## Validation JSON

```c
MermanResult merman_validate_json(
    const uint8_t* source,
    size_t source_len,
    const uint8_t* options_json,
    size_t options_len
);
```

This function returns `MERMAN_OK` when the validation payload was produced. Invalid source is
reported in `data`:

```json
{
  "valid": false,
  "error": "no Mermaid diagram detected",
  "code": 4,
  "code_name": "MERMAN_NO_DIAGRAM"
}
```

## Metadata JSON

```c
MermanResult merman_supported_diagrams_json(void);
MermanResult merman_ascii_supported_diagrams_json(void);
MermanResult merman_themes_json(void);
```

Each function returns a UTF-8 JSON string array. The same buffer ownership rules apply.

## Threading

The first ABI is stateless. Calls may be made concurrently as long as callers obey buffer ownership
rules.
