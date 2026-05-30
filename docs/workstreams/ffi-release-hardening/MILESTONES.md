# FFI Release Hardening — Milestones

Status: Closed with packaging concern
Last updated: 2026-05-30

## M0 — Scope Frozen

Exit criteria:

- The workstream documents agree on problem, non-goals, and validation.
- `ffi-api` remains closed and this lane only references it as prior authority.

## M1 — C Consumer Proof

Exit criteria:

- A C smoke source includes `merman.h`.
- The smoke calls at least `merman_render_svg` and `merman_parse_json`.
- The smoke validates success/error result codes.
- Every non-empty buffer returned by Rust is released with `merman_buffer_free`.

## M2 — Package And Discovery

Exit criteria:

- `cargo package -p merman-ffi --allow-dirty` passes or a concrete blocker is documented.
- Project docs point C consumers to `merman-ffi`, `include/merman.h`, and `docs/bindings/FFI_PROTOCOL.md`.
- The FFI protocol doc states how to build/link the crate at a high level.

Status: met with concern. The package file list is correct, but full package verification is blocked
until the next workspace crate version is published in dependency order because the published
`merman-render 0.6.0` does not expose `ratex-math`.

## M3 — Closeout

Exit criteria:

- Focused nextest and package gates pass.
- Evidence explains any skipped full workspace gates.
- Follow-ons are explicit: UniFFI bindings and raster output are separate lanes.

Status: complete with the package verification concern recorded in `EVIDENCE_AND_GATES.md`.
