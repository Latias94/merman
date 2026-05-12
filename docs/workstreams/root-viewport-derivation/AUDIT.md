# Root Viewport Derivation Audit

This audit maps the workstream objective to concrete artifacts and gates.

## Objective

Replace fixture-scoped root viewport overrides with typed bounds derivation where practical,
starting with State and Mindmap, while keeping `parity-root` and strict release gates green.

## Prompt-to-Artifact Checklist

| Requirement | Artifact or command | Current state |
| --- | --- | --- |
| Track work in `docs/workstreams/root-viewport-derivation/` | This directory and its documents | Started |
| Start with State | `TODO.md`, `MILESTONES.md`, State override audit | In progress |
| Include Mindmap | `TODO.md`, `MILESTONES.md`, Mindmap override audit | Started |
| Replace fixture-scoped overrides where practical | Code changes plus generated table deletion | Started: eleven State root pins, thirteen Mindmap root pins, and one Sequence root pin removed |
| Keep `parity-root` green | Focused `compare-*-svgs --dom-mode parity-root` commands | Full State, Mindmap, and Sequence passes recorded |
| Keep clippy green for render edits | `cargo clippy -p merman-render --all-targets --all-features -- -D warnings` | Passed |
| Keep nextest green for shared behavior edits | `cargo nextest run` | Render crate and strict workspace nextest passed |
| Keep strict release gate green | `cargo run -p xtask -- verify --strict` | Passed |

## Current Baseline

The fearless-refactor closeout recorded these root viewport counts:

- State: `45` entries.
- Mindmap: `52` entries.

Current counts after the State style/entity-placeholder/note-label/transition-label/alias
node-label/package style node-label passes, the Mindmap single-line shape, docs circle plain-label,
docs cloud path-bounds, plain wrapping-label, and post-wrapping sweep passes, and the first
Sequence font-size precedence pass:

- State: `34` entries.
- Mindmap: `39` entries.
- Sequence: `197` entries.
- Root viewport total: `735` entries.
- Text lookup total: `484` entries. This is an intentional four-entry increase because State-owned
  edge-label, note-label, and node-label metrics replaced seven fixture-scoped root viewport pins.
  The simple transition-label pass reused an existing State edge-label metric arm, so it removed
  two more State root pins without increasing text lookup debt. The package style node-label pass
  reused an existing styled node-label metric arm, so it removed one more State root pin without
  increasing text lookup debt. The docs cloud and wrapping-label Mindmap passes did not add any
  text lookup debt.

The latest Mindmap focused disabled-root checks show the plain wrapping prose/icon trio and five
additional stale retained pins are covered by the current layout/bounds derivation. The remaining
retained entries still include long-word min-content drift, Markdown/HTML sanitization,
icon-bearing stress fixtures, shape profiles, and tree-wide transform drift. This workstream
therefore focuses on derivation work, not blind deletion.

The latest State disabled-root sweep still fails as expected with the 34 retained State root pins
acting as current guards. They cluster around HTML-sanitized notes, right-to-left scale bounds with
long IDs, dense or wrapping edge-label bounds, markdown edge labels, note/multiline-label geometry,
unicode/RTL text metrics, style/font precedence, and small browser float or lattice guards.

The latest Sequence check removed the small-font precedence root pin after the text-dimension height
path started rounding to Mermaid-like values. The boundary docs fixture remains pinned: disabled-root
diagnostics still show a 16px actor-column drift from message-width / actor-margin derivation.

## Focused Commands

```sh
cargo run -p xtask -- compare-state-svgs --check-dom --dom-mode parity-root --dom-decimals 3 --report-root-all
cargo run -p xtask -- compare-mindmap-svgs --check-dom --dom-mode parity-root --dom-decimals 3 --report-root-all
cargo run -p xtask -- compare-sequence-svgs --check-dom --dom-mode parity-root --dom-decimals 3 --report-root-all
cargo run -p xtask -- report-overrides --check-no-growth
cargo clippy -p merman-render --all-targets --all-features -- -D warnings
cargo run -p xtask -- verify --strict
```

PowerShell disabled-root diagnostic sweep:

```pwsh
$env:MERMAN_DISABLE_ROOT_VIEWPORT_OVERRIDES='1'
cargo run -p xtask -- compare-state-svgs --check-dom --dom-mode parity-root --dom-decimals 3 --report-root-all
cargo run -p xtask -- compare-mindmap-svgs --check-dom --dom-mode parity-root --dom-decimals 3 --report-root-all
Remove-Item Env:\MERMAN_DISABLE_ROOT_VIEWPORT_OVERRIDES
```

## Verification Log

- 2026-05-11: `cargo run -p xtask -- compare-state-svgs --check-dom --dom-mode parity-root
  --dom-decimals 3 --filter can_have_styles_applied` passed after deleting the State root pin.
- 2026-05-11: `cargo run -p xtask -- compare-state-svgs --check-dom --dom-mode parity
  --dom-decimals 3 --filter can_have_styles_applied` passed.
- 2026-05-11: `cargo run -p xtask -- compare-state-svgs --check-dom --dom-mode parity
  --dom-decimals 3` passed for all State fixtures.
- 2026-05-11: `cargo run -p xtask -- compare-state-svgs --check-dom --dom-mode parity-root
  --dom-decimals 3` passed for all State fixtures.
- 2026-05-11: `cargo run -p xtask -- report-overrides --check-no-growth` passed with root total
  `759` and State root count `44`.
- 2026-05-11: `cargo clippy -p merman-render --all-targets --all-features -- -D warnings`
  passed.
- 2026-05-11: `cargo test -p xtask override_growth_check_rejects_category_growth` passed.
- 2026-05-11: `cargo nextest run -p merman-render` passed with `148` tests after refreshing the
  two affected State layout golden snapshots.
- 2026-05-11: `cargo test -p merman-render
  state_entity_decode_handles_mermaid_placeholders_and_colon_entity` passed.
- 2026-05-11: `cargo run -p xtask -- compare-state-svgs --check-dom --dom-mode parity-root
  --dom-decimals 3 --filter v2_states_can_have_a_class_applied --report-root-all` passed after
  deleting the corresponding State root pin.
- 2026-05-11: `cargo run -p xtask -- compare-state-svgs --check-dom --dom-mode parity-root
  --dom-decimals 3 --filter should_render_a_state_diagram_and_set_the_correct_length_of_t
  --report-root-all` passed after deleting the corresponding State root pin.
- 2026-05-11: `cargo test -p merman-render
  mindmap_label_text_for_layout_trims_single_line_delimiter_text` passed.
- 2026-05-11: `cargo run -p xtask -- compare-mindmap-svgs --check-dom --dom-mode parity-root
  --dom-decimals 3 --filter upstream_cypress_mindmap_spec_square_shape_011 --report-root-all`
  passed after deleting the corresponding Mindmap root pin.
- 2026-05-11: `cargo run -p xtask -- compare-mindmap-svgs --check-dom --dom-mode parity-root
  --dom-decimals 3 --filter upstream_cypress_mindmap_spec_circle_shape_013 --report-root-all`
  passed after deleting the corresponding Mindmap root pin.
- 2026-05-11: `cargo run -p xtask -- compare-mindmap-svgs --check-dom --dom-mode parity-root
  --dom-decimals 3 --filter upstream_cypress_mindmap_spec_rounded_rect_shape_012 --report-root-all`
  passed after deleting the corresponding Mindmap root pin.
- 2026-05-11: `cargo run -p xtask -- compare-mindmap-svgs --check-dom --dom-mode parity
  --dom-decimals 3` passed for all Mindmap fixtures.
- 2026-05-11: `cargo run -p xtask -- compare-mindmap-svgs --check-dom --dom-mode parity-root
  --dom-decimals 3` passed for all Mindmap fixtures.
- 2026-05-11: `cargo run -p xtask -- report-overrides --check-no-growth` passed with root total
  `754` and Mindmap root count `49`.
- 2026-05-11: `cargo clippy -p merman-render --all-targets --all-features -- -D warnings`
  passed after the Mindmap layout change.
- 2026-05-11: `cargo nextest run -p merman-render` passed with `150` tests after refreshing the
  three affected Mindmap layout golden snapshots.
- 2026-05-11: `cargo run -p xtask -- verify --strict` passed, including workspace nextest
  (`1018` passed, `3` skipped), normal SVG DOM parity, and root SVG DOM parity.
- 2026-05-11: `cargo test -p merman-render
  mindmap_plain_label_measurement_ignores_cross_diagram_html_overrides` passed.
- 2026-05-11: with `MERMAN_DISABLE_ROOT_VIEWPORT_OVERRIDES=1`,
  `cargo run -p xtask -- compare-mindmap-svgs --check-dom --dom-mode parity-root --dom-decimals 3
  --filter upstream_docs_mindmap_circle_011 --report-root-all` passed after deleting the docs
  circle Mindmap root pin.
- 2026-05-11: focused disabled-root checks for `upstream_docs_mindmap_bang_013` and
  `upstream_docs_mindmap_cloud_015` still failed with real shape/root drift, so those entries
  remain pinned.
- 2026-05-11: `cargo run -p xtask -- report-overrides --check-no-growth` passed with root total
  `753` and Mindmap root count `48`.
- 2026-05-11: `cargo run -p xtask -- compare-mindmap-svgs --check-dom --dom-mode parity
  --dom-decimals 3` passed for all Mindmap fixtures.
- 2026-05-11: `cargo run -p xtask -- compare-mindmap-svgs --check-dom --dom-mode parity-root
  --dom-decimals 3` passed for all Mindmap fixtures.
- 2026-05-11: `cargo clippy -p merman-render --all-targets --all-features -- -D warnings`
  passed after the docs circle Mindmap layout change.
- 2026-05-11: `cargo nextest run -p merman-render` passed with `151` tests.
- 2026-05-11: `cargo run -p xtask -- verify --strict` passed, including workspace nextest
  (`1019` passed, `3` skipped), normal SVG DOM parity, and root SVG DOM parity.
- 2026-05-12: with `MERMAN_DISABLE_ROOT_VIEWPORT_OVERRIDES=1`,
  `cargo run -p xtask -- compare-state-svgs --check-dom --dom-mode parity-root --dom-decimals 3
  --report-root-all` failed as expected with `284` root-delta rows. Crossing those rows with
  `state_root_overrides_11_12_2.rs` classified the current 42 retained State root pins by drift
  family in `TODO.md`.
- 2026-05-12: `cargo run -p xtask -- compare-state-svgs --check-dom --dom-mode parity-root
  --dom-decimals 3` passed with the retained State root pins enabled.
- 2026-05-12: `cargo run -p xtask -- compare-mindmap-svgs --check-dom --dom-mode parity-root
  --dom-decimals 3` passed with the retained Mindmap root pins enabled.
- 2026-05-12: `cargo run -p xtask -- report-overrides --check-no-growth` passed with root total
  `753`, State root count `42`, Mindmap root count `48`, text lookup total `481`, and zero manual
  raw SVG/path bridges.
- 2026-05-12: `cargo run -p xtask -- compare-state-svgs --check-dom --dom-mode parity-root
  --dom-decimals 3 --filter
  upstream_cypress_statediagram_spec_should_render_a_note_with_multiple_lines_in_it_009
  --report-root-all` passed after replacing the fixture root pin with State note-label bounds.
- 2026-05-12: `cargo run -p xtask -- compare-state-svgs --check-dom --dom-mode parity-root
  --dom-decimals 3 --filter
  upstream_cypress_statediagram_v2_spec_v2_should_render_a_note_with_multiple_lines_in_it_010
  --report-root-all` passed after replacing the paired v2 fixture root pin with the same
  State-owned note-label metric.
- 2026-05-12: refreshed the two affected State note layout goldens with
  `cargo run -p xtask -- update-layout-snapshots --filter <fixture>`.
- 2026-05-12: `cargo run -p xtask -- compare-state-svgs --check-dom --dom-mode parity
  --dom-decimals 3` passed for all State fixtures after the note-label pass.
- 2026-05-12: `cargo run -p xtask -- compare-state-svgs --check-dom --dom-mode parity-root
  --dom-decimals 3` passed for all State fixtures after the note-label pass.
- 2026-05-12: `cargo run -p xtask -- compare-mindmap-svgs --check-dom --dom-mode parity-root
  --dom-decimals 3` passed after the State note-label pass.
- 2026-05-12: `cargo run -p xtask -- report-overrides --check-no-growth` passed with root total
  `751`, State root count `40`, Mindmap root count `48`, text lookup total `482`, and zero manual
  raw SVG/path bridges.
- 2026-05-12: `cargo clippy -p merman-render --all-targets --all-features -- -D warnings`
  passed after the State note-label render/layout change.
- 2026-05-12: `cargo nextest run -p merman-render` passed with `151` tests after refreshing the
  two affected State note layout golden snapshots.
- 2026-05-12: with `MERMAN_DISABLE_ROOT_VIEWPORT_OVERRIDES=1`,
  `cargo run -p xtask -- compare-state-svgs --check-dom --dom-mode parity-root --dom-decimals 3
  --report-root-all` still failed as expected; the remaining failures correspond to retained State
  root guards rather than the removed multiline note pair.
- 2026-05-12: `cargo run -p xtask -- verify --strict` passed after the State note-label pass,
  including `cargo fmt`, workspace all-features check/clippy, override no-growth, feature matrix,
  workspace nextest (`1019` passed, `3` skipped), normal SVG DOM parity, and root SVG DOM parity.
- 2026-05-12: `cargo run -p xtask -- compare-state-svgs --check-dom --dom-mode parity-root
  --dom-decimals 3 --filter
  upstream_cypress_statediagram_spec_should_render_a_simple_state_diagrams_with_labels_013
  --report-root-all` passed after deriving the `Transition 4/5` edge-label bounds from the existing
  State transition metric family.
- 2026-05-12: `cargo run -p xtask -- compare-state-svgs --check-dom --dom-mode parity-root
  --dom-decimals 3 --filter
  upstream_cypress_statediagram_v2_spec_v2_should_render_a_simple_state_diagrams_with_labels_014
  --report-root-all` passed after deleting the paired v2 simple-label State root pin.
- 2026-05-12: refreshed the two affected simple State transition-label layout goldens with
  `cargo run -p xtask -- update-layout-snapshots --filter <fixture>`.
- 2026-05-12: `cargo run -p xtask -- compare-state-svgs --check-dom --dom-mode parity
  --dom-decimals 3` passed for all State fixtures after the transition-label pass.
- 2026-05-12: `cargo run -p xtask -- compare-state-svgs --check-dom --dom-mode parity-root
  --dom-decimals 3` passed for all State fixtures after the transition-label pass.
- 2026-05-12: `cargo run -p xtask -- report-overrides --check-no-growth` passed with root total
  `749`, State root count `38`, Mindmap root count `48`, text lookup total `482`, and zero manual
  raw SVG/path bridges.
- 2026-05-12: `cargo clippy -p merman-render --all-targets --all-features -- -D warnings`
  passed after the State transition-label metric change.
- 2026-05-12: `cargo nextest run -p merman-render` passed with `151` tests after refreshing the
  two affected simple State transition-label layout golden snapshots.
- 2026-05-12: with `MERMAN_DISABLE_ROOT_VIEWPORT_OVERRIDES=1`,
  `cargo run -p xtask -- compare-state-svgs --check-dom --dom-mode parity-root --dom-decimals 3
  --report-root-all` still failed as expected after the transition-label pass; the remaining
  failures correspond to the current 38 retained State root guards.
- 2026-05-12: `cargo run -p xtask -- verify --strict` passed after the transition-label pass,
  including `cargo fmt`, workspace all-features check/clippy, override no-growth, feature matrix,
  workspace nextest (`1019` passed, `1` leaky, `3` skipped), normal SVG DOM parity, and root SVG
  DOM parity.
- 2026-05-12: disabled-root diagnostics for
  `upstream_cypress_statediagram_spec_should_render_a_state_with_a_note_together_with_another_state_008`
  showed the v1/v2 pair should remain pinned for now: the root delta comes from note-cluster rect
  bounds, while direct State node, note-label, and `With +,-` edge-label widths are already aligned.
- 2026-05-12: `cargo run -p xtask -- compare-state-svgs --check-dom --dom-mode parity-root
  --dom-decimals 3 --filter upstream_docs_statediagram_transitions_014 --report-root-all` passed
  after replacing the docs `A transition` root pin with a State edge-label metric.
- 2026-05-12: refreshed the affected docs State transition layout golden with
  `cargo run -p xtask -- update-layout-snapshots --filter upstream_docs_statediagram_transitions_014`.
- 2026-05-12: `cargo run -p xtask -- compare-state-svgs --check-dom --dom-mode parity
  --dom-decimals 3` passed for all State fixtures after the docs transition-label pass.
- 2026-05-12: `cargo run -p xtask -- compare-state-svgs --check-dom --dom-mode parity-root
  --dom-decimals 3` passed for all State fixtures after the docs transition-label pass.
- 2026-05-12: `cargo run -p xtask -- report-overrides --check-no-growth` passed with root total
  `748`, State root count `37`, Mindmap root count `48`, text lookup total `483`, and zero manual
  raw SVG/path bridges.
- 2026-05-12: `cargo clippy -p merman-render --all-targets --all-features -- -D warnings`
  passed after the docs transition-label metric change.
- 2026-05-12: `cargo nextest run -p merman-render` passed with `151` tests after refreshing the
  affected docs State transition layout golden.
- 2026-05-12: with `MERMAN_DISABLE_ROOT_VIEWPORT_OVERRIDES=1`,
  `cargo run -p xtask -- compare-state-svgs --check-dom --dom-mode parity-root --dom-decimals 3
  --report-root-all` still failed as expected after the docs transition-label pass; the removed
  `upstream_docs_statediagram_transitions_014` row no longer appears in the retained State failures.
- 2026-05-12: `cargo run -p xtask -- verify --strict` passed after the docs transition-label pass,
  including `cargo fmt`, workspace all-features check/clippy, override no-growth, feature matrix,
  workspace nextest (`1019` passed, `3` skipped), normal SVG DOM parity, and root SVG DOM parity.
- 2026-05-12: disabled-root focused diagnostics for
  `upstream_cypress_statediagram_v2_spec_v2_state_label_with_names_in_it_025` and
  `stress_state_batch5_state_keyword_spaces_and_alias_064` showed both retained root pins were
  guarding the same State node-label width drift: upstream `Your state with spaces in it` measured
  `193.921875px`, while local measurement produced `195.765625px`.
- 2026-05-12: `cargo run -p xtask -- compare-state-svgs --check-dom --dom-mode parity-root
  --dom-decimals 3 --filter
  upstream_cypress_statediagram_v2_spec_v2_state_label_with_names_in_it_025 --report-root-all`
  passed after replacing the fixture root pin with a State node-label metric.
- 2026-05-12: `cargo run -p xtask -- compare-state-svgs --check-dom --dom-mode parity-root
  --dom-decimals 3 --filter stress_state_batch5_state_keyword_spaces_and_alias_064
  --report-root-all` passed after the same State node-label metric replaced the stress fixture root
  pin.
- 2026-05-12: refreshed the two affected alias node-label layout goldens with
  `cargo run -p xtask -- update-layout-snapshots --filter <fixture>`.
- 2026-05-12: `cargo run -p xtask -- compare-state-svgs --check-dom --dom-mode parity
  --dom-decimals 3` passed for all State fixtures after the alias node-label pass.
- 2026-05-12: `cargo run -p xtask -- compare-state-svgs --check-dom --dom-mode parity-root
  --dom-decimals 3` passed for all State fixtures after the alias node-label pass.
- 2026-05-12: `cargo run -p xtask -- report-overrides --check-no-growth` passed with root total
  `746`, State root count `35`, Mindmap root count `48`, text lookup total `484`, and zero manual
  raw SVG/path bridges.
- 2026-05-12: `cargo test -p xtask override_growth_check_rejects_category_growth` passed after
  tightening the root and text lookup budgets.
- 2026-05-12: `cargo clippy -p merman-render --all-targets --all-features -- -D warnings` and
  `cargo clippy -p xtask --all-targets --all-features -- -D warnings` passed after the alias
  node-label metric and budget changes.
- 2026-05-12: `cargo nextest run -p merman-render` passed with `151` tests after refreshing the two
  affected alias node-label layout goldens.
- 2026-05-12: `cargo run -p xtask -- verify --strict` passed after the alias node-label pass,
  including `cargo fmt`, workspace all-features check/clippy, override no-growth, feature matrix,
  workspace nextest (`1019` passed, `3` skipped), normal SVG DOM parity, and root SVG DOM parity.
- 2026-05-12: disabled-root focused diagnostics for `upstream_pkgtests_state_style_spec_003`
  showed the retained root pin was guarding bold-italic State node-label width drift: upstream
  `id1/id2` measured `24.09375px`, while local measurement produced `24.203125px` and
  `25.78125px`.
- 2026-05-12: `cargo run -p xtask -- compare-state-svgs --check-dom --dom-mode parity-root
  --dom-decimals 3 --filter upstream_pkgtests_state_style_spec_003 --report-root-all` passed after
  extending the existing bold-italic State node-label metric family to `id1/id2`.
- 2026-05-12: refreshed the affected package style layout golden with
  `cargo run -p xtask -- update-layout-snapshots --filter upstream_pkgtests_state_style_spec_003`.
- 2026-05-12: `cargo run -p xtask -- compare-state-svgs --check-dom --dom-mode parity
  --dom-decimals 3` passed for all State fixtures after the package style node-label pass.
- 2026-05-12: `cargo run -p xtask -- compare-state-svgs --check-dom --dom-mode parity-root
  --dom-decimals 3` passed for all State fixtures after the package style node-label pass.
- 2026-05-12: `cargo run -p xtask -- report-overrides --check-no-growth` passed with root total
  `745`, State root count `34`, Mindmap root count `48`, text lookup total `484`, and zero manual
  raw SVG/path bridges.
- 2026-05-12: `cargo test -p xtask override_growth_check_rejects_category_growth` passed after
  tightening the root budget without growing the text lookup budget.
- 2026-05-12: `cargo clippy -p merman-render --all-targets --all-features -- -D warnings`,
  `cargo clippy -p xtask --all-targets --all-features -- -D warnings`, and
  `cargo nextest run -p merman-render` passed after the package style node-label pass.
- 2026-05-12: `cargo run -p xtask -- verify --strict` passed after the package style node-label
  pass, including `cargo fmt`, workspace all-features check/clippy, override no-growth, feature
  matrix, workspace nextest (`1019` passed, `3` skipped), normal SVG DOM parity, and root SVG DOM
  parity.
- 2026-05-12: disabled-root diagnostics for
  `upstream_cypress_statediagram_v2_spec_v2_width_of_compound_state_should_grow_with_title_if_title_is_wi_024`
  showed it should remain pinned for now: the title `Long state name 2` width is off by `1px`, but
  the root also carries a compound cluster-origin delta (`viewBox` local x becomes negative), so
  this needs a compound root-origin/bounds rule rather than only another text metric.
- 2026-05-12: disabled-root diagnostics for
  `upstream_cypress_statediagram_v2_spec_should_let_styles_take_precedence_over_classes_035`
  showed it should remain pinned for now: replacing one root pin would require two narrowly scoped
  node-label widths (`Should NOT be white` and `BState`), which is worse debt than the retained root
  guard until style text measurement has a broader derivation rule.
- 2026-05-12: disabled-root diagnostics for
  `upstream_cypress_statediagram_v2_spec_v2_it_should_be_possible_to_use_a_choice_022` showed it
  should remain pinned for now: the root delta is distributed across several small plain state-label
  width differences, not a single reusable browser measurement fact.
- 2026-05-12: `cargo test -p merman-render viewport_bounds_include_cloud_path_bbox` passed after
  adding a focused guard that Mindmap root viewport bounds include typed cloud SVG path bbox
  extents beyond the layout rectangle.
- 2026-05-12: with `MERMAN_DISABLE_ROOT_VIEWPORT_OVERRIDES=1`, `cargo run -p xtask --
  compare-mindmap-svgs --check-dom --dom-mode parity-root --dom-decimals 3 --filter
  upstream_docs_mindmap_cloud_015 --report-root-all` passed after replacing the fixture root pin
  with typed cloud path bounds.
- 2026-05-12: disabled-root focused checks for `upstream_docs_mindmap_bang_013`,
  `upstream_docs_mindmap_hexagon_017`,
  `upstream_cypress_mindmap_spec_blang_and_cloud_shape_006`,
  `upstream_cypress_mindmap_spec_blang_and_cloud_shape_with_icons_007`, and
  `upstream_node_types` still failed after typed cloud/bang path bounds. The docs bang drift is now
  a small browser text/shape float delta, docs hexagon is a roughjs/text metric delta, and the
  Cypress/profile fixtures still carry tree-wide transform drift, so these remain pinned for now.
- 2026-05-12: `cargo run -p xtask -- compare-mindmap-svgs --check-dom --dom-mode parity-root
  --dom-decimals 3` and `cargo run -p xtask -- compare-mindmap-svgs --check-dom --dom-mode parity
  --dom-decimals 3` passed for all Mindmap fixtures after deleting
  `upstream_docs_mindmap_cloud_015`.
- 2026-05-12: `cargo run -p xtask -- report-overrides --check-no-growth` passed with root total
  `744`, State root count `34`, Mindmap root count `47`, text lookup total `484`, and zero manual
  raw SVG/path bridges.
- 2026-05-12: with `MERMAN_DISABLE_ROOT_VIEWPORT_OVERRIDES=1`, focused `parity-root` checks passed
  for `upstream_cypress_mindmap_spec_a_root_with_wrapping_text_and_a_shape_003`,
  `upstream_cypress_mindmap_spec_text_should_wrap_with_icon_010`, and
  `upstream_html_demos_mindmap_mindmap_with_root_wrapping_text_and_a_shape_002` after Mindmap
  plain labels started using wrapped/min-content HTML-like bounds instead of unwrapped paragraph
  width.
- 2026-05-12: focused disabled-root checks for the related long-word and stress icon fixtures still
  failed with real remaining root drift, so those entries remain pinned for now:
  `upstream_cypress_mindmap_spec_a_root_with_wrapping_text_and_long_words_that_exceed_width_004`,
  `stress_long_labels_br_icons_002`, `stress_mindmap_long_words_wrapping_016`,
  `stress_wrap_long_word_008`,
  `upstream_cypress_mindmap_spec_formatted_label_with_linebreak_and_a_wrapping_label_and_emojis_017`,
  `stress_mindmap_markdown_emphasis_icons_014`, and `stress_mindmap_icons_multi_packs_025`.
- 2026-05-12: `cargo run -p xtask -- report-overrides --check-no-growth` passed with root total
  `741`, State root count `34`, Mindmap root count `44`, text lookup total `484`, and zero manual
  raw SVG/path bridges.
- 2026-05-12: a focused disabled-root sweep across the remaining Mindmap root override keys found
  five stale retained pins already covered by the latest layout/bounds derivation:
  `upstream_cypress_mindmap_spec_braches_with_shapes_and_labels_009`,
  `upstream_docs_tidy_tree_example_usage_002`, `stress_label_escaping_012`,
  `stress_mindmap_delimiters_and_quotes_019`, and `stress_mindmap_unicode_rtl_mixed_029`.
- 2026-05-12: `cargo run -p xtask -- report-overrides --check-no-growth` passed with root total
  `736`, State root count `34`, Mindmap root count `39`, text lookup total `484`, and zero manual
  raw SVG/path bridges.
- 2026-05-12: `cargo run -p xtask -- compare-sequence-svgs --check-dom --dom-mode parity-root
  --dom-decimals 3 --filter stress_sequence_font_size_precedence_090 --report-root-all` passed
  after deleting the Sequence small-font precedence root pin.
- 2026-05-12: `cargo run -p xtask -- compare-sequence-svgs --check-dom --dom-mode parity-root
  --dom-decimals 3 --filter upstream_docs_sequencediagram_boundary_008 --report-root-all` passed
  with the boundary root pin retained. Removing it still leaves local `max-width: 487px` versus
  upstream `471px`, so this remains message-width / actor-margin derivation debt.
- 2026-05-12: `cargo run -p xtask -- compare-sequence-svgs --check-dom --dom-mode parity-root
  --dom-decimals 3 --report-root-all` passed for all Sequence fixtures.
- 2026-05-12: `cargo run -p xtask -- report-overrides --check-no-growth` passed with root total
  `735`, State root count `34`, Mindmap root count `39`, Sequence root count `197`, text lookup
  total `484`, and zero manual raw SVG/path bridges.
- 2026-05-12: `cargo run -p xtask -- verify --strict` passed after the Sequence pass. The strict
  gate included `cargo fmt --check`, workspace `cargo clippy --all-targets --all-features
  -- -D warnings`, workspace `cargo nextest run` (`1022` passed, `3` skipped), override
  no-growth, feature matrix checks, normal SVG DOM parity, and root SVG DOM parity.
- 2026-05-12: refreshed the 29 affected Mindmap layout golden snapshots after the wrapped-label
  layout rule changed node dimensions and tree positions.
- 2026-05-12: `cargo test -p merman-render
  mindmap_plain_wrapping_label_uses_wrapped_container_width`, focused Mindmap `parity-root` checks
  for the three removed root pins, full Mindmap `parity-root`, full Mindmap `parity`,
  `cargo clippy -p merman-render --all-targets --all-features -- -D warnings`,
  `cargo clippy -p xtask --all-targets --all-features -- -D warnings`,
  `cargo nextest run -p merman-render`, `cargo test -p xtask
  override_growth_check_rejects_category_growth`, and `cargo run -p xtask -- verify --strict`
  passed after the wrapping-label pass. The strict gate included workspace nextest
  (`1021` passed, `3` skipped), normal SVG DOM parity, and root SVG DOM parity.

## Open Risks

- Root `viewBox` / `max-width` can be affected by browser-only `getBBox()` behavior inside
  `<foreignObject>`.
- Some entries may remain necessary until text measurement or shape bbox logic improves.
- A root table deletion can pass normal DOM parity but fail `parity-root`, so both modes must be
  checked for touched diagram families.
