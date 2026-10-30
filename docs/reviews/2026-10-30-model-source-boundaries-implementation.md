# Model/source boundaries implementation record

Status: M0 complete; M1–M3 implemented with integration/retirement work still open; M4–M8 in progress, 2026-10-30. This records implementation of the [approved plan](../plans/2026-10-30-model-and-source-boundaries.md), [family inventory](../plans/2026-10-30-family-data-inventory.md) and [ADR-0099](../adr/0099-model-and-source-boundaries.md). Open plan checkboxes remain requirements, not completed claims.

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
| M1 | In progress | Shared lifecycle/config/runtime/resources/text and the pinned data-only identity catalog moved; source provider bindings remain core-owned. Final consumer retirement and M8 admission remain open. |
| M2 | In progress | All 34 canonical payloads and nine real full-data groups moved. The six redundant wrappers are retired; core/model all-family tests passed 2336 cases. Graphical context/projection admission and old-path retirement remain open. |
| M3 | In progress | ASCII production uses model directly; all 1397 runtime tests passed. Independent output receipt and remaining product/feature gates are open. |
| M4 | In progress | Requested-fix implementation is active; no admission result yet. |
| M5 | In progress | Shared markup implementation extracted and its 55 tests passed; immutable presentation and parser-free Render/Export remain open. |
| M6–M8 | Open | Final product, retirement and acceptance work remains. |

Existing correctness results at the prerequisite retain their original scope in the [dependency review](../research/2026-10-30-dependency-review.md). New consumer baselines and implementation gates will be recorded here as they complete. Do not infer a runtime improvement from the package boundary or rewrite historical measurement hashes.

## M0 consumer baseline

All lanes use Rust 1.95.0 on `x86_64-pc-windows-msvc`, empty dependency defaults and their committed fixed feature recipe. The runner uses release `opt-level=s`, thin LTO, one codegen unit, abort-on-panic, no debug info and no incremental compilation. Builds ran sequentially with one job and the shared development target. External counts are distinct resolved normal/build package versions, excluding path dependencies; they are not linked-library or deployed-service counts. These Windows receipts are distinct from the dependency review's representative Linux-target graph.

| Fixed consumer | External versions | Raw release bytes | Stripped bytes |
| --- | ---: | ---: | ---: |
| `boundary-model` | 74 | 540672 | 540672 |
| `boundary-model-ascii` | 76 | 3875840 | 3875840 |
| `boundary-source-ascii` | 76 | 3790336 | 3790336 |
| `boundary-presentation-svg` | 105 | 7020032 | 7020032 |
| `boundary-analysis-diagnostics` | 79 | 2151936 | 2151936 |
| `boundary-analysis-fixes` | 79 | 2159104 | 2159104 |

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
| External normal/build package versions | 74 | 16 |
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
