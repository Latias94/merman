# ASCII Sequence Control Blocks - Handoff

Status: Active
Last updated: 2026-05-29

## Current State

ASCB-010 is complete. The lane is open and scoped to Mermaid sequence control blocks for the ASCII
renderer.

The current ASCII adapter rejects endpoint-less control signals as unsupported `control messages`.
That is the right current behavior, but it is also the gap this lane should close. The useful local
reference is SVG parity's `block_collection.rs`, which already maps core line types into typed
`Loop`, `Opt`, `Break`, `Alt`, `Par`, and `Critical` blocks.

## Active Task

- Task ID: ASCB-020
- Status: Ready

## Next Action

Add executable inventory tests for `loop`, `opt`, `break`, `alt`, `par`, and `critical` Mermaid
inputs. The tests should prove:

- the core render model uses endpoint-less control messages for block markers,
- labels and message type numbers match the expected core line types,
- the current ASCII renderer returns `UnsupportedFeature { feature: "control messages" }`.

Do not implement rendering in ASCB-020. The purpose is to freeze the boundary before changing it.

## Decisions Since Open

- Primary scope is `loop`, `opt`, `break`, `alt`, `par`, and `critical`.
- `rect` is deferred unless intentionally pulled in after the primary subset is stable.
- The ASCII implementation should be a terminal approximation, not a clone of SVG geometry.
- Block collection should live above low-level message/note row painting.

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
