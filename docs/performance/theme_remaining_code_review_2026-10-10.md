# Remaining Theme Code Review — October 10, 2026

This review found additional implementation debt after the first cleanup. The previous
107-module inventory accounted for additions; it did not establish that all remaining code
was necessary or that the retirement plan was complete. The recent-cleanup review alone
also did not review the entire PR. The [plan](../plans/2026-10-10-1535-refactor-theme-addition-necessity-plan.md)
is reopened. Findings below are unimplemented; no production file was changed for this review.

## Frozen scope and evidence

Reviewed HEAD: `a987774ff`. Main: `2928265122d5a3d35e239e3ddbce885c7e342c59`.
The main-to-HEAD diff contains **1,670 changed paths, 405,432 added and 40,413 deleted text
lines**. This includes tests, documentation, generated projections and fixtures, not just
runtime code. The narrower cleanup from `8664ff06` contains **138 paths, 2,991 additions and
18,726 deletions**, including its execution report. These frozen counts exclude this review.

Independent reviewers read current implementations and traced constructors, callers, actual
writers and support-matrix constraints. The main reviewer checked the material findings against
the corresponding source. Repository searches establish construction/read/write claims below.
No Cargo, browser, pixel test or benchmark was run in this review. Earlier passing receipts
remain revision-scoped verification; they do not disprove dead code or guarantee minimality.

| Review track | Actual inspection | Remaining limits |
| --- | --- | --- |
| Theme/compiler/core/bindings | Compiler, definition admission, materializer, wire decode, semantic spec, family program/support, Core theme/config/overlay and parse pipeline, binding theme and recipe entrypoints | Not every public builder or generated field was read line by line |
| State/Flowchart/Sequence/GitGraph | Preparation, source precedence, typography/paint receipts, final observers and writer call chains | Every Flowchart shape and every layout backend branch was not exhaustively inspected |
| Architecture/Block/Class/ER | Theme plans, receipt completion/property paths and writer checkpoints | All tests, shape branches and ELK adapter implementation remain outside exhaustive coverage |
| Other diagram families | Gantt, Pie, Kanban, Mindmap, Timeline, Treemap, Venn, ZenUML, Sankey, C4, Journey, Radar, Requirement, Packet, Quadrant, TreeView, Ishikawa, Railroad, EventModeling, Cynefin and Wardley key production paths | Some text/axis paint subfiles and their tests remain uninspected |
| Native/facade/artifact observation | Native filter receipt, font environment, document/evidence/target admission/digests, SVG observations and their consumers | No native conversion or visual experiment performed |
| Tests/tools | Family root/Node geometry/Class relation/Sequence/State/cancellation cases, including full follow-up reads of ranges 3293–5690, 6475–7488 and 8270–11500 (117 test functions and 9 helpers); selected large SVG helpers; acceptance/qualification/Sequence/State/pixel probes; four theme Python tools and snapshot owner | Other large integration suites remain sampled; this is not exhaustive coverage of every test file |
| User interfaces and transports | CLI options/config/prepare/execute; Playground editor/store/share/input/coordinator; Web runtime/catalog/authoring; WASM and Node canonical dispatch; theme increments in FFI/UniFFI/JNI and real Python/Apple/Android/Flutter/Typst consumers | Not every production adapter method, generated ABI/resource mirror or platform test was read line by line; no cross-platform execution performed |

This is a deeper code review with explicit limits, not a line-by-line signoff on 405,432 additions.
Generated/legal data, static scenes and historical documents retain the earlier accounting and
provenance decisions; this review does not newly certify every platform or adjacent release branch.

## Standards

No new hard violation of the documented repository standards was confirmed. Several current
implementations conflict with the intended simplification direction in practice: dead private
states, primitive argument groups and repeated immutable checks. These are concrete maintenance
findings rather than violations inferred from file length. Their removal must retain the current
source, writer, resource and error contracts.

## Implementation findings

### F1 — Retired catalog success state remains executable scaffolding

`environment.rs:2017–2018` initializes both prepared layout and preparation error to `None`.
There is no `PreparedTextLayout` or candidate construction anywhere in the workspace: only
declarations and implementations remain in `text/prepared.rs:3371–4204`. `prepared` is a private
module; writing `pub` on its backend/request/response types does not make them public interfaces.
Only selected report/failure and workspace evidence types are re-exported through `text.rs`.

Consequently, the following current success branches have no production input:

- State `label_artifact.rs:323–402` and the prepared typography branches in `style_plan.rs`;
- Flowchart `svg_label_artifact.rs:1437/1714/1984` catalog-backed label preparation;
- Sequence `text_artifact.rs:45`, which always returns before its XML scan, ID edits and ledger;
- exporter `font_environment.rs:410–556` exact asset-face matching: observed faces carry
  `key=None/System`, whereas expected retained faces carry `key=Some(asset identity)`.

`FontCatalogInner` also has one successful constructor, `system_fonts()`, with permanently
empty asset/face/alias/family maps. `compile_font_catalog` always rejects custom assets;
`requires_prepared_text_layout()` is therefore always false. Its private lookup, retained-resource
and capability-inference success branches are further retirement candidates.

**Action: reopen U4 and retire the complete private construction/consumer chain.** Keep readable
exchange schema and any genuinely callable public report contract, unsupported-font error order,
normal host measurement, ordinary prepared labels/source projection, math and artifact binding.
Reject forged prepared tokens or unexpected receipts explicitly after removing success matching.
This is dead implementation debt, not an observed supported-user regression or measured size gain.

### F2 — Core host overlay exists only for its own tests

`Engine.post_detection_config_overlay` (`core/lib.rs:616`) is initialized to `None`; its only setter
is under `cfg(test)`. Yet `parse_pipeline.rs:242` retains its production application branch and
`config/overlay.rs:117–388` retains the contribution executor. Provenance readers are test-only;
production still finalizes and stores empty provenance in every `MermaidConfig`.

**Action: reopen U2 and remove the obsolete host/fallback contribution and provenance model.**
`overlay.rs:426–481` is different: GitGraph/ER paint-input freezing has live production consumers.
Preserve detector changes, explicit source/site ownership and ELK keyword selection under a small
current owner. Renaming/removing the entire file without this separation would break semantics.

### F3 — An exact-writer proof misses duplicate properties within one CSS rule

`c6_sequence_proof.rs:433–455` counts at most one value per matching rule by calling
`SvgStyleRuleObservation::declaration`, which returns the last matching declaration. A rule
containing `stroke:#000!important;stroke:#2c2416;` is therefore counted as one expected declaration.
The existing duplicate test at line 699 only appends a second rule.

**Action: fix U7/U9 by counting every matching declaration and adding the same-rule counterexample.**
Do not implement browser cascade. This gap affects the exact SVG writer-declaration proof;
independent native PNG color regions can still reject a visible color mutation. No complete native
qualification bypass was established.

### F4 — Ishikawa resolves the same static fill once per label

`ishikawa/theme.rs:105–128` resolves one Text/default style, then calls `observe_text_style` for
each expectation. That helper repeats winner/capability collection and linearly finds the same
rule in `typed_fill_expectation` (`458–493`). With N visible labels and R family rules, a winning
rule late in the rule list makes this preparation step perform up to O(NR) rule inspections.
Both inputs can vary within their admitted limits; only the one final static fill is consumed.

**Action: U5 resolves winner/capability/fill once outside the loop.** The existing direct-static
paint owner accepts the same unqualified Text selector and transparent/solid representation.
Terminal-specific expectations and Head versus inherited-size roles remain distinct. The proposed
preparation bound is O(N + R); it is not a claim about total render complexity or measured latency.
Use the existing structural-evidence policy with an exact work counter or scale experiment before
admitting a performance claim.

### F5 — Validated immutable receipts replay full completion scans

Gantt (`theme.rs:728/1560`), Journey (`523/784`), Requirement (`393/832`), ER
(`theme.rs:1027/1087`, `terminal.rs:644/669`) and Block (`theme.rs:1000/1022/1414`) check complete
writer evidence before storing a receipt, then scan all checkpoints again for each rule/property.
Block also queries the same properties in both filtering and `all` passes.

**Action: U7 checks completeness once at immutable storage and queries admitted local facts later.**
Current supported selectors bound the number of pending winners in these families. This is
repeated O(T) work with a bounded multiplier, not a demonstrated unbounded quadratic defect.
Do not merely remove defensive checks from a still-mutable receipt. Preserve missing/duplicate/
mutated writer rejection and independent typography facts; measure any latency claim separately.

### F6 — Block observes shell paint when no paint proof is requested

`block/theme.rs:994` creates a receipt whenever any structured theme exists. Thus a canvas-only,
typography-only or fully source-owned node theme can still parse every emitted node shell through
`roxmltree` at line 1283 while `pending` is empty. `finish_evidence` has no node/cluster rule to
certify in that case. This is a confirmed execution path; the proposed demand gate still needs
output/evidence equivalence tests.

**Action: U7 gates the paint observer on actual proof demand.** Keep expectations needed by
`terminal_binding`, and keep the writer's separate canonical-shell/count checks at
`svg/parity/block/render.rs:1022/1344`, typography and label receipts. Adjust `record_terminal(None)`
to the new demand condition rather than deleting its integrity check.

### F7 — Raw complete-spec admission discards a fully decoded graph

Bindings `theme.rs:139–141` fully deserializes `DiagramThemeSpecWireV1` and discards it. The normal
typed decode at line 28 then constructs that graph again from Value. Raw admission is necessary
because Value normalization otherwise loses duplicate fields; graph reconstruction is not itself
a separate required invariant.

**Action: reopen U6 and carry the admitted typed payload to its ordinary decoding position.**
Preserve encoded/depth/duplicate/schema checks and version/union/layout/CSS/payload error order.
Do not replace Serde with another hand-written partial schema checker. Allocation benefit is
unmeasured and this boundary refactor needs error-order tests.

### F8 — Local dead or redundant state and mechanics

These are smaller U2/U5/U7/U9 candidates, not grounds for a new generic framework:

| Location at reviewed HEAD | Evidence and proposed action |
| --- | --- |
| `er/theme.rs:843/1378` | `ErRuleObservation.incomplete` is only default-initialized and read; no write exists. Delete the field and unreachable branch |
| `diagram_theme/materializer.rs:574–585` | Series empty/limit checks repeat `validate_definition_shape` called immediately before token resolution; default series is a fixed valid constant. Keep one owner |
| `svg_artifact_receipts.rs:154–157` | Private immutable fields, validating constructors and no mutator make the canonical rehash redundant. Keep constructor hash and target artifact identity comparison; only internal acceptance cost is affected |
| `kanban/theme.rs:200/1216` and writer `parity/kanban/render.rs:318` | Writer emission, receipts and accumulator repeatedly split the same fact group into six primitives. Introduce a small run-summary value, preserving independent actual/expected observations |
| GitGraph `typography.rs:487`, `text_paint.rs:476`, `node_paint.rs:390/441` | Repeated visible commit-label/tag ordering. Consider one family-local event iterator; keep each receipt's independent cursor and property/source checks |
| `venn/theme.rs:256/261/292–297/378–383` | Single stylesheet writer sets an expected-string clone and a mask only ever 0 or 15. A recorded flag can express the same state while preserving duplicate-call invalidation and actual text counts |
| `tree_view/theme.rs:744/779/831` | `fixed_icons_seen` has no reader. Delete it; total/paintable/unverified icon facts remain |
| `timeline/theme.rs:1160/1177` | A count diverging from `next_palette_index` already permanently invalidates `palette_values_match`; consolidate the equivalent counter after negative-case verification |
| `family/tests/mod.rs:1931/2076/2255` | Same two unsupported source surfaces and Strict/BestEffort assertions for three geometry patches. A local case table preserves all executions without three repeated setup bodies |
| `family/tests/mod.rs:4961` | The radius-8/choice-node case repeats the same patch, source and BestEffort/Strict outcomes already checked at line 2255. Remove that case and its two redundant renders; retain the mixed gradient/stroke case |
| `family/tests/mod.rs:8117/8676` | The two State gradient tests share the same theme/source and BestEffort render. Move the former's verification/source-residual assertions into the latter, retaining its Strict pre-layout rejection; remove one redundant render |
| `family/tests/mod.rs:5044/5095` and `7064/7132` | Flowchart/Swimlane ordinal-palette cases and Sequence Activation/Note precedence cases repeat local execution mechanics. Share those mechanics only; keep both consumers, family/target identity and every distinct assertion |

No aggregate source-deletion estimate is given: candidate windows overlap, replacements have a
cost, and preserved test bodies/public types cannot be counted as deleted production code.

### F9 — Math tests do not establish local geometry or preparation reuse

`family/tests/mod.rs:10553/10739/10869` checks for a prepared-native math group and then uses
whole-SVG `contains("<path")` to claim nonempty math geometry. Node shells, message arrows or
actor glyphs can supply that path even if the math group is empty. Group count and semantic
occurrence identity assertions prove other facts; they do not make this geometry assertion local.

**Action: U9 checks nonempty geometry inside each `merman-prepared-math-native` subtree.**
Use the existing XML observation tools and add a small negative control that retains unrelated
paths while emptying the math subtree. Keep mirrored terminal versus semantic-occurrence counts
and canonical-output-policy checks. This is a confirmed weak test oracle, not an established
production math failure or reason to retire the live math pipeline.

The Swimlane reuse test at `family/tests/mod.rs:10015` supplies a math backend returning constant
HTML and checks the sidecar/final HTML. Calling that backend again during SVG emission produces
the same output and would pass. Add a backend call counter and compare before/after emission;
keep the existing output checks. This demonstrates a weak reuse oracle, not a confirmed extra
production call or measured regression.

### F10 — Playground repeats the same limitation text in one menu

`playground/src/components/ToolbarArtifactActions.tsx:280–301` emits
`share.externalThemeLimit` twice under the same `hasMermanTheme` condition in the export menu.
Theme users see the same explanation twice. **Action: U5 keeps one explanation for the affected
actions**, retaining the external Mermaid/Markdown limitation itself.

Two user-interface findings require a narrower disposition than automatic deletion:

- `playground/src/runtime/merman-browser.ts:74–78` validates arbitrary recipe input by planning a
  fixed Flowchart. The real Rust admission is shared, but the adapter depends on Flowchart/SVG
  capability and the plan route. Evaluate an existing pure admission/materialization owner before
  proposing a new public operation; no current supported-user failure was established.
- Flutter `merman_ffi.dart:2386–2478` projects constructor `runtime_policy` out of request JSON
  with a bounded-depth raw member scanner. This is adapter protocol debt, but preserving duplicate
  fields and the original byte charge is a real requirement. Retain it until an equivalent
  transport boundary is demonstrated; replacing it with decode/re-encode loses those facts.

The inspected CLI, Web, WASM, Node and native theme dispatch paths share canonical binding
operations rather than implementing another theme compiler. Published s2/current s3 share support
and store selector exclusivity have real consumers. Generated API projections are not newly
certified line by line by this review.

## Spec and completion assessment

The existing plan's R6 already requires removing demonstrated unreachable private success state;
F1 proves this is incomplete. R9 requires explicit remaining uncertainty; accounting every module
does not satisfy it by itself. R7 requires truthful actual-output evidence, which F3 needs to fix.
The first cleanup's passing tests and artifact receipts remain useful but do not close these gaps.
Affected remote owner jobs also have no current-source receipt in the local execution report.

No reason was found to delete all repeated-looking observers. State's final writer-range observer
handles special/composite states and HTML/source ownership. Native filter XML and usvg checks
prove different representations. Sequence role typography, Flowchart source masking, GitGraph's
real BestEffort fill, math projection and facade versus native target admission have live consumers
and distinct failure modes. They remain until an equivalent smaller owner is demonstrated.

The next implementation order is: F3/F9 proof and test correction; F1/F2 complete dead-chain retirement;
F4/F6 scoped work reduction; then F5/F7 and the local F8/F10 simplifications. Each batch needs its
own targeted locked nextest, output/error/boundary checks and exact staging. Broader delivery and
artifact gates follow material feature/dependency changes. Structural/timing evidence uses the
existing [performance policy](BENCHMARKING.md#structural-complexity-and-resource-amplification).
No fresh speed, memory or binary-size improvement is claimed by this static review.
