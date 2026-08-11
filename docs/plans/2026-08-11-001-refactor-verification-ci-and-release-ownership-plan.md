---
title: Verification, CI, and Release Ownership Cleanup - Plan
type: refactor
date: 2026-08-11
deepened: 2026-08-11
artifact_contract: ce-unified-plan/v1
artifact_readiness: implementation-ready
product_contract_source: ce-plan-bootstrap
execution: code
---

# Verification, CI, and Release Ownership Cleanup - Plan

## Goal Capsule

- **Objective:** Refactor Merman's verification, CI, release tooling, and repository documentation so that each fact has one natural owner, standing gates prove user-observable behavior, and expensive evidence runs only where its risk justifies the cost.
- **Authority:** `capabilities/feature-surface-v1.json` owns capability semantics, `capabilities/artifact-profiles-v1.json` owns exact Cargo artifact recipes, Cargo metadata owns Rust dependency graphs, package manifests and lockfiles own package dependency state, the pinned Mermaid source owns behavior, final package artifacts and registry state own release truth, and each public interface keeps its existing owner-specific executable contract.
- **Execution profile:** Fearless breaking refactor of internal `xtask` commands, Python/Node scripts, workflow topology, generated evidence, and historical documentation. Delete obsolete commands, partial language interpreters, duplicate projections, and stale records rather than preserving compatibility shims. Public crates, CLI behavior, FFI ABI, npm package shape, SVG semantics, Mermaid parity, and released-channel behavior remain protected by executable evidence.
- **Stop conditions:** Do not replace an old gate before its behavior-level successor exists; weaken descendant SVG structure/semantic parity, root validity/cropping policy, or exact deterministic root fixtures; skip real platform smoke to make CI green; create another repository-wide transport or release-state catalog; implement another partial parser for Cargo, GitHub Actions, JavaScript, DSSE, SLSA, or package-manager output; publish packages; mutate remote branch protection or environment settings without explicit maintainer authorization; or delete `.worktrees/presentation-theme-model`.
- **Tail ownership:** Implement every active unit on `refactor/cleanup-review`, make focused Conventional Commits as coherent units settle, and open one no-badge draft pull request when the replacement PR gate is ready for the required old/new dual-run. Continue pushing focused units to that same PR, mark it ready only after final review, and babysit it to green. The PR must not contain a Compound Engineering badge.

---

## Product Contract

### Summary

Merman currently has strong behavior, package, ABI, and release evidence, but that evidence is mixed with prose contracts, source-shape assertions, one-off investigation code, and duplicated CI orchestration. The result is paradoxical: ordinary edits can fail thousands of lines of bespoke validation while independent lockfiles, Windows execution paths, or remote merge governance remain uncovered.

This refactor makes the verification architecture follow product ownership. A contributor should receive one stable PR result whose owner jobs either ran or deliberately skipped in the same workflow. A Mermaid upgrade should run a deliberate admission workflow, while ordinary verification checks only the selected immutable reference. A release operator should validate the exact final artifact, with crates.io explicitly modeled as the one channel that may require deterministic re-packaging. Documentation should explain current policy and history without becoming an executable database.

### Problem Frame

The audit in `/Users/frankorz/Downloads/merman-cleanup-review.md` identified 24 concerns. Repository research confirms that 15 remain materially true, seven are partially true or narrower than stated, one channel-lifecycle concern is already resolved, and one branch-protection concern requires external GitHub configuration. The highest-risk current defects are not abstract architecture issues:

- `platforms/node/package-lock.json` and `tools/vscode-extension/package-lock.json` still resolve vulnerable `js-yaml 4.3.0` tooling.
- Cargo and npm audit workflows omit independently locked package trees.
- Windows drive paths can still be misclassified as URL schemes in the Node WASM candidate, and several npm subprocess callers bypass the shared cross-platform adapter.
- platform recipe tests invoke Unix shell scripts without declaring a POSIX host requirement.
- `main` has no branch protection or ruleset, so repository-named gates are not enforced by GitHub.

The largest maintainability costs are similarly concrete: a 381-line GitHub Actions subset parser, roughly 2,600 lines of release workflow source-shape tests, a 1,351-line JavaScript subset evaluator, roughly 9,000 lines of unowned debug commands, a 3,968-line current-reference verifier that also governs future ZenUML candidates, a 1.1 MB browser residual ledger, and hundreds of completed workstream documents in the live documentation tree.

### Actors

- A1. **Contributor:** needs fast, stable PR feedback that names the failing product owner and does not break on semantically equivalent YAML or prose edits.
- A2. **Maintainer:** needs a small set of comprehensible authorities, low-cost routine maintenance, and the freedom to refactor internal tools without preserving investigation-era APIs.
- A3. **Release operator:** needs source, artifact, digest, provenance, registry reconciliation, and recovery boundaries that match each registry's actual lifecycle.
- A4. **Security reviewer:** needs every committed lockfile audited and every privileged workflow protected by least privilege, immutable action identity, and maintained update automation.
- A5. **Mermaid parity maintainer:** needs selected-reference verification to stay deterministic while candidate admission remains explicit, source-backed, and unable to block unrelated PRs.
- A6. **Platform and package user:** needs Windows, macOS, Linux, browser, Node, Typst, native binding, and package behavior to remain protected by real build/install/load/smoke evidence.

### Requirements

#### Evidence ownership

- R1. Ordinary Markdown wording, document pairing, backtick paths, historical plan IDs, private function names, job names, step names, and shell command substrings must not be standing release evidence. Executable examples and structured machine inputs remain valid evidence at their natural owners.
- R2. Each machine fact must have one owner. Capability and artifact-profile authorities remain the only repository-wide cross-surface catalogs; interface schemas, package manifests, lockfiles, release receipts, family registries, and fixture manifests remain owner-local. No replacement central transport, channel-state, or evidence-status database may be introduced.
- R3. Rust dependency and feature-closure checks must consume `cargo metadata --format-version 1` with the exact target and feature recipe. Package dependency checks must consume committed manifests and lockfiles. Workflow syntax and common workflow security defects must be delegated to maintained GitHub Actions tooling.
- R4. Crates.io publish order must be derived from the workspace dependency graph, with deterministic ordering only for independent nodes. Python, workflow, and prose copies of the order must be deleted.

#### CI and security

- R5. One pull-request orchestration workflow must always start, run a deterministic CI planner over base/head Git identities, execute or explicitly skip owner jobs within the same run, and finish with one stable `pr-gate` result. The planner consumes NUL-delimited Git name-status data and emits validated owner booleans plus reasons as JSON. Missing bases, diff failures, malformed planner output, unknown/shared paths, classifier or workflow changes, and both sides of renames select all potentially affected owners; only a valid empty diff may run the aggregate alone. The gate fails closed when any selected result is missing. Cross-workflow status must not be inferred by tests or `needs` relationships.
- R6. PR cost must match risk: Linux owns the full workspace and primary parity lane; macOS and Windows own focused host-sensitive suites on relevant PRs and full suites on the explicit lifecycle matrix below; PR fuzz builds harnesses and runs fixed regressions while randomized loops move to scheduled/manual execution; shared renderer changes still trigger complete parity. Record pre-change and post-change owner selection, critical-path duration, and runner-minute evidence for representative docs-only, Node-only, and renderer changes before claiming a cost improvement.
- R7. One cross-platform audit planner must enumerate every version-controlled `Cargo.lock` and `package-lock.json`, apply only narrow documented exclusions, and emit the exact JSON matrices consumed by the audit jobs. Workflow source text and a hand-maintained complete lockfile list are not coverage authorities. Every emitted lock must pass its official-registry audit with no high-severity finding.
- R8. Workflow validation must use exact, reproducibly acquired versions of `actionlint` for syntax and expression semantics and `zizmor` for maintained security analysis. CI obtains them through reviewed SHA-pinned actions or verifies upstream release checksums and records tool versions. Project tests retain only Merman-specific behavior boundaries such as least privilege, trusted source identity, credential isolation, fail-closed gates, artifact identity, digest continuity, and registry reconciliation.
- R9. Every `uses:` step in a job that can publish, request OIDC, consume secrets, use a protected environment, or write repository state must use a reviewed full commit SHA with a readable version comment; all third-party actions are SHA-pinned in every job. Ordinary read-only official actions may use controlled major tags only in unprivileged jobs. GitHub Actions Dependabot must maintain both forms. PR and merge-queue jobs remain read-only, receive no secrets or protected environments, never request OIDC, and cannot produce artifacts consumed by credentialed release jobs.

#### Reference, parity, and performance

- R10. `verify-mermaid-reference` must validate only the selected Mermaid and companion runtime graph, source/package identity, lock state, generated projections, and optionally materialized bytes. Candidate, oracle, deferred-major, and upgrade-decision evidence must run only through an explicit admission workflow. Any change to a selected package, version, integrity, or source identity must carry a reviewed admission decision receipt that binds the old and new selections.
- R11. Signature and provenance admission must invoke official npm/GitHub/Sigstore-capable verification rather than reimplement DSSE, in-toto, SLSA, certificate, or transparency-log semantics. The required admission receipt binds the exact official tool/version, package identity, tarball integrity, behavior result, and raw-output digest; the standing verifier checks only that simple binding and does not reinterpret the protocol.
- R12. `structure` and descendant semantic parity remain blocking release evidence. The blocking root contract must enforce SVG/root structure, finite positive geometry, origin and width/height policy, cropping safety, and exact deterministic root fixtures. Cropping is proven independently by mounting the final SVG in the fixed browser environment and checking painted-content containment rather than reusing production layout bounds; browser-owned exact `getBBox()` numbers remain scheduled/release diagnostics and must not be replaced by another family-tolerance database. Semantic and deterministic exceptional residuals remain exact.
- R13. Performance measurement must have one runner and one receipt schema binding base/head commits, ancestry, corpus and lock digests, machine identity, raw results, and outcome. PR, frontmatter, schedule, manual, artifact, and comment paths consume that receipt instead of reimplementing measurement or status semantics.

#### Internal tools, release, and documentation

- R14. `xtask` must remain a project task entry point. Stable build, compare, generate, package, and owner verification commands stay; uncalled family wrappers and investigation-specific debug commands are deleted or folded into a general command. Typst's package validator remains even if an unneeded standalone wrapper is removed.
- R15. Upstream Cypress fixture collection must execute a proven upstream/runtime harness to emit a versioned structured manifest, or fail explicitly when collection is unsupported. Standing Rust verification checks pinned source/spec/fixture digests and must not maintain a JavaScript evaluator or silently skip unknown calls.
- R16. Release version mutation keeps one shared version interpretation and runs only in a clean, dedicated release worktree. Existing ecosystem-native commands or narrow owner-local editors prepare changes in temporary state with pinned/frozen tools, reject unrelated dependency drift, and produce one reviewable patch. The coordinator validates the complete projected tree, source preimage digests, and `git apply --check` before applying that patch. Preparation and handled validation failures leave the caller worktree unchanged; the contract does not claim power-loss-level multi-file atomicity and does not introduce a generic adapter, journal, or rollback framework.
- R17. Web, Node, Python, Flutter, and GitHub Release retain their existing final-artifact build/verify/publish boundaries and owner-local identity evidence; missing source/file digests or unsafe archive extraction found by a focused audit are fixed at that owner without a universal receipt schema. Credentialed manual publication accepts only a validated immutable release tag or explicit full commit SHA bound to the owner receipt, never a mutable `main` ref; build-only validation may still target `main`. Crates.io is modeled explicitly as a deterministic re-packaging channel: create a pre-publish package receipt, require a matching registry checksum before advancing to dependent crates, and stop in a durable pending-recovery or mismatch state instead of pretending Cargo can upload an arbitrary prebuilt `.crate`. Registry retries require exact source and artifact identity; yank or other destructive remediation is never automatic.
- R18. Documentation must classify current authority, operator guide, machine input, historical report, active workstream, and archived history. Machine inputs are moved to owner paths before any cleanup. Only completed workstream journals with no current inbound links and conclusions already owned elsewhere may be removed, in an isolated final commit; CE plans and public historical targets remain discoverable through a compact archive index. ADR file identities must be unique and checked with a narrow filename/title rule.
- R19. Public crate APIs, CLI behavior, ABI 3, package names and contents, Mermaid 11.16.1 behavior, SVG structure/semantics, platform support, and published-channel semantics must not change without their existing public evidence. Internal command and script compatibility is intentionally not preserved.
- R20. The repository-owned deliverable is a stable `pr-gate`, a precise recommended `main` ruleset and release-environment policy, and truthful read-only reporting of current remote state. Enabling those remote controls requires separate maintainer authorization, is tracked as a non-blocking external product follow-up, and is not counted as solved until read-back verification succeeds.

### Key Flows

- F1. **Pull request:** resolve trusted base/head identities -> classify a NUL-delimited Git diff -> fail broad on unknown/error cases -> run Linux core and affected owner jobs without credentials -> run focused macOS/Windows jobs when relevant -> aggregate success, failure, cancellation, missing results, and allowed skips in the same run -> expose stable `pr-gate`.
- F2. **Mermaid or companion upgrade:** pin source and package candidates -> run the upstream collection/runtime and official signature tools -> review behavior delta and receipt -> select one version -> update the compact selected-reference bundle -> run standing reference and parity gates.
- F3. **Release preparation:** enter a clean dedicated worktree -> validate requested version -> let each ecosystem owner prepare manifests and locks in temporary state -> reject unrelated lock drift -> validate the complete projected tree -> verify and apply one patch -> build final artifacts and owner receipts -> publish in credentialed jobs -> reconcile registries after success, timeout, or response loss.
- F4. **Crates.io publication:** derive topological batches from Cargo metadata -> package and record source/tree/tool/digest evidence -> publish one batch from unchanged source -> require each checksum to match before advancing -> classify exact match, pending visibility, or mismatch -> stop for explicit operator recovery on non-match.
- F5. **Upstream fixture refresh:** derive each collector scope's complete spec set from the pinned upstream test configuration -> execute the corresponding pinned runtime/registration path -> reject missing, reduced, or unsupported registrations -> emit reviewed per-scope manifests and fixtures -> standing verification checks immutable identities without parsing JavaScript.
- F6. **Documentation lookup:** enter through a documentation index -> identify current authority or operator guide -> follow links to machine owners when exact facts are needed -> use the archive index or Git history for completed implementation journals.

### Acceptance Examples

- AE1. Renaming a workflow step, extracting its shell block into a directly tested script, or rewording an alignment document does not fail release verification when behavior and structured authorities are unchanged.
- AE2. Adding a seventh npm lockfile automatically changes the audit planner's emitted matrix and therefore runs an audit without editing workflow YAML. A narrowly excluded generated/test fixture lock requires an explicit tested exclusion with a reason. The same behavior applies to nested Cargo locks.
- AE3. On Windows, `F:\\repo\\binding.js` and `C:/repo/binding.js` become file URLs, an existing `file:` URL remains a URL, npm resolves through the shared command adapter, and a process creation failure reports a diagnostic rather than treating `status: null` as success.
- AE4. A docs-only PR reaches the same `pr-gate` name with expensive owners explicitly skipped; a renderer change runs the complete parity owner; an unknown path, rename, workflow/classifier change, diff error, cancellation, malformed planner result, or missing selected job fails broad or closed as appropriate.
- AE5. An upstream ZenUML candidate evidence-format change cannot break an unrelated PR. Selecting a new ZenUML version requires the explicit admission flow, official signature result, behavior result, and old/new selection receipt before the selected bundle changes.
- AE6. A modified deterministic viewBox, invalid/non-finite root dimension, cropping regression, root width/height policy violation, incorrect production bounds, whole-diagram translation, semantic label, edge marker, or descendant DOM structure is rejected after residual removal. The crop mutations are caught by browser-mounted painted-content containment independent of production bounds. Browser-owned exact bbox movement appears in a scheduled/release report without changing production rendering, broad comparator normalization, or a committed numeric acceptance catalog.
- AE7. A Mermaid upgrade that adds, removes, or renames an in-scope spec, reduces a collector's call/fixture identities, or introduces an unsupported registration fails collection with its scope and source location unless a reviewed removal decision updates that scope; it never silently emits a smaller corpus.
- AE8. Release-version preparation that fails in the Flutter owner after Cargo/npm preparation applies no patch to the caller worktree. A concurrent preimage change or unrelated lock update also aborts before application; the generated patch and disposable release worktree provide recovery rather than an absolute crash-atomicity claim.
- AE9. Each `.crate` receipt and post-publish registry checksum agree before a dependent topological batch starts. Delayed visibility enters a bounded pending-recovery state; a mismatch stops publication, records an incident receipt, and requires an explicit maintainer decision before any yank or resume.
- AE10. The final PR preserves all existing public examples, ABI/platform smoke, selected reference verification, and complete release parity while deleting obsolete internal commands, interpreters, source-shape assertions, and completed workstream clutter.

### Success Criteria

- The two vulnerable `js-yaml 4.3.0` locks are updated and official-registry audits report no high-severity finding for every committed npm lock.
- Audit coverage is mechanically complete for all committed Cargo and npm lockfiles.
- The GitHub Actions subset parser, JavaScript subset evaluator, Rust DSSE/SLSA interpreter, unowned debug commands, family-specific generator wrappers, and prose alignment gates are absent from live code.
- Routine PRs expose one stable same-run `pr-gate`; full macOS/Windows, randomized fuzz, and full expensive evidence remain available on their risk-matched lifecycle.
- Current Mermaid reference verification is independent of future candidate admission.
- The 1.1 MB root residual ledger is removed: blocking root invariants and deterministic fixtures retain mutation-detected sensitivity, while browser-owned bbox numerics are emitted only as diagnostics.
- Release version preparation is owner-driven and patch-based in a clean release worktree; existing immutable artifact boundaries stay intact and crates.io gains a package/checksum receipt with stop-before-dependent recovery semantics.
- Documentation has an explicit authority/lifecycle index, unique ADR IDs, no stale 11.16.0 current-state claim, migrated machine inputs, and no proven-dead completed workstream bulk in the live tree.
- The final branch contains focused commits and one green PR with no badge; remote protection remains truthfully documented as pending until configured.

### Scope Boundaries

This work does not upgrade Mermaid or change its pinned behavior, introduce new user-facing features, redesign ABI 3, rename published packages, publish a release, modify npm/crates.io/PyPI/pub.dev state, or configure GitHub repository settings without separate explicit authorization. It does not remove release preflight checks that validate final bytes, Typst package validation, Apple/Android/Flutter/Python/C smoke, or exact semantic residuals. It does not preserve internal `xtask` or script interfaces that have no active owner.

---

## Planning Contract

### Context and Research

- The source audit is `/Users/frankorz/Downloads/merman-cleanup-review.md`; every finding is dispositioned in the appendix rather than assumed current.
- ADR-0076 requires capability and artifact-profile authorities while explicitly rejecting ordinary prose and source substrings as release evidence.
- `docs/adr/0050-svg-viewbox-parity.md` currently makes three-decimal `parity-root` a release gate. This plan deliberately supersedes its browser-owned bbox portion while preserving blocking descendant parity, root validity/cropping policy, and exact deterministic root fixtures; that ADR must be updated with the replacement evidence.
- ADR-0062 prohibits fixture-specific browser residuals from leaking into production rendering or broad normalizers.
- Existing Web, Node, Python, and Flutter release workflows already build, verify, transfer, and publish final artifacts; the plan preserves that work and focuses release-boundary changes on crates.io and version preparation.
- Official GitHub guidance recommends full commit SHAs for third-party actions, and GitHub Dependabot supports SHA-pinned Actions plus same-line version comments. `actionlint` owns workflow syntax/expression checking, while `zizmor` explicitly remains static analysis rather than runtime proof.
- Read-only repository queries found no `main` protection, no repository rulesets, and no release-environment protection rules. This is an external governance gap, not something a YAML test can repair.

### Key Technical Decisions

#### KTD1. Replace evidence before deleting its predecessor

**Decision:** Every cleanup unit first identifies the user-visible or security property hidden inside the old mechanism, adds or confirms the owner-level executable proof, and only then removes prose, source-shape, duplicated, or hand-parsed evidence.

**Why:** The repository accumulated over-validation around real risks. Deleting the entire mechanism at once would also delete valuable artifact, parity, platform, and fail-closed behavior checks.

**Rejected:** Keeping all old checks indefinitely; deleting large files based only on line count.

#### KTD2. One natural owner per fact, with no new meta-catalog

**Decision:** Consume Cargo metadata, manifests, lockfiles, selected-reference descriptors, family registries, artifact receipts, and public interface schemas directly. Thin orchestration may combine exit status and receipts but may not restate their domain facts.

**Why:** ADR-0076 already removed one attempted central transport/evidence catalog. Recreating it under a cleanup-oriented name would repeat the same failure.

**Rejected:** A universal verification manifest, release-surface status database, or owner-doc table.

#### KTD3. Current runtime selection and future admission have separate lifecycles

**Decision:** Standing reference verification is offline-capable and limited to the selected graph. Candidate collection, browser behavior comparison, signature verification, and future-major decisions are explicit upgrade tasks coordinated by `align-mermaid-release` and may use networked official tools.

**Why:** A future candidate's schema or availability must not block current builds, Pages, or unrelated releases.

**Rejected:** Keeping candidate/oracle/deferred evidence in the standing bundle; dropping admission entirely.

#### KTD4. Standard workflow analyzers plus small behavior contracts replace source-shape testing

**Decision:** Use maintained analyzers for GitHub Actions syntax and common security findings. Move reusable command logic into owner scripts and test those scripts directly. Keep only narrow project-specific workflow policies and end-to-end artifact/failure behavior.

**Why:** A home-grown YAML subset cannot model GitHub Actions, while a test that executes a fail-closed gate or installs a produced package proves something users rely on.

**Rejected:** Expanding `github_workflow_contract.py`; replacing it with another local YAML framework; eliminating all release security tests.

#### KTD5. One same-run PR gate; lifecycle-specific full evidence stays separate

**Decision:** The always-on PR orchestrator owns one directly tested CI planner and same-run aggregation. The planner accepts base/head SHAs, obtains `git diff --name-status -z`, emits owner selections and reasons as JSON, and selects broadly whenever it cannot prove a narrow plan. PR and merge-queue jobs are read-only and credential-free. Scheduled, `main`, manual, and release workflows may remain independent because they are not inputs to the PR aggregate. Linux provides broad routine coverage; host runners provide targeted PR coverage and periodic full coverage.

**Why:** GitHub cannot aggregate absent path-filtered workflows with `needs`. A same-run gate is both simpler and compatible with future branch protection.

**Rejected:** More tests that inspect path filters; requiring every expensive owner on every PR; removing full host safety nets.

The lifecycle ownership is explicit rather than expressed as interchangeable "main, schedule, or release" prose:

| Evidence | Pull request / merge queue | `main` push | Schedule / manual | Release |
| --- | --- | --- | --- | --- |
| Linux workspace and blocking parity | Full for shared/core changes; affected owner for narrow changes | Full | Full diagnostic sweep | Reuse release-relevant blocking evidence |
| macOS/Windows Rust suites | Focused host-sensitive inventory for affected changes | Full workspace | Full workspace safety net | Platform/package smoke, not another full duplicate |
| Fuzz | Build every harness and run committed regressions | Build/regressions | Randomized bounded loops | Reuse latest qualifying safety-net evidence |
| Browser root evidence | Blocking containment for affected/shared changes; exact bbox report omitted | Blocking containment | Full containment plus exact diagnostic report | Full containment plus exact diagnostic report |
| Package artifacts | Affected package build/install/load smoke | Owner smoke | Optional rehearsal | Final-byte build, identity verification, and publisher recheck |
| Performance | Affected representative lane | Record trend | Full regression/reference lanes | No duplicate unless release artifacts change the measured surface |

#### KTD6. Root correctness is blocking; browser bbox numerics are diagnostic

**Decision:** Exact semantic/structural residuals and deterministic root fixtures remain fixture-bound. Blocking root checks cover finite positive dimensions, origin/width/height strategy, descendant semantics, and browser-mounted painted-content containment computed independently of production layout bounds. Exact browser bbox numbers are reported on schedule/release but have no per-fixture or family-tolerance acceptance ledger. Mutation tests for incorrect production bounds, translation, and clipping are the admission gate for removing the old catalog.

**Why:** Browser metrics are noisy, while previous Windows, font, viewBox, and layout regressions show that root correctness remains valuable. A family envelope would merely replace one maintenance database with another.

**Rejected:** Deleting root correctness checks; broad tolerances; a new family/evidence numeric envelope; normalizing viewBox or labels away; keeping 1,392 policy-like rows solely because they already exist.

#### KTD7. Release projection is coordinated, not universally parsed

**Decision:** Keep one version/channel parser and the existing `release-version.py` user entry point, but do not create a generic adapter protocol or transaction engine. The command requires a clean dedicated release worktree, lets each existing ecosystem owner prepare changes in temporary state with pinned/frozen tools, rejects unrelated dependency changes, validates the complete projected tree and preimages, and applies one generated patch only after `git apply --check` succeeds. A failed preparation or handled validation error changes no caller file; an interrupted disposable worktree is recreated from Git rather than recovered by a custom journal.

**Why:** All-surface consistency and failure-before-write are useful, but filesystem-wide crash atomicity is unavailable and one Python module should not understand Cargo.lock, npm lockfiles, Gradle, Dart, CocoaPods, and embedded plist syntax.

**Rejected:** Independent manual version bumps; one regex engine for every format; a generic owner-adapter framework; a custom multi-file rollback journal; silently dropping the existing all-surface consistency check.

#### KTD8. Crates.io is an explicit re-packaging exception

**Decision:** Preserve same-source deterministic packaging, record the locally prepared `.crate` digest and inputs, and verify the published checksum. Do not claim Cargo published a previously uploaded arbitrary file when its supported interface republishes from source.

**Why:** A truthful channel-specific contract is stronger than a false universal artifact model.

**Rejected:** Rebuilding without a receipt; unsupported registry upload tricks; weakening other channels to match Cargo's lifecycle.

#### KTD9. Live documentation is current; Git history is the implementation archive

**Decision:** Keep ADRs, current guides, current engineering memory, CE plans, active workstreams, public historical targets, and owner-local machine inputs in the tree. Move machine inputs out of historical workstreams. Delete only proven-dead agent journals or completed workstreams with no inbound link and an already-migrated conclusion, in a separate final commit. Historical release reports remain explicitly historical and a compact archive index points to current guides or fixed history.

**Why:** Completed execution journals can obscure current authority, but broad deletion would dominate a behavior-sensitive PR and could break public deep links. Current ownership must become clear without turning documentation cleanup into a migration program.

**Rejected:** Treating every historical report as current authority; bulk-deleting CE plans, public historical targets, active workstreams, or normative ADRs; machine-gating ordinary prose.

#### KTD10. Internal compatibility is intentionally broken; public behavior is not

**Decision:** Delete obsolete command names, helper APIs, JSON evidence schemas, and script entry points with no active consumer. Preserve public Rust, CLI, FFI, npm, SVG, and release behavior unless an existing public migration process independently justifies a change. (session-settled: user-directed — the maintainer explicitly authorized fearless and breaking internal refactoring while asking that user-relevant behavior remain the decision criterion.)

**Why:** Compatibility layers for internal investigation tools would retain the exact maintenance cost this work removes.

**Rejected:** Deprecation shims for unowned internal commands; using cleanup as justification for unmeasured public changes.

### High-Level Technical Design

```text
domain authority / final artifact / pinned source
                    |
                    v
          owner-local executable probe
                    |
          +---------+----------+
          |                    |
   PR owner job          schedule/release evidence
          |                    |
          v                    v
  same-run stable gate    receipts and artifacts

upgrade candidate -> explicit admission -> selected reference -> standing verification
release version    -> owner preparation   -> validated patch   -> artifact receipt -> publish/reconcile
```

The diagram is an ownership map, not a mandate for one framework. Owner commands may be Rust, Python, Node, Cargo, npm, or platform-native where that is already the natural implementation.

### System-Wide Impact

- **Public interfaces:** Expected to remain unchanged. Existing API, CLI, ABI, package, SVG, and platform probes become the regression boundary.
- **Internal commands:** Many `xtask` debug/generator/import commands and evidence JSON schemas will be removed. Active documentation and skills must migrate in the same unit.
- **CI topology:** PR triggers move under one orchestration run; schedule/release owners retain independent lifecycle entry points. Check names change deliberately except for the new stable `pr-gate`.
- **Release tooling:** Version preparation becomes patch-based and owner-driven in a dedicated worktree. Credentialed publish jobs continue consuming verified artifacts; crates.io gains a receipt/checksum reconciliation path.
- **Upstream alignment:** Current selected Mermaid behavior remains pinned. Upgrade-time tooling changes, so `align-mermaid-release` must be updated and validated with a fixture refresh dry run.
- **Documentation:** Current authority, machine ownership, ADR identity, and stale baseline claims are corrected first. Only proven-dead workstream journals are optional deletion candidates, isolated in a final commit; CE plans and durable historical targets remain discoverable through an archive index.
- **Failure propagation:** Owner scripts return structured/nonzero failure; same-run aggregation treats failure, cancellation, or missing required output as failure. Allowed path skips are explicit data from the classifier.
- **Security:** All lockfiles become audited; high-privilege actions gain immutable identities; official signature tooling replaces local protocol interpretation; no credential moves into a build job.

### Risks and Dependencies

- **Parity sensitivity:** Residual compaction could hide a real geometry regression. Mitigation: mutation tests, exact semantic residual retention, affected-family PR gates, and full scheduled/release runs.
- **CI false skips:** Change classification could omit an owner. Mitigation: conservative shared-path fallbacks, classifier fixtures, one stable fail-closed aggregate, and main/scheduled full safety nets.
- **Workflow analyzer churn:** New analyzers may initially report legitimate legacy findings. Mitigation: triage findings by behavior and use narrow documented suppressions rather than disabling whole rule classes.
- **Registry availability:** npm audit/signature and post-publish registry observation require network access. Mitigation: standing verification remains offline-capable; networked admission/release paths use bounded retries and retain receipts.
- **Version preparation portability:** Some ecosystem-native tools may not be installed locally. Mitigation: owner routines declare pinned prerequisites, compute deterministic edits in disposable state, and fail before producing or applying the final patch.
- **Large deletion reviewability:** Removing debug code and historical docs can obscure accidental public deletion. Mitigation: caller inventories, focused commits, public contract tests after each deletion unit, and a final diff-level review.
- **External governance:** Branch protection and environment reviewers cannot be completed by code alone. Mitigation: land the stable check first, document the exact settings, then perform and read-back verify the external change only with maintainer authorization.

### Resolved During Planning

- Existing Web, Node, Python, and Flutter artifact boundaries are retained rather than redesigned; only verified residual gaps are changed.
- Release-channel lifecycle documentation is already sufficiently owner-specific; no unified channel lifecycle engine will be built.
- The reported duplicate ADR 0079 is stale. The real duplicate IDs are 0041 and 0050 and will be reassigned with reference updates.
- Full macOS/Windows tests are not deleted. PR scope becomes host-sensitive, while `main` pushes and schedules retain full execution; release runs platform/package smoke without duplicating the full workspace suite.
- Root validity, cropping, descendant parity, and deterministic root fixtures remain release gates. Browser-owned bbox numerics become diagnostics after sensitivity evidence proves the replacement contract.

### External Follow-Up

- After the PR exposes a stable `pr-gate`, configure a `main` repository ruleset requiring pull requests, code-owner approval for the checked-in CI trust roots, and `pr-gate`, blocking force-push and deletion, and verify it by read-only API.
- After the workflow names and permission boundaries settle, request explicit maintainer authorization for the documented registry and GitHub Release environment protections; until then, record them as unsolved external governance rather than a completed plan outcome.

---

## Implementation Units

### Unit Index

| Unit | Title | Primary files | Depends on |
| --- | --- | --- | --- |
| U1 | Security audit completeness and Node/host correctness | audit workflows, Node/VS Code locks, Node tools, platform tests | none |
| U2 | Structured domain authorities and prose-gate removal | `admission.rs`, `snapshots.rs`, Cargo closure/order scripts | none |
| U3 | Selected reference and upgrade admission separation | Mermaid reference verifier, upstream bundle/evidence, alignment skill | none |
| U4 | Workflow validation and stable PR orchestration | Actions workflows, source-shape tests, Dependabot, CI docs | U1 |
| U5 | Risk-matched CI and performance topology | main CI, fuzz, parity, performance workflows | U4 |
| U6a | Pinned upstream fixture collection replacement | collector tooling, per-scope manifests, Cypress/package-test/ELK callers | U3 |
| U6b | `xtask` command and evaluator deletion | debug/generate/import modules, Tree-sitter dependency | U2, U6a |
| U7 | Version projection patch workflow and crates.io receipts | release projection, publish tooling, crates workflow | none |
| U8 | Root residual evidence compaction | comparator/catalog, residual data, parity workflows, ADRs | U5 |
| U9 | Documentation and ADR lifecycle cleanup | alignment docs, workstreams, plans, ADRs, current-state docs | U2-U8 |
| U10 | Fixed-point public verification, simplification, and PR | full branch diff, changelog/docs, PR | U1-U9 |

### U1. Close audit gaps and real Node/host failures

- **Goal:** Remove the current high-severity tooling advisories, make lockfile audit coverage complete, and fix real cross-platform execution before changing CI structure.
- **Requirements:** R6-R9, R19; F1; AE2-AE4.
- **Findings:** 10, 21, 22, and 23.
- **Files:** `.github/workflows/security-audit.yml`, `.github/workflows/npm-audit.yml`, `scripts/audit_plan.py`, its focused tests, `platforms/node/package.json`, `platforms/node/package-lock.json`, `tools/vscode-extension/package.json`, `tools/vscode-extension/package-lock.json`, `platforms/node/src/candidates/wasm.mjs`, `platforms/node/scripts/npm-command.mjs`, `platforms/node/scripts/benchmark/footprint.mjs`, remaining Node pack/install subprocess callers, `platforms/node/tests/`, and `scripts/test_artifact_profile_recipe.py`.
- **Approach:** Update both affected locks to a fixed `js-yaml` through their owning package managers and audit against the official registry. Add `scripts/audit_plan.py`, a cross-platform command that enumerates tracked locks, applies a tiny tested exclusion set, and emits the exact Cargo/npm matrices consumed by audit jobs; do not prove completeness by parsing workflow YAML. Treat Windows drive-letter paths before generic URL schemes, route every npm subprocess through the shared adapter, preserve spawn errors and signals, and compare normalized repo-relative paths in tests. Move POSIX shell recipe execution behind an explicit host/tool predicate; keep syntax and real Apple/Flutter behavior with their owner runners rather than reporting a false cross-platform failure.
- **Test scenarios:** A new unlisted nested Cargo/npm lock; an intentionally excluded non-lock fixture; Node and VS Code audits with the vulnerable lock and fixed lock; Windows backslash and slash drive paths; `file:`, `https:`, relative, and POSIX paths; `npm.cmd` resolution; process spawn returning `status: null`, signal termination, and nonzero exit; platform recipe tests with and without Bash.
- **Verification:** Official-registry audits pass for all committed npm locks, Cargo audit covers all committed Cargo locks, Node package tests pass on the local host, Windows-specific fixtures pass without requiring Windows path semantics from POSIX, and the platform test reports a truthful skip only for a missing non-owner host prerequisite.

### U2. Replace duplicated and prose-owned facts with structured owners

- **Goal:** Remove alignment prose from standing verification, replace Cargo display parsing with metadata, and derive crates.io ordering from the workspace graph.
- **Requirements:** R1-R4, R19; AE1, AE10.
- **Findings:** 1, 14, and 15, plus the machine-ownership part of 2.
- **Files:** `crates/xtask/src/cmd/admission.rs`, `crates/xtask/src/cmd/snapshots.rs`, `crates/xtask/src/cmd/verify.rs`, associated xtask tests, `scripts/verify_artifact_dependency_closures.py`, `crates/xtask/src/cmd/compare/profile_budget.rs`, their tests, `tools/publish.py`, `scripts/verify-release-crate-order.py`, `.github/workflows/release-crates.yml`, `docs/release/PUBLISH_ORDER.md`, and direct callers.
- **Approach:** Derive family admission from the canonical family registry, compare registry, structured fixture manifests, and golden presence. Delete `owner_doc`, prose `covered` state, minimum/coverage pairing, backtick-path scanning, and skill-text checks from Rust strict verification. Rebuild artifact closure traversal from `cargo metadata --filter-platform`, following normal dependency edges and resolved features while preserving meaningful required/forbidden package policies. Compute crates.io publish batches by topological sort of publishable workspace packages; use deterministic lexical ordering only among independent nodes. Make publish tooling the executable consumer and keep prose explanatory.
- **Test scenarios:** Missing family registry entry, missing structured fixture/golden, stale upstream baseline, duplicate family ID, renamed/target-specific/optional/proc-macro dependency, forbidden dependency entering an exact profile, dependency cycle, independent crates, unpublished/private workspace members, prose rename, and deleted alignment document.
- **Verification:** Strict verification still fails executable family/fixture/capability drift but ignores ordinary prose shape. Closure tests produce the same allowed/forbidden decisions from metadata without display parsing. One generated publish order is consumed by local publish and CI, and Markdown is not parsed to prove it.

### U3. Separate selected Mermaid reference from upgrade admission

- **Goal:** Make current reference verification small and stable while retaining a rigorous, explicit upgrade path backed by official signature tooling.
- **Requirements:** R10-R11, R19; F2; AE5.
- **Findings:** 7 and 8.
- **Files:** `crates/xtask/src/cmd/mermaid_reference.rs`, command wiring and tests, `tools/upstreams/MERMAID_REFERENCE_BUNDLE.json`, `tools/upstreams/ZENUML_CORE_ADMISSION.json`, candidate/deferred evidence and attestation artifacts, `playground/scripts/zenuml-core-candidate-matrix.mjs` or its successor, package manifests/locks, `.github/workflows/ci.yml`, release/Page workflows, `docs/adr/0061-external-diagram-dependency-admission.md`, `tools/upstreams/README.md`, and `.agents/skills/align-mermaid-release/`.
- **Approach:** Reduce the standing bundle to selected source/package versions, integrity/digests, runtime registrations, and generated projection inputs. Move candidate/oracle/future-major behavior comparison into an explicit admission entry used only by the alignment skill or a manual upgrade workflow. Invoke a pinned official npm/GitHub/Sigstore-capable verifier during admission and persist a mandatory decision receipt binding old/new selection, tool/version, package identity, tarball integrity, behavior result, and raw-output digest. Standing verification checks only that binding when the selection changes. Delete Rust DSSE/in-toto/SLSA/certificate interpretation. Remove admitted or abandoned candidate/deferred artifacts from the live standing graph; preserve the final selection rationale in an ADR/current upstream guide.
- **Test scenarios:** Selected package/lock/source digest drift; materialized runtime drift; candidate receipt schema changes; missing or invalid official signature result; candidate behavior mismatch; future-major evidence absence; and an unrelated PR with no candidate artifacts present.
- **Verification:** Ordinary `verify-mermaid-reference` passes offline from the selected graph and is exercised by CI/Pages/release. A selected identity change without the matching admission receipt fails; explicit admission fails closed without official verification and behavior evidence. No Rust code parses DSSE/SLSA internals, and candidate/deferred state cannot block standing verification.

### U4. Replace workflow source-shape contracts and build one stable PR gate

- **Goal:** Remove the home-grown Actions parser and brittle tests while preserving Merman-specific security properties and creating an enforceable same-run PR result.
- **Requirements:** R5, R8-R9, R20; F1; AE1, AE4.
- **Findings:** 3, 4, 11, 12, and 13.
- **Files:** `scripts/ci_plan.py`, its focused tests, `scripts/github_workflow_contract.py`, its seven direct consumers, `scripts/test_release_workflow_security.py`, `scripts/test_release_artifact_workflow.py`, relevant owner scripts/tests, `.github/workflows/ci.yml`, PR-triggered owner workflows or reusable workflow files, `.github/dependabot.yml`, workflow analyzer configuration, and `docs/development/CI.md`.
- **Approach:** Run `actionlint` and `zizmor` through exact reviewed tool identities. Before deleting old workflow tests, create a temporary migration ledger that maps every existing assertion to a standard analyzer, a directly executed fail-closed owner entry point, a real artifact/package smoke, or an explicit deletion rationale; the ledger is review evidence, not a new standing catalog. Extract credential-sensitive publish logic into owner entry points that revalidate source, producer, version, digest, and target channel immediately before publish. Implement R5 in `scripts/ci_plan.py`: accept base/head SHAs, consume NUL-safe Git name-status data, emit validated JSON selections/reasons, and fail broad on every uncertainty. Keep PR/reusable jobs read-only with no inherited secrets, OIDC, protected environment, or release-artifact handoff. Pin every action in privileged jobs and every third-party action per R9; add weekly GitHub Actions Dependabot with readable version comments. First run the new gate beside existing checks on a draft PR revision and prove a seeded owner failure is detected; only a later commit deletes the parser and source-shape assertions. Keep schedule/manual/release lifecycle entry points separate.
- **Test scenarios:** Semantically equivalent workflow refactor; invalid Actions expression; template injection; mutable action anywhere in a privileged job; excessive token permission; fork PR; workflow edit; untrusted checkout in a credentialed job; release workflow callable from PR orchestration; owner script failure/cancel; valid empty diff; unknown/shared/deleted/renamed path; missing base; malformed planner output; and a selected job missing its result.
- **Verification:** Exact `actionlint` and `zizmor` identities and policies pass. The new and old gates agree on a real draft-PR revision before the custom parser and source-shape assertions are removed. PR fixtures prove success, broad fallback, failure, cancellation, and skip aggregation, and the real PR exposes one stable `pr-gate`. No PR job has a credential path. A read-only query continues to report remote protection truthfully until external follow-up occurs.

### U5. Match CI and performance cost to risk

- **Goal:** Reduce duplicate routine work without losing platform, fuzz, parity, package, or performance safety nets.
- **Requirements:** R5-R6, R12-R13, R19; F1; AE4, AE6.
- **Findings:** 9 and 20, plus the execution-frequency part of 19.
- **Files:** `.github/workflows/ci.yml`, `.github/workflows/fuzz.yml`, `.github/workflows/performance.yml`, reusable owner workflows, parity command implementation under `crates/xtask/src/cmd/compare/`, benchmark runners/receipt consumers under `tools/bench/`, and CI documentation.
- **Approach:** Record current job durations, repeated fixture-render counts, critical path, and runner minutes for representative docs-only, Node-only, and renderer changes. Keep full workspace nextest and primary parity on Linux. On PRs, run workspace compile plus the explicit host-sensitive inventory on macOS and Windows, including the Windows small-stack ELK regression; run full host nextest on `main` and schedule according to the lifecycle matrix rather than duplicating it in release. Change PR fuzz to harness build plus committed regression corpus and run randomized loops on schedule/manual. First remove duplicated workflow invocations. Reuse immutable parse/render results inside one compare process only if measurement shows repeated work remains a material share of the parity critical path; do not add a cross-command cache or execution framework. Convert performance regression/frontmatter lanes to a descriptor matrix over one measurement/receipt path and one artifact/comment consumer; keep the external reference lane separate.
- **Test scenarios:** Docs-only change, Node-only change, renderer/shared comparator change, Windows layout change, fuzz harness compile failure, fixed regression seed crash, randomized scheduled crash, performance pass/regression/inconclusive/cancelled outcomes, and comment permission failure with artifact retained.
- **Verification:** PR owner selection matches path fixtures and errs toward broader execution on shared paths. Full safety-net triggers match the lifecycle table and remain executable. Docs-only and Node-only plans reduce selected runner work materially (target at least 50% fewer scheduled owner jobs/minutes), planner overhead does not delay the first actionable failure by more than 30 seconds, and the renderer parity critical path does not regress by more than 10% absent explained runner noise. One performance receipt binds all identity inputs and drives artifacts/comments/outcome. CI does not duplicate full platform suites or independent regression/frontmatter orchestration on ordinary PRs.

### U6a. Replace static JavaScript evaluation with pinned executable collection

- **Goal:** Make upstream fixture refresh execute pinned upstream JavaScript/TypeScript semantics and prove that every in-scope source is collected before the partial evaluator is removed.
- **Requirements:** R15, R19; F5; AE7.
- **Findings:** 5.
- **Files:** `crates/xtask/src/cmd/javascript_source.rs`, `crates/xtask/src/cmd/import/cypress.rs`, `crates/xtask/src/cmd/import/pkg_tests.rs`, `crates/xtask/src/cmd/cypress_corpus.rs`, Flowchart ELK coverage callers, pinned Mermaid test configuration, upgrade-only Node collection tooling and per-scope manifests, and `.agents/skills/align-mermaid-release/`.
- **Approach:** Define separate collector scopes for the retained new-family Cypress corpus and Flowchart ELK coverage; delete the unused package-test importer after its caller inventory rather than inventing a manifest for a dead workflow. Each retained scope derives its complete spec set from the pinned upstream test configuration and records source commit, file identities/digests, collector/toolchain identity, registration/call identities, fixture digests, and reviewed removals. The upgrade-only Node collector runs under the pinned Mermaid package-manager lock, loads the actual TypeScript modules through the upstream transpilation/runtime path, and intercepts calls at the imported render-helper boundary while providing only the test-registration host. It does not interpret an AST or emulate JavaScript expressions. Unsupported imports, helpers, runtime side effects, timeouts, missing specs, and reduced identities fail with scope and source location. Run the new collector beside the Rust evaluator and require identity/fixture equivalence for the current three-spec, 36-call Cypress corpus and the retained ELK scope before switching standing verification to manifests.
- **Test scenarios:** Current corpus round-trip; added/deleted/renamed spec; dynamic import/helper; unsupported side effect; timeout; missing registration; reduced call count; reviewed removal; source or manifest tampering; and each retained scope independently changing without rewriting another scope.
- **Verification:** A pinned dry-run collection reproduces every retained current identity and fixture. Standing Rust checks per-scope source/spec/fixture digests without evaluating JavaScript. The old and new paths agree on a real upgrade dry run before the evaluator loses its final caller.

### U6b. Delete unowned `xtask` commands, wrappers, and evaluator code

- **Goal:** Return `xtask` to a small set of stable project tasks after replacement evidence is active, without coupling dead-command deletion to collector correctness.
- **Requirements:** R14-R15, R19; AE10.
- **Findings:** 5 and 6.
- **Files:** `crates/xtask/src/main.rs`, `crates/xtask/src/cmd/mod.rs`, `crates/xtask/src/cmd/debug/`, `crates/xtask/src/state_svgdump.rs`, diagram-specific functions in `crates/xtask/src/cmd/generate.rs`, obsolete importer/evaluator modules, `crates/xtask/Cargo.toml`, `tools/debug/`, and active caller documentation.
- **Approach:** Generate a caller inventory from code, workflows, scripts, skills, and current documentation. Keep general compare/debug-SVG/generate/build/package commands and active `typst-package-smoke`; preserve `validate_typst_plugin` inside package builds. Delete uncalled Architecture/State/Flowchart/Mindmap probes, family-specific SVG wrappers, the unused package-test importer, and the redundant `typst-plugin-smoke` wrapper only when the inventory proves no caller. Remove the JavaScript evaluator and Tree-sitter dependencies only after U6a owns every retained use. Use focused commits so the collector migration and bulk dead-code deletion remain independently reviewable.
- **Test scenarios:** Every retained command has a live owner; removed command names fail cleanly; general SVG generation covers former family wrappers; Typst package build still invokes validation; retained Flowchart coverage uses its manifest; and no source, docs, workflow, or skill references a removed entry point.
- **Verification:** Debug modules, duplicate wrappers, unused importer, JavaScript evaluator, and Tree-sitter dependency are absent only where their caller inventory is empty. Current fixtures, parity, Typst packaging, and retained general commands remain unchanged.

### U7. Make version projection patch-based and add crates.io package receipts

- **Goal:** Preserve cross-surface version consistency without a universal format parser, and close the remaining release-artifact identity exception honestly.
- **Requirements:** R16-R17, R19; F3-F4; AE8-AE9.
- **Findings:** 16 and the remaining part of 17; 18 is explicitly not expanded.
- **Files:** `scripts/release_projection.py`, `scripts/release-version.py`, their tests, small owner-local preparation routines beside existing Cargo/npm/Python/Flutter entry points, Cargo/npm/Python/Flutter manifests and locks, `tools/publish.py`, `.github/workflows/release-crates.yml`, channel-local receipt schemas/tests, `docs/release/RELEASING.md`, and `merman-release` skill guidance if its invocation changes.
- **Approach:** Preserve `release-version.py` as the cross-platform operator entry point and require a clean dedicated worktree. Keep exact owner choices local: TOML-aware routines update Cargo/Python manifests and Cargo regenerates its lock projection; npm owners use pinned `npm version --no-git-tag-version --ignore-scripts` plus package-lock-only regeneration; Flutter/platform files use their existing narrow owner routines and are validated by Dart/Gradle/CocoaPods/plist owner checks where available. All package-manager versions and registries are fixed, unrelated dependency changes fail, and no generic adapter interface is introduced. Prepare the complete tree in disposable state, emit one patch, validate every projection and preimage, run `git apply --check`, then apply once. Before deleting universal parsing paths, run the old and new projectors across the existing stable/prerelease transition corpus and require equivalent intended diffs with no unrelated changes. Preserve existing final-artifact workflows and audit Python/Flutter archive/source binding only for concrete gaps such as unsafe extraction or a missing digest. For crates.io, generate a receipt before each topological batch, publish from the unchanged source/toolchain, and wait for every registry checksum in that batch before advancing. A delayed response records pending recovery; a mismatch stops the chain. Retry requires an exact receipt match, while yank remains an explicit separately authorized operation.
- **Test scenarios:** Cargo/npm/Python/Flutter-only prerelease transitions; pinned tool missing; registry/tool-version drift; unrelated lock dependency change; owner preparation failure; concurrent source modification; malformed or non-applicable patch; crates package digest mismatch; registry delayed visibility; publish accepted before response loss; different-bytes existing version; and partial multi-crate publication recovery without automatic yank.
- **Verification:** The top-level coordinator no longer parses every ecosystem format or edits Cargo.lock blocks, and there is no new adapter/transaction framework. Fault injection proves every preparation and pre-apply failure leaves the caller worktree byte-identical; interrupted disposable state can be recreated from Git and the saved patch. Existing release projection checks remain complete. Web/Node/Python/Flutter artifact identity and safe-extraction tests stay green, and crates.io receipts bind source, package bytes, topological progress, and observed registry checksums.

### U8. Replace browser root residual policy with blocking root invariants and diagnostics

- **Goal:** Delete the browser-owned numeric acceptance ledger while preserving strict detection of semantic, structural, deterministic viewport, cropping, and layout regressions.
- **Requirements:** R12, R19; AE6.
- **Findings:** 19.
- **Files:** `fixtures/_verification/root-parity-residuals.json`, `crates/xtask/src/cmd/compare/root_residual_catalog.rs`, comparator/root tests, root receipt/artifact generation, `.github/workflows/ci.yml`, scheduled/release parity workflows, `docs/adr/0050-svg-viewbox-parity.md`, `docs/adr/0062-fixture-derived-overrides.md`, and current parity documentation.
- **Approach:** Classify current rows into exact semantic/deterministic exceptions and browser-owned bbox measurements. Preserve exact fixture/source/upstream bindings only for the former. Replace the latter with blocking root-contract checks for SVG/root structure, finite positive dimensions, origin and width/height strategy, descendant semantics, and a small deterministic root fixture set. Prove cropping through an independent browser-mount oracle: mount the final SVG in the fixed browser environment, derive painted descendant rectangles from browser layout rather than production bounds, and require containment within the rendered SVG viewport using one documented coordinate-quantization epsilon, never family/fixture thresholds. Handle `foreignObject`/MathML through browser rectangles and exact family invariants. Emit raw exact browser bbox comparison as a schedule/release artifact without an acceptance ledger. Add mutations for viewBox, production bounds, width/height, transforms, clipping, node bounds, markers, labels, and layout offsets. Run the new contract beside the old ledger on a real draft-PR revision and prove each seeded mutation fails before deleting the ledger in a later commit. Run affected families on PRs, all families for shared renderer/comparator changes, and the diagnostic report on schedule/release.
- **Test scenarios:** Browser version/font bbox movement; invalid/non-finite root dimensions; zero/negative dimensions; origin or width/height policy drift; wrong but self-consistent production bounds; whole-diagram translation; clipping; `foreignObject`/MathML containment; deterministic viewBox/transform mutation; semantic label/marker mutation; removed deterministic fixture; source/upstream digest drift; shared renderer change; family-local change; and diagnostic environment identity drift.
- **Verification:** All root-contract and semantic mutation cases fail through the intended independent path, browser exact bbox movement does not fail routine PRs, exact deterministic/semantic residuals remain exact, and production code/normalization contains no fixture-specific workaround. A real PR revision records old/new gate agreement before the 1.1 MB numeric ledger and its catalog maintainer are deleted; schedule/release retain an attributable browser diagnostic artifact.

### U9. Establish documentation lifecycle and remove stale history from the live tree

- **Goal:** Make current authority discoverable, remove prose from machine ownership, and fix proven documentation drift without letting bulk archival dominate the behavior-sensitive diff.
- **Requirements:** R1-R2, R18, R20; F6; AE1, AE10.
- **Findings:** 2, the resolved state of 18, and 24.
- **Files:** create or update the documentation root/index, archive index, and alignment authority index; `docs/alignment/*_MINIMUM.md`, `docs/alignment/*_UPSTREAM_TEST_COVERAGE.md`, `docs/alignment/STATUS.md`; a bounded set of proven-dead `docs/workstreams/` journals; migrate `docs/workstreams/web-wasm-playground/editor-artifact-receipt-v2.json` to a Playground-owned machine path; `docs/knowledge/engineering/current-state.md`; `docs/release/ALPHA3_TO_ALPHA5_REFACTORING_REPORT.md`; `docs/adr/0041-*`, `docs/adr/0050-*`, all inbound references, and a narrow ADR identity test. CE plans remain in place.
- **Approach:** Define six visible classes: current authority, operator guide, machine input, active workstream, historical report, and archived history. Move the editor artifact receipt consumed by Pages and Playground to an owner path before changing its workstream. Merge or detach the 33 paired alignment prose documents from machine gates where their explanatory value is duplicated; capability/fixture machines remain elsewhere. Retain CE plans, active workstreams, public migration/release targets, and everything associated with `presentation-theme-model`. Generate a deletion candidate list and remove only journals that are explicitly completed, have zero current inbound links, and whose conclusions already live in a current owner; keep this optional cleanup in its own final commit and do not make an arbitrary deletion count a completion gate. Reassign the later-created duplicate ADRs, `docs/adr/0041-dagre-graphlib-dugong.md` and `docs/adr/0050-release-quality-gates.md`, to unused IDs, update internal references, and record prior identities in the archive index; preserve the earlier 0041/0050 identities. Add a tiny filename/title identity check. Correct the stale 11.16.0 current-state claim to 11.16.1. Mark historical release reports clearly and point them to current channel guides without changing historical measurements.
- **Test scenarios:** Current authority and archive link resolution, machine receipt owner migration, active workstream and CE-plan retention, optional completed-journal candidate validation, duplicate ADR filename, filename/title mismatch, stale Mermaid baseline search, and ordinary prose rename without release failure.
- **Verification:** Readers can identify the current owner and durable history from the indexes. No duplicate ADR ID, stale current 11.16.0 claim, or machine input under a historical workstream remains. Any journal deletion satisfies all three eligibility predicates and is isolated from behavior changes; all current links resolve and no machine gate parses ordinary prose.

### U10. Run fixed-point review, simplify the final design, and open the PR

- **Goal:** Prove the cleanup retained public contracts, remove abandoned scaffolding, and deliver a reviewable green pull request.
- **Requirements:** R1-R19 and the repository-owned portion of R20; F1-F6; AE1-AE10.
- **Findings:** All 24 dispositions.
- **Files:** Full branch diff, touched README/CONTRIBUTING/release/CI docs, changelog only where user-visible operator behavior changed, and PR metadata.
- **Approach:** Re-run caller inventories and delete transitional helpers, duplicated tests, stale fixtures, and temporary migration evidence. Review the final architecture against ADR-0076, `docs/adr/0050-svg-viewbox-parity.md`, ADR-0062, and the audit principles. Use change-scoped local checks for each unit; let PR CI own expensive cross-platform/full evidence unless a local tool is needed to diagnose a failure. Open the single draft PR at the U4 dual-run checkpoint, continue with focused commits, run correctness, maintainability, project-standard, testing, security/reliability, and adversarial diff review, repair findings, mark the no-badge PR ready, and watch required jobs to green.
- **Test scenarios:** Public Rust examples; CLI help/render/lint; ABI and platform package smoke; Node/Web package install/load; Typst build/package smoke; selected Mermaid reference; full structure/parity/root; all lock audits; workflow analyzers; release dry-run/receipt faults; documentation links; and clean generated/diff state.
- **Verification:** Every repository-owned Definition of Done item has repository or CI evidence. No active caller references a removed command/schema/path. The PR records the real old/new workflow and root-gate checkpoints, explains deleted mechanisms, replacement evidence, retained public guarantees, CI lifecycle changes, unsolved external ruleset follow-up, and residual risk without a badge.

---

## Verification Contract

### Local change-scoped gates

Run the narrow owner tests after each unit instead of repeatedly executing the whole repository. The expected floor is:

```bash
cargo fmt --all -- --check
cargo nextest run -p xtask --no-fail-fast
npm test --prefix platforms/node
python3 scripts/test_audit_plan.py
python3 scripts/test_ci_plan.py
python3 scripts/test_workflow_path_filters.py
python3 scripts/test_artifact_profile_recipe.py
git diff --check
```

When a listed test module is removed or split by the plan, run its owner-level successor. A local toolchain absence is reported, not represented as a pass.

### Structured authority and reference gates

```bash
cargo run -p xtask -- verify-capability-surface
cargo run -p xtask -- verify-artifact-profiles
cargo run -p xtask -- check-alignment
cargo run -p xtask -- verify-mermaid-reference
cargo run -p xtask -- verify --strict
```

`check-alignment` after U2 means executable family/fixture/upstream alignment only. It must not regain document pairing, owner prose, or backtick scanning. Candidate admission is intentionally absent from standing strict verification.

### Workflow and security gates

```bash
actionlint
zizmor --min-severity high .
cargo audit --file Cargo.lock
cargo audit --file fuzz/Cargo.lock
cargo audit --file crates/merman-node/Cargo.lock
npm audit --registry=https://registry.npmjs.org --prefix <each committed npm lock owner>
```

The checked-in workflow owns the exact reproducible analyzer identity and invocation. The audit planner emits matrices directly from tracked locks; its discovery/exclusion behavior is unit-tested without inspecting workflow source. High-risk workflow policies are additionally proven by focused project tests and real release artifact behavior, not by step-name or command-substring assertions.

### Public behavior and package gates

```bash
cargo nextest run --workspace --no-fail-fast
cargo test -p merman --doc
cargo run --release -p xtask -- compare-all --mode structure
cargo run --release -p xtask -- compare-all --mode parity
cargo run --release -p xtask -- compare-all --mode root-contract
npm test --prefix platforms/web
npm run verify:packages --prefix platforms/web
npm test --prefix platforms/node
cargo run --locked -p xtask -- build-typst-package --profile publish
cargo run --locked -p xtask -- typst-package-smoke --profile publish --skip-wasm-build
```

PR CI may select affected owners/families as specified in U5 and U8. Shared renderer, comparator, capability, artifact-profile, or reference changes trigger the full relevant blocking matrix. `main` pushes and schedules retain the full host safety nets; schedules own randomized fuzz; schedule/release own the exact browser-root diagnostic; release owns final package bytes. The lifecycle table is the authority when a lane does not belong on every trigger.

### Release and documentation gates

- Fault-injection tests prove owner preparation and pre-apply failures leave the caller worktree unchanged; the complete patch is validated before application and package receipts bind unchanged source and final bytes.
- Existing Web, Node, Python, Flutter, GitHub Release, native binding, legal, and license verification stays green.
- Crates.io dry-run receipts are produced without publishing during this plan; registry reconciliation logic is tested against recorded/fake responses.
- Documentation links, ADR identities, Mermaid baseline references, and generated projections are checked. Ordinary prose content is human-reviewed, not substring-gated.
- Expensive or unavailable remote/platform checks are delegated to PR CI and named explicitly in the handoff.

---

## Definition of Done

- R1-R19, the repository-owned portion of R20, and AE1-AE10 are satisfied by owner-level executable evidence.
- All tracked Cargo and npm locks are audited; no known high-severity `js-yaml 4.3.0` finding remains.
- Windows paths, npm subprocesses, and POSIX-host test prerequisites have focused regressions and pass on their owner runners.
- `github_workflow_contract.py`, workflow source-shape assertions, the JavaScript evaluator, Rust DSSE/SLSA interpretation, dead debug commands, duplicate generator wrappers, and manual prose admission state are deleted.
- Exact-identity `actionlint`/`zizmor`, same-run owner aggregation, stable `pr-gate`, privileged-job action pinning, credential-free PR jobs, and GitHub Actions Dependabot are active in repository CI.
- Routine PR CI is owner- and risk-scoped; measured narrow PR plans improve materially without regressing the renderer critical path, and each full platform/fuzz/parity/package/release safety net remains on its explicit lifecycle.
- Selected Mermaid reference verification is independent of candidate admission and stays valid without network access; a selected identity cannot change without its official-tool and behavior admission receipt.
- Descendant parity, root validity, independent browser-mounted cropping containment, and deterministic root fixtures remain blocking and mutation-sensitive; the browser numeric ledger is gone, diagnostics remain attributable, and no production workaround exists.
- Version preparation is patch-based and ecosystem-owned without a new transaction framework; Web/Node/Python/Flutter immutable artifact boundaries remain intact; crates.io package receipts, batch barriers, and checksum reconciliation are implemented.
- Documentation exposes current authority and archived history, retains CE plans and active workstreams, migrates machine inputs, removes only eligible dead journals, has unique ADR identities, and names Mermaid 11.16.1 consistently in current-state material.
- Public Rust, CLI, ABI, npm, SVG, Mermaid parity, Typst, native platform, legal, and package contracts pass their existing owner gates.
- The branch is reviewed, simplified, committed, pushed, and represented by one green PR against `main` without a Compound Engineering badge.
- Remote branch/environment protection is documented and truthfully read back, but remains an explicitly unsolved external product follow-up until the maintainer separately authorizes configuration; repository documentation never claims an unverified setting.

---

## Appendix: Audit Finding Disposition

| Finding | Current judgment | Disposition |
| --- | --- | --- |
| 1. Prose as release contract | Valid | U2 removes owner-doc, pairing, path, and skill-text gates while retaining executable alignment. |
| 2. Documentation ownership | Valid | U9 establishes lifecycle classes, migrates machine inputs, and isolates only proven-dead completed journals for optional deletion while retaining CE plans and durable history. |
| 3. Actions subset parser | Valid | U4 deletes it in favor of standard analyzers and owner scripts. |
| 4. Release source-shape tests | Valid but mixed with useful behavior | U4 keeps artifact/security behavior and deletes spelling/topology assertions. |
| 5. JavaScript evaluator | Valid, upgrade-time risk | U6a establishes executable per-scope upstream collection; U6b removes the evaluator only after old/new identity agreement. |
| 6. Historical xtask commands | Partially valid | U6b separately removes uncalled probes/wrappers while preserving general commands and Typst's internal validator. |
| 7. Reference mixed with admission | Fully valid | U3 separates selected standing state from candidate lifecycle. |
| 8. Hand-written DSSE/SLSA | Valid | U3 delegates authenticity to official tools and keeps only receipt binding. |
| 9. CI duplication | Valid with lifecycle exceptions | U5 scopes PR cost but retains full host/release safety nets. |
| 10. Audit gaps | Fully valid | U1 covers every committed lock and fixes current advisories. |
| 11. Cross-workflow aggregate | Fully valid | U4 creates one same-run PR orchestration and stable gate. |
| 12. No remote required checks | Valid external gap | U4 creates the enforceable check; external follow-up configures the ruleset only with authorization. |
| 13. Action pinning | Valid | U4 applies risk tiers and Dependabot instead of indiscriminate policy. |
| 14. Duplicated domain facts | Partially valid | U2 consolidates crates order and consumes existing capability/artifact authorities; no new catalog. |
| 15. Cargo display parsing | Valid in closure verifier | U2 moves it to structured Cargo metadata and preserves semantic exclusions. |
| 16. Universal release projection | Valid, narrower than reported | U7 retains the operator entry point but replaces universal parsing with owner preparation and one validated patch, without a transaction framework. |
| 17. Build/publish boundary | Mostly resolved | U7 preserves solved channels and addresses only crates.io receipts/reconciliation. |
| 18. Different channel lifecycles | Already represented correctly | No lifecycle engine; U7/U9 preserve owner-specific release documentation. |
| 19. Root residual ledger | Valid but safety-critical | U8 deletes browser numeric acceptance rows only after invariant/deterministic mutation evidence; browser bbox becomes diagnostic rather than another tolerance catalog. |
| 20. Performance duplication | Partially valid | U5 unifies runner/receipt/consumers while preserving existing identity evidence. |
| 21. Host-dependent tests | Valid | U1 declares host prerequisites and leaves real platform smoke with owners. |
| 22. Node Windows behavior | Partially fixed, residual defects remain | U1 fixes drive paths, remaining npm callers, null status, and path-normalized tests. |
| 23. High advisory | Fully valid and broader than Node | U1 updates Node and VS Code locks and audits all npm owners. |
| 24. Version/channel/ADR drift | Partially stale | Recent alpha.5 docs and ADR 0079 are already fixed; U9 fixes current-state baseline and duplicate 0041/0050 identities. |
