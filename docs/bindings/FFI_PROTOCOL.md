# Merman Native ABI 3 Protocol

Status: current contract

This document describes the current low-level C-compatible transport implemented by
`merman-ffi`. The generated header
[`crates/merman-ffi/include/merman.h`](../../crates/merman-ffi/include/merman.h) is the
authoritative wire definition. ABI 2 is removed and is not accepted by ABI 3 discovery.

## Build

The descriptor-owned `c-abi-native` profile is the exact recipe for the complete host C artifact
used by release examples and Flutter packaging.

```sh
# Full native SDK artifact.
cargo build -p merman-ffi --release --no-default-features --features svg,analysis,ascii,png,jpeg,pdf,layout-cytoscape,layout-elk,math,system-clock,system-timezone,system-random

# Smaller explicit artifacts.
cargo build -p merman-ffi --release --no-default-features --features svg
cargo build -p merman-ffi --release --no-default-features --features analysis
```

`merman-ffi` produces `cdylib`, `staticlib`, and `rlib`. C and C-compatible hosts must ship a
header and native library from the same Merman release. Cargo features describe a build request;
the loaded artifact's runtime catalog describes what is actually callable.

## Discovery

`merman_get_native_api` is the sole C ABI entry symbol. It returns the common prefix of a
size-tagged function table only after the host proves it understands the declared ABI version and
frozen minimum-prefix layout.

```c
MermanNativeSlice prefix_digest = {
    .struct_size = MERMAN_NATIVE_STRUCT_SIZE(MermanNativeSlice),
    .data = (const uint8_t *)MERMAN_NATIVE_ABI_MINIMUM_PREFIX_LAYOUT_DIGEST,
    .len = strlen(MERMAN_NATIVE_ABI_MINIMUM_PREFIX_LAYOUT_DIGEST),
};
MermanNativeApiRequest request = {
    .struct_size = MERMAN_NATIVE_STRUCT_SIZE(MermanNativeApiRequest),
    .expected_abi_version = MERMAN_NATIVE_ABI_VERSION,
    .expected_minimum_prefix_layout_digest = prefix_digest,
};
MermanNativeApi api = {
    .struct_size = MERMAN_NATIVE_STRUCT_SIZE(MermanNativeApi),
};

if (merman_get_native_api(&request, &api) != MERMAN_NATIVE_STATUS_OK) {
    /* The header and library do not share ABI 3. */
}
```

`api.struct_size` is input capacity and reports the producer's full table size on success. The ABI 3
minimum prefix ends after the five ordered slots `runtime_catalog`, `engine_new`,
`engine_try_close`, `execute_collect`, and `result_free`. A newer producer may append fields after
that prefix. An older consumer supplies its own table capacity, consumes only fields that fit in
that capacity, and verifies every function pointer it calls. Do not reconstruct function names or
dynamically look up per-operation exports.

The returned digests have separate roles:

- `minimum_prefix_layout_digest` is the compatibility key checked by discovery. Its frozen
  structure includes the ABI 3 minimum records, codes, callback, and five function slots.
- `full_descriptor_digest` identifies the producer's complete descriptor. It can change after
  append-only additions without making the frozen prefix incompatible.
- `capability_catalog_digest` identifies the loaded artifact's runtime capability catalog. Two
  ABI-compatible artifacts can intentionally have different capability digests.

All public records begin with `struct_size`. Except for `MermanNativeApi`, which uses `struct_size`
as table capacity, ABI 3 records require the exact generated size. Initialize caller-owned input
records with `MERMAN_NATIVE_STRUCT_SIZE(Type)`. Fully zero-initialize every
`MermanNativeResult` before a producing call, preferably with `MERMAN_NATIVE_RESULT_INIT`; setting
only the first word in otherwise uninitialized storage is invalid because Merman must safely
inspect the ownership token. The generated header and release C smoke tests carry the compile-run
layout fingerprint. Application bindings should consume the generated declarations rather than
implementing a second runtime offset probe.

## Runtime Catalog

`api.runtime_catalog` writes a `MermanNativeResult` whose `metadata_or_error_json` contains the
flat schema-1 JSON catalog:

```json
{
  "schema_version": 1,
  "transport_api_version": 3,
  "package_version": "...",
  "capabilities": {
    "capability_ids": ["..."],
    "operation_ids": ["..."],
    "output_ids": ["..."],
    "system_adapter_ids": ["..."],
    "text_measurement": null
  },
  "registry": { "diagram_family_count": 35 },
  "resources": { "general_binding_default_profile": "interactive", "..." : "..." }
}
```

`capabilities` is the exact subset callable through this transport. Ordinary capability, operation,
and output IDs reflect compiled endpoints. System adapter IDs report clock, time-zone, and
randomness only when the transport's all-or-nothing `native` policy is selectable; incomplete
native sets and externally unified timing instrumentation are not callable through binding JSON
and are omitted. The catalog intentionally does not repeat the global descriptor vocabulary; hosts
should validate shape, sorted/unique IDs, and local relations without maintaining a second
hand-written capability table. The returned JSON is not wrapped in a native-only envelope.

## Generic Operations

Create an opaque engine token once for a chosen options document:

```c
MermanNativeEngineConfig config = {
    .struct_size = MERMAN_NATIVE_STRUCT_SIZE(MermanNativeEngineConfig),
    .options_json = {
        .struct_size = MERMAN_NATIVE_STRUCT_SIZE(MermanNativeSlice),
        .data = NULL,
        .len = 0,
    },
    .text_measure = NULL,
    .text_measure_user_data = NULL,
};
MermanNativeEngineToken engine = 0;
MermanNativeResult result = MERMAN_NATIVE_RESULT_INIT;
MermanNativeStatus status = api.engine_new(&config, &engine, &result);
api.result_free(&result);
```

Then select an operation with `MermanNativeOperationRequest.operation` and execute the same route
for every operation:

```c
static const uint8_t source[] = "flowchart TD\nA --> B";
static const uint8_t options[] = "{\"svg\":{\"diagram_id\":\"request\"}}";
MermanNativeOperationRequest request = {
    .struct_size = MERMAN_NATIVE_STRUCT_SIZE(MermanNativeOperationRequest),
    .operation = MERMAN_NATIVE_OPERATION_SVG,
    .source = {
        .struct_size = MERMAN_NATIVE_STRUCT_SIZE(MermanNativeSlice),
        .data = source,
        .len = sizeof(source) - 1,
    },
    .uri = { .struct_size = MERMAN_NATIVE_STRUCT_SIZE(MermanNativeSlice) },
    .options_json = {
        .struct_size = MERMAN_NATIVE_STRUCT_SIZE(MermanNativeSlice),
        .data = options,
        .len = sizeof(options) - 1,
    },
};
result = (MermanNativeResult)MERMAN_NATIVE_RESULT_INIT;
status = api.execute_collect(engine, &request, &result);
```

Operation enums are generated from [`abi/merman-v3.json`](../../abi/merman-v3.json): SVG, PNG, JPEG,
PDF, ASCII, semantic JSON, layout JSON, analysis JSON, analysis facts, validation JSON, and the two
URI-requiring document analysis operations. `MERMAN_NATIVE_OPERATION_*_REQUIRES_URI_*` identifies
the two document operations; pass an empty URI for all other operations. When a requested
operation is not compiled, the result has `MERMAN_NATIVE_STATUS_UNSUPPORTED_OPERATION`.

`options_json` is a generic binding options document, not a format-specific envelope. Merman
recursively merges it over the engine configuration for this operation: request values replace
matching leaves, while omitted nested values remain inherited. The engine baseline is unchanged
after the call. Runtime-policy selection is engine-owned; a request containing `runtime_policy`
fails with `MERMAN_NATIVE_STATUS_OPTIONS_JSON_ERROR`.

## Results, Errors, And Ownership

The status return and `MermanNativeResult.status` agree. On success, `data` holds the requested
bytes and `metadata_or_error_json` holds versioned operation metadata. On failure,
`metadata_or_error_json` contains a structured UTF-8 error payload. `media_type` is a borrowed
static slice valid until the library unloads.

Failure payload schema `MERMAN_NATIVE_RESULT_SCHEMA_VERSION == 1` always carries `kind` and a
nullable `capability_id`:

```json
{
  "version": 1,
  "ok": false,
  "status": 7,
  "status_name": "unsupported-operation",
  "kind": "missing-capability",
  "capability_id": "svg",
  "message": "SVG rendering requires the svg feature"
}
```

The closed kinds are `generic`, `unknown-operation`, `missing-capability`, `reentrant-call`, and
`busy`.
Unknown operation IDs or numeric codes use `unknown-operation` with a null `capability_id`. A valid operation whose backend is
absent uses `missing-capability` with the exact descriptor capability ID. Both cases retain status
`MERMAN_NATIVE_STATUS_UNSUPPORTED_OPERATION`, so consumers must not infer the distinction from
the numeric status or message text. `reentrant-call` uses
`MERMAN_NATIVE_STATUS_REENTRANT_CALL` when a host callback re-enters or tries to close the same
engine. `busy` uses `MERMAN_NATIVE_STATUS_BUSY` when a callback-configured engine cannot admit a
competing operation or any engine still has an active operation during a close attempt.

`data` and `metadata_or_error_json` are Merman-owned buffers only after Merman has written the
result record. Every Merman-written result owns a process-lifetime monotonic, nonzero
`allocation_token`; tokens are never reused. Call `api.result_free(&result)` after every producing
call that wrote a result, including failures and engine-creation responses, and before reusing that
record. Moving the complete record transfers ownership, provided the source is cleared and no
second live copy remains. `result_free` trusts only `allocation_token`, never nested buffer
pointers. Zero, unknown, random, or already-freed tokens release nothing and only clear the
supplied record. Copying a live token and using the duplicate to free another live record is
outside the same-process hostile-memory threat boundary. Never pass either buffer to a host
allocator.

If the process-lifetime token space itself is exhausted, the producing call returns
`MERMAN_NATIVE_STATUS_INTERNAL_ERROR` and leaves a valid zero-initialized result record untouched;
it cannot write a conforming owned result without a nonzero token.

Engine values are opaque nonzero `uint64_t` tokens, not pointers.
`api.engine_try_close(engine)` never waits:

- If a host callback is active, it returns `MERMAN_NATIVE_STATUS_REENTRANT_CALL`.
- If another operation is active, it returns `MERMAN_NATIVE_STATUS_BUSY`.
- In both failure cases the token remains valid and the host can retry after the operation returns.
- On success, admission is permanently closed before the token is retired. No operation holding a
  previously acquired internal reference can enter after that point, and later calls return
  `MERMAN_NATIVE_STATUS_INVALID_ENGINE`.

The callback and `user_data` are immutable constructor state. The host may release them only after
`engine_try_close` succeeds.

## Host Text Measurement

`MermanNativeEngineConfig.text_measure` is an optional synchronous callback. It receives borrowed
`MermanNativeTextMeasureRequest` fields valid only during the callback and writes a size-tagged
`MermanNativeTextMeasureResult`. The generated
[`merman_text_measurement_abi.h`](../../crates/merman-ffi/include/merman_text_measurement_abi.h)
defines the independent text-measurement protocol version, 19 operation codes, and required result
kinds.

Use the same display font system that will render the final SVG. A host that cannot answer an
operation accurately should initialize the result, set `handled = 0`, and return
`MERMAN_NATIVE_STATUS_OK`; Merman falls back to its vendored measurer for that request. While a
host callback is active, any thread that re-enters or retires that same engine receives
`MERMAN_NATIVE_STATUS_REENTRANT_CALL`; calls using other engines remain independent. Callback
records, returned statuses, and result values are validated by the shared host-measurement decoder;
malformed responses become typed callback failures and use the configured fallback.

The callback itself must return normally. It **MUST NOT** unwind, throw a C++ or foreign-language
exception, propagate SEH, call `longjmp`, or otherwise perform a non-local exit across the ABI
boundary. Merman can convert only a status value that the callback actually returns; it cannot
catch a foreign exception. A language binding must catch callback failures on the host side and
return `MERMAN_NATIVE_STATUS_CALLBACK_ERROR`. In C++17 and newer, the generated callback and
function-pointer types are declared `noexcept` to make this contract visible to the type system.

## Compatibility Rules

- Native ABI 3 is the current contract. Rebuild ABI 2 and pre-freeze ABI 3 consumers against this
  header; see [ABI 3 migration](ABI3_MIGRATION.md).
- Treat the ABI version and minimum-prefix layout digest as the runtime compatibility check. Treat
  the full descriptor digest, capability catalog digest, package version, and generated
  C-consumer layout fingerprint as provenance evidence, not interchangeable compatibility keys.
- Function slots and codes may only be appended. The frozen minimum prefix cannot change within
  ABI 3; changing its layout requires ABI 4.
- Except for the API table's capacity negotiation, records require exact generated sizes. The
  package's C smoke tests compile and run both the current header and the frozen minimum-prefix
  consumer; applications do not need to duplicate a runtime offset probe.
- Treat `MERMAN_NATIVE_RESULT_SCHEMA_VERSION`, error kind strings, and `capability_id` as one
  machine-readable failure contract.
- Treat diagnostics and analysis-facts payload schema versions as independent contracts.
- Ignore unknown JSON fields where a payload schema permits it, but never invent unknown native
  status codes, output IDs, capabilities, or record fields.
- Select an artifact's direct leaf features deliberately (`svg`, `analysis`, `ascii`, `png`,
  `jpeg`, `pdf`, `layout-cytoscape`, `layout-elk`, `math`, and the relevant system adapters);
  smaller artifacts must advertise and enforce their actual output subset. Cross-transport
  preset names are not part of the native ABI contract.

See [`crates/merman-ffi/examples`](../../crates/merman-ffi/examples) for compilable C examples and
[`docs/bindings/HOST_TEXT_MEASUREMENT.md`](HOST_TEXT_MEASUREMENT.md) for platform measurement
guidance.
