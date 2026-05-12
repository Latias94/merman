# Root Viewport Derivation Changelog

## 2026-05-12

- Moved the shared Cypress multiline State note width into a State-owned note-label metric, applied
  it consistently in layout and SVG render measurement, removed the two now-derived note root pins,
  refreshed their layout goldens, tightened the root budget to `751`, and verified
  `xtask verify --strict`.
- Extended the existing State `Transition 1/2/3` edge-label browser metric to the matching
  `Transition 4/5` labels, removed the two simple State transition-label root pins, refreshed their
  layout goldens, tightened the root budget to `749` without growing text lookup debt, and verified
  `xtask verify --strict`.
- Moved the docs `A transition` browser width into State edge-label metrics, removed
  `upstream_docs_statediagram_transitions_014`, refreshed its layout golden, and tightened the root
  budget to `748`; verified `xtask verify --strict`.
- Moved the shared `Your state with spaces in it` browser width into State node-label metrics,
  removed `upstream_cypress_statediagram_v2_spec_v2_state_label_with_names_in_it_025` and
  `stress_state_batch5_state_keyword_spaces_and_alias_064`, refreshed their layout goldens, and
  tightened the root budget to `746`; verified `xtask verify --strict`.
- Extended the existing bold-italic State node-label metric to `id1/id2`, removed
  `upstream_pkgtests_state_style_spec_003`, refreshed its layout golden, and tightened the root
  budget to `745` without growing text lookup debt; verified `xtask verify --strict`.
- Added typed Mindmap cloud path bounds to root viewport derivation, removed
  `upstream_docs_mindmap_cloud_015`, and tightened the root budget to `744` without growing text
  lookup debt.
- Changed Mindmap plain wrapping-label layout to use wrapped/min-content HTML-like bounds instead
  of unwrapped paragraph width, removed three now-derived wrapping/icon root pins, and tightened the
  root budget to `741` without growing text lookup debt; refreshed the affected Mindmap layout
  goldens and verified `xtask verify --strict`.
- Removed five stale Mindmap root pins found by the post-wrapping disabled-root sweep and tightened
  the root budget to `736` without growing text lookup debt.
- Derived the Sequence small-font precedence fixture by rounding the Sequence text-dimension height
  and emitting root CSS with the configured actor label font size, removed
  `stress_sequence_font_size_precedence_090`, kept the boundary docs fixture pinned because its
  actor spacing still has a 16px message-width gap, and tightened the root budget to `735` without
  growing text lookup debt; refreshed the affected Sequence layout golden and verified
  `xtask verify --strict`.
- Routed Sequence `calculateTextDimensions` width measurement through the single-run SVG metric
  path, added the two docs boundary message-width facts, removed
  `upstream_docs_sequencediagram_boundary_008`, tightened the root budget to `734`, and raised the
  SVG text metric table budget to `186` with focused and full Sequence `parity-root` checks green.

## 2026-05-11

- Created the workstream document set for replacing State and Mindmap fixture-scoped root viewport
  overrides with typed layout or emitted-bounds derivation where practical.
- Recorded the State/Mindmap baseline counts, focused parity-root audit commands, disabled-root
  diagnostics, and clippy/nextest expectations for future code changes.
- Narrowed State's 72px border-label height inflation to classDef-compiled border styles, removed
  the now-derived `can_have_styles_applied` State root pin, and tightened the root budget to `759`.
- Refreshed the two affected State style layout goldens and verified full State normal DOM,
  full State `parity-root`, render clippy, xtask budget test, and `merman-render` nextest.
- Decoded Mermaid `encodeEntities` placeholders before State layout label measurement, moved the
  remaining `test({ foo: 'far' })` edge-label browser width into State text metrics, removed the
  two now-derived State root pins, and tightened the root budget to `757`.
- Derived Mindmap single-line delimiter label bounds for the Cypress square, rounded-rect, and
  circle root shape fixtures, refreshed their layout goldens, removed the three now-derived
  Mindmap root pins, and tightened the root budget to `754`.
- Derived the docs Mindmap circle root bounds by keeping plain Mindmap label measurement on raw
  font metrics instead of global HTML width overrides, removed `upstream_docs_mindmap_circle_011`,
  and tightened the root budget to `753`.
