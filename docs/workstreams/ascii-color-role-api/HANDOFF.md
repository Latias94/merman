# ASCII Color Role API - Handoff

Status: Active
Last updated: 2026-05-30

## Current State

The workstream is active. ADR 0067 accepted the public color role API shape and the
`AsciiRenderOptions` migration. No renderer code has changed yet.

## Active Task

- Task ID: ACR-030
- Owner: unassigned
- Files: `crates/merman-ascii/src/color.rs`, `crates/merman-ascii/src/options.rs`,
  `crates/merman-ascii/src/canvas.rs`, `crates/merman-ascii/src/lib.rs`
- Validation: `cargo nextest run -p merman-ascii color canvas`; `cargo fmt --all --check`
- Status: TODO
- Review: no diagram renderer should receive color-specific layout logic
- Evidence: `EVIDENCE_AND_GATES.md`

## Decisions Since Last Update

- Default output should remain plain text and byte-for-byte compatible.
- `Auto` color mode should be opt-in because it depends on environment detection.
- The first implementation should be foreground-only roles; background/fill is a follow-on.
- `AsciiColorRole` should be non-exhaustive.
- `AsciiColorTheme` should have private fields and builder methods.
- Mermaid style mapping should not be bundled with the first role-canvas slice.
- ADR 0067 accepts a pre-1.0 `AsciiRenderOptions` migration: add color fields, keep `Copy`, add
  builder methods, and mark the struct `#[non_exhaustive]`.

## Blockers

- None. Implementation should stay inside ACR-030's infrastructure scope.

## Next Recommended Action

- Implement the role-aware canvas and forced encoders before assigning roles to any diagram family.
