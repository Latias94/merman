# ASCII Sequence Rect And ParOver Blocks - Milestones

Status: Active
Last updated: 2026-05-29

## M0 - Scope And Evidence Freeze

Open the follow-on lane with a clear product boundary for the two deferred Mermaid sequence control
forms.

Exit criteria:

- ASRP-010 complete.
- `rect` and `par_over` parser/model reference points recorded.
- First executable task selected.

## M1 - Executable Boundary Tests

Freeze the current unsupported boundary and core line-type inventory before changing behavior.

Exit criteria:

- ASRP-020 complete.
- Tests prove `rect` emits 22/23 and `par_over` emits 32/21.
- The ASCII renderer still rejects both explicitly until rendering support begins.

## M2 - Rect Region Frames

Add terminal-visible region rendering for Mermaid `rect`.

Exit criteria:

- ASRP-030 complete.
- `rect <style>` renders as a single-section frame in Unicode and ASCII.
- Style text is preserved without attempting ANSI color semantics.

## M3 - ParOver Frames

Add terminal-visible rendering for Mermaid `par_over`.

Exit criteria:

- ASRP-040 complete.
- `par_over <label>` renders with the `par_over` keyword.
- Existing `par`/`and` sectioned frame behavior remains stable.

## M4 - Combinations And Edge Policy

Prove the new frame forms interact correctly with the existing sequence renderer boundary.

Exit criteria:

- ASRP-050 complete.
- Supported combinations have tests.
- Nested, empty, and malformed cases are explicit diagnostics or intentionally supported.

Result:

- ASRP-050 complete on 2026-05-29.
- Notes, activations, create/destroy lifecycle rows, participant boxes, nested blocks, empty
  sections, and malformed ordering are covered for `rect` and `par_over`.
- The box renderer now preserves foreground control-frame text when a participant box overlaps it.

## M5 - Examples, Verification, And Closeout

Package the implementation for users and future maintainers.

Exit criteria:

- ASRP-060 complete.
- Manual example outputs exist for `rect` and `par_over`.
- Final gates pass and the workstream is closed or split cleanly.
