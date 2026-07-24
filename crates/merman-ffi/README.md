# merman-ffi

Embed Merman's headless Mermaid parser, analyzer, layout engine, and render/export backends in a
C-compatible host. It does not require a browser or JavaScript runtime.

> **Native ABI 3:** this crate exposes native ABI 3 only. The ABI 2 entry points and records shipped
> in `0.8.0-alpha.2` and `0.8.0-alpha.3` were removed; rebuild those hosts against the ABI 3 header
> and library from the same Merman release.

## Build

`merman-ffi` has no default features. Choose an explicit capability set:

The committed `c-abi-native` artifact profile owns the complete release recipe used by C
consumers and Flutter packaging.

```sh
# Canonical native SDK artifact: SVG, analysis, ASCII, PNG, JPEG, PDF, layouts, math, and native adapters.
cargo build -p merman-ffi --release --no-default-features --features svg,analysis,ascii,png,jpeg,pdf,layout-cytoscape,layout-elk,math,system-clock,system-timezone,system-random

# A semantic-only embedding.
cargo build -p merman-ffi --release --no-default-features

# A focused SVG artifact.
cargo build -p merman-ffi --release --no-default-features --features svg
```

The crate produces `cdylib`, `staticlib`, and `rlib` artifacts. Include the release-matched
[`include/merman.h`](include/merman.h) instead of copying a header from a moving branch.

## ABI 3 Model

The C surface has one exported discovery symbol:

```c
MermanNativeStatus merman_get_native_api(
    const MermanNativeApiRequest *request,
    MermanNativeApi *out_api
);
```

The host supplies `MERMAN_NATIVE_ABI_VERSION` and
`MERMAN_NATIVE_ABI_LAYOUT_DESCRIPTOR_DIGEST`, then receives a size-tagged function table. Before
consuming a record, initialize caller-owned input records with
`MERMAN_NATIVE_STRUCT_SIZE(Type)`. The generated C header and release C smoke test carry the
compile-run layout fingerprint; applications should not implement a second runtime offset probe.
`MermanNativeResult` is a write-only output record: use
`MERMAN_NATIVE_RESULT_INIT`, which initializes its required `struct_size` without requiring a
host to initialize fields Merman will overwrite. This makes mixed headers, binaries, and foreign
struct packing fail at discovery rather than during rendering.

Every operation follows the same path:

1. Call `merman_get_native_api`.
2. Create an engine token with `api.engine_new`.
3. Set `MermanNativeOperationRequest.operation` to the requested operation enum.
4. Call `api.execute_collect`.
5. Release every result with `api.result_free`, then release the token with `api.engine_free`.

Engine options select runtime state explicitly. Omitting `runtime_policy` uses Merman's
deterministic clock, UTC time zone, and fixed random seed, even when native adapters are compiled.
Set `{ "runtime_policy": "native" }` only when the operation should consult the compiled system
clock, time-zone, and random adapters. If one is unavailable, engine creation returns the typed
unsupported-operation status. Successful generic operation metadata includes
`"runtime_policy":"deterministic"` or `"runtime_policy":"native"`.

`MermanNativeOperationRequest.options_json` accepts the same generic options document for one
operation. Request objects recursively override the reusable engine baseline, while omitted nested
values remain inherited and the baseline itself is not mutated. `runtime_policy` remains
constructor-owned and is rejected in request options.

`api.runtime_catalog` returns the flat schema-1 catalog with package version, compiled capability,
operation, and output IDs, registry facts, resource defaults/limits, and text-measurement
providers. It is the source of truth for the loaded artifact; do not infer availability from Cargo
feature names.

The generic operation enums cover SVG, PNG, JPEG, PDF, ASCII, semantic JSON, layout JSON, analysis,
validation, and URI-requiring document analysis. An unavailable operation returns the typed
`MERMAN_NATIVE_STATUS_UNSUPPORTED_OPERATION` result rather than exposing a separate phantom API.
Failure JSON schema `1` further distinguishes `unknown-operation` from `missing-capability`; only the
latter carries the exact descriptor `capability_id`. All other failures use `generic` with a null
capability ID.

[`examples/render_svg.c`](examples/render_svg.c) is the minimal discovery-and-render program.
[`examples/render_svg_engine.c`](examples/render_svg_engine.c) also shows a host text-measurement
callback installed when the engine is created.

## Ownership And Callbacks

`MermanNativeResult.data` and `metadata_or_error_json` are Merman-owned only after Merman has
written the result, and only until `api.result_free(&result)`. Ownership is bound to the exact
result record address Merman wrote; copying or moving a live result does not transfer ownership.
Call `result_free` on that original record for each Merman-written result, including failures,
before reuse. Repeated calls and calls on a full-size record with only `struct_size` initialized
are harmless, and nested buffer fields are never treated as allocation authority. Inputs and
callback request slices are borrowed for the call only.

Engine values are opaque nonzero tokens, not pointers. `engine_free` retires a token immediately;
an already active operation retains its internal state safely, but the host must not make another
call with the retired token. It is not a quiescence barrier: keep the configured text-measurement
callback and `user_data` valid until every operation started before retirement has returned. Any
thread that re-enters or retires the same engine while a host text-measurement callback is active
receives a typed reentrancy failure; other engine tokens remain usable.

Merman provides deterministic vendored text measurement by default. A preview host can set
`MermanNativeEngineConfig.text_measure` to measure with its actual display font stack. The callback
is synchronous; return `handled = 0` when a request cannot be answered faithfully and Merman will
fall back for that request. The operation/result-kind contract is generated in
[`include/merman_text_measurement_abi.h`](include/merman_text_measurement_abi.h).

## Feature Selection

The public feature names describe callable capabilities:

- `svg`, `analysis`, and `ascii` enable their corresponding operation families.
- `png`, `jpeg`, and `pdf` add real binary output operations.
- `layout-cytoscape`, `layout-elk`, and `math` add their rendering capabilities.
- `system-clock`, `system-timezone`, and `system-random` install native adapters.
Use the generated runtime catalog to determine what the loaded artifact actually supports. The
full wire contract, status semantics, callback rules, and C snippets are in
[the FFI protocol](../../docs/bindings/FFI_PROTOCOL.md).

## Platform Wrappers

- [Apple / Swift](https://github.com/Latias94/merman/blob/main/docs/bindings/APPLE_SWIFT.md)
- [Android / Kotlin](https://github.com/Latias94/merman/blob/main/docs/bindings/ANDROID_JNI.md)
- [Flutter / Dart](https://github.com/Latias94/merman/blob/main/docs/bindings/FLUTTER_DART_FFI.md)
- [Python / UniFFI](https://github.com/Latias94/merman/blob/main/docs/bindings/PYTHON_UNIFFI.md)

## License And Notices

Merman is available under MIT or Apache-2.0. The crate archive includes the release-matched
`LICENSE-MIT` and `LICENSE-APACHE` texts. Project-wide source provenance and third-party legal
materials are recorded in [`THIRD_PARTY_NOTICES.md`](../../THIRD_PARTY_NOTICES.md).
