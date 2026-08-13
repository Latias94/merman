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
4. Require every function in the release-matched table. The descriptor-selected minimum prefix
   ends at `engine_new_with_services` (slot `6`); the current table appends
   `operation_control_new`, `operation_control_cancel`, and `operation_control_release` at slots
   `7`, `8`, and `9`, followed by `execute_collect_controlled` at slot `10`. Require the complete
   `MERMAN_NATIVE_API_EXECUTE_COLLECT_CONTROLLED_PREFIX_SIZE` instead of treating the minimum
   prefix as the complete current table.
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
11. Keep using the unchanged ABI 3 `MermanNativeOperationRequest` layout. To cancel or set a
    deadline, create a nonzero `MermanNativeOperationControlToken`, pass it explicitly to
    `execute_collect_controlled`, and release it after the host no longer needs the registry
    identity. Continue using `execute_collect` for uncontrolled calls.
12. Decode `MERMAN_NATIVE_STATUS_CANCELLED` (`17`) separately from resource-limit failures and
    preserve `details.cancellation.reason` plus `details.cancellation.phase`.

## Digest Roles

`minimum_prefix_layout_digest` is the discovery compatibility key. `full_descriptor_digest`
identifies the complete release descriptor, while `capability_catalog_digest` identifies the
loaded artifact's callable feature surface. Release-matched hosts should validate the complete
table they were generated against; partial historical tables are not a supported consumer target.
The minimum prefix deliberately remains stable through slot `6`; the generated operation-control
and controlled-execute prefix macros describe the appended slot boundaries without changing that
compatibility digest.

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

Operation controls have a separate opaque token domain. `operation_control_new` optionally installs
a relative monotonic deadline; `operation_control_cancel` atomically requests cancellation; and
`operation_control_release` retires the registry token without invalidating a control already
cloned by an in-flight operation. `execute_collect_controlled` borrows a token without changing the
frozen request-record layout; a zero token selects a fresh internal active control and owns nothing
to release.

Cancellation is cooperative. It is observed at parser, layout, adapter, post-processing, and export
checkpoints, so it does not forcefully unwind Rust or foreign code. In particular, an opaque
synchronous backend or host text-measurement callback must return before cancellation can be
observed at the next checkpoint. A cancelled call writes the ordinary owned failure result with
status `17`, no partial output, and `details.cancellation` containing `reason` (`requested` or
`deadline_exceeded`) and the observed phase.

## Verification

Run the generated current-header C smoke test and the Rust ABI contract tests. Historical five-slot
and six-slot headers are no longer checked in or compiled as compatibility fixtures. Size-tagged
table bounds, ordered operation-control and controlled-execute prefix macros, frozen request
layout, pointer alignment, overlap rejection, result/control-token ownership, publication rollback,
cancellation payloads, and callback/close safety remain mandatory current-contract tests.
