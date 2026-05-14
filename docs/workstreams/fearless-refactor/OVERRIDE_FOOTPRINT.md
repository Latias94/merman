# Override Footprint

This file records the generated and manual parity override footprint so growth is visible during
fearless refactor work. Overrides are useful when upstream behavior depends on browser/font
measurement or a temporary raw SVG/path compatibility bridge, but new model fixes should be
preferred when the mismatch comes from our own data or geometry.

## Snapshot: 2026-05-14

Command:

`cargo run -p xtask -- report-overrides`

Mermaid baseline: `@11.12.3`

Generated override modules scanned: `21`.

Manual raw SVG/path bridge files scanned: `0`.

### Category Metadata Snapshot

`xtask report-overrides` now prints category-level owner, source, allowed-use, and expected-removal
metadata for every category, including zero-count categories. This keeps removal criteria and
successful eliminations visible in CI logs and drift reviews, not only in policy prose.

The same category totals are encoded as no-growth budgets in
`cargo run -p xtask -- report-overrides --check-no-growth`, which is part of the strict release
gate. Override growth should therefore be an explicit reviewed decision, not a default model-bug
escape hatch. Manual raw SVG/path bridges now have an exact zero budget, so any bridge
reintroduction fails the strict gate unless the budget is deliberately changed.

The current snapshot reflects a large net reduction in root viewport overrides after topology-
driven viewport calibration replaced several fixture-specific root pins, most `journey` root
viewport pins moved into deterministic renderer behavior, and profile-based `kanban` root height
calibration replaced the remaining fixture-specific Kanban root pins, followed by four obsolete
Sankey pins that now match deterministic emitted bounds and nine obsolete Timeline pins that now
match deterministic root output, then twelve obsolete Pie pins and twelve obsolete ER pins that
also match deterministic root output, thirty-five obsolete Requirement pins, and sixteen obsolete
C4 pins now covered by deterministic root output. It also reflects deletion of the now-empty Block root override module
after all 119 entries proved obsolete, followed by sixty-eight obsolete State pins now covered by
deterministic root output. It then collapses the Class root table from 196 entries to 31 by
removing 166 obsolete pins and adding one missing docs root pin, making Class `parity-root` green
with a 165-entry net reduction, followed by six obsolete Gitgraph pins now covered by
deterministic root output and thirty-two obsolete Sequence pins now covered by deterministic root
output. Sequence participant-type cursor derivation then removed 8 more root pins and refreshed the
affected layout goldens, reducing the Sequence root table from 200 to 192 entries. Flowchart then
dropped 131 obsolete pins and later cleared the
`upstream_docs_math_flowcharts_001` `parity-root` gap by normalizing the browser-sensitive math SVG
baseline and measuring sanitized KaTeX MathML through the Node probe. Pie then replaced its 23
remaining root pins with a typed empty-pie root viewport rule plus shared 1/64px-quantized legend
SVG bbox measurement, deleting the Pie root override module. Mindmap then refreshed typed root
profile calibration, added two small model-derived root profiles, and pruned 28 obsolete root pins
while keeping `parity-root` green. Class then moved its remaining 31 root viewport pins into typed
profile calibration and namespace render-mode rules, deleting the Class root override module while
keeping `parity-root` green. Architecture then added default root viewport calibration for the
nested-groups and reasonable-height profiles and pruned 70 obsolete fixture-scoped pins, leaving 31
Architecture root pins that still guard measured `parity-root` drift. ER, Requirement, State, and
Flowchart then moved empty-diagram root viewport behavior into renderer-derived empty content
bounds, deleting 21 more fixture pins while keeping the affected normal and `parity-root` DOM
filters green. A later full `parity-root` release-gate audit restored ten required root guards:
six Sequence long-message/frame fixtures, two tiny Journey browser float guards, and two GitGraph
viewBox-height guards. The root viewport budget was therefore `760`, and
`cargo run -p xtask -- compare-all-svgs --check-dom --dom-mode parity-root --dom-decimals 3`
passed. The follow-up root viewport derivation workstream then removed one State style-directive
root pin and two State Mermaid entity-placeholder edge-label root pins, so the current root
viewport budget is `757`. It then derived the Mindmap Cypress single-root square, rounded-rect,
and circle shape bounds from single-line delimiter label measurement and removed three more root
pins, tightening the current root viewport budget to `754`. A follow-up Mindmap pass kept plain
Mindmap label measurement on raw font metrics so cross-diagram HTML width overrides no longer
inflate the docs circle root, deleting one more root pin and tightening the current root viewport
budget to `753`. Later State/Mindmap/Sequence root-viewport derivation passes tightened the root
viewport budget to `702`; the latest Sequence long-note/long-message and wrapped-leftOf passes
fixed leftOf note start/width/rewrap behavior, removed fifteen more root pins, and kept the SVG
text metric table budget flat at `186` rows by replacing one stale `FRIENDS` row with the new
long-message fact.
A follow-up GitGraph disabled-root cross-check then found two then-stale fixture-scoped pins that no
longer appeared in the mismatch set. `upstream_cypress_gitgraph_spec_88_should_hide_branches_with_tb_orientation_when_showbranches_is_092`
and `upstream_direction_bt` passed focused `parity-root` without the lookup at that point, so the
GitGraph table dropped to `226` entries and the root viewport no-growth budget tightened to `616`.
The later seeded auto-id warm-up pass restored `upstream_direction_bt` because the corrected
dynamic commit id exposed a real BT-direction bbox guard.
A GitGraph title-bounds pass then included `gitTitleText` in emitted root bbox derivation, removed
13 title-dominated GitGraph root pins, and tightened the root viewport no-growth budget to `603`
with GitGraph at `213` entries.
The next Sequence pass propagated metadata/frontmatter titles into root bounds, removed the
now-derived `upstream_html_demos_sequence_sequence_diagram_demos_002` pin, and tightened the root
viewport no-growth budget to `602`.
The follow-up GitGraph `parallelCommits` layout audit fixed parentless LR branch roots continuing
from the previous branch's commit axis instead of restarting at the origin. That reduced the
focused unconnected-branches disabled-root drift from `+150.250px` to the remaining branch-label
bbox measurement drift, but did not remove a root pin because the final mismatch is still a real
browser text-measurement gap.
A later GitGraph branch-line endpoint pass included renderer-owned branch line endpoints in the
root bbox derivation, matching browser `getBBox()` for zero-length branch lines while leaving the
shared emitted-bounds scanner unchanged. The empty-graph package bucket dropped from roughly
`+34.750px` disabled-root width drift to residual branch-label bbox drift, but the GitGraph table
remained at `213` entries because `override=213 mismatch=213 stale=0 missing=0`.
The follow-up horizontal GitGraph branch-label pass switched LR/RL branch labels to computed text
lengths, kept TB/BT on the wider bbox path to avoid rotated dynamic commit-id root regressions,
and removed 57 now-derived GitGraph root pins, tightening the root viewport no-growth budget to
`545` with GitGraph at `156` entries.
The Flowchart imageSquare layout pass then sized Dagre nodes from rendered image plus label extents
instead of only the image asset, deleting the now-derived
`upstream_docs_flowchart_parameters_136` root pin. A follow-up Flowchart anchor pass modeled
Mermaid's label-ignoring roughjs anchor dot, deleting 12 old-shape set5 root pins while retaining
the one set5 `tb_md_html_false` entry that still has a real 0.06px root drift. A later C1
replacement-glyph measurement pass derived the courier long-name/class-definition Cypress root
viewport without a pin. A follow-up SVG-like subgraph-title pass shared the emitted SVG wrapping
helper with layout and sized default process nodes from wrapped computed text length, deleting the
stage2 long-word title root pin, and the Unicode/entities title root pin. A follow-up
disabled-root cross-check removed two stale subgraph title-margin root pins that now derive
without the lookup. A later font-size precedence pass split SVG root CSS font-size from HTML label
measurement and deleted the now-derived `stress_flowchart_font_size_precedence_073` root pin. The
latest iconSquare pass matched Mermaid's icon-shape outer layout bounds and deleted the
now-derived docs icon-shape root pin. A follow-up custom FontAwesome fallback pass modeled
Mermaid's unregistered `fab:fa-truck-bold` HTML-label behavior and deleted two now-derived custom
icon roots. At that point the root viewport no-growth budget was `523` with Flowchart at `103`
entries.
A later GitGraph seeded auto-id warm-up pass replayed upstream's parse-before-render seeded random
stream consumption before the render-model parse. This aligned dynamic commit ids with the
committed upstream SVG baselines, removed 26 net GitGraph root pins after retaining
`upstream_direction_bt` as a real BT-direction guard, and tightened the root viewport no-growth
budget to `497` with `130` GitGraph entries.
A later GitGraph commit/tag label pass measured commit ids and tag labels with GitGraph-owned
computed text lengths plus 1/64px quantization, avoiding the shared simple bbox path for these
short labels. The disabled-root audit found 65 retained DOM mismatches and 65 stale pins in the
previous 130-entry GitGraph table; deleting the stale pins tightened the root viewport no-growth
budget to `432` with `65` GitGraph entries.
A later Flowchart fork/join pass matched Mermaid `forkJoin.ts` direction-sensitive layout sizing
for LR-rendered graphs, deleting five now-derived old-shape set3 LR root pins and tightening the
root viewport no-growth budget to `427` with Flowchart at `98` entries. A follow-up disabled-root
cross-check found the set3 LR classdef, `md_html_false`, and styles siblings were also stale under
the same typed rule, tightening the current root viewport no-growth budget to `424` with Flowchart
at `95` entries.
A later GitGraph vertical branch-label pass matched Mermaid's `drawText(name).getBBox()` root
behavior for TB/BT labels by using the centered SVG bbox path with ties-to-even 1/64px
quantization. The disabled-root audit found 24 retained DOM mismatches and 41 stale pins in the
previous 65-entry GitGraph table; deleting those stale pins tightened the root viewport no-growth
budget to `383` with `24` GitGraph entries.
A later GitGraph commit/tag label theme-variable pass honored Mermaid's label-specific CSS
variables and measured commit and tag labels with separate font-size styles. Focused disabled-root
checks for the commit/tag font-size docs fixtures stayed green, deleting
`upstream_docs_gitgraph_customizing_commit_label_font_size_032` and tightening the current root
viewport no-growth budget to `382` with `23` GitGraph entries.
A later Sequence stale-pin sweep removed five simple-root pins, tightening the current root
viewport no-growth budget to `362` with `59` Sequence entries. The latest Flowchart table-only
cleanup then collapsed exact-duplicate generated root override match arms into Rust or-patterns.
It does not delete fixture-key coverage, but it removes redundant inventory rows and tightens the
current root viewport no-growth budget to `354` with `87` Flowchart entries.
It also reflects the final
manual raw SVG/path bridge removal, so manual bridge scanning now reports zero bridge files. It
also reflects corrected text-lookup accounting: generated `*_OVERRIDES_*` binary-search tables in
`block`, `er`, and `gantt` are now counted as text metric lookup entries instead of hand-curated
helper functions, and block-wrapped `=> { Some(...) }` text match arms are now counted as lookup
entries. Text lookup ownership has also tightened: ER and Block generated HTML width
tables are no longer consulted by the generic vendored text measurer, and their call sites now
live in their owning diagram modules. The stale Mindmap HTML width table was deleted after layout
snapshots proved the stable Mindmap path does not use it, reducing text lookup debt by 291 entries
while preventing shared text measurement from leaking fixture-specific widths across diagrams. C4
then moved its three per-line SVG bbox height rules into the C4 owner module and deleted the
generated `c4_text_overrides_11_12_2.rs` module, reducing text lookup debt by another 3 entries.
Gantt then dropped the generic `A`, `B`, `C`, `Build`, `Design`, `Noon`, `t1`, `task1`, `test1`,
and `test2` task-width overrides after the font-metric fallback proved stable, reducing text lookup
debt by ten more entries. A follow-up pass dropped `test3` through `test7`, reducing debt by five
more entries. A final pass dropped `task2` through `task4`, reducing debt by three more entries.
The isolated `y68` and `y69` labels then dropped, reducing debt by two more entries.
The duration labels `days`, `hours`, `minutes`, `ms`, and `seconds` then dropped, reducing debt by
five more entries. Nine small-fixture labels from leading-punctuation, callback, proto-id,
year-fallback, and 12-hour time fixtures then dropped, reducing debt by nine more entries. The
`task A` through `task D` labels from `relative_end_mixed` then dropped, reducing debt by four more
entries. The final broad Gantt task labels then dropped, deleting
`gantt_text_overrides_11_12_2.rs` and reducing debt by six more entries. C4 then moved its 17
type-line `textLength` pins into the C4 owner
module and deleted the generated `c4_type_textlength_11_12_2.rs` module, so C4 type-line
`textLength` now lives in owner code instead of the override inventory.
The hand-curated helper total also reflects pruning two redundant public Sankey padding component
helpers before the remaining Sankey node geometry moved back to the `sankey` owner module.
Since then, Pie inlined its fixed margin, center, radius, label font size, title y, and legend
text y literals, Sequence now derives its note padding total from the existing note gap, Journey
inlined its single-use legend placement and mouth offset values, Radar inlined its remaining
legend box size and label x-offset literals, XYChart inlined its bar data-label scale and inset
literals, deleting the empty generated override module, Sequence inlined its self-only frame min pad
literals in block geometry, Sankey inlined its SVG-only label font/gap/dy literals, Treemap
inlined its section header label/value sizing literals, Architecture deleted a dead icon text bbox
helper, and Radar inlined its final legend row spacing value and deleted the now-empty generated
module. Pie also moved its remaining legend rectangle/spacing values into `pie` owner constants and
deleted the now-empty generated module. Sankey then moved its node width/padding values into
`sankey` owner constants and deleted the now-empty generated module, bringing the hand-curated
helper total to 32. Journey moved its fixed viewBox/title/legend/face geometry into `journey`
owner constants and deleted the now-empty generated module, bringing the hand-curated helper total
to 26. Kanban moved its section padding, label foreignObject height, and item row heights into
`kanban` owner constants and deleted the now-empty generated module, bringing the hand-curated
helper total to 21. Treemap moved its section spacing geometry into `treemap` owner constants and
kept the remaining `Item A1` leaf-fit browser tolerance beside the SVG parity loop, deleting the
now-empty generated module and bringing the hand-curated helper total to 18. Sequence moved its
note wrap slack, text line-height math, and frame padding geometry into `sequence` owner constants
and functions, deleting the now-empty generated module and bringing the hand-curated helper total
to 12. Architecture moved its text bbox formulas, canvas-label width scale, service label
extension, and default wrap width into `architecture` owner constants/functions, deleting the
now-empty generated module and bringing the hand-curated helper total to 6. Gitgraph then moved
its branch-label correction control flow into the `gitgraph` owner module and reclassified its
bbox correction data as text metric lookup entries, bringing the hand-curated helper total to 0.
A later GitGraph branch-label pass deleted the 7-entry branch-label bbox correction table after raw
measured widths rounded to 1/64px preserved GitGraph DOM parity.
A later GitGraph commit-label pass deleted the 3-entry literal extra table after rounded measured
widths and existing edge-character corrections still preserved GitGraph DOM parity.
A later GitGraph glyph pass removed the left-side `2`, `6`, `5`, `C`, and `B` corrections after
the smaller correction table stayed green under GitGraph DOM parity.
A subsequent GitGraph glyph pass removed the right-side `C`, `D`, `B`, `0`, `6`, `4`, `a`, and
`d` corrections after the even smaller correction table stayed green under GitGraph DOM parity.
Requirement then dropped the paired `<<contains>>`, `<<satisfies>>`, `<<traces>>`,
`<<Requirement>>`, and `<<Element>>` HTML width/calc max-width lookups after both requirement
parity modes stayed green without them.
Requirement then dropped the paired `<<Functional Requirement>>` HTML width/calc max-width lookups
after both requirement parity modes stayed green without them.
Requirement then dropped the paired `<<Design Constraint>>`, `<<Interface Requirement>>`, and
`<<Physical Requirement>>` HTML width/calc max-width lookups after both requirement parity modes
stayed green without them. The paired `<<Performance Requirement>>` lookup was rechecked in the
same batch and kept because `parity-root` drifted on `upstream_cypress_requirement_spec_example_001`
from `551px` to `551.5px` without it.
Requirement then dropped the paired `Risk: High`, `Risk: Low`, and `Risk: Medium` HTML
width/calc max-width lookups after both requirement parity modes, the override budget, and
`verify --strict` stayed green without them.
Requirement then dropped the paired `Verification: Demonstration` and `Verification: Inspection`
HTML width/calc max-width lookups after both requirement parity modes, the override budget, and
`verify --strict` stayed green without them. `Verification: Analysis` still guards `basic` drift,
while `Verification: Test` was removed in the later recheck after both Requirement parity modes
stayed green.
Requirement then dropped the paired `Type: system` and `Type: test_type` HTML width/calc
max-width lookups after both requirement parity modes, the override budget, and `verify --strict`
stayed green without them. The paired `Type: simulation` lookup was rechecked in the same pass
and kept because simulation-heavy fixtures still drifted without it.
Requirement then dropped the remaining `ID:` HTML width/calc max-width lookup bucket after both
requirement parity modes, the override budget, and `verify --strict` stayed green without it.
Requirement then dropped the remaining `Doc Ref:` HTML width/calc max-width lookup bucket after
both requirement parity modes, the override budget, refreshed Requirement/relations layout
goldens, and `verify --strict` stayed green without it.
Requirement then dropped the paired `Text: A requirement` and `Text: Do thing` HTML
width/calc max-width lookups after both requirement parity modes, the override budget, and
`verify --strict` stayed green without them.
Requirement then dropped the paired `Text: the test text.` HTML width/calc max-width lookup after
both requirement parity modes, refreshed Requirement layout goldens, the override budget, and
`verify --strict` stayed green without it.
Requirement then dropped the paired `Text: base requirement` HTML width/calc max-width lookup
after both requirement parity modes, refreshed the affected Requirement layout golden, the
override budget, and `verify --strict` stayed green without it.
Requirement then dropped the remaining `Text: constraint text`, `Text: design constraint`,
`Text: functional requirement`, `Text: interface requirement`, `Text: performance requirement`,
and `Text: physical requirement` HTML width/calc max-width lookups, followed by all remaining
bold requirement title/entity-name lookups (`constructor`, `dc1`, `e1`, `elA`, `elB`, `elem`,
`myElem`, `myReq`, `req`, the `req_*` type names, `req1`, `req2`, `test_element`, `test_name`,
and `test_req`). Both Requirement DOM parity modes stayed green and the
`upstream_requirement_requirement_types_spec` plus `upstream_requirement_styles_spec` layout
goldens were refreshed, leaving
`requirement_text_overrides_11_12_2.rs` with four confirmed guard labels. A later recheck removed
the `Verification: Test` HTML width/calc max-width pair after both Requirement parity modes stayed
green, leaving only `<<Performance Requirement>>`, `Type: simulation`, and
`Verification: Analysis`. Flowchart then reduced `flowchart_text_overrides_11_12_2.rs` from 48
lookup entries to 45 confirmed guards after full Flowchart `parity-root` and focused text metric
assertions proved those bold/italic markdown, HTML width, and SVG bbox entries still guard upstream
behavior. GitGraph then deleted its 9-entry glyph correction module after the remaining boundary
corrections stayed green without it under the GitGraph DOM parity gate. Together with the
block-wrapped `Some` accounting fix, this left the text lookup total at 681. State then dropped four
rect-with-title span width/height lookups after both State DOM parity modes stayed green, leaving
the text lookup total at 677. A later State cluster-title pass removed three more width lookups
after both State parity modes stayed green, leaving the text lookup total at 674. A follow-up State
node/note label pass removed five more width lookups under the same parity gates, leaving the text
lookup total at 669. A quoted edge-label pass removed three more State width lookups under the same
parity gates, leaving the text lookup total at 666. A follow-up State edge-label pass removed four
more width lookups under the same parity gates, leaving the text lookup total at 662. A State style
label pass removed two more width lookups under the same parity gates, leaving the text lookup total
at 660. A follow-up ER no-attribute calcTextWidth pass removed seventeen zero-valued lookup entries
under the same ER parity gates, leaving the `drawRect` clamp guard in place and reducing the text
lookup total to 643. A later ER pass removed the single-letter entity labels `A` through `F` under
the same ER parity gates, reducing the text lookup total to 637. A follow-up ER pass removed
nineteen additional low-width no-attribute calcTextWidth lookups under the same ER parity gates,
reducing the text lookup total to 618. A later ER pass removed the short relation labels `has`,
`owns`, and `uses` under the same ER parity gates, reducing the text lookup total to 615. A later
ER pass removed the low-width relation labels `contains`, `hasMany`, `leads to`, `owned by`,
`parent`, `places`, and `relates` under the same ER parity gates, reducing the text lookup total to
608. A later ER pass removed the `insured for` relation label width lookup under the same ER parity
gates, reducing the text lookup total to 607. A later ER pass removed the `is teacher of`
relation label under the same ER parity gates, reducing the text lookup total to 606. A later ER
pass removed the remaining seven ER calcTextWidth lookup entries under the same ER parity gates,
reducing the text lookup total to 599. A later ER pass removed the `Author ref` HTML width lookup
under the same ER parity gates, reducing the text lookup total to 598. A later ER pass removed the
remaining six ER HTML width lookup entries under the same ER parity gates, reducing the text
lookup total to 592. A later ER pass removed three more ER HTML width lookup entries under the same
ER parity gates, reducing the text lookup total to 589. A later ER pass removed 21 more ER HTML
width lookup entries across alias, quoted-entity, standalone-entity, accessibility, attribute, and
pkgtests fixtures under the same ER parity gates, reducing the text lookup total to 568. A later
ER cleanup then removed the remaining fixture-specific ER HTML width lookups, leaving
`er_text_overrides_11_12_2.rs` with `string`, `varchar(5)`, and the `DRIVER` clamp guard and
reducing the text lookup total to 549. A follow-up bypass of all 3 entries still failed
`compare-er-svgs --check-dom --dom-mode parity-root --dom-decimals 3` on
`upstream_relationship_variants_spec`, so that 3-entry floor remains required. The stale
`xtask gen-er-text-overrides` command was removed after this file became hand-curated, and the
empty ER `calcTextWidth` lookup table was deleted.
A later Block pass removed the two blank HTML width lookup entries for `" "` and `"   "`,
reducing the text lookup total to 547. Later Class passes removed 44 lookup entries across the
exact `calcTextWidth` pass, the `uses` plain-label cleanup, the `OK` pair cleanup, and the
`ApiClient` cleanup with a dense layout golden refresh, followed by the `ERROR`, `Payment`,
`Cart`, `Server` rendered-width, `Dog`, `Mineral` calc, `Duck`, `Item`, `Order`, `Wheel`,
`connects`, `builds`, `parses`, `emits`, `feedback`, `returns`, `wraps`, `reads`, `depends`,
`owns`, `may-fail`, `references`, `int chimp`, `int gorilla`, `+int age`, `int id`, and
`int[] id` cleanups, the `+eat()`, `+mate()`, and `+run()` rendered-width cleanup, the later
`+quack()` / `+swim()` cleanup, the `+template()` cleanup, the `bar()` rendered-width cleanup, and
the `+isOk() : bool` rendered-width cleanup, reducing the text lookup total to 483. A follow-up
batch removed `+logout()`, `+start()`, `+addUser(user: User)`, `+request() : Response`, and
  `+query(sql: String) : Rows`, reducing the text lookup total to 477. A later Class
`parity-root` recheck restored the `+handle(req: Request) : Response`,
`+query(sql: String) : Rows`, and `+request() : Response` rendered-width guards because
`stress_class_styles_multiple_classdef_016` drifts from `890.25px` to `890.5px` without them,
bringing the text lookup total back to 480.

| category | owner | expected removal |
| --- | --- | --- |
| Root viewport overrides | render parity workstream | Delete entries once typed layout/emitted bounds can derive the same root viewport or a baseline upgrade removes the pinned behavior. |
| Text metric lookup overrides | render parity workstream | Delete entries once vendored/shared text measurement returns the upstream dimensions without fixture-specific lookup arms. |
| SVG text metric tables | render parity workstream | Replace with shared font metrics or browser-probe imports, then delete stale rows. |
| Font metric tables | shared text measurement owner | Regenerate or trim when better vendored font/probe data covers the drift; remove only if a real measurement backend becomes the default. |
| Hand-curated helper overrides | diagram renderer owner | Replace with repeatable generated data or typed model/layout computations as soon as a reliable source exists. |
| Manual raw SVG/path bridges | diagram-specific `svg/parity` module owner | Delete once typed layout/path emission reproduces the upstream literal behavior; keep local owner/removal notes beside each bridge. |

### Root Viewport Overrides

Total entries reported by `xtask`: `362`.

| file | entries |
| --- | ---: |
| `architecture_root_overrides_11_12_2.rs` | 31 |
| `c4_root_overrides_11_12_2.rs` | 35 |
| `er_root_overrides_11_12_2.rs` | 22 |
| `flowchart_root_overrides_11_12_2.rs` | 95 |
| `gitgraph_root_overrides_11_12_2.rs` | 23 |
| `journey_root_overrides_11_12_2.rs` | 2 |
| `mindmap_root_overrides_11_12_2.rs` | 39 |
| `requirement_root_overrides_11_12_2.rs` | 10 |
| `sankey_root_overrides_11_12_2.rs` | 3 |
| `sequence_root_overrides_11_12_2.rs` | 59 |
| `state_root_overrides_11_12_2.rs` | 34 |
| `timeline_root_overrides_11_12_2.rs` | 9 |

Sankey note: the remaining 3 root viewport entries were rechecked by disabling the Sankey root
lookup and running `compare-sankey-svgs --check-dom --dom-mode parity-root --dom-decimals 3`.
They still cover root height drift in
`upstream_docs_sankey_example_002`, `upstream_examples_sankey_energy_flow_001`, and
`upstream_html_demos_sankey_energy_flow_002`, so they are not redundant yet.

Small-bucket audit note: disabling the remaining Timeline, Requirement, and ER root lookups showed
their surviving entries still guard real `parity-root` drift. Pie was the exception; its root bucket
is now deleted after empty-pie viewport and legend bbox behavior moved into typed renderer logic.

Mindmap note: after the single-line delimiter and docs circle plain-label passes, disabling the
remaining Mindmap root lookup still leaves 47 `parity-root` DOM mismatches. Those entries stay in
the budget until their geometry/text profiles move into typed renderer logic. A follow-up
disabled-root audit with `--report-root-all` produced 113 root rows; the largest drift still came
from wrapping text, HTML sanitization, icon, shape, and long-label fixtures, including a
`+705.220px` width delta on the long-word wrapping fixture.

Gitgraph and Flowchart audit note: a 2026-05-09 recheck confirmed that disabling the Gitgraph
direct root lookup still leaves the broad Gitgraph root bucket failing. The full 2026-05-11
`parity-root` sweep restored two additional GitGraph viewBox-height guards for
`upstream_examples_git_basic_git_flow_001` and `upstream_merges_spec`. The Flowchart empty-diagram
pins are now deleted after empty bounds moved into renderer logic. The Flowchart imageSquare,
anchor, C1 replacement-glyph, SVG-like/Unicode subgraph-title, stale title-margin,
HTML-label font-size precedence, iconSquare, custom FontAwesome fallback, and LR fork/join
direction-sensitive layout passes removed 30
more pins. The remaining 95
entries still need root-viewport derivation work before table pruning, not another blind deletion
pass.
A follow-up GitGraph audit using `MERMAN_DISABLE_ROOT_VIEWPORT_OVERRIDES=1` plus
`--report-root-all` produced 251 root rows, with 239 non-zero `max-width` deltas and 241 changed
viewBox dimensions; the largest width deltas came from HTML demo merge graphs at roughly
`-281.558px`. Crossing the disabled-root mismatches with the GitGraph root table exposed two stale
retained pins, which were removed after focused and full GitGraph `parity-root` stayed green. The
GitGraph title-bounds follow-up removed 13 more title-dominated pins by deriving the title bbox in
the emitted root bounds. A later `parallelCommits` audit fixed the structural LR unconnected-branch
commit-axis restart bug, reducing the focused disabled-root width drift from `+150.250px` to
`+0.250px`, but the root entry remains because the residual mismatch is branch-label browser bbox
measurement. A later font-size precedence pass made GitGraph ignore top-level `fontSize` while
still honoring `themeVariables.fontSize` and top-level `fontFamily`, shrinking
`stress_gitgraph_font_size_097` from the large top-level-font drift to `+0.156px` without adding
overrides; the font-size stress pins stay because the residual mismatch is still branch-label
browser bbox drift. A later branch-line endpoint pass taught GitGraph root derivation to include
zero-length branch line endpoints, dropping the empty-graph package bucket from roughly
`+34.750px` to sub-pixel branch-label drift. A follow-up horizontal branch-label pass then used
computed text length for LR/RL branch labels, removed 57 now-derived root pins, and left
`override=156 mismatch=156 stale=0 missing=0`. A later commit/tag label pass moved GitGraph
commit ids and tags to computed text lengths with 1/64px quantization, exposing 65 stale retained
pins in the previous 130-entry table. A later vertical branch-label pass used the centered SVG
bbox path for TB/BT labels and exposed 41 more stale retained pins in the previous 65-entry
table. A later commit/tag label theme-variable pass deleted the docs commit-label font-size pin.
The remaining 23 GitGraph entries still guard real drift around dynamic commit-label, cherry-pick/tag,
height, and horizontal demo bounds, so the next pass should start from those root-delta families
rather than another broad deletion pass.
A follow-up Flowchart audit using the same disabled-root path and `--report-root-all` produced
1068 root rows, with 245 non-zero `max-width` deltas, 286 changed viewBox dimensions, and one
skipped fixture. After the anchor pass removed the old-shape set5 `+258px` cluster, the largest
remaining width deltas are long-name/class-definition roots, subgraph title wrapping, old-shape
set3 all-pairs, and icon/custom-icon fixtures. The remaining root pins need bounds and shape
derivation work rather than blind pruning.

Sequence note: a 2026-05-11 layout recalibration for participant-type actors removed 8 now-
redundant Sequence root pins after the matching layout goldens were refreshed. The later docs
boundary pass removed `upstream_docs_sequencediagram_boundary_008` by using the single-run Sequence
text-dimension path plus two measured message-width facts. A follow-up title/accessibility pass
corrected the default trailing-semicolon font-family widths for `Hello Bob, how are you?` and
`Hello John, how are you?`, removing `title_and_accdescr_multiline`,
`upstream_accessibility_single_line_spec`, and
`upstream_docs_accessibility_sequence_diagram_014` without growing the SVG text metric table.
The residual default-title pair `upstream_title_without_colon_spec` and
`upstream_pkgtests_sequencediagram_spec_020` was then removed under the same corrected
`Hello Bob, how are you?` fact. A simple note-right pass then deleted the `Bob thinks` trio
`upstream_pkgtests_sequencediagram_spec_007`, `upstream_pkgtests_sequencediagram_spec_009`, and
`upstream_pkgtests_sequencediagram_spec_042`, all of which now derive through the existing
note/message bounds without new SVG metric rows. The follow-up whitespace/comment trio
`upstream_pkgtests_sequencediagram_spec_043`, `upstream_pkgtests_sequencediagram_spec_045`, and
`upstream_pkgtests_sequencediagram_spec_046` was then deleted under the same existing bounds.
The simple block trio `upstream_pkgtests_sequencediagram_spec_054`,
`upstream_pkgtests_sequencediagram_spec_055`, and `upstream_pkgtests_sequencediagram_spec_056`
followed, covering loop, rect, and nested-rect `Bob thinks` cases. The alt-control trio
`upstream_pkgtests_sequencediagram_spec_058`, `upstream_pkgtests_sequencediagram_spec_059`, and
`upstream_alt_multiple_elses_spec` was then removed under the same existing bounds. Sequence
cleanup now has a narrower target: participant-type, boundary, and the first
title/default-title/simple-note roots are fixed, but long-message, long-note, and larger frame
expansion still need typed derivation work. A later full
`parity-root` sweep restored six more
required Sequence guards for
`upstream_break_spec`,
`upstream_docs_examples_sequence_diagram_blogging_app_service_communication_015`,
`upstream_docs_sequence_entity_codes_example`, `upstream_docs_sequencediagram_break_062`,
`upstream_par_multiple_ands_spec`, and `upstream_pkgtests_sequencediagram_spec_063`.
A 2026-05-13 follow-up Sequence audit after the layout-owner decomposition used
`MERMAN_DISABLE_ROOT_VIEWPORT_OVERRIDES=1` plus `--report-root-all` and produced 320 root rows, 73
non-zero `max-width` deltas, 80 changed viewBox dimensions, and 80 DOM mismatches. A later
frontmatter-title pass made the rendered metadata title participate in Sequence root sizing and
removed `upstream_html_demos_sequence_sequence_diagram_demos_002`. Later message and actor-width
fact corrections removed the stacked-activation, arrows-variant, simple Cypress, package sequence,
and six docs/control root pins. A follow-up disabled-root mismatch cross-check found five stale
retained simple-root pins and no missing pins, so those were deleted. The remaining Sequence table
is now `59`. The
participant-creation v2 sibling still drifts from upstream `1040x580` to local `1040x591` with
root overrides disabled, so it remains a typed participant vertical-geometry target rather than a
text-width cleanup. The remaining Sequence table should be reduced by typed bounds/text
measurement work rather than by another blind deletion pass.
Journey note: the table was reintroduced with two tiny browser-float root guards after full
`parity-root` exposed `0.125px` and `0.109375px` root width drift in the two long-label Cypress
fixtures. The renderer still derives normal Journey root behavior; these two entries are
fixture-derived parity guards, not a return to a broad Journey root table. A follow-up audit tried
1/16px actor-label width quantization: it fixed the whitespace fixture but left
`upstream_cypress_journey_spec_should_wrap_long_labels_into_multiple_lines_keep_them_under_max_010`
drifting from upstream `1937.125px` to local `1937.25px`, so the two-entry table stays until
Journey actor-label browser measurement is modeled more precisely.

State root note: after the empty-diagram cleanup, a disabled-root audit with `--report-root-all`
produced 283 root rows, with 125 non-zero `max-width` deltas and 125 changed viewBox dimensions.
The largest drift came from right-to-left direction/scale fixtures and dense edge-label fixtures,
including `-180.563px` on `stress_state_direction_rl_scale_and_long_ids_054`, so the remaining
State root pins need scale/direction and edge-label bounds work before another pruning pass.

Largest root-viewport buckets:

- `flowchart`: 95
- `sequence`: 59
- `mindmap`: 39
- `c4`: 35
- `state`: 34
- `architecture`: 31
- `gitgraph`: 23

### Text Metric Lookup Overrides

Total lookup entries reported by `xtask`: `484`.

| file | lookup entries |
| --- | ---: |
| `block_text_overrides_11_12_2.rs` | 123 |
| `class_text_overrides_11_12_2.rs` | 277 |
| `er_text_overrides_11_12_2.rs` | 3 |
| `flowchart_text_overrides_11_12_2.rs` | 45 |
| `requirement_text_overrides_11_12_2.rs` | 6 |
| `state_text_overrides_11_12_2.rs` | 29 |
| `timeline_text_overrides_11_12_2.rs` | 1 |

GitGraph note: the 9-entry glyph correction module was deleted after
`compare-gitgraph-svgs --check-dom --dom-mode parity --dom-decimals 3` stayed green with all
remaining left/right boundary corrections removed.

Class note: the standalone plain-label `uses` lookup was removed after
`compare-class-svgs --check-dom --dom-mode parity-root --dom-decimals 3` stayed green without it,
and the now-empty plain-label bridge was deleted. Later `OK`, `ApiClient`, `ERROR`, `Payment`,
`Cart`, `Server` rendered-width, `Dog`, `Mineral` calc, `Duck`, `Item`, `Order`, `Wheel`,
`connects`, `builds`, `parses`, `emits`, `feedback`, `returns`, `wraps`, `reads`, `depends`,
`owns`, `may-fail`, `references`, `int chimp`, `int gorilla`, `+int age`, `int id`, `int[] id`,
`+eat()`, `+mate()`, `+run()`, `+quack()`, `+swim()`, `+template()`, and the `bar()` rendered-width
and `+isOk() : bool` rendered-width cleanup passes refreshed the affected layout goldens as needed
and reduced the Class text lookup total to 280 and the global text lookup total to 483. The
follow-up `+logout()`, `+start()`, `+addUser(user: User)`, `+request() : Response`, and
`+query(sql: String) : Rows` rendered-width cleanup passes refreshed the affected layout goldens
as needed and reduced the Class text lookup total to 274 and the global text lookup total to 477.
The later Class `parity-root` recheck restored `+handle(req: Request) : Response`,
`+query(sql: String) : Rows`, and `+request() : Response`, then refreshed
`stress_class_styles_multiple_classdef_016.layout.golden.json`; Class text lookups are now 277 and
the global text lookup total was 480. The State root viewport derivation workstream then added one
shared edge-label browser metric for `test({ foo: 'far' })`, raising State text lookups to 26 and
the global text lookup total to 481 while removing two fixture-scoped State root pins.
The `test()` rendered width entry stays pinned because deleting it preserves Class DOM parity but
causes broad default-layout churn across 14 simple Class cypress fixture goldens.
The `DB` and `Server` `calcTextWidth` entries stay pinned because
`class_svg_annotations_and_comment_rows_keep_mermaid_html_caps` still asserts their Mermaid HTML
`max-width` caps.
The `Data` method entries `+toString() : String`, `+fromCode(int code) : Data`, and
`+parse(String text) : Data` stay pinned even though those exact strings are not present in raw
fixture text: Class parsing normalizes source rows into those display keys, and deleting the
calc/rendered pairs breaks Data annotation geometry plus focused HTML cap assertions.
The `uses` rendered width entry also stays pinned because removing it preserves Class DOM parity
but fails `compare-class-svgs --check-dom --dom-mode parity-root --dom-decimals 3` on
`stress_class_unicode_namespace_mix_017`, shifting the root `max-width` from `409px` to `409.25px`
and forcing four Class layout goldens to refresh.
The `Mineral` rendered width entry also stays pinned because deleting it shifts the upstream root
`max-width`.
The `Fish` rendered width entry also stays pinned because deleting it shifts
`upstream_docs_classdiagram_class_diagrams_002` root `max-width` from `902.75px` to `903px`.
The `Zebra` rendered width entry also stays pinned because deleting it shifts the root
`max-width` for docs/basic inheritance fixtures by `0.25px` to `0.5px`.
The `Person` rendered width entry also stays pinned because deleting it shifts root `max-width`
by `0.5px` in the char-sequence, diagram-orchestration, and relation-types Class fixtures.
The `Driver` rendered width entry also stays pinned because deleting it shifts the relation-types
fixture root `max-width` from `1704.25px` to `1704px`.
The `User` rendered width entry also stays pinned: removing it preserved the broad Class DOM and
layout snapshot gates, but `class_svg_namespaces_and_relation_labels_keep_upstream_geometry`
failed on the `Company.Project` cluster geometry during strict verification.
The `manages` rendered width entry also stays pinned for the same geometry reason: removing it
breaks `class_svg_namespaces_and_relation_labels_keep_upstream_geometry` on the `Company.Project`
cluster geometry during strict verification.

ER note: the remaining ER text lookup entries are the `string` and `varchar(5)` width lookups plus
the `DRIVER` drawRect clamp guard. They were rechecked after the latest cleanup, both ER DOM
parity modes stayed green, and the ER layout goldens were refreshed. Individual removal attempts
for `string`, `varchar(5)`, and `DRIVER` still failed
`compare-er-svgs --check-dom --dom-mode parity-root --dom-decimals 3`, so the 3-entry floor stays
in place.

State note: the single diagram-title bbox lookup for `Simple sample` was rechecked by disabling
the lookup. Standard State DOM parity stayed green, but `parity-root` dropped the root `max-width`
from `132.25px` to `132px` on the docs/frontmatter title fixtures, so the lookup still guards root
title sizing drift. A later rect-with-title pass removed the `this is another string`,
`Accumulate Enough Data\nLong State Name`, and `Just a test` width lookups plus the `Just a test`
height lookup after both State parity modes stayed green. A cluster-title pass then removed the
`Configuring`, `NewValuePreview`, and `NotShooting` width lookups under the same two State parity
checks. A node/note label pass then removed `Idle`, `Moving`, `LOG`, `ACT`, and
`this is a short<br/>note` after both State parity modes stayed green. A quoted edge-label pass then
removed `New Data`, `Succeeded`, and `Succeeded / Save Result` under the same two checks. The
next edge-label pass removed `EvCapsLockPressed`, `EvNumLockPressed`, `EvConfig`, and
`EvNewValueSaved1` under the same two checks. A style label pass removed `fast` and `slow`, while
`id3`/`id4` still guard `upstream_state_style_spec` root sizing. The
remaining
`this is a string with - in it` width lookup and the multiline title height lookup still guard root
drift when disabled.

### SVG Text Metric Tables

| file | table rows |
| --- | ---: |
| `svg_overrides_sequence_11_12_2.rs` | 186 |

### Font Metric Tables

| file | table rows |
| --- | ---: |
| `font_metrics_flowchart_11_12_2.rs` | 3774 |

### Hand-Curated Helper Overrides

Total helper functions reported by `xtask`: `0`.

No hand-curated helper override modules remain.

### Manual Raw SVG/Path Bridges

Total bridge functions reported by `xtask`: `0`.

No manual raw SVG/path bridges remain.

### Counting Notes

Counts are inventory units and should not be compared directly across categories:

- Root viewport entries count match arms returning `Some((viewBox, max_width))`.
- Text metric lookup entries count `=> Some(...)` parity branches and rows in generated
  `*_OVERRIDES_*` binary-search tables.
- Font/SVG metric table rows count tuple rows in generated lookup arrays.
- Helper overrides count public helper functions in generated small constant modules.
- Manual raw SVG/path bridge counts are hand-authored `maybe_override_*` functions under
  `crates/merman-render/src/svg/parity/`.
- Category metadata in the report records owner, source, allowed use, and expected removal criteria
  for every generated/manual override category.

## Categories

- Root viewport overrides: fixture-derived `viewBox` / `max-width` pins for browser float and
  emitted-bounds drift. For local audits, `MERMAN_DISABLE_ROOT_VIEWPORT_OVERRIDES=1` disables the
  shared application helper so a compare command can prove which root entries still guard real
  drift.
- Generated text metrics: per-diagram width/height/bbox constants or lookup tables derived from
  upstream browser measurements.
- Raw SVG/path precision bridges: temporary hand-authored geometry/path literal bridges tracked by
  `xtask report-overrides` when named `maybe_override_*`.
- Hand-curated constants: small stable constants such as sequence frame/text spacing helpers.

## Known Gaps

- Manual bridge scanning is intentionally naming-based today: any future hand-authored bridge
  functions must use `maybe_override_*` under `crates/merman-render/src/svg/parity/` to be visible.
- Generated override metadata is category-level. Per-entry fixture/probe provenance still lives in
  generator inputs, generated comments, tests, and upstream fixture names.

## Next Actions

- Keep any future temporary raw SVG/path bridge functions named `maybe_override_*` and documented
  with owner/removal notes.
- Review the largest root-viewport buckets before adding new entries, especially `flowchart`,
  `gitgraph`, `sequence`, `state`, and `mindmap`. Use `--report-root-all` or
  `--report-root-limit <n>` on the supported Flowchart, GitGraph, Mindmap, Sequence, and State
  compare commands when the default top-25 root delta report is too small for a full bucket audit.
- Do not spend another cleanup pass on the 3 remaining Sankey root pins until Sankey root height
  derivation changes; the May 2026 recheck proved they still guard real `parity-root` drift.
- Do not spend another blind table-pruning pass on Gitgraph or Flowchart root pins until their root
  viewport derivation changes; the May 2026 recheck proved both buckets still guard real
  `parity-root` drift.
- Do not spend another blind table-pruning pass on Sequence root pins until sequence typed bounds
  change; the May 11 representative recheck proved multiple common buckets still guard real
  `parity-root` drift.
- A quick deterministic spot-check of several unmentioned Class labels (`Class1`, `OneA`, `A1`,
  `A2`, `Status`, `Foo`, `API`, `C1`, `Beta`, `Core.Alpha`, `B1a`, `Class01<T>`,
  `IRepository<T>`, `GenericClass<T>`, `CoreResult<T>`, `ApiRequest`) did not surface a new
  exact-match prune candidate, so Class cleanup should stay evidence-driven rather than blind.
- Tighten per-entry fixture/probe provenance when regenerating large override tables.
