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
- [ ] Replace one low-risk State fixture group with typed or emitted-bounds derivation.
- [ ] Delete the corresponding generated State root entries and tighten the root budget.
- [ ] Prove State normal DOM parity and `parity-root` stay green.

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
