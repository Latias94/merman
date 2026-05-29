# ASCII Sequence Control Blocks - Milestones

Status: Active
Last updated: 2026-05-29

## M0 - Scope And Semantic Inventory

Opened the lane and fixed the primary scope to Mermaid sequence control blocks represented as
endpoint-less control signals in the core render model.

Exit condition:

- ASCB-010 complete.
- Parser/model and SVG reference points recorded.
- First executable task selected.

## M1 - Executable Boundary Tests

Freeze the current unsupported boundary and line-type inventory in tests before changing behavior.

Exit condition:

- ASCB-020 complete.
- Tests prove current core representation for the selected block forms.
- The ASCII renderer still rejects control blocks explicitly until rendering support begins.

## M2 - Single-Section Blocks

Add the first block-aware render-plan slice for `loop`, `opt`, and `break`.

Exit condition:

- ASCB-030 complete.
- The renderer can frame contained rows for the three single-section block forms.
- Existing message, note, lifecycle, autonumber, and participant-box behavior remains stable.

## M3 - Sectioned Blocks

Extend the same block model to `alt`, `par`, and `critical` section separators.

Exit condition:

- ASCB-040 complete.
- Section labels and separators render deterministically.
- Empty and multi-section cases are covered or explicitly rejected.

## M4 - Nesting And Edge Cases

Settle the product boundary for nested blocks and interactions with lifecycle, participant boxes,
notes, and created/destroyed actors.

Exit condition:

- ASCB-050 complete.
- Supported edge cases have tests.
- Deferred edge cases have explicit diagnostics and support-doc entries.

## M5 - Examples, Verification, And Closeout

Package the implementation for users and future maintainers.

Exit condition:

- ASCB-060 complete.
- Manual example outputs exist for the primary block subset.
- Final gates pass and the workstream is closed or split cleanly.
