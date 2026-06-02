# HPD-060 - Sequence Typed JSON Projection

Date: 2026-06-02

## Context

HPD-060 needed a bounded semantic/render unification pilot rather than a repo-wide migration.
Sequence was the best first target: it already had `SequenceDiagramRenderModel`, but
`SequenceDb::into_model(...)` still manually built compatibility JSON from the parser DB as a
parallel master path.

## Outcome

- Added `SequenceDiagramRenderModel::to_compat_json(...)`.
- Replaced the DB-side manual compatibility JSON builder with
  `self.into_render_model().to_compat_json(&meta.diagram_type)`.
- Removed the now-unneeded `SequenceMessagePayload::into_value(...)` helper.
- Added explicit serde behavior for optional message `placement`, keeping absent placements omitted
  from compatibility JSON instead of serializing them as `null`.
- Expanded the focused typed-vs-JSON parse test so it checks a richer Sequence diagram with actors,
  a box, a note, create/destroy events, and omitted optional message fields.

## Verification

- `cargo fmt --all`
- `cargo test -p merman-core parse_sequence_render_model_uses_typed_variant_without_changing_json_parse --lib`
- `cargo test -p merman-core sequence --lib`
- `cargo test -p merman-core --lib`
- `cargo test -p merman-render sequence_long_leftof_notes_keep_mermaid_11_15_note_width --test sequence_svg_test`
- `cargo run -p xtask -- compare-sequence-svgs --check-dom --dom-mode parity --dom-decimals 3 --out target\compare\sequence_report_parity_after_hpd060_typed_projection.md`
- `cargo run -p xtask -- compare-sequence-svgs --check-dom --dom-mode parity-root --dom-decimals 3 --report-root-all --out target\compare\sequence_report_parity_root_after_hpd060_typed_projection.md`

## Evidence

- `merman-core` lib tests passed: `544` tests.
- Sequence structural SVG parity remained green: all fixtures matched in
  `target\compare\sequence_report_parity_after_hpd060_typed_projection.md`.
- Sequence root parity remains intentionally open: the post-HPD-060 root report has `28` dom
  mismatches, led by the known long left-of note rows and other measurement/root tails.

## Notes

`cargo test -p merman-render --test sequence_svg_test` is not green in the current repository state.
The failures are existing Sequence measurement/root gates (`152.0` vs `151.0` for one vendored note
width expectation and the documented long left-of root-width residual), not part of this core
semantic projection refactor. Do not close those by adding fixture-keyed constants.
