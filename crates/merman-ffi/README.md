# merman-ffi

Embed Merman's headless Mermaid parser, analyzer, layout engine, and render/export backends in a C-compatible host. It does not require a browser or JavaScript runtime.

> **Native ABI 3:** this crate exposes native ABI 3 only. The ABI 2 entry points and records shipped in `0.8.0-alpha.2` and `0.8.0-alpha.3` were removed; rebuild those hosts against the ABI 3 header and library from the same Merman release.

## Choose A Native Surface

| Host | Recommended entry point |
| --- | --- |
| C or C++ | This crate and its generated `merman.h`. |
| Python | The [`merman` PyPI package](https://pypi.org/project/merman/). |
| Swift on Apple platforms | The [Merman Swift package](https://github.com/Latias94/merman/tree/main/platforms/apple#readme). |
| Flutter | The [`merman` pub package](https://pub.dev/packages/merman). |
| Kotlin on Android | The [Merman Android AAR](https://github.com/Latias94/merman/tree/main/platforms/android#readme). |
| Rust | The [`merman` facade](https://crates.io/crates/merman). |

Use the C ABI directly when the host needs a language-neutral function table or owns a custom binding. The platform packages provide safer language-native ownership and error types.

## Build From Source

`merman-ffi` is published as a source crate. Merman does not currently publish a generic prebuilt C SDK; the canonical native artifact profile defines a reproducible host build.

From a repository checkout, build the complete native SDK recipe with:

```sh
python3 scripts/artifact_profile_recipe.py c-abi-native --build --locked
```

`merman-ffi` has no default features. Choose an explicit capability set:

The committed `c-abi-native` artifact profile owns the complete host C ABI recipe, and Flutter owns separate C ABI target-set recipes. The Kotlin Android AAR uses the independent, internal `merman-android-jni` crate instead of this crate.

```sh
# Complete C ABI reference artifact: SVG, analysis, ASCII, exports, layouts, math, and native adapters.
# This explicit recipe includes the EPL-2.0 ELK and OFL-1.1 font closures; ship the matching notices.
cargo build -p merman-ffi --profile native-sdk --no-default-features --features svg,analysis,ascii,png,jpeg,pdf,layout-cytoscape,layout-elk,math,native-runtime

# A semantic-only embedding.
cargo build -p merman-ffi --release --no-default-features

# A focused SVG artifact.
cargo build -p merman-ffi --release --no-default-features --features svg
```

The crate produces `cdylib`, `staticlib`, and `rlib` artifacts. Include the release-matched [`include/merman.h`](include/merman.h) instead of copying a header from a moving branch.

## Run The C Example

On a Unix-like host, build and run the minimal discovery-and-render example from the same checkout:

```sh
profile_dir="$(python3 scripts/artifact_profile_recipe.py c-abi-native --field profile)"
library_dir="target/$profile_dir"

cc -I crates/merman-ffi/include \
  crates/merman-ffi/examples/render_svg.c \
  -L "$library_dir" -lmerman_ffi \
  -Wl,-rpath,"$PWD/$library_dir" \
  -o target/merman-ffi-render-svg

target/merman-ffi-render-svg
```

Windows hosts use the same header and example with the release-matched DLL/import library and the host compiler's normal DLL search configuration.

## ABI 3 Model

The C surface has one exported discovery symbol:

```c
MermanNativeStatus merman_get_native_api(
    const MermanNativeApiRequest *request,
    MermanNativeApi *out_api
);
```

The host supplies `MERMAN_NATIVE_ABI_VERSION` and `MERMAN_NATIVE_ABI_MINIMUM_PREFIX_LAYOUT_DIGEST`, then receives the common prefix of a size-tagged function table. `MermanNativeApi.struct_size` is the host capacity on input and the largest complete producer prefix safely initialized within that capacity on output. The returned minimum-prefix digest is the compatibility key, the full descriptor digest records producer provenance, and the capability catalog digest identifies the loaded artifact. Every other record requires its exact generated size. Use `MERMAN_NATIVE_RESULT_INIT` to fully zero-initialize each result before a producing call; setting only `struct_size` in otherwise uninitialized storage is invalid. The generated C header and release C smoke tests carry the compile-run layout fingerprint, so applications should not implement a second runtime offset probe.

The ABI 3 minimum prefix ends at `engine_new_with_services` (function slot `6`). Operation control
is an additive current-contract extension: `operation_control_new`, `operation_control_cancel`, and
`operation_control_release` occupy slots `7`, `8`, and `9`, while
`execute_collect_controlled` occupies slot `10`. The generated prefix-size macros identify each
complete appended prefix. Release-matched consumers require the complete table through
`MERMAN_NATIVE_API_EXECUTE_COLLECT_CONTROLLED_PREFIX_SIZE`; the smaller minimum prefix remains the
layout-compatibility key rather than a supported reduced host surface. The append-only function
preserves the original ABI 3 request-record layout.

Caller-supplied record pointers must be naturally aligned. Every record and reachable byte range
must remain readable, live, and immutable for the complete call, except for declared output
storage, which must remain writable. Merman can reject detectable size, alignment, range, and
overlap faults; dangling, unreadable, or concurrently mutated memory remains a C caller contract
violation and is not promised a typed error.

Every operation follows the same path:

1. Call `merman_get_native_api`.
2. Create an engine token with `api.engine_new`, or use `api.engine_new_with_services` when the
   engine owns Iconify packs.
3. Optionally create an operation-control token with `api.operation_control_new`.
4. Set `MermanNativeOperationRequest.operation` to the requested operation enum. Call
   `api.execute_collect` without a caller control, or call `api.execute_collect_controlled` with a
   borrowed control token.
5. Release every written result with `api.result_free`.
6. Release any operation-control token with `api.operation_control_release`, then close the engine
   token with `api.engine_try_close`.

Initialize the `out_engine` value to zero before either constructor; a nonzero value is rejected
without being overwritten. `MERMAN_NATIVE_OPERATION_NONE` is a defined metadata/result sentinel
and is not executable: `execute_collect` returns invalid argument with the generic error kind.
Unknown numeric operation codes continue to return unsupported operation with
`unknown-operation`.

`api.engine_new_with_services` embeds the existing `MermanNativeEngineConfig` in a
`MermanNativeEngineServicesConfig` and optionally accepts a contiguous array of
`MermanNativeIconPack` records. Each pack borrows IconifyJSON bytes and an optional UTF-8
registration-name override only until construction returns. Success retains one immutable,
validated registry; the caller can immediately release all pack records and byte buffers. The
constructor never invokes the text-measurement callback. Artifacts without `svg` accept an empty
service list and return typed `missing-capability` for nonempty icon packs without reading them.

Engine options select runtime state explicitly. Omitting `runtime_policy` uses Merman's deterministic clock, UTC time zone, and fixed random seed, even when `native-runtime` is compiled. Set `{ "runtime_policy": "native" }` only when the operation should consult the compiled system clock, time-zone, and random adapters. The binding feature is atomic: `native-runtime` compiles all three adapters together, while the runtime catalog still reports the concrete IDs `system-clock`, `system-timezone`, and `system-random`. If the complete set is unavailable, engine creation returns the typed unsupported-operation status. Successful generic operation metadata includes `"runtime_policy":"deterministic"` or `"runtime_policy":"native"`.

`MermanNativeOperationRequest.options_json` accepts the same generic options document for one operation. Request objects recursively override the reusable engine baseline, while omitted nested values remain inherited and the baseline itself is not mutated. `runtime_policy` remains constructor-owned and is rejected in request options.

Include [`include/merman_resource_contract.h`](include/merman_resource_contract.h) when constructing resource options in C. It projects the current Options JSON schema version plus stable profile and limit strings, minimum values, and override eligibility; the host still owns JSON serialization.

`api.runtime_catalog` returns the flat schema-1 catalog with package version, supported options and binding-payload schemas, named metadata IDs, transport-callable capability/operation/output/system-adapter IDs, registry facts, resource-to-operation mappings, and text-measurement providers. The native clock, time-zone, and random adapters appear only as a complete selectable set, and timing instrumentation is never exposed through binding JSON. The catalog is the source of truth for the loaded artifact; do not infer availability from Cargo feature names.

The generic operation enums cover SVG, PNG, JPEG, PDF, ASCII, semantic JSON, layout JSON, analysis, validation, and URI-requiring document analysis. An unavailable operation returns the typed `MERMAN_NATIVE_STATUS_UNSUPPORTED_OPERATION` result rather than exposing a separate phantom API. Failure JSON schema `1` distinguishes `unknown-operation`, `missing-capability`, `reentrant-call`, and `busy` from `generic`; only `missing-capability` carries a non-null descriptor `capability_id`. Parser and ASCII failures may carry additive `details.diagnostic` fields (`code`, bounded byte `span`, `field`, and `diagram_type`) without embedding complete source text.

## Cooperative Operation Control

`MermanNativeOperationControlToken` is an opaque nonzero token with its own token domain. Create one
with `api.operation_control_new(timeout_ms, has_timeout_ms, &control, &result)`, initializing
`control` to zero and `result` with `MERMAN_NATIVE_RESULT_INIT`. `has_timeout_ms == 0` creates a
control without a deadline and ignores `timeout_ms`; `has_timeout_ms == 1` installs a relative
monotonic deadline, including an immediate deadline when `timeout_ms == 0`.

Pass the token explicitly to
`api.execute_collect_controlled(engine, control, &request, &result)`. A zero control selects the
ordinary active-control behavior. A nonzero value is borrowed for the call: execution clones its
shared state before synchronous work, does not release the token, and holds no control-registry
lock while rendering. `api.operation_control_cancel(control)` atomically requests cancellation for
a live token. `api.operation_control_release(control)` removes only the registry token; an
operation that already cloned the control remains memory-safe and continues observing its
cancellation/deadline state until it returns.

Cancellation is cooperative, not thread termination. Parsing, layout, adapters, SVG
post-processing, and export observe the request at explicit checkpoints. An opaque synchronous
backend or host callback cannot be forcefully interrupted; cancellation is observed after that
call returns to a checkpoint. A cancelled operation returns
`MERMAN_NATIVE_STATUS_CANCELLED` (`17`), publishes no partial output, and carries additive failure
JSON at `details.cancellation`:

```json
{
  "status": 17,
  "status_name": "cancelled",
  "kind": "generic",
  "details": {
    "cancellation": {
      "reason": "requested",
      "phase": "layout"
    }
  }
}
```

The other stable reason is `deadline_exceeded`. Cancellation is distinct from
`MERMAN_NATIVE_STATUS_RESOURCE_LIMIT_EXCEEDED`: resource budgets bound admitted work, while an
operation control lets a host stop work that has become obsolete.

[`examples/render_svg.c`](examples/render_svg.c) is the minimal discovery-and-render program.
[`examples/render_svg_engine.c`](examples/render_svg_engine.c) shows one engine constructed with
both a host text-measurement callback and a borrowed Iconify pack.

## Ownership And Callbacks

`MermanNativeResult.data` and `metadata_or_error_json` are Merman-owned only after Merman has written the result, and only until `api.result_free(&result)`. Ownership is identified by a process-lifetime monotonic nonzero `allocation_token`, never by the nested buffer pointers or record address. Moving the complete result transfers ownership when the source is cleared and no duplicate live token remains. Zero, unknown, stale, and random tokens release nothing. Fully zero-initialize every result before a producing call, release every Merman-written result including failures before reuse, and never pass its buffers to a host allocator. Inputs and callback request slices are borrowed for the call only.

Engine, operation-control, and result-allocation values are opaque nonzero tokens, not pointers.
They use disjoint low-bit domains while preserving the sign bit, so valid tokens remain positive
in signed 64-bit language projections. This catches accidental cross-kind use but does not
authenticate a caller or isolate hostile same-process tenants. `engine_try_close` never waits: it returns
`MERMAN_NATIVE_STATUS_BUSY` while an operation is active and
`MERMAN_NATIVE_STATUS_REENTRANT_CALL` while the engine is inside its host callback, retaining the
token in both cases. A successful close permanently prevents new admissions before retiring the
token and is the point after which the host may release the immutable callback and `user_data`.
Callback-free engines admit concurrent operations; callback-configured engines reject a competing
operation with the typed `busy` failure.

Merman provides deterministic vendored text measurement by default. A preview host can set `MermanNativeEngineConfig.text_measure` to measure with its actual display font stack. The callback is synchronous; return `handled = 0` when a request cannot be answered faithfully and Merman will fall back for that request. A callback must not unwind, throw, propagate SEH, call `longjmp`, or otherwise cross the ABI through a non-local exit; catch host-language failures and return `MERMAN_NATIVE_STATUS_CALLBACK_ERROR`. Cancellation does not preempt a callback: the callback must return normally before the renderer can observe the control at its next checkpoint. The operation/result-kind and request vocabulary contracts are generated in [`include/merman_text_measurement_abi.h`](include/merman_text_measurement_abi.h).

## Feature Selection

There is intentionally no `complete-svg` aggregate in this ABI crate. Select direct leaves so the
source and artifact closure stays visible to the host build; `layout-elk` is an explicit EPL-2.0
choice and `math` brings the separately licensed RaTeX/font materials. The Cargo feature list is
not itself a complete legal notice; use the exact artifact recipe and release-matched notices for
the library you distribute.

The public feature names describe callable capabilities:

- `svg`, `analysis`, and `ascii` enable their corresponding operation families.
- `png`, `jpeg`, and `pdf` add real binary output operations.
- `layout-cytoscape`, `layout-elk`, and `math` add their rendering capabilities.
- `native-runtime` installs the complete system clock, time-zone, and random adapter set. This
  binding-owned feature is intentionally atomic; the runtime catalog still reports the concrete
  adapter IDs `system-clock`, `system-timezone`, and `system-random`.

This crate exports only the native C ABI discovery surface. Android JNI transport code lives in the internal `merman-android-jni` crate, so C ABI artifacts cannot acquire JNI exports through a feature combination.

Use the generated runtime catalog to determine what the loaded artifact actually supports. The full wire contract, status semantics, callback rules, and C snippets are in [the FFI protocol](https://github.com/Latias94/merman/blob/main/docs/bindings/FFI_PROTOCOL.md); ABI 2 and pre-freeze ABI 3 hosts must follow [the ABI 3 migration guide](https://github.com/Latias94/merman/blob/main/docs/bindings/ABI3_MIGRATION.md).

## Platform Wrappers

- [Apple / Swift](https://github.com/Latias94/merman/blob/main/docs/bindings/APPLE_SWIFT.md)
- [Android / Kotlin](https://github.com/Latias94/merman/blob/main/docs/bindings/ANDROID_JNI.md)
- [Flutter / Dart](https://github.com/Latias94/merman/blob/main/docs/bindings/FLUTTER_DART_FFI.md)
- [Python / UniFFI](https://github.com/Latias94/merman/blob/main/docs/bindings/PYTHON_UNIFFI.md)

## License And Notices

Merman is available under MIT or Apache-2.0. The crate archive includes the release-matched
`LICENSE-MIT` and `LICENSE-APACHE` texts. A selected FFI artifact may additionally contain EPL-2.0
ELK code and OFL-1.1 math fonts; distribute the artifact-specific notices and source provenance
with that build. Project-wide materials are recorded in
[`THIRD_PARTY_NOTICES.md`](https://github.com/Latias94/merman/blob/main/THIRD_PARTY_NOTICES.md).
