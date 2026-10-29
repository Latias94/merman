# merman-wasi

A thin `wasm32-wasip1` **cdylib reactor**, independently versioned as
`merman-wasi` ABI **1**. It executes the shared `merman-bindings-core` engine;
there is no transport-specific Mermaid parser, JavaScript glue requirement, native C
function-pointer table, host callback, icon-registry constructor, filesystem font
acquisition, system-font discovery, or network acquisition.

Its binding transport is `wasi`; its semantic target remains `native`.
The artifact admits only deterministic runtime policy and an immutable
`native-font-context` constructor service. The native C ABI's numeric statuses,
operation codes, tokens and layouts do **not** define this ABI. The authoritative
protocol guide is this README, packaged as `docs/WASI_ABI.md` in WASI SDK archives.
The implementation lives in `src/wire.rs`, `src/runtime.rs`, and `src/abi.rs`;
`platforms/wasi/protocol.json` owns the real-host import/export audit policy.

ABI 1 is the initial WASI contract. Unpublished protocol iterations do not have
compatibility aliases or migration adapters. Its version does not reset the
shared binding options schema, native C ABI, or workspace package version.

## Building

The low-level crate defaults to **no features**. A typical standard reactor is:

```console
cargo build -p merman-wasi --target wasm32-wasip1 --profile wasm-size --no-default-features --features all-diagrams,svg,native-fonts,png,bundled-fonts,layout-cytoscape
```

Add `math` for mathematical rendering and additionally `layout-elk` for the full
recipe. Packaging, generated capability/legal projections, import auditing, and
real wazero/wasmtime qualification belong to the integration owner. This crate
does not assert that an untested artifact is qualified.

The pinned RaTeX `embed-fonts` closure currently retains `wasm-bindgen` helper exports
through `ratex-font-loader -> ratex-unicode-font -> system-fonts -> web-sys` on wasm32.
They require no JavaScript imports in these WASI artifacts and are not this SDK's ABI.
The real hosts audit their exact scalar signatures separately and never invoke them;
standard must not contain them. Math uses embedded KaTeX fonts, not system discovery.
See `platforms/wasi/protocol.json` for the bounded implementation-export inventory.

Diagram selectors forward to the shared facade and binding engine. `analysis`
and `ascii` can also be selected explicitly. No `native-runtime`, system-clock,
system-timezone, or system-random feature is exposed. `svg`, `native-fonts`, `png`,
`bundled-fonts`, layout selectors and `math` forward to their existing owners.
Without `native-fonts`, catalog discovery works, but engine construction returns
an owned `missing-capability` error naming `native-fonts`.

## Scalar exports

Initialize the WASI reactor once via its `_initialize` export before calling the
API. Exported `memory` is WebAssembly linear memory. Every API name has the
`merman_wasi_` prefix; every argument is a Wasm `i32` interpreted as an unsigned
`u32`, and only release functions return a signed `i32`.

| Export | Parameters | Result |
| --- | --- | --- |
| `merman_wasi_abi_version` | none | ABI version, `1` |
| `merman_wasi_alloc` | `len` | input offset, or `0` |
| `merman_wasi_free` | `offset` | release status |
| `merman_wasi_engine_new` | `config_off, config_len, fonts_off, fonts_len` | owned result handle |
| `merman_wasi_engine_close` | `engine` | release status |
| `merman_wasi_execute` | `engine, request_off, request_len, source_off, source_len` | owned result handle |
| `merman_wasi_runtime_catalog` | none | owned result handle |
| `merman_wasi_result_info` | `result` | descriptor offset, or `0` |
| `merman_wasi_result_free` | `result` | release status |

Release statuses are independent ABI values: `0` success, `-1` invalid input
allocation, `-2` invalid/closed/wrong-kind handle, `-3` busy/reentrant call.
`alloc` returns zero-initialized input bytes. `free` requires the exact live
allocation base, not an interior pointer.
`alloc(0)` and `free(0)` are rejected. Producing calls return `0` only when they
cannot publish an owned result, including handle exhaustion, or the instance is
busy. `abi_version` returns `0` during reentry. `result_info` returns `0` for an
unknown/freed/wrong-kind handle or a busy instance.

### Result descriptor

`result_info` addresses **40 bytes**, ten consecutive **little-endian u32**
values. Decode bytes explicitly; do not cast to a native host struct.

| Byte offset | Field | Meaning |
| --- | --- | --- |
| 0 | `version` | `1` |
| 4 | `size` | `40` |
| 8 | `status` | `0` success, `1` ordinary owned error |
| 12 | `engine` | constructed/selected engine identity on success, otherwise `0` |
| 16 / 20 | `media_ptr / media_len` | UTF-8 media type bytes |
| 24 / 28 | `data_ptr / data_len` | raw successful output bytes |
| 32 / 36 | `json_ptr / json_len` | operation metadata or structured error JSON |

Empty segments have pointer and length `0`. No segment is NUL-terminated.
Constructor success has empty `data`, `application/json` media, and
`{"version":1,"ok":true}` in `json`; read its engine from the descriptor.
Catalog and named metadata successes have `application/json` media, JSON in
`data`, and empty `json`. Catalog results have engine `0`; named metadata
results retain the selected engine. The catalog's `transport_api_version` is `1`.
Execute successes have raw SVG/PNG/other enabled operation output in `data` and
the unchanged core `BindingOperationMetadata` in `json`. In particular, the
additive `svg_viewport` carries `min_x, min_y, width, height` in CSS pixels when
the SVG root declares a viewBox. Responsive roots without one (such as Info)
omit that field rather than inventing dimensions or failing the render. PNG
pixel dimensions remain in its core `output_plan`. This transport never parses
SVG to invent metadata.

Ordinary errors have descriptor status `1`, engine `0`, `application/json`
media, empty `data`, and the **core error-payload schema** in `json`: `version`,
`ok: false`, `code`, `code_name`, `kind`, nullable `capability_id`, `message`, and
optional structured details, preserving core byte spans. `code`/`code_name`
belong to that binding payload, **not** to descriptor status or the native C ABI.
Unknown operations, absent capabilities, option and parse errors remain owned
ordinary failures and do not poison an engine.

## Input envelopes

Every input byte must be written into a **live allocation returned by this
instance's `alloc`**. Nonempty ranges may be contained subranges of one live
input allocation. They may not span allocations or refer to result memory,
freed buffers, arbitrary module memory, or the host address space. `(0, 0)` is
the canonical empty range; even zero-length nonzero offsets must belong to a
live allocation. Checked arithmetic and safe slice indexing validate every
range before borrowing. Inputs are borrowed only for the synchronous call.

Configuration is UTF-8 JSON:

```json
{
  "version": 1,
  "options": {"resources": {"profile": "interactive"}},
  "fonts": [{"offset": 0, "length": 123456}],
  "bundled_fonts": true
}
```

Configuration requires `version: 1`; `options`, `fonts`, and `bundled_fonts`
are optional. An omitted `fonts` array defaults to empty. Configuration and
request envelopes reject unknown fields and duplicate envelope fields.

Each font entry requires u32 `offset` and `length` fields and rejects unknown
or duplicate fields. These pairs are relative to the **separately supplied raw
font blob**, not to linear memory. Supply actual font lengths; the number above
is illustrative. Font ranges must be nonempty and contained in that blob. Fonts
are copied into the bounded memory-only font service during construction;
the host may free both configuration and font input buffers after the call.
Caller fonts are admitted **before** production bundled fonts, so caller
family/style matches take precedence. `bundled_fonts` defaults to whether the
artifact compiled `bundled-fonts`; explicitly requesting an unavailable bundle
returns a missing-capability error. An empty admitted font set is an error.
Strict glyph coverage is enabled: unsupported shaping or missing glyphs fail
instead of silently claiming accurate measurement. Measurement and PNG painting
share the same admitted font context. **SVG does not embed fonts**: downstream
SVG display still needs matching fonts. Bundle provenance, coverage and OFL
license text remain owned by `merman-export/assets/fonts/` and the SDK package.

Execution uses a separate UTF-8 request envelope and raw source bytes:

```json
{"version":1,"operation_id":"svg","options":{"svg":{"diagram_id":"example"}}}
```

Requests require `version: 1` and a string `operation_id`.
Optional `uri` is a UTF-8 string for document-analysis operations. `options` is
borrowed as JSON `RawValue` and passed unchanged to core validation, retaining
duplicate-field and type errors; it is not normalized into a second options
model. Source, font inputs and PNG outputs are **raw bytes, never base64**.
`operation_id` selects a core operation ID or one of the catalog's
`metadata_ids`. Named metadata uses `BindingEngine::metadata_json` and requires
empty source, no URI, and no nonempty options overlay. All other operations use
`BindingEngine::execute(BindingOperationRequest)` without a parallel parser.

Omitted, null or empty-object **constructor** options select the existing
`interactive` resource profile. Empty **request** options inherit the engine's
profile. Nonempty caller options are preserved: an explicit `constrained`
profile must still reject math through core's ordinary structured error.
The host must also impose its own linear-memory limit and fuel/epoch/timeout
policy; cooperative profiles are not a process sandbox or hard wall-clock bound.

## Ownership, lifecycle and limits

1. Allocate and write config/font inputs; construct and read its owned result.
2. Copy out the engine handle; free that result and the inputs independently.
3. Allocate and write request/source inputs; execute and read its descriptor.
4. Copy output bytes before `result_free`; release inputs with `free`.
5. Reuse the instance/engine **serially**, then close engines and release results.

The result registry owns a stable `Box<[u8]>` containing its descriptor and
payload until `result_free`. Later calls, input release and engine close do not
invalidate other live results. Engine/result handles share a monotonically
increasing nonzero namespace; they are never reused and wrong-kind calls are
rejected by their registries. Constructor engine publication and owned result
publication are transactional. Input offsets, unlike handles, may be reused by
the allocator after release: a stale offset is never a durable identity.

The host must not mutate descriptors/results, retained private module memory,
or any input concurrently with a call. Offsets/handles prevent accidental
misuse, not hostile writes to a shared linear memory. Reacquire the runtime's
memory view after calls that may grow memory. Do not use a host allocator to
release module-owned storage.

Per-instance transport ceilings:

| Resource | Ceiling |
| --- | --- |
| live input allocations | 64 |
| one input / all live input bytes | 32 MiB / 64 MiB |
| config or request envelope | 1 MiB |
| live owned results | 64 |
| one result / all live result bytes, including descriptors | 32 MiB / 64 MiB |
| live engines | 16 |
| one engine / all engines' admitted font input bytes | 32 MiB / 64 MiB |

NativeFontLimits independently bound font buffers/faces, measurement work and
caches. Transport byte accounting includes font bytes before deduplication, but
does not claim to bound every transient renderer allocation or the entire heap.
Count and byte budgets are checked before copying input fonts or assembling
result payloads. Registries and transport copies use `try_reserve` where
available. A result-budget error is itself owned if enough space remains;
otherwise the producer returns `0`. Release results to regain capacity.

Calls are synchronous. Same-instance reentry is rejected without blocking; no
callbacks are installed. An ordinary owned failure permits the next call.
There is **no promised recovery from panic, allocator OOM or a Wasm trap**;
drop the entire instance after a trap rather than trying to reuse its state.

## Tests

Native unit tests exercise the actual runtime, engine and optional renderers;
they use test-only virtual offsets rather than narrowing native pointers.
Tests cover descriptor bytes, ownership, allocation and handle validation,
budgets, constructor rollback, reentry, raw options, error recovery, real SVG
and PNG output, metadata dispatch, bundled fonts and result survival after
engine close. SVG/PNG execution tests require `native-fonts,diagram-flowchart`
and optionally `png`; no renderer stub substitutes for those tests.
The integration owner runs Cargo sequentially and separately tests the actual
reactor and admitted WASI imports in wazero and wasmtime.
