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
# Complete C ABI reference artifact; platform prebuilts use their own default SKU.
cargo build -p merman-ffi --profile native-sdk --no-default-features --features svg,analysis,ascii,png,jpeg,pdf,layout-cytoscape,layout-elk,math,native-runtime

# Smaller explicit artifacts.
cargo build -p merman-ffi --release --no-default-features --features svg
cargo build -p merman-ffi --release --no-default-features --features analysis
```

`merman-ffi` produces `cdylib`, `staticlib`, and `rlib`. C and C-compatible hosts must ship a
header and native library from the same Merman release. `native-runtime` is an atomic binding
feature that compiles the system clock, time-zone, and random adapters together; partial native
runtime feature sets are not exposed by this crate. Cargo features describe a build request, while
the loaded artifact's runtime catalog describes what is actually callable through the concrete
`system-clock`, `system-timezone`, and `system-random` adapter IDs.

## Discovery

`merman_get_native_api` is the sole C ABI entry symbol. It returns the common prefix of a
size-tagged function table only after the host proves it understands the declared ABI version and
the descriptor-derived minimum-prefix layout.

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

`api.struct_size` is input capacity and reports the largest complete producer prefix safely
initialized within that capacity on success. This capacity negotiation is a memory-safety boundary,
not a promise that historical partial tables remain supported. Release consumers use the complete
generated table, require every function pointer they call, and never reconstruct function names or
dynamically look up per-operation exports.

The current table includes `metadata_collect` at function-slot code `5` and
`engine_new_with_services` at code `6`. Release consumers require both functions and must not fall
back to the older constructor or silently discard constructor services.

The returned digests have separate roles:

- `minimum_prefix_layout_digest` is the compatibility key checked by discovery. Its structure is
  derived from the descriptor's ABI 3 minimum records, codes, callback, and function slots.
- `full_descriptor_digest` identifies the producer's complete descriptor. It can change after
  additions without changing the descriptor-selected minimum prefix.
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

Every caller-supplied record pointer must be naturally aligned for its declared C type. All record
storage and every byte range reachable through a record must remain readable, live, and immutable
for the complete call, except for declared output storage, which must remain writable and live.
Merman rejects detectable size, alignment, representable-range, and documented-overlap faults
before typed access. It cannot safely probe dangling or unreadable pointers and cannot convert
concurrent mutation into a typed status; those remain caller contract violations. The void
`result_free` operation treats a safely allocated misaligned result record as invalid and releases
nothing.

## Runtime Catalog

`api.runtime_catalog` writes a `MermanNativeResult` whose `metadata_or_error_json` contains the
flat schema-1 JSON catalog. This abridged example shows every top-level field:

```json
{
  "schema_version": 1,
  "transport_api_version": 3,
  "package_version": "...",
  "options_schema_versions": [2],
  "payload_schemas": [
    { "id": "binding-result", "version": 1 },
    { "id": "operation-metadata", "version": 1 }
  ],
  "metadata_ids": ["supported-diagrams", "..."],
  "capabilities": {
    "capability_ids": ["..."],
    "operation_ids": ["..."],
    "output_ids": ["..."],
    "system_adapter_ids": ["..."],
    "text_measurement": null
  },
  "output_contracts": [
    {
      "id": "svg",
      "media_type": "image/svg+xml",
      "system_fonts": null,
      "embedded_images": null
    }
  ],
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

## Named Metadata

`api.metadata_collect` accepts one borrowed UTF-8 metadata ID and writes its JSON document to
`MermanNativeResult.metadata_or_error_json`. The current IDs are `supported-diagrams`,
`ascii-capabilities`, `diagram-family-capabilities`, `lint-rule-catalog`, `supported-themes`, and
`presentation-catalog`. This generic appended slot restores catalog access without adding
direct exports or placing large detail catalogs in the runtime catalog.

A successful metadata result uses operation `MERMAN_NATIVE_OPERATION_NONE`, has empty `data`, and
owns a nonzero allocation token. Unknown IDs return `MERMAN_NATIVE_STATUS_INVALID_ARGUMENT` with a
structured failure result. A known catalog whose required capability is not compiled, such as the
lint catalog without `analysis`, returns the ordinary typed `missing-capability` failure. Free every
written success or failure with `api.result_free`.

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

Initialize `out_engine` to zero before `engine_new`. The `config` record, its `options_json` byte
storage, `out_engine`, and `out_result` must be pairwise disjoint; obvious overlap is rejected with
`MERMAN_NATIVE_STATUS_INVALID_ARGUMENT` before Merman writes either output. A nonzero
`out_engine` is likewise rejected and is not overwritten.

### Constructor-Owned Icon Services

The appended code-6 constructor combines the existing options/callback record with borrowed
Iconify packs:

```c
static const uint8_t iconify_json[] =
    "{\"prefix\":\"app\",\"icons\":{\"rocket\":{\"body\":\"<path d=\\\"M0 0H16V16H0z\\\"/>\"}}}";

MermanNativeIconPack pack = {
    .struct_size = MERMAN_NATIVE_STRUCT_SIZE(MermanNativeIconPack),
    .json = {
        .struct_size = MERMAN_NATIVE_STRUCT_SIZE(MermanNativeSlice),
        .data = iconify_json,
        .len = sizeof(iconify_json) - 1,
    },
    .registration_name = {
        .struct_size = MERMAN_NATIVE_STRUCT_SIZE(MermanNativeSlice),
    },
};
MermanNativeEngineServicesConfig services = {
    .struct_size = MERMAN_NATIVE_STRUCT_SIZE(MermanNativeEngineServicesConfig),
    .engine_config = config,
    .icon_packs = &pack,
    .icon_pack_count = 1,
};

engine = 0;
result = (MermanNativeResult)MERMAN_NATIVE_RESULT_INIT;
status = api.engine_new_with_services(&services, &engine, &result);
api.result_free(&result);
```

`out_engine` must be zero and remains zero on every failure. The outer record, pack array, options,
pack JSON, registration names, and both outputs follow the generated non-overlap rules. Structural
record storage cannot overlap any nested byte slice; read-only byte slices may overlap each other,
so multiple logical packs may reuse one JSON buffer with different registration names. The
constructor checks the fixed 16-pack ceiling before array multiplication or access, validates all
size tags and slice shapes before parsing, never invokes the host callback, and publishes no token
until the complete immutable registry and engine are ready. Pack records and bytes are borrowed
only until return and may be released immediately after success.

An artifact exposing `svg` advertises the `icon-registry` constructor service in its runtime
catalog and validates nonempty packs. An artifact without `svg` still accepts an empty services
record, but may return typed `missing-capability` for any nonzero `icon_pack_count` without reading
or validating the pack array or nested pack slices. There is intentionally no mutable registry
handle and no registry-specific free function.

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

`MERMAN_NATIVE_OPERATION_NONE` is a defined non-executable sentinel used by catalog, metadata, and
constructor results. Passing it to `execute_collect` returns
`MERMAN_NATIVE_STATUS_INVALID_ARGUMENT` with the generic error kind. A numeric code outside the
known operation vocabulary instead returns `MERMAN_NATIVE_STATUS_UNSUPPORTED_OPERATION` with
`unknown-operation`.

`options_json` is a generic binding options document, not a format-specific envelope. Merman
recursively merges it over the engine configuration for this operation: request values replace
matching leaves, while omitted nested values remain inherited. The engine baseline is unchanged
after the call. Runtime-policy selection is engine-owned; a request containing `runtime_policy`
fails with `MERMAN_NATIVE_STATUS_OPTIONS_JSON_ERROR`.

## Results, Errors, And Ownership

The status return and `MermanNativeResult.status` agree. On successful generic operations, `data`
holds the requested bytes and `metadata_or_error_json` holds versioned operation metadata. Named
metadata calls instead place their JSON document in `metadata_or_error_json`. On failure,
`metadata_or_error_json` contains a structured UTF-8 error payload. `media_type` is a borrowed
static slice valid until the library unloads.

PNG and JPEG operation metadata includes the requested and effective raster plan. PDF operation
metadata includes the requested and effective filter-image plan. Resource ceilings may reduce the
effective scale without changing the operation status, so hosts that expose export sizing should
read `output_plan` instead of echoing request options.

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

Icon-registry construction failures add `details.icon_registry` with a stable `kind_id`, optional
`pack_index`, and a bounded registration name when safe to report. Fixed constructor ceilings also
add `details.resource` with the stable limit ID, phase, actual value, maximum, and
`constructor-fixed` profile. These are additive fields under the frozen five-kind error envelope.

The ABI 3 error-kind vocabulary is frozen and closed: `generic`, `unknown-operation`,
`missing-capability`, `reentrant-call`, and `busy`. Consumers should still treat an unknown kind as
`generic` and every unknown nonzero status as failure when diagnosing a malformed producer.
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

Engine values are opaque nonzero `uint64_t` tokens, not pointers. Engine and result-allocation
tokens use separate low-bit domains over independent monotonic counters. The sign bit is never set,
so every valid token remains positive when a generated language binding represents it as a signed
64-bit integer. Domain tagging rejects accidental cross-kind use; it is not authorization or
hostile same-process tenant isolation.
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

## Contract Evolution Rules

- Native ABI 3 is the current contract. Rebuild ABI 2, prerelease ABI 3, and partial-table consumers
  against this release-matched header; see [ABI 3 migration](ABI3_MIGRATION.md).
- Treat the ABI version and descriptor-derived minimum-prefix layout digest as the runtime
  compatibility check. Treat
  the full descriptor digest, capability catalog digest, package version, and generated
  C-consumer layout fingerprint as provenance evidence, not interchangeable compatibility keys.
- Function slots and codes may only be appended. Changing an existing wire layout requires a new
  ABI version.
- The descriptor and generated Rust/C/Dart projections are the single current ABI authority.
  Descriptor validation, the derived minimum-prefix layout digest, generated-file freshness, and
  current-header lifecycle tests cover layout, ownership, token, status, and operation semantics.
- A breaking semantic change is reviewed directly in `abi/merman-v3.json` and its generated
  artifacts; no second semantic snapshot or hand-maintained approval digest is required.
- A future generated header may add slots. Current consumers require the complete table they were
  generated against instead of maintaining hand-written historical fallbacks.
- Result ownership, token-domain/free behavior, callback non-local-exit prohibition, nonblocking
  close semantics, both constructors' storage separation and zero-output precondition,
  constructor-service ownership, caller-memory obligations, `NONE` handling, status-kind mappings,
  and the closed error-kind vocabulary are defined in the descriptor and exercised by generated
  consumer tests. Pre-release breaking changes update that authority directly instead of changing a
  parallel freeze file.
- Except for the API table's capacity negotiation, records require exact generated sizes. The
  package's C smoke test compiles and runs the current generated header; applications do not need
  to duplicate a runtime offset probe.
- Treat `MERMAN_NATIVE_RESULT_SCHEMA_VERSION`, error kind strings, and `capability_id` as one
  machine-readable failure contract.
- Treat diagnostics and analysis-facts payload schema versions as independent contracts.
- Ignore unknown JSON fields where a payload schema permits it, but never invent unknown native
  status codes, output IDs, capabilities, or record fields.
- Select an artifact's direct output features deliberately (`svg`, `analysis`, `ascii`, `png`,
  `jpeg`, `pdf`, `layout-cytoscape`, `layout-elk`, and `math`) and add `native-runtime` only when
  the complete native runtime policy is required. Smaller artifacts must advertise and enforce
  their actual output subset. The binding-owned aggregate is not a native ABI capability ID;
  runtime discovery continues to expose the concrete system adapter IDs.

See [`crates/merman-ffi/examples`](../../crates/merman-ffi/examples) for compilable C examples and
[`docs/bindings/HOST_TEXT_MEASUREMENT.md`](HOST_TEXT_MEASUREMENT.md) for platform measurement
guidance.
