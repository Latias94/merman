# Root Viewport Derivation Changelog

## 2026-05-13

- Removed two stale GitGraph root viewport pins
  (`upstream_cypress_gitgraph_spec_88_should_hide_branches_with_tb_orientation_when_showbranches_is_092`
  and `upstream_direction_bt`) after disabled-root mismatch cross-checking showed both now pass
  focused `parity-root` without the lookup; full GitGraph `parity-root`,
  `report-overrides --check-no-growth`, render/xtask clippy, and xtask override budget tests
  stayed green, and the root no-growth budget was tightened to `616` with GitGraph at `226`.

## 2026-05-12

- Moved the Sequence participant `<br/>` label line-width browser facts into the Sequence SVG
  metric table, removed `stress_long_participant_labels_br_031`, tightened the root budget to
  `618` with Sequence at `80`, kept the SVG metric table at `186` rows, and revalidated focused
  normal/disabled-root `parity-root` plus `report-overrides --check-no-growth`.
- Routed simple SVG bbox width probes through the existing Sequence metric table, replaced unused
  empty/zero-width rows with the `stress_br_in_messages_notes_011` no-wrap and wrap-prefix layout
  widths, removed its root pin, tightened the root budget to `619` with Sequence at `81`, kept the
  SVG metric table at `186` rows, and revalidated focused normal/disabled-root `parity-root` plus
  `report-overrides --check-no-growth`.
- Moved the wrapped Sequence HTML `<br/>` message-line browser metric into the Sequence SVG metric
  table, removed `stress_sequence_batch5_wrap_html_br_spans_042`, tightened the root budget to
  `620` with Sequence at `82`, kept the SVG metric table at `186` rows by replacing an unused
  stale row, and revalidated focused normal/disabled-root `parity-root` plus
  `report-overrides --check-no-growth`.
- Recalibrated the Sequence SVG metric for literal `<br \t/>` labels to match the upstream
  131px single-line bbox, removed the now-derived `html_br_variants_and_wrap` root pin, and
  revalidated focused normal/disabled-root `parity-root` plus `report-overrides --check-no-growth`.
- Derived wrapped Sequence `leftOf` note width and final rewrap behavior with leftOf-owned bbox
  calibration, refreshed the affected Sequence/ZenUML layout goldens, removed nine more Sequence
  root pins, tightened the root budget to `702` and Sequence root pins to `164`, and revalidated
  focused disabled-root checks, full Sequence `parity-root`, render clippy, render nextest, and
  `report-overrides --check-no-growth`.
- Fixed the Sequence `leftOf` note start recomputation after width clamping, added a shared SVG
  text metric fact for the long `Extremely utterly long line of longness which had previously
  overflown the actor box as it is much longer than what it should be` message, removed the six
  long-note/long-message Sequence root pins, dropped one stale `FRIENDS` row to keep the SVG text
  metric budget at `186`, tightened the root budget to `711`, and revalidated focused/full
  Sequence `parity-root` plus `report-overrides --check-no-growth`.
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
- Corrected the default trailing-semicolon Sequence font-family width facts for
  `Hello Bob, how are you?` and `Hello John, how are you?`, removed
  `title_and_accdescr_multiline`, `upstream_accessibility_single_line_spec`, and
  `upstream_docs_accessibility_sequence_diagram_014`, and tightened the root budget to `731`
  without growing the SVG text metric table.
- Removed the residual default-title Sequence pins `upstream_title_without_colon_spec` and
  `upstream_pkgtests_sequencediagram_spec_020`, tightening the root budget to `729` and Sequence
  root pins to `191` without growing the SVG text metric table.
- Removed the simple `Bob thinks` note-right Sequence trio
  `upstream_pkgtests_sequencediagram_spec_007`, `upstream_pkgtests_sequencediagram_spec_009`, and
  `upstream_pkgtests_sequencediagram_spec_042`, tightening the root budget to `726` and Sequence
  root pins to `188` without growing the SVG text metric table.
- Removed the whitespace/comment `Bob thinks` note-right Sequence trio
  `upstream_pkgtests_sequencediagram_spec_043`, `upstream_pkgtests_sequencediagram_spec_045`, and
  `upstream_pkgtests_sequencediagram_spec_046`, tightening the root budget to `723` and Sequence
  root pins to `185` without growing the SVG text metric table.
- Removed the loop/rect/nested-rect `Bob thinks` block note-right Sequence trio
  `upstream_pkgtests_sequencediagram_spec_054`, `upstream_pkgtests_sequencediagram_spec_055`, and
  `upstream_pkgtests_sequencediagram_spec_056`, tightening the root budget to `720` and Sequence
  root pins to `182` without growing the SVG text metric table.
- Removed the alt-control `Bob thinks` note-right Sequence trio
  `upstream_pkgtests_sequencediagram_spec_058`, `upstream_pkgtests_sequencediagram_spec_059`, and
  `upstream_alt_multiple_elses_spec`, tightening the root budget to `717` and Sequence root pins
  to `179` without growing the SVG text metric table.

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
