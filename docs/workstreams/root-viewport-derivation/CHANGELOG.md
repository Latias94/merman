# Root Viewport Derivation Changelog

## 2026-05-19

- Derived the ER error-demo root by moving the browser `foreignObject` width for `ATLAS-TEAMS`
  into ER-owned HTML label metrics. Upstream emitted the 16px entity label at `94.625px`, while
  the local vendored HTML measurement produced `96.234375px`; that entity-width delta was the
  remaining small-width root driver for `upstream_html_demos_error_example_001`. One text lookup
  replaces the root pin, reducing ER roots from `7` to `6`, global root inventory from `287` to
  `286`, and increasing text lookup from `489` to `490`.
- Rechecked the ER retained-root bucket after the `ATLAS-TEAMS` metric. A full disabled-root ER
  `parity-root` sweep now reports exactly the remaining `6` ER root pins as DOM mismatches:
  recursive relationship, SVG-mode edge-label, docs layout, large HTML demo, and the two multiline
  demo residuals. The global `audit-root-overrides --fail-on-stale` report is also clean with
  `286` inventory entries, `292` fixture keys, `292` retained root-delta keys, `279`
  disabled-root DOM mismatches, `0` stale generated pins, and the same three accepted Mindmap
  outside-table DOM residuals.
- Derived the ER docs markdown-formatting root by moving the browser `foreignObject` width for
  `This **is** _Markdown_` into ER-owned HTML label metrics. Upstream emitted the markdown entity
  with a `123.859375px` label width and `163.859375px` rect width, while the local vendored
  measurement produced `162.25px` and `202.25px`. One text lookup replaces the root pin, reducing
  ER roots from `8` to `7`, global root inventory from `288` to `287`, and increasing text lookup
  from `488` to `489`.
- Rechecked the ER retained-root bucket after the markdown metric. A full disabled-root ER
  `parity-root` sweep now reports exactly the remaining `7` ER root pins as DOM mismatches; the
  retained rows are the recursive relationship, SVG-mode edge-label, docs layout, large HTML demo,
  multiline demo, and error-demo residuals.
- Derived the shared ER `CATEGORY` entity-label root bucket by moving its browser
  `foreignObject` width fact into ER-owned HTML label metrics. The upstream 16px width is
  `74.328125px`, and recording it makes
  `upstream_cypress_erdiagram_spec_should_render_er_diagram_with_1_cardinality_alias_before_relatio_020`
  derive its root naturally with root overrides disabled. One text lookup replaces one root pin,
  reducing ER roots from `9` to `8`, global root inventory from `289` to `288`, and increasing
  text lookup from `487` to `488`.
- Rechecked the ER retained-root bucket after the `CATEGORY` metric. A full disabled-root ER
  `parity-root` sweep now reports exactly the remaining `8` ER root pins as DOM mismatches. The
  edge-label SVG-mode candidate stays retained: a naive root-font-size/+4px label-box rule fixed
  the edge-label fixture height but widened its root to `561.73046875px` versus upstream
  `544.371826171875px`, so the residual is not a safe shared text-width rule.
- Derived the shared ER `Customer Account Tertiary` entity-label root bucket by moving its browser
  `foreignObject` width fact into ER-owned HTML label metrics. Element probes showed the two
  multiline relationship labels were not the root driver; their widths were slightly smaller
  locally, while `Customer Account Tertiary` widened from upstream `189.78125px` to local
  `190.078125px`, shifting the right-side entity column and root width. One text lookup replaces
  the relationship-line-break and HTML demo root pins, reducing ER roots from `11` to `9`, global
  root inventory from `291` to `289`, and increasing text lookup from `486` to `487`.
- Rechecked the ER retained-root bucket after the `Customer Account Tertiary` metric. A full
  disabled-root ER `parity-root` sweep now reports exactly the remaining `9` ER root pins as DOM
  mismatches, while full ER normal DOM parity and full ER `parity-root` stay green. The retained
  rows remain split across recursive relationship geometry, edge-label bounds,
  multiline/attribute-table width drift, docs layout, markdown formatting, and error-demo
  residuals.
- Derived the shared ER `PRODUCT-CATEGORY` entity-label root bucket by moving its browser
  `foreignObject` width fact into ER-owned HTML label metrics. The upstream 16px width is
  `146.71875px` versus the local vendored HTML width `150.890625px`; the `4.171875px` label delta
  propagated through the entity rect, right-side entity column, and root width for the not-so-
  simple/theme/syntax-reference trio. One text lookup replaces three root pins, reducing ER roots
  from `14` to `11`, global root inventory from `294` to `291`, and increasing text lookup from
  `485` to `486`.
- Rechecked the ER retained-root bucket after the `PRODUCT-CATEGORY` metric. A full disabled-root
  ER `parity-root` sweep now reports exactly the remaining `11` ER root pins as DOM mismatches,
  while full ER normal DOM parity and full ER `parity-root` stay green. The retained rows now
  exclude the simple product/category entity-label bucket and remain split across recursive
  relationship geometry, edge-label bounds, multiline/attribute-table width drift, docs layout,
  markdown formatting, and error-demo residuals.
- Derived the shared ER `DELIVERY-ADDRESS` entity-label root bucket by moving the browser
  `foreignObject` width fact into ER-owned HTML label metrics. One 16px text lookup now drives the
  entity rect width and root bounds for the package and docs/accessibility variants, replacing six
  fixture-scoped root pins. The ER root table drops from `20` to `14`, global root inventory is
  `294`, and text lookup increases from `484` to `485`.
- Rechecked the ER retained-root bucket after the `DELIVERY-ADDRESS` metric. A full disabled-root
  ER `parity-root` sweep now reports exactly the remaining `14` ER root pins as DOM mismatches,
  while full ER normal DOM parity and full ER `parity-root` stay green. The retained ER rows still
  split across other entity-label widths, multiline/attribute-table width drift, recursive
  relationship and edge-label bounds residuals, and ELK/layout root differences rather than one
  safe shared rule.
- Derived the ER simple frontmatter-title root viewport without adding fixture, glyph, text, or
  root lookup data. ER title bounds now inherit the root SVG font-size, floor the browser SVG bbox
  width to the 1/32px lattice, and include Chromium's extra 4px vertical title overhang, so
  `upstream_cypress_erdiagram_spec_1433_should_render_a_simple_er_diagram_with_a_title_009`
  naturally derives its `148.03125x518` root with root overrides disabled. The ER root table drops
  from `21` to `20`, global root inventory is `300`, and text lookup remains `484`.
- Rechecked the remaining ER retained-root bucket after the title derivation. A full disabled-root
  ER `parity-root` sweep now reports exactly the remaining `20` ER root pins as DOM mismatches,
  while full ER normal DOM parity and full ER `parity-root` stay green. The retained ER rows still
  split across entity-label browser width drift, multiline/attribute-table width drift, recursive
  relationship and edge-label bounds residuals, and ELK/layout root differences rather than one
  safe shared rule.
- Re-ran the global generated root override audit after the ER title cleanup. The current
  `audit-root-overrides --fail-on-stale` report is clean on stale pins across `300` root inventory
  entries, `306` fixture keys, and `306` retained root-delta keys. It reports `293` disabled-root
  DOM mismatches plus the same three accepted Mindmap outside-table residuals.

## 2026-05-18

- Derived the empty Timeline root viewport without adding fixture, glyph, text, or root lookup
  data. Empty Timeline diagrams now keep `pre_title_box_width` at `0` instead of inventing a
  synthetic 100px content box, so the activity line ends at `3 * leftMargin` and
  `upstream_pkgtests_diagram_orchestration_spec_046` naturally derives its upstream `400px` root.
  The Timeline root table is reduced from `9` to `8`, global root inventory is `301`, and text
  lookup remains `484`.
- Re-ran the global generated root override audit after the Timeline empty-root cleanup. The
  current `audit-root-overrides --fail-on-stale` report is clean on stale pins across `301` root
  inventory entries, `307` fixture keys, and `307` retained root-delta keys. It reports `294`
  disabled-root DOM mismatches plus the same three accepted Mindmap outside-table residuals. The
  remaining eight Timeline roots all still fail with root overrides disabled.
- Derived the repeated Requirement styled-node root trio without adding fixture, glyph, text, or
  root lookup data. Requirement label measurement now honors final node CSS `font-weight` when
  sizing layout boxes and emitted label `foreignObject` widths, so the direct-style and classDef
  `upstream_cypress_requirementdiagram_unified_spec_example_{012,013,014}` roots derive naturally.
  The Requirement root table is reduced from `10` to `7`, global root inventory is `302`, and text
  lookup remains `484`.
- Re-ran the global generated root override audit after the Requirement cleanup. The current
  `audit-root-overrides --fail-on-stale` report is clean on stale pins across `302` root inventory
  entries, `308` fixture keys, and `308` retained root-delta keys. It reports `295` disabled-root
  DOM mismatches plus the same three accepted Mindmap outside-table residuals, so no additional
  root table entry is currently stale.
- Re-ran the global generated root override audit after the Journey cleanup. The current
  `audit-root-overrides --fail-on-stale` report is clean on stale pins across `305` root inventory
  entries, `311` fixture keys, and `311` retained root-delta keys. It reports `298` disabled-root
  DOM mismatches plus the same three accepted Mindmap outside-table residuals, so no additional
  root table entry is currently stale.
- Derived the final two Journey long-label root viewport pins without adding fixture, glyph, text,
  or root lookup data. Journey actor legend line bounds now use single-run SVG computed text length
  floored to the 1/32px browser lattice, matching the emitted `<text><tspan>line</tspan></text>`
  labels for the two Cypress wrapping fixtures. The Journey generated root override module is
  deleted, `report-overrides --check-no-growth` reports `305` root viewport entries, and text
  lookup remains `484`. Focused disabled-root Journey `parity-root`, full Journey `parity-root`,
  full Journey normal DOM parity, render clippy, formatting, the focused unit test, override
  no-growth, and `git diff --check` all pass.
- Ran closeout verification for the root viewport derivation stage and completed the full strict
  gate under explicit root-parity residual governance. `cargo run -p xtask -- verify
  --clippy --all-features --check-overrides --feature-matrix` passes with fmt, workspace
  all-feature check, workspace clippy, override no-growth, feature matrix, workspace nextest
  (`1081` passed, `3` skipped), and normal SVG DOM parity. A fresh `cargo run -p xtask -- verify
  --strict` now passes too because `compare-all-svgs` accepts exactly five recorded root residuals:
  two Class `different_text_labels_037` fixtures (`2355.75px` upstream max-width versus `2345px`
  local) and the three Mindmap docs/example residuals.
- Cleaned up strict-gate maintenance fallout: workspace clippy lints in `xtask` were reduced
  without behavior changes, and the Mindmap cloud layout unit test plus twelve Mindmap layout
  goldens were synchronized with the current `the root` plain-label metric (`58.375px`) and cloud
  rendered-path bbox. Focused `mindmap_cloud_layout_uses_rendered_path_bbox_dimensions` and layout
  snapshot checks pass.
- Derived the Sequence participant creation/destruction lifecycle-height root without adding
  fixture, glyph, text, or root lookup data. Mermaid advances create/destroy cursor positions by
  half of the actor's pre-render layout height; Rust had been using the later type-specific visual
  actor height, which made the `control` participant case accumulate an `11px` root-height drift.
  `upstream_cypress_sequencediagram_v2_spec_should_render_participant_creation_and_destruction_with_differen_012`
  now matches `1040x580` with root overrides disabled, so its generated Sequence root pin was
  deleted. Current global root viewport inventory is `307` entries, and Sequence generated root
  overrides are `58`.
- Removed the remaining hand-written Mindmap profile calibration branches without adding fixture,
  glyph, or root viewport lookup data. The simple docs/package `Photograph -> Waterfall` profile
  now derives from a Mindmap-owned plain HTML label metric for `Waterfall`; `upstream_root_type_cloud`
  derives from the typed cloud path bounds plus the shared `the root` plain label metric; and the
  docs `Root -> A -> {B, C}` / unclear-indentation profile derives from the `Root` plain label
  metric feeding the deterministic COSE layout.
- Verified the Mindmap profile-calibration closeout with focused `parity-root` / full-DOM checks
  for `upstream_pkgtests_diagram_orchestration_spec_077`,
  `upstream_docs_unclear_indentation`, `upstream_docs_mindmap_unclear_indentation_024`,
  `upstream_docs_mindmap_syntax_003`, `upstream_root_type_*`, `upstream_node_types`, and
  `upstream_pkgtests_mindmap_spec_010`, plus `cargo fmt --all --check`,
  `cargo nextest run -p merman-render mindmap_plain_known_labels_use_browser_html_bbox_widths`,
  `report-overrides --check-no-growth`, and `audit-root-overrides --fail-on-stale`.
- At the Mindmap profile-calibration closeout checkpoint, global root viewport inventory remained
  `308` entries, Mindmap generated root overrides remained `39`, text lookup budget remained
  `484`, SVG text metric table rows remained `186`, and
  `svg/parity/mindmap.rs` has `0` hand-written `parity-root calibration` profile branches.
- Rechecked the Flowchart retained-root audit after the Mindmap closeout. The current retained-root
  triage still reports `49` root pins, `301` label delta rows, no removal candidates, and only
  documented deferral buckets: `defer-low-noise-text-lattice` (16),
  `defer-subpixel-text-lattice` (2), `defer-mojibake-font-fallback` (1),
  `defer-courier-font` (8), `defer-icon-font` (19), and `defer-font-env` (3). No clean shared
  text/layout rule appeared, so the remaining Flowchart pins stay retained without adding fixture,
  glyph, or root viewport lookup data.
- Derived the GitGraph `BT` + `parallelCommits` compact axis without adding fixture, glyph, or root
  lookup data. Bottom-to-top parallel graphs now place commits with the same compact parent-axis
  spacing as the top-to-bottom pass and mirror the axis after placement, reducing the natural root
  height for
  `upstream_cypress_gitgraph_spec_71_should_render_gitgraph_with_parallel_commits_vertical_branch_075`
  from `329px` to the upstream `239px`. The generated root pin stays retained because exact width
  parity still depends on the existing `-0.016px` vertical branch-label bbox lattice residual, so
  GitGraph remains at `23` root entries.
- Rechecked the remaining GitGraph retained roots and kept them retained. A fresh disabled-root
  sweep reports `23` generated high-precision root-delta keys and `15` 3-decimal `parity-root` DOM
  mismatches; the other `8` generated keys still differ in exact root attrs but normalize under the
  snapped root DOM lattice. Representative SVGs show mixed-sign 1/64px branch/commit label drift
  plus title/root f32 and tag-height residuals. A shared 10px commit/tag bbox-height probe improved
  `upstream_merges_spec` but caused outside-table height mismatches, so no clean shared rule was
  kept and no fixture/glyph/root lookup data was added.
- Rechecked the then-current Sequence retained note/message/frame root bucket and kept all `59`
  generated Sequence root pins. The disabled-root sweep in
  `target/compare/sequence_disabled_root_current.md` maps all `59` generated keys to
  `parity-root` DOM mismatches with `0` stale entries. The retained rows mix `48` positive width
  drifts, `4` negative width drifts, `7` width-zero height-only drifts, and `11` rows with height
  drift, spanning message/note escaping and line-break text measurement, nested frame/rect
  vertical geometry, participant type/lifecycle geometry, and mixed-sign loop/create-destroy cases.
  No broad shared message, note, or frame slack rule was kept, and no fixture/glyph/root lookup
  data was added. This supersedes the earlier TODO item that waited on broad message-width
  inference before revisiting the bucket. Follow-up ledger verification found no remaining
  unchecked workstream TODO items and passed `git diff --check`, `cargo fmt --all --check`, and
  `cargo run -p xtask -- report-overrides --check-no-growth`. The narrower text escaping /
  line-break subfamily also stays retained: a focused disabled-root slice over
  `upstream_cypress_sequencediagram_spec_should_handle_different_line_breaks_004`,
  `stress_message_text_with_colons_039`,
  `upstream_cypress_sequencediagram_spec_should_handle_line_breaks_and_wrap_annotations_006`,
  `stress_html_entities_and_escaping_038`,
  `upstream_cypress_sequencediagram_v2_spec_should_render_with_wrapped_messages_and_notes_011`,
  `stress_sequence_batch5_whitespace_semicolons_051`, and
  `upstream_docs_sequence_note_with_br` showed `6` positive width drifts, `0` negative width
  drifts, `0` height changes, and one exact match.
- Rechecked the narrower Sequence typed participant width/spacing subfamily and kept the four
  generated root pins retained. With root overrides disabled, the typed Cypress roots still drift
  right by `+12`, `+35`, and `+14` pixels, while the adjacent quoted/typed stress fixture drifts
  left by `-7` pixels. Element probes split the drift across actor columns, message centers, and
  note widths (`150 -> 160` in the Cypress notes/loops fixture, but `678 -> 671` in the quoted
  stress fixture), so no shared actor visual-width, spacing, or emitted-bounds rule was kept.
- Ran the global root override audit with `audit-root-overrides --fail-on-stale`. At that
  checkpoint the report stayed clean on stale pins across the full `307`-entry root viewport
  inventory, and the only remaining outside-table DOM mismatches were the three accepted Mindmap
  docs/example residuals.
- Rechecked the narrower Sequence nested frame / rect vertical geometry subfamily and kept those
  pins retained. Focused disabled-root `parity-root` checks for
  `stress_deep_nested_frames_018`, `stress_nested_frames_001`, and
  `stress_nested_rect_par_029` still fail only on root height (`+16`, `+16`, and `+30`). Element
  probes show mixed mechanisms instead of one shared boundary rule: the deep nested frame footer
  is lower locally while internal loop/message/activation maxima are higher upstream; the nested
  frames fixture shifts footer/message/frame coordinates without a uniform activation shift; and
  the rect/par fixture moves message/activation/footer down while loop and note bounds stay fixed.
  No fixture, glyph, text, or root lookup data was added.
- Rechecked the current State retained-root bucket and kept all `33` generated State root pins.
  The disabled-root sweep in `target/compare/state_disabled_root_current.md` maps all `33`
  generated keys to exact root-delta rows: `20` positive width drifts, `13` negative width drifts,
  `5` rows with height drift, `32` snapped `parity-root` DOM mismatches, and one exact-only root
  guard (`stress_state_unicode_quotes_and_br_in_notes_048`). The retained rows span
  HTML-sanitized noteGroup bounds, right-to-left scaled long-id roots, edge-label wrapping/Dagre
  placement, font-size and style precedence, and small browser-float guards. No broad shared
  State slack or measurement rule was kept, and no fixture/glyph/root lookup data was added.

## 2026-05-16

- Added `xtask audit-root-overrides`, a global generated-table governance audit that expands root
  override fixture keys by diagram family, runs child compare commands with
  `MERMAN_DISABLE_ROOT_VIEWPORT_OVERRIDES=1`, and classifies retained/stale keys by exact
  upstream/local root `viewBox` and `max-width` comparison. The first full run found two stale
  generated pins: `upstream_docs_entityrelationshipdiagram_unicode_text_007` and
  `stress_state_unicode_and_rtl_036`. Focused disabled-root and normal `parity-root` checks passed
  for both, so the pins were deleted and the root override no-growth budget was tightened to
  `308`. The post-delete audit reports `stale=0` across `308` inventory entries / `314`
  fixture keys. It also exposes 12 outside-table normal `parity-root` failures for later
  derivation work: seven Flowchart new-shape set4 height roots, two GitGraph continuous
  development max-width roots, and three Mindmap icon/tidy-tree roots.
- Extended Flowchart retained-root label audit to include `htmlLabels:false` SVG `<text>/<tspan>`
  labels by reporting emitted label-container geometry instead of estimating text width from
  strings. The former non-deferred
  `upstream_cypress_oldshapes_spec_shapessets_shapesset5_tb_md_html_false_038` case now reports
  four SVG Markdown text/container label deltas (`-0.023px`, `-0.023px`, `-0.008px`, `-0.008px`)
  that explain the `-0.060px` root drift, so triage keeps the root pin in
  `defer-subpixel-text-lattice` without adding fixture/glyph lookup data. Full retained-root
  triage now reports `49` root pins, `301` label delta rows, no removal candidates,
  `defer-low-noise-text-lattice` (16), `defer-subpixel-text-lattice` (2),
  `defer-mojibake-font-fallback` (1), `defer-courier-font` (8), `defer-icon-font` (19), and
  `defer-font-env` (3); there are no remaining non-deferred Flowchart buckets.
- Deleted the now-derived Flowchart `newshapesset3_lr_allpairs_067` root pin after a focused
  disabled-root `parity-root` check matched without the generated override. Flowchart root
  overrides are now `43`, total root overrides are now `310`, and the full retained-root audit now
  reports `49` root pins, `297` label delta rows, `defer-low-noise-text-lattice` (16), and
  `layout-shape-geometry` (1).
- Derived the retained Flowchart crossed-circle alias root pin and cleaned up the stacked-rectangle
  shape geometry path without adding fixture/glyph lookup data. `multiRect.ts` treats the Dagre
  `node.width/height` as the final outer bbox, so the renderer now subtracts the 5px stacked offset
  before building the inner rectangle and shifts labels by `(-5,+5)`. The crossed-circle root
  estimator now applies the RoughJS circle bbox asymmetry to `cross-circ`, `summary`, and
  `crossed-circle`, so focused disabled-root `parity-root` has no retained delta for
  `upstream_cypress_flowchart_shape_alias_spec_shape_alias_aliasset37_037` and that root pin was
  deleted. Full retained-root triage now reports `50` root pins, `297` label delta rows,
  `defer-low-noise-text-lattice` (16), and `layout-shape-geometry` (2).
- Derived the retained Flowchart `root-only-layout` outgoing-links-4 pair by including empty
  subgraph-as-node rectangles in the computed root viewBox bounds. Focused disabled-root
  `parity-root` checks now match the `154.921875x364` upstream viewBox for
  `upstream_cypress_flowchart_v2_spec_57_handle_nested_subgraphs_with_outgoing_links_4_{015,016}`,
  so both root pins were deleted. Full retained-root triage now reports `51` root pins and no
  `root-only-layout` bucket; the only remaining non-deferred bucket is
  `layout-shape-geometry` (9).
- Derived the retained Flowchart shared multiline HTML text bucket without adding fixture/glyph
  lookup data. The vendored font measurer now keeps tiny same-glyph DOM-lattice residuals from
  generated two-character samples from accumulating across every overlapping pair in long repeated
  runs. The shared three-line HTML label in
  `upstream_html_demos_flowchart_{flowchart_004,flowchart_046,graph_003}` now measures
  `168.0x72.0`, focused disabled-root `parity-root` checks pass, and those three Flowchart root
  pins were deleted. Full retained-root triage now reports `53` root pins and no
  `shared-multiline-text` bucket.
- Reclassified the retained Flowchart `low-noise-text` bucket as
  `defer-low-noise-text-lattice`. Browser probes match upstream for the sampled labels, but the
  vendored model has mixed-sign 1/64px lattice drift; widening the tiny-pair rule would regress
  other labels, so the ten roots stay pinned without adding lookup data.
- Reclassified the retained Flowchart `newshapesset5_lr_md_html_false` residual into a new
  `defer-subpixel-text-lattice` triage bucket. The rule is deliberately narrow: root max-width
  and full viewBox width/height drift must be below `1/64px`, the boundary contributor must match,
  and there must be no paired label delta rows. This keeps the `-0.008px` SVG Markdown/font
  lattice residual pinned without adding glyph data, while height-only nested-subgraph root drift
  stays in `root-only-layout`.
- Reclassified the retained Flowchart `upstream_docs_diagrams_flowchart_code_flow` root pin as
  mixed-sign default-font accumulation drift. Its boundary label already matches upstream, and a
  shared `break-spaces` min-content experiment improves the long function signatures while making
  the overall root viewport worse, so the residual is not a clean text-rule candidate. The triage
  report now moves this case to `defer-font-env`, keeps the root pin, and leaves the next
  non-deferred Flowchart target on `newshapes set5` shape geometry.
- Reclassified the retained Flowchart `fhd12` mojibake root pin as browser/font fallback drift
  rather than an ordinary shared multiline text candidate. The focused audit still has real root
  drift, but C1-control-byte fallback tuning cannot explain the same fixture consistently without
  becoming glyph/fixture data. `xtask triage-flowchart-root-pins` now emits a
  `defer-mojibake-font-fallback` bucket, leaves the root pin in place, and keeps the next clean
  Flowchart target focused on `layout-text-accumulation`.

## 2026-05-15

- Tightened the Flowchart FontAwesome HTML-label width root family without using a fixture-derived
  glyph table. The model intentionally keeps a clean nominal `1em` inline width for standard
  FontAwesome `<i>` runs, treats the documented unregistered custom-pack example as an empty
  inline element, measures non-markdown icon labels through the same HTML fragment path used for
  emitted `<foreignObject>` content, and preserves whitespace adjacent to inline icon runs.
  Focused normal `parity-root` and `report-overrides --check-no-growth` pass after deleting the
  now-derived `stress_flowchart_icons_unicode_and_wrap_056` root pin. Root entries are now `350`,
  with Flowchart at `83`. The remaining icon pins stay retained because deriving them would
  require real FontAwesome per-icon advance widths, which is outside the clean parity boundary for
  now.

## 2026-05-14

- Derived the Flowchart FontAwesome icon-only multiline label height root: HTML labels that render
  `<i class="fa ..."></i><br/>...` now keep the icon-only line as a normal `1.5em` DOM line box
  during measurement. `stress_flowchart_icons_multiline_br_054` now derives the upstream
  `145.5 x 374` root without a pin; focused disabled-root and normal `parity-root`, full
  Flowchart `parity-root`, render nextest, render clippy, and
  `report-overrides --check-no-growth` pass. Root entries are now `351`, with Flowchart at `84`.
  The remaining icon retained pins were rechecked and still guard real max-width drift such as
  `438.75px` versus `439.5px`, `130.75px` versus `127.75px`, and `92px` versus `94px`.
- Matched Mermaid's split Flowchart htmlLabels semantics for the chained-statement height family:
  node labels use the root `htmlLabels` toggle, while edge labels, subgraph titles, Flowchart CSS
  selectors, and styled/quoted-string node-height parity follow `flowchart.htmlLabels` with root
  fallback. `upstream_cypress_flowchart_spec_20_multiple_nodes_and_chaining_in_one_statement_020`
  now derives the upstream `234.015625 x 300` root without a pin; focused disabled-root and normal
  `parity-root`, full Flowchart `parity-root`, render nextest, render clippy, and
  `report-overrides --check-no-growth` pass. Root entries are now `352`, with Flowchart at `85`.
  The sibling `upstream_flow_vertice_chaining_amp_to_single_spec` remains pinned because
  disabled-root parity still has upstream `312.5px` versus local `312.75px` max-width drift.
- Centralized render numeric config parsing so quoted YAML numbers and CSS `px` config values are
  handled by shared helpers rather than per-diagram copies. A full disabled-root cross-check after
  the migration found all generated root viewport pins still map to DOM mismatches (`stale=0`), so
  no root budget change was made in this pass.
- Parsed plain numeric string Flowchart spacing config as numbers for layout and SVG parity config.
  `flowchart.rankSpacing: '100'` now feeds Dagre as `100.0`, so
  `upstream_cypress_flowchart_spec_23_render_a_simple_flowchart_with_rankspacing_set_to_100_023`
  derives without a root viewport pin. The layout golden was refreshed, focused disabled-root and
  normal `parity-root` checks pass for the fixture, and `report-overrides` now reports `353` root
  entries with Flowchart at `86`.
- Collapsed exact-duplicate Flowchart root override match arms into Rust or-patterns. This is a
  table-only cleanup: the same fixture stems still map to the same `(viewBox, max-width)` tuples,
  but `report-overrides` inventory drops from `362` to `354` root entries, with Flowchart at `87`.
- Removed five stale Sequence root pins after a disabled-root mismatch cross-check found
  `root=64 mismatch=59 stale=5 missing=0`. Focused disabled-root `parity-root` passes for the
  five removed simple-root fixtures, tightening the root no-growth budget to `362` with Sequence
  at `59`.
- Corrected Sequence default text-width facts for `Feeling fresh like a daisy`,
  `Fine, thank you. And you?`, `Hello Charley, how are you?`, and
  `Did you want to go to the game tonight?` from upstream SVG actor/frame spacing, then deleted
  six now-derived docs/control root viewport pins. Focused disabled-root `parity-root` passes for
  the removed fixtures, tightening the root no-growth budget to `367` with Sequence at `64`.
  The participant-creation v2 sibling remains pinned because its disabled-root drift is root
  height (`1040x580` upstream versus `1040x591` local), pointing to participant type/lifecycle
  vertical geometry rather than text width.
- Corrected Sequence default text-width facts for `Hello Bob, how are - you?` and
  `Alice-in-Wonderland` from upstream package sequence actor spacing, then deleted the now-derived
  `upstream_pkgtests_sequencediagram_spec_014`, `015`, `026`, and `027` root viewport pins.
  Focused disabled-root `parity-root` passes for all four fixtures, tightening the root no-growth
  budget to `373` with Sequence at `70`.
- Corrected the Sequence default `How about you John?` message-width fact from the upstream
  simple sequence actor spacing, then deleted the now-derived Cypress simple sequence root
  viewport pin. Focused disabled-root `parity-root` passes for the fixture, tightening the root
  no-growth budget to `377` with Sequence at `74`.
- Corrected the Sequence default `bidirectional_dotted` message-width fact from the upstream
  `arrows_variants` actor spacing, then deleted the now-derived root viewport pin. Focused
  disabled-root `parity-root` passes for the fixture, tightening the root no-growth budget to
  `378` with Sequence at `75`.
- Corrected the Sequence default `Hello Alice, please meet Carol?` message-width fact from the
  upstream stacked-activation actor spacing, then deleted the now-derived
  `activation_stacked` and `upstream_pkgtests_sequencediagram_spec_040` root viewport pins.
  Focused disabled-root `parity-root` checks pass for both fixtures, tightening the root
  no-growth budget to `379` with Sequence at `76`.
- Corrected the Sequence default `Hello Alice, I'm fine and you?` message-width fact from the
  upstream `activation_explicit` actor spacing, then deleted the now-derived root viewport pin.
  Focused normal and disabled-root `parity-root` checks pass for the fixture, tightening the root
  no-growth budget to `381` with Sequence at `78`.
- Honored Mermaid's GitGraph commit/tag label theme variables in emitted CSS and root measurement:
  commit labels now use their own font-size/color/background variables, tag labels use their own
  font-size/color/background/border variables, and root bounds measure commit and tag labels with
  separate styles. Focused disabled-root checks for the commit/tag font-size docs fixtures pass,
  deleting `upstream_docs_gitgraph_customizing_commit_label_font_size_032` and tightening the root
  no-growth budget to `382` with GitGraph at `23`.
- Derived vertical GitGraph branch-label roots from Mermaid's `drawText(name).getBBox()` behavior:
  TB/BT branch labels now use the centered SVG bbox path with 1/64px ties-to-even quantization,
  while LR/RL keeps the computed-length branch-label rule. A disabled-root audit over the
  65-entry GitGraph table found 24 retained DOM mismatches and 41 stale pins; deleting the stale
  pins tightens the root no-growth budget to `383` with GitGraph at `24`.
- Matched Flowchart `fork/join` layout sizing to Mermaid's `forkJoin.ts` direction rule:
  LR-rendered graphs use the vertical `10x70` bar before the `state.padding / 2` Dagre inflation,
  while other directions keep the horizontal `70x10` bar. This removes the 60px LR old-shape
  offset in the set3 fixtures, refreshes the affected layout goldens, and deletes five
  now-derived Flowchart root pins. A follow-up disabled-root cross-check showed the classdef,
  `md_html_false`, and styles siblings were also stale under the same typed rule; deleting those
  three more pins tightens the root no-growth budget to `424` with Flowchart at `95`.
- Derived GitGraph commit and tag label root bounds from GitGraph-owned computed text lengths with
  the same 1/64px quantization used by the upstream SVG text advance path. This avoids routing
  GitGraph short labels through the shared simple bbox path and its Sequence-specific browser facts.
  A disabled-root audit of the pre-change 130-entry GitGraph table found 65 retained DOM
  mismatches and 65 stale pins; deleting the stale half leaves GitGraph root pins at `65` and
  tightens the root no-growth budget to `432`.

## 2026-05-13

- Matched GitGraph seeded auto commit ids to upstream's committed SVG generation pipeline:
  Mermaid consumes the seeded `Math.random()` stream once during `mermaid.parse(code)` before the
  later render pass, so Rust now replays that warm-up parse before building the render model.
  The corrected ids made a disabled-root cross-check expose 27 stale retained root pins; after
  keeping `upstream_direction_bt` because it still guards BT-direction branch/commit-label bbox
  drift, the pass removed 26 net GitGraph root pins. GitGraph root pins are now `130`, and the
  root no-growth budget is `497`.
- Modeled Mermaid's unregistered custom FontAwesome fallback for Flowchart HTML labels:
  `fab:fa-truck-bold` is emitted as an empty `<i class="fab fa-truck-bold">` fallback rather than
  a registered custom SVG icon. The Flowchart HTML label measurer now applies the upstream
  Chromium inline advance for that fallback, so `upstream_docs_flowchart_custom_icons_238` and
  `stress_flowchart_icons_prefixes_and_quotes_052` pass focused disabled-root `parity-root`.
  Flowchart root pins are now `103`, and the root no-growth budget is `523`.
- Derived the Flowchart `iconSquare` root by matching Mermaid's icon-shape outer layout bounds:
  `iconSquare.ts` sizes the icon box as `iconSize + halfPadding * 2`, so Flowchart layout now uses
  `iconSize + node.padding` before Dagre/root bounds. Refreshed
  `upstream_docs_flowchart_icon_shape_132.layout.golden.json`, deleted its root pin, and tightened
  Flowchart root pins to `105` with root no-growth budget `525`.
- Derived the Flowchart font-size precedence root by separating SVG root CSS font size from HTML
  `foreignObject` label measurement: numeric `themeVariables.fontSize` stays a root CSS value, but
  HTML labels measure at 16px unless the theme value is a valid `"NNpx"` CSS string or an explicit
  class/inline font-size applies. `stress_flowchart_font_size_precedence_073` now passes focused
  disabled-root `parity-root`; Flowchart root pins are now `106`, and the root no-growth budget is
  `526`.
- Removed two stale Flowchart subgraph title-margin root viewport pins
  (`upstream_cypress_flowchart_v2_spec_should_render_subgraphs_with_title_margins_set_lr_and_htmllabels_062`
  and `upstream_flowchart_v2_subgraph_title_margins_lr_htmlLabels_false_spec`) after focused
  disabled-root `parity-root` checks showed both roots now derive without the lookup. Flowchart
  root pins are now `107`, and the root no-growth budget is `527`.
- Preserved bare `<`/`>` text while extracting Flowchart HTML labels and added a narrow
  default-stack CJK width cushion for single-line Flowchart HTML labels that contain literal
  comparison symbols. `stress_flowchart_subgraph_title_unicode_and_entities_043` now passes
  focused disabled-root `parity-root`, so its Flowchart root pin was deleted; Flowchart root pins
  are now `109`, and the root no-growth budget is `529`.
- Derived the Flowchart SVG-like long-word subgraph-title root by sharing the emitted SVG text
  wrapping helper with layout and sizing default process nodes from wrapped computed text length.
  `upstream_flowchart_v2_stage2_subgraph_title_wraps_long_word_svglike_spec` now passes focused
  disabled-root `parity-root`, so its Flowchart root pin was deleted; Flowchart root pins are now
  `110`, and the root no-growth budget is `530`.
- Modeled C1 control bytes in mojibake Flowchart HTML labels as Chromium near-full-em replacement
  glyphs. The courier long-name/class-definition Cypress fixture now passes focused
  `parity-root` with root overrides disabled, so its root pin was deleted; Flowchart root pins are
  now `111`, and the root no-growth budget is `531`.
- Derived Flowchart anchor node layout bounds from the seeded 2px roughjs dot instead of the
  ignored label text. The old-shape set5 cluster now derives 12 previously pinned roots; the
  remaining `upstream_cypress_oldshapes_spec_shapessets_shapesset5_tb_md_html_false_038` pin still
  guards a real 0.06px root drift. Flowchart root pins are now `112`, and the root no-growth
  budget is `532`.
- Derived Flowchart imageSquare root bounds from layout-time image plus label extents instead of
  sizing the Dagre node as only the image asset. `upstream_docs_flowchart_parameters_136` now
  passes focused `parity-root` with root overrides disabled, so its Flowchart root pin was deleted;
  Flowchart root pins are now `124` and the root no-growth budget is `544`.
- Switched horizontal GitGraph branch-label layout to computed-length widths while retaining the
  wider bbox path for TB/BT roots where rotated dynamic commit labels dominate. The full
  disabled-root GitGraph cross-check exposed 57 now-derived root pins
  (`override=213 mismatch=156 stale=57 missing=0`); deleting them leaves
  `override=156 mismatch=156 stale=0 missing=0`, GitGraph at `156` root pins, and the root
  no-growth budget tightened to `545`.
- Included GitGraph branch line endpoints in GitGraph-owned emitted root bbox derivation so
  zero-length branch lines affect raw root viewports like browser `getBBox()` does. The focused
  empty-graph package bucket (`upstream_pkgtests_diagram_orchestration_spec_048`,
  `upstream_pkgtests_gitgraph_spec_076`, and `upstream_pkgtests_gitgraph_test_011` through `_013`)
  dropped from roughly `+34.750px` disabled-root width drift to residual
  `+0.250px`/`+0.266px` branch-label bbox drift. No root pin was deleted because the full
  disabled-root cross-check still found `override=213 mismatch=213 stale=0 missing=0`; full
  GitGraph `parity-root`, override no-growth, render/xtask clippy, render nextest, and
  `verify --strict` passed.
- Aligned GitGraph font-size precedence with upstream behavior: `themeVariables.fontSize` now drives
  GitGraph layout measurement and base SVG CSS, while top-level `fontSize` is ignored for this
  diagram and top-level `fontFamily` remains honored. The focused disabled-root
  `stress_gitgraph_font_size_097` root-width drift shrank from roughly `+75.828px` to `+0.156px`;
  `stress_gitgraph_font_size_precedence_098` still has a `+0.438px` branch-label bbox drift, so no
  font-size stress root pin was deleted in this pass.
- Fixed GitGraph `parallelCommits` layout for unconnected LR branch roots by restarting the commit
  axis for parentless commits, matching Mermaid's independent branch timelines. The focused
  disabled-root probe for
  `upstream_cypress_gitgraph_spec_45_should_render_gitgraph_with_unconnected_branches_and_parallel_048`
  shrank from a `+150.250px` root-width drift to the existing `+0.250px` branch-label bbox
  measurement drift; no root pin was deleted in this pass.
- Derived GitGraph title-dominated root viewports from emitted title text bounds, keeping the
  title anchor tied to the pre-title content bbox center like Mermaid `insertTitle(...)`; removed
  13 now-derived GitGraph root pins and tightened the root no-growth budget to `603` with GitGraph
  at `213`.
- Earlier in the GitGraph cleanup, removed two then-stale GitGraph root viewport pins
  (`upstream_cypress_gitgraph_spec_88_should_hide_branches_with_tb_orientation_when_showbranches_is_092`
  and `upstream_direction_bt`) after disabled-root mismatch cross-checking showed both now pass
  focused `parity-root` without the lookup; full GitGraph `parity-root`,
  `report-overrides --check-no-growth`, render/xtask clippy, and xtask override budget tests
  stayed green, and the root no-growth budget was tightened to `616` with GitGraph at `226`.
  The later seeded auto-id warm-up pass restored `upstream_direction_bt` because the corrected
  dynamic commit id exposed a real BT-direction bbox guard.

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
