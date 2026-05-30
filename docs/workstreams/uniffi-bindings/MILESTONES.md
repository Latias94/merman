# UniFFI Bindings — Milestones

Status: Closed
Last updated: 2026-05-30

## M0 — Scope Frozen

Exit criteria:

- UniFFI version and risk are recorded.
- The lane explicitly keeps C ABI canonical.
- Platform packages are out of scope.

## M1 — Shared Binding Facade

Exit criteria:

- A safe facade crate/module exposes render SVG, parse JSON, and layout JSON behavior.
- Options JSON parsing and error mapping move out of unsafe FFI code.
- `merman-ffi` tests still pass without C ABI drift.

Status: complete. `crates/merman-bindings-core` now owns the safe binding behavior and
`merman-ffi` delegates to it while preserving public symbols and buffer ownership.

## M2 — Minimal UniFFI Surface

Exit criteria:

- `crates/merman-uniffi` exists and compiles.
- The crate exposes minimal render/parse/layout methods.
- Errors are mapped consistently with the shared facade.

Status: complete. `merman-uniffi` exposes `MermanEngine` methods for SVG, semantic JSON, and layout
JSON, with structured error mapping from `BindingError`.

## M3 — Generated Binding Smoke

Exit criteria:

- At least one UniFFI bindgen smoke path is proven, or a precise blocker is documented.
- Generated files are either ignored or intentionally committed with rationale.

Status: complete. `cargo test -p merman-uniffi --features bindgen-smoke --test bindgen_smoke`
builds the cdylib and proves Python binding generation into a temporary directory without committed
generated artifacts.

## M4 — Closeout

Exit criteria:

- Focused checks pass.
- Package/release ordering concerns are split from this lane.
- Follow-ons are explicit: iOS, Android, Python, Flutter, Node, or raster as separate lanes.

Status: complete. Focused facade, FFI, UniFFI, feature, bindgen, format, and lint gates passed.
Full workspace nextest was skipped because the working tree contains unrelated uncommitted
ASCII/README changes outside this lane.
