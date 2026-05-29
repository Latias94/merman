# ASCII Sequence Parity - Handoff

Status: Active
Last updated: 2026-05-29

## Current State

ASP-010, ASP-020, ASP-030, ASP-040, ASP-050, and ASP-090 are complete. Copied upstream
`mermaid-ascii` sequence fixtures are exact under the existing normalized-whitespace comparison.
The lane is now moving beyond copied fixtures into typed sequence semantics that users can already
parse through `merman-core`.

## Active Task

- Task ID: ASP-060
- Owner: codex
- Files:
  - `crates/merman-ascii/src/sequence.rs`
  - `crates/merman-ascii/tests/sequence_model.rs`
  - `crates/merman-ascii/SEQUENCE_SUPPORT.md`
- Validation:
  - Focused sequence tests prove boxes or document why they remain unsupported.
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

## Blockers

- None.

## Next Recommended Action

Execute ASP-060 by deciding whether sequence boxes can fit the current line-oriented renderer or
need a deeper layout boundary first.
