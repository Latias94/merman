# Root Viewport Derivation Milestones

## Goal Statement

The next root viewport cleanup stage should reduce fixture-scoped root pins by replacing them with
local derivation rules that are easier to maintain and easier to reason about.

Success means:

- State and Mindmap no longer rely on any root pin that can be derived from typed layout or emitted
  SVG bounds.
- Remaining pins are documented as browser-measurement or model gaps.
- Strict parity gates stay green after each deletion.

## M0: Baseline and Tooling

Status: in progress.

Scope:

- Create the workstream docs.
- Reuse existing root audit tooling from `xtask`.
- Capture current State/Mindmap root override counts and drift families.

Exit criteria:

- `README.md`, `TODO.md`, `MILESTONES.md`, `AUDIT.md`, and `CHANGELOG.md` exist.
- State and Mindmap baseline counts are recorded.
- Focused audit commands are documented.
- `clippy`, `nextest`, `parity-root`, and strict gate expectations are explicit.

## M1: State First Pass

Status: in progress.

Scope:

- Classify State root viewport drift families.
- Replace at least one practical fixture group with typed or emitted-bounds derivation.
- Remove only entries that stay green under both State DOM parity modes.

Progress:

- Classified the then-current 42 retained State root pins with a disabled-root `parity-root` sweep.
  The largest drift families are HTML-sanitized notes, right-to-left scale bounds with long IDs,
  wrapping edge-label bounds, markdown labels, unicode/RTL text metrics, style/font precedence, and
  small browser float/lattice guards.
- Removed `upstream_cypress_statediagram_v2_spec_can_have_styles_applied_034` after narrowing the
  72px border-label height inflation rule to classDef-compiled styles. Direct `style` directives no
  longer receive classDef-only height derivation.
- Removed the two `test({ foo: 'far' })` State root pins after decoding Mermaid
  `encodeEntities` placeholders before layout measurement and moving the remaining browser width
  fact into a shared State edge-label text metric.
- Removed the two shared multiline note State root pins after moving the browser-measured note
  label width into State-owned note metrics and applying it consistently in layout and render.
- Removed the two simple State transition-label root pins after extending the existing
  `Transition 1/2/3` edge-label metric to the matching `Transition 4/5` labels without growing the
  text lookup budget.
- Removed the docs `A transition` State root pin by moving its browser-measured edge-label width
  into State edge-label metrics.
- Removed the shared `Your state with spaces in it` State root pins by moving its browser-measured
  node-label width into State node-label metrics.
- Removed the package style `id1/id2` State root pin by extending the existing bold-italic
  `id3/id4` node-label metric family without growing text lookup debt.
- Retained the `state_with_a_note_together_with_another_state` v1/v2 pair for now because the
  disabled-root drift is in note-cluster rect bounds, not a direct text width mismatch.
- Retained the next compound-title, style-precedence, and choice candidates for now because their
  disabled-root drift does not collapse to a single reusable typed metric.
- Rechecked all current `33` retained State root pins after the broader root-viewport passes. The
  disabled-root sweep still maps every generated key to an exact root-delta row, with `32` snapped
  `parity-root` DOM mismatches and one exact-only guard. The remaining roots mix noteGroup bounds,
  RTL/scale layout, edge-label wrapping, style/font precedence, compound/choice geometry, and
  browser-lattice drift, so no broad shared State rule was safe to keep in this pass.

Exit criteria:

- State root override count shrinks or the attempted candidate is documented as retained.
- `compare-state-svgs --check-dom --dom-mode parity-root --dom-decimals 3` passes.
- `report-overrides --check-no-growth` passes.
- `cargo clippy -p merman-render --all-targets --all-features -- -D warnings` passes if render code
  changed.

## M2: Mindmap First Pass

Status: complete for the first pass; remaining Mindmap root pins are tracked as generated-table
guards or accepted outside-table residuals, not hand-written profile calibration debt.

Scope:

- Classify Mindmap root viewport drift families.
- Replace at least one practical fixture group with typed or emitted-bounds derivation.
- Remove only entries that stay green under Mindmap parity gates.

Progress:

- Removed the three Cypress single-root shape pins (`square_shape_011`, `rounded_rect_shape_012`,
  and `circle_shape_013`) after Mindmap layout measurement started trimming delimiter-created
  labels with exactly one non-empty text line. SVG text emission still preserves the raw upstream
  whitespace, so this is a layout/bounds derivation rather than a DOM rewrite.
- Removed `upstream_docs_mindmap_circle_011` after Mindmap plain label measurement stopped using
  global fixture-derived HTML width overrides that belong to other diagram families.
- Removed `upstream_docs_mindmap_cloud_015` after Mindmap root viewport derivation started
  including typed cloud SVG path bounds in addition to the layout rectangle and label bounds.
- Retained the docs bang/hexagon shape entries for now because the remaining disabled-root drift
  is a small browser text/roughjs float delta rather than a reusable shape-bounds rule.
- Removed the shared plain wrapping-label root pins after Mindmap plain HTML-like label measurement
  stopped re-expanding normal prose to unwrapped paragraph width. The wrapped/min-content metric now
  covers both root wrapping prose and the icon-bearing wrapping Cypress fixture without adding text
  lookup debt.
- Removed five additional stale Mindmap root pins after a post-wrapping disabled-root sweep proved
  those fixtures were already covered by the new layout/bounds rules.
- Removed the old hand-written Mindmap profile calibration branches by moving the remaining
  browser `foreignObject` bbox facts for `Waterfall`, `the root`, and `Root` into Mindmap-owned
  plain HTML label metrics. The final docs `Root -> A -> {B, C}` calibration now derives through
  the same deterministic COSE layout path, and `svg/parity/mindmap.rs` no longer contains
  `parity-root calibration` profile branches.

Exit criteria:

- Mindmap root override count shrinks or the attempted candidate is documented as retained.
- `compare-mindmap-svgs --check-dom --dom-mode parity-root --dom-decimals 3` passes.
- `report-overrides --check-no-growth` passes.
- `cargo clippy -p merman-render --all-targets --all-features -- -D warnings` passes if render code
  changed.

## M3: Broader Root-Debt Plan

Status: started.

Scope:

- Decide whether the State/Mindmap derivation patterns apply to Architecture, Flowchart, Sequence,
  or GitGraph.
- Record the next bucket order using evidence from the first passes.

Progress:

- Started Sequence with the lowest-risk font-size precedence candidate. The height path now rounds
  the Mermaid-like text-dimension height and root CSS inherits the configured Sequence font size,
  allowing `stress_sequence_font_size_precedence_090` to derive its root viewport without a pin.
- Derived `upstream_docs_sequencediagram_boundary_008` by routing Sequence text-dimension width
  measurement through the single-run SVG metric path and adding the two boundary message-width
  facts exposed by the upstream fixture. The root pin is deleted; the remaining Sequence bucket
  should still target reusable message/note/frame bounds before broad table pruning.
- Derived the small Sequence title/accessibility root cluster by correcting the default
  trailing-semicolon font-family message width facts for `Hello Bob, how are you?` and
  `Hello John, how are you?`. The three title/accessibility root pins are deleted while the SVG
  metric table row count stays flat.
- Removed the residual default-title pair `upstream_title_without_colon_spec` and
  `upstream_pkgtests_sequencediagram_spec_020`; both derive from the same corrected
  `Hello Bob, how are you?` message-width fact and no additional SVG metric rows were added.
- Removed the simple `Bob thinks` note-right trio
  `upstream_pkgtests_sequencediagram_spec_007`, `upstream_pkgtests_sequencediagram_spec_009`, and
  `upstream_pkgtests_sequencediagram_spec_042`; focused disabled-root `parity-root` proves the
  existing Sequence note/message bounds now cover these variants without additional SVG metric
  rows.
- Removed the whitespace/comment `Bob thinks` note-right trio
  `upstream_pkgtests_sequencediagram_spec_043`, `upstream_pkgtests_sequencediagram_spec_045`, and
  `upstream_pkgtests_sequencediagram_spec_046`; the same focused disabled-root gate proves these
  formatting variants are now covered by the existing Sequence note/message bounds.
- Removed the loop/rect/nested-rect `Bob thinks` block note-right trio
  `upstream_pkgtests_sequencediagram_spec_054`, `upstream_pkgtests_sequencediagram_spec_055`, and
  `upstream_pkgtests_sequencediagram_spec_056`; existing Sequence note/message bounds now cover
  these block wrappers without adding SVG metric rows.
- Removed the alt-control `Bob thinks` note-right trio
  `upstream_pkgtests_sequencediagram_spec_058`, `upstream_pkgtests_sequencediagram_spec_059`, and
  `upstream_alt_multiple_elses_spec`; existing Sequence note/message bounds now cover these simple
  `alt`/`else` wrappers without adding SVG metric rows.
- Removed the long-note / long-message Sequence six-pack after fixing leftOf note start
  recomputation and adding one shared long-message SVG metric fact. Focused disabled-root
  `parity-root` checks passed for the long-note and long-message fixtures; the stale `FRIENDS`
  row was dropped so the SVG text metric table stayed at `186` rows and `report-overrides
  --check-no-growth` remained green.
- Removed the follow-up wrapped-leftOf / long-note Sequence nine-pack after deriving the leftOf
  note width probe and final rewrap behavior. Focused disabled-root checks, full Sequence
  `parity-root`, render clippy, render nextest, and `report-overrides --check-no-growth` passed;
  root viewport overrides were `702` total with `164` Sequence entries at that point.
- Removed two then-stale GitGraph root pins after disabled-root mismatch cross-checking showed
  `upstream_cypress_gitgraph_spec_88_should_hide_branches_with_tb_orientation_when_showbranches_is_092`
  and `upstream_direction_bt` absent from the mismatch set at that point. Focused and full
  GitGraph `parity-root`, render/xtask clippy, xtask override budget tests, and
  `report-overrides --check-no-growth` passed; root viewport overrides are now `616` total with
  `226` GitGraph entries. A later seeded auto-id warm-up pass restored `upstream_direction_bt`
  because the corrected dynamic commit id exposed a real BT-direction bbox guard.
- Derived GitGraph title-dominated roots by adding the 18px `gitTitleText` bbox to emitted root
  bbox calculation while keeping title placement anchored to the pre-title content center. Removed
  13 now-derived GitGraph title/root pins and tightened the root budget to `603`, leaving `213`
  GitGraph root entries.
- Switched LR/RL GitGraph branch-label layout to computed-length widths after upstream branch
  label rects proved to match text advance better than ASCII-overhang simple bbox width. The
  follow-up disabled-root cross-check exposed and removed 57 now-derived GitGraph root pins,
  tightening the root budget to `545` and leaving `156` GitGraph root entries.
- Matched GitGraph seeded auto commit ids to upstream's parse-before-render SVG fixture pipeline by
  replaying a seed-consuming parse warm-up before the render-model parse. The corrected dynamic ids
  exposed 27 stale retained root pins; after restoring `upstream_direction_bt` as a real
  BT-direction bbox guard, the pass removed 26 net GitGraph root pins. This tightens the root
  budget to `497` and leaves `130` GitGraph root entries, with a disabled-root cross-check of
  `override=130 mismatch=130 stale=0 missing=0`.
- Derived Flowchart imageSquare layout bounds from rendered image plus label extents instead of
  only the image asset, removed the now-derived `upstream_docs_flowchart_parameters_136` root pin,
  and tightened the root budget to `544` with `124` Flowchart root entries.
- Derived Flowchart anchor layout bounds from Mermaid's label-ignoring roughjs dot, removed 12
  now-derived old-shape set5 root pins, and tightened the root budget to `532` with `112`
  Flowchart root entries.
- Derived the courier Flowchart long-name/class-definition root by measuring C1 control bytes in
  mojibake HTML labels as Chromium replacement glyphs, removed one more Flowchart root pin, and
  tightened the root budget to `531` with `111` Flowchart root entries.
- Derived the Flowchart SVG-like long-word subgraph-title root by sharing emitted SVG text wrapping
  with layout and sizing default process nodes from wrapped computed text length. Removed
  `upstream_flowchart_v2_stage2_subgraph_title_wraps_long_word_svglike_spec` and tightened the root
  budget to `530` with `110` Flowchart root entries.
- Derived the Flowchart Unicode/entities subgraph-title root by preserving bare comparison symbols
  in HTML label text extraction and applying a narrow default-stack CJK width cushion for
  single-line labels with literal comparison symbols, then deleted
  `stress_flowchart_subgraph_title_unicode_and_entities_043`
  and tightened the budget to `529` with `109` Flowchart root entries.
- Removed two stale Flowchart subgraph title-margin root pins after focused disabled-root
  `parity-root` checks showed
  `upstream_cypress_flowchart_v2_spec_should_render_subgraphs_with_title_margins_set_lr_and_htmllabels_062`
  and `upstream_flowchart_v2_subgraph_title_margins_lr_htmlLabels_false_spec` now derive without
  the lookup, tightening the budget to `527` with `107` Flowchart root entries.
- Derived the Flowchart font-size precedence root by splitting SVG root CSS font-size from HTML
  `foreignObject` label measurement. Numeric `themeVariables.fontSize` stays on the root CSS path,
  while HTML label measurement uses 16px unless the theme value is a valid `"NNpx"` string or a
  class/inline font-size applies. Removed `stress_flowchart_font_size_precedence_073` and
  tightened the budget to `526` with `106` Flowchart root entries.
- Derived the Flowchart docs icon-shape root by mirroring Mermaid `iconSquare.ts` layout bounds:
  the icon box is `iconSize + halfPadding * 2`, so Rust layout now uses
  `iconSize + node.padding` for `iconSquare` before Dagre/root bounds. Refreshed the affected
  layout golden, removed `upstream_docs_flowchart_icon_shape_132`, and tightened the budget to
  `525` with `105` Flowchart root entries.
- Derived two Flowchart custom FontAwesome fallback roots by matching Mermaid's unregistered
  custom-pack behavior: `fab:fa-truck-bold` falls back to an empty `<i>` in `createText.ts`, and
  Chromium still contributes the observed 1/64px inline advance during HTML-label layout. Removed
  `upstream_docs_flowchart_custom_icons_238` and `stress_flowchart_icons_prefixes_and_quotes_052`,
  tightening the budget to `523` with `103` Flowchart root entries.
- Derived GitGraph commit/tag label root bounds by measuring those labels with GitGraph-owned
  computed text lengths and 1/64px quantization instead of the shared simple bbox width path. The
  disabled-root GitGraph cross-check over the previous 130-entry table found `retained=65` and
  `stale=65`; deleting the stale pins tightens the root budget to `432` and leaves `65` GitGraph
  root entries.
- Derived vertical GitGraph branch-label root bounds by matching Mermaid's
  `drawText(name).getBBox()` path for TB/BT branch labels with centered SVG bbox widths and
  ties-to-even 1/64px quantization. The disabled-root GitGraph cross-check over the previous
  65-entry table found `retained=24` and `stale=41`; deleting the stale pins tightens the root
  budget to `383` and leaves `24` GitGraph root entries.
- Honored GitGraph commit/tag label theme variables in emitted CSS and root measurement by using
  separate commit/tag label styles for font-size, color, background, and tag border semantics.
  Focused disabled-root checks for the commit/tag font-size docs fixtures pass without
  `upstream_docs_gitgraph_customizing_commit_label_font_size_032`, tightening the root budget to
  `382` and leaving `23` GitGraph root entries.
- Derived the GitGraph `BT` + `parallelCommits` compact axis by placing commits in sequence order
  and mirroring the axis after parent-based placement. The focused disabled-root check for
  `upstream_cypress_gitgraph_spec_71_should_render_gitgraph_with_parallel_commits_vertical_branch_075`
  now matches `parity-root` height naturally, but the root pin remains because exact width still
  has the retained vertical branch-label bbox lattice residual; GitGraph stays at `23` entries.
- Rechecked the broader GitGraph branch-label/commit-label/cherry-pick/tag retained roots. The
  current disabled-root sweep still has `23` generated exact root-delta keys, `15` snapped
  `parity-root` DOM mismatches, and no stale deletion target. A commit/tag 10px bbox-height probe
  was rejected because it fixed one retained tag guard while regressing outside-table root heights,
  so the remaining GitGraph table is documented as mixed-sign subpixel browser lattice debt rather
  than a safe pruning candidate.
- Removed the stacked-activation Sequence pair after correcting the default message-width fact for
  `Hello Alice, please meet Carol?` from upstream actor spacing. Focused disabled-root checks pass
  for `activation_stacked` and `upstream_pkgtests_sequencediagram_spec_040`, full Sequence
  `parity-root` stays green, and the root budget is tightened to `379` with `76` Sequence root
  entries.
- Removed the `arrows_variants` Sequence root pin after correcting the default message-width fact
  for `bidirectional_dotted` from upstream actor spacing. Focused disabled-root `parity-root`
  passes for the fixture, and the root budget is tightened to `378` with `75` Sequence root
  entries.
- Removed the simple Cypress Sequence root pin after correcting the default message-width fact for
  `How about you John?` from upstream actor spacing. Focused normal and disabled-root
  `parity-root` pass for the fixture, and the root budget is tightened to `377` with `74`
  Sequence root entries.
- Removed four package Sequence root pins after correcting the shared `Hello Bob, how are - you?`
  message-width fact and `Alice-in-Wonderland` actor-width fact from upstream actor spacing.
  Focused disabled-root `parity-root` passes for `upstream_pkgtests_sequencediagram_spec_014`,
  `015`, `026`, and `027`, and the root budget is tightened to `373` with `70` Sequence root
  entries.
- Removed six docs/control Sequence root pins after correcting shared width facts for
  `Feeling fresh like a daisy`, `Fine, thank you. And you?`, `Hello Charley, how are you?`, and
  `Did you want to go to the game tonight?` from upstream SVG actor/frame spacing. Focused
  disabled-root `parity-root` passes for the six removed fixtures, and the root budget is
  tightened to `367` with `64` Sequence root entries. The participant-creation v2 sibling remains
  pinned because it still has an 11px root-height drift from participant type/lifecycle vertical
  geometry.
- Removed five stale Sequence simple-root pins after a disabled-root mismatch cross-check showed
  they no longer produce DOM mismatches when root overrides are disabled. Focused disabled-root
  `parity-root` passes for the five removed fixtures, tightening the root budget to `362` with
  `59` Sequence root entries.
- Rechecked the current Sequence retained note/message/frame bucket and kept all remaining `59`
  generated root pins. A fresh disabled-root `compare-sequence-svgs` sweep still maps every
  generated key to a `parity-root` DOM mismatch, with `0` stale entries. The retained rows mix
  message/note width drift, text escaping and line-break handling, nested frame/rect height drift,
  participant type/lifecycle height drift, and mixed-sign width cases, so no broad shared
  message/note/frame slack rule is safe to apply. This supersedes the earlier TODO item that
  waited on broad message-width inference before revisiting the bucket.
- Reclassified the narrower Sequence text escaping / line-break subfamily as retained. A focused
  disabled-root slice over `upstream_cypress_sequencediagram_spec_should_handle_different_line_breaks_004`,
  `stress_message_text_with_colons_039`,
  `upstream_cypress_sequencediagram_spec_should_handle_line_breaks_and_wrap_annotations_006`,
  `stress_html_entities_and_escaping_038`,
  `upstream_cypress_sequencediagram_v2_spec_should_render_with_wrapped_messages_and_notes_011`,
  `stress_sequence_batch5_whitespace_semicolons_051`, and
  `upstream_docs_sequence_note_with_br` still shows `6` positive width drifts, `0` negative width
  drifts, `0` height changes, and one exact match. The shared Sequence message/note/wrap helpers
  already cover these paths, but the residual drift still splits across message, note, wrapped,
  and escaping cases, so no new shared rule was kept.
- Derived the Flowchart chained-statement height root by matching Mermaid's split htmlLabels
  semantics: nodes follow root `htmlLabels`, while edge labels, subgraph titles, CSS selectors,
  and styled/quoted-string node-height parity follow `flowchart.htmlLabels` with root fallback.
  Removed `upstream_cypress_flowchart_spec_20_multiple_nodes_and_chaining_in_one_statement_020`
  after focused disabled-root and normal `parity-root` passed, refreshed its layout golden, and
  tightened the root budget to `352` with `85` Flowchart root entries. The sibling
  `upstream_flow_vertice_chaining_amp_to_single_spec` remains pinned for a real `312.5px` versus
  `312.75px` disabled-root max-width drift.
- Derived the Flowchart FontAwesome icon-only multiline label height root by counting inline
  FontAwesome icon-only lines as measured `1.5em` HTML line boxes. Removed
  `stress_flowchart_icons_multiline_br_054` after focused disabled-root and normal `parity-root`
  passed, refreshed its layout golden, and tightened the root budget to `351` with `84`
  Flowchart root entries. The remaining icon retained pins stay pinned because disabled-root
  parity still reports real max-width drift.
- Collapsed exact-duplicate Flowchart root override match arms into Rust or-patterns. This does
  not delete fixture-key coverage or claim a new derivation rule; it removes generated-table
  redundancy for stems that already shared identical root tuples, tightening the inventory budget
  to `354` with `87` Flowchart entries.
- Derived Flowchart old-shape set3 LR fork roots by matching Mermaid `forkJoin.ts`
  direction-sensitive sizing: LR-rendered graphs use a vertical `10x70` bar before
  `state.padding / 2` inflation, while other directions keep the horizontal `70x10` bar. Refreshed
  the affected layout goldens, deleted eight now-derived Flowchart root pins after the follow-up
  stale-pin sweep, and tightened the root budget to `424` with `95` Flowchart root entries.

Exit criteria:

- `AUDIT.md` maps each remaining root bucket to a derivation plan or retention reason.
- Strict release gate passes.
- `cargo nextest run` is green if shared layout or renderer contracts changed.
