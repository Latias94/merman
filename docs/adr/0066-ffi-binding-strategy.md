# ADR 0066: FFI Binding Strategy

- Status: accepted; capability and output identity amended by ADR-0076
- Date: 2026-05-30

ADR-0076 supersedes any implication that an FFI descriptor or binding-specific boolean catalog
owns capability or output semantic IDs. The separate unsafe FFI crate, safe facade, ownership,
panic-containment, and C-plus-UniFFI layering decisions remain accepted. Native ABI 3 is now the
implemented contract: it is generated from `abi/merman-v3.json`, discovered through one
size-tagged API table, and routes every output through the shared binding operation model. ABI 2
has been retired rather than retained as a compatibility layer.

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

1. Keep the canonical stable C ABI in the separate `merman-ffi` crate.
   - Build as `cdylib` and `staticlib`.
   - Keep unsafe code local to this crate.
   - Wrap all exported functions in panic-safe result handling.
   - Discover one size-tagged ABI 3 function table through `merman_get_native_api`.
   - Return each operation through one address-owned `MermanNativeResult`, released through the
     table's `result_free` function.
   - Expose errors through explicit result codes and retrievable error payloads.
   - Prefer UTF-8 bytes plus byte lengths over null-terminated strings for inputs.

2. Route transports through the safe `merman-bindings-core` facade.
   - Convert each generic operation request into the canonical parser, analysis, renderer, or
     exporter operation.
   - Keep public wire payloads versioned and tolerant of unknown fields.
   - Use JSON for options because Mermaid config and render options evolve faster than C structs.

3. Treat UniFFI as a high-level binding transport.
   - `merman-uniffi` exposes `MermanEngine`, generic operation dispatch, and ergonomic convenience
     methods such as `render_svg`, `parse_json`, and `layout_json`.
   - It should share the same safe facade as `merman-ffi`.
   - It should not replace the C ABI for hosts that need C, C++, Flutter/Dart FFI, JNA/JNI, or
     hand-controlled binary packaging.

4. Keep callable output capabilities explicit.
   - SVG, semantic/analysis/layout JSON, ASCII, PNG, JPEG, and PDF use the same generic operation
     contract.
   - Optional backends preserve the ABI shape and return a typed `missing-capability` error when
     absent.
   - RaTeX math support remains feature-gated.

## Implemented Native ABI 3

The current implementation keeps one exported discovery symbol, `merman_get_native_api`.
The host proves the generated ABI version and descriptor digest, receives a function table, checks
record layout through a surface-owned compile-run test, creates an opaque engine token, and uses
one generic execution route for SVG, binary export, ASCII, and JSON outputs. Request options are
deeply merged over the reusable engine baseline for one operation, while runtime-policy selection
remains constructor-owned. Result buffers are released through the discovered `result_free` slot.
The direct runtime catalog exposes both the compiled contract and the closed capability/output
vocabulary.

This design removes the old per-output exported function family, raw engine pointers, and duplicate
ABI-specific capability catalog. It returns one owned output buffer instead of a post-hoc chunk
sink, matching the current backends that already materialize complete outputs. It also lets
generated C, Android, Flutter, and other transport bindings share output IDs without maintaining
parallel operation enums.

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
