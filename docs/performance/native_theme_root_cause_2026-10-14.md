# Native theme root-cause checkpoint, 2026-10-14

Base `3a4b8c393939b69372a8f05a2b971f7cdf6ae529`; audited branch `f47abded02b540d7fe563af7bc75f2fa1bb90c51`.
This investigation follows the [native benchmark](native_main_theme_2026-10-13.md) and precedes production repairs.

## Evidence boundary

Seven specialist agents audited core configuration, family preparation/evidence, standalone SVG finalization, three nonoverlapping output-family groups and shared architecture. A separate Class/Kanban cross-review challenged evidence-removal assumptions.
One-off native probes link the exact existing release libraries; they do not patch production or Criterion. All 38 dumped SVG byte counts and SHA-256 values match the original benchmark preflight. Model/layout JSON, pinned Mermaid 12.1.0 runtime probes and Chromium computed styles supplement source evidence. No profiler time or browser probe is added to the original latency observations.
The benchmark uses default Engine configuration including diagram-scoped defaults. Flowchart/State/Sequence/ER resolve their scoped Redux/Neo defaults; Block/Mindmap use their respective default/classic values. No Merman preset is selected.
The pinned Mermaid baseline is 12.1.0, source `21f72f07ea22c0af48a3149c550654e80d8e40cb`. Installed package source maps and committed family baseline manifests are primary sources. An unrelated source checkout at a different commit was excluded.
Raw evidence lives in `target/bench/experiments/native-main-comparison-20261013/{analysis,svg-output}`. The immutable original diagnostic and confirmation JSON remain unchanged.

## Confirmed behavior failures

1. Theme evaluation reason/source merge order is wrong in reachable scoped transitions. Head keeps `nodeBkg=#cccccc` after a Flowchart source primary color `#123456`; pinned Mermaid and main calculate `#123456`. Same-theme scoped source selection incorrectly recalculates, and truthy unknown root theme incorrectly skips calculation. Invalid source color is also incorrectly accepted during scoped recalculation. The branch is already correct for unchanged global ordinary updates and falsy root controls, where main is wrong: a blanket rollback would regress them.
2. Valid typed font `A&B` causes XML errors in 28 of 38 family fixtures. Every Safe Font control succeeds. The failing owners write logical CSS into XML style text without a terminal encoding boundary. Correct controls include Mindmap, Venn, Railroad variants, Wardley, Agentflow and Usecase; do not reject ampersands or weaken XML validation.
3. Non-palette Neo Swimlane computes `nodeBorder` rather than configured `clusterBorder`. Main and pinned upstream agree on the correct border. The default palette change in head is correct and must remain.
4. Block nodes ignore authored stroke widths including zero, Neo square padding, and round radius. Rounded visual arcs also incorrectly define edge intersections where pinned upstream uses a rectangle. Native and upstream source/browser witnesses establish each gap. These inherited correctness gaps are in the maintainer-requested repair scope, but are not all newly introduced branch regressions.

### Full font coverage

Failed parity fixtures: `flowchart_medium`, `class_medium`, `state_medium`, `sequence_medium`, `er_medium`, `info_medium`, `pie_medium`, `journey_medium`, `timeline_medium`, `gantt_medium`, `requirement_medium`, `gitgraph_medium`, `c4_medium`, `sankey_medium`, `quadrant_medium`, `block_medium`, `packet_medium`, `kanban_medium`, `architecture_medium`, `radar_medium`, `treemap_medium`, `xychart_medium`, `swimlane_medium`, `eventmodeling_medium`, `treeview_medium`, `ishikawa_medium`, `cynefin_medium`, `error_basic`.
Passing parity controls: `mindmap_medium`, `zenuml_medium`, `venn_medium`, `railroad_medium`, `railroad_abnf_medium`, `railroad_ebnf_medium`, `railroad_peg_medium`, `wardley_medium`, `agentflow_basic`, `usecase_basic`.
Both parity and resvg-safe were tested with Safe Font and A&B for each fixture. Strict portability probes separately retain expected missing-font-resource failure; emitting CSS alone does not certify self-contained fonts.

## Every output-different fixture

| Fixture | Exact changed surface | Disposition |
| --- | --- | --- |
| flowchart_medium | Document-owned IDs and source-order nodes | Intentional collision repair/upstream order; geometry and paint unchanged by semantic owner |
| state_medium | One leading empty style declaration | Serialization only; preserve behavior |
| sequence_medium | Scoped filter refs and exact/broader native marker CSS | Correct existing native/filter repair; variants retain bounded coverage |
| er_medium | Four marker attribute-order changes | Serialization only; parsed DOM identical |
| mindmap_medium | 184 quote entities in CSS | Correct XML serialization; decoded DOM identical |
| block_medium | Node height 32 to 40 and stroke viewport envelope | Correct classic geometry; N04–N06/N25 repair other variants |
| gitgraph_medium | CSS variable font to resolved literal | Expected binding; computed default font unchanged |
| radar_medium | Adds title fill rule | Correct glyph-paint binding; no title in fixture |
| treemap_medium | CSSOM serialization of four leaf styles | Serialization only; computed paint unchanged |
| venn_medium | Trebuchet to Recursive Variable | Correct theme font precedence; pinned runtime confirms actual change |
| swimlane_medium | Generated IDs/data identity and removes all-lane border override | Correct default palette repair; N03 repairs non-palette Neo |
| eventmodeling_medium | Root fill/color and explicit role font size | Correct SVG glyph inheritance; source CSS remains available |
| treeview_medium | Explicit font and eight retired proof IDs | Keep font repair; remove unused raster proof protocol (N08) |
| ishikawa_medium | Normalized typography and nineteen bbox attributes | Keep correct typography; remove unconsumed bbox output (N07) |
| railroad_medium | Same-root font selector and CSS XML serialization | Correct root matching/escaping |
| railroad_abnf_medium | Shared Railroad owner changes | Correct root matching/escaping |
| railroad_ebnf_medium | Shared Railroad owner changes | Correct root matching/escaping |
| railroad_peg_medium | Shared Railroad owner changes | Correct root matching/escaping |
| agentflow_basic | Generated IDs and source data identity | Intentional collision repair; geometry unchanged |

All nineteen differences have a source-backed disposition. Equal model/layout projections alone do not establish visible paint equality. In particular, Venn font and Swimlane palette changes are real correct paint changes; Block default geometry changes are correct. ER/State/Mindmap serialization equivalence is separately demonstrated. Original same-byte admission exclusions remain intact.

## High-confidence repair inventory

Confidence is high that each listed defect or unused work exists. It is not a claim that a source change has a measured speedup. Implementation must preserve the actual consumer/error/resource contract.

| ID | Priority | Finding | Unit |
| --- | --- | --- | --- |
| N01 | P1 | Theme rematerialization uses winner origin instead of root-directive truthiness and actual selected-theme transition | U1 |
| N02 | P1 | Logical stylesheet font values reach XML text without terminal escaping; 28/38 A&B fixtures fail | U2 |
| N03 | P2 | Neo non-palette Swimlane border is overridden by generic node CSS | U3 |
| N04 | P2 | Block node CSS ignores authored strokeWidth including zero | U3 |
| N05 | P2 | Block square geometry ignores Neo-specific padding | U3 |
| N06 | P2 | Block round geometry ignores authored Mermaid radius truthiness | U3 |
| N07 | P2 | Ishikawa emits unconsumed per-text bbox metadata | U4 |
| N08 | P2 | TreeView and Flowchart/Swimlane retain raster proof IDs after consumer retirement | U4 |
| N09 | P2 | Legacy theme transactions copy and destroy the complete config rather than local theme state | U5 |
| N10 | P2 | Same-value config assignments detach JSON although only authority metadata changes | U5 |
| N11 | P2 | Derived-ownership pass allocates dependencies when no explicit or compatibility authority exists | U5 |
| N12 | P2 | Legacy transaction regression selects staged Base and misses Default/Extended | U5 |
| N13 | P2 | Class constructs full terminal proof objects before CompletionOnly discards them | U6 |
| N14 | P2 | Class collects all label style facts without checking all actual consumers | U6 |
| N15 | P2 | Class metrics-only helper constructs and discards XHTML style facts | U6 |
| N16 | P2 | VisibleTextStyleFacts retains unused copied content/kind and repeatedly derives summaries | U6 |
| N17 | P2 | Kanban builds label classification when no paint/typography evidence consumer requests it | U6 |
| N18 | P2 | Mindmap baseline prepares evidence-only edge identity/provenance | U6 |
| N19 | P2 | An empty root theme builds an empty prelude allocation | U8 |
| N20 | P2 | BestEffort admission builds detailed reference occurrence evidence then discards it | U7 |
| N21 | P2 | Validated documents without expansion/effects still build indexes and reference graph | U7 |
| N22 | P2 | Explicit non-resvg pipelines discard exact-generation XML proof at String boundary | U8 |
| N23 | P2 | Owned terminal validation is deep-cloned into finalization report then dropped | U8 |
| N24 | P2 | Consuming family completion clones style evidence and temporary strict checks clone it again | U8 |
| N25 | P2 | Block round rectangles incorrectly route edges through the visual rounded arc instead of the upstream rectangular boundary | U3 |

## Performance cause and safe direction

Class, Requirement, Flowchart and Sequence enter scoped Redux materialization on each operation; Journey/Kanban/XY Chart do not. Legacy transaction code adds a full-config COW transaction and destruction; shared profiling confirms these owners execute. Same-value assignments and empty-authority dependency propagation also perform demonstrated unused work. Local theme resolution followed by one successful commit is preferred over caching.
Class constructs rich terminal objects before CompletionOnly discards retention, and metrics-only helpers classify XHTML then discard the facts. Class completion remains meaningful: duplicate/missing/foreign identities, marker/source-owned paint and one-time seal must be preserved with compact canonical slots. Class paint demand includes NodeLabel even when no typography receipt is requested. Kanban facts similarly serve Text/TaskLabel/typography consumers; existing ticket/assigned geometry is correct and stays.
Ordinary standalone SVG now performs required XML/reference admission that main bypassed. The existing validator already fuses XML and reference collection. Its summary consumer nevertheless generates detailed occurrence arrays/postorder and discards them. The same algorithm can request only summary facts, retaining cycle, duplicate-ID, effect/marker amplification and user budgets. Reference-free eligibility must come from validated facts, not textual/family/trusted-input gates.
Owned validation/style results are cloned at consuming seals and temporary strict checks; move or borrow at those owners. Explicit non-resvg pipelines lose exact-generation structural proof at a String boundary and revalidate; a private immutable proof-carrying buffer may transfer existing facts, invalidating them after arbitrary mutation. No grammar-aware renderer framework is required.
The Class finalizer has 719 inclusive CPU samples, with a budget-check subtree of 653 in a 3621-main-thread-sample capture. Counts overlap and do not quantify removable exclusive time. Independently batched stages cannot be subtracted from complete rendering, and their routine-local destruction is timed.

## Rejected or unproved interpretations

- Main is not universally correct. Rollback would undo source-only theme, Venn, palette, geometry and ID fixes.
- Two distinct artifact/resource hashes are not two interchangeable identities; native/public identical-byte artifact hashes already reuse the same digest.
- Strict validation fusion, resvg fingerprint duplication and old quick-xml attribute quadratic behavior are not current default-path findings; existing optimizations and pinned quick-xml 0.42 invalidate those claims.
- Source directives with color limits 64/65 cannot supply cScale12+ because the generated directive-key filter removes those keys. The proposed positive control was invalid, and its raw failed probes are retained. No resource bypass was established from it.
- Class CompletionOnly cannot skip validation or compare totals alone; no-typography is not sufficient to skip facts used by actual NodeLabel paint.
- A&B is valid. A&amp;B contains a semicolon and is rejected by typed FontStack; it is not a valid typed-font XML-decoding counterexample.
- Late cancellation after identity work and deep-XHTML stack behavior remain hypotheses until deterministic probes prove an existing contract failure.
- Immutable coverage caching, hash traversal coalescing and Engine family-base reuse require separate measured justification; they are not required mechanisms.

## Evidence map

| Artifact | Purpose |
| --- | --- |
| `svg-output/receipt.json` | Exact release linkage and 38 SVG preflight hashes |
| `analysis/core-config.md` and `core-{semantic,selection,errors}-*.json` | Config/source truthiness, derivation and error witnesses |
| `analysis/all-family-font-receipt.json` | 152 public safe/A&B × pipeline executions |
| `analysis/output-core-families.md`, `output-chart-families.md`, `output-tail-families.md` | Nonoverlapping 19-fixture coverage and owner/source dispositions |
| `analysis/block-variants-output/` and `analysis/chart-variants/` | Native/upstream geometry and computed-style counterexamples |
| `analysis/family-pipeline.md` and `family-evidence-cross-review.md` | Demand, integrity and actual timed lifecycle |
| `analysis/svg-finalization.md` and `architecture.md` | Resource ownership, consuming copies and rejected shortcuts |

Primary repair inventory: [issues.json](../workstreams/native-theme-regression-20261014/issues.json).
Implementation plan: [2026-10-14-0053-refactor-native-theme-regressions-plan.md](../plans/2026-10-14-0053-refactor-native-theme-regressions-plan.md).

## Implementation discoveries

Correctness work exposed seven additional owner-local issues; they are added to the inventory rather than omitted from the repair.

| ID | Finding | Evidence and disposition |
| --- | --- | --- |
| N26 | The new XML-text adapter scanned an arbitrarily long plain CSS fragment before observing cancellation or a refused append | Two actual cancellation counterexamples failed; direct UTF-8-safe fragment flushes now use the original bounded sink without temporary strings or new budgets |
| N27 | Final XML validation accepted forbidden literal XML 1.0 characters, including normalized attribute character references | Five owner-local counterexamples failed while three legal/name/declaration controls passed; the existing streaming visitor now checks text, CDATA, comments and normalized attributes |
| N28 | Legacy theme calculation lost derived compatibility ownership while the physical source was still Tracking | Owner-local and public Engine counterexamples failed after freezing; `b994d79d7` reads surviving calculation inputs without changing the renderer's frozen-admission predicate; 198 core controls pass |
| N29 | An existing viewport test compared Dagre fallback against an ELK golden when the ELK feature was absent | Eight backend controls prove main, audited head, current code and pinned Mermaid agree on each backend's vertical bounds; a test feature guard fixes the recipe, with the actual ELK and two Dagre fallback tests passing |
| N30 | Font normalization rejects angle brackets inside legal quoted CSS font strings | The public host-font case retains its effective configuration but loses the CSS font; focused normalization tests fail; fix the font boundary while preserving generic source-style security and existing host dynamic CSS capabilities |
| N31 | Accepted deeply nested XHTML exhausts the recursive XML parser stack before style classification | The admitted-input subprocess aborts at XML parsing; a package-contained private import of the existing parser with its official iterative element fix passes strict/loose depth 4,096 on a 2 MiB thread and the upstream depth 200,000 regression |
| N32 | Public SVG/document identity sealing can return an artifact after cancellation | Deterministic public-entry counterexamples fail before `d5ae01105`; existing operation control is observed before and after identity work, and both publication controls pass again |

Raw logs and source/browser witnesses are under `analysis/implementation/` in the existing ignored experiment. N26–N29 have targeted green evidence; N30 and all remaining performance units are still in progress at this checkpoint. Counts from a broader command with an independent failed case are retained as partial results, not described as a complete passing suite.

The recovered implementation continuation verifies N30 and the U5–U8 structural candidates with
3,232 renderer library tests and 93 facade tests, including all 152 font/pipeline scenarios.
Final workspace, independent review, feature/package, parity and renewed timing gates remain
separate evidence; these focused results do not establish a latency improvement or a completed PR.
The current [workstream](../workstreams/native-theme-regression-20261014/WORKSTREAM.json) records
their actual dispositions without replacing the original main comparison.


## Final implementation checkpoint

The continuation committed consumer-bound facts, reference summaries, consuming finalization and
stack-safe XML analysis as `2ab878e70`, strict lint boundaries as `984b37e96`, and the private
feature-matrix recipe as `512589484`. Integration discovered four further high-confidence issues:

| ID | Finding | Verified disposition |
| --- | --- | --- |
| N33 | Railroad strict font receipts compare XML-encoded stylesheet values with logical font values | Decode once at the existing terminal evidence boundary; 46 focused tests pass |
| N34 | Block-only builds reach label/background helpers hidden under the Flowchart feature | Shared family owner and direct label projection; independent Block build passes |
| N35 | C4-only builds import RoughJS helpers through disabled Flowchart | Shared parity owner; independent C4 build passes; moved algorithms remain identical |
| N36 | Strict matrix requests undeclared singleton features from a private partial harness | Derive private cases from declared features while keeping production canonical selector validation mandatory; 23 contract tests and 660 actual build cases pass |

The final workspace has 12,827 passing tests and 11 existing skips. All workspace doctests,
strict Clippy, materialized upstream references, generated/legal projections and full SVG
structure/parity/parity-root comparisons pass. The CI-equivalent release comparison also passes
with root reports. Independent review covers all current changed code paths with zero findings.
The strict aggregate stops later in Web construction at Binaryen133's `approximateOr` assertion;
both local and official same-version binaries reproduce it. The temporary-tool investigation,
remaining browser checks and native latency confirmation are still pending. This checkpoint
establishes implementation and structural evidence, not a completed U9 or latency improvement.


The platform continuation resolves that tool failure as external to the renderer: the official
Binaryen133 executable aborts on the exact saved Web optimizer input, while version133 plus
upstream fix `ca1cf60a6b41fbb20c4d0c1bfb98523b9cabe8ee` retains assertions and completes the
same raw input and the actual five-package build. All 149 Web tests, package/DOM-safety smoke,
eight Playground scripts (79 non-Chromium and 170 Chromium tests) and 255 extension tests pass
with those built artifacts. Original Binaryen133 remains a failed tool gate; the temporary fix
is recorded separately and does not change the repository's tool policy.

Seven focused root contracts and 3,073 private theme acceptance tests pass. The Chromium root
oracle examines 3,712 SVGs with zero blocking findings, retaining 54 browser diagnostics,
2,344 upstream-inherited cases and one exact residual. Native resource smoke verifies compiled
1x/100x operation and zero-work boundaries only; it does not establish allocation improvement.
The full-capability Criterion list verifies 459 benchmarks and 459 preflight receipts across
seven lane groups. Fresh native latency discovery and confirmation remain pending.


The [native repair closeout](native_theme_repair_2026-10-14.md) records the completed renewal.
Final code `ac4a40437` matches all 38 corrected-control outputs. Fourteen frozen confirmation
cases establish non-regression against `d5ae01105`, with no registered confirmed improvement.
Against main, nineteen fixtures match exact output and the thirteen-case confirmation
intersection retains ten material regressions and three inconclusive outcomes. Mindmap restores
main's stylesheet bytes; Usecase now differs only by the 222 quote entities removed by U2,
with its complete decoded DOM and CSS unchanged. The original output-disposition table above
remains the historical investigation. Remaining main latency is active attribution work;
structural closure does not imply full performance recovery.
