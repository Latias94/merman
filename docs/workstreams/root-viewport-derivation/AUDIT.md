# Root Viewport Derivation Audit

This audit maps the workstream objective to concrete artifacts and gates.

## Objective

Replace fixture-scoped root viewport overrides with typed bounds derivation where practical,
starting with State and Mindmap, while keeping `parity-root` and strict release gates green.

## Prompt-to-Artifact Checklist

| Requirement | Artifact or command | Current state |
| --- | --- | --- |
| Track work in `docs/workstreams/root-viewport-derivation/` | This directory and its documents | Current-stage complete |
| Start with State | `TODO.md`, `MILESTONES.md`, State override audit | Current-stage complete; all remaining State roots are documented retained guards |
| Include Mindmap | `TODO.md`, `MILESTONES.md`, Mindmap override audit | Mindmap first pass complete; hand-written profile calibration branches are gone |
| Replace fixture-scoped overrides where practical | Code changes plus generated table deletion, table compression, or shared measurement derivation | Current-stage complete: eleven State root pins, thirteen Mindmap root pins, fifty-five Sequence root pins, two Journey root pins, three Requirement root pins, one Timeline root pin, two hundred four net GitGraph root pins, and thirty-two Flowchart root pins removed; the Mindmap hand-written profile calibration block is now eliminated through shared label metrics |
| Keep `parity-root` green | Focused `compare-*-svgs --dom-mode parity-root` commands | Full State, Mindmap, Sequence, GitGraph, and Flowchart passes recorded |
| Keep clippy green for render edits | `cargo clippy -p merman-render --all-targets --all-features -- -D warnings` | Passed |
| Keep nextest green for shared behavior edits | `cargo nextest run` | Workspace nextest passed in the closeout gate |
| Keep strict release gate green | `cargo run -p xtask -- verify --strict` | Passed with an explicit root-parity residual policy for five exact fixtures |

## Current Baseline

The fearless-refactor closeout recorded these root viewport counts:

- State: `45` entries.
- Mindmap: `52` entries.

Current counts after the State style/entity-placeholder/note-label/transition-label/alias
node-label/package style node-label passes, the Mindmap single-line shape, docs circle plain-label,
docs cloud path-bounds, plain wrapping-label, and post-wrapping sweep passes, the Sequence
font-size/message-width/title/default-title/note-right/long-note/wrapped-leftOf plus later metric
cleanup, frontmatter-title, `arrows_variants`, simple Cypress sequence, and package sequence
message/actor-width passes, the first GitGraph stale-pin
cross-check, and the GitGraph
title-bounds/parallel-branch/font-size/branch-line endpoint/horizontal branch-label, commit/tag
label computed-length, vertical branch-label centered bbox, commit/tag label theme-variable, and
seeded auto-id warm-up passes, and the Flowchart imageSquare image-plus-label, anchor-dot layout-bounds, C1 replacement-glyph,
SVG-like subgraph-title/root-bounds, Unicode/entities HTML title, stale title-margin cleanup,
HTML-label font-size precedence, iconSquare layout-bounds, custom FontAwesome fallback,
FontAwesome icon-only multiline label height, LR fork/join direction-sensitive sizing,
quoted-numeric rankSpacing, chained-statement split `htmlLabels`, FontAwesome label-boundary,
cluster external-flag preservation, recursive title-padding stale-pin cleanup, plain HTML text
metric refresh, root-pin-only Flowchart audit tooling, the global ER/State stale-pin audit passes,
the Journey, Requirement, and Timeline cleanup passes, and the ER simple frontmatter-title pass:

- State: `33` entries.
- Mindmap: `39` entries.
- Sequence: `58` entries.
- GitGraph: `23` entries.
- Flowchart: `43` inventory entries covering `49` fixture keys.
- Architecture: `31` entries.
- C4: `35` entries.
- ER: `20` entries.
- Journey: `0` entries.
- Requirement: `7` entries.
- Sankey: `3` entries.
- Timeline: `8` entries.
- Root viewport total: `300` entries.
- Text lookup total: `484` entries. This stayed flat because the new long-note/message Sequence
  fact replaced one stale `FRIENDS` row, and the wrapped-leftOf follow-up removed nine more root
  pins without adding lookup rows.
- SVG text metric table total: `186` rows. The long-note/message fact kept the budget flat after
  the stale row cleanup.
- Font metric table total: `3774` rows.
- Hand-curated helper overrides and manual raw SVG/path bridges: `0`.

The Mindmap generated root table still contains `39` retained root override entries, and the latest
global audit reports `0` stale generated Mindmap pins. Separately, the old hand-written Mindmap
profile calibration block has been eliminated: the residual docs/package, root-cloud, and
docs unclear-indentation profiles now derive through Mindmap-owned plain HTML label metrics for
`Waterfall`, `the root`, and `Root`. The remaining visible Mindmap outside-table DOM mismatches
are the three accepted docs/example residuals (`upstream_docs_example_icons_br`,
`upstream_docs_tidy_tree_example_usage_002`, and `upstream_examples_mindmap_basic_mindmap_001`),
which still represent browser font/tidy-tree drift rather than table growth.

Journey no longer has a generated root viewport override table. The two remaining long-label
Cypress pins were replaced by Journey-owned actor legend measurement: each legend line uses the
single-run SVG computed text length path and floors the result to the 1/32px browser lattice before
feeding `max_actor_label_width`. Focused disabled-root `parity-root` checks for
`upstream_cypress_journey_spec_should_wrap_*` now pass without fixture root pins, and
`report-overrides --check-no-growth` reports `305` root viewport entries with text lookup still at
`484`. Verification for this pass includes `cargo fmt --all --check`,
`cargo test -p merman-render journey_actor_legend_width_uses_single_run_computed_length_lattice`,
focused disabled-root Journey `parity-root`, full Journey `parity-root`, full Journey normal DOM
parity, `cargo clippy -p merman-render --all-targets --all-features -- -D warnings`, override
no-growth, and `git diff --check`.

Requirement's repeated styled Cypress roots
`upstream_cypress_requirementdiagram_unified_spec_example_{012,013,014}` now derive without root
pins. The shared rule is that Requirement label measurement uses the node's final CSS
`font-weight` when sizing both layout boxes and emitted label `foreignObject` widths. This replaces
three fixture-scoped root pins without adding text lookup data. The remaining seven Requirement
root pins still fail with root overrides disabled: `stress_requirement_font_size_precedence_001`
drifts from upstream `286x758` to local `583.34375x542`, `_007` still needs the negative title/
prototype root origin (`-24.03125 -48 221.796875 434` versus local `0 0 173.75 386`), `_023` and
`_025` keep the long-name height/width roots (`801.421875x224 -> 811.65625x200` and
`859x224 -> 876.140625x200`), `_026` is a small long-text width guard
(`582.984375 -> 585.21875`), the docs combined example is now a narrow 1px styled-label lattice
residual (`430.28125 -> 431.3125`), and the HTML demo stack still differs in both width and
height (`939.79296875x1466 -> 964.953125x1442`).

Timeline's empty orchestration fixture `upstream_pkgtests_diagram_orchestration_spec_046` now
derives without a root pin. The previous layout path treated the absence of pre-title nodes and
lines as a synthetic `100x100` content box, which pushed the Timeline activity line from upstream
`x2=450` to local `x2=550` and widened the root from `400px` to `500px`. Empty Timeline diagrams
now keep `pre_title_box_width` at `0`, so the activity line is based only on the default
`leftMargin` (`150 -> 450`) and the root viewport naturally resolves to `100 50 400 100`.
Focused disabled-root `parity-root` passes for that fixture, and the generated Timeline root table
is reduced from `9` to `8` entries.

The remaining eight Timeline roots still fail with root overrides disabled. They split across
browser text measurement families rather than one typed layout bug: long unbroken-word roots have
small SVG text bbox overhang drift, `timeline_stress_disable_multicolor_and_width`,
`timeline_stress_inline_hashes_and_semicolons`, and `timeline_stress_font_size_precedence` retain
large title/label bbox width differences while rendered text and node geometry match, the
CJK/emoji stress fixture differs only in root height (`631.6 -> 629.6`), and the Fira Sans medical
lifecycle Cypress fixture accumulates text-height/vertical-line drift (`879.4 -> 893.3`). No
broad width slack, title-width correction, or text-height correction was kept because the retained
rows mix small and large width drift with height-only drift.

The latest State disabled-root sweep still fails as expected with the 33 retained State root pins
acting as current guards. Crossing `target/compare/state_disabled_root_current.md` with
`state_root_overrides_11_12_2.rs` found all `33` generated keys still have exact root deltas:
`20` positive width drifts, `13` negative width drifts, `5` rows with height drift, `32` snapped
`parity-root` DOM mismatches, and one exact-only guard
(`stress_state_unicode_quotes_and_br_in_notes_048`). They cluster around HTML-sanitized notes,
right-to-left scale bounds with long IDs, dense or wrapping edge-label bounds, markdown edge
labels, note/multiline-label geometry, unicode/RTL text metrics, style/font precedence, and small
browser float or lattice guards. The mix is too broad for a safe global slack or one shared text
rule.

The latest Sequence checks removed the small-font precedence root pin, the docs boundary root pin,
three title/accessibility root pins, two residual default-title root pins, twelve simple
`Bob thinks` note-right root pins, and the long-note / long-message six-pack. The boundary fixture
now derives actor spacing from the single-run text-dimension width path plus two Sequence
message-width facts, replacing the previous 16px actor-column drift with typed measurement data.
The title/default-title cluster now derives from default-message width facts that preserve
Mermaid's trailing-semicolon default font family. The follow-up stacked-activation pass corrected
`Hello Alice, please meet Carol?` from upstream actor spacing, deleting `activation_stacked` and
`upstream_pkgtests_sequencediagram_spec_040` after focused disabled-root `parity-root` checks.
The simple note-right clusters now derive from the existing Sequence note/message bounds without
new SVG metric rows, including the
whitespace/comment variants. The block note-right trio extends that coverage to loop, rect, and
nested-rect wrappers while keeping larger frame-expansion debt out of scope. The alt-control trio
extends the same coverage to simple `alt`/`else` control wrappers. The long-note / long-message
follow-up fixed `leftOf` note start recomputation after width clamping, added a shared SVG text
metric fact for the long message, and dropped the stale `FRIENDS` row so the SVG text metric
budget stayed at `186` without changing `report-overrides --check-no-growth`.
A second follow-up derived wrapped `leftOf` note width probing and final rewrap behavior, refreshed
the affected Sequence/ZenUML layout goldens, and removed nine more Sequence root pins while keeping
the text lookup and SVG text metric budgets flat. The latest small Sequence pass corrected the
default `Hello Alice, I'm fine and you?` message-width fact from `activation_explicit` actor
spacing, so that fixture now passes focused `parity-root` with root overrides disabled and no
longer needs a root viewport pin. The follow-up `arrows_variants` pass corrected the
`bidirectional_dotted` width fact from `131px` to upstream `130px`, preserving Mermaid's default
50px actor margin and deriving the 450px root viewport without the fixture pin.
The package sequence follow-up corrected `Hello Bob, how are - you?` to the upstream 170px
message-width fact and `Alice-in-Wonderland` to the upstream 136px actor-label width fact. That
removes the residual 1-2px actor-column drift in
`upstream_pkgtests_sequencediagram_spec_014`, `015`, `026`, and `027`, and all four fixtures pass
focused `parity-root` with root viewport overrides disabled.
The docs/control sequence follow-up corrected `Feeling fresh like a daisy`, `Fine, thank you. And
you?`, `Hello Charley, how are you?`, and `Did you want to go to the game tonight?` from upstream
SVG actor/frame spacing. That removed six more root pins:
`upstream_cypress_sequencediagram_spec_should_render_a_sequence_diagram_with_basic_actor_creation_and_d_009`,
`upstream_docs_examples_sequencediagram_loops_alt_and_opt_011`,
`upstream_docs_sequence_alt_and_opt_example`, `upstream_docs_sequence_box_groups_example`,
`upstream_docs_sequence_create_destroy_example`, and
`upstream_docs_sequence_rect_nested_example`. The participant-creation v2 sibling initially
remained pinned after that width pass: with root overrides disabled its width matched, but the root
height still drifted from upstream `1040x580` to local `1040x591`, pointing to participant
type/lifecycle vertical geometry rather than another text-width fact.
A follow-up disabled-root mismatch cross-check over the then-current Sequence table found
`root=64 mismatch=59 stale=5 missing=0`. The stale simple-root pins were
`upstream_cypress_sequencediagram_v2_spec_should_render_a_sequence_diagram_when_usemaxwidth_is_false_030`,
`actor_ids_dashes_and_equals`, `upstream_cypress_sequencediagram_spec_example_001`,
`upstream_cypress_sequencediagram_spec_should_render_a_sequence_diagram_when_usemaxwidth_is_false_059`,
and `upstream_docs_examples_basic_sequence_diagram_005`; all five pass focused disabled-root
`parity-root` and were deleted.

The follow-up participant lifecycle pass resolved that height row without adding lookup data:
create/destroy cursor adjustment now uses the actor's pre-render layout height, matching Mermaid's
message-processing behavior before type-specific SVG glyph rendering mutates actor visuals. The
focused disabled-root check for
`upstream_cypress_sequencediagram_v2_spec_should_render_participant_creation_and_destruction_with_differen_012`
now reports `1040x580 -> 1040x580`, and the generated root pin is deleted.

The Sequence retained-root recheck before that lifecycle follow-up kept the then-remaining `59`
generated pins. The
disabled-root sweep in `target/compare/sequence_disabled_root_current.md` still maps all `59`
generated keys to `parity-root` DOM mismatches, with no stale generated entries. The row shape is
mixed: `48` retained rows have positive width drift, `4` have negative width drift, `7` have zero
width drift but height drift, and `11` have non-zero height drift. The largest width drifts are
message/note text measurement and escaping cases (`upstream_docs_diagrams_mermaid_api_sequence`
`2869 -> 3037`, `upstream_cypress_sequencediagram_spec_should_handle_different_line_breaks_004`
`1002 -> 1101`, and `stress_message_text_with_colons_039` `986 -> 1079`). The height-only rows
are separate nested frame/rect and participant-lifecycle debts (`stress_deep_nested_frames_018`,
`stress_nested_frames_001`, `stress_nested_rect_par_029`, and the v2 participant creation root).
Because the table also contains negative width drift for loop/create-destroy/typed-participant
fixtures, a broad shared message, note, or frame slack would trade one retained pin for outside
regressions rather than proving a typed derivation rule. The Sequence table therefore remains a
real guard set for now, with the next candidate work split by text escaping/line-break metrics,
nested frame vertical geometry, and typed participant width/spacing residuals. This supersedes
the earlier TODO item that waited on broad message-width inference before revisiting the bucket.
Follow-up ledger verification found no remaining unchecked workstream TODO items and passed
`git diff --check`, `cargo fmt --all --check`, and
`cargo run -p xtask -- report-overrides --check-no-growth`.
The fresh global root override audit also stayed clean on stale pins. Running
`cargo run -p xtask -- audit-root-overrides --fail-on-stale` wrote
`target/compare/root_override_global_audit_current.md` and reported `0` stale generated pins
across the full `301`-entry root viewport inventory after the Timeline empty-root cleanup. The
report covers `307` fixture keys, `307` retained root-delta keys, and `294` disabled-root DOM
mismatches. It
still reports three accepted outside-table Mindmap DOM mismatches
(`upstream_docs_example_icons_br`, `upstream_docs_tidy_tree_example_usage_002`, and
`upstream_examples_mindmap_basic_mindmap_001`), so the global retained baseline is stable rather
than stale.
The follow-up ER title root pass keeps that global audit clean after tightening the root inventory
again. `upstream_cypress_erdiagram_spec_1433_should_render_a_simple_er_diagram_with_a_title_009`
now derives from emitted ER title bounds: the title inherits the root SVG font-size, the width
uses the browser 1/32px SVG bbox lattice, and the bounds include Chromium's extra 4px vertical
title overhang. A focused disabled-root ER `parity-root` check passes for that fixture, full ER
normal DOM parity and full ER `parity-root` pass, and the remaining ER disabled-root sweep reports
exactly `20` DOM mismatches matching the remaining ER root table. The refreshed global audit
reports `300` inventory entries, `306` fixture keys, `306` retained root-delta keys, `293`
disabled-root DOM mismatches, `0` stale generated pins, and the same three accepted Mindmap
outside-table residuals.
The typed participant width/spacing residuals also stay retained after a focused recheck. With
root overrides disabled, the Cypress typed participant fixtures still drift right by different
amounts:
`upstream_cypress_sequencediagram_v2_spec_should_render_different_participant_types_with_notes_and_loops_015`
reports `1471x793 -> 1506x793`,
`upstream_cypress_sequencediagram_v2_spec_should_render_parallel_processes_with_different_participant_type_014`
reports `1450x706 -> 1464x706`, and
`upstream_cypress_sequencediagram_v2_spec_should_render_different_participant_types_with_alternative_flows_016`
reports `1450x770 -> 1462x770`; the sibling wrapping-text typed fixture remains an exact
`1650x660 -> 1650x660` match. The adjacent quoted/typed stress fixture moves the other way:
`stress_quoted_participants_and_types_023` reports `878x484 -> 871x484`. Element probes show the
Cypress notes/loops case has actor columns, message centers, and the rightOf note widening and
shifting together (`note` width `150 -> 160` local), while the quoted stress case has quoted-label
actor columns and the over-note width narrowing (`678 -> 671`). This mixed sign and mixed
actor/message/note participation makes a shared actor visual-width, spacing, or emitted-bounds
adjustment unsafe without adding fixture-specific text or root data.
The narrower text escaping / line-break subfamily stays retained as well: the focused disabled-root
slice over `upstream_cypress_sequencediagram_spec_should_handle_different_line_breaks_004`,
`stress_message_text_with_colons_039`,
`upstream_cypress_sequencediagram_spec_should_handle_line_breaks_and_wrap_annotations_006`,
`stress_html_entities_and_escaping_038`,
`upstream_cypress_sequencediagram_v2_spec_should_render_with_wrapped_messages_and_notes_011`,
`stress_sequence_batch5_whitespace_semicolons_051`, and
`upstream_docs_sequence_note_with_br` showed `6` positive width drifts, `0` negative width
drifts, `0` height changes, and one exact match. These cases already flow through the shared
Sequence message/note/wrap helpers, but the remaining drift still splits across message, note,
wrapped, and escaping cases, so no new shared rule was kept.
Closeout verification for this documentation-only reclassification passed `git diff --check`,
`cargo fmt --all --check`, and `cargo run -p xtask -- report-overrides --check-no-growth`.
No `nextest` or `parity-root` gate was rerun because this pass changed only workstream evidence
documents and did not touch Rust source, fixtures, generated tables, or goldens.

The 2026-05-18 closeout gate restored the broad non-root verification set. First,
`cargo run -p xtask -- verify --strict` exposed stale local maintenance debt: workspace clippy
failed on six `xtask` lints, and workspace nextest then failed because
`mindmap_cloud_layout_uses_rendered_path_bbox_dimensions` still expected the old `the root`
Mindmap label width (`58.359375px`) even though the current Mindmap-owned label metric is
`58.375px`. The clippy-only rewrites were mechanical (`collapsible_if`, `type_complexity`,
`redundant_closure`, `unnecessary_map_or`, and `manual_contains`), and the Mindmap cloud unit
test plus the twelve affected Mindmap layout goldens were updated from the current deterministic
layout output. Focused checks then passed:
`cargo test -p merman-render mindmap_cloud_layout_uses_rendered_path_bbox_dimensions` and
`cargo test -p merman-render --test layout_snapshots_test fixtures_match_layout_golden_snapshots_when_present`.
The broad closeout gate without root parity,
`cargo run -p xtask -- verify --clippy --all-features --check-overrides --feature-matrix`, passed
with fmt, workspace all-feature check, workspace clippy, override no-growth, feature matrix,
workspace nextest (`1081` passed, `3` skipped), and normal SVG DOM parity.

Full strict closeout is claimed with explicit residual governance. A fresh
`cargo run -p xtask -- verify --strict` passed through fmt, all-feature check, clippy,
override no-growth, feature matrix, workspace nextest (`1084` passed, `3` skipped), normal SVG
DOM parity, and root parity. The root parity stage now accepts exactly five recorded residuals
and fails on any changed or additional mismatch:

- Class:
  `upstream_cypress_classdiagram_elk_v3_spec_elk_should_render_classes_with_different_text_labels_037`
  and
  `upstream_cypress_classdiagram_handdrawn_v3_spec_hd_should_render_classes_with_different_text_labels_037`
  still report root `max-width` drift (`2355.75px` upstream versus `2345px` local).
- Mindmap:
  `upstream_docs_example_icons_br` and `upstream_examples_mindmap_basic_mindmap_001` still report
  `max-width` drift (`756.25px` upstream versus `756.75px` local), and
  `upstream_docs_tidy_tree_example_usage_002` still reports a root `viewBox` height drift
  (`671.5` upstream versus `671.75` local).

Therefore this workstream is closed with explicit root-parity residual governance: generated root
table inventory is stable and stale-pin-free, the strict release gate is green, and the accepted
root residuals are documented and locked down in `compare-all-svgs`.

The nested frame / rect vertical geometry subfamily also stays retained after a narrower recheck.
Focused disabled-root `parity-root` checks for `stress_deep_nested_frames_018`,
`stress_nested_frames_001`, and `stress_nested_rect_par_029` fail only on root height:
`850x967 -> 850x983`, `850x1045 -> 850x1061`, and `650x712 -> 650x742`. Element-level probes
show this is not one shared frame-bottom rule. In `stress_deep_nested_frames_018`, the local footer
bottom is lower (`962` versus upstream `946`) while local loop/message/activation maxima are above
upstream (`837/827/827` versus `861/841/861`). In `stress_nested_frames_001`, local footer and
some message/frame coordinates are lower, but activation bottom does not follow the same shift
(`885` local versus `939` upstream). In `stress_nested_rect_par_029`, message, activation, and
footer positions move down by `+30`, while the loop line and note bottom remain fixed
(`552 -> 552` and `542 -> 542`). A single bottom-padding, rect-close, or ordinary `par` frame
rule would hide mixed mechanisms, so the three generated root pins remain guards until a narrower
typed vertical cursor model appears.

The latest GitGraph pass measures vertical TB/BT branch labels with the centered SVG bbox path and
ties-to-even 1/64px quantization, matching Mermaid's `drawText(name).getBBox()` root behavior.
LR/RL branch labels keep the computed-length rule. A disabled-root audit over the previous
65-entry table found 24 retained DOM mismatches and 41 stale pins, so the stale pins were deleted.
Together with the earlier title-bounds, branch-line endpoint, horizontal branch-label, commit/tag
computed-length, and seeded auto-id warm-up passes, this left 24 GitGraph entries. A later
commit/tag label theme-variable pass honored Mermaid's label-specific CSS and measurement styles,
deleted `upstream_docs_gitgraph_customizing_commit_label_font_size_032`, and leaves `23`
GitGraph entries. Those remaining entries still guard real dynamic commit-label, cherry-pick/tag,
height, or horizontal demo root drift.

The latest Flowchart imageSquare pass sizes layout from the rendered image plus label extents
instead of treating the Dagre node as only the image asset. This derives
`upstream_docs_flowchart_parameters_136` without a fixture root pin. The follow-up anchor pass
models Mermaid's label-ignoring 2px anchor dot layout and deletes 12 old-shape set5 pins. The C1
replacement-glyph pass derives the courier long-name/class-definition root. The SVG-like
subgraph-title pass shares emitted SVG text wrapping with layout and sizes default process nodes
from wrapped computed text length, deriving the stage2 long-word title root. The Unicode/entities
title pass preserves bare comparison symbols in HTML text extraction and applies a narrow
default-stack CJK width cushion for single-line labels with literal comparison symbols, deriving
`stress_flowchart_subgraph_title_unicode_and_entities_043`. The remaining
old-shape set5 `tb_md_html_false` pin still guards a real 0.06px root drift, and the broader
disabled-root Flowchart audit still shows retained drift around icon-heavy labels, subgraph titles,
title spacing, and other old-shape/all-pairs fixtures. The remaining Flowchart table is therefore a
derivation target, not a blind deletion target. The latest Flowchart font-size precedence pass
separates SVG root CSS font-size from HTML `foreignObject` label measurement: numeric
`themeVariables.fontSize` affects the root CSS while HTML labels still measure at 16px, but a
valid `"NNpx"` theme string and class/inline font-size rules still apply to HTML label
measurement. That derives `stress_flowchart_font_size_precedence_073` without a root pin.
The latest iconSquare pass aligns Flowchart layout bounds with Mermaid's `iconSquare.ts`, where
the icon box is `iconSize + halfPadding * 2`; the Rust layout now feeds Dagre/root bounds with
`iconSize + node.padding` for `iconSquare`, deriving `upstream_docs_flowchart_icon_shape_132`
without a root pin.
The latest table-only Flowchart cleanup collapses exact-duplicate root override match arms for
fixtures that share the same `(viewBox, max-width)` tuple. This lowers the inventory count from
`95` to `87` Flowchart entries and the root viewport budget from `362` to `354`, while preserving
the same fixture-key coverage. It is therefore counted as generated-table debt reduction rather
than a new typed derivation pass.

The follow-up disabled-root Flowchart audit on 2026-05-14 confirms that the table compression did
not hide stale pins. With `MERMAN_DISABLE_ROOT_VIEWPORT_OVERRIDES=1`, the Flowchart `parity-root`
comparison still reports one DOM root mismatch for every retained fixture key: `95` fixture-key
mismatches, `95` retained keys, `0` stale retained pins, and `0` missing pins. The inventory count
remains `87` because `report-overrides` counts `Some((viewBox, max-width))` match arms, while the
disabled-root audit counts all fixture keys covered by those arms. The retained drift splits into
`79` `max-width` style mismatches and `16` `viewBox` mismatches. The largest families are:

- rank spacing, chained statements, and edge geometry: the biggest retained height gaps are
  `upstream_cypress_flowchart_spec_23_render_a_simple_flowchart_with_rankspacing_set_to_100_023`
  at `-150px` and
  `upstream_cypress_flowchart_spec_20_multiple_nodes_and_chaining_in_one_statement_020` at
  `+48px`; these need layout/routing derivation, not table pruning.
- icon and FontAwesome labels: after the icon-only multiline pass, this bucket no longer includes
  the `stress_flowchart_icons_multiline_br_054` `-72px` height gap. The remaining icon/wrap/
  subgraph roots still show real max-width drift up to roughly `10px`, so they need icon glyph,
  wrapping, or cluster measurement work before more pins can be deleted.
- subgraph titles and title spacing: retained title-padding and title-margin fixtures still show
  real width or height drift, led by `stress_flowchart_subgraph_deep_nesting_title_padding_044`
  at `-58.75px` and
  `stress_flowchart_subgraph_title_margins_extreme_nested_030` at `-24.75px`.
- shape profiles and all-pairs fixtures: new-shape/old-shape/stadium/alias roots still include
  both multi-pixel geometry drift and small browser-float guards, so the set3 LR fork/join cleanup
  did not generalize to the remaining shape families.
- wrapping, Unicode, style, and long-label measurement: wrapping-long-text fixtures retain
  `-24px` height gaps, while style/long-name cases still show large width drift such as
  `stress_flowchart_text_style_overrides_076` at `+21.75px`.
- small browser float and repeated demo aliases: many remaining HTML demo and simple upstream
  fixtures are sub-pixel to `0.25px` root guards. They are low value individually, but the
  disabled-root cross-check shows they are still real `parity-root` mismatches today.

No Flowchart root pin was deleted in this pass because the top candidates all still appear in the
disabled-root mismatch set. The next Flowchart derivation pass should therefore start from one
large drift family above, not from another stale-pin sweep.

The latest Flowchart rankSpacing pass parses plain numeric string config values as numbers for
Flowchart layout and SVG parity config. This matches Mermaid's treatment of YAML/frontmatter
values such as `flowchart.rankSpacing: '100'`, so
`upstream_cypress_flowchart_spec_23_render_a_simple_flowchart_with_rankspacing_set_to_100_023`
now derives the 100px Dagre rank separation instead of falling back to 50px. Focused disabled-root
and normal `parity-root` checks pass for that fixture, the layout golden was refreshed, and the
root pin was deleted. The follow-up full disabled-root Flowchart audit now reports `94`
fixture-key mismatches, `94` retained keys, `0` stale retained pins, and `0` missing pins, with
`79` `max-width` mismatches and `15` `viewBox` mismatches. The chained-statement sibling remains
pinned and is now clearly a separate SVG-label/edge-spacing issue rather than rankSpacing config
parsing.

The numeric config parser centralization pass moves finite JSON number, quoted YAML number, and
CSS `px` numeric parsing into `crates/merman-render/src/config.rs` and removes diagram-local
copies from layout and SVG parity modules. Full `merman-render` nextest and full
`compare-all-svgs --check-dom --dom-mode parity-root --dom-decimals 3` passed after the migration.
A disabled-root `compare-all-svgs --report-root-all` audit was then crossed with every generated
root override table. Result: Architecture `31/31`, C4 `35/35`, ER `22/22`, Flowchart `94/94`,
GitGraph `23/23`, Journey `2/2`, Mindmap `39/39`, Requirement `10/10`, Sankey `3/3`, Sequence
`59/59`, State `34/34`, and Timeline `9/9` retained pins still map to disabled-root DOM
mismatches (`stale=0` for all tables). No root viewport pin was deleted in this pass.

The latest Flowchart chained-statement pass derives
`upstream_cypress_flowchart_spec_20_multiple_nodes_and_chaining_in_one_statement_020` by matching
Mermaid's split htmlLabels behavior. Node labels follow the root `htmlLabels` toggle and therefore
still render/measure as HTML labels when only `flowchart.htmlLabels: false` is set; edge labels,
subgraph titles, generated Flowchart CSS, and the browser-style styled/whitespace node-height
quirks follow `flowchart.htmlLabels` with root fallback. The target fixture changed from the old
local disabled-root `234x348` root to an intermediate over-correction of `234x282`; after the
split-rule fix it matches the upstream `234.015625 x 300` root without the fixture pin. Focused
disabled-root and normal `parity-root` checks pass for the removed pin, the layout golden was
refreshed, full Flowchart `parity-root` passes, and `report-overrides --check-no-growth` now
reports `352` root entries with Flowchart at `85`.

The adjacent retained chaining pin
`upstream_flow_vertice_chaining_amp_to_single_spec` was checked before deletion and remains pinned:
with `MERMAN_DISABLE_ROOT_VIEWPORT_OVERRIDES=1` it still drifts from upstream
`max-width: 312.5px` to local `312.75px`. That is a real retained browser-float/edge-spacing guard,
not stale table debt.

The latest Flowchart icon multiline pass derives `stress_flowchart_icons_multiline_br_054` by
counting FontAwesome icon-only HTML lines as measured DOM line boxes. Upstream renders labels like
`<i class="fa fa-twitter"></i><br />for peace` with a 48px `foreignObject` height even though the
first text line is empty; local measurement previously trimmed that line and derived a 302px root
instead of the upstream 374px root. The typed metrics fix keeps the icon-only line as a `1.5em`
line box, focused disabled-root and normal `parity-root` pass for the removed pin, the layout
golden was refreshed, full Flowchart `parity-root` passes, and `report-overrides --check-no-growth`
now reports `351` root entries with Flowchart at `84`.

The remaining icon retained pins were rechecked before deletion and remain pinned. With
`MERMAN_DISABLE_ROOT_VIEWPORT_OVERRIDES=1`, examples still drift from upstream to local max-widths:
`stress_flowchart_icons_basic_051` `438.75px` versus `439.5px`,
`stress_flowchart_icons_in_edge_labels_053` `130.75px` versus `127.75px`,
`upstream_cypress_flowchart_icon_spec_example_002` `92px` versus `94px`, and
`upstream_cypress_flowchart_spec_7_should_render_a_flowchart_full_of_icons_007` `2241.25px`
versus `2241.75px`. These are real retained max-width guards, not stale table debt.

The latest Flowchart FontAwesome label-boundary pass keeps standard FontAwesome icons as clean
nominal inline boxes and keeps the unregistered `fab:fa-truck-bold` custom-pack example as an
empty inline element, matching the upstream DOM behavior without adding a per-icon advance table.
That derives `stress_flowchart_icons_unicode_and_wrap_056`, deletes its root pin, and leaves the
remaining icon-root drift as visible root guards rather than fixture-derived glyph answers.
`report-overrides --check-no-growth` now reports `350` root entries with Flowchart at `83`.

The 2026-05-15 all-diagram retained-root audit applied the parity-boundary rule to every generated
root table. The disabled-root command was expected to fail:

```pwsh
$env:MERMAN_DISABLE_ROOT_VIEWPORT_OVERRIDES='1'
cargo run -p xtask -- compare-all-svgs --check-dom --dom-mode parity-root --dom-decimals 3 --report-root-all
Remove-Item Env:\MERMAN_DISABLE_ROOT_VIEWPORT_OVERRIDES
```

The output was captured in `target/disabled_root_audit_2026-05-15_current.txt` and crossed with
the generated root override tables after expanding Rust or-pattern arms. Result:

| table | retained fixture keys | disabled-root mismatches | stale | missing |
| --- | ---: | ---: | ---: | ---: |
| Architecture | 31 | 31 | 0 | 0 |
| C4 | 35 | 35 | 0 | 0 |
| ER | 22 | 22 | 0 | 0 |
| Flowchart | 91 | 91 | 0 | 0 |
| GitGraph | 23 | 23 | 0 | 0 |
| Journey | 2 | 2 | 0 | 0 |
| Mindmap | 39 | 39 | 0 | 0 |
| Requirement | 10 | 10 | 0 | 0 |
| Sankey | 3 | 3 | 0 | 0 |
| Sequence | 59 | 59 | 0 | 0 |
| State | 34 | 34 | 0 | 0 |
| Timeline | 9 | 9 | 0 | 0 |
| Total | 358 | 358 | 0 | 0 |

No root pin was deleted in this audit because no retained root key disappeared from the
disabled-root mismatch set. The difference between `350` inventory entries and `358` retained
fixture keys comes from generated or-pattern compression; `report-overrides` counts `Some(...)`
match arms, while the audit expands every fixture stem covered by those arms.

## Focused Commands

```sh
cargo run -p xtask -- compare-state-svgs --check-dom --dom-mode parity-root --dom-decimals 3 --report-root-all
cargo run -p xtask -- compare-mindmap-svgs --check-dom --dom-mode parity-root --dom-decimals 3 --report-root-all
cargo run -p xtask -- compare-sequence-svgs --check-dom --dom-mode parity-root --dom-decimals 3 --report-root-all
cargo run -p xtask -- compare-gitgraph-svgs --check-dom --dom-mode parity-root --dom-decimals 3 --report-root-all
cargo run -p xtask -- compare-flowchart-svgs --check-dom --dom-mode parity-root --dom-decimals 3 --report-root-all
MERMAN_DISABLE_ROOT_VIEWPORT_OVERRIDES=1 cargo run -p xtask -- compare-flowchart-svgs --dom-mode parity-root --dom-decimals 3 --report-root-pins-only --report-root-all --report-label-root-pins-only --report-label-all
cargo run -p xtask -- report-overrides --check-no-growth
cargo clippy -p merman-render --all-targets --all-features -- -D warnings
cargo run -p xtask -- verify --strict
```

PowerShell disabled-root diagnostic sweep:

```pwsh
$env:MERMAN_DISABLE_ROOT_VIEWPORT_OVERRIDES='1'
cargo run -p xtask -- compare-state-svgs --check-dom --dom-mode parity-root --dom-decimals 3 --report-root-all
cargo run -p xtask -- compare-mindmap-svgs --check-dom --dom-mode parity-root --dom-decimals 3 --report-root-all
cargo run -p xtask -- compare-gitgraph-svgs --check-dom --dom-mode parity-root --dom-decimals 3 --report-root-all
cargo run -p xtask -- compare-flowchart-svgs --check-dom --dom-mode parity-root --dom-decimals 3 --report-root-all
Remove-Item Env:\MERMAN_DISABLE_ROOT_VIEWPORT_OVERRIDES
```

## Verification Log

- 2026-05-18: Derived the Sequence participant creation/destruction lifecycle-height root by using
  the actor's pre-render layout height for create/destroy cursor advancement instead of
  type-specific SVG visual height. Focused disabled-root `parity-root` now matches
  `upstream_cypress_sequencediagram_v2_spec_should_render_participant_creation_and_destruction_with_differen_012`
  at `1040x580 -> 1040x580`; create/destroy neighbors and typed participant neighbors also pass.
  Full Sequence `parity-root` passes, `report-overrides --check-no-growth` reports `307` root
  entries with `58` Sequence entries, `cargo fmt --all --check` and render clippy pass, and
  Sequence-focused nextest passes (`13` tests). `cargo nextest run -p merman-render --no-fail-fast`
  is still
  blocked by unrelated Mindmap drift: `mindmap_cloud_layout_uses_rendered_path_bbox_dimensions`
  and the global layout snapshot test over existing Mindmap goldens fail, while the other `196`
  render tests pass.
- 2026-05-16: Flowchart retained-root triage now separates mojibake/C1 fallback drift from
  ordinary shared multiline text candidates. The focused `fhd12` audit still reports real root
  drift (`1926.810px` upstream max-width versus `1905.300px` local), but character-level review
  showed that a shared C1 fallback constant would improve the `SA/DBA` execution labels while
  worsening other same-fixture mojibake labels such as the submit/owner-confirmation lines. The triage tool
  therefore classifies
  `upstream_cypress_flowchart_handdrawn_spec_fhd12_should_render_a_flowchart_with_long_names_and_class_defini_012`
  as `defer-mojibake-font-fallback` instead of `shared-multiline-text`, keeping the root pin
  rather than adding fixture/glyph lookup data. Evidence files:
  `target/compare/flowchart_fhd12_no_overrides.md`,
  `target/compare/flowchart_fhd12_triage.md`, and
  `target/compare/flowchart_root_pin_triage_no_overrides_current.md`.
- 2026-05-16: Flowchart retained-root triage now separates root-only subpixel text-lattice noise
  from shape geometry. The full retained-root command
  `cargo run -p xtask -- compare-flowchart-svgs --dom-mode parity-root --no-root-overrides --report-root-pins-only --report-root-all --report-label-root-pins-only --report-label-all --out target/compare/flowchart_root_pin_label_audit_current.md`
  produced `56` root delta rows and `300` label delta rows; the follow-up
  `cargo run -p xtask -- triage-flowchart-root-pins --in target/compare/flowchart_root_pin_label_audit_current.md --out target/compare/flowchart_root_pin_triage_current.md`
  reports no removal candidates and classifies
  `upstream_cypress_newshapes_spec_newshapessets_newshapesset5_lr_md_html_false_086` as
  `defer-subpixel-text-lattice` (`-0.008px`, same `flowchart-n55-16` boundary, no paired label
  delta rows). The classifier checks full viewBox width/height drift as well as max-width drift,
  so the two nested-subgraph outgoing-link fixtures with `20px` height drift remain
  `root-only-layout`. Current full retained Flowchart buckets are `shared-multiline-text` (3),
  `low-noise-text` (10), `defer-subpixel-text-lattice` (1), `layout-shape-geometry` (9),
  `root-only-layout` (2), `defer-mojibake-font-fallback` (1), `defer-courier-font` (8),
  `defer-icon-font` (19), and `defer-font-env` (3).
- 2026-05-16: Flowchart shared multiline HTML text drift is now derived by the shared vendored
  font measurer instead of a fixture/glyph lookup. Chromium reports repeated same-glyph runs such
  as `sss` and `tttsssssssssssssssssssssss` on a 1/64px DOM lattice; applying the generated
  two-character residual to every overlapping pair made long runs too narrow. The renderer now
  treats only those tiny same-glyph residuals as per-pair-cell lattice noise. Focused disabled-root
  `parity-root` checks pass for `upstream_html_demos_flowchart_flowchart_004`,
  `upstream_html_demos_flowchart_flowchart_046`, and
  `upstream_html_demos_flowchart_graph_003`, so their Flowchart root pins were deleted. The full
  retained-root audit now produces `53` root delta rows and `297` label delta rows; triage reports
  no removal candidates and current buckets `defer-low-noise-text-lattice` (10),
  `defer-subpixel-text-lattice` (1), `layout-shape-geometry` (9), `root-only-layout` (2),
  `defer-mojibake-font-fallback` (1), `defer-courier-font` (8), `defer-icon-font` (19), and
  `defer-font-env` (3).
- 2026-05-16: Flowchart `root-only-layout` outgoing-links-4 drift is now derived without a
  fixture/glyph lookup. The rendered SVG already emitted empty subgraph `B` as an ordinary node,
  but the root viewBox bounds skipped layout nodes absent from `node_dom_index`; including empty
  subgraph-as-node rectangles restores the missing top 20px. Focused disabled-root `parity-root`
  checks now match upstream for
  `upstream_cypress_flowchart_v2_spec_57_handle_nested_subgraphs_with_outgoing_links_4_015` and
  `_016`, so both root pins were deleted. The full retained-root audit now produces `51` root
  delta rows and `297` label delta rows; triage reports no removal candidates and current buckets
  `defer-low-noise-text-lattice` (10), `defer-subpixel-text-lattice` (1),
  `layout-shape-geometry` (9), `defer-mojibake-font-fallback` (1), `defer-courier-font` (8),
  `defer-icon-font` (19), and `defer-font-env` (3).
- 2026-05-16: Flowchart stacked-rectangle aliases and crossed-circle aliases now share the upstream
  shape geometry rules without fixture/glyph lookup data. `multiRect.ts` stores Dagre dimensions
  after the 5px stacked offset has already expanded the final outer bbox, so the renderer now
  subtracts that offset before drawing the inner rectangle and applies Mermaid's `(-5,+5)` label
  shift. This removes the local `+5px` right/bottom path-boundary drift in
  `upstream_cypress_flowchart_shape_alias_spec_shape_alias_aliasset34_034`; the fixture remains
  pinned only because its plain labels still have `±0.016px` low-noise text-lattice drift.
  `cross-circ`, `summary`, and `crossed-circle` now share the RoughJS circle bbox asymmetry in the
  root estimator, so focused disabled-root `parity-root` has no retained delta for
  `upstream_cypress_flowchart_shape_alias_spec_shape_alias_aliasset37_037` and that root pin was
  deleted. The full retained-root audit now produces `50` root delta rows and `297` label delta
  rows; triage reports no removal candidates and current buckets `defer-low-noise-text-lattice`
  (16), `defer-subpixel-text-lattice` (1), `layout-shape-geometry` (2),
  `defer-mojibake-font-fallback` (1), `defer-courier-font` (8), `defer-icon-font` (19), and
  `defer-font-env` (3).
- 2026-05-16: Flowchart `upstream_cypress_newshapes_spec_newshapessets_newshapesset3_lr_allpairs_067`
  now passes focused disabled-root `parity-root` without the generated root override, so the
  stale pin was deleted. The generated root inventory is now `310` total entries with `43`
  Flowchart entries. The full retained-root audit now produces `49` root delta rows and `297`
  label delta rows; triage reports no removal candidates and current buckets
  `defer-low-noise-text-lattice` (16), `defer-subpixel-text-lattice` (1),
  `layout-shape-geometry` (1), `defer-mojibake-font-fallback` (1), `defer-courier-font` (8),
  `defer-icon-font` (19), and `defer-font-env` (3).
- 2026-05-16: Flowchart retained-root label audit now includes `htmlLabels:false` SVG
  `<text>/<tspan>` labels by pairing emitted label-container geometry. This removes the audit
  blind spot for
  `upstream_cypress_oldshapes_spec_shapessets_shapesset5_tb_md_html_false_038`: focused audit
  reports four three-line SVG Markdown text/container deltas (`-0.023px`, `-0.023px`, `-0.008px`,
  `-0.008px`) and the right boundary remains the same `flowchart-n44-13` polygon contributor.
  The residual is accumulated SVG Markdown/font lattice drift rather than a clean shape geometry
  rule, so the root pin stays retained without fixture/glyph lookup data. The full retained-root
  audit now produces `49` root delta rows and `301` label delta rows; triage reports no removal
  candidates and current buckets `defer-low-noise-text-lattice` (16),
  `defer-subpixel-text-lattice` (2), `defer-mojibake-font-fallback` (1),
  `defer-courier-font` (8), `defer-icon-font` (19), and `defer-font-env` (3), with no remaining
  `layout-shape-geometry` bucket.
- 2026-05-16: global root override governance is now a reusable xtask pass. `xtask
  audit-root-overrides --fail-on-stale` expands generated root table fixture keys by diagram
  family, runs child compare commands with `MERMAN_DISABLE_ROOT_VIEWPORT_OVERRIDES=1`, and decides
  staleness from exact upstream/local root `viewBox` and `max-width` attrs rather than DOM report
  rounding. The first run found two stale generated pins:
  `upstream_docs_entityrelationshipdiagram_unicode_text_007` and
  `stress_state_unicode_and_rtl_036`. Focused disabled-root and normal `parity-root` checks passed
  for both after deletion. The post-delete audit reports `308` inventory entries, `314` fixture
  keys, `314` retained root-delta keys, and `0` stale generated pins.
- 2026-05-16: the same global audit surfaces 12 outside-table normal `parity-root` failures. These
  are not stale retained pins and should not be hidden by adding fixture/glyph tables by default:
  seven Flowchart `newshapesset4` roots have viewBox height drift, two GitGraph continuous
  development fixtures have `349.75px` upstream versus `349.5px` local max-width, and three
  Mindmap docs/example fixtures have icon/tidy-tree root drift. Focused normal probes confirm
  representatives from all three families fail today, so the next practical work is typed
  geometry/root-bounds derivation or an explicit policy decision to accept weaker root parity for
  those cases.
- 2026-05-16: Flowchart `newshapesset4` root height parity now comes from `HtmlLike` text
  measurement rather than a root pin. Long multi-hyphen compounds such as
  `half-rounded-rectangle` are treated as browser-breakable tokens, which restores the affected
  two-line `foreignObject` label height and removes the seven Flowchart outside-table candidates
  without adding fixture, glyph, or root viewport lookup data. The outside-table audit target is
  now narrowed to two GitGraph continuous-development roots and three Mindmap icon/tidy-tree
  roots.
- 2026-05-16: GitGraph `continuous_development_graph_{005,006}` root width parity now comes from
  the emitted title/root bbox path. When a centered title alone expands both horizontal root bounds,
  the final width gets the observed Chromium 1/128px lattice bias before `f32` upward rounding. The
  focused continuous-development checks and full GitGraph `parity-root` pass without adding
  fixture, glyph, or root viewport lookup data. The outside-table audit target is now narrowed to
  three Mindmap icon/tidy-tree roots.
- 2026-05-16: the three remaining Mindmap outside-table docs/example roots are classified as
  accepted weak root-parity residuals rather than new table debt. The `upstream_docs_example_icons_br`
  and `upstream_examples_mindmap_basic_mindmap_001` SVGs share the same docs/basic tree; node-level
  inspection shows plain label width drift, with `Pen and paper` measuring `102.53125px` upstream
  versus `103.265625px` locally. `upstream_docs_tidy_tree_example_usage_002` uses the same label
  family and propagates those small text deltas through tidy-tree placement, crossing the normalized
  root-height bucket from `671.5` upstream to `671.75` local. Updating the old profile calibration
  would hide browser font/tidy-tree drift behind another fixture-shaped rule, so the audit keeps
  these three visible until a broader typed text/layout model is worth the complexity.
- 2026-05-16: Mindmap's old profile calibration block was reduced from 12 fixture-shaped branches
  to 8. Removed branches: `upstream_node_types` and `upstream_root_type_bang` are now covered by
  generated root overrides, `upstream_shaped_root_without_id` and the stale
  `upstream_docs_example_icons_br` branch no longer match the current raw viewport after earlier
  text/layout changes. Focused `parity-root` checks pass for the deleted retained cases.
  A follow-up typed HTML bbox rule then removed `upstream_pkgtests_mindmap_spec_018`; the
  then-remaining branches were classified as must-retain until broader typed rules exist:
  `mindmap/basic`, the simple docs/package chain, `upstream_decorations_and_descriptions`,
  `upstream_hierarchy_nodes`, `upstream_docs_unclear_indentation`,
  `upstream_root_type_cloud`, and `upstream_whitespace_and_comments`.
- 2026-05-16: `upstream_pkgtests_mindmap_spec_018` no longer uses a Mindmap profile calibration.
  Mindmap plain one-line HTML labels ending in `[]` / `()` now model Chromium's
  `getBoundingClientRect()` lattice for these delimiter-pair labels by reducing the vendored
  one-line HTML bbox width by `1/32px` when it already lands on the 1/64px browser grid and is
  below the wrapping container. This derives `String containing []` and the sibling
  `String containing ()` evidence in `upstream_pkgtests_mindmap_spec_019` without fixture, glyph,
  or root lookup data. Focused `parity-root` and full-DOM checks pass for
  `upstream_pkgtests_mindmap_spec_018`; focused full-DOM also passes for
  `upstream_pkgtests_mindmap_spec_019`. The old Mindmap profile calibration block now has 7
  retained branches. `upstream_root_type_cloud` stays retained for now: the typed cloud SVG path
  bounds/layout rule is already active, but the remaining single-node root tuple still reflects
  the browser HTML label bbox lattice for `the root` (`58.359375px` local vs `58.375px`
  upstream) rather than another cloud shape lookup.
- 2026-05-16: `upstream_whitespace_and_comments` no longer uses a Mindmap profile calibration.
  The old raw tuple guard (`337.2026680068237` x `389.4263190830933`) is stale after the typed
  Mindmap shape/text passes. Current natural output is `317.0134437302554` x
  `345.3722723123543`, close to the upstream `317.027587890625` x `345.3640441894531`, and the
  focused `parity-root` / full-DOM comparisons pass without a fixture, glyph, or root lookup.
  The old Mindmap profile calibration block now has 6 retained branches.
- 2026-05-18: the old Mindmap profile calibration block has been fully removed. The final retained
  branches were replaced by Mindmap-owned plain HTML label metrics rather than fixture, glyph, or
  root viewport lookup data: `Waterfall` derives the simple docs/package chain,
  `the root` derives `upstream_root_type_cloud` on top of the typed cloud path-bounds rule, and
  `Root` derives the docs `Root -> A -> {B, C}` / unclear-indentation profile by feeding the
  deterministic COSE layout the browser `foreignObject` bbox width. Focused `parity-root`,
  full-DOM, and SVG-position debug checks pass for the affected fixtures, and
  `svg/parity/mindmap.rs` has zero hand-written `parity-root calibration` profile branches.
- 2026-05-18: Flowchart retained-root recheck found no clean shared browser/font model to delete
  the next pin batch. The retained-root audit
  `cargo run -p xtask -- compare-flowchart-svgs --dom-mode parity-root --no-root-overrides
  --report-root-pins-only --report-root-all --report-label-root-pins-only --report-label-all
  --out target/compare/flowchart_root_pin_label_audit_current.md` passed as a reporting command,
  and `cargo run -p xtask -- triage-flowchart-root-pins --in
  target/compare/flowchart_root_pin_label_audit_current.md --out
  target/compare/flowchart_root_pin_triage_current.md` reports `49` root pins, `301` label delta
  rows, and no root-pin removal candidates. The remaining buckets are
  `defer-low-noise-text-lattice` (16), `defer-subpixel-text-lattice` (2),
  `defer-mojibake-font-fallback` (1), `defer-courier-font` (8), `defer-icon-font` (19), and
  `defer-font-env` (3). The sampled low-noise/default labels still have mixed-sign 1/64px lattice
  drift, SVG Markdown residuals stay subpixel, and the mojibake, Courier, icon, custom-font, and
  `code_flow` accumulation cases remain font-environment dependent. No fixture, glyph, or root
  viewport lookup data was added; all current Flowchart pins remain retained. Verification after
  the documentation update passed with `cargo fmt --all --check`,
  `cargo run -p xtask -- report-overrides --check-no-growth`, and
  `cargo run -p xtask -- audit-root-overrides --fail-on-stale`; the global audit still reports
  `308` inventory entries, `314` retained root-delta keys, `0` stale pins, and the same three
  accepted Mindmap outside-table residuals.
- 2026-05-18: GitGraph `BT` + `parallelCommits` root height drift is now derived by a typed axis
  rule rather than a root table. The fixture
  `upstream_cypress_gitgraph_spec_71_should_render_gitgraph_with_parallel_commits_vertical_branch_075`
  showed a natural disabled-root height of `329px` before the pass even though upstream's compact
  bottom-to-top graph is `239px` high. The fix lays out parallel commits in sequence order,
  applies the same parent-axis spacing as the top-to-bottom compact graph, then mirrors commit
  positions for `BT`; this keeps the branch label baseline at `210px` and aligns the emitted arrow
  and commit positions with upstream. Focused evidence:
  `cargo nextest run -p merman-render parallel_bt_commits_use_mirrored_compact_axis`,
  `cargo run -p xtask -- update-layout-snapshots --diagram gitgraph --filter
  upstream_cypress_gitgraph_spec_71_should_render_gitgraph_with_parallel_commits_vertical_branch_075`,
  and a disabled-root
  `compare-gitgraph-svgs --check-dom --dom-mode parity-root --dom-decimals 3 --report-root-all`
  filter pass for the same fixture. The generated root pin is intentionally retained: exact root
  attrs still differ by `-0.016px` in width (`331.006591796875px` upstream versus
  `330.990966796875px` local), matching the known vertical branch-label bbox lattice residual.
  A full disabled-root GitGraph recheck now has `15` `parity-root` DOM mismatches, with `spec_71`
  removed from the mismatch list while still present as a high-precision retained root-delta row.
  Post-change verification passed `cargo fmt --all --check`, full GitGraph normal DOM parity, full
  GitGraph `parity-root`, `report-overrides --check-no-growth`, and
  `audit-root-overrides --fail-on-stale`; the global audit reports GitGraph `23` inventory entries,
  `15` disabled-root DOM mismatches, `0` stale pins, and the same three accepted Mindmap
  outside-table residuals. The broader layout snapshot test was not green because of pre-existing
  Mindmap snapshot mismatches unrelated to the GitGraph fixture updated in this pass.
- 2026-05-18: rechecked the remaining GitGraph retained roots for a shared branch-label,
  commit-label, cherry-pick, or tag measurement rule. The fresh disabled-root sweep
  `cargo run -p xtask -- compare-gitgraph-svgs --check-dom --dom-mode parity-root
  --dom-decimals 3 --report-root-all --out target/compare/gitgraph_disabled_root_current.md`
  still has `23` generated root-delta keys and `15` 3-decimal `parity-root` DOM mismatches.
  Crossing the generated table with the mismatch list leaves `8` exact-root guards that do not
  currently fail the snapped DOM signature:
  `upstream_cypress_gitgraph_spec_71_should_render_gitgraph_with_parallel_commits_vertical_branch_075`,
  `upstream_html_demos_git_cherry_pick_from_branch_graph_015`,
  `upstream_html_demos_git_cherry_pick_from_main_graph_017`,
  `upstream_html_demos_git_cherry_pick_from_main_graph_018`,
  `upstream_html_demos_git_merge_feature_to_advanced_main_graph_007`,
  `upstream_html_demos_git_merge_from_main_onto_developed_branch_graph_025`,
  `upstream_html_demos_git_merge_from_main_onto_undeveloped_branch_graph_022`, and
  `upstream_html_demos_git_simple_branch_and_merge_graph_001`. Representative SVG inspection
  shows mixed-sign 1/64px drift rather than one reusable correction:
  `develop`/`feature` vertical branch-label rects are local `-0.015625px`, while `newbranch` and
  `0-a13d8e6` in the HTML branch/merge fixture are local `+0.015625px`; title fixtures mix
  title/root f32 lattice with rotated commit-label height, and tag guards include small tag polygon
  height residuals. A probe that changed GitGraph 10px commit/tag label bbox height from `15px`
  to the observed `15.05078125px` rect height fixed `upstream_merges_spec` in isolation but
  introduced many outside-table 0.25px-height `parity-root` mismatches; restricting the probe to
  TB/BT preserved LR/RL but no longer fixed the retained tag case. No fixture, glyph, or root
  viewport lookup data was added, no code change was kept, and the GitGraph table remains at
  `23` retained exact-root entries.
- 2026-05-18: rechecked the current State retained roots for shared note-cluster, RTL/scale,
  edge-label wrapping, style/font precedence, or browser-float derivation rules. The disabled-root
  sweep
  `cargo run -p xtask -- compare-state-svgs --check-dom --dom-mode parity-root --dom-decimals 3 --report-root-all --out target/compare/state_disabled_root_current.md`
  exited `1` as expected and produced `284` root-delta rows. Crossing that report with
  `state_root_overrides_11_12_2.rs` found all `33` generated State keys still have exact root
  deltas: `20` positive width drifts, `13` negative width drifts, `5` rows with height drift, and
  `32` snapped `parity-root` DOM mismatches. The only generated key without a snapped DOM mismatch
  is `stress_state_unicode_quotes_and_br_in_notes_048`, which still differs by an exact root
  width (`398.375 -> 398.367`) and remains an exact guard. Representative retained families are
  not one mechanism: `stress_state_html_sanitization_notes_025` expands from `365.93x402` to
  `799.11x530` through HTML-sanitized notes / noteGroup bounds;
  `stress_state_direction_rl_scale_and_long_ids_054` and
  `stress_state_batch5_direction_rl_scale_long_ids_065` shrink scaled RTL roots to `826.01px`;
  wrapped edge-label / Dagre cases include
  `upstream_cypress_statediagram_v2_spec_should_render_edge_labels_correctly_with_multiple_transitions_040`
  (`1283.54 -> 1143.46`) and `stress_state_scale_wrapping_long_edge_labels_038`
  (`375.64 -> 286.63`); `stress_state_font_size_precedence_071` mixes a `-30.30px` width drift
  with `386 -> 422` height drift; and the note-pair
  `state_with_a_note_together_with_another_state` stays on the known small note-cluster bounds
  delta. No global width/height slack, note text width, edge-label width, or font-size rule was
  clean enough to keep without trading retained roots for fixture-like or outside-table drift, so
  no fixture, glyph, or root viewport lookup data was added. Verification after the documentation
  update passed `git diff --check`, `cargo fmt --all --check`, State normal DOM parity,
  State `parity-root`, `cargo run -p xtask -- report-overrides --check-no-growth`, and
  `cargo run -p xtask -- audit-root-overrides --fail-on-stale --out
  target/compare/root_override_global_audit_current.md`. The global audit reports `308` root
  inventory entries, `314` fixture keys, `314` retained root-delta keys, `0` stale generated pins,
  and the same three accepted Mindmap outside-table residuals.
- 2026-05-19: derived the simple ER frontmatter-title root without adding fixture, glyph, text, or
  root lookup data. ER title root bounds now inherit the root SVG font-size, floor the title SVG
  bbox width to Chromium's 1/32px lattice, and include the extra 4px vertical title overhang. The
  focused disabled-root `parity-root` check for
  `upstream_cypress_erdiagram_spec_1433_should_render_a_simple_er_diagram_with_a_title_009` passes
  with natural root `0 0 148.03125 518`, so the generated ER root arm was deleted. Full ER normal
  DOM parity, full ER `parity-root`, `report-overrides --check-no-growth`, and the global root
  override audit pass; the audit reports `300` inventory entries, `306` fixture keys, `306`
  retained root-delta keys, `293` disabled-root DOM mismatches, and `0` stale generated pins.
- 2026-05-18: derived the empty Timeline root viewport without adding fixture, glyph, text, or
  root lookup data. The empty `timeline` render model has no pre-title nodes or lines, so it should
  not seed bounds from a synthetic `100x100` content box. The layout now keeps
  `pre_title_box_width` at `0`, which makes the activity line end at `450` (`3 * leftMargin`) and
  derives the upstream `100 50 400 100` root for
  `upstream_pkgtests_diagram_orchestration_spec_046`. Focused disabled-root `parity-root` passes
  for that fixture, full Timeline normal DOM parity and full Timeline `parity-root` pass, and
  `report-overrides --check-no-growth` reports `301` root entries with `8` Timeline entries.
  A fresh global audit reports `301` inventory entries, `307` fixture keys, `307` retained
  root-delta keys, `294` disabled-root DOM mismatches, `0` stale generated pins, and the same three
  accepted Mindmap outside-table residuals. The remaining eight Timeline pins were rechecked under
  disabled-root and remain retained: long-word roots still show small SVG text bbox overhang drift,
  the disable-multicolor/inline-hash/font-size stress roots show title/label browser bbox width
  drift despite matching emitted text and node geometry, and the Unicode/Fira Sans roots are
  height/vertical-line text metric residuals rather than stale table debt.
- 2026-05-16: Flowchart `low-noise-text` retained roots are now explicitly deferred as
  `defer-low-noise-text-lattice`. Browser probes for the affected plain/default-stack labels
  (`Find elements`, `Leave element`, `outside 1`, `node-X`, `Reject: reason`, `Go shopping 1`,
  `This is the (text) in the box`, and related labels) match upstream widths exactly, while the
  vendored model drifts by mixed signs on the 1/64px lattice. A broad tiny-pair suppression
  experiment improved some narrow labels but regressed already-too-wide labels, so this bucket is
  documented as low-value DOM/font lattice noise rather than a clean shared metric rule.
- 2026-05-15: Flowchart compare now supports focused retained-root audit rows via
  `--report-root-pins-only` and `--report-label-root-pins-only`. This keeps the label-level
  audit scoped to fixtures still covered by `flowchart_root_overrides_11_12_2.rs` instead of
  mixing in unrelated non-pinned fixtures. With root overrides disabled,
  `MERMAN_DISABLE_ROOT_VIEWPORT_OVERRIDES=1 cargo run -p xtask -- compare-flowchart-svgs
  --dom-mode parity-root --dom-decimals 3
  --report-root-pins-only --report-root-all --report-label-root-pins-only --report-label-all
  --out target/compare/flowchart_root_pin_label_audit_2026_05_15.md` passed as a reporting
  command and produced `67` retained Flowchart root fixture-key rows plus `386` retained label
  delta rows. The top retained buckets are still serif/custom-font style drift, long mojibake/CJK
  labels, icon/FontAwesome labels, punctuation-heavy plain labels, and shape/edge geometry drift.
- 2026-05-15: a focused disabled-root `parity-root` check rejected twelve apparent stale
  Flowchart candidates whose max-width delta was `0.000` but whose full root viewBox still
  differed. The rejected candidates were `upstream_flowchart_v2_stadium_shape_spec`, the three
  `wrapping_long_text_with_a_new_line` fixtures, the two nested-subgraph outgoing-link fixtures,
  the line-break/trapezoid Cypress fixtures, the `newshapesset5_lr_md_html_true` fixture, and
  the three HTML demo simple-graph aliases. All remain pinned because `parity-root` compares the
  full `viewBox`, not only `max-width`.
- 2026-05-15: `cargo run -p xtask -- report-overrides --check-no-growth` passed after the
  Flowchart root-pin-only audit tooling change with root total `326`, Flowchart root count `59`,
  text lookup total `484`, SVG text metric table total `186`, font metric table total `3774`,
  and zero helper overrides or manual raw SVG/path bridges.
- 2026-05-15: Flowchart recursive title-padding stale-pin cleanup removed
  `stress_flowchart_subgraph_deep_nesting_title_padding_044` from
  `flowchart_root_overrides_11_12_2.rs`. Focused normal and disabled-root
  `compare-flowchart-svgs --filter stress_flowchart_subgraph_deep_nesting_title_padding_044
  --check-dom --dom-mode parity-root --dom-decimals 3 --report-root-all --text-measurer vendored`
  both passed after the earlier recursive cluster title bbox derivation. The adjacent retained
  `stress_flowchart_subgraph_title_margin_extremes_015` and
  `stress_flowchart_subgraph_title_long_with_punct_038` still fail disabled-root comparison with
  real `max-width` drift, so their pins remain.
- 2026-05-15: after deleting the stale Flowchart title-padding pin,
  `cargo run -p xtask -- report-overrides --check-no-growth` passed with root total `348`,
  Flowchart root count `81`, text lookup total `484`, SVG text metric table total `186`, font
  metric table total `3774`, and zero helper overrides or manual raw SVG/path bridges. Full
  `cargo run -p xtask -- compare-flowchart-svgs --check-dom --dom-mode parity-root
  --dom-decimals 3 --text-measurer vendored` passed. `cargo nextest run -p merman-render` first
  hit MSVC `LNK1102` under parallel linking, then passed with `CARGO_BUILD_JOBS=1` and `-j 1`
  (`174` tests passed).
- 2026-05-15: `cargo run -p xtask -- report-overrides --check-no-growth` passed with root total
  `350`, Flowchart root count `83`, text lookup total `484`, SVG text metric table total `186`,
  font metric table total `3774`, and zero hand-curated helper overrides or manual raw SVG/path
  bridges.
- 2026-05-15: full disabled-root retained-root audit:
  `MERMAN_DISABLE_ROOT_VIEWPORT_OVERRIDES=1 cargo run -p xtask -- compare-all-svgs --check-dom --dom-mode parity-root --dom-decimals 3 --report-root-all`
  exited `1` as expected and wrote output to
  `target/disabled_root_audit_2026-05-15_current.txt`. Crossing the disabled-root DOM mismatches
  with generated root tables found `358` retained fixture keys, `358` mismatches, `0` stale pins,
  and `0` missing pins. No root viewport entry was deleted in this audit.
- 2026-05-14: Sequence stale-pin cross-check with root overrides disabled found
  `root=64 mismatch=59 stale=5 missing=0`. Focused disabled-root `parity-root` passed for the
  five stale simple-root fixtures, and `report-overrides` now reports root total `362`, Sequence
  root count `59`, text lookup total `484`, SVG text metric table total `186`, font metric table
  total `3774`, and zero manual raw SVG/path bridges.
- 2026-05-14: Flowchart root override table compression collapsed exact-duplicate generated match
  arms into or-patterns for fixture stems that already shared identical `(viewBox, max-width)`
  tuples. This preserves behavior and fixture-key coverage while reducing `report-overrides`
  inventory from `362` to `354` root entries, with Flowchart at `87`.
- 2026-05-14: Flowchart disabled-root audit command:
  `MERMAN_DISABLE_ROOT_VIEWPORT_OVERRIDES=1 cargo run -p xtask -- compare-flowchart-svgs --check-dom --dom-mode parity-root --dom-decimals 3 --report-root-all`.
  It exited `1` as expected for a retained-drift audit and wrote the diagnostic output to
  `target/flowchart_disabled_root_2026-05-14.txt`. Parsing that report and crossing it with
  `flowchart_root_overrides_11_12_2.rs` found `95` fixture-key mismatches, `95` retained keys,
  `0` stale retained pins, and `0` missing pins. The `87` Flowchart inventory entries reported by
  `xtask report-overrides` are match-arm rows after or-pattern compression, not fixture-key
  coverage. No Flowchart root pin was removed in this pass.
- 2026-05-14: Flowchart quoted-numeric rankSpacing now derives
  `upstream_cypress_flowchart_spec_23_render_a_simple_flowchart_with_rankspacing_set_to_100_023`
  without a root viewport pin. Focused disabled-root and normal `parity-root` checks pass for the
  fixture after parsing `flowchart.rankSpacing: '100'` as `100.0`, and
  `fixtures/flowchart/upstream_cypress_flowchart_spec_23_render_a_simple_flowchart_with_rankspacing_set_to_100_023.layout.golden.json`
  was refreshed. A full disabled-root Flowchart audit written to
  `target/flowchart_disabled_root_2026-05-14_after_rankspacing.txt` found `94` fixture-key
  mismatches, `94` retained keys, `0` stale retained pins, and `0` missing pins. `report-overrides`
  now reports root total `353` and Flowchart at `86`.
- 2026-05-14: Render numeric config parsing is centralized in
  `crates/merman-render/src/config.rs`. Full render nextest and full `parity-root` passed, and a
  disabled-root cross-check across generated root tables found `stale=0`, so the root budget stays
  at `353`.
- 2026-05-14: `cargo run -p xtask -- verify --strict` passed after the Flowchart table compression
  and no-growth budget tightening. The strict gate covered fmt, all-features check, workspace
  all-target/all-features clippy, override no-growth, feature matrix checks, workspace nextest
  (`1035` passed, `3` skipped), normal SVG DOM parity, and full SVG root parity.
- 2026-05-14: Sequence docs/control width facts now match upstream SVG actor/frame spacing for
  `Feeling fresh like a daisy`, `Fine, thank you. And you?`, `Hello Charley, how are you?`, and
  `Did you want to go to the game tonight?`. Six docs/control Sequence root pins were deleted
  after focused disabled-root `parity-root` checks passed. `report-overrides` now reports root
  total `367`, Sequence root count `64`, text lookup total `484`, SVG text metric table total
  `186`, font metric table total `3774`, and zero manual raw SVG/path bridges.
- 2026-05-14: GitGraph vertical branch-label root bounds now use Mermaid's
  `drawText(name).getBBox()`-style centered SVG bbox for TB/BT branch labels with ties-to-even
  1/64px quantization, while LR/RL keeps the computed-length branch-label rule. With
  `MERMAN_DISABLE_ROOT_VIEWPORT_OVERRIDES=1`, the audit of the previous 65-entry GitGraph table
  found 24 retained DOM mismatches and 41 stale pins. The stale pins were deleted, leaving
  GitGraph at `24` root entries and root total `383`.
- 2026-05-14: GitGraph commit/tag label theme-variable parity now honors Mermaid's
  `commitLabelFontSize`, `tagLabelFontSize`, label colors, backgrounds, and tag border variables
  in emitted CSS and root measurement. Focused disabled-root checks for
  `upstream_docs_gitgraph_customizing_commit_label_font_size_032` and
  `upstream_docs_gitgraph_customizing_tag_label_font_size_033` passed without the commit-label
  root pin, leaving GitGraph at `23` root entries and root total `382`.
- 2026-05-14: GitGraph commit and tag label root bounds now use GitGraph-owned
  `getComputedTextLength()`-style widths with 1/64px quantization. With
  `MERMAN_DISABLE_ROOT_VIEWPORT_OVERRIDES=1`, the audit of the previous 130-entry GitGraph table
  found 65 retained DOM mismatches and 65 stale pins. The stale pins were deleted, leaving
  GitGraph at `65` root entries.
- 2026-05-14: full normal GitGraph DOM, full GitGraph `parity-root`, and
  `report-overrides --check-no-growth` passed after the commit/tag label cleanup.
  `report-overrides` now reports root total `432`, GitGraph root count `65`, text lookup total
  `484`, SVG text metric table total `186`, font metric table total `3774`, and zero manual raw
  SVG/path bridges.
- 2026-05-13: GitGraph seeded auto commit ids now mirror upstream's committed SVG generation
  sequence by running a seed-consuming parse warm-up before the render-model parse. The simple
  seeded fixture now produces `0-5b722bd`, matching the upstream parse-before-render pipeline
  rather than the earlier single-render `0-ab40cda` stream position.
- 2026-05-13: with `MERMAN_DISABLE_ROOT_VIEWPORT_OVERRIDES=1`, the post-seed GitGraph audit
  found `override=130 mismatch=130 stale=0 missing=0` after deleting 26 net stale pins. The
  `upstream_direction_bt` pin remains because the corrected dynamic commit id exposes a real
  BT-direction branch/commit-label bbox root drift.
- 2026-05-13: full normal GitGraph DOM and full GitGraph `parity-root` passed after the seeded
  auto-id warm-up and root-table pruning. `report-overrides --check-no-growth` passed with root
  total `497`, GitGraph root count `130`, text lookup total `484`, SVG text metric table total
  `186`, and zero manual raw SVG/path bridges.
- 2026-05-13: with `MERMAN_DISABLE_ROOT_VIEWPORT_OVERRIDES=1`, a GitGraph disabled-root audit
  produced 251 root rows. Crossing the disabled-root mismatch list with
  `gitgraph_root_overrides_11_12_2.rs` found two then-stale retained pins:
  `upstream_cypress_gitgraph_spec_88_should_hide_branches_with_tb_orientation_when_showbranches_is_092`
  and `upstream_direction_bt`. The later seeded auto-id warm-up pass restored
  `upstream_direction_bt` because the corrected dynamic commit id exposed a real BT-direction bbox
  guard.
- 2026-05-13: focused `cargo run -p xtask -- compare-gitgraph-svgs --check-dom --dom-mode
  parity-root --dom-decimals 3 --filter <fixture> --report-root-all` passed for both removed
  GitGraph fixtures.
- 2026-05-13: full `cargo run -p xtask -- compare-gitgraph-svgs --check-dom --dom-mode
  parity-root --dom-decimals 3`, `cargo run -p xtask -- report-overrides --check-no-growth`,
  `cargo clippy -p merman-render -p xtask --all-targets --all-features -- -D warnings`, and
  `cargo nextest run -p xtask -p merman-render gitgraph override_growth_check` passed after the
  GitGraph cleanup. The targeted nextest expression matched the xtask override budget tests; a
  separate `cargo nextest run -p merman-render gitgraph` found no render-side GitGraph tests, so
  GitGraph SVG parity is covered by the compare command.
- 2026-05-13: `report-overrides --check-no-growth` passed with root total `616`, GitGraph root
  count `226`, text lookup total `484`, SVG text metric table total `186`, and zero manual raw
  SVG/path bridges.
- 2026-05-13: GitGraph title-root derivation added `gitTitleText` bbox coverage to emitted root
  bounds and removed 13 title-dominated GitGraph root pins. Full GitGraph `parity-root`,
  `report-overrides --check-no-growth`, and render/xtask clippy passed. A disabled-root
  cross-check found `213` retained GitGraph root entries and `213` matching DOM mismatches, so no
  additional stale GitGraph root pins were deleted in this pass. `report-overrides
  --check-no-growth` reported root total `603`, GitGraph root count `213`, text lookup total
  `484`, SVG text metric table total `186`, and zero manual raw SVG/path bridges.
- 2026-05-13: GitGraph `parallelCommits` parentless LR commits now restart from the commit-axis
  start, matching Mermaid's independent branch timelines for unconnected branches. Focused
  disabled-root checks on
  `upstream_cypress_gitgraph_spec_45_should_render_gitgraph_with_unconnected_branches_and_parallel_048`
  reduced the root-width drift from `+150.250px` to `+0.250px`; the remaining drift comes from
  branch-label browser bbox measurement, so no root pin was removed. `cargo fmt --check`,
  `cargo nextest run -p merman-render parallel_lr_unconnected_branches_restart_commit_axis`,
  full GitGraph normal DOM, full GitGraph `parity-root`, and `report-overrides
  --check-no-growth` passed for this pass.
- 2026-05-13: GitGraph root bbox derivation now includes the branch line endpoints emitted by the
  GitGraph renderer, covering browser `getBBox()` behavior for zero-length branch lines without
  changing the shared emitted-bounds scanner. Focused disabled-root checks for
  `upstream_pkgtests_diagram_orchestration_spec_048`, `upstream_pkgtests_gitgraph_spec_076`, and
  `upstream_pkgtests_gitgraph_test_011` through `_013` reduced the former roughly `+34.750px`
  width gap to `+0.250px`/`+0.266px` branch-label bbox drift. Full disabled-root GitGraph
  cross-check still found `override=213 mismatch=213 stale=0 missing=0`; full GitGraph
  `parity-root`, `report-overrides --check-no-growth`, render/xtask clippy, and
  `cargo nextest run -p merman-render` passed, so no root pin was deleted. The follow-up
  `cargo run -p xtask -- verify --strict` gate also passed with root total `602`, GitGraph root
  count `213`, Sequence root count `79`, text lookup total `484`, and SVG text metric table total
  `186`.
- 2026-05-13: Horizontal GitGraph branch-label layout now uses
  `<text>.getComputedTextLength()`-style width instead of ASCII-overhang simple bbox width, while
  TB/BT directions keep the previous branch-label bbox path because rotated dynamic commit IDs can
  dominate vertical root bounds. Full disabled-root GitGraph cross-check changed from
  `override=213 mismatch=156 stale=57 missing=0` before deletion to
  `override=156 mismatch=156 stale=0 missing=0` after deleting the 57 now-derived GitGraph root
  pins. Full GitGraph `parity-root`, `report-overrides --check-no-growth`, and the xtask
  override budget regression test passed with root total `545`, GitGraph root count `156`, text
  lookup total `484`, and SVG text metric table total `186`.
- 2026-05-13: Flowchart imageSquare layout now derives Dagre node bounds from the rendered image
  plus label extent. Focused disabled-root and normal `parity-root` checks passed for
  `upstream_docs_flowchart_parameters_136`, so its root pin was deleted. Full Flowchart
  `parity-root`, `report-overrides --check-no-growth`, `cargo nextest run -p merman-render`, and
  `cargo run -p xtask -- verify --strict` passed with root total `544` and Flowchart root count
  `124`.
- 2026-05-13: Flowchart anchor nodes now ignore labels for Dagre layout and use the seeded 2px
  roughjs dot bbox, matching Mermaid's no-label anchor renderer. Disabled-root `parity-root`
  checks passed for the old-shape set5 stale-pin cluster except
  `upstream_cypress_oldshapes_spec_shapessets_shapesset5_tb_md_html_false_038`, which still has a
  real 0.06px root drift and remains pinned. `report-overrides` then reported root total `532` and
  Flowchart root count `112`.
- 2026-05-13: Flowchart C1 control bytes in mojibake HTML labels now measure as Chromium
  near-full-em replacement glyphs. The courier long-name/class-definition Cypress fixture passes
  focused disabled-root `parity-root`, while the handdrawn/default-font sibling still has real
  residual drift and remains pinned. `report-overrides` now reports root total `531` and
  Flowchart root count `111`.
- 2026-05-13: Flowchart SVG-like subgraph title layout now reuses emitted SVG text wrapping and
  default process nodes size from wrapped computed text length. The stage2 long-word subgraph-title
  fixture passes focused disabled-root `parity-root`, so its root pin was deleted.
  `report-overrides` now reports root total `530` and Flowchart root count `110`.
- 2026-05-13: Flowchart HTML label extraction now preserves bare `<` and `>` comparison text, and
  single-line default-stack labels with literal comparison symbols get a narrow CJK width cushion.
  The Unicode/entities subgraph-title fixture passes focused disabled-root `parity-root`, so its
  root pin was deleted. `report-overrides` now reports root total `529` and Flowchart root count
  `109`.
- 2026-05-13: Focused disabled-root `parity-root` checks showed the two Flowchart subgraph
  title-margin fixtures
  `upstream_cypress_flowchart_v2_spec_should_render_subgraphs_with_title_margins_set_lr_and_htmllabels_062`
  and `upstream_flowchart_v2_subgraph_title_margins_lr_htmlLabels_false_spec` now derive their
  root viewport without fixture pins. Both pins were deleted, and `report-overrides` now reports
  root total `527` and Flowchart root count `107`.
- 2026-05-13: Flowchart HTML label measurement now uses a separate measurement base style from
  SVG root CSS. Numeric `themeVariables.fontSize` remains a root CSS value but does not resize
  `foreignObject` label measurement; valid `"NNpx"` theme strings and class/inline font-size
  rules still do. `stress_flowchart_font_size_precedence_073` passes focused disabled-root
  `parity-root`, so its root pin was deleted. `report-overrides` now reports root total `526` and
  Flowchart root count `106`.
- 2026-05-13: Flowchart `iconSquare` layout now includes Mermaid's icon-shape outer padding
  (`iconSize + halfPadding * 2`, equivalent to `iconSize + node.padding`) before Dagre/root bounds.
  `upstream_docs_flowchart_icon_shape_132` passes focused disabled-root `parity-root`, so its root
  pin was deleted and its layout golden was refreshed. `report-overrides` now reports root total
  `525` and Flowchart root count `105`.
- 2026-05-13: Flowchart FontAwesome HTML-label measurement now models Mermaid's unregistered
  custom-pack fallback for `fab:fa-truck-bold` as the empty `<i>` emitted by upstream
  `createText.ts`, including the Chromium 1/64px inline advance observed in the docs custom-icons
  fixture. `upstream_docs_flowchart_custom_icons_238` and
  `stress_flowchart_icons_prefixes_and_quotes_052` pass focused disabled-root `parity-root`, so
  both root pins were deleted. `report-overrides` now reports root total `523` and Flowchart root
  count `103`.
- 2026-05-13: Before the imageSquare layout-bounds pass, a Flowchart disabled-root mismatch
  cross-check found `125` override entries and `125` matching disabled-root DOM mismatches, so no
  stale retained Flowchart root pin was deleted in that audit pass.
- 2026-05-13: Sequence disabled-root mismatch cross-check found `80` override entries and `80`
  matching disabled-root DOM mismatches, so no stale retained Sequence root pin was deleted in
  this pass.
- 2026-05-12: with `MERMAN_DISABLE_ROOT_VIEWPORT_OVERRIDES=1`, focused
  `compare-sequence-svgs --check-dom --dom-mode parity-root --dom-decimals 3 --filter ...` passed
  for the nine removed Sequence root pins. `cargo clippy -p merman-render --all-targets
  --all-features -- -D warnings`, `cargo nextest run -p merman-render`, full
  `cargo run -p xtask -- compare-sequence-svgs --check-dom --dom-mode parity-root --dom-decimals 3`,
  and `cargo run -p xtask -- report-overrides --check-no-growth` passed with root total `702`,
  Sequence root count `164`, text lookup count `484`, and SVG text metric total `186`.
- 2026-05-12: with `MERMAN_DISABLE_ROOT_VIEWPORT_OVERRIDES=1`, focused `compare-sequence-svgs`
  `--check-dom --dom-mode parity-root --dom-decimals 3` passed for the six long-note/long-message
  fixtures after fixing leftOf note start recomputation and adding the shared long-message SVG
  metric fact. `report-overrides --check-no-growth` passed with root total `711`, Sequence root
  count `173`, and SVG text metric total `186`.
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
  after deleting the boundary root pin and deriving the actor spacing through Sequence
  message-width metrics.
- 2026-05-12: with `MERMAN_DISABLE_ROOT_VIEWPORT_OVERRIDES=1`, the same focused boundary
  `parity-root` check passed, proving the root viewport is no longer fixture-pinned.
- 2026-05-12: `cargo run -p xtask -- compare-sequence-svgs --check-dom --dom-mode parity-root
  --dom-decimals 3 --report-root-all` passed for all Sequence fixtures.
- 2026-05-12: `cargo run -p xtask -- report-overrides --check-no-growth` passed with root total
  `734`, State root count `34`, Mindmap root count `39`, Sequence root count `196`, text lookup
  total `484`, SVG text metric table total `186`, and zero manual raw SVG/path bridges.
- 2026-05-12: `cargo run -p xtask -- verify --strict` passed after the Sequence boundary pass.
  The strict gate included `cargo fmt --check`, workspace `cargo clippy --all-targets --all-features
  -- -D warnings`, workspace `cargo nextest run` (`1022` passed, `3` skipped), override
  no-growth, feature matrix checks, normal SVG DOM parity, and root SVG DOM parity.
- 2026-05-12: `cargo test -p merman-render
  sequence_default_message_widths_match_mermaid_default_font_family` passed after correcting the
  default Sequence message-width facts for `Hello Bob, how are you?` and
  `Hello John, how are you?`.
- 2026-05-12: with `MERMAN_DISABLE_ROOT_VIEWPORT_OVERRIDES=1`, focused `parity-root` checks passed
  for `title_and_accdescr_multiline`, `upstream_accessibility_single_line_spec`, and
  `upstream_docs_accessibility_sequence_diagram_014` after deleting their root pins.
- 2026-05-12: `cargo run -p xtask -- compare-sequence-svgs --check-dom --dom-mode parity-root
  --dom-decimals 3 --report-root-all` passed for all Sequence fixtures after the title/accessibility
  pass.
- 2026-05-12: `cargo run -p xtask -- report-overrides --check-no-growth` passed with root total
  `731`, State root count `34`, Mindmap root count `39`, Sequence root count `193`, text lookup
  total `484`, SVG text metric table total `186`, and zero manual raw SVG/path bridges.
- 2026-05-12: `cargo run -p xtask -- verify --strict` passed after the Sequence
  title/accessibility pass. The strict gate included `cargo fmt --check`, workspace
  `cargo clippy --all-targets --all-features -- -D warnings`, workspace `cargo nextest run`
  (`1023` passed, `3` skipped), override no-growth, feature matrix checks, normal SVG DOM parity,
  and root SVG DOM parity.
- 2026-05-12: with `MERMAN_DISABLE_ROOT_VIEWPORT_OVERRIDES=1`, focused `parity-root` checks passed
  for `upstream_title_without_colon_spec` and `upstream_pkgtests_sequencediagram_spec_020`, so the
  residual default-title root pins were deleted.
- 2026-05-12: `cargo run -p xtask -- report-overrides --check-no-growth` passed with root total
  `729`, State root count `34`, Mindmap root count `39`, Sequence root count `191`, text lookup
  total `484`, SVG text metric table total `186`, and zero manual raw SVG/path bridges.
- 2026-05-12: with `MERMAN_DISABLE_ROOT_VIEWPORT_OVERRIDES=1`, focused `parity-root` checks passed
  for the simple `Bob thinks` note-right trio `upstream_pkgtests_sequencediagram_spec_007`,
  `upstream_pkgtests_sequencediagram_spec_009`, and `upstream_pkgtests_sequencediagram_spec_042`;
  the three residual root pins were deleted.
- 2026-05-12: `cargo run -p xtask -- report-overrides --check-no-growth` passed with root total
  `726`, State root count `34`, Mindmap root count `39`, Sequence root count `188`, text lookup
  total `484`, SVG text metric table total `186`, and zero manual raw SVG/path bridges.
- 2026-05-12: `cargo run -p xtask -- compare-sequence-svgs --check-dom --dom-mode parity-root
  --dom-decimals 3 --report-root-all` passed for all Sequence fixtures after the simple note-right
  pass.
- 2026-05-12: `cargo run -p xtask -- verify --strict` passed after the Sequence simple note-right
  pass. The strict gate included `cargo fmt --check`, workspace `cargo clippy --all-targets
  --all-features -- -D warnings`, workspace `cargo nextest run` (`1023` passed, `3` skipped),
  override no-growth, feature matrix checks, normal SVG DOM parity, and root SVG DOM parity.
- 2026-05-12: with `MERMAN_DISABLE_ROOT_VIEWPORT_OVERRIDES=1`, focused `parity-root` checks passed
  for the whitespace/comment `Bob thinks` note-right trio
  `upstream_pkgtests_sequencediagram_spec_043`, `upstream_pkgtests_sequencediagram_spec_045`, and
  `upstream_pkgtests_sequencediagram_spec_046`; the three residual root pins were deleted.
- 2026-05-12: `cargo run -p xtask -- report-overrides --check-no-growth` passed with root total
  `723`, State root count `34`, Mindmap root count `39`, Sequence root count `185`, text lookup
  total `484`, SVG text metric table total `186`, and zero manual raw SVG/path bridges.
- 2026-05-12: `cargo run -p xtask -- compare-sequence-svgs --check-dom --dom-mode parity-root
  --dom-decimals 3 --report-root-all` passed for all Sequence fixtures after the
  whitespace/comment note-right pass.
- 2026-05-12: `cargo run -p xtask -- verify --strict` passed after the Sequence
  whitespace/comment note-right pass. The strict gate included `cargo fmt --check`, workspace
  `cargo clippy --all-targets --all-features -- -D warnings`, workspace `cargo nextest run`
  (`1023` passed, `3` skipped), override no-growth, feature matrix checks, normal SVG DOM parity,
  and root SVG DOM parity.
- 2026-05-12: with `MERMAN_DISABLE_ROOT_VIEWPORT_OVERRIDES=1`, focused `parity-root` checks passed
  for the block `Bob thinks` note-right trio `upstream_pkgtests_sequencediagram_spec_054`,
  `upstream_pkgtests_sequencediagram_spec_055`, and `upstream_pkgtests_sequencediagram_spec_056`;
  the three residual loop/rect/nested-rect root pins were deleted.
- 2026-05-12: `cargo run -p xtask -- report-overrides --check-no-growth` passed with root total
  `720`, State root count `34`, Mindmap root count `39`, Sequence root count `182`, text lookup
  total `484`, SVG text metric table total `186`, and zero manual raw SVG/path bridges.
- 2026-05-12: `cargo run -p xtask -- compare-sequence-svgs --check-dom --dom-mode parity-root
  --dom-decimals 3 --report-root-all` passed for all Sequence fixtures after the block note-right
  pass.
- 2026-05-12: `cargo run -p xtask -- verify --strict` passed after the Sequence block note-right
  pass. The strict gate included `cargo fmt --check`, workspace `cargo clippy --all-targets
  --all-features -- -D warnings`, workspace `cargo nextest run` (`1023` passed, `3` skipped),
  override no-growth, feature matrix checks, normal SVG DOM parity, and root SVG DOM parity.
- 2026-05-12: with `MERMAN_DISABLE_ROOT_VIEWPORT_OVERRIDES=1`, focused `parity-root` checks passed
  for the alt-control `Bob thinks` note-right trio `upstream_pkgtests_sequencediagram_spec_058`,
  `upstream_pkgtests_sequencediagram_spec_059`, and `upstream_alt_multiple_elses_spec`; the three
  residual alt root pins were deleted.
- 2026-05-12: `cargo run -p xtask -- report-overrides --check-no-growth` passed with root total
  `717`, State root count `34`, Mindmap root count `39`, Sequence root count `179`, text lookup
  total `484`, SVG text metric table total `186`, and zero manual raw SVG/path bridges.
- 2026-05-12: `cargo run -p xtask -- compare-sequence-svgs --check-dom --dom-mode parity-root
  --dom-decimals 3 --report-root-all` passed for all Sequence fixtures after the alt-control
  note-right pass.
- 2026-05-12: `cargo run -p xtask -- verify --strict` passed after the Sequence alt-control
  note-right pass. The strict gate included `cargo fmt --check`, workspace
  `cargo clippy --all-targets --all-features -- -D warnings`, workspace `cargo nextest run`
  (`1023` passed, `3` skipped), override no-growth, feature matrix checks, normal SVG DOM parity,
  and root SVG DOM parity.
- 2026-05-13: after the Sequence actor/root-bounds layout-owner decomposition,
  `MERMAN_DISABLE_ROOT_VIEWPORT_OVERRIDES=1 cargo run -p xtask -- compare-sequence-svgs
  --check-dom --dom-mode parity-root --dom-decimals 3 --report-root-all --out
  target/compare/sequence_disabled_root_audit_2026_05_13.md` exited `1` as expected for a
  disabled-root audit. The report contained 320 root rows, 73 non-zero `max-width` deltas, 80
  changed viewBox dimensions, and 80 DOM mismatches, matching the current Sequence root table size;
  no stale retained Sequence root pin was found.
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
