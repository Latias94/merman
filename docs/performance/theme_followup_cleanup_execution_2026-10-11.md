# Theme follow-up cleanup execution — October 11, 2026

## Scope and evidence boundary

This execution implements the eight confirmed findings from the follow-up review of main
`2928265122d5a3d35e239e3ddbce885c7e342c59` through branch
`36488c8e26f3dc1c4248549a24a897f00d3ba2d6`. The implementation comparison starts at that
branch revision; it does not repeat or add together the earlier cleanup's source counts.
The owning [necessity plan](../plans/2026-10-10-1535-refactor-theme-addition-necessity-plan.md)
and [earlier execution](theme_remaining_cleanup_execution_2026-10-10.md) retain their original
revision and verification boundaries.

The review established eight actionable findings. It did not inspect every changed line of the
large PR or establish that all other code is minimal. This execution uses existing consumer and
artifact checks, without a new compatibility layer, evidence framework, dependency or CI budget.

## Implementation and coverage ownership

| Finding | Change | Retained or strengthened owner |
| --- | --- | --- |
| 1. Numeric whitespace | Reuse the existing ECMAScript whitespace predicate when deriving the color-operation limit | Real Engine inputs distinguish U+0085 from U+FEFF; the 64-operation limit remains enforced |
| 2. Bound class lookup | Look up selected `classDef` entries directly and sort by their existing definition ordinal | Definition order, unknown/duplicate selections and budget-error ordering; Flowchart, Swimlane and Agentflow SVG consumers |
| 3. Unreachable prepared text | Retire the private nonempty label ledger and its derived proofs after tracing every production constructor | Raw/postprocessed reserved-token rejection, actual usvg font observation, resource and cancellation ordering, math evidence and existing public reports |
| 4. Packet text events | Aggregate actual static role counts; retain per-occurrence writer order only for ordinal rules | Mixed static/ordinal winners, ByteStart/ByteEnd source ownership, missing and empty terminals, actual Packet SVG output |
| 5. Railroad outcome | Remove the typography outcome enum and field derived entirely from requested/active state | Existing per-property applicability, verified CSS, missing receipts, unsupported values and visible-text checks |
| 6. Class native paint | Write theme paint to actual namespace and interface text terminals; derive completion from the actual writer | Dagre/ELK native terminals, source precedence, solid/transparent paint, usvg glyph fill and PNG pixels |
| 7. Duplicate materialization tests | Remove repeated same-entry budget/palette/effect cases from the operation suite | Dedicated materializer boundary cases; distinct typed/JSON, invalid-color, version, resource and error-order tests remain |
| 8. Old Sequence fixture proof | Remove the test-only role-text contract, branch and helper module | Current Cyberpunk scene semantics reject missing message/note/loop roles and conflicting inline paint; generic native/font, neo/filter, PNG and public preset round-trips remain |

The numeric-input and Class-terminal regressions failed before their respective fixes and passed
afterwards. Packet's new characterization initially assumed that a bare configuration value
carried explicit source provenance; it was corrected to use Engine's actual site configuration
path and passed against the old production implementation before the storage change.

The Sequence negative controls also passed before the obsolete proof branch was removed. They
exercise the current shared scene observer rather than a replacement validator.

## Verification

Implementation and the applicable local owner checks are complete. Before deleting finding 3's chain,
43 focused renderer/export tests passed, covering the existing raw and postprocessing boundaries,
standalone invalidation and error ordering, font authorization and missing glyphs, math evidence,
cancellation and State fallback observations. The first seven changes passed 437 affected owner
tests and 15 private Sequence/preset SVG/PNG tests. These counts cover overlapping boundaries and
must not be added into a number of distinct behaviors.

All Cargo builds use the existing target directory, `--locked` and `CARGO_BUILD_JOBS=2`;
nextest runs use two test threads. Final-source results are:

| Owner | Result |
| --- | --- |
| Focused seal/postprocessing, native font, cancellation, State fallback and Class usvg/PNG checks | 44 passed |
| Core/render/export/bindings libraries and affected family/theme integration targets, with math, ELK, Cytoscape, PNG/JPEG/PDF | 5,781 passed; 2 skipped |
| Public Rust facade libraries and theme, authoring, SVG/PNG/PDF, composition and operation consumers | 180 passed; 2 skipped |
| Private acceptance/render/export/facade libraries and current preset qualification/public Cyberpunk consumers through `run_theme_acceptance.py` | 2,728 passed; 2 skipped |
| Existing strict CLI Clippy owner: `cargo clippy --locked -p merman-cli --all-targets --all-features -- -D warnings` | Passed |
| Independent public consumer, package exclusion and private/retired import boundaries through `verify_theme_acceptance_boundary.py` | Passed |
| Full SVG corpus through the existing `compare-all-svgs` owner: structure, parity and parity-root, three decimals and diagnostic browser text layout | Passed without changing normalization or budgets |
| `cargo fmt --all -- --check` and `git diff --check` | Passed |
| Relative links and trailing whitespace in this record and the owning plan | Passed |

The Class native observer now checks glyph fill opacity: usvg retains transparent paint as a
color with zero opacity. This avoids incorrectly treating an invisible flattened path as visible
black paint; solid-color checks and actual PNG checks remain independent assertions.

Earlier check results do not certify changes made after their run.

## Fresh source review

Review run `20261011-023015-8b9dc1b2` compared the same implementation base with a frozen
working-tree patch containing 33 Rust paths and the plan's three-line execution link. Its recorded
HEAD, `28e3babdd4d48ea3ee35e387d802b436808ccc6e`, identifies dispatch time, not the uncommitted
source by itself. The tracked patch was checked byte-for-byte against that frozen patch after the
final source checks. This execution document was untracked at dispatch and excluded from the
source review.

Correctness, project standards, testing, maintainability, security, performance, API contract,
reliability and adversarial reviewers completed. An independent validation pass rejected the sole
candidate after checking the fallback marker reader and processing order against the base:
the alleged single-quote/empty-FO bypass already had the same trigger path before this follow-up.
No actionable finding or unresolved verification item remains in that source review. Its rejection
does not resolve the existing observation limitation documented below.

Private qualification, full SVG and public/native publication-boundary checks were pending when
the review dispatched; all subsequently passed on the unchanged source. The review uses Codex
subagents and does not claim a separate serving-model family or a new audit of the entire PR.
The applicable R4/R6/R7/R8/R9/R10 and KTD5/KTD6/KTD9 obligations are reconciled through the
implementation/coverage table, final-source gates, disjoint accounting and explicit limits here.
The umbrella plan's older revision-bound claims are not rewritten as fresh completion receipts.

## Actual source accounting

Direct `git diff --numstat 36488c8e26f3dc1c4248549a24a897f00d3ba2d6` accounting for Rust
files gives the following disjoint owner partition. Source files include their inline unit tests;
the table is not a production-versus-test line percentage. Documentation is counted separately.

| Owner | Added | Deleted | Paths |
| --- | ---: | ---: | ---: |
| Core input and regression | 30 | 2 | 2 |
| Native export and unit tests | 45 | 145 | 2 |
| Renderer source and unit tests | 445 | 1,485 | 20 |
| Renderer integration tests | 63 | 98 | 5 |
| Private Sequence qualification | 83 | 627 | 3 |
| Public native-output regression | 83 | 0 | 1 |
| **Rust total** | **749** | **2,357** | **33** |

The net source reduction is **1,608 lines**. No dependency, lockfile, feature, profile or CI budget
changed. Most deletions retire the private text ownership chain and the old Sequence fixture proof;
new code provides direct terminal paint, bounded preparation and real input/output regressions.

## Simplification decisions

Three separate passes examined reuse, code quality and efficiency. No further production change
was applied. A suggestion to rename the reserved-token diagnostic was skipped because this
behavior-preserving pass retains existing error text. A suggestion to remove export token
observation was withdrawn after tracing the different parsing boundaries: the raw scanner can
admit `merman&#x2d;prepared-…`, while usvg decodes it to a reserved identity. The real sealed-SVG
and native-Tree regression now verifies that the export font plan still rejects these identities.
The efficiency pass found no additional actionable issue.

## Local delivery

The seven implementation and regression commits after the comparison base are:

| Commit | Outcome |
| --- | --- |
| `a619bcaf3` | ECMAScript numeric whitespace and real input regression |
| `ba82154ed` | Class namespace/interface terminal paint and family output regression |
| `5b9273eb9` | Bound class-definition preparation and ordering checks |
| `bf0bab109` | Packet static event aggregation and Railroad derived-state retirement |
| `28e3babdd` | Duplicate materialization cases and old Sequence fixture proofs retired |
| `509275592` | Unreachable prepared-text evidence and duplicate private SVG ownership retired |
| `2a89954ca` | Public Class native glyph and PNG regression |

This record and its plan link are committed separately. The final Rust source through
`2a89954ca` has the same bytes as the verified frozen review patch. No remote fetch, merge,
push, PR update or publication is performed as part of this follow-up; pre-existing local commits
remain on the feature branch.

## Retained limits

The bound class lookup removes a full definition-table scan per node. Packet static themes no
longer need a per-text event list. These are source-level work and storage changes; no matched
latency, allocation, memory or artifact-size experiment has been performed for this follow-up.
Deleting source lines is not a measurement of binary savings.

Public Mermaid inputs, structured authoring, presets, versioned recipes and shares retain their
current consumers. Public prepared-text and export report fields are not removed solely because
their current supported values are zero. Real native font and math checks remain independent of
theme admission and SVG attribute checks.

The FO marker preservation change retains the old fallback's observation boundary: valid positive
width/height, visible text, and the existing exact double-quoted attribute reader. Single-quoted,
spaced-equals, empty or zero-size cases already lost that marker in the base implementation and
remain unchanged. This is a documented existing limitation, not a claim that every possible
reserved attribute survives lossy cleanup. No earlier guard or new XML interpretation layer was
introduced, so existing resource-before-token ordering remains intact.

Python/Rust catalog metadata duplication, historical facade-name negative imports, binding-cache
cleanup and unconfirmed raster helpers remain separate candidates. This execution does not
establish that they are obsolete. Remote CI, other operating systems and the complete installed
language-consumer matrix require their own delivery evidence. The full SVG check includes the
existing deterministic parity-root checks; no separate Chromium painted-content containment or
browser-interaction run is certified. This follow-up does not change root geometry or the oracle.
