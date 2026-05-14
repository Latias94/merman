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

- [ ] Revisit Flowchart after State/Mindmap patterns are proven.
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
- [ ] Revisit the broader Sequence note/message/frame bucket after message width can be inferred
  without fixture-specific text rows.
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
- [ ] Revisit broader GitGraph vertical commit/tag and cherry-pick root drift after a typed
  measurement rule can explain the remaining disabled-root mismatches. The 23 retained entries are
  current guards, so the next GitGraph pass should start from root-delta families rather than
  another blind table deletion.
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
- [ ] Revisit broader GitGraph branch/merge/tag root bounds after they can be derived without
  fixture pins. The next useful target is vertical branch/commit-label and cherry-pick/tag bbox
  drift, not another blind GitGraph table-pruning pass.

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
