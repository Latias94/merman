# Native ABI 3 Migration

Native ABI 3 freezes a single discovery symbol, a five-slot minimum function-table prefix, and the
published six-slot prefix that adds `metadata_collect`. The published `0.8.0-alpha.3` packages used
ABI 2; the current source uses ABI 3. ABI 2 and earlier pre-freeze ABI 3 drafts are intentionally
incompatible, while the reviewed ABI 3 prefix remains append-only. Rebuild every C, C++, Dart FFI,
or custom native host against the release-matched generated headers.

## Required Host Changes

1. Resolve only `merman_get_native_api`. Per-operation exports and ABI 2 symbols no longer exist.
2. Send `MERMAN_NATIVE_ABI_VERSION` and `MERMAN_NATIVE_ABI_MINIMUM_PREFIX_LAYOUT_DIGEST` in `MermanNativeApiRequest`.
3. Set `MermanNativeApi.struct_size` to the host table capacity. On success, Merman writes only complete fields that fit and reports the largest safely initialized prefix in that field.
4. Use the five ordered prefix slots: `runtime_catalog`, `engine_new`, `engine_try_close`, `execute_collect`, and `result_free`.
5. Fully zero-initialize `MermanNativeResult` with `MERMAN_NATIVE_RESULT_INIT` before every producing call.
6. Free each Merman-written result through its nonzero `allocation_token`. Moving the complete result transfers ownership only when the source is cleared and no duplicate live token remains.
7. Replace unconditional engine destruction with `engine_try_close`. Retry after `MERMAN_NATIVE_STATUS_BUSY`; treat `MERMAN_NATIVE_STATUS_REENTRANT_CALL` as an illegal close from an active callback; release callback state only after `MERMAN_NATIVE_STATUS_OK`.
8. Keep the callback and `user_data` immutable for the engine lifetime. Create another engine to use another callback.
9. Ensure callbacks return normally. Catch every host-language exception before it crosses C, C++, Dart FFI, or another foreign boundary and return `MERMAN_NATIVE_STATUS_CALLBACK_ERROR`.
10. Use the generated text-measurement constants in `merman_text_measurement_abi.h`. Do not hand-code wrap mode, direction, white-space, phase, operation, or result-kind numbers.
11. Treat fields after `result_free` as optional appended slots. `metadata_collect` is the published
    slot at code `5`; check the returned producer table size and pointer before calling it, then free
    every written metadata result.
12. The current table appends `engine_new_with_services` at code `6`. Use it only when the returned
    table reaches `MERMAN_NATIVE_API_ENGINE_NEW_WITH_SERVICES_PREFIX_SIZE`. Empty services may fall
    back to `engine_new` for an older producer; **non-empty icon-pack services** must fail
    explicitly rather than being ignored. A text measurer can still use the legacy
    callback-bearing constructor when that constructor is present; it is not silently dropped.

## Digest Roles

`minimum_prefix_layout_digest` is the only descriptor digest used to accept or reject ABI 3 discovery. It covers the frozen records, status and operation vocabularies, callback shape, and five-slot table prefix.

`full_descriptor_digest` identifies the complete producer descriptor. Append-only codes or slots can change it while remaining compatible with the frozen prefix.

The published six-slot prefix ends with `metadata_collect`. It returns binding metadata collections
through one generic function and does not change minimum-prefix discovery compatibility. A producer
that exposes only the frozen five-slot prefix can still be loaded; its host must report named
metadata as unavailable rather than reading beyond the returned table.

The current full table appends `engine_new_with_services`. Its records carry constructor-owned text
measurement and bounded Iconify packs without adding a registry handle or changing the six-slot
prefix. A compatible older producer remains usable for empty-service engines and legacy
text-measurer requests. It cannot accept non-empty icon-pack input, and a high-level binding must
report that incompatibility instead of silently dropping the packs.

`capability_catalog_digest` identifies the loaded artifact's callable capabilities. It can differ between full and capability-focused artifacts that implement the same native ABI.

Do not require the full descriptor digest, capability catalog digest, or package version to equal values from another compatible ABI 3 build unless the application deliberately pins that exact artifact.

## Record And Result Rules

Every record except `MermanNativeApi` requires exactly the generated `struct_size`. The API table alone interprets `struct_size` as caller capacity and returns the largest complete prefix safely initialized within that capacity.

`MermanNativeResult` is both an output and an ownership handle. Merman reads the zero state before writing it, assigns a process-lifetime monotonic token, and never uses the nested buffer addresses as allocation authority. `result_free` releases only a currently live token, clears the supplied record, and makes stale or random tokens harmless. A copied live token can still release the original allocation, so duplicate live result records are outside the same-process hostile-memory threat boundary.

## Engine Admission

Callback-free engines admit concurrent operations. An engine configured with a host text-measurement callback serializes operation admission; a competing operation returns the typed `busy` failure.

`engine_try_close` is a nonblocking quiescence check. It retains the engine token on `BUSY` or `REENTRANT_CALL`. A successful close permanently prevents new admissions before retiring the token, so it is the point after which host callback state can be destroyed.

## Operation Authority

The transport-neutral operation vocabulary comes from the capability descriptor and is generated as `OperationKey` in Rust. Numeric `MERMAN_NATIVE_OPERATION_*` codes are a C ABI projection owned only by `merman-ffi`; custom bindings should not treat those numbers as the cross-transport domain model.

## Verification

Regenerate and verify the checked-in ABI projections before packaging:

```sh
cargo run -p xtask -- gen-native-abi
cargo run --locked -p xtask -- verify-native-abi
cargo nextest run --locked -p merman-ffi --no-default-features
python3 -m unittest scripts/test_native_symbol_contract.py
```

The FFI test suite compiles consumers against the current generated header, the frozen minimum ABI
3 header, and the published six-slot header. The native symbol contract permits exactly one
Merman-owned C export: `merman_get_native_api`.
