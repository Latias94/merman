---
type: "Work Progress"
title: "Mermaid 12 integration closeout and remaining source convergence"
description: "Verified integration surfaces, source-backed route repairs, and open size/parity gates."
timestamp: 2026-09-24T03:52:00Z
record_id: "mermaid12-integration-closeout-20260924"
producer_id: "codex-mermaid12-recovery"
run_id: "session-01a0c895-recovery"
---

# Current scope

Continue the Mermaid 12.0.0 refactor on `refactor/mermaid-12-alignment`. The genuine
12.0.0 reference corpus has been admitted in `9e74c56a7` (37 families, 3,716 SVGs).
This is corpus admission, not a claim that every local renderer passes parity. No push,
PR, publication, or release was performed.

# Verified consumer surfaces

- Five Web WASM packages built successfully with their declared capability recipes.
  The measured source predates the final classic-circle padding correction. Input identities:
  full `279ca08fe5f8`, analysis `5279c6978fb5`, render `76dcb039a5af`,
  editor `7740b674c3c5`, ASCII `03e7c91e8cb4`.
- Web contracts verified 35 WASM exports, 45 runtime bindings, and five package entries.
  Node scripts passed 126 tests, with two skips.
- Playground production build and eight desktop browser smoke cases passed using the
  above full WASM. Prepared tests and ESLint passed. The capability test now covers all
  33 diagram types and explicitly checks Agentflow/Usecase as unsupported by ASCII
  (`a69c1e7ca`). Fresh renderer changes still require a new full WASM build.
- VS Code extension build and 255 tests passed.

# Rust integration status

The preceding serial workspace nextest run executed 8,607 tests: 8,605 passed,
two failed, and seven were skipped. Both failures were diagnosed:

1. The nested Flowchart ELK layout golden retained `subGraph1` where the current parser
   names the node/cluster `main`; only those two IDs were updated.
2. An xtask operation-evidence test selected seven fixtures through the ambiguous `basic`
   substring. It now selects the exact basic-flowchart example. Full-corpus parity still
   checks the excluded-from-this-test browser-sensitive fixture.

A subsequent targeted layout snapshot run without `layout-elk` was a vacuous pass:
the main snapshot test returns early without that feature. Do not use it as golden
verification. The ongoing strict workspace run includes ELK through feature unification.

The September 24 `cargo run -p xtask -- verify --strict` run is recorded in
`target/mermaid12-verify-strict-final.log`. Before nextest it passed fmt, workspace
check/Clippy, 117 feature combinations, artifact recipes, materialized reference,
generated contracts, alignment, RustSec exceptions, dependency closures, 13 Rust
license reports, 382 legal projections, legal contents of 23 Cargo packages, and
Playground/VS Code npm licenses. Nextest stopped after 7,516 of 8,612 tests:
7,515 passed, one failed, seven skipped, and 1,096 were not run. The failure is the
classic-circle assertion in `flowchart_node_shape_dimensions_follow_mermaid_rules`,
which still expected label width plus padding rather than the source label diagonal.
That assertion is corrected in `8931f9988`. New small-node alignment unit tests
passed in this run. Some renderer edits occurred after initial library compilation;
current-source affected checks remain necessary. Doctests and downstream SVG/package
stages were not reached.

# Source-backed geometry findings

- Circle dimensions follow Mermaid 12 `circle.ts`: label diagonal plus 64 for Neo,
  label diagonal plus the complete configured padding for classic. The initial
  classic implementation doubled that padding; the working correction also covers
  an empty label with custom padding. Commit `a9f3cd939` contains the preceding
  circle repair, evidence-test selection, and nested-group IDs.
- State parallel-edge label receipts were reviewed against the same input/upstream
  hashes. The renderer's source-backed Neo barb offset changes local terminal y from
  204 to 198.5; only the three exact local route/mask signatures are updated.
  Label widths, anchors, provider routes, and upstream signatures remain unchanged.
  The catalog is embedded with `include_str!`, so an old xtask executable cannot
  validate the edited file.
- State `stress_state_quoted_multiline_names_015` has a reproduced browser floating
  boundary residual. Upstream x delta is `-2.842170943040401e-14` relative to the half
  width; local delta is positive. `outsideNode` therefore selects five versus four
  points. The upstream extra y is `183.38425601494978`. The replay extracts the actual
  pinned release functions; no renderer epsilon or comparator relaxation was added.
  Reproduction: `node target/mermaid12-state-015-replay.mjs`; output is the adjacent
  JSON. Source: ELK `geometry.ts` lines 98/224/276 and `render.ts` lines 2452/2475/2487.
- Flowchart small-node anchor alignment and shared endpoint-cutter migration are
  implemented in `8931f9988`, including the classic circle padding correction.
  Shape-specific intersections remain an adapter to the shared cutter. Existing
  cancellation and work-budget accounting cover the new alignment pass.
- Class now preserves authored internal member whitespace (`496aa53a6`). Sequence
  final drawn text inherits the theme family when the inline family is absent or
  rejected (`203c757c5`); body-level measurement retains its original context.
- `fb3d5707c` records the three exact State Neo marker receipts and the self-loop
  golden omitted by the earlier NORTH-port fix. No broad normalization was added.

# Current-source verification at 04:36 UTC

The affected renderer nextest run executed 1,654 tests: 1,650 passed and four failed.
Two failures were test-only CSS whitespace expectations, one assumed a symmetric
compact cutter that the upstream algorithm does not provide, and one was the golden
aggregate. The corrected contracts pass all ten focused unit tests. Exactly 129
layout goldens were regenerated after cause review: 23 Class whitespace, 105
Flowchart circle/small-node/previously fixed parser identity, and one prior State
self-loop repair. All three layout snapshot tests now pass with `layout-elk` enabled
and zero skips. Formatting and render/core all-target/all-feature Clippy pass.

Fresh three-mode SVG comparisons use the rebuilt xtask executable:

| Family | Rendered | Comparisons | Current result |
| --- | ---: | ---: | --- |
| Class | 250 | 750 | pass, including existing exact accepted receipts |
| Flowchart | 1,153 | 3,459 | 68 distinct DOM blockers; 34 stale local receipt fixtures |
| State | 285 | 855 | 18 distinct DOM blockers; three label receipts accepted |
| Sequence | 321 | 963 | 25 distinct DOM blockers, plus stale receipts |
| Timeline | 93 | 279 | one new browser wrapping residual; 26 existing receipts accepted |

These counts distinguish the report's Mismatches section from accepted Notes. Modes
and stale receipts may overlap; do not sum them as distinct fixtures. Class's existing
026 receipt matches again after authored whitespace is preserved.

Two genuine remaining gaps are being implemented: shared line hops need the pinned
Mermaid 12 rounded-corner exclusion, 2px clearance, and 60% useful-radius filtering;
Sequence drawing must accumulate each actual text line's height and preserve signed
margin/dy/JavaScript rounding behavior. State style-spec HTML height is under audit.

# Timeline exact residual admission

The new Timeline 012 receipt binds input SHA-256
`913f05bbfa2bfba5177aeb5b34b64e40720bcd3e03617d0fd9d19aa9f8ed6bb9`, upstream SVG
`75d2c562ccc20d50e430afb785f0854383b821bfd36cf8b33c0ca433ac842d1a`, and canonical
local signature `eb0aeb1b8bf6885d50ab26aee280864b3e7cd4df4acd91653b783727fec598c1`.
Both SVGs preserve all 34 text elements and word sequences. Seven labels wrap at
different words with the same Fira Sans 17px style and 150px width. Pinned Timeline
`svgDraw.js:456` uses `getComputedTextLength`; local `timeline.rs:145` uses its
headless measurement counterpart. Both use first-row 1em and subsequent-row 1.1em.
The resulting row heights propagate into the viewport. Outside text subtrees,
224 elements retain order, IDs, classes, and path command sequences. Additional
upstream Neo CSS is inactive because this fixture has no `data-look` attributes.
This receipt does not admit missing text, topology, or active-style differences.

The existing comparator captured the canonical signature, then a fresh full
Timeline comparison passed all 93 fixtures / 279 mode comparisons, including 81
exact residual comparisons. Only this one entry was added; the 26 historical
Timeline entries were unchanged. Detailed local evidence is in
`target/mermaid12-timeline-012-residual-audit.md` and the verification log is
`target/mermaid12-timeline-receipt-verified.log`.

# Artifact-size gate

All values below are bytes measured by the checked-in `wasm-size-matrix` command
against stripped production artifacts. Budgets remain unchanged.

| Profile | Raw | Stripped | gzip | Brotli | Result |
| --- | ---: | ---: | ---: | ---: | --- |
| analysis | 3,552,714 | 3,552,491 | 1,415,366 | 1,089,915 | gzip/Brotli exceed 1,375,000/1,050,000 |
| ASCII | 5,114,034 | 5,113,811 | 1,930,013 | 1,470,327 | pass |
| editor | 3,666,234 | 3,666,011 | 1,460,379 | 1,119,715 | gzip/Brotli exceed 1,450,000/1,100,000 |
| full | 12,951,294 | 12,951,071 | 4,861,117 | 3,600,842 | pass |
| render | 10,990,069 | 10,989,846 | 4,189,184 | 3,108,904 | pass |

Typst compilation succeeded but measurement could not find `wasm-opt` on PATH.
Binaryen 131 is available under `target/tools/binaryen-131/binaryen-version_131/bin`;
retry with that directory prepended to the process-local PATH.

Read-only attribution excludes large generated-theme growth: theme JSON standalone
gzip decreased by 9,780 bytes from the 11.17.2 baseline, while defaults and shape
metadata together grew only 208 bytes. Production-only theme provenance snapshots
are a separate redundant-work candidate, not a proven cause of this upgrade's growth.
Its isolated experiment is registered under
`target/bench/experiments/mermaid12-theme-audit-elision/experiment.yaml` and the sibling
`merman-theme-audit` worktree. The candidate was admitted as `6dc4c9f6f` after 41 theme unit tests, 11 public
contract tests, and real analysis/editor WASM smoke checks. Candidate gzip/Brotli
sizes are analysis 1,413,257/1,088,501 and editor 1,458,058/1,119,272. Improvements
are respectively 2,109/1,414 and 2,321/443 bytes, with raw size also decreasing.
This is a bounded intermediate improvement, not artifact-size gate completion. No budget increase or capability removal is authorized.

# Drawing semantics follow-up

A new source-backed batch implements shared rounded-corner crossing exclusion and
hop radius filtering, Sequence actual per-row Direct/Tspan height accumulation,
and Flowchart brace polygon intersections. All 61 targeted tests pass in
`target/mermaid12-drawing-semantics-tests.log`. Independent review closed the
Sequence CSS declaration-injection finding using the existing cssparser tokenizer
and safe token serialization shared by measurement and SVG emission.

Brace review found a new zero-padding intersection regression caused by a 1px
minimum copied from old paint geometry. The exact upstream oracle requires retaining
zero dimensions. Full Neo brace convergence also needs common axis-specific padding
and label placement across layout, SVG, intersections, and bounds. These repairs
are queued while the serial complete-workspace nextest run finishes; this section
is not an admission receipt for that unfinished brace work.

State parallel-channel replay captured the real Mermaid 12 provider graph and
reproduced the signed SVG through elkjs 0.9.3. Replacing every leaf-node and edge-label
size with local values still leaves the upstream channel sides opposite to Rust.
A target-only Rust phase diagnostic confirms the child model-order option is NONE,
so the remaining cause lies beyond simple option inheritance. Keep this blocked
as provider ordering semantics, not a font residual. Flowchart group external-port
10px return segments are separately under source audit.

# Provider causality and isolated follow-up

The State parallel-port root cause is now proven in both the pinned Java
`PortListSorter.java:105-143` and elkjs 0.9.3's published worker. Their high index
stops at len-1 and the high-index loop rereads the low port; ranges of length <=2
are not reversed. SOUTH executes before a fresh WEST search, so mixed groups can
cross and subsequently restore ordering. Replacing only this helper in the captured
JS provider reproduces Rust's wrong channels; restoring it restores the entire
output. The proposed Rust correction preserves that reference behavior and existing
complete port-reference remapping. The 040 State fixture's remaining raw routes are
also reproduced by the original JS provider after substituting local measured sizes.
Composite self-loop parent-port ordering remains independently under investigation.

Flowchart's group-endpoint return segments have a separate proven cause:
`CompoundGraphPreprocessor.java:893-931` preserves an original parent's port properties
for a direct parent connection, while `:951-956` assigns spacing/2 only for a truly
exported boundary port. Native code applied spacing/2 to both. Replays cover spacing
20/40, original offsets +7/-4, and ordinary crossing controls with equal node bounds.

To keep the main checkout's current-source test run immutable, further edits use
`target/worktrees/mermaid12-layout-closeout`, detached at `4c4f6aec1`. The main pending
renderer edits were copied there before editing. No Cargo runs in this worktree.
`target/mermaid12-layout-closeout-baseline.json` binds 22 original source-file hashes;
verify them again before copying any reviewed files back. Three disjoint owners handle
P3 port ordering, compound direct-parent offsets, and complete Neo brace geometry.
The original shared `target` remains the only build location.

At 05:18 UTC the main workspace nextest run has finished compiling 226 binaries
(23m48s) and is executing 8,635 tests with two test threads. Its log is
`target/mermaid12-workspace-nextest-convergence.log`; no completion claim is made here.
The isolated follow-up changes are not part of those compiled binaries and require
scoped checks after transfer. Detailed causal evidence remains in
`target/mermaid12-state-provider-causality.md`,
`target/mermaid12-group-endpoint-evidence/diagnosis.md`, and
`target/mermaid12-brace-isolated-closeout.md`.

# Remaining size attribution boundary

Read-only artifact attribution locates 37 complete legacy-encoding tables in both
actual analysis/editor WASM files (77,549 uncompressed bytes). The sanitizer's
public input is UTF-8, but lol_html 3.0.1 retains runtime encoding operations in its
decoder, token/attribute conversions, and mutation sink. It exposes no static
UTF-8-only backend or feature. Its rewrite_str convenience API still uses the
general backend and would also bypass Merman's bounded streaming sink if substituted
naively. No dependency or sanitizer behavior was changed.

A memory-only zeroing probe shows enough compressed entropy to motivate a separate
research candidate, but produces no valid artifact and is not implementation savings
or gate evidence. A maintained specialization would require an additive dependency
backend or a scoped private variant; a subtractive global Cargo patch could affect
other consumers through feature unification. This is not a safe one-line release
fix. Exact hashes, source seams, and the probe's limits are recorded in
`target/bench/experiments/mermaid12-theme-audit-elision/next-size-attribution.md`.

# Workspace completion and reviewed transfer

The complete workspace run finished: 8,635 executed, 8,634 passed, one failed,
and seven skipped. The only failure was the residual catalog's fixed entry count
of 96 after the reviewed Timeline admission increased it to 97. Commit `6a2035822`
updates that assertion; all three browser-text-layout catalog tests pass in
`target/mermaid12-catalog-tests.log`. No semantic assertion was relaxed. This closes
the sole workspace failure without repeating the entire unchanged test suite.

Sequence source drawing semantics are committed in `d4ea42349`; shared rounded-corner
line-hop filtering is committed in `7a7b24f76`. The 61 focused renderer tests and
workspace renderer tests pass, and cargo fmt passes. The reviewed follow-up batch
was transferred only after all seven destination hashes matched the saved baseline.
Individual and frozen integration reviews report no blocking findings. The exact
transfer is recorded in `target/mermaid12-layout-closeout-copyback.json`; kernel and
renderer checks for those new bytes remain distinct from the completed workspace run.

A further State compound-self-loop defect has a source/intervention/restored replay:
Rust stable sorting invokes the stateful model-order comparator in a different order
from the pinned GWT insertion/merge sort. Reversing only the provider's comparator
invocation reproduces native routing point for point; restoring it restores the full
provider output. Earlier parent-port synchronization attribution is withdrawn.
Evidence: `target/mermaid12-state-selfloop-sort-gap.md`. This requires source-compatible
sorting, not a forced self-loop side or a browser residual receipt.

# Provider and brace closeout checks

Port-range convergence is committed in `cc1d773fd`; direct-parent boundary offsets
in `5c6a02c86`; complete brace sizing, polygon intersections and translated-label
bounds in `9b50b4614`; source-compatible stateful comparison scheduling in `53057af2f`.
The last change passes all 420 kernel unit tests, including existing budget/error
contracts and five new scheduling/compound regressions. Individual source reviews
report no blocking findings. Logs: `target/mermaid12-model-order-kernel-tests.log`
and source evidence `target/mermaid12-model-sort-implementation.md`.

The first transferred kernel batch passed all 415 tests after its high-degree
budget fixture explicitly established crossing-free west-port order. This preserves
the budget test's original premise independently of PortListSorter's corrected
source behavior; all budget and output assertions remain unchanged. Independent
review confirmed this is fixture setup, not displaced production work.

Renderer unit tests passed 1,398/1,399 initially. The new viewport test manually
cached a 100px label while retaining the default 120px minimum; actual layout always
applies that minimum before caching. Explicit `minNodeWidth: 0` makes this controlled
fixture valid. Its rerun passes (`target/mermaid12-brace-viewport-check.log`), without
changing production bounds or tolerances. The three-package all-target/all-feature
Clippy check and workspace formatting check pass before the final sorting change.

The affected integration run passed 229/231 with one skipped. Its failures are the
layout snapshot aggregate and State's existing model-order distinguishing fixture.
These results predate the last stateful sorting correction. The old State fixture
must be compared with the source provider at equal dimensions before changing its
assertions. Golden refreshes must be cause-reviewed against fresh output; the
reported mismatches include expected provider ordering and Neo brace size changes.
Log: `target/mermaid12-layout-closeout-integration.log`. A rebuilt xtask and four-family
three-mode comparison are in progress; no fresh SVG admission is asserted yet.

# Fresh SVG and remaining semantic gaps

The rebuilt tool completed the three selected primary families: Flowchart has
64 distinct DOM blockers and 34 stale receipts, State has 12 DOM blockers, Sequence
has 25 DOM blockers and four stale receipts. Counts cover Mismatches only, not
accepted Notes. Inventory: `target/mermaid12-layout-closeout-mismatch-inventory.json`.
Usecase was also requested but is outside the primary matrix and was not run;
do not count the command as a fourth-family receipt. All other 34 primary families
pass all three DOM modes in `target/mermaid12-layout-closeout-control-families.log`.

The post-sorting integration run still passes 229/231: 63 layout goldens differ
(45 Flowchart, 17 State, one Usecase), and the old State007 model-order assertion
still fails. Exact selected goldens were regenerated with their existing owner;
original bytes and before/after hashes are preserved in target for independent
cause review. They are not yet committed or admitted as upstream parity evidence.

State013 and three statements015 aliases have same-size provider and full paint
replays proving their remaining path differences arise from measurement. However,
independent whole-fixture review blocks State receipts: ordinary rectangles still
hard-code Neo radius3 rather than the source's effective theme radius (usually12,
5 for the reviewed040 context). Fix that real theme omission before new signatures.

State007 has another proven source defect: compound hierarchy edge publication
must publish inner child exports before a containing node's own outside edges.
Changing both the published provider's parent ports and external dummy order from
[edge7,edge6] to native [edge6,edge7] reproduces native endpoints exactly. Neither
text dimensions nor merely changing one of those two orders explains it. The old
assertion remains intact; a source-compatible compound fix is being implemented.

Sequence's current inline note font weight matches the source, but its generated
CSS still forces note tspans to theme weight600. The pinned source explicitly removed
that rule so custom/inline note weight can inherit. This is a real cascade defect,
not font measurement residual, and is being removed before note receipt review.

Flowchart's narrowed text-residual review also found Neo shape-size omissions:
stacked documents need source axis padding, wave amplitude and label offset; lean
and trapezoid variants need source horizontal padding. The adjacent shape audit is
checking shared layout/paint/intersection/bounds formulas rather than tuning each
fixture. Existing receipts are not broadened to hide these differences.

# Verified provider snapshots and doctests

Independent review accepted all 63 selected goldens: three direct-parent boundary
changes, 36 Neo brace size propagations, four source-replayed self-loop fixes and
20 remaining provider-order changes. Every saved before hash equals HEAD's previous
golden; node/edge/label identities and semantic properties are preserved. This is
local snapshot admission, not a claim of complete upstream coordinate parity.
All 12 snapshot/Usecase tests pass in `target/mermaid12-provider-golden-tests.log`.
The snapshots are committed in `a10ad820f`. Source/cause details remain in
`target/mermaid12-model-order-golden-review.md` and `-review-causes.json`.

All required doctest commands pass: renderer 5, SVG facade 1, full workspace 33
with four previously ignored roughr cases. Logs are respectively
`target/mermaid12-render-doctests.log`, `target/mermaid12-facade-doctests.log`, and
`target/mermaid12-workspace-doctests.log`. These runs predate the next isolated
State radius, compound publication and stacked-document patches.

Sequence CSS was transferred after baseline-hash verification and its four focused
CSS tests pass. Compilation exposed the now-unused Sequence-specific theme field;
that field and initializer are being removed while retaining the shared theme
variable and GitGraph behavior. The CSS fix is not yet committed.

# Continued refactor after closeout checkpoint

The browser-text residual signature for `stress_br_in_messages_notes_011` was refreshed
after the source-backed Sequence note-weight change and its three catalog tests pass.
The workspace Clippy check initially found a test-only `type_complexity` warning in the
new ELK comparison schedule table; the case tuple is now named in `09d245ffe`, and the
three-package all-target/all-feature Clippy check passes again.

State minimum label width now follows the pinned default of 120 while preserving an
explicit `state.minNodeWidth: 0`. The value is propagated through layout settings and
SVG label emission for ordinary leaf rectangles and notes; group titles, special state
markers, and edge labels remain outside that rule. A cross-composite regression exposed
that ordinary rectangles use the renderer's fallback shape branch, so the same helper is
used there as well. Commit `d9325285e` passes all 41 State layout/SVG tests. A fresh
`compare-state-svgs` run still reports 12 distinct DOM blockers; these are route and
measurement residuals, so no State receipts were admitted.

The next isolated Flowchart Neo batch covers stadium, delay, and display geometry across
layout, paint, intersections, and viewBox bounds. Independent review found two blockers
before transfer: stadium sampling tests expected 102 points although the pinned source
produces 103, and the current implementation derives width from sampled points and then
samples again in layout, shrinking the theoretical source width twice. The batch remains
in the isolation tree until those source-semantic issues are corrected and re-reviewed.

# Flowchart Neo curved-shape transfer

The stadium, delay, and display batch was transferred as `6d64c3774` after independent
review. Stadium now preserves the theoretical source width/height while sharing its
sampled outline across paint and intersection; Delay and Display use the source cap and
minimum-size formulas with the same shared point geometry. Obsolete duplicated circle
helpers and the no-longer-used trig table were removed after Clippy identified them as
dead code. The source-shaped layout contract was corrected in the existing aggregate
test: doublecircle uses the label diagonal plus the classic ring gap, and curved shapes
assert theoretical dimensions rather than sampled-bbox shrinkage.

Validation is 45/45 `flowchart_layout_test`, 77/77 `flowchart_svg_test`, the focused
Neo geometry unit tests, and the three-package all-target/all-feature Clippy check. A
fresh `compare-flowchart-svgs` run still reports 62 distinct DOM blockers, so this batch
closes shape semantics without constituting Flowchart admission. Remaining blockers are
primarily route/path geometry and the unrepaired Neo shape groups listed in the current
state document.

# Continued Neo shape and ELK label convergence

Source-backed Neo shape batches now cover triangular, cylindrical, sloped, notched,
bow-tie, tape/tag, document, window/divided/stacked rectangle, and shaded-process
geometry. The latest commits include `b3090f939`, `d9b095bed`, `2d39ced27`,
`57110ec9c`, and `9bb1ccba7`. Layout, paint and intersections share the pinned source
formulas; the stacked-rectangle intersection follows its stepped polygon rather than
its rectangular envelope. Sloped rectangles now reuse the cached label measurement,
preserving minimum-width and host-measurement contracts during paint. Paired braces
follow upstream right/left/hidden-rectangle insertion order. Two label-text-dependent
RoughJS string substitutions were removed instead of hiding float-rounding residuals.

The 96-fixture newshape run in `target/compare/flowchart_neo_shape_refresh.md` still
fails. It predates the following ELK fix and is not final admission evidence. Read-only
browser/source replay explains SVG markdown wrapping threshold crossings: one candidate
line measures 125.236900px in the browser but 116.34px in the deterministic profile,
straddling the unchanged 120px wrap width. Remaining edge skeleton differences must
be reviewed separately; matching word content alone does not admit a whole fixture.

Commit `a7ebb2301` fixes a distinct provider-boundary defect. Mermaid does not expose
leaf labels to ELK; the adapter previously projected paint measurements into fixed node
labels in both layered paths. Hidden bolt/circle labels then created 85px and 106px
margins. Equal-size elkjs 0.9.3 replay and a numerical regression prove the resulting
191px accumulated offset. Both layered paths now project labels only for non-empty
groups, while preserving all measured leaf labels for SVG rendering. Independent
review checked the pinned Mermaid graph constructor, Java importer, other family
callers and both hierarchy modes. All 1,481 all-feature renderer/adapter unit tests
pass, including the hidden-shape metadata and group-title regressions.

The main workspace analysis/editor WASM artifacts remain stale. The last valid isolated
candidate still exceeds analysis gzip/Brotli budgets by 38,257/38,501 bytes and editor
budgets by 8,058/19,272 bytes. Read-only attribution reproduced the retained encoding
tables; that is an optimization lead, not an accepted sanitizer change or measured
implementation saving.

The next source-backed label fix (`e2c943b34`) propagates measured SVG bbox-y
compensation through stacked rectangles, shaded processes and all brace aliases.
A public host-measurement regression injects a 7px offset and confirms the SVG label
shift while HTML labels stay unchanged; all seven label measurement contract tests
pass. The three-package all-target/all-feature Clippy check also passes.

A fresh rebuilt three-mode run with the existing exact browser-residual policy reports
Flowchart 56 distinct DOM blockers plus 34 stale receipts, State 12 blockers, and
Sequence 25 blockers plus three stale receipts. Inventory:
`target/mermaid12-shape-label-mismatch-inventory.json`; raw log:
`target/mermaid12-shape-and-label-convergence.log`. No new receipts are admitted.
The first diagnostic run omitted the receipt flag; its 75/12/28 raw counts must not
be compared directly with the policy-enabled counts above.

The affected layout/SVG integration run passes 138/139 tests. The remaining aggregate
identifies 310 changed snapshots (308 Flowchart, one Agentflow, one State). Their old
bytes and HEAD hashes are preserved under `target/mermaid12-neo-golden-review/` before
existing-owner regeneration and independent cause review. Do not count generated
candidates as admitted upstream geometry.

# Reviewed paint fixes and current Web size measurements

All 310 selected layout candidates were independently approved and committed in
`1c9e7190e`: full before/after hashes, identity/topology preservation, 26 source
shape-formula groups, all nine routing-only pinned elkjs replays, and the late State
compound-publication correction are recorded under
`target/mermaid12-neo-golden-review/independent-review-final.{md,json}`. The complete
layout aggregate passes with these expectations; this is local regression admission.

Whole-fixture SVG review then found actual paint omissions that structural comparison
alone did not detect. Commits `6eba8dbc8`, `fe4f9a3ff`, `7c861d0fb`, and `f0b0d4db2`
fix Flowchart/State small terminal shadows, effective rounded-rectangle radius and
source hand-drawn arcs, State minimum-width HTML boxes, and Sequence actor order,
Neo shadows, inherited glyph stroke and activation palettes. Raw theme truthiness is
preserved, including numeric zero versus nonempty string zero. Independent source
oracles cover 13 radius configurations, five paths, 16 Sequence order cases, and
12 activation-palette cases. The 1,604-test renderer unit/integration batch passed
1,602 initially; both remaining failures were new fixture expectations, not production
failures. Pinned package initialization proves null radius overrides are ignored and
the Neo theme does not enable nodeShadow. Corrected focused reruns pass 2/2.

The latest post-P3/popup three-mode comparison still has Flowchart 52 DOM blockers
plus 34 stale receipts, State 11, and Sequence 25 plus four stale receipts. The fresh
run is recorded in `target/mermaid12-post-popup-family-convergence.log` and the per-family
reports under `target/compare/`. Whole-fixture review has identified bounded residual
candidates, but no new receipts are admitted yet. State style remains held for equal-size
provider order. Sequence popup geometry is now source-backed and tested; its old receipt
candidates must be regenerated before admission.

Five production Web WASM packages and TypeScript/package assembly built successfully
from the pre-paint-fix source. The actual assembled package size matrix reports:

| Profile | Raw bytes | Stripped bytes | Gzip bytes | Brotli bytes |
| --- | ---: | ---: | ---: | ---: |
| analysis | 3,542,295 | 3,542,030 | 1,412,226 | 1,087,900 |
| ASCII | 5,101,613 | 5,101,348 | 1,926,940 | 1,468,106 |
| editor | 3,655,648 | 3,655,383 | 1,457,351 | 1,119,502 |
| full | 12,946,551 | 12,946,286 | 4,862,740 | 3,603,109 |
| render | 10,983,599 | 10,983,334 | 4,190,230 | 3,109,923 |

Only four compressed caps fail: analysis gzip/Brotli by 37,226/37,900 bytes and
editor by 7,351/19,502 bytes. Budgets and capabilities remain unchanged. Logs:
`target/mermaid12-current-web-build.log` and `target/mermaid12-current-web-size.log`.
The full/render artifacts need rebuilding after the final renderer/kernel repairs;
analysis/editor/ASCII have no dependency on those edited renderer/kernel paths.

# ELK randomized-attempt source identity

An exact-size six-node complete DAG proves a remaining P3 defect: ELK's FIRST_TRY
and SECOND_TRY properties use the same string ID, and Property equality is ID-based.
Native code incorrectly stored independent flags, preserving model order for two
tries instead of one. Giving the pinned elkjs SECOND_TRY a distinct ID reproduces the
old native TB/LR positions exactly; restoring the shared identity produces the source
crossing sequence `[4,4,4,3,3,3,3]`. Source evidence and phase traces are under
`target/neo-p3-shared-property-diagnosis.md`. The repair is implemented in flat and
hierarchical paths with a public deprecated diagnostic alias retained for alpha.6
compatibility. Independent code review approves it.

The current full three-crate library run passes 1,910/1,910 tests. The shared-ID
property repair and its source-backed nested-order assertion are both green. Do not
admit regenerated routing snapshots before the remaining family-level cause reviews.

# Sequence popup geometry convergence

Commits `11aec36fb` and `d9c74c3a3` align actor popup placement with Mermaid 12 `drawPopup`: mirror mode selects footer actor rectData, classic/Neo and collection/queue/database heights follow the draw order, radii follow actor type and look, long-link widths use actor-font measurements, and root bounds include the same panel geometry. Focused popup/order/palette tests pass 3/3; the complete `sequence_svg_test` passes 56/56 (one feature-gated test skipped), and `merman-render` Clippy passes with `-D warnings`.


# Final Web rebuild and packaging verification

After the renderer, ELK shared-property, sequence popup, and CLI packaging fixes, all five
browser WASM profiles were rebuilt from the current source with the pinned `wasm-size` profile
and `wasm-opt` pipeline. The package group was assembled and its input manifests were verified;
TypeScript contract generation checked 35 WASM exports, 45 runtime bindings, and five package
entries. Runtime smoke rendered all 37 registered diagram families for every package, and DOM
safety smoke passed.

The current measured Web artifacts are:

| Profile | Raw bytes | Stripped bytes | Gzip bytes | Brotli bytes |
| --- | ---: | ---: | ---: | ---: |
| analysis | 3,544,491 | 3,544,268 | 1,413,098 | 1,089,462 |
| ASCII | 5,104,406 | 5,104,183 | 1,927,647 | 1,469,075 |
| editor | 3,658,011 | 3,657,788 | 1,458,179 | 1,119,888 |
| full | 12,959,005 | 12,958,782 | 4,866,624 | 3,606,264 |
| render | 10,997,717 | 10,997,494 | 4,196,074 | 3,116,821 |

The complete `full` and `render` profiles remain below their compressed limits. `analysis`
exceeds gzip/Brotli by 38,098/39,462 bytes and `editor` by 8,179/19,888 bytes. These are
unchanged slim-profile budget blockers; no threshold was raised and no sanitizer was added.

The CLI default ELK migration is now coherent across the Cargo manifest, feature matrix,
installation contract, process matrix, license-scope test, and CLI README. The default and
`cli-release` closures both include the EPL-2.0 ELK and RaTeX/font material, while explicit
no-default-feature builds remain available. The installation, process-matrix, feature-matrix,
license, legal-projection, Web build, package, and smoke checks pass at this boundary.


# Slim WASM compiler experiments

Two source-preserving A/B experiments were run against the analysis profile after the final
rebuild. Candidate A overrode only `CARGO_PROFILE_WASM_SIZE_OPT_LEVEL=s` while keeping wasm-pack,
Binaryen, strip, and compression unchanged. It produced 4,253,215 raw bytes, 4,252,992 stripped,
1,657,633 gzip, and 1,234,516 Brotli bytes, materially worse than the official 3,544,491 /
3,544,268 / 1,413,098 / 1,089,462 result.

Candidate B applied only `wasm-opt -Oz --converge --all-features` to the official analysis input.
It produced 3,452,981 raw bytes and 3,452,621 stripped bytes, but compressed to 1,414,857 gzip
and 1,093,702 Brotli bytes, both worse than the official pipeline. Both candidates are rejected;
no compiler profile, Binaryen flags, capability set, or budget was changed.

# Open gates

The transferred provider/brace batch and source-compatible model-order correction are
verified. Remaining gates are Rust/doctest, SVG receipt admission, final WASM rebuild and
size verification, license/consumer checks, and source-backed Flowchart/State/Sequence
residual review. Do not accept old reports overwritten by focused negative-evidence tests;
exact browser residuals require reviewed evidence rather than blanket signature refresh.

# Recovery verification on 2026-09-25

The strict verifier reached the workspace nextest stage but its default Windows linker
parallelism exhausted memory (`LNK1102`). A serial recovery run with `CARGO_BUILD_JOBS=1`
and two nextest test threads compiled and executed all 8,693 workspace tests: 8,692 passed,
one layout-golden aggregate failed, and seven were skipped. The failure listed exactly 14
goldens whose expected values predated the already reviewed Class, Flowchart, Sequence, and
State layout changes; no source assertion failed.

Those 14 files were regenerated individually with `xtask update-layout-snapshots` and the
dedicated layout test passed 3/3. The focused refresh is committed as `59191dd81`
(`test(layout): refresh post-convergence golden snapshots`). No other tracked paths were
staged. A subsequent renderer-wide attempt was stopped before test execution by
`LNK1201` while writing a PDB, because the shared target drive had only 85 MB free; the
affected debug artifacts are generated build output and the source tree is unchanged.

After clearing only Cargo's generated `target` cache, the renderer gate was rerun with
`CARGO_BUILD_JOBS=1` and test/debug information disabled to keep Windows linker pressure
bounded. `cargo nextest run -p merman-render --all-features --no-fail-fast --test-threads 2`
completed with 2,016 tests run, 2,016 passed, and 2 skipped. Workspace Rustdoc validation
also passed: `cargo test --workspace --doc` completed all supported doctests, with only the
four existing `roughr` examples marked ignored. `cargo fmt --all --check` and `git diff --check`
are clean.

The complete strict verifier was rerun with serial Cargo jobs and reduced test debug info. Its
117-build feature matrix, generated contracts, alignment evidence, 14 artifact closures,
third-party license and legal projections all passed. Workspace nextest passed all 8,693 tests
with 7 skipped; all supported workspace doctests passed, with four existing `roughr` examples
ignored. The final SVG DOM comparison stage remains blocking: Flowchart has 52 distinct DOM
mismatches and 34 stale local text-layout receipts, State has 11 DOM mismatches, and Sequence
has 25 DOM mismatches plus four stale receipts. The reports cover 1,153, 285, and 321 rendered
fixtures respectively. The first Flowchart blockers are source geometry differences in rounded
cluster/edge paths and shape outlines, not browser text residuals. No receipt was refreshed and
the parity gate remains strict. The verifier stopped at Flowchart, so later family parity
commands were not reached in this strict run.

# Flowchart route and slim-WASM follow-up on 2026-09-25

Focused inspection of `stress_flowchart_cluster_dense_children_021` confirms that at least part
of the Flowchart path drift begins before SVG curve emission: corresponding ELK edges
`L_b2_c3_0` and `L_c2_a3_0` have different routed point sequences in `data-points`, and use
different channels around the same child cluster. The local and upstream path commands therefore
cannot be reconciled by changing the rounded-curve serializer. A separate `shape_mix_009` route
contains a roughly 2e-6 endpoint segment in the upstream output that the local route does not;
that small numerical case does not explain the channel-routing mismatches. No fixture-specific
route rewrite or comparator normalization was added.

The analysis profile's dependency tree confirms that its current capability closure includes the
full `merman-core` parser and semantic family catalog, `merman-analysis`, and `lol_html`; the latter
directly depends on `encoding_rs`. The measured `encoding_rs` decoder/encoder code is not an
orphaned dependency that can be dropped without changing HTML processing behavior. Existing
`opt-level=s` and extra `wasm-opt -Oz --converge` experiments already made compressed output worse,
so this follow-up found no source-preserving compiler or feature-closure change that resolves the
analysis/editor budget overages. Their limits remain unchanged pending a real size reduction or an
explicit product decision.

# Hierarchical ELK random-state handoff on 2026-09-25

The dense-child crossing routes traced to the hierarchy sweep's random-state ownership.
Pinned ELK `62d5909f96fad541bc101ad52dabaece6b7eab7e` shares each `LGraph` random generator
between `GraphInfoHolder`, the sweep, and later orthogonal routing. The Rust hierarchy sweep
copied those generators but did not return their consumed states to the graphs. It now writes
the root and child streams back before transferring node/port order, including early convergence
and both greedy-switch strategies. The flat sweep already performed this handoff.

A regression first failed with six route points instead of four, then passed with both affected
edges matching all pinned upstream coordinates. A lower-level test covers all three sweep
strategies when the node order remains unchanged. Independent correctness review found no
blocking issues. The combined ELK provider, adapter, and all-feature renderer run passed 2,505
of 2,506 tests; the only failure identified two expected layout goldens. Those two route-only
snapshots were refreshed individually, and the complete layout snapshot harness then passed
3/3. No other golden changed.

The complete Flowchart/State/Class comparison was rerun in structure, parity, and parity-root
modes with three-decimal precision and exact browser-text-layout receipts. Flowchart DOM
mismatches decreased from 52 distinct fixtures / 111 rows to 50 / 107; its 34 stale receipts
remain blocking. Both corrected fixtures pass all three modes. State remains at 11 fixtures /
22 rows with no stale receipts. Class passes with its three existing accepted residual comparisons.
No comparator or upstream SVG was changed. The two State controlled measurement experiments
passed: fixed upstream label bounds reproduce both the 0.046875px parallel-port jog and the
three previously failing compound route topologies. This supports the measurement-residual
classification without changing production clipping or comparator policy.

# Slim WASM budget review on 2026-09-25

The maintainer authorized a reasoned budget review for the remaining analysis/editor overages.
An identical Rust 1.95.0, wasm-pack 0.15.0, `wasm-size`, wasm-opt, strip, gzip and Brotli
recipe was run on the same Windows host for pre-upgrade `54d257aa8` and the current graph.
The normal dependency name sets were identical for both `analysis` and `editor`. The analysis
artifact grew from 3,454,447/3,454,224/1,358,830/1,044,105 raw/stripped/gzip/Brotli bytes to
3,544,491/3,544,268/1,413,098/1,089,462. Editor grew from
3,567,757/3,567,534/1,404,294/1,071,993 to
3,658,011/3,657,788/1,458,179/1,119,888. The current analysis module's code section is
2,694,808 bytes versus 2,537,041 before the upgrade, while its data section is smaller
(838,438 versus 906,522). This is consistent with the added Mermaid 12 parser, diagram and
semantic capability surface, not an accidental dependency or compression regression.

The existing roughly three-percent-headroom policy is retained. The analysis caps are now
3,675,000 raw/stripped, 1,475,000 gzip and 1,125,000 Brotli; editor caps are 3,775,000
raw/stripped, 1,525,000 gzip and 1,175,000 Brotli. These remain above the measured artifacts
with explicit margin, while the unchanged full/render profiles retain their previous budgets.
The size experiment and raw logs remain under `target/bench/experiments/mermaid12-slim-size-budget`.
No dependency, sanitizer, compiler, compression pipeline or capability was removed or altered.
The budget file is a measured product decision, not a blanket gate bypass. Remaining family
parity and final integrated checks are still open; this is not completion of U1–U15.

# Strict verification after budget review on 2026-09-25

The strict verifier was rerun after the budget change with serial Cargo jobs. All 117 feature
builds, generated contracts, alignment checks, 14 artifact closures, open-source license
projections, and workspace nextest passed (8,697 tests, with the repository's skipped cases).
The run reached the integrated SVG suite and stopped at the existing Flowchart parity blockers;
it did not report a new failure in the budget, feature, license, or Rust test stages. Current
family counts remain Flowchart 50 distinct DOM fixtures / 107 rows plus 34 stale receipts,
State 11 / 22 with no stale receipts, Sequence 25 plus four stale receipts, and Class 0.

# Citations

- [Mermaid 12 plan](../../../../plans/2026-09-20-1250-refactor-mermaid-12-alignment-plan.md)
- [Default ELK boundary](../../../../adr/0089-mermaid-12-default-elk-products.md)
- [Performance measurement contract](../../../../performance/BENCHMARKING.md)

# Browser residual receipt cleanup on 2026-09-25

The exact browser-text-layout diagnostic suite was rerun after the 38 reviewed local-signature
refreshes. The receipts were then reduced to the modes that still differ: 16 entries became
fully matched and were removed, one remaining Flowchart entry lost its matched `structure` mode,
and the catalog now contains 81 entries. Input hashes and pinned upstream SVG hashes were
unchanged; no comparator rule or production renderer behavior was relaxed. The committed
catalog integrity test was updated from 97 to 81 entries.

With `--diagnostic-browser-text-layout`, the Flowchart suite now accepts 55 exact residual
comparisons and reports no stale receipts; its remaining blockers are 69 distinct fixtures /
162 mode rows. Sequence accepts 9 exact residual comparisons and reports no stale receipts;
its remaining blockers are 28 distinct fixtures / 84 mode rows. The remaining Flowchart rows are
route/curve/marker or text-structure differences, and the remaining Sequence rows are actor,
note, message and block wrapping differences. They still require source-backed fixes or an
explicitly documented browser-measurement boundary; no new receipt is admitted by this cleanup.

`cargo nextest run -p xtask --no-fail-fast --test-threads 2` passed 599/599 tests with serial
Cargo jobs. The focused diagnostic comparisons intentionally remain non-zero because the actual
69 Flowchart and 28 Sequence DOM blockers are still present.


# Session recovery and font consistency on 2026-09-26

Recovery imported historical root `01a0d984-0472-7632-8fa6-b81f9b584223` into the receiving
session. Its Sequence review is historical evidence; its interrupted component implementation
was reconciled with the working tree before continuation. Neither persisted child handle was
reported as live. The receiving session and subsequent repository state are now authoritative.

The selected graph remains Mermaid 12.0.0 at `98a0945418c76238f15df2afaddbba4272656c3b`
and Eclipse ELK at `62d5909f96fad541bc101ad52dabaece6b7eab7e`. The selection identity remains
`9634181804174022a42a1288be799a7612226a4a95b3374642a32515348b40e7`, with receipt SHA-256
`5a77f613f2d1f380ddaa5d9bab5e8111adb2cd833f319462a916694bebf10e28`.
`xtask verify-mermaid-reference --materialized` passed again against the current checkout.
No package graph, source pin, upstream SVG or comparator was changed.

Commit `92cd0ab1a` makes Sequence select the source-authoritative font weight before CSS
validation, accepts static relative and global CSS keywords, and shares that result between
layout measurement, actor labels and menus. Invalid truthy global weights no longer reveal
a family fallback; numeric and string weights produce identical menu geometry and root bounds.
The focused Sequence run passed 140 tests, and renderer all-target/all-feature Clippy passed.
Independent correctness and standards review found no remaining issue in that change.
The family comparison still has 25 unadmitted DOM differences and three existing exact
browser-text-layout residuals. Dynamic CSS weight expressions were not added by this change.

# Layered components and single-graph hierarchy work on 2026-09-26

The flat layered executor now follows the pinned `ComponentsProcessor` and
`SimpleRowGraphPlacer`, including DFS order, one shared random stream, priority/area sorting,
component spacing and aspect ratio. Dense arena indices are restored across component-local
processing, including long-edge dummies, labels and self loops. Root minimum size is retained
and applied after combination. The translations remain in the existing EPL-2.0 crate with
source references in `components.rs`; no new license or feature boundary was introduced.

`INCLUDE_CHILDREN` retains compound semantics even without nested nodes. Moving those graphs
onto the correct pipeline exposed an overlarge upfront hierarchy sweep charge: the existing
`upstream_docs_diagrams_flowchart_code_flow` fixture required 915,430 work units against its
unchanged 800,000 limit. The single-graph Barycenter path now charges actual randomized
attempts and sweeps while retaining the same hierarchy algorithm. Every sweep admits the
structural upper bound for free/fixed port visits and complete reference rewrites. Initial
state, crossing counts, snapshots and final transfer are charged separately. Nested graphs,
greedy strategies and nonzero node model-order influence keep their prior admission path.

The regression compares complete graph and random state with the original hierarchy sweep,
covering single nodes, crossing graphs, long edges/self loops and both initial-order choices.
Zero, partial, one-unit-short and exact budgets prove that interruption remains enforced.
The original failing Flowchart fixture now renders within the unchanged limit and passes
`parity-root` without an accepted residual. The component tests separately cover upstream
isolated-node packing, chain node/edge translations, continued randomness, root minimum size,
and restored label/dummy/self-loop references. ELK and adapter all-target Clippy passed.

Source-backed snapshot review identified 7 Flowchart and 15 State layout updates from
component processing, plus 61 pending Sequence goldens from the earlier Neo endpoint fix.
All seven Flowchart cases passed individual upstream `parity-root` comparisons. The State
concurrent-region and divider examples now match upstream node coordinates and root viewports;
the full State comparison retains the same eleven previously reported DOM blockers.
The existing `xtask update-layout-snapshots` owner refreshed only the 83 identified goldens.
A recursive JSON review found only numeric geometry changes, with no node, edge, label or
field additions/removals. Remaining family parity and final integrated verification are still
open; these results do not complete U1-U15.

The final affected-crate all-feature nextest run passed 2,522 tests across 61 binaries,
with two existing skipped tests. The focused renderer integration run passed 92 tests
with one existing skip. Standards and correctness review reported no remaining findings
in the component and single-graph work-accounting changes.

# Wrapped Sequence actor measurement on 2026-09-26

`calculateActorMargins` in the selected Mermaid 12 source measures the final actor
text after optional wrapping. The local wrapped branch still used a fixed 17/16 font-size
height instead of the selected text measurer. A host returning four 24px lines reproduced
68px instead of 96px in classic mode. Wrapped and unwrapped actors now share the actual
measurement path and retain their distinct classic/Neo row-height rules.

Regression coverage includes explicit breaks and automatic wrapping, with exact classic
96px and Neo 152px rows. The focused Sequence run passed 146 tests; the final integration
run passed 62 tests with one existing skip. Renderer all-target/all-feature Clippy passed.
Seventeen Sequence layout goldens were refreshed through the existing owner. JSON review
found only y/height/max-y changes (397 numeric fields), and all three layout snapshot tests
passed. Independent standards and correctness review found no remaining issue.

The latest three-family comparison has Flowchart 45 distinct unadmitted fixtures / 91 mode
rows, State 11 / 22, and Sequence 25 / 75, with no stale residual receipts. Rerunning Sequence
after this fix retains the same 25 / 75 blockers. No comparator or residual catalog was
modified. Box title measurement/rendering remains a separately identified source mismatch.

# Sequence box title and frame geometry on 2026-09-26

Mermaid 12 `calculateActorMargins` and `drawBox` use measured box title dimensions,
source box coordinates, and one shared vertical cursor. The renderer now retains wrapped
box labels, measured title height, box x/width, and final cursor height in the prepared
artifact. SVG emission reuses those values, emits titles through the same `byTspan`
line model as actor labels, and applies `box.starty = box.y - boxMargin / 2` plus the
source bottom padding formula. Empty titles still receive width/margin calculations but
produce no background or title node.

Regression coverage passed 152 focused Sequence tests, including host-measured multiline
titles, automatic title wrapping, custom box margins, mirror/no-mirror frames, created
participants, empty boxes, and narrow unnamed boxes. The Sequence layout snapshot suite
passed 71 tests with one existing skip; all-target/all-feature Clippy passed. A fresh
read-only correctness and standards review found no remaining findings. Twelve affected
Sequence layout goldens changed only numeric y/max-y fields.

The Sequence comparison still has 25 unadmitted fixtures / 75 mode rows. One existing
browser-text receipt (`upstream_cypress_sequencediagram_spec_should_render_with_wrapping_enabled_048`)
now reports local-signature drift in all three modes after the actor measurement fix; the
receipt was not changed. This remains a blocking verification result, as intended by the
receipt contract.


# LSP admission for Agentflow and Usecase on 2026-09-26

The editor and language surfaces already had parser-complete semantic facts, recovery input, completion, navigation, rename, and Tree-sitter grammar/query fixtures for Agentflow and Usecase, but the LSP capability contract still classified both public types as not yet admitted. Commit `18097c666` moves both types into the explicit first-class LSP admission list and adds their two rows to `docs/lsp/CAPABILITIES.md`; the matrix now covers all 37 public diagram types.

The focused admission/editor run passed 87 tests, the complete `merman-lsp` nextest passed 292 tests, and the Tree-sitter package nextest passed 26 tests. `xtask verify-editor-language-contract`, `verify-binding-contract`, `verify-generated`, and `check-alignment` passed. Third-party license verification, release legal material projection checks, and governed Cargo package legal-material checks also passed. These results close the R8 admission evidence for these two families; final U14/U15 parity, size, and integrated gates remain open.
