# Platform FFI Packaging - Milestones

Status: Closed
Last updated: 2026-05-30

## M0 - Scope And Evidence Freeze

Exit criteria:

- Workstream docs agree on scope, non-goals, validation, and first tasks.
- Completed C ABI and UniFFI lanes are referenced instead of reopened.

Status: complete.

## M1 - Cross-Platform Script Entrypoints

Exit criteria:

- Python equivalents exist for the current PS1 platform scripts.
- Python scripts compile with `py_compile`.
- Docs prefer Python commands for cross-platform verification.

Status: complete.

## M2 - Apple Local Verification

Exit criteria:

- Apple XCFramework build runs locally or records exact missing-toolchain blockers.
- SwiftPM can inspect the local package after the binary target exists.
- Generated XCFramework contents include module maps for `MermanFFI`.

Status: complete.

## M3 - Focused Binding Gates

Exit criteria:

- `merman-ffi` targeted tests pass.
- UniFFI bindgen smoke passes or records an exact Python/toolchain blocker.
- Whitespace checks pass for touched files.

Status: complete.

## M4 - Closeout

Exit criteria:

- TODO and handoff state match actual completion.
- Remaining packaging work is split into explicit follow-ons.
- No generated native artifacts are unintentionally tracked.

Status: complete.
