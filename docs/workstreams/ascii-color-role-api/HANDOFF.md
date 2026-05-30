# ASCII Color Role API - Handoff

Status: Draft
Last updated: 2026-05-30

## Current State

The workstream is opened as a design draft. No renderer code has changed. The proposed direction is
plain-by-default color roles with centralized ANSI/HTML finalization and no layout involvement.

## Active Task

- Task ID: ACR-020
- Owner: unassigned
- Files: `docs/adr`, `crates/merman-ascii/src/options.rs`, `crates/merman-ascii/src/lib.rs`
- Validation: accepted ADR or this workstream remains draft
- Status: TODO
- Review: public API decision review
- Evidence: `DESIGN.md`, new ADR

## Decisions Since Last Update

- Default output should remain plain text and byte-for-byte compatible.
- `Auto` color mode should be opt-in because it depends on environment detection.
- The first implementation should be foreground-only roles; background/fill is a follow-on.
- `AsciiColorRole` should be non-exhaustive.
- `AsciiColorTheme` should have private fields and builder methods.
- Mermaid style mapping should not be bundled with the first role-canvas slice.

## Blockers

- Public API migration strategy for `AsciiRenderOptions` is undecided.

## Next Recommended Action

- Write the ADR for `AsciiColorMode`, `AsciiColorRole`, `AsciiColorTheme`, and the
  `AsciiRenderOptions` migration before editing renderer code.
