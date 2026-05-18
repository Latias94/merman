# Root Viewport Derivation TODO

This backlog tracks root viewport override replacement work. Deleting an entry is only complete
when a typed/layout/emitted-bounds rule explains the same root `viewBox` and `max-width`.

## P0: Workstream Baseline

- [x] Create the workstream document set.
- [x] Record the current State and Mindmap root override baseline.
- [x] Confirm the focused audit commands for State and Mindmap.
- [x] Add clippy and nextest expectations to the success criteria.

## P1: State Root Derivation

- [x] Classify the then-current 42 State root pins by drift family.
  Evidence: the 2026-05-12 disabled-root State `parity-root` sweep produced 284 root-delta rows;
  crossing the report with `state_root_overrides_11_12_2.rs` identified the 42 retained pins and
  the drift families at that point:
  - right-to-left direction and scale bounds.
  - dense or wrapping edge-label bounds.
  - note and multiline-label bounds.
  - styled/classed state shape bounds.
  - small browser float/rounding deltas.
  Highest-impact retained fixtures are led by HTML-sanitized notes, RL/scale long IDs, wrapped
  edge labels, dense graph labels, markdown edge labels, unicode/RTL labels, and font/style
  precedence cases.
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
- [x] Replace the shared State multiline note pair with note-label bounds derivation.
  Evidence: a State-owned note label browser width now drives both layout and render measurement
  for the shared multiline Cypress note text, replacing two fixture-scoped root pins:
  `upstream_cypress_statediagram_spec_should_render_a_note_with_multiple_lines_in_it_009` and
  `upstream_cypress_statediagram_v2_spec_v2_should_render_a_note_with_multiple_lines_in_it_010`.
- [x] Tighten the current root budget after the note-label pass.
  Evidence: State root count is now `40`, root viewport total is `751`, and the text lookup budget
  is explicitly `482` because one reusable State note-label browser metric replaced two root pins.
- [x] Replace the simple State transition-label pair with the existing edge-label metric family.
  Evidence: the existing `Transition 1/2/3` State edge-label metric now also covers
  `Transition 4/5`, replacing two fixture-scoped root pins without growing the text lookup budget:
  `upstream_cypress_statediagram_spec_should_render_a_simple_state_diagrams_with_labels_013` and
  `upstream_cypress_statediagram_v2_spec_v2_should_render_a_simple_state_diagrams_with_labels_014`.
- [x] Tighten the current root budget after the simple transition-label pass.
  Evidence: State root count is now `38`, root viewport total is `749`, and the text lookup budget
  remains `482` because the pass reused an existing State edge-label metric arm.
- [x] Replace the docs State transition edge-label root pin with edge-label bounds derivation.
  Evidence: the browser-measured `A transition` State edge-label width replaces
  `upstream_docs_statediagram_transitions_014`.
- [x] Tighten the current root budget after the docs transition-label pass.
  Evidence: State root count is now `37`, root viewport total is `748`, and the text lookup budget
  is explicitly `483` because one State edge-label browser metric replaced one fixture root pin.
- [x] Replace the shared State alias node-label pair with node-label bounds derivation.
  Evidence: the browser-measured `Your state with spaces in it` State node-label width replaces
  two fixture-scoped root pins:
  `upstream_cypress_statediagram_v2_spec_v2_state_label_with_names_in_it_025` and
  `stress_state_batch5_state_keyword_spaces_and_alias_064`.
- [x] Tighten the current root budget after the alias node-label pass.
  Evidence: State root count is now `35`, root viewport total is `746`, and the text lookup budget
  is explicitly `484` because one State node-label browser metric replaced two fixture root pins.
- [x] Replace the matching package style `id1/id2` pair with the existing styled node-label metric
  family.
  Evidence: the existing bold-italic `id3/id4` State node-label metric now also covers `id1/id2`,
  replacing `upstream_pkgtests_state_style_spec_003` without growing text lookup debt.
- [x] Tighten the current root budget after the package style node-label pass.
  Evidence: State root count is now `34`, root viewport total is `745`, and the text lookup budget
  remains `484` because the pass reused an existing styled State node-label metric arm.
- [x] Classify the `state_with_a_note_together_with_another_state` v1/v2 pair as retained for now.
  Evidence: disabled-root diagnostics show the remaining drift comes from note-cluster rect bounds;
  direct node, note-label, and edge-label widths are already effectively aligned, so this needs a
  Dagre noteGroup/cluster bounds rule rather than another text width lookup.
- [x] Classify the immediate post-style-pass State candidates that should remain pinned for now.
  Evidence: disabled-root diagnostics show:
  `upstream_cypress_statediagram_v2_spec_v2_width_of_compound_state_should_grow_with_title_if_title_is_wi_024`
  still combines a `rectWithTitle` title-width delta with a compound root-origin/cluster-transform
  delta; `upstream_cypress_statediagram_v2_spec_should_let_styles_take_precedence_over_classes_035`
  would require two very fixture-like style text widths to replace one root pin; and
  `upstream_cypress_statediagram_v2_spec_v2_it_should_be_possible_to_use_a_choice_022` is driven
  by several small plain state-label width deltas rather than one reusable rule.
- [x] Prove State normal DOM parity and `parity-root` stay green.
  Evidence: full `compare-state-svgs` passed in both `parity` and `parity-root` DOM modes after
  the note-label, transition-label, alias node-label, and package style node-label passes.
- [x] Run focused State code-quality checks for this pass.
  Evidence: `cargo clippy -p merman-render --all-targets --all-features -- -D warnings`,
  `cargo test -p xtask override_growth_check_rejects_category_growth`, and
  `cargo nextest run -p merman-render` passed after the note-label, transition-label, and alias
  node-label passes. The same checks also passed after the package style node-label pass.

## P1: Mindmap Root Derivation

- [x] Classify the 52 remaining Mindmap root pins by drift family.
  Known initial families:
  - wrapping text and long-word bounds.
  - icon-bearing node bounds.
  - shape-specific SVG bbox bounds.
  - markdown / HTML sanitization label bounds.
  - tree-wide transform and tidy-tree bounds.
  Evidence: disabled-root Mindmap diagnostics still show wrapping text, HTML sanitization,
  icon-bearing nodes, shape profiles, and tree-wide transform drift as the dominant remaining
  families.
- [x] Replace one low-risk Mindmap fixture group with typed or emitted-bounds derivation.
  Evidence: `mindmap_label_text_for_layout` now trims delimiter-created labels that contain a
  single non-empty text line, while preserving true multi-line labels and raw SVG text emission.
- [x] Delete the corresponding generated Mindmap root entries and tighten the root budget.
  Evidence: removed the Cypress `square_shape_011`, `rounded_rect_shape_012`, and
  `circle_shape_013` Mindmap root pins; Mindmap root count is now `49`, and the root no-growth
  budget is now `754`.
- [x] Replace the docs circle cross-diagram HTML-width leakage with Mindmap-owned plain-label
  measurement.
  Evidence: Mindmap plain labels now use raw font metrics rather than fixture-derived HTML width
  overrides from other diagram families, allowing `upstream_docs_mindmap_circle_011` to derive its
  root viewport without a fixture pin.
- [x] Tighten the current Mindmap root budget after the docs circle pass.
  Evidence: Mindmap root count is now `48`, root viewport total is `753`, and text lookup debt did
  not grow.
- [x] Replace the docs cloud shape root pin with emitted path bbox derivation.
  Evidence: Mindmap root viewport derivation now includes typed `cloud` SVG path bounds in addition
  to the layout rectangle and label bounds, allowing `upstream_docs_mindmap_cloud_015` to derive its
  root viewport without a fixture pin.
- [x] Tighten the current Mindmap root budget after the docs cloud pass.
  Evidence: Mindmap root count is now `47`, root viewport total is `744`, and text lookup debt did
  not grow.
- [x] Replace the shared plain wrapping-label group with wrapped container bounds derivation.
  Evidence: Mindmap plain HTML-like labels now use the wrapped/min-content measurement result
  directly instead of re-expanding normal prose to its unwrapped paragraph width, replacing three
  fixture-scoped root pins:
  `upstream_cypress_mindmap_spec_a_root_with_wrapping_text_and_a_shape_003`,
  `upstream_cypress_mindmap_spec_text_should_wrap_with_icon_010`, and
  `upstream_html_demos_mindmap_mindmap_with_root_wrapping_text_and_a_shape_002`.
- [x] Tighten the current Mindmap root budget after the wrapping-label pass.
  Evidence: Mindmap root count is now `44`, root viewport total is `741`, and text lookup debt did
  not grow.
- [x] Sweep remaining Mindmap root pins after the wrapping-label derivation.
  Evidence: focused disabled-root checks showed five retained pins were already covered by the new
  layout/bounds rules, replacing:
  `upstream_cypress_mindmap_spec_braches_with_shapes_and_labels_009`,
  `upstream_docs_tidy_tree_example_usage_002`, `stress_label_escaping_012`,
  `stress_mindmap_delimiters_and_quotes_019`, and `stress_mindmap_unicode_rtl_mixed_029`.
- [x] Tighten the current Mindmap root budget after the post-wrapping sweep.
  Evidence: Mindmap root count is now `39`, root viewport total is `736`, and text lookup debt did
  not grow.
- [x] Classify the immediate docs shape candidates that remain pinned for now.
  Evidence: disabled-root checks show `upstream_docs_mindmap_bang_013` is now down to a small
  browser text/shape float delta after the typed bang path bbox is included, while
  `upstream_docs_mindmap_hexagon_017` remains a small roughjs/text metric delta. The Cypress
  bang/cloud combinations still have larger tree-wide transform drift, so these should not be
  replaced by fixture-like text widths.
- [x] Prove Mindmap normal DOM parity and `parity-root` stay green.
  Evidence: full `compare-mindmap-svgs` passed in both `parity` and `parity-root` DOM modes after
  all Mindmap passes.

## P2: Larger Buckets

- [x] Revisit Flowchart after State/Mindmap patterns are proven.
  Evidence: the 2026-05-14 Flowchart disabled-root audit crossed
  `target/flowchart_disabled_root_2026-05-14.txt` with
  `flowchart_root_overrides_11_12_2.rs`. The table still covers `95` fixture keys even though
  `report-overrides` counted `87` inventory entries after or-pattern compression. All `95`
  retained keys still appear in the disabled-root mismatch set, with `0` stale retained pins and
  `0` missing pins. The top retained families are rank-spacing/chained-statement and edge-geometry
  height drift, icon/FontAwesome line-break and glyph bounds, subgraph title/title-margin spacing,
  shape profile and all-pairs geometry, wrapping/Unicode/style/long-label measurement, plus small
  browser-float guards. The follow-up rankSpacing, chained-statement, icon-only multiline, and
  FontAwesome label-boundary passes removed the next derivable Flowchart root pins, so current
  `report-overrides` counts `83` Flowchart inventory entries, and the latest disabled-root audit
  now reports `91` retained fixture keys, `0` stale pins, and `0` missing pins.
- [x] Revisit the first low-risk Sequence root candidate after State/Mindmap patterns are proven.
  Evidence: Sequence small-font text height now rounds Mermaid-like
  `calculateTextDimensions(...).height`, the SVG root CSS follows the configured actor label font
  size, and `stress_sequence_font_size_precedence_090` passes `parity-root` without a root pin.
- [x] Derive the Sequence boundary docs fixture through message-width measurement.
  Evidence: Sequence now measures `calculateTextDimensions` widths through the single-run SVG text
  path and includes the two boundary message-width facts; `upstream_docs_sequencediagram_boundary_008`
  passes focused `parity-root` with `MERMAN_DISABLE_ROOT_VIEWPORT_OVERRIDES=1`, so its root pin was
  deleted.
- [x] Derive the small Sequence title/accessibility root cluster through default-message widths.
  Evidence: the Sequence SVG metric facts for `Hello Bob, how are you?` and
  `Hello John, how are you?` now reflect Mermaid's default trailing-semicolon font family, and
  `title_and_accdescr_multiline`, `upstream_accessibility_single_line_spec`, and
  `upstream_docs_accessibility_sequence_diagram_014` pass focused disabled-root `parity-root`, so
  their root pins were deleted.
- [x] Remove the residual Sequence default-title pair covered by the same message-width facts.
  Evidence: `upstream_title_without_colon_spec` and `upstream_pkgtests_sequencediagram_spec_020`
  pass focused disabled-root `parity-root`, so their root pins were deleted without growing the
  SVG text metric table.
- [x] Remove a small Sequence note/message/frame bucket slice once it is covered by existing
  bounds.
  Evidence: the simple `Bob thinks` note-right trio `upstream_pkgtests_sequencediagram_spec_007`,
  `upstream_pkgtests_sequencediagram_spec_009`, and `upstream_pkgtests_sequencediagram_spec_042`
  pass focused disabled-root `parity-root`, so their root pins were deleted without growing the
  SVG text metric table.
- [x] Remove the follow-up Sequence whitespace/comment note-right slice under the same bounds.
  Evidence: the `Bob thinks` whitespace/comment trio
  `upstream_pkgtests_sequencediagram_spec_043`, `upstream_pkgtests_sequencediagram_spec_045`, and
  `upstream_pkgtests_sequencediagram_spec_046` pass focused disabled-root `parity-root`, so their
  root pins were deleted without growing the SVG text metric table.
- [x] Remove a simple Sequence block note-right slice under the same bounds.
  Evidence: the loop/rect/nested-rect `Bob thinks` trio
  `upstream_pkgtests_sequencediagram_spec_054`, `upstream_pkgtests_sequencediagram_spec_055`, and
  `upstream_pkgtests_sequencediagram_spec_056` pass focused disabled-root `parity-root`, so their
  root pins were deleted without growing the SVG text metric table.
- [x] Remove a simple Sequence alt-control note-right slice under the same bounds.
  Evidence: the alt-control `Bob thinks` trio `upstream_pkgtests_sequencediagram_spec_058`,
  `upstream_pkgtests_sequencediagram_spec_059`, and `upstream_alt_multiple_elses_spec` pass focused
  disabled-root `parity-root`, so their root pins were deleted without growing the SVG text metric
  table.
- [x] Remove the Sequence long-note / long-message six-pack with one shared SVG metric fact.
  Evidence: the two leftOf long-note fixtures and four long-message fixtures pass focused
  disabled-root `parity-root` after fixing leftOf note start recomputation and adding the shared
  long-message browser SVG metric fact. The stale `FRIENDS` row was deleted so the SVG text metric
  table stayed at `186`; full Sequence `parity-root` and `report-overrides --check-no-growth`
  passed.
- [x] Remove the follow-up Sequence wrapped-leftOf / long-note nine-pack.
  Evidence: wrapped `leftOf` notes now derive Mermaid's initial note-width probe and final rewrap
  behavior through Sequence-owned bbox calibration. The two wrapped-leftOf fixtures plus seven
  adjacent long-note/message/root candidates pass focused disabled-root `parity-root`; full
  Sequence `parity-root`, render clippy, render nextest, and `report-overrides --check-no-growth`
  passed. Root viewport overrides are now `702` total, with `164` Sequence entries; text lookup
  remains `484` and the SVG text metric table remains `186`.
- [x] Remove the stacked-activation Sequence pair covered by a shared message-width fact.
  Evidence: the browser `calculateTextDimensions` width fact for
  `Hello Alice, please meet Carol?` now matches upstream actor spacing. `activation_stacked` and
  `upstream_pkgtests_sequencediagram_spec_040` pass focused disabled-root `parity-root`; full
  Sequence `parity-root` passes, and `report-overrides --check-no-growth` reports `379` root
  entries with `76` Sequence entries.
- [x] Remove the `arrows_variants` Sequence root pin covered by a shared message-width fact.
  Evidence: the browser `calculateTextDimensions` width fact for `bidirectional_dotted` now
  matches the upstream 130px width, so the actor columns keep Mermaid's default 50px margin.
  `arrows_variants` passes focused disabled-root `parity-root`, and
  `report-overrides --check-no-growth` reports `378` root entries with `75` Sequence entries.
- [x] Remove the simple Cypress Sequence root pin covered by a shared message-width fact.
  Evidence: the browser `calculateTextDimensions` width fact for `How about you John?` now
  matches the upstream 140px actor-spacing width, so John stays at x=440 and the fixture derives
  the 790px root viewport. The focused disabled-root `parity-root` check passes, and the root
  no-growth budget is tightened to `377` with `74` Sequence root entries.
- [x] Remove four package Sequence root pins covered by shared message/actor width facts.
  Evidence: the browser `calculateTextDimensions` width facts for
  `Hello Bob, how are - you?` and `Alice-in-Wonderland` now match upstream actor spacing.
  `upstream_pkgtests_sequencediagram_spec_014`, `015`, `026`, and `027` pass focused
  disabled-root `parity-root`, and `report-overrides --check-no-growth` reports `373` root
  entries with `70` Sequence entries.
- [x] Remove six docs/control Sequence root pins covered by shared text-width facts.
  Evidence: the browser `calculateTextDimensions` width facts for `Feeling fresh like a daisy`,
  `Fine, thank you. And you?`, `Hello Charley, how are you?`, and
  `Did you want to go to the game tonight?` now match upstream SVG actor and frame spacing.
  `upstream_cypress_sequencediagram_spec_should_render_a_sequence_diagram_with_basic_actor_creation_and_d_009`,
  `upstream_docs_examples_sequencediagram_loops_alt_and_opt_011`,
  `upstream_docs_sequence_alt_and_opt_example`, `upstream_docs_sequence_box_groups_example`,
  `upstream_docs_sequence_create_destroy_example`, and
  `upstream_docs_sequence_rect_nested_example` pass focused disabled-root `parity-root`, and
  `report-overrides --check-no-growth` reports `367` root entries with `64` Sequence entries.
- [x] Derive the Sequence participant-creation v2 lifecycle height root without adding lookup data.
  Evidence: Mermaid advances create/destroy lifecycle cursors by half of the actor's pre-render
  layout height, not by the later type-specific SVG visual height. Sequence lifecycle adjustment
  now uses `actor_base_heights`, so
  `upstream_cypress_sequencediagram_v2_spec_should_render_participant_creation_and_destruction_with_differen_012`
  moves the second half of the diagram back up by the prior `11px` drift and matches upstream
  `1040x580` with root overrides disabled. The generated root pin was deleted, the target layout
  golden was refreshed, and `report-overrides --check-no-growth` reports `307` root entries with
  `58` Sequence entries.
- [x] Sweep Sequence for stale retained root pins after the docs/control width-fact cleanup.
  Evidence: a disabled-root mismatch cross-check found 5 stale retained pins and no missing pins.
  Focused disabled-root `parity-root` passed for
  `upstream_cypress_sequencediagram_v2_spec_should_render_a_sequence_diagram_when_usemaxwidth_is_false_030`,
  `actor_ids_dashes_and_equals`, `upstream_cypress_sequencediagram_spec_example_001`,
  `upstream_cypress_sequencediagram_spec_should_render_a_sequence_diagram_when_usemaxwidth_is_false_059`,
  and `upstream_docs_examples_basic_sequence_diagram_005`. `report-overrides --check-no-growth`
  reports `362` root entries with `59` Sequence entries.
- [x] Revisit the broader Sequence note/message/frame bucket after message width can be inferred
  without fixture-specific text rows.
  Evidence: superseded by the 2026-05-18 Sequence retained-root reclassify. The fresh disabled-root
  sweep in `target/compare/sequence_disabled_root_current.md` still maps all `59` generated keys
  to `parity-root` DOM mismatches with `0` stale entries, so no safe shared message/note/frame rule
  was kept and the bucket remains retained rather than becoming fixture-specific text rows.
- [x] Reclassify the Sequence text escaping / line-break subfamily as retained.
  Evidence: a focused disabled-root slice over
  `upstream_cypress_sequencediagram_spec_should_handle_different_line_breaks_004`,
  `stress_message_text_with_colons_039`,
  `upstream_cypress_sequencediagram_spec_should_handle_line_breaks_and_wrap_annotations_006`,
  `stress_html_entities_and_escaping_038`,
  `upstream_cypress_sequencediagram_v2_spec_should_render_with_wrapped_messages_and_notes_011`,
  `stress_sequence_batch5_whitespace_semicolons_051`, and
  `upstream_docs_sequence_note_with_br` shows `6` positive width drifts, `0` negative width
  drifts, `0` height changes, and one exact match. The existing Sequence message/note/wrap
  helpers already route these labels through shared measurement and line-splitting code, but the
  remaining drift still splits across message, note, wrapped, and escaping cases, so no new shared
  rule was kept without fixture/text lookup data.
- [x] Reclassify the Sequence nested frame / rect vertical geometry subfamily as retained.
  Evidence: focused disabled-root `parity-root` checks for `stress_deep_nested_frames_018`,
  `stress_nested_frames_001`, and `stress_nested_rect_par_029` still fail only on root height:
  `850x967 -> 850x983` (`+16`), `850x1045 -> 850x1061` (`+16`), and
  `650x712 -> 650x742` (`+30`). Element-level SVG probes show the residuals do not share one
  bottom-padding or frame-boundary rule: `stress_deep_nested_frames_018` has the footer bottom
  lower locally (`946 -> 962`) while loop/message/activation maxima are higher upstream
  (`861/841/861` upstream versus `837/827/827` local); `stress_nested_frames_001` shifts the
  footer and message/frame maxima down locally but not activation height uniformly
  (`activationBottomMax 939 -> 885`); and `stress_nested_rect_par_029` shifts message,
  activation, and footer down by `+30` while `loopLineYMax` and note bottoms stay fixed
  (`552 -> 552`, `542 -> 542`). No fixture, glyph, text, or root lookup data was added.
- [x] Remove the first then-stale GitGraph root pins found by disabled-root mismatch
  cross-checking.
  Evidence: `upstream_cypress_gitgraph_spec_88_should_hide_branches_with_tb_orientation_when_showbranches_is_092`
  and `upstream_direction_bt` were present in the GitGraph root table but absent from the
  disabled-root mismatch set. Both passed focused `parity-root` without a lookup; full GitGraph
  `parity-root`, `report-overrides --check-no-growth`, render/xtask clippy, and xtask override
  budget tests stayed green. Root viewport overrides are now `616` total, with `226` GitGraph
  entries, and the root no-growth budget is tightened to `616`. A later seeded auto-id warm-up
  pass restored `upstream_direction_bt` because the corrected dynamic commit id exposed a real
  BT-direction bbox guard.
- [x] Derive GitGraph title-dominated roots from emitted title text bounds.
  Evidence: GitGraph now adds the 18px title bbox to the post-emission root bbox using the
  pre-title content center, matching Mermaid `insertTitle(...)` semantics. Disabled-root
  cross-checking exposed 13 now-derived pins, which were removed from
  `gitgraph_root_overrides_11_12_2.rs`. Root viewport overrides are now `603` total, with `213`
  GitGraph entries, and the root no-growth budget is tightened to `603`.
- [x] Fix the GitGraph `parallelCommits` unconnected-branch axis rule before pruning the broader
  branch bucket.
  Evidence: parentless commits now restart the commit axis in LR/RL as well as TB/BT, matching
  Mermaid's independent branch timelines. The focused disabled-root probe for
  `upstream_cypress_gitgraph_spec_45_should_render_gitgraph_with_unconnected_branches_and_parallel_048`
  dropped from a `+150.250px` root-width drift to `+0.250px`, while full GitGraph normal DOM and
  `parity-root` stayed green. No root pin was deleted because the remaining drift is branch-label
  browser bbox measurement, not commit-axis layout.
- [x] Align GitGraph font-size precedence before pruning the font-size stress pins.
  Evidence: GitGraph now ignores top-level `fontSize` in layout and base SVG CSS, matching upstream
  `stress_gitgraph_font_size_097`; `themeVariables.fontSize` still wins for
  `stress_gitgraph_font_size_precedence_098`, and top-level `fontFamily` stays honored. Focused
  disabled-root diagnostics reduced `stress_gitgraph_font_size_097` from a large top-level
  `fontSize` drift to `+0.156px`; no root pin was removed because `097`/`098` still have sub-pixel
  branch-label bbox drift.
- [x] Include GitGraph branch line endpoints in emitted root bbox derivation.
  Evidence: GitGraph now adds its own branch line endpoints to the post-emission bbox before title
  anchoring, matching browser `getBBox()` behavior for zero-length branch lines without changing
  the shared emitted-bounds scanner. The empty-graph and related package fixtures
  (`upstream_pkgtests_diagram_orchestration_spec_048`,
  `upstream_pkgtests_gitgraph_spec_076`, and `upstream_pkgtests_gitgraph_test_011` through
  `_013`) dropped from a roughly `+34.750px` disabled-root width gap to the remaining
  `+0.250px`/`+0.266px` branch-label bbox drift. Full GitGraph `parity-root`,
  `report-overrides --check-no-growth`, render/xtask clippy, and render nextest stayed green; no
  root pin was deleted because the retained GitGraph table still matches the disabled-root DOM
  mismatch set.
- [x] Use computed-length branch-label widths for horizontal GitGraph roots and prune the derived
  pins.
  Evidence: LR/RL branch labels now use `<text>.getComputedTextLength()`-style widths instead of
  ASCII-overhang simple bbox widths, matching upstream branch-label rect/root behavior for the
  horizontal bucket. TB/BT keep the wider bbox path because rotated dynamic commit IDs can dominate
  vertical roots. The full disabled-root GitGraph cross-check exposed 57 now-derived pins
  (`override=213 mismatch=156 stale=57 missing=0`); after deleting them, the cross-check is
  `override=156 mismatch=156 stale=0 missing=0`. This tightened root viewport overrides to `545`
  total with `156` GitGraph entries.
- [x] Match GitGraph seeded auto commit ids to the upstream SVG fixture pipeline and prune the
  newly stale root pins.
  Evidence: upstream committed GitGraph SVG fixtures are produced after `mermaid.parse(code)`
  consumes the seeded `Math.random()` stream before the later render pass. Rust now replays that
  warm-up parse before constructing the render model, so the simple seeded fixture produces
  `0-5b722bd` rather than the earlier single-render stream value `0-ab40cda`. Disabled-root
  cross-checking then exposed 27 stale retained GitGraph root pins; `upstream_direction_bt` was
  restored because it still guards real BT-direction branch/commit-label bbox drift, for 26 net
  deletions. Root viewport overrides are now `497` total with `130` GitGraph entries, and the
  disabled-root cross-check is `override=130 mismatch=130 stale=0 missing=0`.
- [x] Derive GitGraph commit/tag label root widths from computed text length.
  Evidence: GitGraph commit id labels and tag labels now use GitGraph-owned
  `<text>.getComputedTextLength()`-style widths with 1/64px quantization instead of the shared
  simple SVG bbox width path. A disabled-root audit over the previous 130-entry GitGraph root table
  found 65 retained DOM mismatches and 65 stale pins, so the stale pins were deleted. Root viewport
  overrides are now `432` total with `65` GitGraph entries.
- [x] Derive vertical GitGraph branch-label roots from centered SVG bbox widths.
  Evidence: TB/BT branch labels now follow Mermaid's `drawText(name).getBBox()` behavior with the
  centered SVG bbox path and ties-to-even 1/64px quantization, while LR/RL keep the
  computed-length branch-label rule. A disabled-root audit over the previous 65-entry GitGraph
  root table found 24 retained DOM mismatches and 41 stale pins, so the stale pins were deleted.
  Root viewport overrides are now `383` total with `24` GitGraph entries.
- [x] Honor GitGraph commit/tag label theme variables in emitted CSS and root measurement.
  Evidence: commit labels now use `commitLabelFontSize`, `commitLabelColor`, and
  `commitLabelBackground`; tag labels now use `tagLabelFontSize`, `tagLabelColor`,
  `tagLabelBackground`, and `tagLabelBorder`; root bounds measure commit and tag labels with
  separate styles. Focused disabled-root checks for the commit/tag font-size docs fixtures pass
  without `upstream_docs_gitgraph_customizing_commit_label_font_size_032`, so that root pin was
  deleted. Root viewport overrides are now `382` total with `23` GitGraph entries.
- [x] Recheck broader GitGraph branch-label, commit-label, cherry-pick, and tag retained roots
  before doing more table pruning.
  Evidence: the current disabled-root GitGraph sweep
  (`target/compare/gitgraph_disabled_root_current.md`) reports 23 generated root pins, 23
  high-precision root-delta keys, and 15 `parity-root` DOM mismatches. The remaining 8 generated
  keys are not stale: they still differ in exact root attrs, but the 0.25px `parity-root` lattice
  normalizes them at 3 decimals. Representative inspection showed mixed-sign 1/64px drift:
  `develop`/`feature` vertical branch-label rects are local `-0.015625px`, `newbranch` and
  `0-a13d8e6` are local `+0.015625px`, the title fixture combines title/root f32 lattice with
  rotated label height, and the tag guards include small tag polygon height residuals. A probe that
  raised GitGraph 10px commit/tag bbox height to the observed `11.05078125px` improved
  `upstream_merges_spec` but introduced many outside-table height mismatches; restricting it to
  TB/BT no longer fixed the retained LR/RL tag case. No clean shared rule was found without adding
  fixture, glyph, or root lookup data, so GitGraph stays at `23` entries.
- [x] Derive the Flowchart imageSquare docs parameters root from layout-time image plus label
  bounds.
  Evidence: `upstream_docs_flowchart_parameters_136` now sizes the Dagre node from the rendered
  image and label extents instead of only the image asset. The focused disabled-root
  `compare-flowchart-svgs --filter upstream_docs_flowchart_parameters_136 --dom-mode parity-root`
  check passed, so the fixture root pin was deleted. Root viewport overrides are now `544` total,
  Flowchart has `124` entries, and the root no-growth budget is tightened to `544`.
- [x] Derive Flowchart anchor shape layout bounds from the tiny roughjs anchor dot.
  Evidence: Flowchart anchor nodes now ignore labels for layout like Mermaid and use the seeded
  roughjs 2px dot bbox for Dagre. Disabled-root checks passed for the old-shape set5 stale-pin
  cluster except the retained `upstream_cypress_oldshapes_spec_shapessets_shapesset5_tb_md_html_false_038`
  0.06px drift guard, so 12 root pins were deleted. Root viewport overrides are now `532` total,
  Flowchart has `112` entries, and the root no-growth budget is tightened to `532`.
- [x] Derive the courier Flowchart long-name/class-definition root from C1 replacement-glyph
  measurement.
  Evidence: C1 control bytes in mojibake Flowchart HTML labels now measure as Chromium-style
  near-full-em replacement glyphs. Focused disabled-root `parity-root` passes for
  `upstream_cypress_flowchart_spec_12_should_render_a_flowchart_with_long_names_and_class_definitio_012`,
  so its root pin was deleted. The handdrawn/default-font sibling still has real residual root
  drift and remains pinned. Root viewport overrides are now `531` total, Flowchart has `111`
  entries, and the root no-growth budget is tightened to `531`.
- [x] Derive the Flowchart SVG-like long-word subgraph-title root from shared emitted wrapping.
  Evidence: Flowchart layout now reuses the emitted SVG text wrapping helper for plain SVG cluster
  titles and sizes default process nodes from wrapped computed text length. Focused disabled-root
  `parity-root` passes for
  `upstream_flowchart_v2_stage2_subgraph_title_wraps_long_word_svglike_spec`, so its root pin was
  deleted. Root viewport overrides are now `530` total, Flowchart has `110` entries, and the root
  no-growth budget is tightened to `530`.
- [x] Derive the Flowchart Unicode/entities subgraph-title root from HTML text extraction and a
  narrow CJK width cushion.
  Evidence: `flowchart_label_plain_text_for_layout` now preserves bare `<` / `>` comparison text
  instead of treating it as markup, and Flowchart HTML label metrics apply a default-stack CJK
  cushion only for single-line labels with literal comparison symbols.
  `stress_flowchart_subgraph_title_unicode_and_entities_043`
  passes focused disabled-root `parity-root`, the root pin was deleted, and the root no-growth
  budget is tightened to `529` with `109` Flowchart entries.
- [x] Delete stale Flowchart subgraph title-margin root pins exposed by the disabled-root audit.
  Evidence: focused disabled-root `parity-root` passes for
  `upstream_cypress_flowchart_v2_spec_should_render_subgraphs_with_title_margins_set_lr_and_htmllabels_062`
  and `upstream_flowchart_v2_subgraph_title_margins_lr_htmlLabels_false_spec` after bypassing the
  lookup, so both root pins were deleted. The root no-growth budget is tightened to `527` with
  `107` Flowchart entries.
- [x] Derive the Flowchart font-size precedence root from HTML label measurement semantics.
  Evidence: Flowchart now uses a separate HTML-label measurement base style. Numeric
  `themeVariables.fontSize` still drives SVG root CSS but does not resize `foreignObject` HTML
  label measurement; valid `"NNpx"` theme strings and class/inline font-size rules still apply.
  `stress_flowchart_font_size_precedence_073` passes focused disabled-root `parity-root`, the root
  pin was deleted, and the root no-growth budget is tightened to `526` with `106` Flowchart
  entries.
- [x] Derive the Flowchart docs icon-shape root from iconSquare outer layout bounds.
  Evidence: Mermaid `iconSquare.ts` sizes the icon box as `iconSize + halfPadding * 2`; Flowchart
  layout now mirrors that as `iconSize + node.padding` for `iconSquare` before Dagre/root bounds.
  `upstream_docs_flowchart_icon_shape_132` passes focused disabled-root `parity-root`, the root pin
  was deleted, the affected layout golden was refreshed, and the root no-growth budget is tightened
  to `525` with `105` Flowchart entries.
- [x] Derive the Flowchart custom FontAwesome fallback roots from Mermaid's `createText.ts`
  behavior.
  Evidence: Mermaid emits an empty `<i class="fab fa-truck-bold">` for the unregistered custom
  icon-pack example. Flowchart HTML-label measurement now applies the matching Chromium inline
  advance, so `upstream_docs_flowchart_custom_icons_238` and
  `stress_flowchart_icons_prefixes_and_quotes_052` pass focused disabled-root `parity-root`. Both
  root pins were deleted, and the root no-growth budget is tightened to `523` with `103` Flowchart
  entries.
- [x] Derive the Flowchart old-shape set3 LR fork roots from direction-sensitive fork/join layout.
  Evidence: Mermaid `forkJoin.ts` uses a vertical `10x70` bar only when the rendered graph `dir`
  is `LR`, then inflates Dagre dimensions by `state.padding / 2`. Flowchart layout now mirrors that
  rule, removing the 60px LR old-shape offset. The focused old-shape set3 LR fixtures pass
  disabled-root `parity-root`; five root pins were deleted in the layout pass. A follow-up
  disabled-root stale-pin cross-check also found the classdef, `md_html_false`, and styles siblings
  absent from the mismatch set, so those three pins were deleted too. The root no-growth budget is
  tightened to `424` with `95` Flowchart entries.
- [x] Collapse exact-duplicate Flowchart root override match arms.
  Evidence: several retained Flowchart fixture stems share byte-for-byte identical `(viewBox,
  max-width)` tuples. The generated table now groups those stems with Rust or-patterns, preserving
  fixture-key coverage while reducing `report-overrides` inventory from `362` to `354` root
  entries and Flowchart from `95` to `87` entries.
- [x] Derive the quoted-numeric Flowchart rankSpacing root.
  Evidence: Flowchart layout and SVG parity config now parse plain numeric strings such as
  `flowchart.rankSpacing: '100'` as finite numbers, so
  `upstream_cypress_flowchart_spec_23_render_a_simple_flowchart_with_rankspacing_set_to_100_023`
  passes focused disabled-root and normal `parity-root` checks without a root viewport pin. The
  affected layout golden was refreshed, and the root no-growth budget is tightened to `353` with
  `86` Flowchart entries.
- [x] Centralize numeric config parsing before the next root pruning pass.
  Evidence: `crates/merman-render/src/config.rs` now owns finite JSON number, quoted YAML number,
  and CSS `px` numeric parsing for render modules. Full `merman-render` nextest and full
  `compare-all-svgs --dom-mode parity-root` passed. A disabled-root cross-check across generated
  root tables found no newly stale pins, so the root budget correctly stays at `353`.
- [x] Derive the Flowchart chained-statement / edge-spacing height root.
  Evidence: Flowchart now separates root `htmlLabels` semantics for node labels from
  `flowchart.htmlLabels` semantics for edge labels, subgraph titles, CSS selectors, and the
  styled/quoted-string node-height parity helpers. The target fixture
  `upstream_cypress_flowchart_spec_20_multiple_nodes_and_chaining_in_one_statement_020` passes
  focused disabled-root and normal `parity-root` without a root viewport pin, its layout golden was
  refreshed, and `report-overrides --check-no-growth` reports `352` root entries with `85`
  Flowchart entries. The sibling `upstream_flow_vertice_chaining_amp_to_single_spec` remains
  pinned because disabled-root parity still reports upstream `312.5px` versus local `312.75px`
  max-width drift.
- [x] Derive the Flowchart FontAwesome icon-only multiline label height root.
  Evidence: HTML label measurement now counts inline FontAwesome icon-only lines such as
  `<i class="fa ..."></i><br/>...` as normal `1.5em` DOM line boxes. The target fixture
  `stress_flowchart_icons_multiline_br_054` passes focused disabled-root and normal
  `parity-root` without a root viewport pin, its layout golden was refreshed, and
  `report-overrides --check-no-growth` reports `351` root entries with `84` Flowchart entries.
  The remaining icon retained pins were rechecked and still show real disabled-root max-width
  drift, so they are not stale table debt.
- [x] Tighten the Flowchart FontAwesome label boundary without adding a per-icon glyph table.
  Evidence: standard FontAwesome icons now use a clean nominal inline box, while the unregistered
  `fab:fa-truck-bold` custom-pack example remains an empty inline element. This matches the
  upstream DOM boundary without deriving icon advances from root deltas. The
  `stress_flowchart_icons_unicode_and_wrap_056` root pin was deleted after focused disabled-root
  and normal `parity-root` checks passed; `report-overrides --check-no-growth` now reports `350`
  root entries with `83` Flowchart entries.
- [x] Reclassify all retained root pins under the new parity boundary.
  Evidence: the 2026-05-15 full disabled-root audit crossed the generated root tables with
  `compare-all-svgs --check-dom --dom-mode parity-root --dom-decimals 3 --report-root-all` under
  `MERMAN_DISABLE_ROOT_VIEWPORT_OVERRIDES=1`. The generated tables cover `358` fixture keys after
  expanding or-patterns, and the disabled-root mismatch set also contains `358` keys:
  Architecture `31/31`, C4 `35/35`, ER `22/22`, Flowchart `91/91`, GitGraph `23/23`, Journey
  `2/2`, Mindmap `39/39`, Requirement `10/10`, Sankey `3/3`, Sequence `59/59`, State `34/34`,
  and Timeline `9/9`, with `stale=0` and `missing=0` for every table. No root pin was deleted in
  this audit because all retained keys still guard visible `parity-root` drift.
- [x] Reclassify the retained Flowchart `fhd12` mojibake root pin without adding glyph data.
  Evidence: focused retained-root audit and character-level inspection show that the residual
  default-font-stack drift comes from C1 control-byte/mojibake browser fallback behavior. A single
  shared C1 fallback adjustment improves some labels but regresses other same-fixture labels, so
  `xtask triage-flowchart-root-pins` now reports this case as
  `defer-mojibake-font-fallback` rather than `shared-multiline-text`. The root pin remains, no
  fixture/glyph lookup table was added, and the retained-root triage keeps this case out of the
  ordinary shared multiline text bucket.
- [x] Reclassify the retained Flowchart `code_flow` accumulation root without adding glyph data.
  Evidence: focused retained-root audit shows that the root right boundary is
  `flowchart-intersectRect-155`, whose label width already matches upstream, while the largest
  upstream/local label deltas live elsewhere and have mixed signs: long function signatures are
  too narrow locally, but several default-stack multiline/plain labels are too wide. A tested
  shared `break-spaces` min-content adjustment fixed the long signatures but regressed the root
  max-width from `-0.500px` drift to about `+4.100px`, so the residual is treated as accumulated
  browser/font shaping rather than a clean shared metric rule. `xtask triage-flowchart-root-pins`
  now places this case in `defer-font-env`, keeps the root pin, and still avoids fixture/glyph
  lookup data.
- [x] Derive the next Flowchart disabled-root shape-family geometry as far as clean shared rules
  allow. Evidence: `lined-document` now renders from its label-box path instead of the inflated
  Dagre/updateNodeBounds box, and curly brace/comment root bounds reuse the same label-box geometry
  as the SVG emitter instead of the symmetric `node.width / 2` approximation. Focused disabled-root
  audit for `upstream_cypress_newshapes_spec_newshapessets_newshapesset5_lr_md_html_false_086`
  improves from `+2.913px` to `-0.008px`, with the boundary now consistently on
  `flowchart-n55-16` (`brace-r`). The root pin remains because the residual is a sub-1/64px SVG
  Markdown/font lattice difference, and deleting the pin still requires exact root parity.
- [x] Reclassify the `newshapesset5_lr_md_html_false` `-0.008px` residual without adding lookup
  data. Evidence: full retained-root triage now emits a narrow `defer-subpixel-text-lattice`
  bucket for
  `upstream_cypress_newshapes_spec_newshapessets_newshapesset5_lr_md_html_false_086`: root
  max-width and viewBox drift are both below `1/64px`, the boundary contributor is the same
  `flowchart-n55-16` path on both sides, and there are no paired label delta rows. The
  classifier also requires full viewBox width/height drift to stay below the same threshold, so
  height-only retained pins such as the nested-subgraph outgoing-link fixtures remain
  `root-only-layout` instead of being hidden as text-lattice noise.
- [x] Derive the retained Flowchart shared multiline HTML text root pins without adding lookup
  data. Evidence: repeated same-glyph HTML runs now suppress only tiny 1/64px DOM-lattice
  residuals that came from generated two-character pair samples, so the shared
  `tttsssssssssssssssssssssss` line measures `168.0px` and the three-line HTML label keeps its
  `168.0x72.0` layout metrics. Focused disabled-root `parity-root` checks pass for
  `upstream_html_demos_flowchart_flowchart_004`,
  `upstream_html_demos_flowchart_flowchart_046`, and
  `upstream_html_demos_flowchart_graph_003`, so those three root pins were deleted. The full
  retained-root triage now reports `53` root pins, `297` label delta rows, no removal candidates,
  and no `shared-multiline-text` bucket.
- [x] Derive the retained Flowchart `root-only-layout` outgoing-links-4 root pins without adding
  lookup data. Evidence: empty subgraphs that Mermaid emits as ordinary nodes are now included in
  the root viewBox bounds, so focused disabled-root `parity-root` checks match the
  `154.921875x364` upstream viewBox for
  `upstream_cypress_flowchart_v2_spec_57_handle_nested_subgraphs_with_outgoing_links_4_{015,016}`.
  Both root pins were deleted. The full retained-root triage now reports `51` root pins, `297`
  label delta rows, no removal candidates, and no `root-only-layout` bucket.
- [x] Derive the retained Flowchart crossed-circle alias root pin and tighten stacked-rectangle
  geometry evidence without adding lookup data. Evidence: stacked rectangle aliases now render from
  the final `multiRect.ts` bbox instead of expanding the 5px stack offset twice, labels shift by
  `(-5,+5)`, and the root bbox estimator applies the crossed-circle RoughJS asymmetry to
  `cross-circ`, `summary`, and `crossed-circle`. Focused disabled-root `parity-root` now has no
  retained delta for
  `upstream_cypress_flowchart_shape_alias_spec_shape_alias_aliasset37_037`, so that root pin was
  deleted. The full retained-root triage now reports `50` root pins, `297` label delta rows, no
  removal candidates, `defer-low-noise-text-lattice` (16), and `layout-shape-geometry` (2).
- [x] Delete the already-derived Flowchart `newshapesset3_lr_allpairs_067` root pin. Evidence:
  focused disabled-root `parity-root` matches without the generated override, and
  `report-overrides --check-no-growth` now reports `310` total root overrides with `43`
  Flowchart entries. The full retained-root triage now reports `49` root pins, `297` label delta
  rows, no removal candidates, `defer-low-noise-text-lattice` (16), and
  `layout-shape-geometry` (1).
- [x] Extend Flowchart retained-root label audit to cover SVG `<text>/<tspan>` labels for
  `htmlLabels:false`. Evidence: label reports now include emitted SVG label-container geometry,
  and focused audit for
  `upstream_cypress_oldshapes_spec_shapessets_shapesset5_tb_md_html_false_038` shows four
  three-line SVG Markdown text/container deltas (`-0.023px`, `-0.023px`, `-0.008px`,
  `-0.008px`) explaining the retained `-0.060px` root drift. The full retained-root triage now
  reports `49` root pins, `301` label delta rows, no removal candidates,
  `defer-low-noise-text-lattice` (16), `defer-subpixel-text-lattice` (2), and no
  `layout-shape-geometry` bucket.
- [x] Run a generated-table global root override governance audit by diagram family. Evidence:
  `xtask audit-root-overrides --fail-on-stale` now expands all generated root table fixture keys,
  runs disabled-root `parity-root` compares, and verifies retained keys by exact SVG root attrs.
  The first run found stale ER and State pins
  `upstream_docs_entityrelationshipdiagram_unicode_text_007` and
  `stress_state_unicode_and_rtl_036`; focused disabled-root plus normal `parity-root` checks pass
  for both after deletion. The post-delete audit reports `308` root inventory entries, `314`
  fixture keys, `314` retained root-delta keys, and `0` stale generated pins.
- [x] Triage the outside-table normal `parity-root` failures surfaced by the global audit without
  adding new fixture/glyph lookup tables by default. Evidence so far: the seven Flowchart
  `newshapesset4` height roots are now derived by browser-like `HtmlLike` wrapping for long
  multi-hyphen compounds, and the two GitGraph `continuous_development_graph_{005,006}` max-width
  roots are now derived by applying GitGraph title-expanded root width's 1/128px browser lattice
  bias. The three remaining Mindmap docs/example roots are explicitly accepted as weaker root
  parity for now: `upstream_docs_example_icons_br` and
  `upstream_examples_mindmap_basic_mindmap_001` are the same docs/basic tree and are dominated by
  plain label width drift (`Pen and paper` is `102.53125px` upstream versus `103.265625px` local),
  while `upstream_docs_tidy_tree_example_usage_002` propagates the same small text deltas through
  tidy-tree placement into a `671.5` versus `671.75` normalized height bucket. No pass added
  fixture, glyph, or root viewport lookup data.
- [x] Clean up the old Mindmap profile calibration block and split the remaining debt into
  keep/delete buckets. Evidence: deleted stale profile calibrations for `upstream_node_types`,
  `upstream_root_type_bang`, `upstream_shaped_root_without_id`, and the no-longer-matching
  `upstream_docs_example_icons_br` profile. Focused `parity-root` checks for
  `upstream_node_types`, `upstream_root_type_bang`, and `upstream_pkgtests_mindmap_spec_018`
  still pass. A follow-up typed HTML bbox rule then replaced
  `upstream_pkgtests_mindmap_spec_018`, so the retained list was `mindmap/basic`, the simple docs/package
  `Photograph -> Waterfall` tree, `upstream_decorations_and_descriptions`,
  `upstream_hierarchy_nodes`, `upstream_docs_unclear_indentation`,
  `upstream_root_type_cloud`, and `upstream_whitespace_and_comments`. These retained blocks were
  replacement candidates only when a typed shape/text/tidy-tree rule could explain them.
- [x] Replace the Mindmap `upstream_pkgtests_mindmap_spec_018` single-node calibration with a typed
  HTML label bbox rule. Evidence: Mindmap plain one-line labels ending in `[]` / `()` now apply the
  local browser `getBoundingClientRect()` 1/32px lattice correction after vendored HTML measurement,
  deriving the root, node center, rect width, and `foreignObject` width for
  `upstream_pkgtests_mindmap_spec_018`. Focused `parity-root` and full-DOM checks pass for
  `upstream_pkgtests_mindmap_spec_018`; `upstream_pkgtests_mindmap_spec_019` also passes full-DOM
  with the same typed rule. The remaining profile calibration block count is 7:
  `mindmap/basic`, the simple docs/package `Photograph -> Waterfall` tree,
  `upstream_decorations_and_descriptions`, `upstream_hierarchy_nodes`,
  `upstream_docs_unclear_indentation`, `upstream_root_type_cloud`, and
  `upstream_whitespace_and_comments`. `upstream_root_type_cloud` remains retained because the
  current typed cloud rendered-path bbox layout rule is already in place, while the residual
  single-node tuple now tracks the browser HTML label bbox lattice for `the root`
  (`58.359375px` local vs `58.375px` upstream).
- [x] Remove the stale Mindmap `upstream_whitespace_and_comments` profile calibration.
  Evidence: after the typed Mindmap shape/text passes, the old raw profile tuple
  (`337.2026680068237` x `389.4263190830933`) no longer matches current output. The natural local
  root is now `317.0134437302554` x `345.3722723123543` against upstream
  `317.027587890625` x `345.3640441894531`, and focused `parity-root` / full-DOM checks pass
  without fixture, glyph, or root lookup data. The remaining profile calibration block count is 6:
  `mindmap/basic`, the simple docs/package `Photograph -> Waterfall` tree,
  `upstream_decorations_and_descriptions`, `upstream_hierarchy_nodes`,
  `upstream_docs_unclear_indentation`, and `upstream_root_type_cloud`.
- [x] Reclassify the Mindmap `upstream_docs_unclear_indentation` profile calibration as retained.
  Evidence: the parser already resolves the uneven indentation into Mermaid's hierarchy
  (`Root -> A -> {B, C}` with levels `0/4/8/6`), so the residual is not an indentation
  normalization gap. Focused SVG position debugging still shows rendered node-center drift after
  layout, such as `node_2` at `102.686300` upstream versus `98.606399` local on the y axis, while
  the retained root width/height calibration aligns the root SVG. Focused `parity-root` and
  full-DOM checks pass without adding fixture, glyph, or root lookup data.
- [x] Remove the stale Mindmap `upstream_hierarchy_nodes` profile calibration.
  Evidence: existing typed rect/rounded/default shape sizing and hierarchy layout bounds now derive
  the fixture without the old raw tuple (`161.3125` x `375.79146455711737`) matching current output.
  The natural local root is `121.3125` x `345.8237327179229` against upstream
  `121.3125` x `345.82373046875`, and focused SVG position debugging shows all node centers match.
  Focused `parity-root` and full-DOM checks pass without fixture, glyph, or root lookup data. The
  remaining profile calibration block count is 5: `mindmap/basic`, the simple docs/package
  `Photograph -> Waterfall` tree, `upstream_decorations_and_descriptions`,
  `upstream_docs_unclear_indentation`, and `upstream_root_type_cloud`.
- [x] Remove the stale Mindmap `upstream_decorations_and_descriptions` profile calibration.
  Evidence: the typed rect/rounded shape sizing plus icon and label metric rules already derive the
  fixture. The old raw tuple (`589.185529642115` x `462.11530275173845`) no longer matches the
  current natural root (`467.0902368108591` x `383.4868377684121`), and focused SVG position
  debugging shows only small root-viewBox and node-center drift. Focused `parity-root` and full-DOM
  checks pass without fixture, glyph, or root lookup data. The remaining profile calibration block
  count is 4: `mindmap/basic`, the simple docs/package `Photograph -> Waterfall` tree,
  `upstream_docs_unclear_indentation`, and `upstream_root_type_cloud`.
- [x] Remove the stale Mindmap `mindmap/basic` profile calibration.
  Evidence: the existing default-node shape bounds, plain HTML label metrics, and layout bounds now
  derive the fixture without the old raw tuple (`293.08423285144113` x `69.24704462177965`)
  matching current output. After deleting the block, the natural local root is
  `294.05145288721656` x `54` against upstream `294.05145263671875` x `54`; focused
  `parity-root` and full-DOM checks pass without fixture, glyph, or root lookup data. The remaining
  profile calibration block count is 3: the simple docs/package `Photograph -> Waterfall` tree,
  `upstream_docs_unclear_indentation`, and `upstream_root_type_cloud`.
- [x] Remove the Mindmap simple docs/package `Photograph -> Waterfall` profile calibration.
  Evidence: deleting the root-profile block exposed that the package fixture drift came from the
  plain HTML label metric for `Waterfall` (`66.203125px` upstream versus `67.109375px` local), not
  default-node shape sizing or tidy-tree layout. A Mindmap-owned plain-label metric now derives the
  `upstream_pkgtests_diagram_orchestration_spec_077` root and full DOM without fixture, glyph, or
  root lookup data, and the sibling Cypress `Waterfall` fixture still passes full-DOM comparison.
  Focused `parity-root`, full-DOM, SVG-position debug, and label-metric unit checks pass. The
  remaining profile calibration block count is 2: `upstream_docs_unclear_indentation` and
  `upstream_root_type_cloud`.
- [x] Remove the Mindmap `upstream_root_type_cloud` profile calibration.
  Evidence: typed cloud rendered-path bounds were already deriving the shape geometry; deleting the
  root-profile block exposed only the plain HTML label metric residual for `the root`
  (`58.375px` upstream versus `58.359375px` local). A Mindmap-owned plain-label metric now derives
  `upstream_root_type_cloud` without fixture, glyph, or root lookup data. Focused `parity-root` and
  full-DOM checks pass for the `upstream_root_type_*` family, and sibling full-DOM checks pass for
  `upstream_node_types` and `upstream_pkgtests_mindmap_spec_010`. The remaining profile
  calibration block count is 1: `upstream_docs_unclear_indentation`.
- [x] Remove the final Mindmap `upstream_docs_unclear_indentation` profile calibration.
  Evidence: deleting the block showed the natural root still drifted from upstream
  `242.63980102539062` x `210.3271942138672` to local `241.95128845087675` x
  `209.94832264052502`, while the semantic hierarchy was already correct. The root cause was the
  shared plain HTML label metric for `Root` (`32.1875px` upstream versus `32.171875px` local);
  correcting that label metric feeds the deterministic COSE layout and derives the docs
  `Root -> A -> {B, C}` profile naturally. Focused `parity-root`, full-DOM, and SVG-position debug
  checks pass for `upstream_docs_unclear_indentation`, and sibling full-DOM checks pass for
  `upstream_docs_mindmap_unclear_indentation_024` and `upstream_docs_mindmap_syntax_003`. The
  remaining Mindmap profile calibration block count is 0.
- [x] Recheck the retained Flowchart root pins for a clean shared browser/font model before doing
  more table pruning.
  Evidence: the 2026-05-18 retained-root audit and triage still report `49` root pins, `301`
  label delta rows, and no removal candidates. All remaining buckets are explicit deferrals:
  `defer-low-noise-text-lattice` (16), `defer-subpixel-text-lattice` (2),
  `defer-mojibake-font-fallback` (1), `defer-courier-font` (8), `defer-icon-font` (19), and
  `defer-font-env` (3). No clean shared text/layout rule appeared, so no fixture/glyph/root lookup
  table was added and the current Flowchart pins stay retained.
- [x] Derive the GitGraph `BT` + `parallelCommits` compact commit axis without adding lookup data.
  Evidence: Mermaid lays out the compact parallel commit axis in sequence order and then mirrors it
  for bottom-to-top rendering. Rust now follows that typed rule instead of treating reversed parse
  order as a new linear timeline, reducing
  `upstream_cypress_gitgraph_spec_71_should_render_gitgraph_with_parallel_commits_vertical_branch_075`
  from a `330.991x329` natural root to `330.991x239`. The focused disabled-root
  `parity-root` check now passes without the generated root override. The pin remains because the
  exact root width still differs by `-0.016px` from the vertical branch-label bbox lattice, so it
  is still a real high-precision guard rather than stale table debt.
- [x] Reclassify the remaining GitGraph branch/merge/tag root bounds as retained until a typed
  browser measurement model can explain the mixed-sign exact root drift.
  Evidence: after the `BT` + `parallelCommits` axis derivation, a fresh disabled-root full sweep
  still has 23 high-precision generated root-delta keys but only 15 3-decimal `parity-root` DOM
  mismatches. The non-mismatch generated keys are `spec_71`, `cherry_pick_from_branch_graph_015`,
  `cherry_pick_from_main_graph_{017,018}`, `merge_feature_to_advanced_main_graph_007`,
  `merge_from_main_onto_{developed,undeveloped}_branch_graph_{025,022}`, and
  `simple_branch_and_merge_graph_001`; each remains an exact root guard. A shared commit/tag height
  probe regressed outside-table fixtures, confirming the remaining roots are subpixel browser
  lattice debt rather than a safe table-pruning target.
- [x] Reclassify the then-current Sequence note/message/frame retained roots as retained until a
  narrower typed measurement rule appears.
  Evidence: the fresh disabled-root Sequence sweep
  `cargo run -p xtask -- compare-sequence-svgs --check-dom --dom-mode parity-root --dom-decimals 3 --report-root-all --out target/compare/sequence_disabled_root_current.md`
  under `MERMAN_DISABLE_ROOT_VIEWPORT_OVERRIDES=1` still reports all `59` generated Sequence root
  pins as `parity-root` DOM mismatches, with `0` stale pins. The retained rows split into mixed
  families instead of one safe shared rule: `48` positive width drifts, `4` negative width drifts,
  `7` width-zero height-only drifts, and `11` rows with height drift. Top width cases are message
  and note text measurement/escaping roots such as `upstream_docs_diagrams_mermaid_api_sequence`
  (`2869 -> 3037`, plus `239px` height drift),
  `upstream_cypress_sequencediagram_spec_should_handle_different_line_breaks_004`
  (`1002 -> 1101`), and `stress_message_text_with_colons_039` (`986 -> 1079`). Height-only
  cases such as `stress_deep_nested_frames_018`, `stress_nested_frames_001`, and
  `stress_nested_rect_par_029` are frame/rect vertical geometry debt, while
  `upstream_cypress_sequencediagram_v2_spec_should_render_participant_creation_and_destruction_with_differen_012`
  still reflects participant type/lifecycle height drift (`1040x580 -> 1040x591`). Mixed-sign
  rows such as `upstream_cypress_sequencediagram_spec_should_render_rect_around_and_inside_loops_039`
  (`871 -> 861`, height `695 -> 725`) and
  `stress_sequence_batch5_create_destroy_in_par_046` (`734 -> 725`) make a global message/note or
  frame slack unsafe. A narrower typed-participant recheck over
  `upstream_cypress_sequencediagram_v2_spec_should_render_different_participant_types_with_alternative_flows_016`,
  `upstream_cypress_sequencediagram_v2_spec_should_render_different_participant_types_with_notes_and_loops_015`,
  `upstream_cypress_sequencediagram_v2_spec_should_render_parallel_processes_with_different_participant_type_014`,
  and `stress_quoted_participants_and_types_023` still shows mixed-sign root widths
  (`+12`, `+35`, `+14`, and `-7`) with actor, message, and note offsets moving in different
  directions, so there is still no safe shared actor-width or spacing rule to delete. No fixture,
  glyph, or root lookup table was added.
- [x] Follow up the retained Sequence participant type/lifecycle height row with a typed lifecycle
  cursor rule.
  Evidence:
  `upstream_cypress_sequencediagram_v2_spec_should_render_participant_creation_and_destruction_with_differen_012`
  no longer belongs to the retained set after lifecycle cursor adjustment switched from
  type-specific visual actor height to pre-render actor layout height. Focused disabled-root
  `parity-root` now reports `1040x580 -> 1040x580`, the create/destroy neighbor group and typed
  participant group both pass, and full Sequence `parity-root` remains green. Root viewport
  overrides are now `307` total with `58` Sequence entries.
- [x] Reclassify the current State retained roots as retained until narrower note, scaled-root,
  edge-label wrapping, style/font, or browser-lattice rules appear.
  Evidence: the fresh disabled-root State sweep
  `cargo run -p xtask -- compare-state-svgs --check-dom --dom-mode parity-root --dom-decimals 3 --report-root-all --out target/compare/state_disabled_root_current.md`
  under `MERMAN_DISABLE_ROOT_VIEWPORT_OVERRIDES=1` still maps all `33` generated State root keys
  to exact root-delta rows. Crossing the report with
  `state_root_overrides_11_12_2.rs` found `20` positive width drifts, `13` negative width drifts,
  `5` rows with height drift, `32` snapped `parity-root` DOM mismatches, and one exact-only guard
  (`stress_state_unicode_quotes_and_br_in_notes_048`). The retained rows span distinct mechanisms:
  HTML-sanitized note `foreignObject` / noteGroup bounds
  (`stress_state_html_sanitization_notes_025`, `365.93 -> 799.11`, height `402 -> 530`),
  right-to-left scaled long-id bounds (`stress_state_direction_rl_scale_and_long_ids_054`,
  `1006.57 -> 826.01`), edge-label wrapping/Dagre placement
  (`upstream_cypress_statediagram_v2_spec_should_render_edge_labels_correctly_with_multiple_transitions_040`,
  `1283.54 -> 1143.46`), font-size precedence
  (`stress_state_font_size_precedence_071`, `182.30 -> 152.00`, height `386 -> 422`), and
  small browser float guards. A broad root slack or text-width correction would hide mixed
  mechanisms rather than prove a typed derivation rule, so no fixture, glyph, or root lookup table
  was added.
- [x] Derive the two remaining Journey long-label root pins with actor legend browser text
  measurement.
  Evidence: Journey actor legend root bounds now measure each emitted legend line through the
  single-run SVG computed-length path and floor to the 1/32px browser lattice. Focused
  disabled-root `parity-root` for
  `upstream_cypress_journey_spec_should_wrap_text_on_whitespace_without_adding_hyphens_009` and
  `upstream_cypress_journey_spec_should_wrap_long_labels_into_multiple_lines_keep_them_under_max_010`
  passes without generated root overrides, so `journey_root_overrides_11_12_2.rs` is deleted and
  the root viewport budget tightens to `305`.
- [x] Refresh the global generated root override audit after deleting the Journey root table.
  Evidence: `cargo run -p xtask -- audit-root-overrides --fail-on-stale` writes
  `target/compare/root_override_global_audit_current.md` and passes with `305` inventory entries,
  `311` fixture keys, `311` retained root-delta keys, `298` disabled-root DOM mismatches, `0`
  stale generated pins, and the same three accepted Mindmap outside-table DOM residuals.
- [x] Derive the repeated Requirement styled-node root trio from final CSS font-weight label
  measurement.
  Evidence: Requirement layout/render measurement now treats node labels as bold when the compiled
  node CSS contains `font-weight:bold` / `bolder` / numeric weights `>= 600`. Focused disabled-root
  `parity-root` passes for
  `upstream_cypress_requirementdiagram_unified_spec_example_{012,013,014}`, those three generated
  root arms were deleted, and `report-overrides --check-no-growth` reports `302` root entries with
  `7` Requirement root entries.
- [x] Refresh the global generated root override audit after deleting the Requirement styled trio.
  Evidence: `cargo run -p xtask -- audit-root-overrides --fail-on-stale` writes
  `target/compare/root_override_global_audit_current.md` and passes with `302` inventory entries,
  `308` fixture keys, `308` retained root-delta keys, `295` disabled-root DOM mismatches, `0`
  stale generated pins, and the same three accepted Mindmap outside-table DOM residuals.

## P3: Release Closeout

- [x] Run `cargo run -p xtask -- verify --strict`.
  Evidence: strict passed after the Sequence alt-control note-right pass, including fmt,
  workspace clippy, workspace nextest, override no-growth, feature matrix, normal DOM parity, and
  root DOM parity.
- [x] Run `cargo clippy -p merman-render --all-targets --all-features -- -D warnings`.
  Evidence: focused render clippy passed before the final strict gate.
- [x] Run `cargo nextest run` if shared rendering/layout behavior changed.
  Evidence: the final strict gate reran workspace nextest with `1023` passed and `3` skipped after
  the Sequence alt-control note-right root pin cleanup.
- [x] Update `CHANGELOG.md` and the workstream changelog.
- [x] Complete `AUDIT.md` with prompt-to-artifact evidence.
