# ASCII Sequence Renderer Modularization - Milestones

Status: Closed
Last updated: 2026-05-29

## M0 - Scope And Evidence Freeze

Exit criteria:

- The refactor target is explicit.
- Non-goals protect behavior and public API compatibility.
- First extraction task is bounded and independently verifiable.

Primary evidence:

- `docs/workstreams/ascii-sequence-renderer-modularization/DESIGN.md`
- `docs/workstreams/ascii-sequence-renderer-modularization/TODO.md`

## M1 - Model And Validation Boundary

Exit criteria:

- Internal sequence model types and validation logic no longer live directly in the facade file.
- Unsupported-feature diagnostics remain stable.
- Existing sequence behavior and golden tests pass.

Primary gates:

- `cargo nextest run -p merman-ascii sequence`
- `cargo nextest run -p merman-ascii sequence_golden`

## M2 - Layout And Rendering Boundaries

Exit criteria:

- Layout state and row rendering have owner modules.
- `sequence.rs` is a facade/orchestrator rather than the single owner of every sequence concern.
- Package-level ASCII tests pass.

Primary gates:

- `cargo nextest run -p merman-ascii sequence`
- `cargo nextest run -p merman-ascii sequence_golden`
- `cargo nextest run -p merman-ascii`

## M3 - Control-Block Readiness

Exit criteria:

- Final module boundary is documented.
- `sequence-control-blocks` remains a separate follow-on scope.
- No control-block behavior is implemented in this lane.

## M4 - Closeout

Exit criteria:

- Gate set is recorded.
- Review has no blocking findings.
- Remaining work is completed, deferred, or split into a follow-on.
- `WORKSTREAM.json` status is updated.
