# Changelog

All notable changes to this project will be documented in this file.

The format is based on *Keep a Changelog*, and this project adheres to *Semantic Versioning*.

## [Unreleased]

### Changed

- Root override governance: add a global disabled-root audit across all generated root viewport
  tables, delete two stale ER/State root pins proven by focused `parity-root` checks, and tighten
  the root override no-growth budget to `308`. The audit now reports no stale generated root pins
  and surfaces the remaining outside-table root-only candidates separately.
- Flowchart root audit: include `htmlLabels:false` SVG `<text>/<tspan>` labels in label-level
  retained-root reports by pairing emitted label-container geometry, so the remaining
  `oldshapes set5` pin is classified as SVG Markdown text lattice drift instead of shape
  geometry.
- Flowchart parity: render stacked-rectangle aliases from the layout bbox without double-applying
  the 5px stack offset, shift their labels with Mermaid's `multiRect.ts` offset, and apply the
  crossed-circle RoughJS root-bbox asymmetry to all aliases. This removes the retained
  `shape_alias_aliasset37_037` root viewport pin and narrows the remaining Flowchart
  `layout-shape-geometry` bucket.
- Flowchart root audit: remove the now-derived `newshapesset3_lr_allpairs_067` root viewport pin
  after focused disabled-root `parity-root` matched without the generated override, tightening the
  inventory to `310` total root overrides and `43` Flowchart root overrides.
- Flowchart parity: include empty subgraph-as-node rectangles in computed root viewBox bounds for
  nested-root diagrams, removing the two retained outgoing-links-4 root viewport pins without
  adding fixture or glyph lookup data.

### Added

- `xtask audit-root-overrides`: cross-check generated root viewport tables by diagram family
  against disabled-root `parity-root` outputs, using exact SVG root attr comparison for retained
  keys and reporting stale plus outside-table DOM candidates.
- `xtask triage-flowchart-root-pins`: classify accumulated subpixel SVG text/container drift as
  `defer-subpixel-text-lattice` when small SVG text label deltas explain the retained root
  boundary without adding fixture or glyph lookup data.
- `xtask triage-flowchart-root-pins`: add a narrow `defer-subpixel-text-lattice` bucket for
  retained Flowchart root pins whose max-width and full viewBox drift are both below `1/64px`,
  boundary contributors match, and no paired label delta rows exist.
- `xtask triage-flowchart-root-pins`: reclassify subpixel mixed-sign plain/default-stack text
  drift as `defer-low-noise-text-lattice` once browser probes match upstream and no clean shared
  metric rule explains the vendored 1/64px lattice residual.
- Sequence parity: add KaTeX/math measurement and SVG rendering for actors, messages, notes, and
  box/block labels, and wire the Sequence SVG comparer to the Node KaTeX backend.
- Docs: add the `docs/workstreams/fearless-refactor/` workstream with roadmap, TODOs, milestones,
  and render-model inventory for the next cleanup-focused release.
- Docs: add a generated parity override footprint snapshot for fearless-refactor governance.
- Docs: add an override policy for text/render width compatibility data.
- Docs: add a post-migration sequence typed render-model performance spot-check.
- Docs: add a current-main Criterion performance baseline covering pipeline and targeted
  flowchart/architecture/mindmap stress benches.
- Docs: add a gantt JSON baseline and typed render-model performance spot-check.
- Docs: add a pie typed render-model performance spot-check.
- Docs: add a packet typed render-model performance spot-check.
- Docs: add a timeline typed render-model performance spot-check.
- Docs: add a journey typed render-model performance spot-check.
- Docs: add a requirement typed render-model performance spot-check.
- Docs: add a sankey typed render-model performance spot-check.
- Docs: add a radar typed render-model performance spot-check.
- Docs: add an info typed render-model performance spot-check.
- Docs: add a zenuml typed render-model performance spot-check.
- Docs: add a quadrant chart typed render-model performance spot-check.
- Docs: add a gitGraph typed render-model performance spot-check.
- Docs: add a treemap typed render-model performance spot-check.
- Docs: add a block typed render-model performance spot-check.
- Docs: add an ER typed render-model performance spot-check.
- Docs: add a JSON clone audit for layout/render API ownership boundaries.
- Docs: add a public render API and CLI cleanup decision note for the fearless-refactor workstream.
- Docs: add a lint-allow audit for remaining source-level allowances and removal criteria.
- Docs: add a post-Class-cleanup full bench gate record for the release readiness pass.
- Docs: add a post-Flowchart-override-inventory full bench gate record for the release readiness
  pass.
- Docs: record the Flowchart disabled-root audit that classifies the remaining root viewport
  guards and confirms no retained Flowchart root pin is currently stale.
- `xtask verify --strict`: add a strict refactor/release gate that includes `cargo fmt`,
  `cargo check --workspace --all-features`, workspace all-target/all-features Clippy with
  `-D warnings`, `cargo nextest run`, and SVG DOM parity checks.
- `xtask verify --root-parity`: add an opt-in full SVG root parity gate; `--strict` now includes
  it after normal DOM parity.
- `xtask report-overrides --check-no-growth`: reject root viewport lookup call sites that bypass
  the shared root override helper.
- `xtask compare-all-svgs --report-root`: forward root-delta reporting to Sequence in addition to
  Flowchart.
- Root viewport audits: add `MERMAN_DISABLE_ROOT_VIEWPORT_OVERRIDES=1` as a local-only switch for
  proving whether generated root pins still guard real `parity-root` drift.
- `xtask compare-gitgraph-svgs --report-root`: add root viewport delta reporting for GitGraph and
  include it in `compare-all-svgs --report-root`.
- `xtask compare-*-svgs --report-root-all`: add full root delta report output, with
  `--report-root-limit <n>` for capped audit tables.
- `xtask compare-mindmap-svgs --report-root` and `compare-state-svgs --report-root`: add root
  viewport delta reporting and include both in `compare-all-svgs --report-root`.
- Docs: close the fearless-refactor M5 obsolete-override cleanup item for the current release after
  documenting the remaining override buckets as derivation or measurement targets.
- `xtask` root delta reports: reuse DOM comparison XML normalization so browser-valid
  `<foreignObject>` fragments do not make root reporting fail strict verification.
- State root viewport derivation: limit the 72px border-label height inflation rule to
  classDef-compiled border styles, letting direct `style ... border:...` nodes derive their root
  viewport without a fixture-scoped pin.

### Fixed

- Flowchart HTML text metrics: prevent tiny same-glyph DOM-lattice residuals from generated
  two-character font samples from accumulating across every overlapping pair in long repeated
  runs. This derives the shared multiline HTML label used by
  `upstream_html_demos_flowchart_{flowchart_004,flowchart_046,graph_003}` and removes three
  Flowchart root viewport pins without adding fixture or glyph lookup data.

### Changed

- Flowchart parity: render `lined-document` from its label-box path and derive curly
  brace/comment root bounds from the same label-box geometry used by the SVG emitter. The focused
  disabled-root `newshapesset5_lr_md_html_false` shape-family drift drops from `+2.913px` to
  `-0.008px`; the root pin remains because the residual is below a clean geometry rule and still
  misses the exact deletion gate.
- Flowchart parity: keep FontAwesome HTML icon-only lines as normal `1.5em` line boxes during
  label measurement. This derives
  `stress_flowchart_icons_multiline_br_054` without a root viewport pin, tightening Flowchart root
  overrides to 84 entries and total root overrides to 351 while leaving the remaining icon pins as
  real max-width drift guards.
- Flowchart parity: split node-label `htmlLabels` semantics from `flowchart.htmlLabels` semantics
  for edge labels, subgraph titles, generated CSS, and styled/quoted-string height parity. This
  derives the chained-statement fixture
  `upstream_cypress_flowchart_spec_20_multiple_nodes_and_chaining_in_one_statement_020` without a
  root viewport pin, tightening Flowchart root overrides to 85 entries and total root overrides to
  352.
- Render config parsing: centralize numeric and CSS `px` config parsing in `merman-render` so
  layout and SVG parity paths share finite-number, quoted YAML number, and CSS pixel semantics
  instead of carrying diagram-local `json_f64` copies. A disabled-root cross-check found no newly
  stale root viewport pins; the root viewport no-growth budget remains `353`.
- Flowchart root derivation: parse plain numeric string config values such as
  `flowchart.rankSpacing: '100'` as numbers for layout and SVG parity config. This derives
  `upstream_cypress_flowchart_spec_23_render_a_simple_flowchart_with_rankspacing_set_to_100_023`
  without a root viewport pin, refreshes its layout golden, and tightens the root viewport
  no-growth budget to `353` with Flowchart at `86`.
- Flowchart root override inventory: collapse exact-duplicate generated match arms into Rust
  or-patterns, preserving fixture-key coverage while tightening the root viewport no-growth budget
  to `354` with Flowchart at `87`.
- Sequence root derivation: remove five stale simple-root pins after a disabled-root mismatch
  cross-check showed they are now covered by existing root derivation. Sequence root pins drop to
  `59`, and the root viewport no-growth budget tightens to `362`.
- Sequence root derivation: correct Mermaid `calculateTextDimensions` width facts for
  `Feeling fresh like a daisy`, `Fine, thank you. And you?`, `Hello Charley, how are you?`, and
  `Did you want to go to the game tonight?`, so the docs/control Sequence fixtures derive actor,
  frame, and root bounds without fixture-scoped pins. Six more Sequence root pins are deleted;
  Sequence root pins drop to `64`, and the root viewport no-growth budget tightens to `367`.
- Sequence root derivation: correct Mermaid `calculateTextDimensions` width facts for
  `Hello Bob, how are - you?` and `Alice-in-Wonderland`, so package sequence fixtures `014`,
  `015`, `026`, and `027` derive actor spacing and root viewport without fixture-scoped pins.
  Sequence root pins drop to `70`, and the root viewport no-growth budget tightens to `373`.
- Sequence root derivation: correct the Mermaid `calculateTextDimensions` width fact for
  `How about you John?`, so the simple upstream Cypress sequence fixture derives John actor
  spacing and its 790px root viewport without a fixture-scoped pin. Sequence root pins drop to
  `74`, and the root viewport no-growth budget tightens to `377`.
- Sequence root derivation: correct the Mermaid `calculateTextDimensions` width fact for
  `bidirectional_dotted`, so `arrows_variants` keeps the upstream 50px actor margin and derives
  its 450px root viewport without a fixture-scoped pin. Sequence root pins drop to `75`, and the
  root viewport no-growth budget tightens to `378`.
- Sequence root derivation: correct the Mermaid `calculateTextDimensions` width fact for
  `Hello Alice, please meet Carol?`, so the stacked-activation fixtures now derive their actor
  spacing and root viewport without fixture-scoped pins. Sequence root pins drop to `76`, and the
  root viewport no-growth budget tightens to `379`.
- Sequence root derivation: correct the Mermaid `calculateTextDimensions` width fact for
  `Hello Alice, I'm fine and you?`, so `activation_explicit` now derives its actor spacing and root
  viewport without a fixture-scoped pin. Sequence root pins drop to `78`, and the root viewport
  no-growth budget tightens to `381`.
- GitGraph theme parity: honor Mermaid's commit/tag label theme variables for emitted CSS and root
  measurement, including `commitLabelFontSize`, `tagLabelFontSize`, commit/tag label colors,
  backgrounds, and tag borders. The focused disabled-root checks for the commit/tag font-size docs
  fixtures stay green, `upstream_docs_gitgraph_customizing_commit_label_font_size_032` no longer
  needs a root viewport pin, and the root viewport no-growth budget tightens to `382` with
  GitGraph at `23`.
- GitGraph root derivation: measure vertical branch-label backgrounds from the centered SVG bbox
  path with 1/64px ties-to-even quantization, matching Mermaid's
  `drawText(name).getBBox()` branch-label placement. A disabled-root audit showed 24 of the
  previous 65 GitGraph root pins still guard real drift, so 41 stale pins were deleted and the root
  viewport no-growth budget tightened to `383` with GitGraph at `24`.
- Flowchart fork/join layout: match Mermaid's `forkJoin.ts` direction-sensitive sizing by using a
  vertical fork/join bar only in LR-rendered graphs while preserving the state-padding inflation.
  This removes the 60px LR old-shape layout drift, refreshes the affected layout goldens, deletes
  8 now-derived Flowchart root viewport pins after a follow-up disabled-root stale-pin sweep, and
  tightens the root viewport no-growth budget to `424` with Flowchart at `95`.
- GitGraph root derivation: measure commit and tag labels with GitGraph-owned
  `getComputedTextLength()`-style widths plus 1/64px quantization instead of the shared simple
  SVG bbox path. The disabled-root audit showed only 65 of the previous 130 GitGraph root pins
  still guarded real root drift, so 65 stale pins were deleted and the root viewport no-growth
  budget tightened to `432` with GitGraph at `65`.
- GitGraph seeded auto ids: mirror upstream SVG fixture generation by running a seed-consuming
  parse warm-up before the render-model parse, so generated commit ids match Mermaid's
  parse-before-render pipeline; this also removes 26 net GitGraph root viewport pins and tightens
  the root viewport no-growth budget to `497` with GitGraph at `130`.
- Flowchart label measurement: model Mermaid's unregistered custom FontAwesome fallback for
  `fab:fa-truck-bold` as the empty `<i>` fallback plus the observed Chromium inline layout delta.
  This derives the `upstream_docs_flowchart_custom_icons_238` and
  `stress_flowchart_icons_prefixes_and_quotes_052` roots without fixture pins and tightens the
  root viewport no-growth budget to `523`.
- Flowchart icon shapes: size `iconSquare` layout bounds from Mermaid's
  `iconSize + node.padding` outer box, refresh the affected icon-shape layout golden, and remove
  the now-derived `upstream_docs_flowchart_icon_shape_132` root viewport pin. This tightens the
  root viewport no-growth budget to `525`.
- Flowchart label measurement: separate SVG root CSS font size from HTML `foreignObject` label
  measurement so numeric `themeVariables.fontSize` does not inflate HTML labels, while `"NNpx"`
  font-size strings still apply. This derives
  `stress_flowchart_font_size_precedence_073` without a root viewport pin and tightens the root
  viewport no-growth budget to `526`.
- Flowchart root viewport overrides: remove two stale subgraph title-margin pins
  (`upstream_cypress_flowchart_v2_spec_should_render_subgraphs_with_title_margins_set_lr_and_htmllabels_062`
  and `upstream_flowchart_v2_subgraph_title_margins_lr_htmlLabels_false_spec`) after focused
  disabled-root `parity-root` checks showed both roots now derive without the lookup, tightening
  the root viewport no-growth budget to `527`.
- GitGraph layout: restart the commit axis for unconnected branch roots under `parallelCommits`,
  matching Mermaid's independent branch timelines without adding new root viewport pins.
- GitGraph font sizing: align layout and emitted base CSS with Mermaid's GitGraph precedence by
  honoring `themeVariables.fontSize` but ignoring top-level `fontSize`; the top-level
  `fontFamily` fallback remains in place, and the remaining font-size stress root pins are retained
  for sub-pixel branch-label/browser bbox drift.
- Sequence root bounds: propagate metadata/frontmatter titles into Sequence layout root sizing so
  rendered fallback titles get Mermaid's 40px title viewport expansion; this removes the now-derived
  `upstream_html_demos_sequence_sequence_diagram_demos_002` root pin and tightens the root viewport
  no-growth budget to `602`.
- GitGraph root derivation: include `gitTitleText` bounds in the emitted root bbox and remove 13
  now-derived title-dominated root pins, tightening the root viewport no-growth budget to `603`.
- GitGraph root viewport overrides: remove stale fixture-scoped pins after disabled-root
  diagnostics show they now derive without lookup coverage, while retaining
  `upstream_direction_bt` as a real BT-direction branch/commit-label bbox guard after seeded
  auto-id alignment.
- Flowchart root derivation: preserve bare `<`/`>` text in HTML labels and apply a narrow
  default-stack CJK width cushion for single-line labels with literal comparison symbols, deleting
  the now-derived `stress_flowchart_subgraph_title_unicode_and_entities_043` root viewport pin and
  tightening the root viewport no-growth budget to `529`.
- Sequence layout internals: split config lookup, Mermaid geometry constants, and text/math label
  measurement helpers and their owner tests into `sequence/` submodules without changing render
  output.
- Sequence layout internals: split block directive cursor-step planning into
  `sequence/block_steps.rs`, keeping `layout_sequence_diagram_typed` focused on orchestration
  while preserving Sequence SVG parity and override budgets.
- Sequence layout internals: split block frame bounds accumulation into `sequence/block_bounds.rs`,
  reducing another local state machine in `layout_sequence_diagram_typed` without changing derived
  root bounds.
- Sequence layout internals: split rect open-stack geometry and final rect horizontal bounds into
  `sequence/rect.rs`, keeping rect-specific Mermaid quirks out of the main message layout loop.
- Sequence layout internals: split activation stack state into `sequence/activation.rs`, narrowing
  the main message layout loop to endpoint geometry decisions.
- Sequence layout internals: split note node layout and wrapped-note measurement quirks into
  `sequence/notes.rs`, leaving the main message loop to apply the produced node, rect bounds, and
  cursor step.
- Sequence layout internals: split regular message edge layout into `sequence/messages.rs`,
  keeping endpoint, wrap, label, and cursor geometry behind a focused helper while the main loop
  applies actor lifecycle state.
- Sequence layout internals: split created/destroyed actor vertical lifecycle state into
  `sequence/actors.rs`, keeping Mermaid `starty`/`stopy` anchors and cursor bumps behind a focused
  state object.
- Sequence layout internals: move bottom actor box and lifeline construction into
  `sequence/actors.rs`, so participant lifecycle geometry has a single layout owner.
- Sequence layout internals: split root content/viewBox bounds derivation into
  `sequence/root_bounds.rs`, localizing mirror actor, popup, boxed participant, and self-message
  root sizing quirks.
- Sequence layout internals: move top actor box construction into `sequence/actors.rs` and remove
  the obsolete `max_actor_visual_height` accumulator.
- Sequence layout internals: bundle actor measurement, per-message spacing, box margins, and x
  coordinate planning into `SequenceActorLayoutPlan`.
- Sequence layout internals: split the remaining directive, rect, note, and message loop
  orchestration into `sequence/orchestration.rs`, leaving `sequence.rs` as a high-level actor
  planning, graph assembly, and bounds finalization coordinator.
- Sequence layout/SVG parity: move the participant `<br/>` line-width browser facts into the
  Sequence SVG metric table, deriving the `stress_long_participant_labels_br_031` root viewport
  while keeping the SVG metric table at `186` rows and tightening the root budget to `618` with
  Sequence at `80`.
- Sequence layout/SVG parity: let simple SVG bbox measurement reuse Sequence metric facts for
  `wrapLabel` width probes, deriving the `stress_br_in_messages_notes_011` no-wrap/wrap root
  viewport without growing the SVG metric table.
- Sequence layout/SVG parity: move the wrapped HTML `<br/>` message-width browser metric into the
  Sequence SVG metric table, deleting the `stress_sequence_batch5_wrap_html_br_spans_042` root
  viewport pin without growing the table budget.
- Sequence layout/SVG parity: recalibrate the literal `<br \t/>` single-line SVG width used by
  Sequence labels, deleting the `html_br_variants_and_wrap` root viewport pin.
- State layout/SVG parity: decode Mermaid `encodeEntities` placeholders before measuring State
  labels, and move the `test({ foo: 'far' })` edge-label browser width into State text metrics so
  two fixture-scoped State root viewport pins can be deleted.
- State layout/SVG parity: move the shared multiline note browser width into State note-label
  metrics and apply it in both layout and render measurement, deleting two fixture-scoped State
  root viewport pins.
- State layout/SVG parity: extend the existing transition edge-label metric to matching
  `Transition 4/5` labels, deleting two additional fixture-scoped State root viewport pins without
  growing the text lookup budget.
- State layout/SVG parity: move the docs `A transition` browser width into State edge-label
  metrics, deleting one fixture-scoped State root viewport pin.
- State layout/SVG parity: move the shared `Your state with spaces in it` browser width into State
  node-label metrics, deleting two fixture-scoped State root viewport pins.
- State layout/SVG parity: extend the existing bold-italic State node-label metric to matching
  `id1/id2` styled labels, deleting one package style root viewport pin without growing text lookup
  debt.
- Mindmap layout/SVG parity: trim delimiter-created labels that contain a single non-empty text
  line for layout measurement, preserving raw SVG text emission while deleting three
  fixture-scoped root viewport pins for Cypress single-root shape fixtures.
- Mindmap layout/SVG parity: keep plain Mindmap label measurement on raw font metrics so
  cross-diagram HTML width overrides do not inflate shaped roots, deleting the docs circle root
  viewport pin.
- Sequence layout/SVG parity: derive the small-font precedence fixture by rounding
  Mermaid-like text-dimension heights and emitting Sequence root CSS with the configured actor
  label font size, deleting one fixture-scoped root viewport pin.
- Sequence layout/SVG parity: route Mermaid-like text-dimension widths through the single-run SVG
  metric path and add the two boundary message-width facts, deleting the
  `upstream_docs_sequencediagram_boundary_008` root viewport pin.
- Sequence layout/SVG parity: correct default trailing-semicolon font-family message widths for
  common `Hello Bob`/`Hello John` labels, deleting three title/accessibility root viewport pins
  without growing the SVG text metric table.
- Sequence layout/SVG parity: delete the residual default-title root viewport pair covered by the
  corrected `Hello Bob` message width fact.
- Sequence layout/SVG parity: delete a simple `Bob thinks` note-right root viewport cluster now
  covered by existing Sequence note/message bounds.
- Sequence layout/SVG parity: delete the follow-up `Bob thinks` whitespace/comment note-right
  root viewport trio under the same existing Sequence note/message bounds.
- Sequence layout/SVG parity: delete the simple loop/rect/nested-rect `Bob thinks` block note-right
  root viewport trio under the same existing Sequence note/message bounds.
- Sequence layout/SVG parity: delete the alt-control `Bob thinks` note-right root viewport trio
  under the same existing Sequence note/message bounds.
- Core/render pipeline: centralize typed render-model dispatch and suppressed error-diagram
  construction so public parse/render entrypoints share one fallback path.
- Core/render API: `parse_diagram_for_render_model_sync` is now the single render-optimized parse
  entrypoint; semantic JSON callers should continue using `parse_diagram_sync`.
- Sequence render pipeline: add a typed render model for layout/SVG render-model dispatch while
  keeping the semantic JSON parse API stable.
- Render performance: avoid rebuilding `MermaidConfig` in class typed/config layout and render
  paths, and avoid cloning the sequence typed render model for title fallback.
- Architecture renderer: move render settings/CSS derivation, service/group node emission, edge
  bounds/DOM emission, and root viewport finalization into dedicated modules while preserving SVG
  parity.
- Architecture renderer: dedupe edge label wrap/bbox/transform calculation so bounds accumulation
  and DOM emission reuse the same local plan.
- Flowchart layout: borrow normal edges during self-loop expansion and keep ownership only for
  generated helper edges.
- Flowchart self-loops: share explicit helper-edge construction between layout and SVG rendering
  while preserving Mermaid marker and label differences.
- Sequence renderer: borrow block labels and message ids during block collection instead of
  copying them into temporary `String`s.
- Sequence renderer: borrow activation message/actor ids while building activation overlay plans.
- Sequence renderer: render non-wrapped actor, message, and note label lines from borrowed `<br>`
  splits instead of collecting temporary `String` lines.
- Sequence renderer: avoid cloning block label fragments when probing hyphenated long-word wraps.
- `xtask` font metrics generator: make the ridge solver testable and remove its
  `needless_range_loop` lint allowance.
- Flowchart parser AST: box the standalone node statement variant and remove the local
  `large_enum_variant` lint allowance.
- State parser AST: box relation statement payloads behind a dedicated relation struct and remove
  the local `large_enum_variant` lint allowance.
- Render layout API: box `LayoutDiagram` payloads and remove the public layout enum
  `large_enum_variant` lint allowance while keeping serialized layout output stable.
- Core parser lint hygiene: scope the LALRPOP generated `empty_line_after_outer_attr` allowance to
  parser wrapper modules instead of the whole `merman-core` crate.
- State semantic DB: use a single `HashMap::entry` path when creating state records, removing an
  insert-then-unwrap lookup.
- Gantt formatting: avoid a peek-then-unwrap loop while parsing fractional-second directives.
- Flowchart labels: avoid UTF-8 scanner unwraps in HTML label normalization paths.
- C4 semantic DB: centralize shape, boundary, and relation record creation helpers and remove
  local insert/lookup unwraps.
- GitGraph semantic DB: replace merge/cherry-pick production unwraps with explicit validation
  branches while preserving existing parser errors.
- Render layout cleanup: remove local invariant expects from GitGraph bounds calculation and
  recursive Class/State extracted-graph layout.
- Block semantic DB: use `HashMap::entry` for block creation and remove the local
  ensure-block existence expect.
- Class/ER parser cleanup: remove redundant `accDescr` brace scans in the lexer paths.
- State/Treemap cleanup: remove local layout and tree-construction unwraps from production
  paths.
- SVG path-bounds helper: replace the local initialize-then-unwrap pattern with
  `Option::get_or_insert`.
- Architecture foreign-object cleanup: replace stack-close expects with explicit split-off
  handling.
- Core/render scanner cleanup: replace local character-scan and delimiter-stack unwraps in
  preprocess, Gantt, QuadrantChart, Timeline, Journey, Flowchart labels, and Markdown label helpers
  with explicit optional branches.
- Core time cleanup: centralize UTC fixed-offset construction behind a shared helper and reuse it
  across Gantt/runtime code paths.
- Gantt parser cleanup: replace fixed-date and duration regex invariant unwraps with explicit
  fallible branches.
- Class SVG root cleanup: surface missing root placeholders as explicit render errors instead of
  local expect panics.
- Gantt render cleanup: remove the thin render-side UTC helper and call the shared core time
  helper directly.
- Render cleanup: remove local production unwraps from Architecture alignment flattening, Gantt
  compact section grouping, and Sequence self-frame width planning.
- Kanban render pipeline: parse a typed render model for layout/render-only flows instead of
  deserializing layout input from semantic JSON.
- Gantt render pipeline: parse a typed render model for layout/render-only flows and reuse it in
  SVG render-model dispatch instead of deserializing private JSON transport structs.
- Pie render pipeline: parse a typed render model for layout/render-only flows and reuse it in SVG
  render-model dispatch instead of deserializing duplicate private transport structs.
- Packet render pipeline: parse a typed render model for layout/render-only flows and reuse it in
  layout/SVG render-model dispatch instead of cloning config-heavy semantic JSON.
- Timeline render pipeline: parse a typed render model for layout/render-only flows and reuse it in
  layout/SVG render-model dispatch instead of deserializing private transport structs.
- Journey render pipeline: parse a typed render model for layout/render-only flows and reuse it in
  layout/SVG render-model dispatch while preserving `scoreIsNaN` semantic JSON compatibility.
- Requirement render pipeline: parse a typed render model for layout/render-only flows and reuse it
  in layout/SVG render-model dispatch instead of deserializing duplicate private transport structs.
- Sankey render pipeline: parse a typed render model for layout/render-only flows and reuse it in
  layout dispatch instead of deserializing private JSON transport structs.
- Radar render pipeline: parse a typed render model for layout/render-only flows and reuse it in
  layout/SVG render-model dispatch instead of deserializing duplicate private transport structs.
- Info render pipeline: parse a typed render model for layout/render-only flows and reuse it in
  layout/SVG render-model dispatch while keeping the semantic JSON API stable.
- ZenUML render pipeline: translate ZenUML once and parse a sequence typed render model for
  render-only flows instead of routing through semantic JSON fallback.
- Quadrant chart render pipeline: parse a typed render model for layout/render-only flows and
  reuse it in layout/SVG render-model dispatch instead of deserializing private JSON transport
  structs.
- GitGraph render pipeline: parse a typed render model for layout/render-only flows, reuse it in
  layout/SVG render-model dispatch, and borrow typed commit/branch indexes instead of cloning
  private JSON transport structs.
- Gitgraph renderer: route root viewport overrides through the shared root override helper while
  preserving title centering from the final viewBox.
- State renderer: route both root viewport override paths through the shared root override helper
  while preserving default max-width formatting.
- Treemap render pipeline: parse a typed render model for layout/render-only flows and reuse it in
  layout/layout-only SVG render-model dispatch instead of deserializing private JSON transport
  structs.
- Block render pipeline: parse a typed render model for layout/render-only flows and reuse it in
  layout/SVG render-model dispatch instead of deserializing private JSON transport structs.
- ER render pipeline: parse a typed render model for layout/render-only flows and reuse it in
  layout/SVG render-model dispatch instead of deserializing private JSON transport structs.
- Benchmarks: repair the `treemap_medium` fixture so Criterion measures treemap instead of
  skipping it as a parse error.
- CLI render command: share layout-option construction and SVG rasterization output handling across
  Mermaid-input and SVG-input render flows.
- Class renderer: reuse sorted edge order and marker-adjusted point buffers while borrowing edge ids
  for edge-label center lookup.
- Render text subsystem: split shared text types, deterministic width heuristics, Mermaid-like
  Markdown tokenization, Markdown HTML/XHTML label fragments, SVG/font bbox helpers, and flowchart
  HTML parity helpers into dedicated `text/*` modules, and move the `TextMeasurer` trait,
  flowchart-aware text metrics, and deterministic/vendored measurers into dedicated measurement
  boundaries while keeping existing `crate::text::*` callers stable.
- Render text overrides: add a `text/overrides.rs` lookup boundary for generated text override
  data and colocate flowchart override lookup tests with it.
- Timeline: colocate long-word SVG bbox override lookup and regression tests with the timeline
  layout owner.
- Class renderer: split edge ids/classes, geometry/order, edge label/terminal emission, and shared
  HTML label helpers out of `svg/parity/class/render.rs`.
- Class renderer: move class edge paths, edge labels, terminals, edge data-point encoding, and
  edge timing accumulation into `svg/parity/class/edge.rs`.
- Class renderer: move SVG content-bounds accumulation helpers into
  `svg/parity/class/bounds.rs`.
- Class renderer: move class node shell, basic-container emission, HTML row measurement, HTML
  label-group emission, SVG class node body emission, SVG title emission, SVG label-run emission,
  and divider emission into `svg/parity/class/node.rs`.
- Class renderer: move class node render-order/index construction into
  `svg/parity/class/namespace.rs`.
- Class renderer: move namespace wrapper/subgraph render-mode selection into
  `svg/parity/class/namespace.rs`.
- Class renderer: move namespace cluster group emission into
  `svg/parity/class/namespace.rs`.
- Class renderer: move root viewBox/max-width calibration and class diagram title positioning into
  `svg/parity/class/viewbox.rs`.
- Class renderer: move SVG root opening, accessibility title/description emission, root
  viewBox/max-width placeholders, and graph-margin constant into `svg/parity/class/root.rs`.
- Class renderer: move class SVG render setting derivation for htmlLabels, font sizing, padding,
  viewport padding, and theme defaults into `svg/parity/class/settings.rs`.
- Class renderer: move shared cluster/edge group orchestration for `clusters`, `edgePaths`, and
  `edgeLabels` into `svg/parity/class/groups.rs`.
- Class renderer: replace the remaining generated root viewport pins with typed profile
  calibration and model-derived namespace render-mode selection.
- Class renderer: move node traversal, namespace-subgraph transitions, note/interface dispatch, and
  class node body orchestration into `svg/parity/class/nodes.rs`.
- Architecture renderer: move JSON/typed render-model access into
  `svg/parity/architecture/model.rs` and foreignObject XHTML normalization into
  `svg/parity/architecture/foreign_object.rs`.
- Architecture renderer: move Mermaid architecture icon bodies into
  `svg/parity/architecture/icons.rs` and SVG label wrapping/text emission into
  `svg/parity/architecture/labels.rs`.
- Architecture renderer: move edge direction/arrow helpers, bounds helpers, and group rectangle
  recursion into `svg/parity/architecture/geometry.rs`.
- Architecture renderer: move SVG root opening, accessibility title/description emission, empty
  diagram fallback sizing, and root viewBox/max-width placeholders into
  `svg/parity/architecture/root.rs`.
- Architecture renderer: add default root viewport calibration for nested-groups and
  reasonable-height profiles, then prune obsolete generated Architecture root pins from 101 to 31.
- Sequence renderer: start the actor/participant split by moving actor label emission into
  `svg/parity/sequence/actors.rs`.
- Sequence renderer: move pre-actor box/rect frame emission into
  `svg/parity/sequence/frames.rs`.
- Sequence renderer: move actor popup menu emission into `svg/parity/sequence/actors.rs` and share
  sequence node geometry helpers through `svg/parity/sequence/geometry.rs`.
- Sequence renderer: move actor-man top/bottom variant emission into
  `svg/parity/sequence/actors.rs`.
- Sequence renderer: split actor popup menu emission and actor-man variants into
  `svg/parity/sequence/actor_popup.rs` and `svg/parity/sequence/actor_man.rs`.
- Sequence renderer: move top/bottom actor box and lifeline emission into
  `svg/parity/sequence/actors.rs`.
- Sequence renderer: move SVG render settings/config parsing into
  `svg/parity/sequence/settings.rs`.
- Sequence renderer: move root SVG opening, accessibility title/description, and sequence viewport
  override handling into `svg/parity/sequence/root.rs`.
- Sequence renderer: route root viewport overrides through the shared root override helper while
  preserving title placement from the computed content width.
- Sequence renderer: move activation precomputation and group emission into
  `svg/parity/sequence/activation.rs`.
- Sequence renderer: move note emission into `svg/parity/sequence/notes.rs`.
- Sequence renderer: move message-prelude interaction overlay orchestration for notes,
  activations, and block frames into `svg/parity/sequence/interactions.rs`.
- Sequence renderer: move loop/alt/par/critical block model collection into
  `svg/parity/sequence/block_collection.rs`.
- Sequence renderer: move block label wrapping, loop text emission, and block frame range helpers
  into `svg/parity/sequence/blocks.rs`.
- Sequence renderer: split block label wrapping and loop text emission into
  `svg/parity/sequence/block_text.rs`.
- Sequence renderer: split block frame/message range geometry into
  `svg/parity/sequence/block_geometry.rs`.
- Sequence renderer: split actor label, lifeline wrapper, and non-actor-man shape emission into
  `svg/parity/sequence/actor_shapes.rs`.
- Sequence renderer: split actor-man glyph geometry and SVG emission into
  `svg/parity/sequence/actor_man_glyphs.rs`.
- Sequence renderer: move message label/line emission and autonumber handling into
  `svg/parity/sequence/messages.rs`.
- Sequence renderer: share block frame and label-box emission helpers across
  loop/alt/par/critical variants.
- Sequence renderer: share block message y-range and separator y-position helpers across
  loop/alt/par/critical variants.
- Sequence renderer: share single-section loop/opt/break block emission through a common helper.
- Sequence renderer: share multi-section alt/par block emission through a common helper.
- Sequence renderer: move critical block emission into the shared block module while preserving
  its Mermaid-specific multi-section frame widening and header-height behavior.
- Sequence renderer: add a block render context so loop/alt/par/critical frame helpers share one
  explicit parameter bundle instead of repeated long argument lists.
- Sequence renderer: render notes inline with the message-prelude interaction stream so note DOM
  groups stay ordered relative to completed loop/alt/par/critical block frames.
- Class renderer: move note node emission and note-specific render timing accounting into
  `svg/parity/class/note.rs`.
- Class renderer: move interface node emission into `svg/parity/class/interface.rs`.
- Class renderer: move namespace ordering and nested subgraph emission into
  `svg/parity/class/namespace.rs`.
- Class renderer: remove the duplicate namespace-subgraph edge path/label emitter and route it
  through the shared optimized edge group path.
- Class renderer: move SVG text wrapping, label bbox, and bold-width compensation helpers into
  `svg/parity/class/label.rs`.
- Class renderer: move render lookup maps, small config helpers, and timing detail emission into
  `svg/parity/class/context.rs`.
- Class renderer: move HTML class node body emission into `svg/parity/class/node.rs`.
- Render text tests: move markdown-only tokenization and label-fragment tests next to the split
  Markdown modules.
- `xtask report-overrides`: scan all generated override modules by category instead of relying on
  a hand-maintained file list.
- Override governance: prune redundant Class text lookup entries, document retained parser-normalized
  Data method guards, and keep the no-growth budget at `480` global text lookup entries after
  restoring three Class rendered-width guards needed for `parity-root`.
- Release readiness: revalidate `cargo run -p xtask -- verify --strict` and
  `cargo bench -p merman --features render` after the Flowchart root override inventory cleanup.

### Removed

- Core/render API: removed the obsolete `parse_diagram_for_render_sync` compatibility API and its
  async alias, plus the old `mindmap` / `stateDiagram` JSON-for-render helper paths.
- Render feature flags: removed the stale `merman-render/flowchart_root_pack` experimental debug
  feature and its disabled post-layout packing code.
- Class renderer: removed the generated `class_root_overrides_11_12_2.rs` table after typed root
  viewport calibration covered the remaining entries.

## [0.4.0] - 2026-03-12

### Added

- `xtask`: support custom fixture roots in SVG baseline generation/comparison, add Markdown-aware text measurement, and
  integrate an opt-in Node/Puppeteer KaTeX path when `tools/mermaid-cli` is available.
- Docs: add and expand `docs/workstreams/*` parity planning material, including root viewport (`parity-root`) checks and
  text-measurement alignment notes.
- Tests/Fixtures: add a broad parity corpus covering font-size precedence, HTML label wrapping, Markdown `<br/>`
  continuations, unknown XML entities, KaTeX flowcharts, text-style overrides, and root viewport probes across multiple
  diagram types.

### Changed

- Text parity work now consolidates large amounts of fixture-derived width/height/padding data into generated
  `*_text_overrides_11_12_2` tables instead of leaving diagram-specific literal branches inline across layout/render code.
- SVG/style precedence now follows Mermaid more consistently: `themeVariables.fontSize` and `themeVariables.fontFamily`
  win where upstream uses them, and parity tooling captures more text-style drift during SVG comparison.

### Fixed

- Text/Markdown: align shared HTML/SVG text handling with Mermaid for inline code, failed `__` delimiter runs,
  paragraph-vs-raw-block HTML labels, punctuation-heavy URL wrapping, hyphenated-token min-content width, and trailing
  whitespace height edge cases.
- Flowchart: align HTML/SVG label wrapping, class/style text application, entity decoding, edge-label DOM/background/root
  bbox behavior, and complete the upstream Cypress new-shapes strict-XML buckets.
- Class: reduce strict-XML drift across note labels, namespaces, generics, relations/cardinality terminals, style
  propagation, annotation-driven sizing, and SVG/HTML title/member width measurement.
- ER: align relationship-label Markdown/backtick handling, root `htmlLabels` semantics, and entity/root font-size
  precedence with Mermaid baselines.
- State/Class/Mindmap/Kanban/Architecture: align remaining HTML label widths, wrapping-width handling, shared text
  constants, width parsing, and icon/service label fallback geometry between layout and SVG render.
- Block: complete strict XML parity for the Mermaid block corpus and align remaining marker-aware terminals, `space:N`
  handling, HTML label sizing, and shape-specific geometry.
- Requirement/GitGraph/Timeline/Treemap/Sequence/Sankey/C4/Journey/Pie/Radar/XYChart/Gantt: move repeated text constants
  into generated overrides and close the remaining text-geometry, viewport, and font-size precedence gaps that affected
  parity fixtures.
- Theme/CSS: stop implicitly applying `base` defaults under `theme=default`, seed Mermaid-like base/neutral xychart
  defaults, and prefer `themeVariables.fontFamily` in emitted root SVG styles.
- Core/Layout internals: clean the remaining strict Clippy offenders in `dugong-graphlib`, `dugong`, and parser helper
  code, and scope vendored `manatee` FCoSE lint exceptions to the algorithm module so current stable Clippy stays
  actionable outside the imported numeric code.
- Toolchain/CI: pin the workspace Rust toolchain to `1.87.0` and make CI install the same version explicitly, so
  release and local checks stop drifting with floating `stable`.
- Toolchain/CI: drop GitHub Actions `cargo fmt` / `cargo clippy` steps for now so release CI focuses on build, tests,
  and parity checks while the remaining render hot spots are still being aligned.
- Maintenance: normalize `rustfmt` output in parity/text/timeline/xtask helpers so the pinned toolchain now passes
  workspace format checks without local-vs-CI drift.

## [0.3.0] - 2026-03-02

### Added

- Promoted additional in-scope deferred fixtures into the committed corpus (state parser specs, flowchart icon specs,
  class diagram specs, and math examples) and generated upstream SVG baselines.

### Fixed

- Architecture: refresh compound bounds after FCoSE spring iterations before applying `relocateComponent`-style centering
  (fixes `parity-root` root `max-width` drift in deep compound/group fixtures).
- Flowchart: unescape quoted string labels (e.g. Windows paths like `C:\\Temp\\...`) and preserve Unicode punctuation in
  label text.
- `xtask compare-flowchart-svgs`: skip ELK flowchart fixtures requested via `layout: elk` / `flowchart.defaultRenderer=elk`
  (prevents layout failures while ELK parity is deferred).
- Flowchart: align icon node shape rendering with upstream Mermaid (`icon` vs `iconSquare`) to avoid NaN path data and
  restore SVG DOM parity for AWS icon fixtures.
- Flowchart: improved `iconSquare` RoughJS path parity (rounded-rect path structure) for upstream icon shape fixtures.
- Class: align `htmlLabels` split semantics more closely with Mermaid: notes now respect global `htmlLabels` + class padding, while relation title labels switch to SVG `<text>/<tspan>` + background groups only when `flowchart.htmlLabels=false` is explicitly active.
- Class: render `htmlLabels: false` labels via SVG `<text>/<tspan>` (avoid `<foreignObject>` DOM mismatches in parity
  baselines).
- Text: closer-to-upstream Mermaid Markdown tokenization for flowchart SVG labels and layout measurement (fixes
  underscore/emphasis boundary edge cases).
- Radar: fixed detailed-entry parsing so decimal values like `3.2` are not misparsed as axis `3` with value `0.2`.
- Treemap: tightened header parsing to match Mermaid CLI (`treemap:` / `treemap utilities` now fail) and preserved the
  upstream behavior where trailing whitespace-only lines are treated as a syntax error.
- `xtask audit-gaps`: avoid trimming trailing whitespace when parsing deferred fixtures (prevents false “parse OK” on
  grammars like Treemap that treat trailing whitespace-only lines as an error).
- `xtask audit-gaps`: added `--check-upstream-render-deferred-ok` to identify promotable deferred fixtures
  (in-scope + upstream render OK).
- `xtask` SVG DOM compares: further reduced noisy `parity-root` root viewport diffs by snapping `max-width`/`viewBox`
  to a coarser lattice (0.25px).
- `xtask gen-upstream-svgs` / `compare-state-svgs`: allow generating/validating upstream baselines for renderable state
  parser fixtures while skipping the known upstream-crashing `upstream_state_parser_spec` fixture.
- Architecture: improved compound/nesting layout alignment by extending the FCoSE port with a compound graph model and
  closer-to-upstream bounds/centroid propagation behavior.
- Architecture: improved edge parsing/modeling compatibility (including `lhsInto`/`rhsInto` metadata when present).
- Architecture: removed fixture-id keyed label wrapping/formatting special-cases by tightening `createText(...)`-like
  SVG label wrapping and matching Mermaid CLI attribute newline serialization (`&#10;`).
- `xtask` SVG DOM compares: stabilized anonymous edge wrapper ordering for Architecture and reduced non-actionable text
  diffs caused by line wrapping sensitivity.
- README: fixed the Stress gallery Architecture fixture reference and refreshed the Architecture showcase render.

### Not Released / WIP

- Architecture: geometry-level parity (placements, viewport, and routing coordinates) is still being aligned to upstream
  Cytoscape/FCoSE. SVG DOM parity is compared in `dom-mode parity`, so expect occasional layout snapshot churn while we
  tighten numeric fidelity.
- Flowchart: HTML-label `$$...$$` (KaTeX) fixtures now participate in strict DOM parity via the opt-in
  `NodeKatexMathRenderer`; only environments without the local `tools/mermaid-cli` toolchain still fall back to
  non-math comparisons.
- Flowchart: `flowchart-elk` layout is not implemented yet; compare tooling skips those fixtures (still kept in the
  corpus for parser coverage).
- `merman-core`: dropped support for legacy Architecture edge shorthand (e.g. `a L--R b`, `a (L--R) b`) to align with
  Mermaid@11.12.3's Langium parser; use port-colon syntax instead (e.g. `a:L -- R:b`).
- `merman-render`: introduced a pluggable `MathRenderer` interface for `$$...$$` math labels (no default KaTeX backend;
  pure-Rust remains the default).
- `xtask`: added `audit-gaps` to summarize parser-only fixtures and deferred corpus status (helps drive “missing
  implementation” work off reproducible reports).
- `xtask audit-gaps`: optionally probe upstream renderability for parser-only fixtures via Mermaid CLI (flags:
  `--check-upstream-render`, `--upstream-timeout-secs`).

## [0.2.0] - 2026-02-26

### Added

- Imported additional upstream fixtures from Cypress and package tests (requirement, gantt, ER, flowchart, sequence, state, class, quadrantchart, xychart, radar, kanban, architecture, block, mindmap, timeline) to expand SVG parity coverage.
- Imported additional upstream fixtures from Mermaid's parser package tests (architecture, gitgraph, info, packet, pie) to expand SVG parity coverage.
- Imported upstream HTML demo fixtures (flowchart, sequence, quadrantchart, sankey, xychart) to expand golden-driven parity coverage.

### Fixed

- Improved `<foreignObject>` readability fallback for raster outputs (PNG/JPG/PDF): remove the white text outline overlay and render a semi-transparent `.labelBkg` background when present (closer to upstream Mermaid defaults).
- Reduced cross-platform SVG DOM drift in `parity-root` compares by snapping root `style` `max-width` and `viewBox` to a stable lattice.
- Further reduced `parity-root` drift by bias-snapping root `max-width` and masking `viewBox` origin (x/y) while still tracking viewport size changes (w/h).
- Block: aligned `doublecircle` SVG structure to match upstream Mermaid DOM output.
- Aligned C4 `sprite` rendering with upstream Mermaid: only `person`/`external_person` emit `<image>` sprites.
- ER: align Markdown formatting in entity labels even when the entity has no attributes.
- Flowchart: preserve cyclic self-loop helper mid-edge labels (fixes missing self-loop label DOM).
- Pie: support `accTitle:` / `accDescr:` on the header line (as accepted by upstream Mermaid parser tests).
- `import-upstream-pkg-tests`: avoid failing the import when all candidates are skipped (still prints a skip summary).
- `import-upstream-pkg-tests --with-baselines`: defer fixtures that fail upstream baseline generation / render as upstream error output under `fixtures/_deferred/` (keeps the corpus without breaking parity gates).
- Reduced churn during `import-upstream-docs --with-baselines` by skipping blank-info code fences that lack an explicit Mermaid diagram directive (e.g. `flowchart` / `graph`).
- Reduced churn during `import-upstream-cypress --with-baselines` by deferring out-of-scope class fixtures (`htmlLabels=false`, `layout=elk`, `look!=classic`) under `fixtures/_deferred/`.
- Improved `import-upstream-pkg-tests` Mermaid source extraction to handle `"..."` / `'...'` literals and template strings with `${...}` interpolation.
- Sequence: render diagram titles from metadata/frontmatter when the semantic model title is empty (aligns upstream HTML demos).
- Sequence: adjusted wrapped note line breaks to match upstream Mermaid `wrapLabel(...)` behavior (11.12.3 baselines).
- QuadrantChart: derive default theme colors from `themeVariables` (including `hsl(...)`/hex parsing) to match upstream theme behavior.

### Changed

- Refreshed README showcase renders after parity updates (architecture/mindmap/sankey/gantt).
- CI: run `parity-root` SVG DOM comparisons as a non-blocking check on Ubuntu (keeps `parity` as the gate).
- Documented that the root viewport override baselines track Mermaid 11.12.3 (override module filenames still use the historical `*_11_12_2.rs` suffix).
- Updated upstream Mermaid baselines to 11.12.3 and refreshed `fixtures/upstream-svgs/**`.
- `import-upstream-html`: flowchart fixtures containing `$$...$$` math labels now use the stable `*_katex` suffix and
  participate in full SVG DOM parity when the local KaTeX backend is available.
- Deferred upstream HTML treemap demos that render as upstream error output under `fixtures/_deferred/` (avoid permanently failing parity gates).

### Removed

- Removed `mermaid-rs-renderer` (`mmdr_`) fixtures and baselines from this repository; fixtures are now sourced only from upstream Mermaid.

## [0.1.0] - 2026-02-22

### Added

- Headless Mermaid parsing and semantic JSON output (`merman-core`).
- Headless layout + SVG rendering with DOM parity gates against upstream baselines (`merman-render`).
- Ergonomic wrapper crate for UI integrations (`merman`, feature-gated via `render` / `raster`).
- CLI for detection, parsing, layout, and rendering (`merman-cli`).
- Raster outputs (PNG/JPG/PDF) via pure-Rust SVG conversion (`resvg` / `svg2pdf`).
- Golden snapshots and parity tooling (`xtask`, `fixtures/**`, `docs/alignment/STATUS.md`).
- ZenUML headless compatibility mode (subset translated to `sequenceDiagram`; not parity-gated).
- Local performance regression tracking via Criterion (`cargo bench -p merman --features render --bench pipeline`).

### Changed

- SVG renderer implementation is organized under `svg::parity` to reflect the upstream-as-spec intent.
- State diagram root viewport (`viewBox`/`max-width`) defaults to SVG-emitted bounds scanning (closest to browser `getBBox()`); set `MERMAN_STATE_VIEWPORT=layout` to use layout-derived bounds.
