# ASCII Sequence Parity - Handoff

Status: Active
Last updated: 2026-05-29

## Current State

ASP-010, ASP-020, ASP-030, ASP-040, ASP-050, ASP-060, ASP-090, and ASP-100 are complete. Copied
upstream `mermaid-ascii` sequence fixtures are exact under the existing normalized-whitespace
comparison. The lane is now moving beyond copied fixtures into typed sequence semantics that users
can already parse through `merman-core`.

## Active Task

- Task ID: ASP-070
- Owner: codex
- Files:
  - `crates/merman-ascii/src/sequence.rs`
  - `crates/merman-ascii/tests/sequence_model.rs`
  - `crates/merman-ascii/SEQUENCE_SUPPORT.md`
- Validation:
  - Follow-on task names exact renderer state needed.
- Status: READY

## Decisions Since Open

- Treat copied upstream sequence fixtures as already covered.
- Keep unsupported richer Mermaid sequence constructs explicit.
- Add open-arrow support before notes/boxes/activations because it is a small typed-model slice with
  low layout risk.
- Message types `5` (`A->B`) and `6` (`A-->B`) now render; Unicode uses open arrowheads.
- Rich sequence work is split by increasing renderer-state risk: notes first, boxes second,
  activations/create-destroy third, wrapping last.
- Single-line typed notes now render for left-of, right-of, and over placements. Wrapped and
  multiline notes remain unsupported.
- Typed sequence boxes now render as enclosing text borders around actor groups. Fill colors are not
  represented in plain text. Wrapped, empty, and unknown-actor boxes remain explicit unsupported
  features.

## Blockers

- None.

## Next Recommended Action

Execute ASP-070 by splitting or implementing activation plus create/destroy renderer-state
semantics.
