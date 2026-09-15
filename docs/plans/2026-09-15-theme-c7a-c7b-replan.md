# Presentation Theme C7a/C7b Replan

Date: 2026-09-15

## Decision

Keep the current presentation-theme architecture and treat the merged `origin/main` baseline as the integration point. Do not expand the proof framework or add new compatibility abstractions until a concrete contract gap is demonstrated.

C7a is the release-facing track. C7b is the long-tail migration track and does not block the C7a contract freeze unless it exposes a shared public-model defect.

## Verified starting point

The merge commit is `3e71378be` (remote main `4c2ac7817`); post-merge follow-ups
through `15e8ecbd5` are not full-platform release evidence. Before freezing, audit the
conflict resolutions against both parents, including binding generations, public docs,
SVG pipeline behavior, dependency changes, and generated license projections.

Block generic Text cutover and production provider retirement already landed before the
merge. KTD17 v89 records 506 routes and 814 route-profile witnesses; the executable Legacy
inventory is empty. The bridge, historical probes, and tombstones are restricted to unit
tests or internal acceptance cfg. The [coverage snapshot](../rendering/diagram-theme-coverage.md)
and [provider retirement verification](../knowledge/engineering/verification/2026-09-14-theme-provider-retirement.md)
distinguish this completed migration from remaining family breadth. Revalidate the
retirement after integration; do not implement it again.

Shared authoring errors, support discovery, and qualification-cell vectors already exist
under `crates/merman-theme-authoring-fixtures/fixtures/authoring-v1/`. Installed consumer
records cover earlier source revisions. Check their current transport consumers and
fill demonstrated gaps rather than creating another golden format.

## C7a: release-facing closure

1. **Post-merge regression gate**
   - Run formatting, CI workflow contract tests, theme acceptance, support discovery, retirement tests, and the representative native export checks from the same commit.
   - Record the exact source SHA and artifact profile in the acceptance receipt.

2. **Public discovery and catalog promotion**
   - Complete first-party discovery for Rust, Web, Node, Typst, UniFFI, and Native C ABI.
   - Bind each qualified catalog cell to the artifact profile actually built and tested.
   - Keep unknown `profile_id` and `admission_status` values fail-closed.

3. **Cross-transport authoring contract**
   - Re-run existing shared invalid-input golden cases through Web, Node, Typst, UniFFI, and Native C ABI; add cases only for uncovered contract behavior.
   - Compare error envelope, path, details, and admission status; do not create a second validation framework.

4. **Budget decision**
   - Keep the current budget unless a measured artifact exceeds it on the merged source.
   - If a budget is exceeded, amend it with a recorded size breakdown, ownership of the increase, and a proof that the resulting package remains reasonable.
   - Do not embed Inter or another product font merely to satisfy visual fixtures. Use host or consumer-provided fonts unless a product requirement establishes a distributable font obligation.

5. **Freeze criteria**
   - Each declared C7a candidate scenario has qualification cells bound to its actual artifact. Unqualified presets and targets keep empty cells; host-dependent evidence never implies Portable admission.
   - Public support discovery agrees across transports.
   - Release preflight and package-content checks pass from a clean checkout.
   - No open P1 correctness or CI coverage gap remains.

## C7b: long-tail migration

1. Revalidate the completed Block route/provider retirement against the merged code, including the independent retirement authority and production cfg boundary.
2. Preserve historical probes and migration records in acceptance-only code; remove a helper only after confirming it has no remaining consumer or audit obligation.
3. Keep historical Class cutover failures separate from completed Class retirement, as the current coverage snapshot does.
4. Inventory remaining family mechanism breadth independently of the zero Legacy route count. Do not create new family bridges to improve route counts.

## Deferred unless measured

- XY Chart terminal-cache copies and receipt escaping.
- Block marker/edge XML receipt cost.
- SystemOnly font database cloning.
- Any broad proof-engine or family-trait rewrite.

These are optimization candidates. Each requires a workload, a before/after measurement, and a semantics-preserving change before entering the release critical path.

## Working sequence

1. Finish post-merge regression evidence.
2. Close C7a discovery, catalog, cross-transport golden, and profile matrix gaps.
3. Freeze the C7a contract and publish the evidence record.
4. Close the Block retirement audit and continue the remaining C7b family breadth work.
5. Revisit deferred performance work only when a benchmark shows material impact.

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
compiled and remains outside that published 0.8 compatibility line. Recommend the new line;
keep the main worktree's package version unchanged until the maintainer selects it.

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
a nextest `LEAK` marker whose cause was not investigated in this run. Log:
`/tmp/merman-c7a-current-retirement.log`. This revalidates current Block/provider retirement and
support discovery but does not close release-version, final platform/profile rollout, or C7a freeze.
