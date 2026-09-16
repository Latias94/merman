# Presentation Theme C7a/C7b Replan

Date: 2026-09-15
Updated: 2026-09-16

## Decision

Keep the current presentation-theme architecture and treat the merged `origin/main` baseline as the integration point. Do not expand the proof framework or add new compatibility abstractions until a concrete contract gap is demonstrated.

C7a is the release-facing track. C7b is the long-tail migration track and does not block the C7a contract freeze unless it exposes a shared public-model defect.

## Selected release candidate

On 2026-09-15 the maintainer selected **`v0.8.0-alpha.7`**. This supersedes the earlier
0.9 recommendation and all historical statements below that version selection is pending.
Prepare the workspace and coupled packages for alpha.7; keep new theme protocol identities
at v1. Preserve native `render/batch -e` during 0.8.x, as previously documented.
The fresh alpha.7 consumer compiles, but published alpha.6 with candidate siblings still
fails the prerelease compatibility gate on both the core error enum and retired renderer APIs.
The maintainer accepted that bounded transition on 2026-09-16. The explicit alpha.6-to-alpha.7
gate mode records the known incompatibility and still requires candidate compilation. The
release-contract decision is closed; final artifact validation, catalog rollout and C7a freeze
remain open.

## Current status and execution baseline

The fixed native candidate is `a5e3cd2d6`. Its clean macOS ARM64 checkout built CLI/LSP
archives, passed archive execution and complete preset-qualification replay, and passed all
146 optimized theme acceptance tests plus 135 archive/release Python contracts. Three
presets each have six artifact-bound, host-dependent cells; the other seven remain unqualified.
Version, preparation changelog and license checks pass. See the
[native candidate record](../knowledge/engineering/verification/2026-09-16-c7a-native-candidate-a5e3cd2d6.md).

The same CLI ran 34 literal reference diagrams through ten presets and SVG/PNG/PDF: 990/1020
outputs succeeded; all failures belong to one upstream-invalid GitGraph example. The Japanese
Flowchart parser gap was repaired. Browser computed styles exposed a separate Flowchart
arrow-color usability gap, recorded for bounded investigation before declaring presets ready.
These execution counts do not establish complete theme support or portable qualification.

The release-impact audit now compares verified published alpha.6 packages with candidate
artifacts. Render-capable npm and Python packages grew substantially despite passing current
budgets. The same-host default SVG diagnostic also signals a broad latency increase on the
17 fixtures with byte-identical output; 18 changed-output fixtures remain excluded. Independent
confirmation now reports default-render regressions for Class tiny/medium and XY Chart medium
after noise calibration and eight fresh AB/BA pairs; stage attribution is in progress. Treat
these as open impact-review items,
not accepted overhead or a reason to relax budgets. See the
[impact audit](../knowledge/engineering/verification/2026-09-16-theme-capability-impact-audit.md).

The same candidate now has installed Python wheel, Node N-API/WASM and all five Web browser
package witnesses,
including shared support/error vectors and public catalog checks. A directory-alias entrypoint
bug in the validation scripts was repaired separately; the candidate witnesses were rerun with
canonical paths and nonempty result counters. Chromium exercised authoring and real terminal
rendering in both complete Web render packages. Earlier Web/Typst budgets passed at the revisions
named in the [transport evidence](../knowledge/engineering/verification/2026-09-15-c7a-transport-catalog-follow-up.md).
The same-candidate Typst publish package now passes its shared vectors, 22 positive compiles
and nine expected failures under Typst 0.15.1. The exact C SDK profile passes 66 tests and
actual-dylib C consumer/error-vector probes. Web package-group verification and all twenty
existing Web size checks pass; all four Typst limits also pass without budget changes. The macOS Dart/Native Assets ABI, example and authoring smoke
also pass with the exact Flutter desktop profile. Other host consumers retain their gates. Linux/Windows, the Swift compiler floor and final same-source preflight retain their
own open gates. Historical checks are not a final-candidate all-host run.

C5 and representative C6a are complete. All 33 families have typed consumption paths, and
the production Legacy route inventory is empty, including Block. Post-merge retirement,
public acceptance isolation and representative native qualification have been revalidated.
Block migration is completed work; retain its regression coverage rather than scheduling a
second migration. Remaining family mechanism breadth belongs to C7b. See the
[coverage snapshot](../rendering/diagram-theme-coverage.md) and
[provider retirement record](../knowledge/engineering/verification/2026-09-14-theme-provider-retirement.md).

`origin/main` at `4c2ac7817` was merged by `3e71378be` and was verified as an ancestor during
the September 15 preparation. The historical records below describe the merge audit, fixes,
platform builds and tests at their actual source revisions. They are not evidence that every
platform was rebuilt at `72980c2fe`.

At the recorded ancestry check in `b10576aae`, tracked `origin/main` remained an ancestor.
That observation is not a claim about subsequent remote changes. Refresh the remote before a
future integration and review any new main changes as a separate source input. The accepted
alpha.6 transition does not waive candidate-owned dependency, artifact or host checks.

### Version policy for the remaining work

Use the latest published artifact for each public surface as the comparison baseline. The next
workspace release is `v0.8.0-alpha.7`; it is the only release-number increment in this candidate.
All newly introduced, unpublished theme contracts (definition, expansion, complete spec,
materialized wire, authoring diagnostics, support/catalog, qualification and execution evidence)
remain at version `1`. Do not increase a theme number for internal iterations, and do not reset a
published or independently owned ABI merely because the theme work is unpublished. Migration
batch IDs, acceptance ledgers, hash domains and historical evidence filenames retain their
identities and are not product version counters. The complete per-owner audit is maintained in
[`UNRELEASED_CONTRACT_VERSIONS.md`](../release/UNRELEASED_CONTRACT_VERSIONS.md).

| Workstream | Current status | Remaining exit condition |
| --- | --- | --- |
| Core model and production provider retirement | Implemented and revalidated | Preserve the boundary in final candidate regressions |
| Alpha.7 version projection and theme v1 consolidation | Prepared | Keep generated projections current after implementation changes |
| Published alpha.6 with candidate siblings | Known incompatibility accepted; bounded gate mode implemented | Preserve exact candidate siblings and the explicit known-impact record |
| Discovery, authoring golden and catalog/profile delivery | Implemented with source-specific evidence | Close demonstrated gaps through final owner-built consumers |
| C7a candidate and contract freeze | Open | Complete the declared same-source release matrix and archive/catalog binding |
| C7b family breadth | Separate continuing work | Evaluate mechanisms independently of the empty Legacy inventory |

## Refactoring policy

Converge unpublished theme APIs, schemas and consumers on one v1 contract. Remove superseded
unpublished shapes and adapters once their consumers migrate. Preserve published contract
history and migration identities; they are not development version counters to reset.

Keep accurate error classification, resource admission, source ownership and writer-owned
terminal evidence. A refactor must preserve these semantics or document and test a deliberate
public contract change. Do not encode an internal failure as an unrelated old error merely to
satisfy an exhaustive match in a published dependency.

Use existing production owners and the shared authoring/support vectors under
`crates/merman-theme-authoring-fixtures/fixtures/authoring-v1/`. Delete transitional helpers only after
checking their consumers and audit obligations. Keep historical retirement authority within
acceptance-only code, and consolidate duplicate facts into projections of their existing owner.
Do not introduce a general proof engine, feature-dependent error taxonomy, package rename or
new version coordinator solely to bypass this release failure.

## First task: alpha.7 compatibility boundary

Treat an intentional API migration by an alpha.7 adopter separately from an existing alpha.6
consumer failing after fresh dependency resolution. The recorded failure is the latter:
alpha.6 has non-exact sibling requirements and exhaustively matches `merman_core::Error`;
the candidate adds variants and makes the enum non-exhaustive. New exact requirements in
alpha.7 do not amend the published alpha.6 manifest.

1. Inspect the published requirements and actual resolved package graph using the existing
   fresh-consumer checker. Identify the smallest real contract boundary responsible for the
   failure; the current ASCII lane is not an all-feature compatibility proof.
2. Evaluate a compile-compatible implementation only if it preserves the new diagnostic
   semantics and has a simple, durable API. Verify any proposed fix with the real previous
   facade, the candidate consumer, and focused diagnostic/transport regressions.
3. If that requires misleading error classifications or a compatibility subsystem, document
   the rejected design and compare an isolated Cargo version boundary with explicitly accepting
   the old-consumer resolution break. Keep alpha.7 selected while presenting the decision.
   Neither changing the release line nor accepting that break is implicit in refactoring consent.
4. Record the decision, affected consumers, migration guidance and actual check results in the
   existing release-line verification document. A policy exception requires explicit maintainer
   approval and corresponding release-instruction updates; never turn a failed check into a
   success or silently skip the lane.

Exit: the candidate and required previous-consumer checks pass with preserved error semantics,
or the maintainer explicitly approves a documented alternative release contract. An unresolved
decision keeps C7a open. Complete the bounded assessment and present the decision rather than
repeatedly running the same failing build.

### Assessment outcome

The [bounded assessment](../knowledge/engineering/verification/2026-09-15-theme-release-line-preparation.md#bounded-compatibility-assessment-at-be2d12d20)
compiled the alpha.7 ASCII and SVG consumers and reproduced failures in both old-facade recipes.
SVG additionally depends on the retired presentation/rendering API, so changing only the error
enum cannot close this boundary. The existing checker now includes both features (`2ed72b4cf`),
and correctly rejects the old/new combination. Its 45 focused unit/workflow tests pass.

On 2026-09-16 the maintainer accepted alpha.7 with an explicit, bounded acceptance of the
old-consumer resolution break. Keep the current product model, exact alpha.7 sibling requirements
and accurate error semantics. Keep the checker strict by default. Its explicit
`--accept-alpha6-transition` mode implements only this approved version pair and still requires
candidate compilation; both release workflows opt into it for that pair. Do not restore the
retired presentation API or use the first-prerelease shortcut.
The failed alpha.6 lane remains known-impact evidence; it is not relabeled as a pass. Rebuild and
rerun the candidate-owned matrix after this policy decision.

The transition gate implementation was validated locally on 2026-09-16: the real lockfile-free
alpha.7 ASCII/SVG consumer compiled, and the previous lane was reported as known incompatible
and NOT VERIFIED. The 49 focused checker/workflow tests include candidate failure, invalid
exception version pairs, strict default behavior, and execution of both workflow command
branches. This closes the policy-to-gate mismatch only, not the remaining C7a artifact matrix.

## Execution order and goal completion

The active goal is C7a candidate preparation for `v0.8.0-alpha.7`, without tagging or publishing.
Keep this goal active while any required gate below remains unresolved; progress counts and
local witnesses do not establish completion.

1. Refresh remaining installed consumers and shared transport vectors with their owner commands.
   Preserve the completed `a5e3cd2d6` native archive/qualification record; rebuild affected artifacts
   if implementation changes. Record missing hosts explicitly.
2. Apply the approved alpha.6 transition contract through the bounded checker mode. The decision
   is complete; preserve its known-impact record.
3. Pin a clean candidate after the decision and any resulting changes. Run its declared
   host/compiler/profile matrix, installed consumers and shared transport vectors. Preserve
   unknown-status rejection and bind catalog qualification to the artifacts actually tested.
4. Close C7a only when required gates pass under the approved contract and the release handoff
   identifies the source, artifacts, qualification scope and any explicitly deferred C7b work.

No new proof framework, unmeasured cache rewrite or historical-ledger deletion is required by
this sequence. Keep measured size growth separate from correctness; amend budgets only when
an actual candidate measurement and attribution justify the change.

## C7a: release-facing closure

1. **Final candidate regression gate**
   - Run formatting, CI workflow contract tests, theme acceptance, support discovery, retirement tests, and the representative native export checks from the same commit.
   - Record the exact source SHA and artifact profile in existing owner receipts.
   - Keep prior passes at their original source identities. Pin one clean candidate after the
     compatibility decision and cleanup; rerun affected gates when candidate inputs change.

2. **Public discovery and catalog promotion**
   - Complete first-party discovery for Rust, Web, Node, Typst, UniFFI, and Native C ABI.
   - Bind each qualified catalog cell to the artifact profile actually built and tested.
   - Unknown `profile_id` and `admission_status` values must never establish Portable or Verified
     admission. Transports may preserve open metadata where their public contract permits it.

3. **Cross-transport authoring contract**
   - Re-run existing shared invalid-input golden cases through Web, Node, Typst, UniFFI, and Native C ABI; add cases only for uncovered contract behavior.
   - Compare error envelope, path, details, and admission status; do not create a second validation framework.

4. **Budget decision**
   - Keep the current budget unless a measured artifact exceeds it on the merged source.
   - If a budget is exceeded, amend it with a recorded size breakdown, ownership of the increase, and a proof that the resulting package remains reasonable.
   - Do not embed Inter or another product font merely to satisfy visual fixtures. Use host or consumer-provided fonts unless a product requirement establishes a distributable font obligation.

5. **Freeze criteria**
   - The compatibility boundary has the passing evidence or explicitly approved release contract
     described above. Version selection alone does not satisfy this condition.
   - Each declared C7a candidate scenario has qualification cells bound to its actual artifact. Unqualified presets and targets keep empty cells; host-dependent evidence never implies Portable admission.
   - Public support discovery agrees across transports.
   - Release preflight and package-content checks pass for the declared candidate from a clean
     checkout. Use the existing owner workflows and artifact profiles; enumerate required hosts
     and compiler floors before execution, including Windows and the Apple Swift 5.9 floor.
   - Package installation, shared authoring/support vectors, qualification cells and archive
     replay refer to the tested artifact. Unsupported and unqualified scopes remain explicit.
   - Missing host execution remains an open gate, not a local test substitute.
   - No open P1 correctness or CI coverage gap remains.

## C7b: long-tail migration

1. Keep the completed Block route/provider retirement closed. Include its independent authority and production cfg boundary in final regressions; reopen implementation only for a reproduced regression.
2. Preserve historical probes and migration records in acceptance-only code; remove a helper only after confirming it has no remaining consumer or audit obligation.
3. Keep historical Class cutover failures separate from completed Class retirement, as the current coverage snapshot does.
4. Inventory remaining family mechanism breadth independently of the zero Legacy route count. Do not create new family bridges to improve route counts.

## Latest bounded cleanup

The renderer helper cleanup completed on `a6b5d75e2` scopes four helpers to test builds where
production consumers do not exist. Focused nextest coverage passed 72/72, production
`cargo check` passed, and the full SVG structure comparison passed with the existing documented
browser-text-layout residuals. This is a maintenance boundary cleanup, not evidence that C7a
compatibility or the final artifact matrix is closed.

Do not remove acceptance metadata accessors, historical retirement authority, or catalog
qualification helpers solely because a production feature build does not call them; those
paths remain owned by acceptance and release verification.

## Deferred unless measured

- XY Chart terminal-cache copies and receipt escaping.
- Block marker/edge XML receipt cost.
- SystemOnly font database cloning.

Each optimization requires a workload, a before/after measurement, and a semantics-preserving
change before entering the release critical path. A broad proof-engine or family-trait rewrite
is outside this closure plan; it requires a separately demonstrated design need.

## Working sequence and completion

1. Resolve the alpha.7 compatibility boundary through the bounded assessment above.
2. Complete demonstrated first-party contract gaps and remove verified obsolete transitional
   code. Refresh version, legal and generated projections through their existing owners.
3. Pin the clean candidate and execute the required owner preflight/profile matrix. Exercise
   public discovery and shared authoring vectors through real installed consumers; bind catalog
   cells and archive replay to the actual artifacts. Keep Block retirement in this regression gate.
4. Reconcile outcomes in the existing verification records and close C7a only when all declared
   freeze criteria pass. Record the candidate SHA, version, profiles, qualifications, host limits
   and migration contract, then prepare the concrete release handoff.
5. Continue C7b family breadth separately. Revisit deferred optimization only when measurements
   justify it; neither task is an implicit requirement to freeze the bounded C7a candidate.

The execution goal is a reviewable, validated alpha.7 C7a release candidate, with explicit scope
and no unresolved required gate. A plan update, version bump, successful subset of tests, or
compatibility decision by itself does not complete that goal. Publication, tag creation and
publishing workflow dispatch remain outside preparation and require explicit shipping authority.
Request a maintainer decision when the required release contract changes; report unavailable
host/toolchain evidence precisely rather than weakening the gate.

## Historical evidence boundary

The sections below retain their original source identities and describe earlier work. Statements
that version selection, Block revalidation or a particular follow-up was pending are historical;
the current status and working sequence above govern execution. Do not relabel old artifact
receipts as evidence for the selected alpha.7 candidate.

## Historical integration verification before version consolidation

On the working tree based on `15e8ecbd5`, the following checks ran after restoring
preflight qualification generation, replay, and catalog-companion upload:

- Workflow contracts: 87 tests passed (`test_ci_plan`, `test_release_workflow_security`,
  `test_ci_workflow_android_emulator`, `test_fuzz_config`); actionlint passed for
  `release-preflight.yml`. The added shell test uses recording verifiers to exercise
  x86_64 qualification, ARM64 smoke, and failure propagation; it does not build archives.
- Internal acceptance manifest digest: one test passed (121 excluded by the filter).
- Release retirement: 10 tests passed across all four integration targets, with no skips.

The Rust commands used `scripts/run_theme_acceptance.py` to enable the internal cfg.
Direct Cargo invocations without that cfg ran zero tests and supply no acceptance evidence.

The UniFFI mismatch found at `acffcfe1b` is repaired by `a1a8e407d` in the API 9 integration tranche:
ASCII requested/effective layout metadata and the theme-authoring error envelope now coexist,
and Apple/Python projections use the new probe. Old API 7/8 probes stay absent.
The restored consumers exercise both Auto layout branches and theme errors. Local checks:
60/60 Rust binding tests, 7/7 generator tests, 29/29 Python contracts, Swift and Python native
smokes, native ABI and shared binding contracts, and Rust formatting. Swift 6.3.2 on macOS
ARM64 compiled and linked the checked-in generated source against the `apple-uniffi-native`
host library. Python used a freshly generated package from that same library; this is not
an installed `python-uniffi-native` wheel or a full XCFramework/Swift 5.9 matrix claim.
The repository platform verifier also passed: Android target clippy, Dart ABI checks and
analysis, and rebuilt Flutter Native Assets authoring/support/resource smokes. The actual
UniFFI native library exports API 9 and rejects API 7/8 symbol lookup. These checks do not
claim Android device execution or all published platform packages.

Next, review post-merge lockfile churn and regenerate legal projections, then finish auditing
both merge parents and rebuild the final package profiles. Linux archive execution, the full
platform matrix, and C7a contract freeze remain open.

## Unreleased version consolidation

Registry and GitHub release checks on 2026-09-15 identify `v0.8.0-alpha.6` (published
2026-09-02) as the latest workspace prerelease; its UniFFI API is 6. The API 9 integration
record above describes an unpublished development build. The next public UniFFI contract
is consolidated to API 7, retaining both merged record changes and rejecting the published
API 6 probe. Native Apple-profile rebuild and regenerated Swift/Python consumers passed:
60 Rust binding tests, 29 Python tests, and real Swift/Python render and authoring smokes.
This remains local macOS ARM64 evidence, not the complete release artifact matrix.

Audit the remaining version axes against that published baseline before the C7a freeze.
New theme formats start at 1; remove superseded unpublished support-query shapes rather
than carrying a compatibility branch. Keep published formats and historical migration
batch identities distinct from this first-release numbering.

The [version audit](../release/UNRELEASED_CONTRACT_VERSIONS.md) records each public and internal
axis against actual published artifacts. Before C7a freeze, use the consolidated first-release
theme v1 contracts, UniFFI API 7 and Typst ABI 3. Preserve the identity of earlier migration and
verification records; their historical numbers are not product versions. Regenerate candidate
records and final artifact profiles from the resulting source rather than editing old receipts.

Version consolidation has passed the scoped checks listed in that audit, including the real
published Typst ABI probe, shared transport support vectors, native preset qualification and
rebuilt local Python/Swift consumers. C7a remains unfrozen. Next resume the post-merge lockfile
and legal-projection audit, then rebuild the final artifact/profile candidate matrix.

## Post-merge dependency and embedding repair

The lockfile audit uses theme parent `239fbed0b`, main parent `4c2ac7817`, and merge base
`b381b842f`. Main adds `merman-doc`, `proc-macro-crate 3.5.0`,
`pulldown-cmark-escape 0.11.0`, and `toml_edit 0.25.13+spec-1.1.0`. Re-resolving those
additions against the theme parent's lock succeeds offline without broad upgrades.
Keep two targeted updates: `rustls 0.23.45` addresses RUSTSEC-2026-0285 dated 2026-09-14,
and `chacha20 0.10.2` replaces the yanked 0.10.1 lock entry. Cargo-deny passes advisories,
bans, licenses and sources with that graph. Generated legal reports and projections follow
the existing owner scripts: 13 source reports and 382 release material projections pass freshness
checks, the third-party contract passes, and the contract unit tests pass (18 tests). The old
platform reports omitted theme dependencies such as font decoding, shaping and hashing; the
regenerated reports include them without introducing another license kind.

The shared-source audit also found main's embedding contract disconnected from the theme
pipeline. Rustdoc calls `with_browser_inline_contract` and `with_rebased_ids`, which the
merged pipeline omitted. Restore those entry points and Mindmap's renderer-owned duplicate-ID
normalization using `DiagramFamilyId`; preserve prepared-math occurrence checks before
publication. Restore Error's SVG style namespace and scope TreeView/ZenUML styles without
replacing their typed writers. Event Modeling and Ishikawa already have scoped typed writers.
The combined pipeline, prepared-math, shared document and Rustdoc checks pass all 265 tests,
including both Rustdoc end-to-end consumers. The full structure comparison passes all 35
family dispatches with the existing diagnostic browser-text-layout receipts. All 34
representative artifact dependency-closure checks and the curated feature matrix pass.
The 16 font asset tests pass, including duplicate WOFF2 table rejection and canonicalization
with the restored decoder version. The native-sdk Apple host library rebuild passes;
UniFFI 0.32.0 regenerates the checked-in Swift/Python source without a diff. Python
contracts pass 29 tests, its real consumer passes all 22 support vectors and three
budgeted authoring operations through both consumer modes, and the rebuilt Swift
module passes the Apple consumer smoke. This is macOS ARM64 host evidence, not a
complete platform rebuild or a C7a freeze.

## Post-merge artifact execution

The [artifact verification record](../knowledge/engineering/verification/2026-09-15-c7a-post-merge-artifacts.md)
tracks the clean candidate through `835d1a8c4`. The actual Node native tarballs were installed
and exercised; five Web package/WASM smokes passed; Typst 0.15.0 compiled 22 documents and
rejected nine negative fixtures. All 24 Web/Typst size checks passed within the existing
budgets. The final candidate's CLI archive matched 18 qualified outputs and passed fresh
record replay, retaining only the declared host-dependent cells.

This execution found and fixed the ordinary Node CI npm mismatch, old qualification/Web/Typst
version expectations, unmigrated Typst authoring examples, and the Typst smoke's assumption
that Cargo output lives inside the checkout. The shared-target `xtask` was explicitly rebuilt
for the candidate before accepting independent-checkout evidence. No production theme model
or qualification framework was added.

The follow-up runs below complete the workspace, retirement and installed-consumer work
that remained after this initial artifact tranche. Keep each artifact's actual source identity;
earlier macOS observations do not substitute for Linux/Windows owner execution.

At clean `747f359b2`, the complete default workspace regression passed 11102/11102 tests
(12 skips), and the six explicit Release retirement/qualification integration targets
passed 13/13 with no skips. The Block coverage and Rustdoc resource fixture corrections
are recorded in the artifact verification document. This closes that post-merge local
regression tranche. The same source also built and installed the macOS ARM64
`python-uniffi-native` wheel; its authoring/catalog/resource consumer passed. The independent
Node-WASM tarball was also built, packed, installed and exercised through both consumer modes.
At clean `b7a526353`, the [browser/platform owner record](../knowledge/engineering/verification/2026-09-15-c7a-browser-platform-verification.md)
adds 110 Chromium desktop, 49 Firefox/WebKit and 12 Chromium mobile passes. Public Cargo
acceptance isolation passed, and the same source passed local Android transport clippy plus
rebuilt Flutter authoring/resource consumers. The [Android artifact/device record](../knowledge/engineering/verification/2026-09-15-c7a-android-artifact-verification.md)
adds both AAR architectures and Maven staging at `5e76f9835`, 46 JVM passes, and nine API 36
ARM64 device passes at `6517f6b1f`. This does not replace the CI API 29 x86_64 lane.
The [Apple artifact record](../knowledge/engineering/verification/2026-09-15-c7a-apple-artifact-verification.md)
adds the complete five-target XCFramework, unchanged generated bindings, Swift package and macOS
ARM64 consumer passes, plus ZIP/legal/checksum validation. The older Swift compiler floor remains
an independent CI obligation. Next finish Linux/Windows and the remaining declared platform lanes,
reconcile the final archive/profile records, then close
the named candidate, public rollout and compatibility freeze. Browser portability qualification
and unqualified preset breadth remain outside these application-smoke claims.

At clean `6517f6b1f`, the [Linux archive record](../knowledge/engineering/verification/2026-09-15-c7a-linux-archive-verification.md)
adds actual x86_64 CLI/LSP archives, runtime verification, all 18 archive-bound preset cells
and complete record/catalog replay. The Linux execution used a macOS-hosted VM and Rosetta;
it is not a native CI runner claim. The artifact-local companion retains HostDependent
admission and leaves seven presets unqualified. The same record now includes a repaired,
installed Linux Python wheel with shared authoring/catalog/support/resource smokes passing;
its default SVG SDK profile gains no CLI PNG qualification. Commit `a6b9f9ae5` closes the
preflight smoke-before-repair gap, and its clean checkout passed 67 release/qualification
contract tests. Continue Linux Node and the remaining native host/compiler lanes, then bind
the final same-source candidate/rollout/freeze. `/tmp/merman-c7a-a6b9f9ae5` is the clean
integration candidate for that work, not a declared frozen contract.

## Post-merge audit and Linux Node follow-up

The audit against `6be5b63c7..a6b9f9ae5` was rechecked at `b7246bbe4`. Its five actionable findings
were repaired: host npm preflight setup (`19ada22e1`), Markdown prefix allocation and repeated
negative include scanning (`92bcdf026`), and conditional rustdoc order/facade helper resolution
(`ff0cd48e6`). The [repair verification](../knowledge/engineering/verification/2026-09-15-post-merge-rustdoc-audit-fixes.md)
records 51 doc/macro tests, 44 CLI tests, 88 workflow contracts, the npm 11/12 reproduction,
scoped Clippy, and the same-toolchain structural resource comparison. This closes those findings,
not the full C7a candidate gate. The headless plan and shared Markdown ADR now use current
contracts and valid, unique ADR identities.

The [Linux Node installed-package record](../knowledge/engineering/verification/2026-09-15-c7a-linux-node-verification.md)
adds successful GNU/glibc-2.31 and musl consumers built from clean `a6b9f9ae5` clones. Both ran the
shared authoring vectors after installing the actual loader and native tarballs. The record names
source and archive hashes; newer README, workflow, and doc-runtime changes still require fresh
owner-built artifacts. Do not reuse these receipts under a newer source label.

The [version audit](../release/UNRELEASED_CONTRACT_VERSIONS.md) remains the numbering policy: new,
unpublished theme schemas, recipes, catalogs, support claims and qualification contracts start at
1. Existing public interfaces compare with their last published artifact and receive one candidate
compatibility bump. Migration batches, historical acceptance records, hash domains and ADR IDs
remain independent identities. Alpha.6 history is unchanged; new source behavior stays Unreleased.

Next, assemble the final candidate from one selected clean source revision and execute its owner
preflight/profile matrix, including refreshed package contents and receipts. The formal release
version/date, catalog/artifact rollout and C7a contract freeze remain open. Avoid adding another
proof framework or restoring retired production providers while closing these gates.

## Candidate regression and release acceptance follow-up

A clean checkout at `8232bfbace80e99faff341aefb4352327d705404` passed the full default
workspace nextest run: **11,106/11,106 tests, 12 skips**. One ASCII Mindmap test was reported
as leaky by nextest despite passing; its isolated recheck at `57ec788c0` passed without that
marker. This is not evidence of a production memory leak, and no timeout or test policy was
relaxed. The clean Node contract run at `8232bfbac` passed **108/108**, including actual npm
pack checks with npm 12.0.2. Its host Node was 26.6.0; pinned Node 24.21.0 remains the owner
workflow environment.

The combined release package tests found one stale npm-11 response in the Web pack-path test.
Commit `57ec788c0a4b49227cf8226d4e0df43686ac6a1e` corrects that fixture to the npm-12 named
record without changing the production decoder or path assertion. This is the only change
from `8232bfbac`; no Rust source, Cargo input or generated contract changed. The clean checkout
was then advanced to `57ec788c0`, where all **102** Node/Web package-group, release workflow,
qualification and archive-bundle contract tests passed.

At clean `57ec788c0`, Release acceptance with the internal cfg and explicit
`png,jpeg,pdf,layout-cytoscape` features passed **16/16 tests across eight integration targets,
zero skips**: C6 runtime, exact preset qualification, KTD17 cutover, KTD23 retirement, Block
and Class projections, Flowchart markers, and native exports. The independent public Cargo
consumer also compiled the production APIs with every advertised producer feature enabled,
while all four private receipt imports and six retired theme imports failed as required.
This revalidates the production Block/provider retirement and public acceptance boundary.

Logs are `/tmp/merman-c7a-8232bfbac-workspace.log`,
`/tmp/merman-c7a-8232bfbac-node-contracts-canonical.log`, and
`/tmp/merman-c7a-57ec788c0-{release-contracts,release-acceptance,public-boundary,ascii-leak-recheck}.log`.
The checkout remains at `/tmp/merman-c7a-8232bfbac`; its actual HEAD is `57ec788c0` after the
fixture-only follow-up, so the directory name must not be treated as source authority.

The Linux container is also clean at `57ec788c0`. Its non-publishing cargo-dist plan passed,
and the existing sequential archive-build/runtime/qualification/replay owner is running with
logs at `/tmp/merman-c7a-57ec788c0-linux-archives.log`. Until that command completes, the older
archive records remain the only completed Linux artifact evidence. The workspace version is
still the development `0.8.0-alpha.6` and changelogs remain Unreleased; selecting the actual next
release version/date and passing its immutable preflight are still required before C7a freeze.

The same-source Linux archive refresh at `57ec788c0` completed after this follow-up: CLI/LSP
runtime checks, 18 qualification comparisons, and complete qualification/catalog replay all passed.
The catalog companion binds the exact archive and executable bytes and retains only the three
native alpha preset entries with six HostDependent cells each. It does not promote shared SDK
catalog cells. The formal preflight remains intentionally pending while the changelog is
`Unreleased` and no next workspace version/date has been selected.

## Release-line decision discovered during candidate preparation

`origin/main` was fetched again and remains `4c2ac7817`, already an ancestor of this branch.
The [release-line experiment](../knowledge/engineering/verification/2026-09-15-theme-release-line-preparation.md)
found and repaired stale theme dependency edges in `fuzz/Cargo.lock` (`753cdc1c2`). It then
successfully prepared all 29 version projections for both alpha.7 and a new 0.9 alpha.1 line.
Alpha.7's fresh candidate compiled, but the real published alpha.6 facade failed against its
candidate siblings because of the evolved core error enum. The new `0.9.0-alpha.1` candidate
compiled and remains outside that published 0.8 compatibility line. That experiment recommended
a new line; the subsequent maintainer decision selects alpha.7 instead, as recorded above.

The version choice is a prerequisite to immutable release preflight, not the only remaining
C7a requirement. Final cross-host/profile execution, catalog rollout and explicit contract freeze
still need the evidence described above. Preserve the current error classification and retired
API boundaries; do not undo them just to make an old registry facade accept a new sibling.


## Linux ARM64 archive follow-up

At clean `7e9c613fa`, the [ARM64 archive record](../knowledge/engineering/verification/2026-09-15-c7a-linux-arm64-archive-verification.md)
adds actual native CLI/LSP archive builds and successful execution through both existing archive
verifiers. The task ran under macOS-hosted ARM64 virtualization; it is not a hosted CI result.
Both archive digests were rechecked after copying to the host. No preset cells were promoted:
this lane validates archive contents and runtime behavior, while the named preset qualification
profile remains Linux x86_64-specific. The next release line is still unselected, and the final
same-source platform matrix, public rollout and C7a contract freeze remain open.


## Transport catalog contract follow-up

At clean `3fe3b1b97`, the [transport catalog follow-up](../knowledge/engineering/verification/2026-09-15-c7a-transport-catalog-follow-up.md)
passed the focused Python qualification, Node/Web catalog, and Flutter ABI 3 contract checks.
Unknown qualification identifiers remain fail-closed at the CLI projection boundary, while Node/Web
keep their documented open metadata behavior. This closes no packaged-artifact or release-preflight
gate; the final same-source owner matrix and contract freeze remain open.

## Theme acceptance rerun at `5980cc9ec`

At `5980cc9ec`, the acceptance targets passed again: retirement and representative
qualification 10/10, KTD23 retirement 4/4, route cutover 1/1, and renderer support discovery 48/48,
all with zero skips. The route authorization test took 334.152 seconds; one passing C6 test received
a nextest `LEAK` marker; an isolated acceptance-configured rerun with the original feature set
passed without the marker. The recheck is recorded in the transport verification note. Log:
`/tmp/merman-c7a-current-retirement.log`. This revalidates current Block/provider retirement and
support discovery but does not close release-version, final platform/profile rollout, or C7a freeze.
