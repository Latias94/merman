# Model/source boundaries implementation record

Status: M0 complete; M1–M7 implementation integrated, with final validation in progress under M8, 2026-10-30. This records implementation of the [approved plan](../plans/2026-10-30-model-and-source-boundaries.md), [family inventory](../plans/2026-10-30-family-data-inventory.md) and [ADR-0099](../adr/0099-model-and-source-boundaries.md). Open plan checkboxes remain requirements, not completed claims.

## Frozen source and observations

- Production base: `a5c73612fb2288e5effdbb07b3e1c3e683968603`, including the tested dependency-review prerequisite `8888358b1873c89049c0e430615c91d406dac224` and documentation only after it.
- Remote main: `4e9fa48139361bb30f81cb86d413597643978a4f`, fetched and inspected on 2026-10-30; no newer source commits needed integration at M0.
- Worktree: `merman-deps-review`, branch `plan/fearless-model-boundaries-20261030`. The primary checkout's unrelated untracked fixture is outside this work.
- Local experiment registration: shared ignored `target/bench/experiments/model-source-boundaries/experiment.yaml`. Receipts retain actual source, fixture, lock, profile, target and toolchain identities.
- Independent consumer fixtures were committed as `2e71bfa333b5e9ae6dcee41bdad2be81c162e444` before production edits. Direct-model, model-to-ASCII and source-to-ASCII baseline receipts passed first. Remaining SVG/analysis builds run serially against independent archives of the same immutable production base while M1 edits are prepared; no production commit is admitted before all six base receipts pass. This overlaps editing with archived baseline collection without changing its source, inputs or build recipe.

## Execution record

| Slice | Status | Evidence |
| --- | --- | --- |
| M0 | Complete | All six independent consumers compiled and passed both runtime inputs, repeated-output checks and raw/stripped equivalence. |
| M1 | Implemented; final gates pending | Shared lifecycle/config/runtime/resources/text and the pinned data-only identity catalog moved. Source provider bindings remain core-owned; forwarding retirement is integrated. |
| M2 | Implemented; final gates pending | All 34 canonical payloads and nine real full-data groups moved. Six redundant wrappers are retired. Post-retirement core/model all-feature tests passed 2399 cases; the strict isolated producer case remains part of M8. |
| M3 | Implemented; final gates pending | ASCII production uses model directly. The fixed model-to-ASCII checkpoint retains exact primary/fallback/report outputs; final consumer measurements and product recipes remain. |
| M4 | Implemented; final gates pending | Default-off requested fixes and immutable evidence are integrated into CLI/LSP/bindings. Semantic-only recipes passed 287/322 cases and the combined all-feature product libraries passed 937 cases; final exact recipes remain. |
| M5 | Implemented; final gates pending | Single markup owner, checked immutable presentation and parser-free Render/Export closures are integrated. Render's full library passed 3303 cases; final focused boundary regressions, integration tests and DOM owners remain. |
| M6 | Implemented; final gates pending | All 46 artifact profiles and closure checks pass. Product recipes, docs and legal projections are migrated; actual package/installed consumers and final recipe execution remain. |
| M7 | Implemented; final gates pending | Public core model namespaces, old wrappers and source-to-render private bridges are retired. The family inventory records the retained output contracts; acceptance-cfg callers are migrated and await execution. |
| M8 | In progress | Final feature, DOM, Linux, source-package, installed-product and measurement owners remain required. No publication has run. |

Existing correctness results at the prerequisite retain their original scope in the [dependency review](../research/2026-10-30-dependency-review.md). New consumer baselines and implementation gates will be recorded here as they complete. Do not infer a runtime improvement from the package boundary or rewrite historical measurement hashes.

## M0 consumer baseline

All lanes use Rust 1.95.0 on `x86_64-pc-windows-msvc`, empty dependency defaults and their committed fixed feature recipe. The runner uses release `opt-level=s`, thin LTO, one codegen unit, abort-on-panic, no debug info and no incremental compilation. Builds ran sequentially with one job and the shared development target. External counts are distinct resolved normal/build package versions, including proc-macro packages and excluding path dependencies; they are not linked-library or deployed-service counts. The M3 recount corrected earlier summaries that omitted rows marked `(proc-macro)`: model 74→82, model/ASCII and source/ASCII 76→84, SVG 105→114, diagnostics/fixes 79→87, and the partial M2 model candidate 16→18. Raw receipts, inputs, output hashes and byte observations are unchanged; `dependency-count-reconciliation.json` preserves both count definitions. These Windows receipts are distinct from the dependency review's representative Linux-target graph.

| Fixed consumer | External versions | Raw release bytes | Stripped bytes |
| --- | ---: | ---: | ---: |
| `boundary-model` | 82 | 540672 | 540672 |
| `boundary-model-ascii` | 84 | 3875840 | 3875840 |
| `boundary-source-ascii` | 84 | 3790336 | 3790336 |
| `boundary-presentation-svg` | 114 | 7020032 | 7020032 |
| `boundary-analysis-diagnostics` | 87 | 2151936 | 2151936 |
| `boundary-analysis-fixes` | 87 | 2159104 | 2159104 |

Receipts are under ignored `target/bench/experiments/model-source-boundaries/base-<recipe>/receipt.json`; the experiment ledger indexes all six actual runs and output hashes. Each executable consumed `alpha.json` and `beta.json`, and raw/stripped outputs agreed. The eight Python staging/receipt contracts passed before collection. Raw and stripped byte counts happen to match for these PE executables; both files and the stripping-tool identity are retained.

This baseline establishes no improvement. The [fixture coverage boundaries](../../tools/bench/fixtures/model-source-boundaries/README.md#coverage-boundaries) remain explicit: manual State lacks the baseline's private document builder, graphical construction still uses core parsing, diagnostics still pays eager fix generation, and source-to-ASCII compiles the fixture's unused model construction code. Final independent presentation/export admission and full model/feature/security owners remain required beyond these fixed observations. No cold-compile timing result has been collected yet.

## M1 extraction decisions

Shared operation/control, runtime policy/context, civil time/time-zone values and `SourceSpan` have no required source-parser dependency. Core keeps its ambient parse-context TLS and source-span scanning helpers; moving shared values does not require publishing those private orchestration functions. The operation's observed-cancellation path stays distinct from a new checkpoint, preserving writer I/O error ordering.

The configuration value and iterative JSON lifecycle move with their controlled merge/clone/drop operations. Source filtering becomes core functions around that shared value. Core's presentation journal consumes a documented merge mutation observer; it does not regain access to private storage. Cross-package test-support counts actual COW events so existing appearance/theme tests retain their assertions instead of replacing them with constant observations.

Pure lifecycle/runtime/time tests follow their types. Parse terminal/deadline and TLS restoration tests stay core-owned, with only the necessary feature-gated fake-clock/runtime test entrypoints across the package boundary. Adapter implementations and their capability query move together; core feature selection forwards to the actual shared owner.

Mindmap's compatibility projector currently derives its diagram ID from core's ambient context. Its M5 projection migration must receive an explicit operation context or frozen ID fact; the source-only TLS must not migrate into the model package to preserve this incidental call path.

## M1 shared-contract checkpoint

The initial package now owns operation control and sticky terminals, runtime policy/context and adapters, civil time/time-zone resolution, `SourceSpan`, `MermaidConfig`, iterative JSON ownership and resource-profile identity. Core retains source-policy functions, ambient parse context, scanning helpers and source-specific lifecycle tests. Theme binding stores the shared configuration value directly rather than opening its private backing field. Temporary core forwarding keeps this commit buildable and remains explicitly scheduled for M7 deletion.

Validation on the shared Windows target, with ordinary builds limited to two jobs:

- Core plus model with all source families, compatibility JSON, editor facts and terminal text: 2351 nextest tests passed (run `3e5f9fad-b64e-4056-8dcc-13a3848ede5f`).
- Model with every current system adapter and test-support feature: 56 tests passed (run `2425fc46-ffa6-4c6f-a44f-a3e007255578`).
- Focused core runtime order/restoration after adapting the assertion to the shared owner's compiled capability query: two tests passed (run `03acd0a4-77fa-4df1-8256-97f57a24de08`). These runs overlap and are not summed into a distinct test count.
- Facade `all-diagrams,svg,ascii,analysis` with defaults disabled compiled successfully, covering the current output and analysis consumers of the moved contracts.
- The existing artifact-profile owner validated all 43 exact recipes, including the new parser/output-free `rust-model` library profile. Its absence expectations remain separate from data selection.
- Package-scoped formatting and production-library Clippy passed with `-D warnings` for the same all-family core recipe.
- A bounded independent static review found no correctness blocker or lost operation/config tests. Core's parse-specific four operation tests and source authorization checks remain at their source owner.

The new package's project licenses and Mermaid source notice were projected through the existing legal owner; no dependency versions changed in this checkpoint. Full legal/report regeneration, source packaging and final closure admission remain M6/M8 work. Configuration/source namespace forwarding is temporary. Family data, shared family identity and model-dependent resource-policy methods remain implementation work; this checkpoint does not close M1 or claim a model-to-ASCII dependency reduction yet.

## M2 family extraction

The first data group contains Error, Architecture, C4, Cynefin, Event Modeling, QuadrantChart, Requirement, Sankey, Venn and Wardley: ten of the 34 canonical payloads. Their 53 public data/vocabulary definitions have one implementation under `merman_model::families`; the nine selectable languages use independent model features, while Error data remains unconditional. Shared warning facts and stable rule IDs also have one model owner.

Core retains each grammar, DB, source scanner, sanitization policy, diagnostics and editor construction. Family sanitizers are source functions accepting shared data. Architecture's source direction recognizer stays in its parser while its pure direction vocabulary/accessor moves. Canonical serde attributes, field order and defaults are retained; no built-in model is recovered through JSON or source replay. Temporary core data re-exports remain scheduled for M7 retirement.

The default model package still excludes family data unless selected and does not advertise a parser. Core forwards only its selected families. Existing source-based family tests remain core-owned and exercise those exact moved types. Model-only selection and all-family source behavior are validated separately; this partial extraction does not complete the full inventory or source/output dependency cuts.

The first family-group source/model run passed all 2351 tests (nextest run
`4e8c5427-335f-48c6-9c0f-c6170390575c`) under the same all-family core recipe as M1. Default model
selection independently passed 50 shared-value/lifecycle/time tests before that run. Broader final
family/output admission remains open.

The first group's model-only production graph excludes core, YAML/JSON5, HTML5 recovery, URL and CSS
admission. Its all-features model run passed 56 tests (`3d96d9ab-096f-4991-a5a4-8d9925138567`);
production-library Clippy with `-D warnings`, formatting and the facade
`all-diagrams,svg,ascii,analysis` consumer check passed. No external package version changed.

## M2 Flowchart, Class and ER checkpoint

Thirteen of 34 canonical payloads now have a model-package owner. Flowchart's public model and
subgraph CSS provenance move separately from its parser-private node/edge/syntax records. Pure
Flowchart render labels/context are `presentation-data`. Class keeps one canonical model, one real
full presentation view and controlled borrowed topology; ER retains its canonical graph and exact
alias/label selection. Core sanitizers and original-source adapters remain source-owned.

Class DB retains private mutable style observations and selects first witnesses at the same events;
it passes the completed four-witness set to the shared facts constructor at model construction.
Witness payloads retain private storage and read accessors. These are declared data APIs and do not
certify a caller's parse history. The Class vocabulary constants have one shared owner. Minimal
Class entity decoding is defined once in the shared text module and reused by core and full Class
views.

Generic `ModelComplexity` construction, Serde counting and iterative JSON traversal now belong to
model. Core's parsed/full/semantic enum adapters are ordinary source-owner functions. Existing
source/output consumers call those adapters while their data dependencies migrate; no foreign
inherent impl or duplicate complexity algorithm is introduced. Shared JSON counts preserve raw
cardinality, logical versus container depth, saturation and bounded scratch behavior.

The combined core/model nextest run passed 2354 tests (`008b070b-996e-4bfd-b3fb-6c1049afcfb9`).
Agentflow-only, Class-only and empty core checks passed. The bounded independent review found no
correctness blocker in witness capture, COW/context pairing, complexity counts or caller migration.
State's flat-document construction boundary is integrated in the same implementation batch.

## M2 State completed-document boundary

State raises the canonical data inventory to fourteen of 34 payloads. The model package owns the
flat document/statement/ID types, canonical and actual full node/edge/model data, borrowed terminal
views, flat serializers and pure complexity. Grammar, DB, render preparation and Mermaid-JSON
expansion/routing remain source-owned. The grammar takes `StateDocumentScratch`; the existing
LALRPOP generator refreshes the checked-in parser and its source digest.

`StateDocumentBuilder` separates mutable construction from completed immutable storage. Its
controlled completion validates root, child indices, unique ownership and reachability with iterative
traversal. State records may continue sharing references into the completed tree. Private ID storage
and document fields remain private, with allocation-order reads and completed-tree accessors.
Cancellation is checked before validation failure and during traversal. Private source-construction
invariant failures use the internal error channel; original grammar diagnostics retain their own
channel. New boundary controls cover invalid roots/children/cycles/duplicates/unreachable documents,
record sharing, borrowed canonical/full data, deep clone/drop/statistics and cancellation.

The completed-tree validation adds linear work and scratch proportional to the document count. It
establishes a real cross-package construction invariant; no runtime speedup is attributed to it.
Original source recovery/concurrency and deep lifecycle tests remain required. The combined
core/model run passed all 2362 tests (`7e0e1883-51a0-41d7-9091-70aee38b95d9`). Model all-features
passed 79 tests (`e3d6b2e1-0738-4ce7-8e11-4e999b770ef6`), production-library Clippy passed with
`-D warnings`, and facade `all-diagrams,svg,ascii,analysis` compiled. Existing LALRPOP generation and
verification passed. A bounded independent review found no State tree/lifecycle/error blocker.
All 1397 ASCII runtime tests passed (`7eed2dbd-3129-4121-b4e9-c658b250103b`), including terminal safety, fallback, cancellation and resource owners. The eight fixed-consumer staging/receipt tests also passed after migrating the model fixture imports. A committed independent model consumer observation follows; these are partial implementation results rather than completion of M2 or the whole plan.

## Partial independent model consumer result

The committed `17f1a112bf7a25fd56ae0597b7977880b34648b8` library and migrated fixture were compared with
the original M0 source/fixture receipts using `boundary-model --expect-receipt`. The two runtime
inputs and exact consumed Flowchart/Class/State outputs match. Target, toolchain, profile and fixture
feature are unchanged; only the model import/dependency owner migrated.

| Observation | M0 base | M2 checkpoint |
| --- | ---: | ---: |
| External normal/build package versions | 82 | 18 |
| Raw release executable bytes | 540672 | 541184 |
| Stripped executable bytes | 540672 | 541184 |

The independent production tree excludes core, Mermaid grammar, YAML/JSON5, HTML5 recovery, URL
normalization and CSS admission. It retains Serde/JSON, ordered maps, hash and error/proc-macro
dependencies used by these types. No external package version was added to the matched consumer.
The graph reduction is real; the executable is 512 bytes larger (0.0947%), so this is not a binary
size improvement. No compile-time or runtime speed claim follows from the package count.

Raw evidence is retained at ignored `target/bench/experiments/model-source-boundaries/`:
`m2-boundary-model/receipt.json`, `m2-model-consumer-comparison.json` and
`m2-graph-family-validation.json`. This result covers the fixed direct-model consumer only. ASCII,
graphical/export and diagnostics closures, the remaining twenty payloads and M6–M8 final admission
are still open; it does not substitute for those owners.

## M1/M2 terminal-family and admission checkpoint

Sequence, Gantt, GitGraph, TreeView, ZenUML, Mindmap and XY bring the canonical inventory to
21 of 34 payloads. The shared owner retains Sequence message constants, lifecycle fallback,
borrowed compatibility serialization and deep JSON accounting; Gantt's actual full tasks and
borrowed terminal metadata; Mindmap's canonical and full data, explicit display rules and
borrowed terminal traversal; and XY's canonical and full display policy. Source grammar, DBs,
date resolution, sanitizers, seeded IDs, warning remapping and Mindmap projection construction
remain core-owned. TreeView keeps its existing nested representation.

Gantt's private `GanttOutputData` records Empty versus Model independently of `compat-json`.
Full source construction always carries that declared fact; manually constructed and
deserialized data defaults to Model. Its producer test explicitly covers presentation without
the serializer feature. This is output shape evidence, not proof of source admission.

`InputResourcePolicy` and its descriptors, override/restriction errors and generic source-byte
and model checks now have one model-package owner. Core's checks over parsed/source enum
values use source-owned complexity adapters with the shared scalar admission method instead
of inherent methods on a foreign type. Pure Class, Flowchart, Sequence, ZenUML and Mindmap
statistics move with their data. Terminal normalization and streaming character-reference
decoding likewise have one shared text owner, selected by optional `terminal-text` and
`html-entities` features. The default direct-model graph does not enable those text dependencies.

Validation used serial Cargo builds with two jobs and the shared Windows target. The selected
Sequence/Gantt/GitGraph/TreeView/ZenUML/Class/Flowchart recipe passed 1261 tests
(`e42d1c94-46e2-40cd-a50c-4abac939aca9`). Adding Mindmap/XY and the moved shared text owners
passed 1364 tests (`443a1906-3521-4ce6-8dce-e4ed5d541be7`). These counts overlap. Both recipes
included presentation, compatibility JSON, editor facts and terminal text. The same selected
families compiled through facade SVG, ASCII and analysis with defaults disabled; existing
unused-code warnings in this subset are not a Clippy admission result. Model formatting passed.
Remaining-family and whole-matrix tests, parser-free ASCII/render/export closures and final
measurement/legal/package gates remain open. No new size or performance result is claimed.


## Complete canonical-data and parser-free ASCII checkpoint

All 34 canonical payloads now have a model-package owner. Agentflow, Usecase, Railroad,
Ishikawa, Block, Kanban and Treemap retain their original meaning and pure data/lifecycle behavior.
The nine actual full-data groups and fourteen terminal adapters use their family owners. Info,
Pie, Journey, Timeline, Packet and Radar use canonical payloads directly, paired with small
`presentation-data` output declarations in the full context. `PacketRenderBlock` is replaced by
`PacketBlock`, without an alias. Canonical serde requirements are retained rather than widened
with defaults from the retired wrappers.

The semantic enum, declared metadata, pure aggregate/family statistics and pinned shape vocabulary
have shared owners. Source remapping is a core function. One portable catalog declares 35 logical
identities and 43 diagram-type IDs. Core binds detector order/effects, editor semantics and actual
callbacks through the closed diagram-type enum; model availability never becomes parser evidence.
The shared Flowchart payload remains available when Agentflow or Swimlane selects it, while each
output owner retains its own handler gates.

ASCII production depends directly on model; core is a development-only parser provider for existing
source-to-terminal tests. Local capability rows use model availability plus local handlers. The
source facade intersects those observations with actual core parsers, preserving host declarations
under feature union. Terminal primary/fallback accounting and the original source fixtures remain
unchanged. Pure generic-type and shape rules move with the text/model owner instead of being
copied into ASCII.

Shared markup contains the existing HTML5 sanitizer/sink, sanitizer-owned generated DOMPurify
defaults, URL normalization, structural CSS predicates and Khroma operations. Core and Render use
this single implementation. Core's source comment cleanup remains source-owned. Existing generator
and generated-snapshot paths point at the new defaults owner; new package/legal/profile registration
is prepared through existing owners. Final legal/report and artifact verification remains M6/M8 work.

Validation on Windows with serial two-job Cargo builds:

- Model all features: 121 tests passed (`9454c730-1766-434e-b5be-e40e996cbf3d`); an additional isolated
  Agentflow/shared-Flowchart enum case passed (`a5f0ffac-d5cd-40dc-b810-4be919aee863`).
- Markup all features: 55 tests passed (`c4d4a5c9-c946-4870-aa32-bf8b1bf0b1b8`).
- Core plus model, all families, compatibility JSON, editor facts and terminal text: 2336 tests
  passed (`b363d6bf-ec13-4a27-b013-d60003309196`).
- ASCII all families: all 1397 tests passed (`d0681be7-466b-458f-b82b-4a64673021b2`). These counts
  overlap and are not summed.
- Production core/model/markup/ASCII all-feature Clippy passed with `-D warnings` after the
  shape-vocabulary relocation and focused wrapper adaptations. Facade `all-diagrams,svg,ascii`
  with defaults disabled compiled successfully. Analysis integration remains its separate M4 slice.
- ASCII all-family, Agentflow-only, Swimlane-only and C4-only production checks passed; empty model checked.
  The all-family normal/build Cargo tree contains no core, Mermaid grammar, YAML/JSON5, HTML5,
  URL or CSS admission package. This is actual graph evidence; independent consumed-output/size
  receipts and the strict feature matrix remain open.

The six full-output declarations are captured without compatibility serialization. Parsed APIs
consume them explicitly; ordinary canonical-model projection defaults to declared Model output.
Render's layout envelope now borrows the same context for projection of its prepared family data.
The focused graphical producer/projector test passed (`76a20590-bd34-49d0-80f0-e7cc070be384`):
one parameterized case covers empty and header-only construction for all six retired wrappers,
then compares the actual prepared layout envelope with the paired parsed projection. This run
compiled 111 test binaries but executed only that case; 5112 unrelated tests were skipped. The
all-family core presentation recipe without `compat-json` passed 16 focused output-data cases
(`cb304415-19b3-4eee-92f1-591a1e90586b`), including Gantt and Block/Kanban/Treemap declarations.
Two test-only Architecture/GitGraph helpers now use presentation gates/controlled construction so
this producer recipe does not accidentally depend on compatibility serialization. Existing unused
code warnings in that narrow test recipe remain distinct from production Clippy. The four legal
projection unit tests and eight fixed-consumer staging/receipt tests passed. Full SVG DOM, final
consumer sizes/cold compilation and parser-free graphical/export graphs are not claimed by this
extraction.


## Independent M3 model-to-ASCII result

The committed `03c6df3c86edc91248a1617343def817cf2c0323` source and existing fixed consumer were
compared with the original M0 source/fixture using `boundary-model-ascii --expect-receipt`.
Both runtime inputs produce identical Flowchart/Class/State primary output, complete width-12
fallback, and consumed report metadata. Raw and stripped outputs agree. The target, toolchain,
release profile and consumed operation are unchanged.

| Observation | M0 base | M3 checkpoint |
| --- | ---: | ---: |
| External normal/build package versions, including proc macros | 84 | 28 |
| Raw release executable bytes | 3875840 | 2370560 |
| Stripped executable bytes | 3875840 | 2370560 |

The complete normal/build tree contains no core, Mermaid grammar, YAML/JSON5, markup package,
HTML5 recovery, URL normalization or CSS admission. Required character-reference decoding and
Unicode rules remain present. No external package version was added. The fixed executable is
1505280 bytes smaller (38.84%); this does not imply faster execution or cold compilation.

Evidence is retained at ignored `target/bench/experiments/model-source-boundaries/`:
`m3-boundary-model-ascii/receipt.json`, `m3-ascii-consumer-comparison.json` and the count
reconciliation record above. The experiment registration indexes this checkpoint. This is the
independent model-to-ASCII lane only. Source-to-ASCII, graphical/export, analysis/fixes, controlled
cold compilation and final product/feature/legal/package gates remain part of the active goal.


## M4 requested-fix checkpoint

Analysis now separates compiled repair capability from replacement generation for a diagnostic
projection. `merman-analysis/fixes` is default-off. `FixGeneration::Omit` is the default policy;
`Generate` is available only with that feature. Both diagnostic policy and analysis options expose
an explicit `with_fix_generation` choice. Diagnostic/fix DTO and option wire schemas are unchanged.

Canonical diagnostic candidates retain bounded immutable repair evidence instead of eager fix
vectors. Rule selection, suppression and severity projection precede replacement generation.
Shared YAML migration edits are materialized once within each requesting projection. Markdown
projections map the complete action transaction back to the same original document. Omit/Generate
reuse source allocations, parser facts, environment identity, repair evidence and fence identities;
no reparse or mutable canonical-fix cache is added. Retained evidence accounting uses the shared
iterative JSON allocation-weight helper in production. Captured repair config is bounded authored
data; source-site configuration is released after capture.

The semantic all-family diagnostic recipe passed 287 tests
(`7a124ee2-1377-4c16-a583-9d90bd5cf6bc`), and the fixes-capable recipe passed 322 tests
(`48dcbe6a-d059-42ca-a541-9c3a031f25d2`). Counts overlap. Both production-library recipes passed
Clippy with `-D warnings`. Their actual normal/build Cargo trees exclude `serde-saphyr` without
fixes and include the existing writer when fixes is selected. The fixed independent fixes consumer
now explicitly selects the feature and Generate, retaining its original inputs and applied-edit
contract; original M0 source/fixture remains pinned separately.

The compatibility-JSON analysis recipe waits for the active immutable-presentation integration.
Host capability and product recipes, LSP diagnostic/code-action selection, complete independent
receipts and final retention/installed-product gates remain required before closing M4.


## Independent source and analysis checkpoint observations

The remaining committed M3 source lane and both M4 analysis lanes passed `--expect-receipt`
against their original M0 fixtures. Each consumed both original runtime inputs with exact output
and report equality; raw and stripped outputs agree. Counts include proc-macro package rows.

| Fixed consumer | Source / fixture commit | External versions, base → candidate | Stripped bytes, base → candidate |
| --- | --- | ---: | ---: |
| Source-to-ASCII | `03c6df3c8` / same | 84 → 84 | 3790336 → 3822592 |
| Analysis diagnostics | `6cdecd98b` / same | 87 → 82 | 2151936 → 2119680 |
| Analysis fixes | `6cdecd98b` / same | 87 → 87 | 2159104 → 2188800 |

The source workflow still compiles its parser and is 32256 bytes larger (0.85%); no source-workflow
size reduction is claimed. Diagnostic-only analysis excludes `serde-saphyr` and its four exclusive
transitive packages (`autocfg`, `base64`, `nohash-hasher`, `num-traits`). The fixes consumer explicitly
selects the writer and Generate and retains the full original-document edit transaction, including
CRLF and emoji coordinates. Its artifact is 29696 bytes larger (1.38%); the diagnostic artifact is
32256 bytes smaller (1.50%). No external package version was added to any of these matched lanes.

Evidence is under ignored `target/bench/experiments/model-source-boundaries/`:
`m3-boundary-source-ascii/receipt.json`, `m3-source-ascii-consumer-comparison.json`,
`m4-boundary-analysis-diagnostics/receipt.json`, `m4-analysis-diagnostics-comparison.json`,
`m4-boundary-analysis-fixes/receipt.json` and `m4-analysis-fixes-comparison.json`. The registration
indexes these actual results. There is still no controlled cold-compilation or runtime-speed result.
These fixed observations do not close graphical/export, full host/product integration, retirement,
SVG DOM, strict feature, package/legal or final M8 requirements.


## M5 shared presentation construction and source integration

The model package now owns immutable `PresentationConfig`, `PresentationMetadata`, the full typed
`RenderSemanticModel`, one `PresentationContext` enum, and checked `PresentationDiagram` input.
Ordinary authored configuration owns its declared inputs without inferring source-derived theme
residuals. Explicit frozen declarations validate their recipe, ownership, family paint, model,
context, and catalog relationships. Neither constructor authenticates a source producer.

Core retains `SourcePresentationConfig`, `SourcePresentationMetadata`, mutation journals,
detector replay, theme evaluation, initialization caches, source sanitation/remapping, and its
privately constructed `ParsedDiagramRender`. The latter now contains the checked shared value;
output consumers borrow it through `presentation()`, or consume it through `into_presentation()`.
Finalization remains controlled and fallible after sanitation and source-coordinate remapping.
Validation failures do not re-enter the suppressed family-parse-error path.

Construction-local validation policy is independent of existing output model admission. Source
default/config freezing uses an unbounded construction policy with cooperative cancellation;
the request still admits the same authored model complexity at the existing operation boundary.
This preserves small valid model budgets without charging all pinned configuration defaults as
authored model data. Independent data producers can explicitly bound their construction work.

All family compatibility projectors now have a model-owned implementation and a local
`CompatProjectionError`. Source wrappers adapt that error to the existing parse diagnostic.
State retains managed iterative document expansion per occurrence. Treemap retains its 8 MiB
complete-output preflight before the combined source path's editor-fact handoff. Architecture
retains authored root-layout presence separately from resolved configuration, including null.
Mindmap freezes an explicit runtime-derived diagram identity independently of `compat-json`.
Its public tree projector rejects cycles and repeated incoming edges by internal node identity;
repeated authored names and labels remain valid. It no longer substitutes null for an incomplete
child projection. This uses the pinned Mindmap DB's single-parent construction rule.

The source library passed 2101 tests (`1fd4ca19-86c6-4c31-9fe8-6e705ca8416f`). The combined
core/model recipe then passed all 2387 tests across 24 binaries, including integration contracts
(`df7b9899-4391-4b95-acd5-db9ad3f50cb9`). After the bounded Mindmap tree check and two added
regressions, all model-library and source-Mindmap cases passed: 229 tests, 2033 unrelated cases
skipped (`ea071ddc-3fac-44fc-a81b-eb26d9dd65e9`). Core and model production libraries passed
all-feature Clippy with `-D warnings`. These counts overlap.

Render/Export consumer migration, the independent graphical/source-output-facts fixtures,
remaining host/product compilation, public forwarding retirement, and final M8 gates remain
active. No parser-free graphical closure, final size, runtime speed, or cold-compilation result is
claimed by this source-integration checkpoint.


## M5/M6 production consumer and catalog checkpoint

Render accepts shared `PresentationDiagram` values and has no production core dependency. Export
uses shared execution/resource contracts while retaining its actual Render SVG-seal and font
interfaces. SVG navigation serialization moved to Render; shared HTML/URL safety remains in
markup. The pure theme catalog, pinned baseline projection, bounded color-count coercion, and
ECMAScript whitespace predicate have one model owner. Source theme execution and initialization
remain core-owned. Render exposes a shared compatibility recipe; the facade installs it through
the public source-owned theme plan, retaining a bounded one-plan initialization cache.

The isolated `renderer-data` consumer successfully generated real SVG from caller-constructed
Flowchart data without core. The normal all-family Render/library check, all-feature Export
library check, all-feature facade library check, and combined all-feature bindings-core, LSP, and
CLI check passed on Windows. Render's all-feature production library passed strict Clippy.
Partial optional-feature recipes retain their existing unused-code warnings; a successful
all-feature Clippy run does not assert that every singleton recipe is warning-free.

The actual existing closure verifier passed every representative-target artifact profile,
including runtime and declared normal/build exclusions. Model/ASCII exclude source ingestion and
shared HTML5/URL admission; Render/Export exclude core and the Mermaid grammar/YAML/JSON5 stack.
Diagnostic-only recipes exclude `serde-saphyr` and the fixes feature. These are Cargo graph checks
for the descriptor's target triples, not executions of Linux, Apple, Android, or WASM products.
Catalog verification admitted all 46 exact profiles. Capability-surface and moved Mermaid-baseline
projections passed their owners. Seven WASM input/provenance tests also passed.

The fixes capability keeps all previous bit identities and appends bit 21. Exactly the plan's 20
full CLI/LSP/editor/native SDK recipes select it. Diagnostic publication omits materialization;
code actions use the same retained generation for a requested Generate projection. Narrow host
declarations still govern both cached and override bindings paths. Product runtime and installed
consumer tests remain required in addition to these compile and static-review observations.

The new packages appear in the derived coupled publish graph before their dependents. Generated
Rust license reports (13), third-party source notices, all 446 release legal projections, and legal
contents of all 28 governed Cargo packages passed the current owner checks. No publication ran.
Final package/fresh-consumer admission, complete graphical tests/parity, strict feature recipes,
Linux/browser owners, and final independent measurements remain active.

After public forwarding retirement and the shared theme catalog/numeric helper extraction, the
full core/model all-feature recipe passed 2399 tests across 24 binaries
(`0913affd-0cd1-48ea-a44f-f46eac441862`). The complete Render all-feature library recipe passed
3303 tests with one existing skipped case (`12babe04-86a4-419c-8205-bbfd4d32e84b`). The first
Render run exposed fixtures that had relied on unowned `from_value` configuration. Those local
fixtures now declare checked frozen defaults explicitly; ordinary authored input still owns its
supplied leaves. The tests' typed paint/typography assertions remain unchanged.

Review also corrected two migration-local behavior changes before this passing Render run.
Flowchart's standalone formula measurement retains its strict default locally instead of applying
it to Class/Usecase. Agentflow's Flowchart routing projection preserves declared ownership without
creating authored assignments; deep values use iterative cloning. Regressions cover frozen facts
and a depth-2048 value on a 128 KiB stack. The independent raw Mindmap projector also retains
managed JSON ownership on its missing-identity error path; its small-stack regression passed.

The combined all-feature analysis, editor-core, LSP and bindings-core library run passed all
937 tests (`bc478328-ed98-4746-bdff-e25160cebd55`). This exercises the requested-fix projection,
generation reuse, host capability and product library contracts together; it does not replace
the diagnostics-only recipe, protocol/server smoke, installed consumer or final feature matrix.

The full workspace `cargo check --locked --workspace --all-features --all-targets --keep-going`
completed successfully on Windows after migrating test, example, bench, Rustdoc, Web and
Tree-sitter callers. The private theme-acceptance harness requires its separate cfg runner and
is not exercised by that ordinary workspace check. The UniFFI measurement test callback now
handles the already-declared `HtmlLineHeight` operation; this changes no transport enum or version.
Fifty unused Render integration-test imports were removed rather than restoring old aliases.

Current performance probe source fingerprints were refreshed after the import, manifest and
lockfile changes. All 240 Python performance contract tests passed. This refresh preserves lane
recipes, metric thresholds, candidate-admission settings and expected output identities; it is not
a new performance measurement or a claim that an existing memory threshold has been requalified.

## M8 environment preparation

The task-owned Linux environment is an ext4 directory under
`/home/frankorz/merman-model-boundaries-20261030` in Ubuntu 26.04. It reuses the installed Linux
Rust 1.95.0 toolchain without changing the user's default. Task-local nextest 0.9.148, Node
24.21.0/npm 12.2.1, esbuild 0.28.1 and Chrome Headless Shell 156.0.8078.47 passed version checks.
The offline Cargo home contains the lockfile's 568 registry archives and 533 sparse-index records;
archive contents/checksums were checked during staging. Browser libraries and fontconfig are
private extracts of 23 Ubuntu packages verified against signed repository indexes. There was no
system package installation, network reconfiguration or Docker startup.

Setup receipts and environment declarations are retained in ignored
`target/m8-linux-setup-20261030/` and the corresponding Linux scratch root. Environment preparation
alone is not Linux filesystem, process-containment, browser or product execution evidence. Those
owners must run against the frozen candidate source before their results are admitted below.

## M8 implementation checkpoint and first executions

The integrated implementation is committed locally as
`d97d521bbee8e539b55d723d2246c76520c06053`. A verified Git bundle staged that exact commit and
its complete reachable history in the task's ext4 source directory. The selected Mermaid
reference retains its authentic Git identity. All 568 registry archives were rechecked against
the candidate lock; its SHA-256 is
`c19255d0822a0373fe1ccf76cb25c8e476c6ff5b483864c2c52482c94f576e68`.

The complete all-feature facade, CLI, ASCII and Export nextest run passed all 2628 cases, with
14 existing conditional/ignored cases skipped (`aba6883c-cc3b-4a72-b1a1-8200604d6ce1`). Five new
independent-model construction/CSS regressions passed
(`ce60e84e-a0dc-4c62-b2ea-85f3efe662f6`). These cover cheap item/text admission before depth
indexes, cancellation during index construction, canonical CSS validation without explicit
context, shared CSS accounting, and actual SVG preservation of model-owned styles.

On the ext4 candidate, the real-browser parser oracle passed all nine Node tests and the
reference-runtime packaging/path-containment owner passed all eight tests. These ran the selected
Mermaid runtime in the private Chrome Headless Shell, including public parse/config observations
and rejection of external imports, side output, compiler output escapes and a redirected artifact
directory. They do not substitute for the Rust Unix process-tree/timeout and filesystem owners.

The first strict feature run stopped at structural validation: the selector checker mistook
ASCII's matching source-fixture dev dependency for a production selector edge. It now permits
only the corresponding singleton selector on a metadata-declared dev dependency. Production
forwarding remains required; cross-family and `all-diagrams` dev forwarding remain rejected.
All 37 feature-matrix unit tests passed (`63dcfb23-a38f-4f97-aedd-64d94285636d`). The complete
strict matrix is rerunning; no strict-matrix pass is claimed at this checkpoint.

The isolated presentation-only source recipe also exposed a disabled-parser error-classification
bug: without core `compat-json`, known identity was incorrectly treated as a compiled parser.
The missing-render consistency diagnostic now depends on actual canonical parser availability.
Disabled source families retain `UnsupportedDiagram`, lenient Error projection and outer
cancellation. Its dedicated no-family/presentation-only nextest regression passed
(`51bf75bc-28a0-4cc7-a988-b95058d34724`). The Agentflow/editor singleton additionally required the
shared Flowchart shape-span helper's `SourceSpan` import when neither Flowchart nor Swimlane
parsing is selected. The isolated consumer now imports selectors from the defining model owner.

The binding-contract generator refreshed the six Web/Node/Python/Kotlin/Dart capability tables
with the existing declared fixes key. Binding contracts, native ABI, generated LALRPOP parsers,
theme snapshots, Mermaid-reference projections, and the 46 artifact/capability descriptors passed
their verification owners. Web manifest/descriptor tests and real Cargo closures passed 32 cases
after excluding the temporary metadata consumer from selected repository inputs. A regression
checks an in-repository temporary directory and preserves the complete real input list. Full Web
tests require the actual `dist/pkg` build first; an early invocation without those artifacts is
not admitted as product validation.

All 34 workspace packages passed individual `cargo fmt -- --check` runs. The aggregate command
exceeds Windows' command-line limit; individual package invocation is the equivalent format owner.
Whitespace checks passed, and published changelog sections remain identical after line-ending
normalization to the main publication-sync baseline. There has been no push, tag or publication.
