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

Exit criteria:

- State root override count shrinks or the attempted candidate is documented as retained.
- `compare-state-svgs --check-dom --dom-mode parity-root --dom-decimals 3` passes.
- `report-overrides --check-no-growth` passes.
- `cargo clippy -p merman-render --all-targets --all-features -- -D warnings` passes if render code
  changed.

## M2: Mindmap First Pass

Status: in progress.

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
- Removed two stale GitGraph root pins after disabled-root mismatch cross-checking showed
  `upstream_cypress_gitgraph_spec_88_should_hide_branches_with_tb_orientation_when_showbranches_is_092`
  and `upstream_direction_bt` no longer needed lookup coverage. Focused and full GitGraph
  `parity-root`, render/xtask clippy, xtask override budget tests, and
  `report-overrides --check-no-growth` passed; root viewport overrides are now `616` total with
  `226` GitGraph entries.
- Derived GitGraph title-dominated roots by adding the 18px `gitTitleText` bbox to emitted root
  bbox calculation while keeping title placement anchored to the pre-title content center. Removed
  13 now-derived GitGraph title/root pins and tightened the root budget to `603`, leaving `213`
  GitGraph root entries.
- Switched LR/RL GitGraph branch-label layout to computed-length widths after upstream branch
  label rects proved to match text advance better than ASCII-overhang simple bbox width. The
  follow-up disabled-root cross-check exposed and removed 57 now-derived GitGraph root pins,
  tightening the root budget to `545` and leaving `156` GitGraph root entries.
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

Exit criteria:

- `AUDIT.md` maps each remaining root bucket to a derivation plan or retention reason.
- Strict release gate passes.
- `cargo nextest run` is green if shared layout or renderer contracts changed.
