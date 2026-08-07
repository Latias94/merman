# Native ABI 3 Migration

The `0.8.0-alpha.3` packages used native ABI 2. Current source uses native ABI 3 and intentionally
does not preserve ABI 2 or prerelease ABI 3 consumer compatibility. Rebuild every C, C++, Dart FFI,
or custom native host against the generated headers from the same Merman release.

## Required Host Changes

1. Resolve only `merman_get_native_api`; per-operation exports and ABI 2 symbols no longer exist.
2. Send `MERMAN_NATIVE_ABI_VERSION` and
   `MERMAN_NATIVE_ABI_MINIMUM_PREFIX_LAYOUT_DIGEST` in `MermanNativeApiRequest`.
3. Set `MermanNativeApi.struct_size` to the complete generated table size. Merman never writes
   beyond that caller-declared capacity and reports the largest complete initialized prefix.
4. Require every function in the release-matched table, including `metadata_collect` and
   `engine_new_with_services`. Do not silently fall back to an older constructor or discard
   constructor services.
5. Fully zero-initialize `MermanNativeResult` with `MERMAN_NATIVE_RESULT_INIT` before every
   producing call.
6. Free every Merman-written result through its nonzero `allocation_token`. Moving a complete
   result transfers ownership only after the source record is cleared.
7. Replace unconditional engine destruction with `engine_try_close`. Retry after
   `MERMAN_NATIVE_STATUS_BUSY`; a reentrant close from an active callback is invalid.
8. Keep callback and `user_data` storage valid and immutable until a quiescent close succeeds.
9. Catch every host-language exception before it crosses the native callback boundary and return
   `MERMAN_NATIVE_STATUS_CALLBACK_ERROR`.
10. Use the generated text-measurement constants rather than hand-coding protocol numbers.

## Digest Roles

`minimum_prefix_layout_digest` is the discovery compatibility key. `full_descriptor_digest`
identifies the complete release descriptor, while `capability_catalog_digest` identifies the
loaded artifact's callable feature surface. Release-matched hosts should validate the complete
table they were generated against; partial historical tables are not a supported consumer target.

## Record, Result, And Service Rules

Every record except `MermanNativeApi` requires exactly the generated `struct_size`. The API table
alone treats `struct_size` as caller capacity, preserving memory safety for undersized or future
tables without promising source or behavior compatibility for old consumers.

`MermanNativeResult` is both output and ownership handle. Merman authorizes allocation release by
its monotonic token, clears the supplied record on `result_free`, and ignores stale or unknown
tokens. Result storage must not overlap input records or borrowed input bytes.

Use `engine_new_with_services` for all reusable-engine construction. It accepts constructor-owned
text measurement and bounded Iconify pack inputs. The outer config, pack records, and nested byte
slices are borrowed only until construction returns; a successful engine owns parsed state and
retains only the callback state whose lifetime is explicitly part of the service contract.

## Verification

Run the generated current-header C smoke test and the Rust ABI contract tests. Historical five-slot
and six-slot headers are no longer checked in or compiled as compatibility fixtures. Size-tagged
table bounds, pointer alignment, overlap rejection, result-token ownership, publication rollback,
and callback/close safety remain mandatory current-contract tests.
