# Remaining Theme Cleanup Execution — October 10, 2026

This is the implementation record for the reopened
[plan](../plans/2026-10-10-1535-refactor-theme-addition-necessity-plan.md), following the
[call-chain review](theme_remaining_code_review_2026-10-10.md) at `a987774ff`.
The reopened implementation and local verification are complete. This record covers the
authorized local refactor; remote delivery gates have not been run at this source.
No speed, memory or binary-size gain is inferred from source deletions.

## Exact Sequence declaration proof (U7/U9, F3)

Commit: `c2eb20866`. The writer proof now counts every matching declaration, including multiple
properties inside one rule. It retains the existing bounded observation grammar and does not
interpret browser CSS cascade. Native raster proofs remain separate.

The existing duplicate-rule test now also covers identical same-rule values and an earlier
`!important` value followed by the expected value. Before the fix, the important-predecessor
case failed its rejection assertion. After the fix, all 13 `c6_sequence_proof` tests passed:

```text
CARGO_BUILD_JOBS=2 python3 scripts/run_theme_acceptance.py nextest run --locked \
  -p merman-theme-acceptance --lib -E 'test(c6_sequence_proof)' \
  --test-threads 2 --cargo-quiet
```

The whole library was initially invoked without diagram features; two preset tests rejected
Flowchart capability before reaching their assertions. That invocation is not a passing receipt.
The final owner run must use its actual diagram feature recipe. Focused Sequence proof tests
operate on observed SVG fixtures and passed under the stated recipe.

## Core overlay and family tests (U2/U6/U9, F2/F7/F9/F10)

Core commit `3b907235e` deletes the private host/fallback contribution executor, injectors,
application/provenance and empty production state. The live GitGraph/ER raw paint authority
freeze moves to `config/paint_inputs.rs`, consuming detector changes directly. Same-value
assignments, nested replacement, scoped appearance, secure filtering and keyword ELK tests
remain. This batch adds 352 and deletes 2,247 source/test lines: net 1,895 fewer lines.

Family test commit `ec39d2169` retains the 12 geometry executions and both palette/Sequence
consumers in local case tables. It removes three redundant renders and 166 net test lines.
Math assertions now inspect each native math subtree, and a negative control rejects empty
groups even when unrelated paths remain. The Swimlane test now counts backend render/measure
calls before and after SVG emission, retaining its sidecar and HTML checks. Production math
did not change; these are stronger test oracles, not evidence of a production math failure.

Bindings commit `f38bafabc` retains raw complete-spec reconstruction. Carrying its typed value
would require a context-aware Serde seed through the options/base/request-overlay/authoring
owners, a duplicate envelope schema or a placeholder payload. No measured benefit justifies
that complexity. A six-case test through `parse_options` confirms that raw payload errors
precede schema/layout/CSS while envelope errors retain their later position.

The integrated Core/family/theme owners passed 2,117 tests:

```text
CARGO_BUILD_JOBS=2 cargo nextest run --locked \
  -p merman-core -p merman-render -p merman-bindings-core \
  --features merman-render/all-diagrams,merman-render/math,merman-render/layout-elk,merman-bindings-core/all-diagrams,merman-bindings-core/svg,merman-bindings-core/layout-elk \
  --lib -E 'package(merman-core) | test(family::tests) | test(theme::tests)' \
  --test-threads 2 --cargo-quiet
```

An earlier invocation enabled ELK only on the renderer and failed the binding's advertised
capability check. The corrected recipe above enables both owners; no source gate was weakened.
Workspace formatting and `git diff --check` passed.

Playground commit `02ecec568` keeps one external-theme limitation in the export menu. Existing
mobile recipe/share interaction now requires exactly one explanation. TypeScript checking,
`build:prepared` and that Chromium browser case passed. The built Web runtime was reused for
this UI-only verification; it is not a fresh-WASM receipt for later Rust changes.

## Second-wave boundary decisions (U4–U7)

Implementation settled on October 11. The source commits and current verification below
attest this second wave; earlier first-wave receipts remain separately revision-scoped.

The artifact receipt is constructor-owned and immutable: its fields are private, accessors return
shared references or copied values, and it has no deserializer or mutator. Construction still
observes the real SVG and computes canonical identity. `proves_artifact` now compares the nonzero
requested artifact identity directly, retaining actual-document binding. The unchanged-constructor
characterization passed before removal of its redundant canonical rehash: matching artifact,
zero identity, unrelated identity and cloned receipt are covered by the existing receipt test.
The characterization and current private renderer library both passed after the change.

The private Core compatibility constructor is renamed `try_new` with its real compiler and test
callers; the obsolete “without family overlays” name acquires no compatibility alias.
Materialization still checks authored series shape in `validate_definition_shape` before its sole
private token resolver. The resolver removes its second identical shape check, retaining color
validation. Existing typed/JSON empty and oversized series/error-order cases remain the owner.

The raw complete-spec decoding decision above remains intentional. Playground's current
`materializeTheme` accepts definitions, whereas the editor accepts both definition and complete-spec
recipes. `svgPlanJson` uses the canonical binding admission for both forms; replacing it with
materialization would narrow the accepted input or introduce a new public operation. Retain the
fixed Flowchart probe in the full browser runtime; it is coupled to that runtime's SVG/Flowchart
capability, and no pure complete-recipe operation or current supported-user failure was found.
Flutter's raw member projection likewise remains necessary to preserve duplicate JSON fields and
the original options byte charge. Neither retained implementation establishes global minimality.

GitGraph typography and text paint share the full expected commit/tag sequence with independent
receipt cursors. Node paint retains its separate commit-label and tag cursors: filtering two
complete streams would scan the other event kind unnecessarily. No mode flag or universal evidence
iterator is added to erase those distinct access patterns.

Current Fuzz documentation drops the two font targets absent from `fuzz/Cargo.toml`. The migration
guide no longer recommends an unavailable embedded catalog backend. Dated historical records retain
their original revision and do not serve as current feature promises.

## Complete private font-chain retirement (U4)

The second implementation removes `PreparedTextLayout`, its candidate/request/backend/fallback
model, production session slots that were always `None`, State/Flowchart catalog branches and
the inactive Sequence text sidecar. `FontCatalog` has one successful system-font constructor and
now stores only its established fingerprint. Public asset/face/alias getters remain empty;
System/NoEmbedding, readable wire/report types, host measurement and unsupported-font errors
remain. Explicit text capability requirements still affect the recipe report and identity.

The catalog's retained-resource pass only checked five constant zero values. Immutable empty
getters cannot exceed any nonnegative ceiling, and those checks have no cancellation or observable
side effect. It and its direct calls are removed. The theme's actual effect-resource re-admission
remains. Private face/table/decoded-expansion helpers lose their nonexistent decoder owner;
the public resource IDs, profiles and hard caps remain. The existing hard-cap test uses the
policy checker for those retained schema fields instead of preserving an unused decoder algorithm.
Raw asset bytes/counts, aliases, encoded input and preflight limits still have live checks.

Ordinary prepared source labels, entity/Unicode projection, measurement, retained-byte
reservations and mathematical sidecars retain their real owners. Sequence CSSOM still rejects an
invalid inline font before falling back to the inherited valid font. Flowchart metric style and
wrapping style remain distinct, including ELK and Swimlane. State retains measured geometry,
HTML wrapping, source precedence and post-writer evidence. Deleting private catalog preparation
does not delete these independent paths.

Native export identifies reserved prepared-label IDs using the existing canonical ID parser.
Unknown or invalid tokens, an unexpected receipt, missing evidence and artifact/count mismatch
still fail. Nonempty private catalog receipts cannot be produced by a public renderer, and their
former asset-face matcher could never match system-only observed faces; the closed branch now
rejects explicitly. Public native font-plan diagnostics and host-dependent system/glyph/fallback
observation remain. The new `system_font_export_rejects_reserved_labels_and_missing_prepared_evidence`
test covers the empty case, grouped base/line identity, valid/invalid reserved tokens and missing
prepared evidence. Two additional tests parse an actual `usvg::Tree` using existing licensed
subset fonts: they verify system-face classification, unadmitted-face deduplication and missing
glyph/fallback reporting, plus base/line/invalid ID collection and a line-only counterexample.
All five font-environment tests passed on the final source; no host-installed font is required
by the two new Tree tests. Lower-level synthetic mutation/projection rejection tests remain distinct.

Dead test-only font preparations retire with their owner. Private range/digest constructors used
by surviving synthetic ledger tests compile only for tests; the unused range adapter disappears.
The reservation getter remains test-only because resource reconciliation and cluster negatives
still use it. This preserves actual shared resource-meter checks and does not add a new charge to
ordinary labels merely to give a retired helper a caller.

## Local fact admission and observation (U5/U7)

Gantt, Journey, Requirement, ER and Block validate a complete receipt before moving it into
immutable `OnceLock` storage. Later local property queries read admitted facts rather than scan
all checkpoints again. ER's never-written incomplete bit is removed. Expected values remain
independent from actual writer checkpoints. Journey negatives construct independent candidates
rather than add `Clone` to a production receipt.

Block's shell XML observer is required only when an actual paint expectation exists. An empty
pending rule set alone does not authorize skipping it: mixed unsupported geometry and supported
paint still need observation. Separate canonical shell, geometry, role/count, typography and label
checks remain. The new five-case `node_paint_receipt_tracks_actual_paint_demand_without_changing_writer_evidence` test compares
actual output/receipt demand and retains missing, duplicate, mutated and second-seal rejection.

Ishikawa resolves its one static Text fill outside label iteration through the existing direct
paint resolver. Kanban carries one small actual terminal-facts value through emission; expected
aggregate facts remain separately calculated. GitGraph's shared commit/tag iterator retains
independent cursors and tests reversed tags, hidden commit labels, merge/custom labels and empty
tags. Venn replaces a duplicate font string and fixed mask with a stylesheet-observed flag,
retaining invalidation on a second write. Timeline retains ordered slots and permanent invalidation;
TreeView retains total/paintable/unverified icon validation after removing its unread counter.
These are maintenance and work-structure changes; no latency or allocation benefit is claimed.

## Simplification and lint scope

Three independent read-only passes applied the `ce-simplify-code` reuse, quality and efficiency
rubrics to `a987774ff` through this working tree. Reuse and efficiency returned no additional
actionable findings. The quality pass identified two obsolete parameters on the private
`write_svg_text_source_word_lines_impl`: every remaining caller supplies `true` and `1.1`.
Those values now remain in the body, and its argument-count exemption is removed.

The CLI workflow's actual strict Clippy owner passed with `--all-targets --all-features -D warnings`.
An additional broader invocation selecting the renderer's own complete test targets reported
existing unrelated test lint debt; it is not a passing receipt or a required substitute for the
CLI owner. Production warnings identified real private support boundaries: the borrowed requested
layout mirror is removed, while no-op/external math fixtures and the root-SVG test accessor compile
only under their real test/internal-acceptance callers. Compiled math remains a production backend.
No warning rule or user/resource assertion was weakened.

## Retained boundaries and remaining-module dispositions (U1)

The original [107-module index](theme_pr_addition_necessity_review_2026-10-10.md#disjoint-module-accounting)
continues to assign the entire frozen PR once. This batch closes its demonstrated candidates;
it does not replace representative source review with a claim that every added line is minimal.
Earlier decisions and receipts remain in the [first execution record](theme_addition_cleanup_execution_2026-10-10.md).

| Previously uncertain owner | Current disposition and concrete reason |
| --- | --- |
| Infrastructure/text/catalog | Retire the private font construction chain and constant-empty state described above; retain real source-to-visible projection, measured ordinary labels, cancellation and resource reconciliation |
| GitGraph fallback/shape/family writers | Retain actual BestEffort static fill and concrete shape/terminal writers; typography/text paint share ordering, while node paint keeps its different cursor pattern |
| State observation | Retain `StateThemeTerminalReceipt::observe_svg`: writer ranges and actual occurrences distinguish missing branches, HTML/source overrides, filtered occurrences and special/composite shapes; a plan or generic final traversal does not provide these facts |
| SVG pipeline | Retain `StandaloneSvgArtifact::finalize_exact` and final validation at changed/external artifact boundaries; prepared math and postprocessing can change bytes, and no smaller reusable validation-fact owner was demonstrated |
| Native filters | Retain `preflight_native_filter_receipt` and `resolved_tree_matches`: raw units/references/regions and resolved tree applications are different representations, not duplicate checks of one admitted value |
| Core Treemap | Retain `treemap_compatibility_output_preflight` before JSON expansion; cancellation, saturating byte estimates, nested/flat copies and repeated class styles protect allocation before constructing the amplified value |
| xtask comparator | Retain family-local normalization and unknown-syntax rejection in `svgdom.rs`; no semantic normalization or general CSS interpreter is introduced |
| Bindings/Playground/Flutter | Retain the separately explained raw complete-spec decode, full-recipe browser probe and raw request projection; their error, accepted-schema and original-byte contracts differ |
| Release scripts and other hosts | Retain bounded compiler/archive/source checks and canonical shared Rust dispatch; no public protocol, dependency, feature declaration or generated interface changes in this reopened batch |
| Historical documents/performance evidence | Retain dated source-bound decisions, digests and provenance under `docs/ARCHIVE.md`; correct current promises in migration/Fuzz guides, and keep old cost measurements explicitly revision-scoped |

Each remaining concrete family, public contract/transport, static asset, attribution, generated
projection and adjacent release module retains its indexed responsibility. Earlier local changes
already retired the private fixture crate, historical route classification and unpublished share
input. That disposition closes the known scope; it is not exhaustive certification of all shape,
test, platform or release branches, and does not promise that no future simplification exists.

## Test ownership after retirement (U9)

| Previous test or case owner | Current observation | Coverage consequence |
| --- | --- | --- |
| Three `unverified_flowchart_node_surfaces_keep_typed_*_fail_closed` tests | `unverified_flowchart_node_surfaces_keep_typed_geometry_fail_closed`: 3 patches × 2 source surfaces × 2 modes | All 12 executions remain; only repeated mechanics are shared |
| Later radius-8/choice-node case and earlier geometry case | Geometry table retains the same source, patch and both outcomes; later mixed gradient/stroke remains | Remove two duplicated renders, retain distinct combined-paint case |
| `state_structured_gradient_is_a_semantic_residual_not_verified_source_css` | `require_portable_rejects_state_semantic_residual_before_layout` also checks Unverified and empty source residuals | Remove one duplicated BestEffort render; Strict rejection remains |
| Separate Flowchart/Swimlane palette and Activation/Note precedence bodies | Local family/target tables retain both consumers and independent mechanism/residual expectations | Fewer functions are not fewer behavioral cases |
| Sequence duplicate-rule proof | `c6_sequence_proof` also checks same-rule properties and the important predecessor | Strengthen exact declarations; independent PNG proofs remain |
| Whole-SVG path and constant backend output assertions | Three compiled-math tests inspect nonempty paths inside each native math group; `native_math_geometry_observation_ignores_unrelated_paths` rejects unrelated paths; Swimlane counts backend calls across emission | Stronger geometry and reuse oracles; no production math defect inferred |
| Private catalog-backed preparation and prepared-only State branches | Retire self-tests with the unavailable constructor; retain Sequence's three CSSOM validity/fallback tests, ordinary source projection, geometry and resource-ledger negatives | Removing unreachable behavior does not retire supported host measurement |
| Preconstructed native observations alone | Two actual Tree tests above cover face/glyph classification and reserved IDs, including a line-only counterexample | Add independent final-tree coverage; five current font-environment tests pass |
| Receipt completeness and Block shell checks | Missing/duplicate/mutated/second-seal negatives remain; the five-case demand test renders actual family output | Do not derive actual writer evidence from the expected plan |
| Binding complete-spec errors and artifact identity | Six error-order cases plus matching/zero/other/cloned artifact assertions | Preserve boundary order and document binding without pinning a private collection |

This is a behavior/boundary mapping of changed candidates, not a generated coverage manifest.
Function count, looped case count and source lines are different measures. No matched test timing,
allocation or whole-render performance improvement is claimed.

## Final source accounting (U8)

Final code commit: `72574bbfd9fd02fea761c704020ec2c6da218901`. Second-wave commits are
`7e460dec9` (private font chain), `b7bf07841` (local facts) and `72574bbfd` (immutable receipts).
Their staged blobs were compared byte-for-byte with the frozen code used by tests, artifact
builds and source review. The 76-path code-identity inventory has SHA-256
`bad686516c25d9b7caf50178278fc565b3d65017aaf73f56ef3aaffd44f9d903`;
the inventory includes the independently reviewed native test supplement. No code changed
after that inventory was captured.

Git NUL-delimited numstat with rename detection compares `a987774ff` with the final tracked tree.
This execution record is excluded to avoid counting its own evolving text. The disjoint total is
**2,465 additions, 11,915 deletions, net 9,450 fewer text lines across 81 paths**; no binary path
changes in this reopened batch. Workspace manifests, dependencies, feature declarations, public
schema, budgets and generated legal/interface projections have no change from the reopened base.

| Disjoint file classification | Added | Deleted | Net | Paths |
| --- | ---: | ---: | ---: | ---: |
| Shared Rust including inline tests | 1,621 | 10,911 | -9,290 | 71 |
| Rust independent tests and acceptance | 491 | 987 | -496 | 3 |
| Documentation | 350 | 13 | +337 | 5 |
| Playground source | 0 | 3 | -3 | 1 |
| Browser tests | 3 | 1 | +2 | 1 |
| **Total, excluding this record** | **2,465** | **11,915** | **-9,450** | **81** |

Rust files contribute **2,112 additions and 11,898 deletions**, net **9,786 fewer lines**.
A conservative, overlapping test-body view recognizes independent Rust test/acceptance files
and trailing top-level `mod tests` bodies: **1,185 additions, 2,916 deletions**, net **1,731 fewer
recognized test lines**. The remaining **927 additions, 8,982 deletions**, net **8,055 fewer
shared source/test-helper lines**, include helpers outside those recognized spans; they are not
all production code. These views must not be added to the file classification table.

The largest individual reduction is `text/prepared.rs`: 153 additions and 4,256 deletions,
from 4,691 to 588 lines. It retains the source CSS decisions and lower projection/report schema;
it no longer carries a nonexistent catalog backend.

The following directory groups assign all 81 reopened paths once. Unlike the original 107-module
family partition, SVG writers have their own `render/svg` group here; inline and independent tests
stay with their directory owner. This is a current cost view, not another count of the original PR.

| Reopened directory owner | Added | Deleted | Paths |
| --- | ---: | ---: | ---: |
| `crates/merman-bindings-core` | 45 | 0 | 1 |
| `crates/merman-core` | 355 | 2,253 | 6 |
| `crates/merman-export` | 169 | 227 | 1 |
| `render/block` | 234 | 22 | 1 |
| `render/diagram_theme` | 68 | 222 | 6 |
| `render/infrastructure` | 39 | 579 | 6 |
| `render/er` | 28 | 16 | 2 |
| `render/family` | 321 | 576 | 3 |
| `render/flowchart` | 84 | 941 | 5 |
| `render/gantt` | 15 | 10 | 1 |
| `render/gitgraph` | 135 | 132 | 3 |
| `render/ishikawa` | 26 | 84 | 1 |
| `render/journey` | 21 | 6 | 1 |
| `render/kanban` | 60 | 60 | 2 |
| `render/math` | 17 | 6 | 2 |
| `render/requirement` | 5 | 4 | 1 |
| `render/sequence` | 10 | 570 | 4 |
| `render/state` | 91 | 1,105 | 5 |
| `render/svg` | 178 | 762 | 14 |
| `render/swimlane` | 10 | 14 | 1 |
| `render/text` | 156 | 4,272 | 4 |
| `render/timeline` | 0 | 5 | 1 |
| `render/tree_view` | 1 | 5 | 1 |
| `render/venn` | 7 | 16 | 1 |
| `crates/merman-theme-acceptance` | 37 | 11 | 1 |
| `docs/performance` | 257 | 2 | 2 |
| `docs/plans` | 88 | 2 | 1 |
| `docs/release` | 4 | 4 | 1 |
| `docs/security` | 1 | 5 | 1 |
| `playground/source` | 0 | 3 | 1 |
| `playground/tests` | 3 | 1 | 1 |
| **Total** | **2,465** | **11,915** | **81** |

The direct comparison with frozen main `2928265122d5a3d35e239e3ddbce885c7e342c59`, also
excluding this execution record, is **395,933 additions and 40,364 deletions across 1,670 paths**,
including **104 binary paths**. This is computed directly, not by subtracting two independently
reported addition counts. The original 107-module inventory keeps its original denominator;
remaining responsibilities, test data, attribution and historical records are not certified minimal.

## Final local verification

All builds were serialized with `CARGO_BUILD_JOBS=2`; nextest used `--locked` and two test threads.
Counts below overlap where owners cover different features or trust boundaries, so they are not
an aggregate number of distinct behaviors.

| Current-source owner | Result |
| --- | --- |
| Core/render/export/bindings libraries and affected materializer, font, source, family SVG and support integration targets; renderer math/ELK/Cytoscape, native PNG/JPEG/PDF and binding ELK features enabled | 5,842 passed, 4 skipped |
| Private acceptance/renderer/export/facade libraries through `run_theme_acceptance.py` | 2,731 passed, 2 skipped |
| Private preset qualification and Block/Class/Flowchart projection integration owners, `png,all-diagrams` | 9 passed |
| Private all-diagram support discovery | 52 passed |
| Fresh release `family::evidence_support::tests`, no default features | 13 passed, 1 manual benchmark ignored |
| Fresh release preset qualification, `png,all-diagrams,layout-cytoscape` | 11 passed |
| Facade authoring, markers, geometry, edge effects, render operation and composed effects, `all-diagrams,svg,png,pdf,layout-cytoscape` | 92 passed |
| Final native font-environment tests, including two actual Tree tests | 5 passed |
| Published consumer/private-import boundary script | Public producer-feature consumer compiles; 8 private imports reject with E0432 |
| Full `compare-all-svgs --check-dom --dom-modes structure,parity,parity-root --dom-decimals 3 --diagnostic-browser-text-layout --report-root` | All three blocking modes pass |
| Root viewport oracle on that comparison output | 3,712 fixtures; 0 blocking containment failures; existing browser/upstream residual ownership retained |
| CLI workflow strict Clippy owner, `-p merman-cli --all-targets --all-features -- -D warnings` | Pass |
| Web build, five actual package smokes and DOM safety | Pass; 42 exports, 53 runtime bindings and 5 package entries checked through TypeScript |
| Typst minimal / complete feature owners | 14 / 34 passed |
| Typst publish package and shared WASM authoring vectors | Pass; support=22, materialization=2, errors=3 |
| Actual Typst 0.15.0 package smoke | 23 fixtures compile; 9 expected failures verified |
| Canonical native CLI, `cli-release` / dist / aarch64-apple-darwin | Build passes; Flowchart/Sequence/XYChart Cyberpunk each export SVG and PNG, 6 successful commands |
| Fresh Playground typecheck/build and Chromium desktop theme selection | Pass; 50 browser tests pass |
| Firefox/WebKit terminal-text observation smoke | 46 pass |
| Workspace fmt, Git diff whitespace and changed-doc relative links | Pass |

The first release comparison attempt encountered an obsolete Core object from a shared build
cache, reporting the removed constructor against current compiler code. Refreshing the current
Core release cache rebuilt the comparison successfully; no compatibility alias was restored.
The two optimized runs above were repeated after that refresh, and their fresh logs supersede
older release receipts. All five Web input manifests were checked against current file hashes,
with no outside-workspace or retired-file entry. Typst and native build logs show the current
Core and renderer compilation. The extra broad renderer-test Clippy invocation remains a failed,
non-required baseline-debt diagnostic; it is not relabeled as a pass.

## Canonical artifact measurements

The existing [artifact recipes](../../capabilities/artifact-profiles-v2.json) and
[fixed budgets](../release/WASM_SIZE_BUDGETS.json) remain unchanged. Web measurements use
assembled package WASM; Typst stripped/compressed measurements include its existing Binaryen 131
`-Oz` and strip steps. Rust 1.95.0 and the existing owner compression algorithms were used.

| Artifact profile | Raw bytes | Stripped bytes | Gzip bytes | Brotli bytes |
| --- | ---: | ---: | ---: | ---: |
| `web-analysis` | 3,818,332 | 3,818,067 | 1,507,877 | 1,154,940 |
| `web-ascii` | 5,351,272 | 5,351,007 | 2,013,546 | 1,528,392 |
| `web-editor` | 3,931,084 | 3,930,819 | 1,553,297 | 1,186,091 |
| `web-full` | 16,245,910 | 16,245,645 | 6,074,894 | 4,490,051 |
| `web-render` | 14,341,619 | 14,341,354 | 5,422,528 | 4,011,585 |
| `typst-wasm` | 19,381,058 | 11,775,574 | 4,543,608 | 3,369,467 |

All **24 budget metrics pass**. The actual Typst publish package WASM is **11,775,574 bytes**,
SHA-256 `59deb0f0d45f75b98302db278e4858c42479c9c0aa113f4dc806a171bbab6ad4`.
The native CLI is **53,622,208 bytes**, SHA-256
`a95827a22cdc22507255179432f0b49b09980b3dc68dac2d32d205118afa0ca6`.
These are fresh absolute sizes, not a matched before/after proof of theme savings. No A/A timing,
allocation or memory experiment was performed, and no such improvement is claimed.

## Independent review and delivery boundary

The actual eight-lens source review of `a987774ff` through the frozen working tree completed
with no actionable finding. Two advisory native Tree coverage gaps were fixed, independently
reviewed in two supplemental passes and dynamically verified above. Three simplification lenses
also completed; the one accepted quality suggestion removed constant private writer parameters.
Review artifacts: `20261011-003647-1db74dc8`. The source review and supplemental test review are
separate receipts; the original frozen diff is not retroactively rewritten.

The final independent delivery reconciliation completed with **Ready** for the authorized local
scope and no findings (`delivery-review.json` in the same review run). It binds the current
committed source, accounting, owner results and artifact identities without rewriting the original
source-review verdict. The reviewers are Codex subagents; no external provider was used and
different-model independence is not claimed.
Remote CI, other operating systems and previously migrated installed language consumers have
not been rerun at this final source. They remain delivery/platform limits; earlier receipts do
not substitute for them. The authorized local goal performs no merge or publication.
