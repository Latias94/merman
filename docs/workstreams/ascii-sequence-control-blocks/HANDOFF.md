# ASCII Sequence Control Blocks - Handoff

Status: Active
Last updated: 2026-05-29

## Current State

ASCB-010, ASCB-020, ASCB-030, and ASCB-040 are complete. The lane is open and scoped to Mermaid
sequence control blocks for the ASCII renderer.

The ASCII adapter now renders single-section `loop`, `opt`, and `break` endpoint-less control
signals as labeled text frames. It also renders sectioned `alt`/`else`, `par`/`and`, and
`critical`/`option` blocks with labeled separators. The core line-type inventory is covered by
`sequence_control_blocks_are_core_control_signals`; supported frame rendering is covered by
`sequence_single_section_control_blocks_render_unicode_frames` and
`sequence_single_section_control_blocks_render_ascii_frames`,
`sequence_single_section_control_blocks_frame_notes`,
`sequence_sectioned_control_blocks_render_unicode_frames`, and
`sequence_sectioned_control_blocks_render_ascii_frames`. Repeated section separators and notes
inside sectioned frames are covered by
`sequence_sectioned_control_blocks_frame_multiple_sections_and_notes`.

## Active Task

- Task ID: ASCB-050
- Status: Ready

## Next Action

Settle nesting and edge-case policy. The most important cases are nested control blocks, empty
sections, lifecycle events inside blocks, created/destroyed actors inside blocks, participant boxes
inside blocks, and notes inside sectioned blocks. Supported cases need tests; deferred cases need
explicit diagnostics and support-doc entries.

## Decisions Since Open

- Primary scope is `loop`, `opt`, `break`, `alt`, `par`, and `critical`.
- `rect` and `par_over` are deferred unless intentionally pulled in after the primary subset is
  stable.
- The ASCII implementation should be a terminal approximation, not a clone of SVG geometry.
- Block collection should live above low-level message/note row painting.
- ASCB-020 froze the current unsupported boundary before rendering support starts.
- ASCB-030 intentionally rejects nested and empty single-section blocks for now; ASCB-050 owns the
  final edge-case policy.
- ASCB-040 keeps `rect` and `par_over` deferred as `control messages`.

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
