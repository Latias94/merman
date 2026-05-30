# UniFFI Bindings — Milestones

Status: Active
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

## M3 — Generated Binding Smoke

Exit criteria:

- At least one UniFFI bindgen smoke path is proven, or a precise blocker is documented.
- Generated files are either ignored or intentionally committed with rationale.

## M4 — Closeout

Exit criteria:

- Focused checks pass.
- Package/release ordering concerns are split from this lane.
- Follow-ons are explicit: iOS, Android, Python, Flutter, Node, or raster as separate lanes.
