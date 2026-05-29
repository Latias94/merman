# ASCII Sequence Control Blocks - Handoff

Status: Closed
Last updated: 2026-05-29

## Current State

ASCB-010 through ASCB-060 are complete. The lane is closed for the primary Mermaid sequence
control-block subset in the ASCII renderer.

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

ASCB-050 covers the current edge-case policy: nested blocks and empty sections are explicit
unsupported diagnostics; activations, create/destroy lifecycle rows, notes, and participant boxes
are supported with control-block frames. ASCB-060 generated manual examples, ran the broader
closeout gate, updated README, and closed the lane.

## Active Task

- Task ID: None
- Status: Closed

## Next Action

Open a follow-on lane only if product scope needs one of the deferred boundaries:
`rect`/`par_over`, nested control-block rendering, empty-section rendering, or exact
Mermaid/SVG-style visual parity for control blocks.

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
- ASCB-060 closes the lane with the primary control-block subset shipped and broader ASCII feature
  gates passing.

## Blockers

- None.

## Useful Commands

```bash
cargo fmt --all --check
cargo nextest run -p merman-ascii sequence
cargo nextest run -p merman-ascii sequence_golden
cargo nextest run -p merman-ascii
cargo nextest run -p merman --features ascii
cargo nextest run -p merman-cli --features ascii
cargo clippy -p merman-ascii -p merman --features ascii --all-targets -- -D warnings
git diff --check
```
