# Fearless Refactor Changelog

This log records completed changes that materially advance the fearless-refactor workstream.
Detailed planning remains in `TODO.md` and `MILESTONES.md`.

## 2026-05-11

- Removed the redundant Class `Server` rendered width override after `parity-root` stayed green
  and the affected style layout golden was refreshed, reducing Class text lookups from `313` to
  `312` and the global text lookup budget from `516` to `515`; kept the `calcTextWidth` cap
  because a focused test still asserts Mermaid's `max-width: 92px`.
- Removed the redundant Class `Cart` `calcTextWidth` override after Class `parity-root` and the
  layout snapshot gate stayed green, reducing Class text lookups from `314` to `313` and the
  global text lookup budget from `517` to `516`.
- Removed the redundant Class `Payment` width overrides after `parity-root` stayed green and the
  affected layout golden was refreshed, reducing Class text lookups from `316` to `314` and the
  global text lookup budget from `519` to `517`.
- Removed the redundant Class `ERROR` width overrides after the strict gate stayed green, reducing
  Class text lookups from `318` to `316` and the global text lookup budget from `521` to `519`.
- Removed the redundant Class `ApiClient` width overrides after refreshing the dense layout
  golden, reducing Class text lookups from `320` to `318` and the global text lookup budget from
  `523` to `521`.
- Removed the redundant Class `OK` width overrides after the strict gate stayed green, reducing
  Class text lookups from `322` to `320` and the global text lookup budget from `525` to `523`.
- Normalized Sequence label-width measurement to match Mermaid's rounded SVG bbox semantics
  while keeping the single-run height path, then refreshed the affected Sequence and ZenUML
  layout goldens. This fixed the remaining `title_and_accdescr_multiline` root drift and aligned
  the vendored literal `<br \t/>` note-width expectation.
- Changed Sequence message cursor startup to use the base actor layout height rather than the
  post-render special-shape bbox, which aligned participant-type spacing with upstream. Refreshed
  the affected Sequence layout goldens and removed 8 now-redundant Sequence root viewport
  overrides (`participant_types`, `stress_participant_types_006`,
  `upstream_docs_sequencediagram_control_010`,
  `upstream_docs_sequencediagram_inline_alias_syntax_023`,
  `upstream_pkgtests_sequencediagram_spec_103`,
  `upstream_pkgtests_sequencediagram_spec_104`,
  `upstream_pkgtests_sequencediagram_spec_111`, and
  `upstream_pkgtests_sequencediagram_spec_129`), reducing the root viewport budget from `758` to
  `750` while keeping Sequence `parity-root` green.
- Added a shared xtask root viewport delta helper and wired `compare-sequence-svgs --report-root`
  to emit the same upstream/local root drift table format as Flowchart, making the remaining
  Sequence root pin cleanup easier to inspect.
- Replaced empty-diagram root viewport pins with renderer-derived empty content bounds for
  Flowchart, State, ER, and Requirement. This removed 21 root viewport override entries
  (`flowchart` 10, `state` 9, `er` 1, `requirement` 1), reducing the root viewport budget from
  `779` to `758`, while the affected fixtures stayed green under both `parity-root` and normal
  DOM comparison filters.
- Rechecked representative Sequence root viewport entries by temporarily bypassing the lookup for
  `participant_types`, `title_and_accdescr_multiline`,
  `upstream_docs_examples_basic_sequence_diagram_005`, and a long-message cypress fixture. The
  participant-type path now derives cleanly, but title and long-message guards still fail
  `parity-root` without the lookup, so Sequence still needs more bounds derivation work rather
  than another blind table-pruning pass.

## 2026-05-10

- Closed the M2 typed-model milestone as complete for all non-error in-tree Mermaid 11.12.3
  diagrams. `RenderSemanticModel::Json` remains only as the intentional `error` payload/custom
  registry fallback boundary, while remaining cleanup continues under M5 override governance.
- Removed the obsolete `xtask gen-er-text-overrides` command and generator after the remaining ER
  text override file became a three-entry hand-curated guard. The ER render path no longer probes
  an empty `calcTextWidth` table before using the shared measurement fallback.
- Corrected the stale Block text override provenance comment: the table is historical
  fixture-derived data now governed by targeted parity/layout rechecks rather than an in-tree bulk
  generator.
- Tightened override deletion policy for layout-affecting text lookups: Block ordinary labels now
  explicitly require layout snapshot evidence in addition to DOM parity because vendored
  measurement equality does not prove the default deterministic layout path is safe.
- Audited the remaining 123 Block HTML width lookups against the default deterministic layout
  measurer and found zero exact width matches, so Block text pruning is paused until the shared
  deterministic measurement path improves.
- Audited the remaining State default 16px node/edge text guards against the deterministic layout
  measurer and found zero exact width matches, so State text pruning is also paused pending
  measurement improvements.
- Removed the `clippy::all` umbrella allowance from `crates/merman-render/src/generated/mod.rs`
  after replacing the generated font-metrics lookup loop with `Iterator::find` and updating the
  `xtask gen-font-metrics` template; generated and fixture-derived parity data now stays under
  normal `merman-render` clippy coverage.
- Removed 21 redundant Class `calcTextWidth` lookup entries whose deterministic fallback now
  returns the same rounded width, after both Class DOM parity modes and layout snapshots stayed
  green, reducing Class text lookups from `344` to `323` and the global text lookup budget to
  `526`. Kept the `bar()`, `E`, `IService`, `+run() : Status`, `Client`, and `+start()` entries
  because focused SVG tests assert those Mermaid HTML `max-width` caps explicitly.
- Removed the standalone Class SVG plain-label lookup for `uses` after
  `compare-class-svgs --check-dom --dom-mode parity-root --dom-decimals 3` stayed green without
  it, deleting the now-empty plain-label bridge and reducing the global text lookup budget to
  `525`.
- Removed two redundant blank Block HTML width lookups after both Block DOM parity modes stayed
  green and the Block layout snapshots stayed green, reducing the global text lookup budget to
  `547` at that point.
- Collapsed the remaining ER HTML width lookups to 3 entries after both ER DOM parity modes stayed
  green and ER layout snapshots were refreshed, reducing the global text lookup budget to `549`.
  A follow-up bypass of the 3-entry floor still failed `parity-root`, and individual removal
  attempts confirmed that `string`, `varchar(5)`, and `DRIVER` still guard real ER drift, so the
  floor remains required.
- Removed 21 more ER HTML width lookups across alias, quoted-entity, standalone-entity,
  accessibility, attribute, and pkgtests fixtures after both ER DOM parity modes stayed green,
  reducing ER text lookups from `43` to `22` and the global text lookup budget to `568`.
- Removed three more redundant ER HTML width lookups (`code`, `generic`, and `SPACED`) after both
  ER DOM parity modes stayed green, reducing ER text lookups from `46` to `43` and the global text
  lookup budget to `589`.
- Removed the remaining six ER HTML width lookups (`Short code`, `Generic`, `Title`,
  `author-ref[name](1)`, `type<T>`, and `key+comment`) after both ER DOM parity modes stayed
  green, reducing ER text lookups from `52` to `46` and the global text lookup budget to `592`.
- Removed the redundant ER HTML width lookup for `Author ref` after both ER DOM parity modes
  stayed green, reducing ER text lookups from `53` to `52` and the global text lookup budget to
  `598`.
- Removed the remaining seven ER calc-text-width lookups (`Author ref`, `SPACED`, `Short code`,
  `author-ref[name](1)`, `key+comment`, `type<T>`, and `varchar(5)`) after both ER DOM parity
  modes stayed green, reducing ER text lookups from `60` to `53` and the global text lookup
  budget to `599`.
- Removed the redundant ER relation label width lookup for `is teacher of` after both ER DOM
  parity modes stayed green, reducing ER text lookups from `61` to `60` and the global text lookup
  budget to `606`.
- Removed the redundant ER relation label width lookup for `insured for` after both ER DOM parity
  modes stayed green, reducing ER text lookups from `62` to `61` and the global text lookup budget
  to `607`.
- Removed seven additional low-width ER relation label width lookups (`contains`, `hasMany`,
  `leads to`, `owned by`, `parent`, `places`, and `relates`) after both ER DOM parity modes stayed
  green, reducing ER text lookups from `69` to `62` and the global text lookup budget to `608`.
- Removed three redundant short ER relation label width lookups (`has`, `owns`, and `uses`) after
  both ER DOM parity modes stayed green, reducing ER text lookups from `72` to `69` and the global
  text lookup budget to `615`.
- Removed seventeen redundant ER no-attribute calc-text-width lookups whose fallback widths still
  clamp to `minEntityWidth` after both ER DOM parity modes stayed green, reducing ER text lookups
  from `114` to `97` and the global text lookup budget to `643`.
- Removed six redundant single-letter ER entity label width lookups (`A` through `F`) after both ER
  DOM parity modes stayed green, reducing ER text lookups from `97` to `91` and the global text
  lookup budget to `637`.
- Removed nineteen additional low-width ER no-attribute calc-text-width lookups after both ER DOM
  parity modes stayed green, reducing ER text lookups from `91` to `72` and the global text lookup
  budget to `618`.
- Removed two redundant State style label width lookups (`fast` and `slow`) after both State DOM
  parity modes stayed green, reducing State text lookups from `27` to `25` and the global text
  lookup budget to `660`.
- Removed four redundant State edge-label width lookups after both State DOM parity modes stayed
  green, reducing State text lookups from `31` to `27` and the global text lookup budget to `662`.
- Removed three redundant State quoted edge-label width lookups after both State DOM parity modes
  stayed green, reducing State text lookups from `34` to `31` and the global text lookup budget to
  `666`.
- Removed five redundant State node/note label width lookups after both State DOM parity modes
  stayed green, reducing State text lookups from `39` to `34` and the global text lookup budget to
  `669`.
- Removed three redundant State cluster title width lookups after both State DOM parity modes stayed
  green, reducing State text lookups from `42` to `39` and the global text lookup budget to `674`.
- Removed four redundant State rect-with-title span width/height lookups after both State DOM
  parity modes stayed green, reducing State text lookups from `46` to `42` and the global text
  lookup budget to `677`.
- Deleted the empty GitGraph text override module after rechecking that all remaining
  branch-label and commit-label glyph corrections stayed green without it under
  `compare-gitgraph-svgs --check-dom --dom-mode parity --dom-decimals 3`, reducing the global
  text lookup budget to `681`.
- Removed the redundant Requirement `Verification: Test` HTML width/calc max-width lookup pair
  after both Requirement DOM parity modes stayed green, reducing Requirement text lookups from 8
  to 6 and the global text lookup budget to `690`.
- Added `xtask verify --feature-matrix` and included it in `--strict`, so the release gate now
  checks `merman` with no default features, `render`, and `raster`, plus `merman-core` without its
  default feature set.
- Collapsed State v2 Dagre input graph construction into a single shared builder used by both the
  production layout path and the debug/xtask comparison helper, deleting the duplicate debug-only
  graph construction while keeping State tests and `parity-root` green.
- Tightened `xtask report-overrides --check-no-growth` to the then-current category totals for root
  viewport entries (`779`) and text lookup entries (`690`), so the strict gate now rejects
  reintroducing the deleted override footprint.
- Reduced the Flowchart text override module from 48 entries to 45 confirmed guards: bold/italic
  markdown deltas, HTML width guards, and SVG bbox guards for the fixtures that still drift without
  them under root parity or focused text metric assertions.
- Fixed `report-overrides` text lookup accounting for block-wrapped `=> { Some(...) }` match arms,
  which kept Class text lookups at `344`, Flowchart text lookups at `45`, and the then-current
  total at `690`.
- Removed the remaining redundant Requirement bold title/entity-name HTML width/calc max-width
  lookups (`constructor`, `dc1`, `e1`, `elA`, `elB`, `elem`, `myElem`, `myReq`, `req`, the
  `req_*` type names, `req1`, `req2`, `test_element`, `test_name`, and `test_req`) after
  Requirement DOM parity and root parity stayed green, then refreshed the
  `upstream_requirement_requirement_types_spec` and `upstream_requirement_styles_spec` layout
  goldens.
- Removed the remaining redundant Requirement `Text:` HTML width/calc max-width lookups for
  `constraint text`, the subtype text labels, and `performance requirement` after Requirement DOM
  parity and root parity stayed green, then refreshed the
  `upstream_requirement_requirement_types_spec` and `upstream_requirement_styles_spec` layout
  goldens.
- Removed the redundant Requirement `Text: base requirement` HTML width/calc max-width lookup
  after Requirement DOM parity, root parity, refreshed Requirement layout golden, override
  budget, and `verify --strict` stayed green.
- Removed the redundant Requirement `Text: the test text.` HTML width/calc max-width lookup after
  Requirement DOM parity, root parity, refreshed Requirement layout goldens, override budget, and
  `verify --strict` stayed green.
- Removed the redundant Requirement `Text: A requirement` and `Text: Do thing` HTML
  width/calc max-width lookups after Requirement DOM parity, root parity, override budget, and
  `verify --strict` stayed green.
- Removed the redundant Requirement `Doc Ref:` HTML width/calc max-width lookup bucket after
  Requirement DOM parity, root parity, override budget, refreshed Requirement/relations layout
  goldens, and `verify --strict` stayed green.
- Removed the redundant Requirement `ID:` HTML width/calc max-width lookup bucket after
  Requirement DOM parity, root parity, override budget, and `verify --strict` stayed green.
- Removed the redundant Requirement `Type: system` and `Type: test_type` HTML width/calc
  max-width lookups after Requirement DOM parity, root parity, override budget, and
  `verify --strict` stayed green, while keeping `Type: simulation` after simulation-heavy
  fixtures drifted without it.
- Removed the redundant Requirement `Verification: Demonstration` and `Verification: Inspection`
  HTML width/calc max-width lookups after Requirement DOM parity, root parity, override budget,
  and `verify --strict` stayed green. `Verification: Analysis` remained after `basic` still
  drifted when it was removed; `Verification: Test` was removed in the later recheck above.
- Removed the redundant Requirement `Risk: High`, `Risk: Low`, and `Risk: Medium` HTML
  width/calc max-width lookups after Requirement DOM parity, root parity, override budget, and
  `verify --strict` stayed green.
- Removed the redundant Requirement `<<Design Constraint>>`, `<<Interface Requirement>>`, and
  `<<Physical Requirement>>` HTML width/calc max-width lookups after both DOM parity modes stayed
  green, while keeping `<<Performance Requirement>>` after root `max-width` drift was confirmed.
- Removed the redundant Requirement `<<Functional Requirement>>` HTML width and calc max-width
  lookups after both DOM parity modes stayed green without them, then refreshed the affected
  Requirement layout goldens.
- Removed the redundant Requirement `<<Element>>` HTML width and calc max-width lookups after both
  DOM parity modes stayed green without them, then refreshed the affected Requirement layout
  goldens.
- Removed the redundant Requirement `<<Requirement>>` HTML width and calc max-width lookups after
  both DOM parity modes stayed green without them, then refreshed the affected Requirement layout
  goldens.
- Removed the redundant Requirement `<<traces>>` HTML width and calc max-width lookups after both
  DOM parity modes stayed green without them, then refreshed the affected Requirement layout
  goldens.
- Removed the redundant Requirement `<<satisfies>>` HTML width and calc max-width lookups after
  both DOM parity modes stayed green without them, then refreshed the affected Requirement layout
  goldens.
- Removed the redundant Requirement `<<contains>>` HTML width and calc max-width lookups after
  both DOM parity modes stayed green without them, then refreshed the Requirement layout goldens
  to match the new layout.
- Removed the redundant Class generated text override smoke tests after DOM parity and layout
  tests already covered the live class lookup paths.
- Removed the redundant ER generated drawrect-clamp smoke test while keeping the ER-owned label
  metrics and htmlLabels behavior tests.
- Removed the redundant State generated text helper smoke test after layout snapshots, SVG DOM
  parity, and the strict release gate covered the live helper path.
- Removed the redundant Requirement generated text-lookup smoke tests after SVG DOM parity and
  the strict release gate already covered the live lookup path.
- Removed the redundant GitGraph generated text-lookup smoke test after the live renderer path
  stayed covered by SVG DOM parity checks and the override no-growth gate.
- Removed eight more GitGraph glyph correction lookups for the right-side `C`, `D`, `B`, `0`, `6`,
  `4`, `a`, and `d` characters after DOM parity stayed green with the even smaller correction
  table.
- Removed five more GitGraph glyph correction lookups for the left-side `2`, `6`, `5`, `C`, and
  `B` characters after DOM parity stayed green with the smaller correction table.
- Removed three redundant GitGraph commit-label literal extra lookups after the rounded measured
  widths and existing edge-character corrections stayed green.
- Regenerated the GitGraph layout goldens after deleting the 7-entry branch-label bbox correction
  table; DOM parity stayed green with the simplified measured-width path.
- Removed seven redundant GitGraph branch-label bbox correction lookups and simplified branch-label
  width planning to the shared measured-width-plus-1/64px-quantization path after GitGraph DOM
  parity stayed green without the branch-label table.
- Removed the obsolete `xtask gen-gantt-text-overrides` command and generator after the Gantt text
  override table was deleted, so the command layer no longer advertises a stale production output.
- Added shared closed-path and Mermaid arc-point helpers in `roughjs_common` and routed the
  flowchart and state point-path helpers through them, deleting the duplicated local
  implementations.
- Added `TYPED_MIGRATION_TIMING.md` as the canonical index for typed migration timing evidence and
  follow-up canaries.
- Removed the generic Gantt `A` task-width override after `compare-gantt-svgs --check-dom
  --dom-mode parity --dom-decimals 3` stayed green.
- Removed the generic Gantt `B` and `C` task-width overrides after `compare-gantt-svgs --check-dom
  --dom-mode parity --dom-decimals 3` stayed green.
- Removed the generic Gantt `Build` and `Design` task-width overrides after
  `compare-gantt-svgs --check-dom --dom-mode parity --dom-decimals 3` stayed green.
- Removed the generic Gantt `Noon` task-width override after `compare-gantt-svgs --check-dom
  --dom-mode parity --dom-decimals 3` stayed green.
- Removed the generic Gantt `t1` task-width override after `compare-gantt-svgs --check-dom
  --dom-mode parity --dom-decimals 3` stayed green.
- Removed the generic Gantt `task1` task-width override after `compare-gantt-svgs --check-dom
  --dom-mode parity --dom-decimals 3` stayed green.
- Removed the generic Gantt `test1` task-width override after `compare-gantt-svgs --check-dom
  --dom-mode parity --dom-decimals 3` stayed green.
- Removed the generic Gantt `test2` task-width override after `cargo run -p xtask -- verify
  --strict` stayed green with regenerated layout goldens.
- Removed the generic Gantt `test3` through `test7` task-width overrides after `cargo run -p
  xtask -- verify --strict` stayed green with regenerated layout goldens.
- Removed the generic Gantt `task2` through `task4` task-width overrides after `cargo run -p
  xtask -- verify --strict` stayed green with regenerated layout goldens.
- Removed the isolated Gantt `y68` and `y69` task-width overrides after `cargo run -p xtask --
  verify --strict` stayed green with regenerated layout goldens.
- Removed the Gantt duration-label task-width overrides for `days`, `hours`, `minutes`, `ms`, and
  `seconds` after `cargo run -p xtask -- verify --strict` stayed green with regenerated layout
  goldens.
- Removed nine small-fixture Gantt task-width overrides spanning leading punctuation, callback,
  proto-id, year fallback, and 12-hour time labels after `cargo run -p xtask -- verify --strict`
  stayed green with regenerated layout goldens.
- Removed the Gantt `task A` through `task D` task-width overrides in `relative_end_mixed` after
  `cargo run -p xtask -- verify --strict` stayed green with regenerated layout goldens.
- Removed the final Gantt task-width overrides and deleted
  `gantt_text_overrides_11_12_2.rs`, leaving Gantt task labels on the shared text measurement path.
- Lifted RoughJS rectangle and circle generation into the shared parity helper layer, so State and
  Flowchart now reuse the same seeded shape emission code as well as the same path formatting.
- Introduced a shared RoughJS parity helper layer for hex parsing and `opsToPath` formatting, so
  State and Flowchart no longer duplicate the same low-level conversion code.
- Collapsed repeated Flowchart RoughJS stroke dash parsing into a shared private helper and
  narrowed Flowchart node helper internals that no longer need sibling-module visibility.
- Collapsed duplicated Flowchart RoughJS op-set SVG path serialization into a single private
  helper after Flowchart DOM parity and the strict gate stayed green.
- Narrowed State link sanitizer internals to file-private helpers after the State parity gate and
  strict gate stayed green.
- Collapsed the duplicated State label HTML line-wrapping and entity-preservation logic behind
  shared private helpers, kept the public State label entry points thin, and revalidated State DOM
  parity plus the strict gate.
- Collapsed State raw/non-raw context resolution behind shared helper implementations, removed
  now-unused wrappers, and narrowed `state_strip_note_group` to file-private visibility after State
  DOM parity and the strict gate stayed green.
- Inlined `prefer_fast_state_viewport_bounds` into the two State viewport call sites after
  `cargo run -p xtask -- compare-state-svgs --check-dom --dom-mode parity --dom-decimals 3` and
  `cargo run -p xtask -- verify --strict` stayed green.
- Inlined `maybe_insert_midpoint_for_basis` into the flowchart edge path builder after
  `cargo run -p xtask -- compare-flowchart-svgs --check-dom --dom-mode parity --dom-decimals 3
  --filter flowchart` and `cargo run -p xtask -- verify --strict` both stayed green without the
  helper.
- Deleted `maybe_pad_cyclic_special_basis_route` from the flowchart basis helper after
  `cargo run -p xtask -- compare-flowchart-svgs --check-dom --dom-mode parity --dom-decimals 3
  --filter flowchart` and `cargo run -p xtask -- verify --strict` both stayed green without it.
- Removed the obsolete flowchart straight-except-one-endpoint helper after full flowchart DOM
  parity stayed green without it.
- Revalidated the full `cargo bench -p merman --features render` gate after the first 20-minute
  attempt timed out, and recorded the successful run in
  `docs/performance/spotcheck_2026-05-10_full_bench_gate.md`.
- Rechecked the redundant flowchart cluster-run edge helper and kept it in place after
  `cargo run -p xtask -- verify --strict` exposed flowchart DOM mismatches without the special
  case.
- Rechecked the obsolete flowchart degenerate path helper and kept it in place after
  `cargo run -p xtask -- verify --strict` exposed flowchart DOM mismatches on subgraph-descendant
  fixtures without it.
- Made the mmdr benchmark helper scripts lockfile-aware and added `--mmdr-toolchain` so the
  reference checkout can run under a compatible Rust toolchain while this workspace remains pinned.
  Recorded a fresh standard-canary stage spotcheck in
  `docs/performance/spotcheck_2026-05-10_standard_canaries_stage_mmdr_toolchain.md`, keeping
  Architecture layout and broad render fixed-cost as the current performance signals.
- Added a shared import fixture-file helper module so cleanup and defer logic now lives in one
  place while the cypress, docs, examples, html, and pkg_tests modules keep thin policy wrappers.
  Revalidated the refactor with `cargo run -p xtask -- verify --strict`.
- Removed stale `workspace_root` plumbing from `xtask` fixture, snapshot, compare, debug,
  generate, import, and override helpers after centralizing project-root helpers, restoring the
  strict gate including workspace clippy.
- Added project-root helpers in `cmd::paths` for `fixtures`, `target`, `repo-ref/mermaid`,
  `repo-ref/dompurify`, and `tools/mermaid-cli`, then routed the `generate`, `audit`,
  `compare/xml`, `compare/flowchart`, `overrides`, and import call sites through them, deleting
  the repeated workspace-root path scaffolding from the command layer.
- Added a shared `xtask` compare-diagram path helper so the per-diagram SVG compare commands now
  build fixture, upstream, report, and output directories through one owner instead of repeating
  the same workspace-root path scaffolding.
- Revalidated the workspace-root helper cleanup with `cargo run -p xtask -- verify --strict`,
  which covers workspace clippy, nextest, snapshot gates, and SVG parity checks.
- Moved `xtask` workspace-root discovery into a dedicated `cmd::paths` module and routed the
  remaining `compare`, `debug`, `generate`, `import`, `overrides`, `verify`, `snapshots`, and
  `state_svgdump` call sites through it, deleting the last repeated `CARGO_MANIFEST_DIR`
  parent-walking code from the command layer.
- Centralized snapshot update diagram selector matching so semantic and layout snapshot generation
  share the same directory alias rules and scoped error-fixture behavior.
- Centralized `xtask` `.mmd` fixture discovery for semantic snapshots, layout snapshots, and
  alignment checks, keeping `_deferred`, `upstream-svgs`, parser-only, and filename filter policy in
  one place.
- Added a shared `xtask` single-directory fixture listing helper and routed the SVG compare
  commands through it, deleting repeated parser-only scan loops across the compare diagram modules.
- Reused the same fixture listing helper in upstream SVG generation and the Architecture debug
  tooling, keeping the diagram-specific exclusions local while deleting the shared scan boilerplate.
- Promoted recursive `.mmd` fixture discovery into the shared `xtask` fixture helper and moved
  snapshot generation plus `audit-gaps` onto it, so parser-only, deferred, and upstream-SVG scan
  policy is no longer reimplemented per command.
- Extracted a shared `xtask` fixture-to-SVG export helper and refactored `gen-debug-svgs` plus
  the ER, Flowchart, State, Class, and C4 generators onto it, removing repeated scan/read/write
  loops from the command layer.
- Recorded the current Mindmap/Architecture local pipeline canary in
  `docs/performance/spotcheck_2026-05-10_mindmap_architecture_canary_pipeline_long.md`, preserving
  the strong local layout-stage signal and the small `parse/mindmap_medium` watch item.
- Moved the three stable C4 SVG bbox line-height rules into the C4 owner module, deleted
  `c4_text_overrides_11_12_2.rs`, moved the 17 C4 type-line `textLength` pins into the owner
  module, and kept the type-line `textLength` logic in owner code.
- Rechecked the lone Timeline text lookup and documented that it still guards the
  `upstream_long_word_wrap` root `max-width` parity pin.
- Removed the thin render-side UTC helper in Gantt and called the shared core time helper
  directly.
- Turned the Class SVG root placeholder lookup into an explicit render error instead of a local
  expect panic.
- Replaced Gantt fixed-date and duration regex invariant unwraps with explicit fallible branches.
- Centralized zero-offset timezone construction behind `merman_core::time::utc_fixed_offset()` and
  reused it in runtime/Gantt code paths.
- Replaced local character-scan and delimiter-stack unwraps in preprocess, Gantt date formatting,
  QuadrantChart parsing, Timeline/Journey wrapping, Flowchart labels, and shared Markdown label
  helpers with explicit optional branches.
- Reworked the Architecture foreign-object close-tag handling to use `split_off` and explicit
  fallback branches instead of stack-pop expects.
- Replaced the `svg::parity::path_bounds` initialize-then-unwrap helper with
  `Option::get_or_insert`, keeping the same computed path bounds.
- Removed local layout and tree-construction unwraps from State renderer edge post-processing and
  Treemap hierarchy construction, keeping the same DOM outputs while avoiding local panics.
- Removed redundant `accDescr` brace scans from the Class and ER lexer paths by reusing the
  already-trimmed leading whitespace offset.
- Replaced BlockDB's insert-then-unwrap block creation with a single `HashMap::entry` path while
  preserving block ordering and parser behavior.
- Removed local render-layout invariant expects from GitGraph bounds calculation and Class/State
  recursive extracted-graph layout, turning inconsistent graph state into explicit layout errors.
- Replaced GitGraph merge and cherry-pick semantic DB unwraps with explicit validation branches
  while preserving GitGraph parser errors and SVG DOM parity.
- Centralized C4 shape, boundary, and relation record creation behind DB helpers and removed local
  C4 insert/lookup unwraps while preserving C4 parser tests and SVG DOM parity.
- Replaced Flowchart HTML label scanner unwraps with explicit UTF-8 character advances while
  preserving Flowchart render tests and SVG DOM parity.
- Replaced the Gantt d3-time-format fractional-second parser's peek-then-unwrap loop with an
  explicit peek/advance loop while preserving Gantt DOM parity.
- Replaced StateDB's insert-then-unwrap state lookup with a single `HashMap::entry` path while
  preserving State parser tests and SVG DOM parity.
- Scoped the LALRPOP generated `empty_line_after_outer_attr` allowance to parser wrapper modules
  and removed the broad `merman-core` crate-level allowance.
- Boxed public `LayoutDiagram` payloads and removed the render model `large_enum_variant`
  allowance while preserving serialized layout shape.
- Boxed State AST relation statement payloads behind a dedicated `RelationStmt` and removed the
  state `large_enum_variant` lint allowance without changing parser or render output.
- Boxed the standalone Flowchart AST node statement variant and removed its
  `large_enum_variant` lint allowance while keeping the parser/build path unchanged.
- Added a lint-allow audit for the remaining source-level allowances, including the confirmed
  generated State parser `filter_map_identity` allowance and the larger enum migration candidates.
- Removed local production unwraps from Architecture alignment flattening, Gantt compact section
  grouping, and Sequence self-frame width planning without changing DOM parity.
- Made the `xtask` font-metrics ridge solver module-local and covered it with focused tests, then
  removed its `needless_range_loop` lint allowance.
- Added an override-report gate that rejects root viewport lookup call sites outside the shared
  root override helper contract.
- Routed both State root viewport override paths through the shared root override helper while
  preserving the existing default max-width formatting.
- Routed Sequence root viewport override application through the shared root override helper while
  preserving title placement from the computed content width.
- Routed Gitgraph root viewport override application through the shared root override helper while
  preserving title centering from the final viewBox.
- Added default Architecture root viewport calibration for nested-groups and reasonable-height
  profiles, then pruned 70 obsolete Architecture root pins, reducing root viewport overrides to 779
  while keeping Architecture `parity-root` green.
- Moved the remaining Class root viewport pins into typed profile calibration and model-derived
  namespace render-mode selection, then deleted the Class root override module, reducing root
  viewport overrides to 849 while keeping Class `parity-root` green.
- Modeled section-less Pie root viewport behavior and legend bbox width in the renderer, then
  deleted the Pie root override module, reducing root viewport overrides to 908 while keeping Pie
  `parity-root` green.
- Refreshed Mindmap typed root viewport profile calibration, added two small model-derived profiles,
  and pruned 28 obsolete Mindmap root pins, reducing root viewport overrides to 880 while keeping
  Mindmap `parity-root` green.
- Rechecked the 3 remaining Sankey root viewport pins by disabling the Sankey root lookup and
  confirming `parity-root` still drifts on the three energy-flow fixtures, so those pins stay in the
  override budget until Sankey root height derivation changes.
- Removed the remaining generated `dead_code` allowances from override modules and generator
  templates; the source tree now has no `dead_code` allow entries.
- Collapsed Flowchart callback actions to the semantic state actually used by rendering, removing
  the last non-generated `dead_code` allow from `merman-core` / `merman-render`.
- Removed local dead parity helpers in ER, GitGraph, and State after clippy, targeted nextest, and
  each touched diagram family's DOM parity gate stayed green.
- Narrowed Flowchart parity context/API surface by deleting unused style/class/cluster wrappers and
  removing context fields that were only initialized, leaving the flowchart parity subtree free of
  non-generated `dead_code` allows.
- Removed stale core parser helpers: the unused `BlockDb` id generator, old Flowchart
  collect/merge helpers, and an unnecessary `TitleKind` dead-code allow.
- Removed unused no-bounds D3 curve path wrappers from `svg/parity/curve.rs`; active renderers now
  use the shared path-and-bounds entrypoints or the still-used basis/linear path helpers.
- Deleted the unused Flowchart `edge_bbox` helper module and narrowed the remaining cyclic-special
  basis helper visibility after Flowchart tests, clippy, and SVG DOM parity stayed green.
- Moved ER and Block HTML width override ownership out of the shared vendored text measurer and
  back into the owning diagram modules, then deleted the stale Mindmap HTML width override table
  and generator. Generic HTML text measurement can no longer be hijacked by diagram-specific
  fixture strings, and text lookup debt is down by 291 entries.
- Tightened the manual raw SVG/path bridge no-growth budget from 1 to 0 and added a regression
  test, so strict verification now rejects any bridge reintroduction unless the budget is
  intentionally reviewed.
- Normalized the Flowchart math upstream SVG baseline for `upstream_docs_math_flowcharts_001` to
  the current Mermaid CLI + Chrome output and made the Node KaTeX probe retry system browsers while
  measuring the sanitized MathML that SVG emission uses, clearing the last Flowchart `parity-root`
  gap without adding root viewport pins.
- Removed 131 obsolete Flowchart root viewport pins now covered by deterministic root output,
  reducing root viewport overrides to 931 while keeping Flowchart `parity-root` green.
- Removed thirty-two obsolete Sequence root viewport pins now covered by deterministic root output,
  reducing root viewport overrides to 1062 while keeping Sequence `parity-root` green.
- Removed six obsolete Gitgraph root viewport pins now covered by deterministic root output,
  reducing root viewport overrides to 1094 while keeping Gitgraph `parity-root` green.
- Collapsed the Class root viewport table from 196 entries to 31 by removing 166 obsolete pins and
  adding one missing existing docs root pin, reducing root viewport overrides to 1100 while making
  Class `parity-root` green.
- Removed sixty-eight obsolete State root viewport pins now covered by deterministic root output,
  reducing root viewport overrides to 1265 while keeping State `parity-root` green.
- Removed all 119 obsolete Block root viewport pins and deleted the now-empty Block root override
  module, reducing root viewport overrides to 1333 while keeping Block `parity-root` green.
- Removed sixteen obsolete C4 root viewport pins now covered by deterministic root output,
  reducing root viewport overrides to 1452 while keeping C4 `parity-root` green.
- Removed thirty-five obsolete Requirement root viewport pins now covered by deterministic root
  output, reducing root viewport overrides to 1468 while keeping Requirement `parity-root` green.
- Removed twelve obsolete ER root viewport pins now covered by deterministic root output,
  reducing root viewport overrides to 1503 while keeping ER `parity-root` green.
- Removed twelve obsolete Pie root viewport pins now covered by deterministic root output,
  reducing root viewport overrides to 1515 while keeping Pie `parity-root` green.
- Removed nine obsolete Timeline root viewport pins now covered by deterministic root output,
  reducing root viewport overrides to 1527 while keeping Timeline `parity-root` green.
- Removed four obsolete Sankey root viewport pins now covered by deterministic emitted bounds,
  reducing root viewport overrides to 1536 while keeping Sankey DOM parity green.
- Made `xtask report-overrides` print zero-count categories with metadata and `no entries`, so
  helper/bridge elimination remains visible in strict-gate logs instead of disappearing from the
  report.
- Reclassified Gitgraph bbox correction data as text metric lookup entries and moved branch-label
  correction control flow back into the `gitgraph` owner module, reducing the helper footprint to
  zero while keeping the measured correction table visible in override reporting.
- Moved Architecture text bbox formulas, canvas-label width scale, service label extension, and
  default wrap width into `architecture` owner constants/functions, deleting the now-empty
  Architecture text override module and reducing helper footprint to 6.
- Moved Sequence note wrap slack, text line-height math, and frame padding geometry into
  `sequence` owner constants/functions, deleting the now-empty Sequence text override module and
  reducing helper footprint to 12.
- Moved Treemap section spacing geometry into `treemap` owner constants and kept the remaining
  `Item A1` leaf-fit browser tolerance beside the SVG parity loop, deleting the now-empty Treemap
  text override module and reducing helper footprint to 18.
- Moved Kanban section padding, label foreignObject height, and item row heights into `kanban`
  owner constants, deleting the now-empty Kanban text override module and reducing helper
  footprint to 21.
- Moved Journey fixed viewBox/title/legend/face geometry into `journey` owner constants, deleting
  the now-empty Journey text override module and reducing helper footprint to 26.
- Moved Sankey node width/padding values into `sankey` owner constants and a private padding helper,
  deleting the now-empty Sankey text override module and reducing helper footprint to 32.
- Moved Pie's remaining legend rectangle/spacing values into `pie` owner constants shared by
  layout and SVG, deleting the now-empty Pie text override module and reducing helper footprint to
  34.
- Inlined Radar legend row spacing in layout and deleted the now-empty Radar text override module,
  reducing generated override modules to 35 and the helper footprint to 36.
- Removed the dead Architecture icon text bbox helper, leaving Architecture text overrides focused
  on production layout/SVG parity call sites and reducing the helper footprint to 37.
- Removed Sankey SVG-only label font/gap/dy helpers by inlining the fixed values in the renderer,
  leaving only node geometry and padding helpers and reducing the helper footprint to 38.
- Removed Sequence self-only frame min pad helpers by inlining the fixed values in block geometry,
  reducing the helper footprint to 41.
- Removed Treemap section header label/value sizing helpers by inlining the fixed values in the
  renderer, leaving only the shared spacing helpers and leaf-fit tolerance and reducing the helper
  footprint to 43.
- Removed XYChart bar data-label scale and inset helpers by inlining the fixed values in the SVG
  renderer, deleting the now-empty generated override module and reducing the helper footprint to
  48.
- Removed Pie's single-use margin, center, radius, legend label font size, title y, and legend
  text y helpers by inlining the fixed values at the layout/render call sites, reducing the
  hand-curated helper footprint to 50.
- Removed Radar legend box size and label x-offset helpers by inlining the fixed values at the
  render call sites, reducing the hand-curated helper footprint to 56.
- Removed single-use Journey legend placement and mouth offset helpers by inlining the upstream
  fixed values at the layout call sites, reducing the hand-curated helper footprint to 58.
- Refreshed `OVERRIDE_FOOTPRINT.md` after `xtask verify --strict` so the snapshot now reports zero
  manual raw SVG/path bridge files and matches the current override inventory.
- Cached XYChart axis tick labels inside the layout axis state so `calculate_space`,
  `tick_distance`, and axis drawable generation reuse the same labels instead of rebuilding them.
  The follow-up smoke records `layout/xychart_medium` at `55.129-60.551 us` in
  `docs/performance/spotcheck_2026-05-09_xychart_layout_tick_cache.md`.
- Reduced XYChart SVG render allocation overhead by replacing the temporary DOM arena's
  per-node `BTreeMap` attribute tables with static tags and insertion-order attribute vectors,
  centralizing nested group creation, and writing shared XYChart CSS directly into the output
  buffer. The follow-up pipeline smoke records `render/xychart_medium` at `113.74-122.92 us` in
  `docs/performance/spotcheck_2026-05-09_xychart_render_allocation_cleanup.md`.
- Fixed the benchmark comparison scripts so the local `mermaid-rs-renderer` checkout runs its
  Criterion benches under `MMDR_RUN_CRITERION_BENCHES=1` instead of falling back to smoke
  validation.
- Refreshed the rolling `docs/performance/COMPARISON.md` baseline after the C4 direct
  render-model parse cleanup. C4 end-to-end is now about `1.3x` slower than
  `mermaid-rs-renderer`, while Architecture and XYChart remain the largest current canary gaps.
- Added dedicated C4/XYChart cross-repo end-to-end and stage spotcheck reports at
  `docs/performance/spotcheck_2026-05-09_c4_xychart_mmdr_comparison.md` and
  `docs/performance/spotcheck_2026-05-09_c4_xychart_stage_mmdr.md`.
- Added a Mindmap/Architecture/C4 stage spotcheck at
  `docs/performance/spotcheck_2026-05-09_mindmap_architecture_c4_stage_mmdr.md`, confirming
  Architecture layout remains the largest observed stage gap after the C4 parse cleanup.
- Routed C4 render-model parsing directly from `C4Db` into `C4DiagramRenderModel`, removing the
  render-only semantic-JSON-to-typed bridge. The targeted pipeline smoke now observes
  `parse/c4_medium` at `36.946-40.355 us` and `end_to_end/c4_medium` at `176.19-191.27 us`; see
  `docs/performance/spotcheck_2026-05-09_c4_direct_render_model_parse.md`.
- Pruned the Architecture layout JSON compatibility model by deleting unused node/edge fields and
  the unused top-level group separation helper while keeping workspace clippy, nextest, and
  Architecture DOM parity green.
- Removed the final manual raw SVG/path bridge by collapsing the flowchart degenerate
  subgraph-descendant route into generic single-point path emission; `xtask report-overrides` now
  reports zero manual bridge files.

- Replaced the remaining 7 generated `kanban` root viewport pins with profile-based root height
  calibration, removing the generated Kanban root table while keeping `parity-root` green.
- Pruned 4 obsolete `kanban` root viewport entries from the generated table after confirming the
  remaining 7 fixture-specific pins still gate `parity-root`.
- Removed the redundant Kanban label line-height helper by reusing the existing foreignObject
  height constant, reducing the hand-curated helper footprint to 82.
- Collapsed the XYChart bar data-label scale helpers into one public helper, further reducing the
  hand-curated helper footprint to 81.
- Removed the derived Treemap section header center-y helper and computed it from the header
  height directly, reducing the hand-curated helper footprint to 80.
- Collapsed the Pie center point into one public helper for both axes, reducing the
  hand-curated helper footprint to 79.
- Removed the redundant Radar legend baseline-y helper and used the literal `0.0` directly,
  reducing the hand-curated helper footprint to 78.
- Removed two derived Pie legend-position helpers by computing legend x-offsets from the existing
  rectangle size and spacing constants, reducing the hand-curated helper footprint to 76.
- Removed the derived Pie label-radius helper and two Treemap header spacing helpers by computing
  them from existing layout constants, reducing the hand-curated helper footprint to 73.
- Removed two derived Journey helpers by reusing the legend circle radius for legend text baseline
  alignment and the viewBox top padding for title y-position, reducing the helper footprint to 71.
- Removed the derived Sequence self-message separator extra-y helper by computing it from the
  existing frame envelope extra-y value, reducing the helper footprint to 70.
- Removed the derived Kanban item label inset helper by reusing the existing section padding
  constant, reducing the helper footprint to 69.
- Removed the derived Architecture singleton service offset helper by reusing the existing service
  label bottom extension constant, reducing the helper footprint to 68.
- Consolidated XYChart bar data-label horizontal and vertical inset helpers into one shared inset
  helper, reducing the helper footprint to 67.
- Hardened `xtask report-overrides` helper counting so restricted-visibility helpers still count
  toward the hand-curated helper budget.
- Repaired the `xychart_medium` bench fixture and recorded a C4/XYChart pipeline bench smoke so the
  remaining typed-model performance notes no longer depend on future benchmarkable fixtures.
- Added a render-feature regression test that keeps every `pipeline` bench fixture parseable and
  renderable so Criterion cannot silently lose coverage through pre-check skips.
- Removed the obsolete generated `journey` root viewport override table and its renderer call site
  after DOM parity passed without the 4 fixture-specific pins.
- Consolidated `merman-cli` render execution around internal `RenderRequest` and
  `RasterRequest` structs so parse/layout/render and SVG-raster handling share a smaller execution
  boundary without changing CLI behavior.

## 2026-05-08

- Corrected `xtask report-overrides` text lookup accounting so generated `*_OVERRIDES_*`
  binary-search tables are counted as text metric lookup entries instead of hand-curated helpers,
  with refreshed no-growth budgets and footprint docs.
- Collapsed redundant public Sankey padding component helpers into private constants, leaving only
  the `showValues`-aware public padding lookup in the override footprint.
- Removed unused requirement-layout `max-width` calculation state plus dead state/gantt helper
  functions that were kept only behind `dead_code` allows.
- Added a focused `text_measure_stress` Criterion bench for vendored font measurement and wrapped
  label paths before future cache work.
- Recorded the `text_measure_stress` same-machine Criterion spotcheck in
  `docs/performance/spotcheck_2026-05-08_text_measure_stress.md`.
- Removed a dead private font-metric quantizer and made the flowchart cluster-width probe
  test-only so production text-measure code stays slimmer.
- Added category-level owner/source/allowed-use/expected-removal metadata to `xtask
  report-overrides`, plus a regression test so generated override categories keep explicit removal
  criteria.
- Removed dead xtask debug/generator helpers, including unused state analyzer geometry, an obsolete
  font-metrics browser char-width helper, a stale flowchart width estimator, and an unused SVG
  override scratch struct.
- Added an override no-growth budget gate to `xtask report-overrides` and wired it into
  `xtask verify --strict` so new override growth must be explicit.
- Replaced `check-upstream-svgs`' long-argument helper with a request struct, removing the last
  `clippy::too_many_arguments` allow from `xtask` source.
- Removed 19 redundant architecture root viewport overrides after topology-driven calibration
  covered the matching profiles.
- Expanded `xtask report-overrides` to inventory hand-authored `maybe_override_*` raw SVG/path
  bridge functions under `svg/parity`, with stable `/` paths in report output.
- Fixed override helper-function counting in `xtask report-overrides` and added regression tests
  for helper and manual bridge detection.
- Documented the then-current flowchart degenerate path bridge with owner/removal criteria and
  refreshed `OVERRIDE_FOOTPRINT.md` for the generated-plus-manual report snapshot.
- Replaced sequence parity renderer long-argument helpers with focused render contexts and removed
  the sequence module-level `clippy::too_many_arguments` allow while keeping sequence DOM parity
  green.
- Structured SVG path-bounds cubic/arc inputs and removed the `path_bounds` module-level
  `clippy::too_many_arguments` allow.
- Structured shared SVG curve path emission around `PathPoint`/`PathCubic`, merged duplicate basis
  bounded/unbounded logic, and removed the `curve` module-level `clippy::too_many_arguments` allow.
- Grouped journey text candidate geometry/font inputs into small structs and removed the `journey`
  module-level `clippy::too_many_arguments` allow.
- Replaced treemap root viewBox's long-argument rectangle bounds helper with a small accumulator
  and removed the `treemap` module-level `clippy::too_many_arguments` allow.
- Replaced requirement label foreignObject emission with a small input struct and removed the
  `requirement` module-level `clippy::too_many_arguments` allow.
- Bundled sankey relaxation parameters into a small context struct and removed the `sankey`
  module-level `clippy::too_many_arguments` allow.
- Replaced timeline node layout's positional content/geometry/text arguments with
  `TimelineNodeRequest` and removed the `timeline` module-level `clippy::too_many_arguments`
  allow.
- Bundled sequence block frame width planning inputs into `BlockFrameWidthContext` and removed the
  `sequence` module-level `clippy::too_many_arguments` allow.
- Replaced C4 SVG tspan text emission's positional geometry/font arguments with `C4TspanText` and
  removed the `svg/parity/c4` module-level `clippy::too_many_arguments` allow.
- Bundled C4 layout recursion inputs and output state into `C4LayoutContext` /
  `C4LayoutState`, removing the `c4.rs` module-level `clippy::too_many_arguments` allow.
- Replaced architecture edge label geometry arguments, recursive group bounds arguments, and the
  render-model entry argument list with focused context structs, removing the
  `svg/parity/architecture.rs` module-level `clippy::too_many_arguments` allow.
- Replaced class marker defs helper argument lists with `MarkerContext` / `MarkerSpec`, removing
  the `svg/parity/class` module-level `clippy::too_many_arguments` allow.
- Replaced state RoughJS rectangle arguments with `StateRoughRectSpec`, removing the
  `svg/parity/state` module-level `clippy::too_many_arguments` allow and narrowing the requirement
  renderer call site to the same spec shape.
- Replaced vendored font-metric table argument lists with `FontMetricProfile`, removing the
  `text.rs` module-level `clippy::too_many_arguments` allow.
- Replaced flowchart label, node layout, recursive layout, place-graph, and cluster rect argument
  bundles with request/context structs, removing the `flowchart/mod.rs` module-level
  `clippy::too_many_arguments` allow.
- Replaced core flowchart semantic and state layout long-argument helpers with
  `FlowchartSemanticContext`, `TypedLayoutContext`, and `JsonLayoutContext`, and made
  `StateDb::add_state` merge `StateStmt` directly. Source code no longer carries
  `clippy::too_many_arguments` allows.
- Recorded an isolated Criterion spotcheck for the core flowchart/state context cleanup using
  `flowchart_medium` and `state_medium` in separate target directories.
- Removed the obsolete `render_layout_svg_parts_for_render_model` compat dispatcher and the
  no-config typed wrappers it exclusively served; typed render-model SVG dispatch now uses the
  `*_with_config` surface.
- Closed the render-only JSON clone cleanup batch after class, sequence, and render-model dispatch
  paths were reduced to intentional compatibility and lazy-sanitizer fallbacks.
- Removed the unused no-config class layout entrypoints so class note HTML measurement now keeps
  the parser's borrowed `MermaidConfig` through the typed path.
- Closed the flowchart/class/sequence hot-loop clone audit, leaving only compatibility, debug, and
  graphlib key ownership boundaries for future API-level work.
- Added `GATES.md` as the canonical refactor, parity, performance, and release gate reference for
  this workstream.
- Updated the root README architecture notes to describe the typed render-model path and the
  compatibility layout/render boundaries.
- Documented generated override parity as narrow Mermaid `@11.12.3` browser/export facts with
  explicit removal triggers.
- Added `TYPED_RENDERER_GUIDE.md` to document the standard checklist for new typed diagram renderer
  migrations.
- Simplified class layout namespace lookup by precomputing namespace parent/child pairs once per
  render pass and reusing the namespace declaration order vector across graph setup and cluster
  emission.
- Added `class_namespace_dense` to the pipeline benchmark fixture set and recorded the baseline in
  `docs/performance/spotcheck_2026-05-08_class_namespace_dense_layout.md`.
- Moved `c4` from JSON-fallback rendering to `C4DiagramRenderModel`.
- Removed the render-side C4 JSON transport structs; JSON layout compatibility now deserializes
  into the shared core render model before using the typed layout and SVG paths.
- Routed public `merman::render::render_svg_sync` C4 rendering through the typed render model and
  layout-only SVG emission.
- Added typed-model and public-render regression coverage for C4.
- Recorded the C4 typed render-path spotcheck in
  `docs/performance/spotcheck_2026-05-08_c4_typed_render_model.md`.
- Moved `xychart` from JSON-fallback rendering to `XyChartDiagramRenderModel`.
- Removed the render-side xychart JSON transport structs; JSON layout compatibility now deserializes
  into the shared core render model before using the typed layout path.
- Routed public `merman::render::render_svg_sync` xychart rendering through the typed render model
  and layout-only SVG emission.
- Added typed-model and public-render regression coverage for xychart.
- Recorded the xychart typed render-path spotcheck in
  `docs/performance/spotcheck_2026-05-08_xychart_typed_render_model.md`.
