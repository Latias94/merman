# ASCII Sequence Control Blocks - Handoff

Status: Active
Last updated: 2026-05-29

## Current State

ASCB-010 and ASCB-020 are complete. The lane is open and scoped to Mermaid sequence control blocks
for the ASCII renderer.

The current ASCII adapter rejects endpoint-less control signals as unsupported `control messages`.
That boundary is now covered by
`sequence_control_blocks_are_core_control_signals_and_currently_unsupported`. The useful local
reference for ASCB-030 remains SVG parity's `block_collection.rs`, which already maps core line
types into typed `Loop`, `Opt`, `Break`, `Alt`, `Par`, and `Critical` blocks.

## Active Task

- Task ID: ASCB-030
- Status: Ready

## Next Action

Implement the first real render-plan slice for single-section `loop`, `opt`, and `break` frames.
Start by collecting endpoint-less control markers into typed block spans without changing sectioned
block behavior yet.

## Decisions Since Open

- Primary scope is `loop`, `opt`, `break`, `alt`, `par`, and `critical`.
- `rect` and `par_over` are deferred unless intentionally pulled in after the primary subset is
  stable.
- The ASCII implementation should be a terminal approximation, not a clone of SVG geometry.
- Block collection should live above low-level message/note row painting.
- ASCB-020 froze the current unsupported boundary before rendering support starts.

## Blockers

- None.

## Useful Commands

```bash
cargo fmt --all --check
cargo nextest run -p merman-ascii sequence
cargo nextest run -p merman-ascii sequence_golden
cargo nextest run -p merman-ascii
git diff --check
```
