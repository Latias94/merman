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
| Replace fixture-scoped overrides where practical | Code changes plus generated table deletion or table compression | Started: eleven State root pins, thirteen Mindmap root pins, fifty-four Sequence root pins, two hundred four net GitGraph root pins, and thirty Flowchart root pins removed; the latest Flowchart table cleanup also collapses eight exact-duplicate inventory arms without changing fixture-key coverage |
| Keep `parity-root` green | Focused `compare-*-svgs --dom-mode parity-root` commands | Full State, Mindmap, Sequence, GitGraph, and Flowchart passes recorded |
| Keep clippy green for render edits | `cargo clippy -p merman-render --all-targets --all-features -- -D warnings` | Passed |
| Keep nextest green for shared behavior edits | `cargo nextest run` | Render crate and strict workspace nextest passed |
| Keep strict release gate green | `cargo run -p xtask -- verify --strict` | Passed |

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
HTML-label font-size precedence, iconSquare layout-bounds, custom FontAwesome fallback, LR
fork/join direction-sensitive sizing, and quoted-numeric rankSpacing passes:

- State: `34` entries.
- Mindmap: `39` entries.
- Sequence: `59` entries.
- GitGraph: `23` entries.
- Flowchart: `86` inventory entries.
- Root viewport total: `353` entries.
- Text lookup total: `484` entries. This stayed flat because the new long-note/message Sequence
  fact replaced one stale `FRIENDS` row, and the wrapped-leftOf follow-up removed nine more root
  pins without adding lookup rows.
- SVG text metric table total: `186` rows. The long-note/message fact kept the budget flat after
  the stale row cleanup.

The latest Mindmap focused disabled-root checks show the plain wrapping prose/icon trio and five
additional stale retained pins are covered by the current layout/bounds derivation. The remaining
retained entries still include long-word min-content drift, Markdown/HTML sanitization,
icon-bearing stress fixtures, shape profiles, and tree-wide transform drift. This workstream
therefore focuses on derivation work, not blind deletion.

The latest State disabled-root sweep still fails as expected with the 34 retained State root pins
acting as current guards. They cluster around HTML-sanitized notes, right-to-left scale bounds with
long IDs, dense or wrapping edge-label bounds, markdown edge labels, note/multiline-label geometry,
unicode/RTL text metrics, style/font precedence, and small browser float or lattice guards.

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
`upstream_docs_sequence_rect_nested_example`. The participant-creation v2 sibling remains pinned:
with root overrides disabled its width matches, but the root height still drifts from upstream
`1040x580` to local `1040x591`, so the next fix belongs in participant type/lifecycle vertical
geometry rather than another text-width fact.
A follow-up disabled-root mismatch cross-check over the then-current Sequence table found
`root=64 mismatch=59 stale=5 missing=0`. The stale simple-root pins were
`upstream_cypress_sequencediagram_v2_spec_should_render_a_sequence_diagram_when_usemaxwidth_is_false_030`,
`actor_ids_dashes_and_equals`, `upstream_cypress_sequencediagram_spec_example_001`,
`upstream_cypress_sequencediagram_spec_should_render_a_sequence_diagram_when_usemaxwidth_is_false_059`,
and `upstream_docs_examples_basic_sequence_diagram_005`; all five pass focused disabled-root
`parity-root` and were deleted.

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
- icon and FontAwesome labels: the bucket includes the `stress_flowchart_icons_multiline_br_054`
  `-72px` height gap plus icon/wrap/subgraph width gaps up to roughly `10px`; these need icon
  line-break/glyph/cluster measurement work before pins can be deleted.
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

## Focused Commands

```sh
cargo run -p xtask -- compare-state-svgs --check-dom --dom-mode parity-root --dom-decimals 3 --report-root-all
cargo run -p xtask -- compare-mindmap-svgs --check-dom --dom-mode parity-root --dom-decimals 3 --report-root-all
cargo run -p xtask -- compare-sequence-svgs --check-dom --dom-mode parity-root --dom-decimals 3 --report-root-all
cargo run -p xtask -- compare-gitgraph-svgs --check-dom --dom-mode parity-root --dom-decimals 3 --report-root-all
cargo run -p xtask -- compare-flowchart-svgs --check-dom --dom-mode parity-root --dom-decimals 3 --report-root-all
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
