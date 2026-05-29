# ASCII Sequence Parity - Handoff

Status: Active
Last updated: 2026-05-29

## Current State

ASP-010, ASP-020, and ASP-040 are complete. Copied upstream `mermaid-ascii` sequence fixtures are
exact under the existing normalized-whitespace comparison. The lane is now moving beyond copied
fixtures into typed sequence semantics that users can already parse through `merman-core`.

## Active Task

- Task ID: ASP-030
- Owner: codex
- Files:
  - `crates/merman-ascii/SEQUENCE_SUPPORT.md`
  - `docs/workstreams/ascii-sequence-parity`
- Validation:
  - Support matrix names exact unsupported rich sequence constructs.
  - Follow-on tasks are independently executable.
- Status: READY

## Decisions Since Open

- Treat copied upstream sequence fixtures as already covered.
- Keep unsupported richer Mermaid sequence constructs explicit.
- Add open-arrow support before notes/boxes/activations because it is a small typed-model slice with
  low layout risk.
- Message types `5` (`A->B`) and `6` (`A-->B`) now render; Unicode uses open arrowheads.

## Blockers

- None.

## Next Recommended Action

Execute ASP-030 by splitting notes, sequence boxes, activations, create/destroy, and wrapping into
ordered follow-on work.
