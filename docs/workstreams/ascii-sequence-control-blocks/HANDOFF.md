# ASCII Sequence Control Blocks - Handoff

Status: Active
Last updated: 2026-05-29

## Current State

ASCB-010, ASCB-020, and ASCB-030 are complete. The lane is open and scoped to Mermaid sequence
control blocks for the ASCII renderer.

The ASCII adapter now renders single-section `loop`, `opt`, and `break` endpoint-less control
signals as labeled text frames. The core line-type inventory is covered by
`sequence_control_blocks_are_core_control_signals`; supported frame rendering is covered by
`sequence_single_section_control_blocks_render_unicode_frames` and
`sequence_single_section_control_blocks_render_ascii_frames`.

The useful local reference for ASCB-040 remains SVG parity's `block_collection.rs`, which already
maps core line types into typed `Alt`, `Par`, and `Critical` sections.

## Active Task

- Task ID: ASCB-040
- Status: Ready

## Next Action

Extend the control-block plan to sectioned `alt`/`else`, `par`/`and`, and `critical`/`option`
frames. Reuse the current frame/span path, but add section separators and per-section labels instead
of treating section markers as plain unsupported control messages.

## Decisions Since Open

- Primary scope is `loop`, `opt`, `break`, `alt`, `par`, and `critical`.
- `rect` and `par_over` are deferred unless intentionally pulled in after the primary subset is
  stable.
- The ASCII implementation should be a terminal approximation, not a clone of SVG geometry.
- Block collection should live above low-level message/note row painting.
- ASCB-020 froze the current unsupported boundary before rendering support starts.
- ASCB-030 intentionally rejects nested and empty single-section blocks for now; ASCB-050 owns the
  final edge-case policy.

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
