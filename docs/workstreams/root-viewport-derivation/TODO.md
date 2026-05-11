# Root Viewport Derivation TODO

This backlog tracks root viewport override replacement work. Deleting an entry is only complete
when a typed/layout/emitted-bounds rule explains the same root `viewBox` and `max-width`.

## P0: Workstream Baseline

- [x] Create the workstream document set.
- [x] Record the current State and Mindmap root override baseline.
- [x] Confirm the focused audit commands for State and Mindmap.
- [x] Add clippy and nextest expectations to the success criteria.

## P1: State Root Derivation

- [ ] Classify the 45 remaining State root pins by drift family.
  Known initial families:
  - right-to-left direction and scale bounds.
  - dense or wrapping edge-label bounds.
  - note and multiline-label bounds.
  - styled/classed state shape bounds.
  - small browser float/rounding deltas.
- [x] Replace one low-risk State fixture group with typed or emitted-bounds derivation.
  Evidence: direct `style ... border:...` statements no longer trigger the classDef-only 72px
  label-height inflation rule.
- [x] Delete the corresponding generated State root entries and tighten the root budget.
  Evidence: removed `upstream_cypress_statediagram_v2_spec_can_have_styles_applied_034`; root
  no-growth budget was tightened to `759`.
- [x] Replace the Mermaid entity-placeholder edge-label group with layout/text derivation.
  Evidence: State layout now decodes Mermaid `encodeEntities` placeholders before measuring labels,
  and the shared browser-measured `test({ foo: 'far' })` edge-label width replaces two fixture
  root pins:
  `upstream_cypress_statediagram_v2_spec_v2_should_render_a_state_diagram_and_set_the_correct_length_of_t_031`
  and `upstream_cypress_statediagram_v2_spec_v2_states_can_have_a_class_applied_032`.
- [x] Tighten the current root budget after the edge-label pass.
  Evidence: State root count is now `42`, root viewport total is `757`, and the text lookup budget
  is explicitly `481` because one reusable State edge-label browser metric replaced two root pins.
- [x] Prove State normal DOM parity and `parity-root` stay green.
  Evidence: full `compare-state-svgs` passed in both `parity` and `parity-root` DOM modes.
- [x] Run focused State code-quality checks for this pass.
  Evidence: `cargo clippy -p merman-render --all-targets --all-features -- -D warnings`,
  `cargo test -p xtask override_growth_check_rejects_category_growth`, and
  `cargo nextest run -p merman-render` passed.

## P1: Mindmap Root Derivation

- [ ] Classify the 52 remaining Mindmap root pins by drift family.
  Known initial families:
  - wrapping text and long-word bounds.
  - icon-bearing node bounds.
  - shape-specific SVG bbox bounds.
  - markdown / HTML sanitization label bounds.
  - tree-wide transform and tidy-tree bounds.
- [ ] Replace one low-risk Mindmap fixture group with typed or emitted-bounds derivation.
- [ ] Delete the corresponding generated Mindmap root entries and tighten the root budget.
- [ ] Prove Mindmap normal DOM parity and `parity-root` stay green.

## P2: Larger Buckets

- [ ] Revisit Flowchart after State/Mindmap patterns are proven.
- [ ] Revisit Sequence after typed note/message/frame bounds have a reusable derivation pattern.
- [ ] Revisit GitGraph after branch/merge/tag root bounds can be derived without fixture pins.

## P3: Release Closeout

- [ ] Run `cargo run -p xtask -- verify --strict`.
- [ ] Run `cargo clippy -p merman-render --all-targets --all-features -- -D warnings`.
- [ ] Run `cargo nextest run` if shared rendering/layout behavior changed.
- [ ] Update `CHANGELOG.md` and the workstream changelog.
- [ ] Complete `AUDIT.md` with prompt-to-artifact evidence.
