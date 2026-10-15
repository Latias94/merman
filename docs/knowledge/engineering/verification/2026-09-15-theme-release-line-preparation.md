# Theme release-line preparation

Date: 2026-09-15. Source: `753cdc1c2`, following the archive candidate `57ec788c0`.
This is a local version proposal and compatibility experiment, not a selected release or freeze.

## Fuzz lockfile repair

Preparing a new release initially failed before applying any caller changes: the fuzz workspace's
lockfile did not contain the current theme dependency graph. The root lockfile normalized cleanly;
the missing `merman-theme-contract`, canonical serializer and local dependency-edge updates were
confined to `fuzz/Cargo.lock`. Cargo's offline workspace update refreshed that lock without
upgrading existing registry packages. Commit `753cdc1c2` contains the generated result.

`cargo check --locked --manifest-path fuzz/Cargo.toml` and the root facade's locked SVG check passed.
The stable toolchain check is a lock/compile observation, not the nightly fuzz campaign.
The release coordinator correctly rejected the original drift and left its caller unchanged;
its rejection must not be relaxed to combine dependency changes with a version bump.

## Proposed versions tested

Both proposals used the existing transactional `release-version.py set` coordinator in fresh,
linked worktrees of an independent clone. Cargo 1.95.0 and npm 12.0.2 were used; Node 24.21.0 and
npm 12.0.2 came from task-local installations. An initial PATH ordering selected Node's bundled
npm 11.19.0 and was rejected; placing the npm 12 executable first resolved that environment issue.
The ordinary maintainer worktree was not assigned a new release version.

| Proposal | Version projections | Fresh candidate consumer | Published alpha.6 with candidate siblings |
| --- | --- | --- | --- |
| `0.8.0-alpha.7` | Passed | Passed | Failed with three E0004 errors in the published facade's `diagnostic.rs` |
| `0.9.0-alpha.1` | Passed | Passed | Intentionally not combined: outside the 0.8 Cargo compatibility line |

The command was `python3 scripts/verify_prerelease_compatibility.py --version <proposal>
--previous-version 0.8.0-alpha.6`. This existing check uses the ASCII facade feature recipe; it is
not an all-feature compatibility audit. The positive candidate compile and the rejected
same-line old consumer are real Cargo executions with fresh consumer lockfiles.

Published alpha.6 sibling requirements are not exact. Its facade exhaustively matches the old
`merman_core::Error`; the new core adds `Internal`, `ThemeEvaluationLimit`, and `non_exhaustive`.
The registry facade therefore cannot compile against these new siblings. Replacing correctly
classified errors or restoring obsolete public APIs solely to fit that old match is not proposed.
A new 0.9 release line is the recommended alternative under the existing release admission rule.
This does not change the new, unpublished theme schemas, recipe revisions or catalog versions:
they remain 1. The workspace/package release and the theme wire contract are independent axes.

## Reviewable local proposals

Both proposals have 29 version-owned files, 117 additions and 117 deletions. No error-model or
compatibility alias change was applied. The coordinator completed and the resulting version
projections and whitespace checks passed.

- Alpha.7 worktree: `/tmp/merman-alpha7-linked2`; patch
  `/tmp/merman-alpha7-version-proposal.patch`, SHA-256
  `77f5de865c35ff52ecb63a3ea7f8bd6b982d57b18fd3074be90938e8661c0060`.
- 0.9 alpha.1 worktree: `/tmp/merman-alpha9-linked`; patch
  `/tmp/merman-alpha9-version-proposal.patch`, SHA-256
  `3e3df906697dc649d33468c39497b4a588bdb149e0ea129090c8bf5bf895444c`.
- Logs: `/tmp/merman-alpha7-{version-check,compatibility}.log` and
  `/tmp/merman-alpha9-{version-prep,version-check,compatibility}.log`.
- Release-projection, prerelease-compatibility and fuzz-configuration unit contracts: 33 passed.
  Those fixture-based tests are separate from the actual Cargo results above.

After the maintainer selects the release line, regenerate the version projection from the selected
clean source rather than applying an old proposal over intervening changes. Review version-specific
migration promises, installation examples and package-local changelogs, stamp the intended release
date, then run immutable preflight. Windows and remaining host/compiler-floor lanes, final
artifact/catalog rollout and C7a contract freeze still need their owner evidence. Selecting a
version alone does not complete those gates, and no tag or publishing workflow was triggered.

## Independent checks while release selection is pending

At source `d444a6e1f`, the complete ordinary CI script test command passed 606/606 tests:
`python3 -m unittest discover -s scripts -p 'test_*.py'`. Simulated platform runs printed by those
unit tests are not actual platform builds. The exact CI-pinned nightly lockfile command also
passed: `cargo +nightly-2026-07-01 check --manifest-path fuzz/Cargo.toml --locked`, with one Cargo
build job. This closes the missing nightly compile observation for the repaired fuzz lockfile;
it does not run libFuzzer, ASan or the randomized corpus campaign.

The generated legal owners passed without changing files: 13 Rust dependency reports,
382 release legal projections and the third-party license contract. Logs are
`/tmp/merman-c7a-d444a6e1f-{script-contracts,legal-freshness,nightly-fuzz-check}.log`.
No release version or date was assigned by these checks.

A 0.9 selection also needs the existing native CLI `render/batch -e` retirement commitment
reviewed: its current warning promises removal in v0.9.0. Keep that migration separate from the
permanent root/mmdc compatibility surface. The original version-only proposal did not change
this behavior; the supplementary isolated proposal below now covers that migration.

## Isolated 0.9 CLI migration proposal

The linked candidate at `/tmp/merman-alpha9-linked` now supplements the version proposal with
removal of the native `render/batch -e` aliases, their warning state and argument scanner. Parsing
returns `Cli` directly. Native `-f/--format`, explicit `mmdc -e` and the permanent hidden root
mmdc interface remain supported. README and Unreleased migration text describe that boundary.
The maintainer worktree still has its existing version and native alias behavior; this preparation
does not select the release line.

Actual CLI process and distribution-asset tests passed 30/30, with zero skips, using the candidate
version and the CLI default features plus `layout-elk`. Coverage includes both native format
spellings, rejection of separated and attached `-e` before input acquisition, duplicate-format
rejection, and SVG output through root/mmdc compatibility entry points. The first concurrent
nextest run marked two read-only distribution-document tests `LEAK`; after asset regeneration,
the serial rerun passed all 30 without those markers. This is a test-runner observation, not a
production heap-leak measurement.

The existing `generate_cli_assets.py --write` owner regenerated the 15 manual pages for the
candidate version, checked freshness under both configured timezone/epoch environments, and
verified their Cargo source-package inclusion. Completions did not change. Formatting and
`git diff --check` passed. Logs are `/tmp/merman-alpha9-cli-retirement.log`,
`/tmp/merman-alpha9-cli-assets.log` and `/tmp/merman-alpha9-cli-final.log`.

The full supplementary proposal contains 49 files, 230 additions and 307 deletions, including
the 29 version projections and 15 generated manual pages. Review patch:
`/tmp/merman-alpha9-cli-version-proposal.patch` (57,554 bytes), SHA-256
`e3fd5bbb03796c176fd04a8081ecfe544f156016c6e5f3ed05be7a1783cd94af`.
It remains based on `753cdc1c2`; regenerate version-owned files from the selected clean source
when applying the release decision. These CLI checks do not replace final platform/archive
validation or C7a contract freeze.


## Maintainer selection: v0.8.0-alpha.7

On 2026-09-15 the maintainer selected `v0.8.0-alpha.7` for the upcoming theme release. This
supersedes the earlier 0.9 recommendation. The unpublished theme contracts remain v1, and
the isolated 0.9 CLI alias-removal proposal is not part of alpha.7.

A fresh linked worktree `/tmp/merman-alpha7-final` at source
`0d58624868ebf327f324388d92c0906d7021daf5` ran the existing transactional version owner with
Node 24.21.0 and npm 12.0.2. All 29 version projections were generated successfully and passed
the version and static release-surface checks. Root and coupled package changelogs name the
selected candidate with `Unreleased` status; no publication date has been asserted.

The real `verify_prerelease_compatibility.py --version 0.8.0-alpha.7 --previous-version
0.8.0-alpha.6` run compiled the fresh candidate, then failed the previous-with-candidate-siblings
lane with three E0004 errors in the published facade's `diagnostic.rs`. The current core enum
is non-exhaustive and has additional `Internal` and `ThemeEvaluationLimit` variants. Removing
only the attribute cannot restore the old exhaustive match. The old published manifest has
non-exact sibling requirements, so exact requirements in the new workspace do not repair it.
The error classification, compatibility checker and original published artifacts were not altered.
This preparation does not resolve compatibility admission or authorize publication.

Logs: `/tmp/merman-alpha7-final-version-check.log`,
`/tmp/merman-alpha7-final-surface.log`, and `/tmp/merman-alpha7-final-compatibility.log`.


Alpha.7 preparation then regenerated the 15 CLI manual pages and the 13 Rust license reports
through their existing owners. Legal synchronization refreshed the four affected copies;
all 382 legal projections passed freshness checks. The first full script run exposed only
the stale license input digest; after regeneration, all 606 tests passed in the linked worktree
and again in the primary worktree (`/tmp/merman-alpha7-primary-script-tests.log`). The repository
changelog test now reads the workspace version rather than hard-coding alpha.6; historical
alpha.6 fixtures remain unchanged. Version, changelog, CLI asset, static surface, formatting
and whitespace checks passed. These successful preparation checks do not override the failed
published-facade compatibility lane or substitute for immutable release preflight.


## Bounded compatibility assessment at `be2d12d20`

This assessment uses tracked source `be2d12d205b49acf6bb868301467870285eee6f0` with the
selected alpha.7 projections. It does not change the published alpha.6 package, candidate
error semantics, retired production APIs, or the selected release version.

The cached crates.io `merman-0.8.0-alpha.6/Cargo.toml` declares `0.8.0-alpha.6`, without `=`,
for `merman-core` and each optional coupled sibling. Fresh consumer metadata confirms that
Cargo can select registry `merman` alpha.6 together with local candidate alpha.7 siblings.
The experiment uses the existing owner's manifest generator and candidate-package discovery,
with `default-features = false`, separate `ascii` and `svg` recipes, and no copied lockfile.
Cargo runs offline against its existing registry cache with one build job and the shared target.
Local `[patch.crates-io]` entries make the unpublished candidate available; this is a prospective
package combination, not a claim that alpha.7 is currently published.

| Consumer recipe | Resolved relevant packages | Actual compile result |
| --- | --- | --- |
| Candidate, ASCII | `merman`, `merman-core`, `merman-ascii` alpha.7 | Passed |
| Previous facade, ASCII | Registry `merman` alpha.6; core and ASCII alpha.7 | Failed: three E0004 errors in old diagnostic matches |
| Candidate, SVG | `merman`, core, renderer and theme contract alpha.7 | Passed |
| Previous facade, SVG | Registry `merman` alpha.6; core, renderer and theme contract alpha.7 | Failed: nine E0432/E0425/E0603 errors |
| Previous facade, ASCII; isolated removal of `Error`'s `non_exhaustive` attribute | Same versions, one-line experimental core change | Failed: three E0004 errors explicitly naming `Internal` and `ThemeEvaluationLimit` |

The SVG failure broadens the required compatibility decision beyond the core error enum.
The old facade imports `merman_render::presentation`, `RenderFamilyKind`, public math types,
`prepare_with_render_policy` and `plan_render_with_policy`. The candidate has deliberately
removed or restricted those APIs. Repairing only `Error`, or isolating only the core package
version, cannot restore this facade/renderer boundary. Nine emitted errors are the observed
compiler result, not an exhaustive count of every incompatible API.

The one-line experiment lives only in a detached worktree; the primary implementation retains
`#[non_exhaustive]` and the accurate internal/resource error variants. Adding feature-dependent
variants would still leave the independent renderer API failures. Reintroducing the complete
presentation surface would preserve a superseded product contract and add compatibility work
outside the chosen convergence direction. Neither approach is accepted as a closure fix.

### Existing checker coverage repair

Commit `2ed72b4cf982dd49d930456731d4c61dfeba2b33` enables both `ascii` and `svg` in the existing
fresh-consumer check. It adds no second checker or new resolution policy. The focused prerelease
and release-workflow unit tests pass 45/45, including propagation of a previous-facade compile
failure. The real owner `verify(...)` function was then run with both features, offline, using
the shared target: the alpha.7 candidate passes and the old facade fails with the same nine
renderer-boundary errors. This is a correct rejection of the selected candidate under the
existing policy, not a green release gate. These checks are not an all-feature or artifact matrix.

### Decision required before candidate freeze

| Option | Effect on the product and consumers | Disposition |
| --- | --- | --- |
| Restore the old sibling API surface in alpha.7 | Requires more than an error-enum adjustment, including the retired presentation and rendering API | Rejected for this convergence plan; retain the new product contract |
| Isolate the incompatible coupled packages on a new Cargo version line | Lets old facade requirements retain compatible old packages; the earlier 0.9 candidate experiment passed its fresh-consumer lane | Technically coherent alternative; changing the selected version needs a maintainer decision and regenerated projections |
| Keep alpha.7 and explicitly accept the previous-consumer resolution break | Preserves the selected number and new APIs; some users remaining on alpha.6 can fail after dependency resolution selects new siblings | Requires explicit acceptance of this impact and a scoped release-policy amendment before proceeding |

The last option is not merely permission for alpha.7 adopters to change source code. The affected
case also includes a consumer retaining `merman = "=0.8.0-alpha.6"` while its non-exact transitive
requirements resolve candidate siblings. Exact sibling requirements in the new release do not
repair the already-published facade. A preserved compatible lockfile can avoid that particular
upgrade, but is not a general compatibility guarantee or sufficient release admission evidence.

If the maintainer chooses the alpha.7 exception, amend the release instructions and owner gate
explicitly for that transition, preserve the failing result as known impact, require the new
candidate's coherent graph to compile, and publish migration guidance with the release. The
exception must not silently become the policy for future releases. Do not build a generic waiver
framework. No exception, version-line change, tag or publication is authorized by this assessment.

The bounded assessment is complete. The compatibility decision and C7a freeze remain open.
Final immutable artifact rebuilding should follow that decision, since a different package line
would change the candidate's manifests, generated projections and artifact identities.

Logs and review artifacts:

- Experiment directory: `/var/folders/zk/87rg5ff15mlfnplph83p5ntm0000gn/T/merman-alpha7-boundary-lsz_43dx`; contains fresh consumer manifests, resolved package metadata, compile logs, `results.json`, and the isolated one-line patch.
- Experiment driver: `/tmp/merman-alpha7-boundary-assessment.py`; diagnostic scaffolding, not a new repository release tool.
- Combined owner check: `/tmp/merman-alpha7-combined-compatibility.log`.
- Focused unit/workflow checks: `/tmp/merman-alpha7-boundary-unit-tests.log`.

## Current alpha.7 candidate rerun at `26e566f5b`

The current candidate reran the repository script suite with `python3 -m unittest discover -s scripts -p 'test_*.py'`: 607 tests passed. The alpha.7 version projection check and release changelog check passed for `0.8.0-alpha.7`. Legal projections passed with `scripts/sync-release-legal-materials.py --check` (382 files), and governed Cargo package legal materials passed for 24 packages. `git diff --check` passed.

The real prerelease checker was then run offline for `0.8.0-alpha.7` against published `0.8.0-alpha.6`. The fresh alpha.7 consumer compiled. The `previous-with-candidate-siblings` lane failed again with the same nine SVG compilation errors in the published alpha.6 facade: removed `merman_render::presentation`, `RenderFamilyKind`, `prepare_with_render_policy`, `plan_render_with_policy`, and public `math` exports. This confirms the compatibility result at the current candidate and keeps the release-contract decision open; no checker weakening or compatibility exception was applied.

## Maintainer decision: accept alpha.6 transition impact (2026-09-16)

The maintainer accepted the bounded previous-facade resolution break for `v0.8.0-alpha.7`.
This is a release-policy decision for the published alpha.6 manifest and does not claim that
mixed alpha.6/alpha.7 consumers compile. Alpha.7 retains the accurate new `Error` variants and
retired renderer boundary, and all coupled workspace dependencies remain exact
`=0.8.0-alpha.7` requirements. The compatibility checker must continue to record the alpha.6
failure; candidate-owned alpha.7 graphs and artifacts still require independent passing checks.
Consumers should keep a coherent alpha.6 lockfile or migrate the facade and coupled siblings
together. No compatibility shim, error reclassification or checker weakening is authorized.

The candidate-only compatibility gate was rerun after the decision with
`verify_prerelease_compatibility.py --version 0.8.0-alpha.7 --allow-missing-previous` and
passed the fresh alpha.7 Cargo resolution. The previous-facade lane remains intentionally
recorded as a known-impact failure and is not run as a required passing lane under the accepted
transition policy.

## Candidate-owned acceptance rerun after the alpha.6 policy decision (2026-09-16)

Release-mode owner commands passed after accepting the bounded transition impact:

- preset qualification library: 8/8;
- route cutover, preset qualification and Block/Class/Flowchart retirement integration tests:
  3 + 1 + 1 + 1 + 2 + 2 = 10/10;
- native export smoke: 1/1.

These checks exercise the alpha.7 candidate-owned model and retirement boundary. They do not
turn the recorded alpha.6 previous-facade failure into a pass, and they do not prove missing
Linux, Windows or Swift 5.9 host execution.
