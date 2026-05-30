# FFI API

Status: Draft
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

## Closeout Condition

This lane can close when the first FFI crate exports the SVG/JSON C ABI, header consumers can link
and call it, buffer ownership is tested, panic and invalid-input behavior are covered, docs describe
the protocol, and the final gate set is recorded in `EVIDENCE_AND_GATES.md`.
