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

`merman_get_native_api` is the sole C ABI entry symbol. It returns a size-tagged function table
only after the host proves it understands the declared ABI version and descriptor digest.

```c
MermanNativeSlice digest = {
    .struct_size = MERMAN_NATIVE_STRUCT_SIZE(MermanNativeSlice),
    .data = (const uint8_t *)MERMAN_NATIVE_ABI_LAYOUT_DESCRIPTOR_DIGEST,
    .len = strlen(MERMAN_NATIVE_ABI_LAYOUT_DESCRIPTOR_DIGEST),
};
MermanNativeApiRequest request = {
    .struct_size = MERMAN_NATIVE_STRUCT_SIZE(MermanNativeApiRequest),
    .expected_abi_version = MERMAN_NATIVE_ABI_VERSION,
    .expected_layout_descriptor_digest = digest,
};
MermanNativeApi api = {
    .struct_size = MERMAN_NATIVE_STRUCT_SIZE(MermanNativeApi),
};

if (merman_get_native_api(&request, &api) != MERMAN_NATIVE_STATUS_OK) {
    /* The header and library do not share ABI 3. */
}
```

The table supplies `runtime_catalog`, `engine_new`, `engine_free`, `execute_collect`, and
`result_free`. A host must verify each function pointer it will call. Do not reconstruct function
names or dynamically look up per-operation exports.

All public records begin with `struct_size`. Initialize caller-owned input records with
`MERMAN_NATIVE_STRUCT_SIZE(Type)`. `MermanNativeResult` is deliberately different: it is a
write-only output record. Initialize only its `struct_size` before a call, preferably with
`MERMAN_NATIVE_RESULT_INIT`; Merman never reads the remaining fields and writes the entire record.
The generated header and release C smoke test carry a compile-run layout fingerprint. Application
bindings should consume the generated declarations rather than implementing a second runtime
offset probe.

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
  "resources": { "schema_version": 1, "..." : "..." }
}
```

`capabilities` is the exact compiled subset. The catalog intentionally does not repeat the global
descriptor vocabulary; hosts should validate shape, sorted/unique IDs, and local relations without
maintaining a second hand-written capability table. The returned JSON is not wrapped in a
native-only envelope.

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

The closed kinds are `generic`, `unknown-operation`, `missing-capability`, and `reentrant-call`.
Unknown operation IDs or numeric codes use `unknown-operation` with a null `capability_id`. A valid operation whose backend is
absent uses `missing-capability` with the exact descriptor capability ID. Both cases retain status
`MERMAN_NATIVE_STATUS_UNSUPPORTED_OPERATION`, so consumers must not infer the distinction from
the numeric status or message text. `reentrant-call` is returned when a host callback re-enters or
retires the same engine; it is a distinct invalid-argument status so a host can reject that call
without confusing it with a missing renderer.

`data` and `metadata_or_error_json` are Merman-owned buffers only after Merman has written the
result record. Their ownership is registered against that exact `MermanNativeResult` address.
Call `api.result_free(&result)` after every operation that wrote a result, including errors and
engine-creation responses, and before reusing that same record. Do not copy or move a live result
and then free the copy: copying the fields does not transfer ownership, and only the original
record address can release its allocation. `result_free` never trusts the nested buffer pointers;
it reads only the size prefix, releases an allocation registered for the record address, and
clears the complete record. Calling it repeatedly, or on a full-size output record with only
`struct_size` initialized, is harmless. Never pass either buffer to a host allocator.

Engine values are opaque nonzero `uint64_t` tokens, not pointers. `api.engine_free(engine)` retires
the token; a subsequent call returns `MERMAN_NATIVE_STATUS_INVALID_ENGINE`. If a host retires an
engine while an operation is already running, that operation keeps its internal state until it
finishes, but the host must make no further calls using the token. `engine_free` is not a
quiescence barrier: the host must keep the configured text-measurement callback and `user_data`
valid until every operation started before retirement has returned.

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
panics, exceptions, malformed records, and wrong result kinds become typed callback failures or
per-request fallback rather than crossing the native boundary.

## Compatibility Rules

- Native ABI 3 is the current contract; ship headers and libraries from the same package.
- Treat ABI version, descriptor digest, record size, and the generated C-consumer layout fingerprint
  as one compatibility check. The package's C smoke test compiles and runs this fingerprint; an
  application does not need to duplicate a runtime offset probe.
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
