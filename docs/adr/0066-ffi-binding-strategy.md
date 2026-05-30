# ADR 0066: FFI Binding Strategy

- Status: accepted
- Date: 2026-05-30

## Context

`merman` is becoming useful outside Rust-only integrations: editors, mobile clients, desktop
applications, Flutter shells, JVM hosts, and Node-based tools can all benefit from headless Mermaid
parsing and rendering without launching a browser.

The public Rust API is intentionally safe and modular:

- `merman-core` parses Mermaid into metadata, semantic JSON, and typed render models.
- `merman-render` lays out diagrams and emits Mermaid-parity SVG.
- `merman` exposes convenience wrappers such as `HeadlessRenderer`.

Those crates currently forbid unsafe code. FFI will require unsafe boundary handling, memory
ownership rules, panic containment, and ABI compatibility commitments. That work should not leak
into the core crates.

The local RaTeX reference (`repo-ref/RaTeX`) uses a small `ratex-ffi` crate with
`cdylib`/`staticlib` outputs, a stable C ABI, heap-owned UTF-8 JSON results, explicit free
functions, thread-local last-error storage, platform-specific wrappers, and a documented JSON
protocol. That shape is a good fit for `merman` because the natural cross-language products are
already byte/string payloads: SVG, JSON, PNG/JPEG bytes, and PDF bytes.

UniFFI is also attractive for high-level Swift, Kotlin, Python, and Ruby bindings, but it should not
be the only public boundary. It is best treated as a generated convenience layer above a stable
binding facade, not as the canonical ABI contract for all hosts.

## Decision

Create FFI support as a separate boundary, not inside the existing safe crates.

1. Add a future `merman-ffi` crate for the canonical stable C ABI.
   - Build as `cdylib` and `staticlib`.
   - Keep unsafe code local to this crate.
   - Wrap all exported functions in panic-safe result handling.
   - Return owned byte buffers with explicit `merman_buffer_free`.
   - Expose errors through explicit result codes and retrievable error payloads.
   - Prefer UTF-8 bytes plus byte lengths over null-terminated strings for inputs.

2. Add a safe binding facade before or inside the FFI crate.
   - Convert request payloads into calls to `merman::render::HeadlessRenderer`.
   - Keep public wire payloads versioned and tolerant of unknown fields.
   - Use JSON for options because Mermaid config and render options evolve faster than C structs.

3. Treat UniFFI as an optional high-level binding layer.
   - A future `merman-uniffi` crate may expose `MermanEngine` and simple methods such as
     `render_svg`, `parse_json`, and `layout_json`.
   - It should share the same safe facade as `merman-ffi`.
   - It should not replace the C ABI for hosts that need C, C++, Flutter/Dart FFI, JNA/JNI, or
     hand-controlled binary packaging.

4. Start with SVG and JSON.
   - First C ABI milestone: Mermaid source in, SVG/semantic JSON/layout JSON out.
   - Raster outputs may follow behind a feature gate because they pull heavier dependencies.
   - RaTeX math support should remain feature-gated.

## Initial C ABI Shape

The exact names can change during the `ffi-api` workstream, but the first pass should follow this
shape:

```c
typedef struct {
    uint8_t* data;
    size_t len;
} MermanBuffer;

typedef struct {
    int32_t code;
    MermanBuffer data;
} MermanResult;

MermanResult merman_render_svg(
    const uint8_t* source,
    size_t source_len,
    const uint8_t* options_json,
    size_t options_json_len
);

MermanResult merman_parse_json(
    const uint8_t* source,
    size_t source_len,
    const uint8_t* options_json,
    size_t options_json_len
);

MermanResult merman_layout_json(
    const uint8_t* source,
    size_t source_len,
    const uint8_t* options_json,
    size_t options_json_len
);

void merman_buffer_free(MermanBuffer buffer);
```

`MermanResult.code == 0` means success. Non-zero codes return an error payload in `data`, encoded as
UTF-8 JSON or UTF-8 text according to the workstream's final protocol decision.

## Alternatives

1. Put `extern "C"` exports directly in `merman`.
   - Pros: fewer crates.
   - Cons: mixes unsafe ABI concerns into the safe public Rust crate and weakens the existing
     module boundary.

2. Use UniFFI as the only FFI layer.
   - Pros: faster Swift/Kotlin/Python/Ruby ergonomics and generated bindings.
   - Cons: not a universal ABI for C/C++/Flutter/JNA consumers, and less direct control over the
     low-level binary contract.

3. Follow RaTeX exactly with only C strings and thread-local last errors.
   - Pros: proven local reference and simple platform wrappers.
   - Cons: null-terminated strings are less suitable for arbitrary byte payloads such as PNG/PDF;
     explicit buffers and result payloads are a better base for `merman`.

4. Publish only CLI integration.
   - Pros: no ABI surface.
   - Cons: too slow and awkward for embedding in editors and applications that need in-process
     rendering.

## Consequences

- Core parsing/rendering crates can keep their safe-code policy.
- The C ABI becomes the long-term compatibility anchor.
- UniFFI can still be added without forcing every host through UniFFI's generated model.
- Options and error formats need a documented protocol and compatibility policy before release.
- The FFI crate must own extra gates: header checks, memory ownership tests, panic containment, and
  dynamic/static linking smoke tests.

## Follow-up

Open `docs/workstreams/ffi-api` to design and implement the first stable FFI slice.
