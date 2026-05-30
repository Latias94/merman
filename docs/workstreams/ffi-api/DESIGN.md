# FFI API

Status: Complete
Last updated: 2026-05-30

## Why This Lane Exists

`merman` has a stable safe Rust API and a CLI, but downstream hosts may need in-process rendering
without embedding Rust directly:

- desktop editors and preview panes
- iOS/macOS Swift apps
- Android/Kotlin/JVM apps
- Flutter plugins through Dart FFI
- C/C++ hosts
- Node or other runtimes that prefer a native addon wrapper

FFI changes a hard-to-change public boundary, so the lane starts from ADR 0066 before exporting any
symbols.

## Relevant Authority

- `docs/adr/0066-ffi-binding-strategy.md`
- `docs/adr/0059-raster-output-strategy.md`
- `docs/adr/0063-extensible-svg-output-pipeline.md`
- `docs/adr/0064-host-styling-svg-postprocessors.md`
- `crates/merman/src/render/mod.rs`
- Local reference: `repo-ref/RaTeX/crates/ratex-ffi`
- Local reference: `repo-ref/RaTeX/docs/binding-architecture.md`

## Problem

The current API surface is Rust-native. The CLI is useful for tools, but it is not a good embedding
boundary for hosts that need low-latency in-process Mermaid parsing, layout, SVG emission, or raster
conversion.

The FFI surface must be small, stable, memory-safe for callers, panic-safe across the ABI boundary,
and expressive enough for future options such as render pipelines, diagram ids, strict/lenient
parsing, RaTeX math, and raster output.

## Target State

- A new `crates/merman-ffi` crate builds a C ABI library as `cdylib` and `staticlib`.
- Existing safe crates keep `#![forbid(unsafe_code)]`.
- C ABI inputs use `(const uint8_t* data, size_t len)` instead of null-terminated strings.
- C ABI outputs use owned buffers and an explicit `merman_buffer_free`.
- Result codes distinguish success, invalid input, parse/render errors, unsupported output, panic,
  and internal serialization errors.
- Options are JSON to avoid ABI churn as Mermaid and merman config evolve.
- The initial product supports:
  - `render_svg`
  - `parse_json`
  - `layout_json`
- Raster outputs are added later behind the existing `raster` feature.
- UniFFI is treated as an optional high-level layer over the same safe binding facade.

## In Scope

- `crates/merman-ffi/`
- workspace `Cargo.toml`
- public C header or generated header artifacts
- FFI smoke tests and memory ownership tests
- initial docs for C ABI usage
- optional planning stubs for `crates/merman-uniffi/`

## Out Of Scope

- Rewriting the Rust `merman` public API.
- Moving unsafe code into `merman-core`, `merman-render`, or `merman`.
- Changing default SVG parity behavior.
- Shipping first-party Node/Python/Flutter packages in the first slice.
- Making UniFFI the only supported ABI.

## Starting Assumptions

| Assumption | Confidence | Evidence | Consequence if wrong |
| --- | --- | --- | --- |
| SVG and JSON outputs are enough for the first FFI slice. | High | Current CLI/library usage and RaTeX reference. | Pull raster into M1 only if a real downstream host requires it. |
| JSON options are more stable than exported C structs for Mermaid config. | High | Mermaid config evolves and `SvgRenderOptions` is Rust-native. | Add small C structs later only for hot-path primitives. |
| C ABI should be canonical even if UniFFI is added. | High | Flutter/C/JNA/JNI consumers need direct ABI control. | If target hosts are only Swift/Kotlin/Python, UniFFI can be prioritized as a facade. |
| `HeadlessRenderer` is the right first internal engine. | High | It already bundles parse/layout/svg defaults and render methods. | Introduce a smaller safe facade if options become hard to express. |
| Byte buffers are better than C strings for merman. | High | Raster and PDF outputs are bytes, not text. | Text-only APIs can still expose convenience string wrappers later. |

## API Direction

First safe facade:

```rust
pub struct FfiRenderRequest {
    pub source: String,
    pub options_json: Option<String>,
}

pub struct FfiResponse {
    pub mime_type: &'static str,
    pub bytes: Vec<u8>,
}
```

First C ABI:

```c
typedef struct {
    uint8_t* data;
    size_t len;
} MermanBuffer;

typedef struct {
    int32_t code;
    MermanBuffer data;
} MermanResult;

MermanResult merman_render_svg(const uint8_t* source, size_t source_len,
                               const uint8_t* options_json, size_t options_len);

MermanResult merman_parse_json(const uint8_t* source, size_t source_len,
                               const uint8_t* options_json, size_t options_len);

MermanResult merman_layout_json(const uint8_t* source, size_t source_len,
                                const uint8_t* options_json, size_t options_len);

void merman_buffer_free(MermanBuffer buffer);
```

The exact protocol should be frozen in `docs/bindings/FFI_PROTOCOL.md` before the ABI is called
stable.

## Frozen M0 Protocol Decisions

These decisions complete `FFI-010` and are the starting contract for `FFI-020`.

### C Names

Use the `merman_` prefix for all exported symbols.

Initial public types:

- `MermanBuffer`
- `MermanResult`

Initial public functions:

- `merman_render_svg`
- `merman_buffer_free`

Planned follow-up functions:

- `merman_parse_json`
- `merman_layout_json`

### Result Codes

`MermanResult.code == 0` means success. Non-zero codes are errors.

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

### Payloads

On success, `MermanResult.data` contains the raw output for the called function:

- `merman_render_svg`: UTF-8 SVG bytes
- future `merman_parse_json`: UTF-8 semantic JSON bytes
- future `merman_layout_json`: UTF-8 layout JSON bytes

On error, `MermanResult.data` contains UTF-8 JSON:

```json
{
  "version": 1,
  "ok": false,
  "code": 6,
  "code_name": "MERMAN_RENDER_ERROR",
  "message": "layout failed: ..."
}
```

Callers must ignore unknown fields and tolerate missing optional fields.

### Memory Ownership

- Every non-empty `MermanResult.data` returned by Rust must be released with
  `merman_buffer_free`.
- `merman_buffer_free` is a no-op for `data == NULL`.
- Double-free remains caller misuse.
- The implementation may allocate through `Vec<u8>` internally, but callers must treat the buffer as
  opaque Rust-owned memory.

### Input Rules

- `source == NULL && source_len == 0` is accepted as an empty source and will usually return
  `MERMAN_NO_DIAGRAM`.
- `source == NULL && source_len > 0` returns `MERMAN_INVALID_ARGUMENT`.
- `options_json == NULL && options_len == 0` means defaults.
- `options_json == NULL && options_len > 0` returns `MERMAN_INVALID_ARGUMENT`.
- Inputs must be UTF-8 when their length is non-zero.

### Options JSON

Options use a versioned, tolerant JSON object:

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

Defaults match `HeadlessRenderer::default()` unless explicitly documented otherwise. Unknown fields
are ignored. Invalid known values return `MERMAN_INVALID_ARGUMENT` or
`MERMAN_OPTIONS_JSON_ERROR`, depending on whether the JSON parsed successfully.

### Threading

The first ABI slice is stateless and thread-safe by construction. A future opaque engine handle may
be added only if benchmarks show repeated setup cost matters or host configuration needs long-lived
state.

## Closeout Condition

This lane can close when the first FFI crate exports the SVG/JSON C ABI, header consumers can link
and call it, buffer ownership is tested, panic and invalid-input behavior are covered, docs describe
the protocol, and the final gate set is recorded in `EVIDENCE_AND_GATES.md`.
