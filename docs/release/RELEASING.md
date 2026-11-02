# Releasing

Status: maintained release operator guide.

Merman releases use a preflight-first flow. Run the release preflight workflow against the intended
source ref and version before any registry or GitHub Release publication. After preflight passes,
push a `v<version>` workspace tag only for the tag-triggered CLI/LSP and crates.io surfaces. Flutter
uses its own `flutter-v<version>` tag because pub.dev requires a tag-triggered workflow; Python,
Android, Apple, Web, Node, and VS Code remain owner-dispatched surfaces.

Main's [binding delivery plan](../plans/2026-11-01-1248-refactor-binding-delivery-plan.md)
adds complete SVG math products and ecosystem-specific handoffs. These are unreleased changes;
they do not replace published 0.9.x artifacts. Use [prebuilt product choices](PREBUILT_PRODUCTS.md)
for host selection and the tagged package guides for existing installations. Candidate
qualification, a complete handoff and a publicly resolvable package are distinct release states.

Release classification follows SemVer syntax, not the major version number. Versions without a
prerelease suffix, including `0.7.0` and `0.8.0`, are regular releases. Only versions with a
prerelease component such as `0.8.0-alpha.5`, `0.8.0-beta.1`, or `0.8.0-rc.1` use a prerelease
channel. Do not describe the project or a `0.x` release as alpha solely because its major version
is zero.

## Maintenance branches

Use `release/0.9.x` for the compatible 0.9 maintenance line, including 0.9.1, 0.9.2, and later patch releases. The branch was renamed from `release/0.9.1` after the 0.9.2 backports were merged; published tags retain their original commits.

- Target backport pull requests at `release/0.9.x`. Accept compatible bug fixes, security fixes, and the documentation or packaging changes needed to release them.
- Keep the published 0.9 integration contracts and Mermaid 12.1.0 baseline. New features, the compiled-theme migration, and Mermaid baseline upgrades continue on `main`.
- Prefer fixes that land on `main` first, then cherry-pick them with their regression tests. Adapt a fix to the maintenance implementation when a direct cherry-pick would import development APIs. Forward-port maintenance-only fixes when they also affect `main`.
- Keep `v0.9.1`, `v0.9.2`, and later release tags immutable. The branch moves as fixes are merged; a tag identifies the exact released source.
- Freeze the candidate to a 40-character commit SHA and run the release preflight for that SHA and version before creating its tag. The preflight takes an immutable SHA rather than the maintenance branch name; publication remains owned by the existing tag and dispatch workflows.

The maintainer-authorized exception for issues #242/#243 adds the native C and WASI SDK
delivery surface to 0.9.x. It does not authorize other feature backports or a baseline,
Options schema, existing ABI-prefix, codec-discovery or resource-policy migration.
In particular, maintenance retains its legacy constrained math admission rather than
adopting main's newer rejection policy. Each SDK profile still requires its own preflight
and actual host qualification before publication.

The branch name does not publish a version. Historical release notes may retain the branch name used at the time of publication.

## Contract version baseline

Version a public API, ABI, or serialized format against its latest published contract, including
prereleases. Keep a new, unpublished contract at version 1. Consolidate incompatible changes
within one unreleased development cycle into a single next contract version; commits and merge
repairs do not each allocate another version. A compatible change need not bump the contract.
Package versions follow their own SemVer and channel rules.

Before consolidation, check the owning registry, release tag, and separately distributed artifacts.
Do not reuse a version that has already shipped. Regenerate and verify matching bindings together;
use source identities and content digests to distinguish unpublished build artifacts.
Migration batch IDs and historical evidence revisions identify past observations, not public
compatibility versions. Preserve their identity and references rather than relabeling old evidence.

## Release Stages And Evidence

Keep each selected delivery surface in one observable state:

| State | Required evidence |
| --- | --- |
| Selected | Maintainer request, version and delivery scope |
| Prepared | Reviewed source and packaged documentation, owner checks and upload constraints |
| Frozen | Immutable source/tree plus exact artifact and producing run/attempt |
| Submitted | Identified upload attempt or upstream PR |
| Accepted | Registry acceptance or upstream merge |
| Available | Exact version and matching artifact served by the owning registry or release |

A successful preflight proves preparation, and a green upstream PR proves its checks. Neither
establishes registry availability. An upstream merge may precede catalog/API propagation. Record
these states and outstanding conditions in [Publish Order](PUBLISH_ORDER.md); complete checklist
items only for the state actually observed.

Carry the maintainer's authorization forward for the same version, surfaces and necessary recovery.
Resolve the immutable source during preparation. A new surface, conflicting artifact, tag move or
incident yank needs its own explicit decision. The current request and upstream contribution rules
determine authorization; a cached status document does not.

Prepare final user documentation before freezing a package. Record run IDs, registry observations
and pending channels in publication receipts. Keep tool versions and build recipes in their owning
workflows/descriptors, and use the release skill for the sequence and conditional checks.

## Release Workflows

| Workflow | Publishes | Channel |
| --- | --- | --- |
| `release-preflight.yml` | Nothing; dry-run/build verification only | GitHub Actions artifacts |
| `release-sdk.yml` | Nothing; qualifies native C and WASI standard/math/full archives | GitHub Actions artifacts |
| `release-sdk-publish.yml` | Already-qualified SDK archives from a successful, tag-matched run | Existing GitHub Release only |
| `release.yml` | `merman-cli` and `merman-lsp` binary archives, installers, and checksums | GitHub Release |
| `release-cli-registries.yml` | Nothing; verifies published stable assets and produces separately validated WinGet/Scoop candidates | GitHub Actions artifacts |
| `release-crates.yml` | Rust workspace crates | crates.io |
| `release-yank-crates.yml` | Selected workspace-coupled versions after an incomplete publication | crates.io |
| `release-apple.yml` | Seven assets: binary ZIP, complete local SwiftPM ZIP, thin remote tree ZIP, three checksum sidecars and remote manifest | Actions candidates by default; existing GitHub Release with `publish=true` |
| `release-python.yml` | Five `merman` wheels: macOS ARM64/x86-64, GNU Linux ARM64/x86-64 and Windows x86-64 MSVC | Actions candidates by default; existing GitHub Release with `publish=true`, plus PyPI with `publish_to_pypi=true` |
| `release-flutter.yml` | Frozen thin `merman` facade and separate target-pinned native assets for Android, iOS, macOS, Windows and Linux | Tag-triggered native assets to an existing Flutter GitHub Release, then facade to pub.dev; manual runs validate only |
| `release-android.yml` | AAR, complete Maven repository ZIP and signed standard Central bundle handoff | Actions candidates by default; existing GitHub Release with `publish=true`; no Central upload |
| `release-web.yml` | admitted `@mermanjs/web` browser package group | Actions candidates by default; npm with `publish_to_npm=true` |
| `release-node.yml` | experimental `@mermanjs/node` loader, seven native platform packages, and explicit `@mermanjs/node-wasm`: nine packages | Actions candidates by default; npm with `publish_to_npm=true` |
| `release-tree-sitter-mermaid.yml` | `tree-sitter-mermaid` crate, `@mermanjs/tree-sitter-mermaid`, language WASM, and source archive | crates.io, npm, and GitHub Release |
| `vscode-extension.yml` | Platform-specific `merman-vscode` VSIX artifacts | GitHub Actions artifacts |
| `homebrew.yml` | Nothing; Homebrew/core formula health check only | Homebrew |

Before publication, review the complete workflow table above, including channels outside the
FFI/native static contract. In the existing dated release entry in [Publish Order](PUBLISH_ORDER.md),
record each relevant workspace-coupled delivery channel as `selected` or `deferred`. For each
`deferred` channel, record its current independently verified published version and delivery
evidence; if no version can be verified, state that limitation explicitly. Preflight coverage and
shared version metadata do not select a channel or establish its published version. Independently
versioned packages and external channels retain their own release scope and contracts.

Honor an explicitly selected subset and carry forward existing authorization for the same release,
surfaces, and necessary recovery without asking again. Invoke each selected publishing owner through
its documented tag trigger or manual dispatch. A release is complete for its selected scope only
after every selected owner succeeds and an independent query of its registry or actual public
GitHub Release assets confirms the exact version and matching artifacts. A successful upstream or
preflight build, an existing Actions artifact, or an asset name alone does not prove delivery of
the selected source and bytes.

The public GitHub Release body must identify the channels actually delivered and the deferred
channels still on their verified older versions. Keep selected channels awaiting delivery explicitly
pending, and add download links only for confirmed public artifacts. Detailed source identities,
receipts, and dated observations remain in Publish Order; this scope review adds no automatic
publication of deferred channels.

The SDK workflow is deliberately separate from the normal tag-triggered Release: preflight uploads
qualified SDK archives as Actions artifacts, while `release-sdk-publish.yml` manually attaches them
to an existing GitHub Release. To use a preflight run as the SDK producer, dispatch it from a ref
whose run `head_sha` is the exact release source commit; the publisher checks that identity against
the immutable tag. After the primary Release exists, publish using that successful preflight run:

```bash
RELEASE_TAG="v<version>"
PREFLIGHT_RUN_ID="<successful source-matched release-preflight run id>"
gh workflow run release-sdk-publish.yml \
  --ref "$RELEASE_TAG" \
  -f build_run_id="$PREFLIGHT_RUN_ID" \
  -f release_tag="$RELEASE_TAG"
```

The publisher validates all 18 archives and 18 checksum sidecars, attests the verified archive
bytes, and uploads them without rebuilding or replacing existing assets. A successful preflight by
itself does not make SDK downloads available. See [SDK delivery](../../distribution/sdk/README.md)
for the profile and target choices.

The word "FFI" covers two different deliverables. `merman-bindings-core`, `merman-ffi`,
`merman-uniffi`, and `merman-wasm` are source crates in the crates.io graph. Android AAR, Apple
XCFramework, Python wheels, and Flutter Native Assets are separately built artifacts owned by their
own workflows. `merman-android-jni` is intentionally `publish = false`; its public delivery is the
AAR, not a crates.io package. A green preflight proves that those owner builds can succeed, but it
does not publish them and it does not imply that every surface ran.

Run the static, machine-readable FFI/native ownership check before dispatching any owner workflow:

```bash
VERSION="<version>"
python3 scripts/release_surface_contract.py --version "$VERSION"
```

The check is a contract of workflow, artifact profile, and source-crate ownership for the five
FFI/native surfaces above. It does not replace the complete workflow table, and it does not cache
registry status; after publication, query each owning registry or GitHub Release directly.

Most platform owners use manual `workflow_dispatch`. Apple, Android and Python accept a
`release_tag` version label and `source_ref`, resolving the selected source to an immutable commit
and tree before building. With the default `publish=false`, `source_ref` may be `main`, a full
40-character commit or the matching tag; the candidate version's tag need not exist. With
`publish=true`, it must be the matching immutable tag and the GitHub Release must already exist.
The workflow definition may come from `main` for workflow-only recovery without changing that
artifact source. Android additionally requires an explicit `maven_group` and
`prepare_central_bundle=true` for release uploads; PyPI requires both `publish=true` and
`publish_to_pypi=true`. Web/Node default to `publish_to_npm=false`; publication accepts the matching
tag or a reviewed full commit, while `main` is build-only. VS Code resolves its explicit
`source_ref` for packaging. Flutter is the exception: pub.dev automated publishing only accepts GitHub
Actions runs triggered by a pushed git tag, so `release-flutter.yml` publishes from a
`flutter-v<version>` tag and uses manual runs with an explicit `source_ref` for validation only.
Its native-asset job requires an existing non-draft Release for that Flutter tag and verifies
every pinned public download before pub.dev publication. The pub.dev trusted publisher must use
the matching `flutter-v{{version}}` tag pattern.

The crates.io workflow is idempotent only when deterministic local re-packaging produces the checksum
already recorded by the registry. Before each topological batch it packages every member from the
unchanged source, writes a source/tool/artifact receipt, and preflights existing registry checksums. Every missing
member in the batch must pass its locked publish dry-run before the first member is published; each
then receives one publish attempt. The workflow does not enter the next
batch until every checksum in the current batch matches; delayed visibility or a lost response
produces a durable pending-recovery receipt instead of a blind retry.

For WinGet and Scoop, run the independent [CLI registry candidate workflow](../releasing/CLI.md#scoop-and-winget-draft-candidate-contract) after the stable GitHub Release exists. Its read-only preparation and validation do not block binary publication or submit to external registries.

## Required Credentials

| Surface | Credential |
| --- | --- |
| crates.io publish | Trusted Publishing / OIDC for each crate, its owning release workflow, and the `crates.io` environment |
| crates.io incident yank | Dedicated `CARGO_REGISTRY_YANK_TOKEN` secret with yank permission in the `crates.io` environment |
| pub.dev | Automated publishing configured for `merman`, this repository, tag pattern `flutter-v{{version}}`, and the `pub.dev` environment |
| PyPI | Trusted Publishing / OIDC configured for `merman` and `release-python.yml` |
| npm | Trusted Publishing / OIDC configured for every admitted `@mermanjs/web*` and `@mermanjs/node*` package plus `@mermanjs/tree-sitter-mermaid`, this repository, each owning release workflow, and the `npm` environment; allow direct `npm publish` for these operators |
| GitHub Release assets | `GITHUB_TOKEN` from Actions |
| Android Central bundle signing | Protected `maven-central` environment: variables `ANDROID_MAVEN_VERIFIED_GROUP` and `ANDROID_MAVEN_SIGNING_KEY_ID`, secrets `ANDROID_MAVEN_SIGNING_KEY` and `ANDROID_MAVEN_SIGNING_PASSWORD` |
| VS Code Marketplace | Not configured. Marketplace publishing would need `VSCE_PAT`, an explicit publish job, and VSIX provenance verification before enabling. |

Publish/signing jobs use GitHub Environments (`crates.io`, `pypi`, `pub.dev`, `npm`,
`github-release`, and `maven-central`).
Configure required reviewers on those environments if publication should require explicit approval.

Binding build jobs keep `contents: read` and credential-free checkouts. Android's optional signing
job has read-only repository permissions and no Central upload client; the configured verified
group must match `maven_group` exactly. Flutter's native-asset uploader alone receives
`contents: write` in `github-release`; the separate pub.dev job retains `contents: read` plus
`id-token: write` and verifies the frozen thin archive before configuring Dart OIDC. Preparing a
signed Central bundle needs signing inputs, not Portal upload credentials. Actual Portal submission
and promotion remain a separately authorized operator handoff.

Existing Trusted Publisher bindings remain valid across version upgrades with the same package,
repository, workflow and environment. A newly admitted npm package or PyPI/pub.dev project needs
its own binding before its first automated upload. Homebrew, Scoop and WinGet use external
repository review and require no registry Trusted Publisher binding.

pub.dev's publisher settings bind the repository, version tag pattern and optional required
environment. They do not expose a workflow-filename field. The repository owns the
`release-flutter.yml` implementation that handles the matching tag.

For crates.io, register owner `Latias94`, repository `merman`, environment `crates.io`, and the
workflow filename belonging to each crate:

| Crates | Trusted workflow filename |
| --- | --- |
| Workspace-coupled crates, including `merman-unicode-linebreak` | `release-crates.yml` |
| `roughr-merman` | `release-independent-crate.yml` |
| `tree-sitter-mermaid` | `release-tree-sitter-mermaid.yml` |

The single-crate owners use `rust-lang/crates-io-auth-action` immediately before uploading. The
workspace operator exchanges a fresh GitHub OIDC token before each actual upload and revokes the
temporary crates.io token afterwards. This avoids consuming the 30-minute token lifetime during
package compilation, prior batches or registry propagation. Dry runs and checksum observations
remain unauthenticated; the operator removes OIDC request credentials from Cargo subprocess
environments. Authentication failure does not record an attempted upload or fall back to a secret.

An initial upload of a new crate still requires a maintainer API token before its Trusted Publisher
can be registered. Inspect the release's package plan for new names, bootstrap each admitted new
crate in dependency order, then configure its owning workflow binding for later versions. Record
the exact accepted version and checksum in Publish Order.

Local workflow and mocked protocol/recovery checks validate the integration without uploading
another version; they do not prove live token exchange or registry upload. Confirm those on the
next new-version publication, then remove the old publishing secret and optionally enforce
trusted-only uploads. Keep the observed authentication state in Publish Order. The dedicated
yank credential remains separate: Trusted Publishing does not authorize incident yanks.

If a crates.io run stops after publishing only part of a release, keep the tag immutable and retain
the uploaded `crates-io-receipts-*` artifact. Rerun the workflow only from that same immutable tag.
A GitHub rerun downloads the prior attempt's receipts; a new manual recovery must pass
the prior workflow id as `recovery_run_id`. The publisher recreates every `.crate` and requires the
prior prepared/result receipts to match the source, tree, toolchain, graph, manifests, and artifacts
before it observes or mutates the registry. Matching versions are skipped, missing versions continue
in topological order, and a different registry checksum stops before further publication. A mismatch
requires an explicit maintainer decision; the normal publisher never yanks.

For a new-run crates.io recovery, use the immutable tag and the run id that uploaded the prior
receipts:

```bash
gh workflow run release-crates.yml \
  -f release_tag="$RELEASE_TAG" \
  -f source_ref="refs/tags/$RELEASE_TAG" \
  -f recovery_run_id="<prior-run-id>"
```

Android now retains the complete selected-version Maven publication and supports protected signing
of those frozen bytes into a standard Central upload bundle. `prepare_central_bundle=false`
skips signing and reads no signing credentials; `true` selects the `maven-central` environment.
The namespace is an explicit deployment input, and no ownership of `io.merman` or another group is
established by repository metadata. The actual owner must verify the namespace, review and submit
the signed bundle through Portal, then resolve and verify the exact public coordinate from an
external app. No workflow uploads to Central. See the
[Android handoff guide](../../platforms/android/RELEASING.md) for signing configuration and the
standard bundle format.

VS Code Marketplace publishing is credential-blocked. `.github/workflows/vscode-extension.yml`
packages and verifies platform VSIX artifacts only; Marketplace publication needs a dedicated
publish job, `VSCE_PAT`, and artifact provenance verification before it is enabled.

The PyPI project `merman` exists. Keep PyPI Trusted Publishing configured for owner `Latias94`,
repository `merman`, workflow `release-python.yml`, and environment `pypi`. A PyPI Pending
Publisher is only needed before the first trusted publish of a new project name.

The public browser packages are `@mermanjs/web`, `@mermanjs/web-analysis`,
`@mermanjs/web-editor`, `@mermanjs/web-ascii`, and `@mermanjs/web-render`. Configure npm Trusted
Publishing for every package in that lockstep group with workflow file `release-web.yml` and GitHub
environment `npm`. Trusted publishes automatically include npm provenance; the workflow does not
need `--provenance`. A new package name needs a maintainer's 2FA-protected first publication
before its trusted publisher can be configured. Use the frozen-group procedure below; a new
workflow run can rebuild different native bytes. Do not add the bootstrap credential to GitHub Actions.

The independent syntax package is `@mermanjs/tree-sitter-mermaid`, owned by
`release-tree-sitter-mermaid.yml`. Its first `0.1.0` npm publication used a verified workflow
artifact and a maintainer's 2FA-protected bootstrap credential before Trusted Publishing could be
configured for that exact scoped name. That manual bootstrap does not carry npm provenance; keep
that boundary explicit. Trusted Publishing is now configured for later versions through
`release-tree-sitter-mermaid.yml` and the `npm` environment. The Rust crate remains
`tree-sitter-mermaid`; the two registry packages share a version but not a registry name.

Native prebuild bytes are not assumed to reproduce across independent workflow runs. A first-publish
or recovery operator must publish and reconcile the exact candidate from one workflow run, then
rerun only that run's failed jobs. Do not manually publish a candidate from one run and ask a later
run to accept a separately rebuilt npm tarball as byte-identical.

The experimental Node group is `@mermanjs/node`, its seven native platform packages, and the
explicit `@mermanjs/node-wasm` package: nine packages. Native targets are macOS ARM64/x86-64,
Linux ARM64/x86-64 with glibc or musl, and Windows x86-64 MSVC. Its workflow builds and
install-smokes each actual native target plus the Node WASM artifact, packages platform binaries
and the WASM package before the root
loader, and uses the same direct-publish plus integrity-preflight boundary as the browser group.
npm only allows a trusted publisher to be configured for an existing package, so the first release
of each new Node package requires the documented one-time, 2FA-protected bootstrap from the
verified `release-node.yml` workflow artifact; configure OIDC for each newly bootstrapped name,
including the two new Linux ARM64 names. Existing package bindings remain version-independent.
The bootstrap artifact is for that one manual publication only, and its registry version
remains without npm provenance; a later OIDC run cannot add provenance to an existing tarball. From
the next version onward, start a publishing workflow run against the reviewed source so it builds,
verifies, and publishes its own same-run package group. If publication fails, rerun the failed job
in that workflow run; if the original artifact must be recovered in a new run, pass
`recovery_run_id` together with the exact 40-character `source_ref` and let the workflow verify the
prior run identity before downloading the single unexpired package-group artifact. Do not rebuild
the group and assume npm bytes will match, and do not add a persistent npm token to the repository
workflow.

Before starting a Web or Node publication that includes new package names, choose one procedure:

1. Prefer one `publish_to_npm=true` run with an **actually configured environment approval**
   holding its publication job. After the build completes, verify that run's package-group
   artifact, bootstrap only the new names from its exact tarballs under the requested final
   tag, configure their trusted publishers, then approve that original waiting job. It can
   reconcile the matching manual publications and publish the remaining members. An environment
   without an approval rule does not hold the job; confirm the rule before dispatching this route.
2. If using a nonpublishing candidate without that hold, manually publish and reconcile the
   **complete original group** in manifest order under the requested final tag. Preserve its
   artifact and record the lack of npm provenance for manually accepted versions. Configure
   trusted publishers for later versions. Do not bootstrap only a subset and start a new build
   to complete that same version.

Neither procedure uses report-less cross-run recovery to authorize new uploads. A build-only
candidate has no publishing history report; the recovery owner deliberately observes registry
state without publishing missing members in that case. For subsequent ordinary releases, use
the existing same-run OIDC owner and verify its registry provenance independently.

For an npm-only alpha test, `release-node.yml` treats `release_tag` as the package version label and
`source_ref` as the build source. The source may be a reviewed full commit SHA newer than the
same-named workspace tag; the workflow records the resolved commit in the package-group manifest,
and release notes must not claim that separately published channels are byte-identical.

The npm publish jobs run on GitHub-hosted Ubuntu with the owning workflow's pinned Node/npm
versions, enter the `npm` environment, request `id-token: write`, and check out the trusted workflow
revision without credentials. Node additionally checks out the immutable source only to read its
package descriptor; Web uses the trusted revision's descriptor. Trusted verifiers check the
downloaded package-group hashes before publishing missing packages directly under the final tag.
These operators require direct `npm publish` permission; a stage-only trusted publisher is
insufficient. The jobs must not build, test, or execute
source scripts or scripts contained in the downloaded artifact. Do not add `NPM_TOKEN`,
`NODE_AUTH_TOKEN`, `--provenance`, `provenance=false`, or `NPM_CONFIG_PROVENANCE=false`.

Keep preflight and publisher toolchains aligned. Node GNU/musl container images define libc and
native compatibility baselines independently of the newer host packaging tools. Refresh them only
with equivalent platform coverage and artifact evidence; floating tags can change package bytes
between attempts. Apple compatibility likewise requires the exact compiler-floor owner lane on
the selected source. A newer Swift compiler does not prove that floor; move the lane to an
equivalent supported runner/toolchain when its runner becomes unavailable.

With the release binary URL supplied, the Apple packager emits seven assets: the binary XCFramework ZIP, complete local SwiftPM ZIP,
complete thin remote SwiftPM tree ZIP, their three checksum sidecars, and a standalone copy of the
remote manifest. The thin tree includes matching `Sources/Merman`, resource declarations, README
and legal material; its `.binaryTarget(url:checksum:)` binds the separate binary ZIP's HTTPS URL
and checksum. Native source and delivery-repository commits are distinct, avoiding a source/tag
checksum cycle. The workflow never creates or pushes a delivery repository. Publishing and tagging
that exact tree, then resolving its public Git URL, remain separate rollout operations. Optional
`remote_package_url` and `remote_package_revision` inputs qualify an existing exact HTTPS repository
commit against the staged thin archive; supplying neither produces no remote transport evidence.
See the [Apple handoff and HTTPS qualification guide](../../platforms/apple/README.md#prepare-the-remote-swiftpm-handoff).

Homebrew installs `merman-cli` from the formula in `homebrew/core`; it is not published directly by
this repository. After a stable release, Homebrew's autobump flow should pick up the new GitHub tag.
For an ordinary version/checksum bump, track its existing bot PR rather than opening a duplicate;
the [official 0.9.0 update](https://github.com/Homebrew/homebrew-core/pull/317871) follows this flow.
Manual intervention is needed when autobump does not pick up a release, checks fail, or the formula
needs dependency, patch or installation changes.
Use the scheduled `homebrew.yml` workflow, or dispatch it with an optional `expected_version`, to
verify formula freshness and run the installed CLI, linkage, and formula-test contracts. The
workflow is deliberately independent of repository tags because Homebrew/core tracks stable
versions on its own publication schedule.

Before submitting a Homebrew update, follow its current
[contribution rules](https://github.com/Homebrew/homebrew-core/blob/HEAD/CONTRIBUTING.md).
AI-assisted changes and PR prose require the contributor's personal review and tool/model
disclosure; the contributor must answer subsequent maintainer questions and review comments
without AI assistance. Prepare the complete patch and PR description for that review first.

## External Package-Manager Updates

Observe the current registry and existing PRs before submitting an update. Homebrew's autobump and
Scoop's `checkver`/`autoupdate` normally follow new stable releases. Track their bot changes and
intervene when an update is missed, checks fail, or installation/dependency logic needs changing.
When the bot has already published the same version, close a duplicate manual update with the
upstream publication evidence. PR closure alone does not mean the release is missing.

Conda-forge's version bot proposes feedstock updates; inspect every advertised platform's build
before acceptance. When maintainers permit edits to the bot's branch, append the focused fix there
and preserve its PR/history. An `automerge` label requests merging after green CI; apply it only
within the authorized merge scope. Repair the first causal recipe or dependency failure using upstream build
documentation. Preserve compiler hardening and the supported target matrix. A local recipe render
or dependency solve is useful preparation; native CI owns cross-platform compilation and package
tests that the local host cannot run.

For unaccepted WinGet or Nixpkgs initial-package submissions, update the existing maintainer PR to
the new release instead of opening a second initial submission. Preserve package identity and
upstream review history. Verify current CLA/review status rather than treating historical bot
comments as active blockers. Keep source archive checksums, Nix NAR hashes and installed-artifact
checks distinct. Follow each upstream repository's current contribution and AI-assistance rules.

Use the [CLI registry candidate contract](../releasing/CLI.md#scoop-and-winget-draft-candidate-contract)
for WinGet/Scoop validation and the tag-owned CLI verifier for installed capabilities, completions,
manual pages and legal material. Candidate generation, upstream merge and registry propagation are
separate results. Record each result and its remaining acceptance condition in Publish Order.

## Release Evidence

Before tagging, identify the packages and artifact workflows that will actually publish, then run
their owner-specific dry runs. `PACKAGE_SURFACES.md` is a package-choice guide; manifests,
descriptors, artifact profiles, and workflows are the executable evidence. After publication, query
the owning registry or `gh release view "v$VERSION"` directly instead of inferring availability from
a repository-maintained status cache.

### Reusing Immutable Preflight Evidence

Finish version projections, dated changelogs, and packaged installation guidance before freezing the source SHA. A later source edit requires new evidence even when it changes only release documentation.

A successful full preflight for an explicit 40-character source SHA uploads a small `release-preflight-<source>-attempt-<N>` evidence artifact. The tagged crates workflow may reuse its strict Cargo feature matrix and Web build/install smoke when the source commit, tree, version, repository, workflow commit, run attempt, and immutable artifact digest all match. Run preflight from the same reviewed workflow commit intended for publication to make reuse eligible. Branch/tag input preflights remain useful but do not issue reusable evidence.

Missing, expired, older-workflow, or unverifiable evidence falls back to the original checks. Fresh registry consumer resolutions, independent-crate version checks, static contracts, the publish graph, Typst gates, and every publish-time dry-run and registry checksum barrier still execute. Reuse never authorizes publication or proves current registry availability.

### Registry Propagation And Recovery

An accepted upload can remain absent from a registry replica or its consumer index. npm publication records upload intent before sending and keeps the original manifest-bound package group; crates.io completes a batch only after both its API and Cargo sparse index expose matching, non-yanked checksums. Observation has a bounded wall-clock budget. A propagation timeout is a recoverable observation result, not permission to rebuild the artifact or resend an uncertain upload.

Recovery reports bind their producing run and attempt. Workflows select the immediately preceding attempt, or the explicitly selected source run's current attempt. Missing reports, gaps in publication history, or incompatible older reports preserve observation-only mode across subsequent retries and all remaining batches. A successful prefix does not prove that a later package was never uploaded. Only complete job history showing that the actual upload step was skipped can establish that no send occurred.

Keep the original run, artifact, and receipts when retrying. If observation still cannot establish the registry state, inspect that state before taking further publishing action; do not clear the uncertainty flag, substitute an older report, move the tag, or treat a 404 alone as evidence that an upload never happened. Conflicting hashes stop immediately.

## Version Checklist

`Cargo.toml` `[workspace.package].version` is the sole authority for a workspace release. While no
next version is selected, keep the root changelog at `[Unreleased]` and leave the workspace version
unchanged. After the maintainer selects a version, prepare the projection in an exclusive Git
linked worktree. The `set` command rejects the primary checkout, tracked or untracked dirt, and an
npm executable that does not match the `packageManager` pin in `playground/package.json`. Cargo and
npm owners prepare manifests and locks in a disposable detached worktree; the coordinator validates
the complete projected tree and source preimage, checks one binary patch, then applies that patch
once to the caller worktree.

After a maintenance release from a separate branch, synchronize its workspace version projections
back to `main` through the same projection command. Retain pending development changes under
`[Unreleased]` and copy the published release notes beneath them. The version on a development
checkout is a package baseline, not evidence that its full source was published. The `0.9.1` patch
tag excludes the theme refactor and Mermaid `12.2.0` alignment; retain its immutable source and
independent package mappings. Select a new version and pass immutable preflight before publishing
the development source.

Verify every checked-in path without supplying a second version value:

```bash
python3 scripts/release-version.py set --version <version>
python3 scripts/release-version.py
```

Preparation or pre-apply validation failures leave the caller worktree unchanged. Inspect the
reported owner failure, keep the release worktree clean, and rerun the same command; do not copy
partial files out of the disposable worktree or hand-edit generated locks.

After the version projection succeeds, regenerate the CLI completions, manual pages, and
Playground catalog from the selected package version, then verify their source gates:

```sh
python3 scripts/generate_cli_assets.py --write
python3 scripts/generate_cli_assets.py --check
cargo run -p xtask -- gen-playground-example-catalog
cargo run -p xtask -- verify-playground-example-catalog
```

Refresh workspace package versions in the governed dependency paths in
`docs/security/RUSTSEC_EXCEPTIONS.json`, preserving the advisory, affected dependency versions,
review dates, and exit conditions. Run `python3 scripts/verify_rustsec_exceptions.py` to check those
paths against the new lockfile.

The gate discovers workspace members and validates their inherited package versions, internal workspace dependency requirements, `Cargo.lock`, Web package and lock metadata, the Playground's local Web lock, the fuzz-workspace lock, Python's PEP 440 projection, and Android and Flutter manifests. For a prerelease, every coupled root workspace dependency must use the exact Cargo requirement `=X.Y.Z-alpha.N`, `=X.Y.Z-beta.N`, or `=X.Y.Z-rc.N`; stable releases continue to use the ordinary compatible requirement. This permits intentional alpha API breaks without allowing a fresh Cargo resolution to mix sibling release lines.

`cargo check --locked` is not sufficient evidence for a prerelease. The release gates also create
fresh consumers without a copied lockfile and compile the candidate graph plus the previous facade
against candidate sibling packages with both `ascii` and `svg` enabled when both versions are on the
same Cargo compatibility line. These features exercise the facade/core and facade/renderer boundaries;
this check does not replace the complete release feature and artifact matrix. A
failure in that same-line previous-facade lane means the release must restore compatibility or start
a new release line; do not rely on downstream lockfiles to hide the mixed graph. Published registry
tarballs are immutable, so a dependency requirement defect cannot be repaired by editing this
repository after publication; the next release must carry the corrected manifest and pass this gate.

The maintainer accepted one bounded exception on 2026-09-16: published alpha.6 consumers
can break when resolving alpha.7 siblings. For precisely `--version 0.8.0-alpha.7
--previous-version 0.8.0-alpha.6`, add `--accept-alpha6-transition`. Candidate compilation remains
mandatory; the old/new lane is explicitly reported as **NOT VERIFIED, known incompatible**,
not as a compatibility pass. Without that flag the checker still reproduces and rejects the
mixed graph. Both release workflows select the flag only for this exact pair. It cannot apply
to alpha.8, another previous version, or a first-release run. See the recorded decision in
[PUBLISH_ORDER.md](PUBLISH_ORDER.md); alpha.7 exact sibling requirements prevent future mixing
but cannot repair the published alpha.6 manifest. Do not use `--allow-missing-previous` for this
transition. All other same-line releases retain the rule above.

The prerelease compatibility gate is admission control for a new version. A backfill of an
immutable prerelease that was published before this gate existed may use the original tag and the
owner-specific artifact workflows while recording its historical compatibility exception. It must
not move the tag or treat the exception as permission for a later prerelease.

Keep the target Changelog entry marked `Unreleased` during ordinary preparation. Use an unversioned `[Unreleased]` heading while the next workspace version is undecided, then add the selected version before release preflight. Immediately before the immutable preflight, replace `Unreleased` with the intended tag date in `YYYY-MM-DD` form and verify that its version matches the workspace release authority. Do not tag an `Unreleased` entry or reuse a date from an abandoned release attempt.

`python3 scripts/verify_release_changelog.py --version <workspace-version>` accepts an unversioned `[Unreleased]` entry during development and still checks every versioned projection against the supplied workspace version. Immutable preflight adds `--require-date`, which requires every first entry to name that version and a valid date; it never skips pending changes to validate an older release underneath them.

Treat the root `CHANGELOG.md` as the canonical project-wide release narrative and package changelogs as audience-specific projections of the same release delta. Update only the package changelogs for surfaces included in the release; do not copy the complete root entry or create one changelog per Rust crate.

For a stable release following several prereleases, make the pending stable entry cumulative from the previous stable version and label that comparison explicitly. Lead with an upgrade-path table and final breaking contracts, then group new capabilities, changes/fixes, security, measurement evidence, and availability by user impact. Keep published alpha entries unchanged; use the versioned upgrade guides for incremental alpha-to-alpha actions. Do not carry superseded intermediate API names or alpha-only benchmark claims into the final contract. The stable upgrade guide owns detailed old-to-new instructions and examples; the embedding guide owns runnable Rust recipes. Package projections must use their own publication boundary (for example, Node has no 0.7.0 package), final schema/API versions, and actual capability recipe.


| Surface | Registry or audience behavior | Changelog source |
| --- | --- | --- |
| Node | The root loader tarball includes the user-facing changelog for the nine-package main-line group | `platforms/node/CHANGELOG.md` |
| Flutter/Dart | pub.dev renders the package-root changelog as its Changelog tab | `platforms/flutter/CHANGELOG.md` |
| Python | PyPI project metadata links Python users to the package changelog | `platforms/python/merman/CHANGELOG.md` |
| Android | The Android package README links consumers to its JNI/AAR-specific history | `platforms/android/CHANGELOG.md` |
| Apple | The Apple package README links consumers to its Swift/XCFramework-specific history | `platforms/apple/CHANGELOG.md` |
| VS Code | The unpublished extension has an independent version and release boundary | `tools/vscode-extension/CHANGELOG.md` |

For workspace-coupled packages, keep the target package entry at `Unreleased` during preparation and stamp it with the same intended tag date as the root entry immediately before immutable preflight. For an authorized channel-only publication, keep the root entry at `Unreleased` but stamp the channel's package-local entry before building its immutable package artifact. Each projection should contain only user-visible behavior, migrations, compatibility notes, and performance claims verified for that surface. Independently versioned surfaces keep their own version and publication date.

README files are ordinary source documentation and `release-version.py` never rewrites them.
Before immutable preflight, finish the root README, each shipped package README, and the closest
installation guides with the target version, minimum toolchain, supported capabilities, and final
install/import/run examples. Describe normal use directly rather than conditioning it on a later
publication; keep local source builds in development sections. Release-facing Cargo examples use
exact target prerelease versions, and source-only Git installs pin a reviewed full commit.
Inspect the README bytes in each prepared package before freezing the source so the published
artifact already contains usable documentation.

Track candidate status, pending channels, source identities, and verified registry availability in
release records such as `PUBLISH_ORDER.md`. Preparing a target-version install example is source
work, not publication evidence or authorization to publish. Historical reports retain dated context.
Keep public binary download instructions for a deferred channel at its verified published version;
a workspace version projection does not justify pointing users to an unavailable channel archive.

A focused audit command is:

```bash
rg -n --glob '**/README.md' --glob '**/CHANGELOG.md' \
  '0\.[0-9]+\.[0-9]+-(alpha|beta|rc)\.[0-9]+|\[[^]]+\] - Unreleased|published registry packages can still|candidate from Git|[Aa]fter .*is published|[Ww]hen .*is published|[Aa]fter .*is available|[Bb]efore publication|not yet published|git\s*=\s*"https://github\.com/Latias94/merman' \
  README.md CHANGELOG.md crates distribution docs platforms tools
```

Classify each match rather than applying a repository-wide mechanical rewrite. Package-local READMEs included in crates, cargo-dist archives, npm tarballs, wheels, Flutter packages, Apple/Android bundles, Typst packages, or VSIX files are part of the release experience even though they are not version authorities.

After publication or recovery, query each owning registry and GitHub Release independently and
record accepted versions, checksums, availability, and receipts in `PUBLISH_ORDER.md`. User guides
should already describe the frozen target package. Check for preparation defects and correct the
source for the next release if an immutable artifact contains stale documentation. Registry
channels remain independent; pending delivery belongs in the release record.

For Web and Node npm groups, verify every package member's exact registry integrity and inspect its
complete dist-tag map; the downloaded package-group manifest pins the requested final tag. The
shared operator in `scripts/npm_package_group.py` publishes missing versions with `npm publish
--tag` and does not run separate `npm dist-tag` commands. An existing exact version must match both
integrity and the requested tag before further uploads; conflicts stop the operator. npm now
documents [OIDC dist-tag management](https://docs.npmjs.com/trusted-publishers/#managing-dist-tags-with-trusted-publishing)
with a separately enabled `Allow npm dist-tag` permission and supported CLI versions. That is not evidence that
this repository's publisher has that permission, and the current workflow has no automated tag
repair path. Reconcile any required tag correction through a separately authorized operation
before resuming the same frozen group. Confirm that the published tarball already contains the dated
package-local changelog; if it does not, record the immutable artifact defect and fix the source for
the next version. A channel-only npm release does not select the next workspace version.

The unpublished VS Code extension, the Typst package wrapper, and `roughr-merman` own independent
version tracks. They are intentionally excluded from workspace projection. Record the workspace
runtime bundled in VSIX and Typst artifacts through provenance instead of rewriting those package
versions. Before any later `roughr-merman 0.12.x` publication, run `cargo-semver-checks` against
the latest published compatible baseline. Version `0.12.3` is the forward-looking compatibility
floor; the historical `0.12.1` to `0.12.3` recovery is not re-litigated by future release gates.

For the current release lane, also review `docs/release/PUBLISH_ORDER.md`.

## Release Preflight

Before tagging or publishing, run:

```bash
VERSION="<version>"
SOURCE_SHA="$(git rev-parse HEAD)"
PREFLIGHT_REF="<reviewed branch or tag at the intended workflow commit>"
gh workflow run release-preflight.yml --ref "$PREFLIGHT_REF" \
  -f scope=release -f version="$VERSION" -f source_ref="$SOURCE_SHA" \
  -f android_maven_group="io.merman.preflight"
```

The preflight workflow verifies release versions, the static release-surface contract,
prerelease fresh-resolution compatibility, independent-crate Rust API compatibility,
package file lists, registry-independent Rust crate publish dry-runs, the five-target Python wheel
matrix, Android's external Maven app and 16 KB checks, extracted Apple local package/device/simulator
qualification, the Web package dry-run and installed Chromium/Worker consumers, Node native
package-group build/install smokes, platform VSIX packaging, the frozen Flutter thin-package dry-run
and relocated macOS app, and
the native C/WASI SDK archive matrix. The SDK owner qualifies standard, math, and full
separately: five native library targets plus `wasm32-wasip1`, with actual host catalog,
font/output smokes, artifact-specific legal reports and native/WASI PNG comparisons.
See [SDK delivery](../../distribution/sdk/README.md) for the exact recipes. It does
not publish to any registry or attach the SDK archives to the GitHub Release. SDK publication is a
separate manual workflow after the primary release exists; the selected preflight run must have
`head_sha` equal to the tag commit.

`android_maven_group` is a nonpublishing preflight coordinate, not a Central namespace claim.
For an unreleased main-line binding change, dispatch the same workflow with `scope=bindings`.
That scope runs the Python, Android, Apple, Web, Node and Flutter candidate consumers at the
selected immutable source. It omits release-readiness metadata, CLI/LSP, VSIX and C/WASI release
jobs and never issues a reusable `release-preflight-*` receipt. Keep the source's `Unreleased`
changelog and current version projections intact. Use the default `scope=release` and complete
all release gates before publishing; a successful binding candidate is not release authorization.

Python's Linux lanes use digest-pinned manylinux_2_28 images on native ARM64/x86-64 runners, assert
the actual glibc 2.28 baseline, repair/audit the final wheel and install it on Python 3.9 and the
current owner runtime at that floor. macOS ARM64/x86-64 and Windows x86-64 MSVC execute the same
installed-wheel smoke. Keep the audit and consumer receipts with the producing run; changing a
wheel tag alone does not qualify a lower libc floor. Published 0.9.2's three-wheel coverage,
including its manylinux_2_35 x86-64 wheel, remains historical release behavior.

To validate individual main-line binding candidates without publication, select the checkout's
projected version and reviewed source. The commands below are recipes, not dispatch records:

```bash
RELEASE_TAG="v<version>"
SOURCE_SHA="<reviewed-40-character-source-commit>"
MAVEN_GROUP="<selected-nonpublishing-group>"
gh workflow run release-apple.yml \
  -f release_tag="$RELEASE_TAG" -f source_ref="$SOURCE_SHA" -f publish=false
gh workflow run release-android.yml \
  -f release_tag="$RELEASE_TAG" -f source_ref="$SOURCE_SHA" \
  -f maven_group="$MAVEN_GROUP" -f prepare_central_bundle=false -f publish=false
gh workflow run release-python.yml \
  -f release_tag="$RELEASE_TAG" -f source_ref="$SOURCE_SHA" \
  -f publish=false -f publish_to_pypi=false
gh workflow run release-web.yml \
  -f release_tag="$RELEASE_TAG" -f source_ref="$SOURCE_SHA" -f publish_to_npm=false
gh workflow run release-node.yml \
  -f release_tag="$RELEASE_TAG" -f source_ref="$SOURCE_SHA" -f publish_to_npm=false
gh workflow run release-flutter.yml \
  -f release_tag="flutter-v<version>" -f source_ref="$SOURCE_SHA"
```

Apple remote qualification additionally requires both `remote_package_url` and
`remote_package_revision`; the exact binary ZIP must already be reachable at the manifest's HTTPS
URL for a nonpublishing run. Signing an Android candidate is separately selectable with
`prepare_central_bundle=true` while keeping `publish=false`; it requires the protected signing
inputs and exact configured verified group, and retains a standard bundle without uploading it.

Release CI keeps integrity checks at boundaries where bytes or trust cross jobs or registries. GitHub
Actions use reviewed full version tags; source identity uses the commit and tree; SHA-256
is used for downloaded release tools, staged release artifacts, and immutable registry reconciliation
(crates.io, npm, and PyPI). Ordinary unit tests and in-workspace builds do not calculate or compare
extra hashes. pub.dev uses a member-level content comparison because Dart rewrites tar metadata.
Python, Android, Apple and Flutter's native-asset owners compare existing assets with the
original run artifact and upload only missing items; differing bytes fail closed and no asset
is overwritten. Python verifies the complete public wheel set before its PyPI job can run.
Tree-sitter native
prebuild recovery stays within the same workflow run.

The formal Release's final Linux CLI archive must also pass
[scoped preset qualification](../rendering/preset-qualification.md) and fresh replay before the
release verification gate passes. Its Flowchart/State/Sequence system-font observations remain
in a private workflow artifact. The gate attaches only the public
`merman-cli-x86_64-unknown-linux-gnu.preset-catalog.json` companion to a new publication bundle,
binding it to the source and final archive and recording its digest and size in
`release-verification.json`. Registry candidate, attestation and publishing jobs verify that this
companion is present before consuming the final bundle. The original archives are not repacked.
Shared Rust/SDK catalogs remain alpha with empty qualification scope; this archive observation
does not qualify other targets, font installations or arbitrary source text.

Release-archive smoke tests should verify user-observable contracts rather than incidental representation choices. Accept legal binary token and whitespace forms, and allow valid asynchronous notification ordering while still requiring bounded output, the expected protocol responses, successful exit, and exact archive contents. Reproduce failures against the final archive before changing product code.

Record `VERSION` and `SOURCE_SHA` with the preflight run. The run must be green for that exact immutable commit, not merely for a branch name that can move while preflight is running. Create the tag only through the `Tag And Push` step after that run succeeds. A complete native release still requires explicit, successful owner workflow runs for every authorized artifact surface; absence of an owner run is incomplete delivery, not an implicit skip.

For local spot checks, run the normal Rust and platform gates:

```bash
cargo nextest run --cargo-quiet
python3 scripts/artifact_profile_recipe.py cli-release --build-host --locked
python3 -m py_compile \
  scripts/verify-platform-bindings.py \
  scripts/build-python-uniffi-wheel.py \
  platforms/android/build-android.py \
  platforms/flutter/build-native.py
bash -n scripts/build-apple-xcframework.sh platforms/ios/build-ios.sh
python3 scripts/build-python-uniffi-wheel.py --run-smoke
```

On macOS with Xcode:

```bash
bash scripts/build-apple-xcframework.sh
swift run --package-path platforms/apple/examples/smoke MermanAppleSmoke
```

For Flutter:

```bash
python3 platforms/flutter/build-native.py host
cd platforms/flutter
flutter pub get
flutter analyze
dart format --set-exit-if-changed \
  lib/merman.dart lib/src/merman_ffi.dart lib/src/operation_metadata.dart \
  example tool hook
dart run tool/abi3_contract_test.dart
dart run tool/native_assets_contract_test.dart
dart run example/main.dart
dart run example/smoke.dart
dart pub publish --dry-run
```

These are local source spot checks. Release/preflight owners build all supported native targets,
then `scripts/flutter_native_assets.py stage` freezes separate compressed assets and generates the
manifest into staging. `scripts/flutter_release_archive.py pack --native-manifest` produces a thin
facade, and `extract --require-thin` verifies that frozen archive into a fresh directory before
its dry-run. The facade contains `hook/native-assets.json` and no native libraries or archives;
the manifest pins target URLs, sizes and both compressed/library digests. Do not replace that
final-package check with a dry-run from the source directory.

For the already-extracted candidate and its frozen assets:

```bash
python3 scripts/verify_flutter_pub_package.py \
  --package-root "<extracted-thin-package>" --require-thin --max-compressed-mb 99
python3 platforms/flutter/tool/qualify_packaged_app.py \
  --package-root "<extracted-thin-package>" --assets-root "<frozen-native-assets>" \
  --output "<packaged-app-receipt.json>"
```

The macOS qualifier fetches only the selected host asset through a loopback fixture, builds a
release app, relocates it, removes fixture package/cache inputs and executes the bundled library.
It establishes that host slice, not runtime coverage for every cross-built target. The build hook
supports verified caching, offline mode and a local native override; see
[Flutter target acquisition](../../platforms/flutter/README.md#target-native-assets-unreleased).

For local npm validation:

```bash
cd platforms/web
npm ci
npm run build
npm run smoke
npm run verify:packages
artifact_dir="$(mktemp -d)"
python3 ../../scripts/web_package_group.py pack \
  --root ../.. \
  --descriptor web-surface-descriptor.json \
  --artifact-dir "$artifact_dir" \
  --version "<version>" \
  --source-sha "$(git -C ../.. rev-parse HEAD)" \
  --target-dist-tag alpha
python3 ../../scripts/web_package_group.py verify-artifact \
  --manifest "$artifact_dir/web-package-group.json" \
  --artifact-dir "$artifact_dir" \
  --descriptor web-surface-descriptor.json
rm -rf "$artifact_dir"
```

Normal Web releases must use `release-web.yml`. It preflights every existing exact package version
and target tag, then publishes missing versions directly under the requested `alpha`, `beta`, `rc`,
or `latest` tag in manifest order, with the default package last. Do not publish a member manually:
the workflow's manifest and post-publish integrity checks are the source of truth for a retry.

If a Web publication is partial, do not start a new build for the retry. Dispatch the same workflow
with `recovery_run_id` set to the original `release-web.yml` run and `source_ref` set to that run's
verified package-group manifest `source_sha`:

```bash
gh workflow run release-web.yml \
  -f release_tag="v<version>" \
  -f source_ref="<manifest-source-sha>" \
  -f recovery_run_id="<original-web-run-id>" \
  -f publish_to_npm=true
```

The recovery path downloads the original unexpired artifact, verifies its source, version, tag,
and tarball integrity, and skips members already accepted by npm. A new build from the same source
is not an equivalent recovery artifact and can be rejected if npm already accepted one member.

For local Node package validation on the current host:

```bash
cd platforms/node
npm ci
npm test
npm run check:packages
package_root="$(mktemp -d)"
node scripts/assemble-packages.mjs --loader-only --output-root "$package_root"
npm pack "$package_root/node" --dry-run
```

The release preflight and `release-node.yml` remain responsible for building, installing, and
render-smoke-testing all seven native targets plus the explicit Node WASM package. Each native
consumer installs only the root package with optional dependencies enabled, then runs on Node
22.0.0; Linux ARM64 glibc/musl lanes execute on ARM64 runners. A local host build does not substitute
for that matrix. Normal Node releases publish the seven platform packages and
the WASM package before the root loader and verify the exact package-group integrity and target tags
before publishing any missing package directly under the selected final tag. Both Node recipes
include math and keep native-to-WASM fallback explicit. Final Web full/render math-inclusive size
evidence belongs to [the 2026-11-01 WASM report](../performance/binding_wasm_budget_2026-11-01.md);
older measurements and parity records retain their dated scope.

For local VS Code VSIX validation:

```bash
python3 scripts/artifact_profile_recipe.py lsp-stdio-release --build-host --locked
python3 scripts/artifact_profile_recipe.py cli-release --build-host --locked
cd tools/vscode-extension
npm ci
npm test
npm run prepare:binaries
target="$(node -p 'process.platform + "-" + process.arch')"
npm run package -- --target "$target" --out "merman-vscode-${target}.vsix"
npm run verify:vsix -- --vsix "merman-vscode-${target}.vsix" --platform "$target" --target "$target"
```

The extension manifest is the authority for the VSIX version. The workspace version identifies the
bundled `merman-lsp` and `merman-cli` runtime for provenance only; it must not rewrite or validate
the extension's independent version. Keep the changelog under `Unreleased` until the first `0.1.0`
Marketplace publication is intentionally prepared.

Before changing Web or Typst artifact profiles, also run the surface-specific
gates:

```bash
npm ci --prefix platforms/web
npm run build --prefix platforms/web
npm run smoke --prefix platforms/web
cargo run -p xtask -- wasm-size-matrix --surface web \
  --web-package-root platforms/web/packages \
  --budget-file docs/release/WASM_SIZE_BUDGETS.json
cargo run -p xtask -- wasm-size-matrix --surface typst --budget-file docs/release/WASM_SIZE_BUDGETS.json
cargo run --locked -p xtask -- verify-typst-profile-constants
python3 scripts/verify_artifact_dependency_closures.py --profile typst-wasm
cargo run --locked -p xtask -- build-typst-package --profile publish
cargo run --locked -p xtask -- typst-package-smoke --profile publish --skip-wasm-build
```

The Web package build uses `wasm-pack --profile wasm-size`, so CI and local release machines need
`wasm-pack` 0.15.0 or newer. `npm run verify:packages --prefix platforms/web` validates each descriptor
package as an independently installable artifact. Release preflight must build/check each admitted
package's matching TypeScript, wasm-bindgen glue, single WASM, provenance, generated binding API,
editor schema,
and the complete descriptor-defined language catalog; it is not valid to publish only a wrapper declaration.
Typst package builds additionally require Binaryen `wasm-opt version 133` and `wasm-tools`; the
post-link optimizer and stripper versions are recorded in the artifact manifest.
`merman-typst-plugin` is the Typst-compatible transport and must remain separate from
browser/wasm-bindgen artifacts. Cargo defaults are intentionally empty. The sole public package
profile is `publish`; it consumes the exact `typst-wasm` artifact recipe, which pins
`default-features = false`, the `svg`, `analysis`,
`layout-cytoscape`, and `layout-elk` features, the `wasm-size` Cargo profile, and the
`wasm32-unknown-unknown` target. Feature bundles are additive closure descriptions, not alternate
release artifacts. The exact dependency gate admits `json5`, `html5ever`, and `url` as measured
pure-Rust dependencies of invariant Mermaid language, configuration, and sanitization semantics.
They remain covered by the exact artifact size budget and final WASM import gate; browser bindings,
randomness, clocks, and other system adapters remain forbidden. Release validation requires Typst
plugin ABI 3, independently from native ABI 3, the closed export surface including `analyze_json`
and `theme_operation_json`, and the descriptor-owned `publish` artifact. Its private directory contains the
stripped WASM and provenance manifest; `--skip-wasm-build` is allowed only because it validates the manifest's
exact artifact profile, package feature bundle, default-feature policy, inputs, tools, versions,
flags, and artifact digest before package
reuse. Do not package the private raw Cargo output under `target/wasm-build/`. The artifact provenance
manifest remains private to `target/typst-wasm-artifacts/`; the package transaction binds the
verified artifact to an in-memory frozen wrapper/license source snapshot and must fail before
replacing the prior version if live source or any staged byte changes.

The crates.io workflow checks the exact workspace plugin's dependency closure, WASM size, version, ABI, capability catalog, and successful/error protocol calls through the package builder, then compiles the real Typst examples with `typst-package-smoke --examples-only`. The independent wrapper's complete suite retains its pinned bundled-engine assertion and runs in its package owner; publishing a workspace plugin patch does not change that wrapper's version mapping.

These commands are Typst owner preflight only. The Cargo crate `merman-typst-plugin` and the Typst Universe package `@preview/merman:0.4.0` are separate publication surfaces; publishing the crate does not publish the wrapper. The 0.4.0 wrapper was published on 2026-10-09, bundling Merman 0.8.0; see the [dated publication record](PUBLISH_ORDER.md#typst-040-publication). The selected next wrapper is 0.5.0, bundling Merman 0.9.0; follow [Typst preparation](TYPST_050_PREPARATION.md). Prepare its final registry import and usage examples before packaging; keep `--package-path` in development guidance. Retain the generated package and private artifact receipt, then verify the registry's exact version after acceptance and record that evidence in `PUBLISH_ORDER.md`.

## Tag And Push

```bash
test -n "$VERSION"
test -n "$SOURCE_SHA"
test "$(git rev-parse HEAD)" = "$SOURCE_SHA"
git tag "v$VERSION" "$SOURCE_SHA"
git push origin "v$VERSION"
```

Do not move or force-update release tags after publication. Release tags are the immutable source
anchor for crates, CLI/LSP artifacts, and platform assets.

`release.yml` creates the primary GitHub Release and uploads CLI and standalone LSP artifacts.
Apple, Android, and Python attach their additional assets to that existing release only when
`publish=true`. Their upload jobs fail if the Release does not exist; retained Actions artifacts
alone do not establish delivery. Flutter's native assets use its separate Flutter-tagged Release.

Asset uploads are intentionally non-destructive: never use `--clobber`, delete an accepted asset,
or replace its bytes. Python, Android, Apple and Flutter's native-asset owners support
byte-verified recovery from the original run artifact; mismatches stop the upload. Rerun only
the original run's failed upload/publication jobs and preserve its successful builders.
Inspect the original run and existing release before recovering.

After the primary release exists, dispatch the selected manual platform owners. Run only the
commands for channels selected in the release scope; this list is a command reference:

```bash
RELEASE_TAG="v<version>"
MAVEN_GROUP="<verified-owned-group>"
gh workflow run release-python.yml \
  -f release_tag="$RELEASE_TAG" -f source_ref="$RELEASE_TAG" \
  -f publish=true -f publish_to_pypi=true
gh workflow run release-android.yml \
  -f release_tag="$RELEASE_TAG" -f source_ref="$RELEASE_TAG" \
  -f maven_group="$MAVEN_GROUP" -f prepare_central_bundle=true -f publish=true
gh workflow run release-web.yml -f release_tag="$RELEASE_TAG" -f source_ref="$RELEASE_TAG" -f publish_to_npm=true
gh workflow run release-node.yml -f release_tag="$RELEASE_TAG" -f source_ref="$SOURCE_SHA" -f publish_to_npm=true
gh workflow run vscode-extension.yml -f source_ref="$RELEASE_TAG"
gh workflow run homebrew.yml
```

For a selected Apple release, normally use the workflow definition from the immutable release tag:

```bash
RELEASE_TAG="v<version>"
gh workflow run release-apple.yml \
  --ref "$RELEASE_TAG" \
  -f release_tag="$RELEASE_TAG" -f source_ref="$RELEASE_TAG" -f publish=true
```

Apple uses a tag-scoped concurrency group shared across workflow branches with
`cancel-in-progress: false`. This serializes active runs for the same tag without cancelling an
in-progress publisher; it does not make archives rebuilt by separate runs byte-identical.

Before any upload, Apple requires a published, non-draft Release, then downloads each existing
same-named asset and uses `cmp` against this run's original artifact. Any difference fails before uploading; matching assets are kept and only
missing items are uploaded. If the upload job fails, rerun that failed upload job within the same
workflow run so it consumes the original build artifact. Do not rerun the build job or dispatch a
new run to retry that upload: repackaging the same source can produce different archive bytes,
which must not be mixed with assets from the original run. Existing assets are never clobbered.

After uploading, the Apple owner anonymously downloads all seven public assets: the binary ZIP,
local SwiftPM ZIP, thin remote SwiftPM ZIP, their three checksum sidecars and the standalone
remote manifest. It compares every downloaded byte with the original upload candidates and
verifies all three ZIP checksums before reporting success. Independently confirm those assets on
the selected Release before marking asset delivery complete. Public SwiftPM repository installation
still requires the separate delivery tree rollout and exact remote consumer evidence.
Authenticated access to a draft Release does not prove public availability. An existing upstream
build artifact does not mean preflight published the XCFramework; existing public assets must
still match the selected source and bytes.

If the SDK surface is selected, publish its already-qualified artifacts separately:

```bash
RELEASE_TAG="v<version>"
PREFLIGHT_RUN_ID="<successful source-matched release-preflight run id>"
gh workflow run release-sdk-publish.yml \
  --ref "$RELEASE_TAG" \
  -f build_run_id="$PREFLIGHT_RUN_ID" \
  -f release_tag="$RELEASE_TAG"
```

This step adds all 18 native C and WASI SDK archives to the existing Release only after checking the
producer run, tag commit, manifests, catalogs, payload hashes, and checksum sidecars. It does not
rebuild the SDK or update the release notes body; after verifying the assets, add a concise download
table for the available profiles and targets to that release's notes.

The VS Code workflow currently packages and verifies platform VSIX artifacts only; Marketplace or
Open VSX publishing requires a separate credential-backed release workflow.

Do not rely on a manual `release-flutter.yml` run for pub.dev publication. A manual run still builds,
freezes target native assets and a thin facade, analyzes, formats, performs the frozen package
dry-run and executes the packaged-app qualifier. Only a pushed `flutter-v<version>` tag enables
native-asset upload and the real pub.dev publish job. For an independent Flutter release, first
validate the intended source, then create the package tag on the
resolved reviewed commit:

```bash
gh workflow run release-flutter.yml \
  -f release_tag="flutter-v<version>" \
  -f source_ref="<reviewed-commit>"
git tag "flutter-v<version>" "<reviewed-commit>"
git push origin "flutter-v<version>"
```

The Flutter tag needs an existing non-draft GitHub Release before native-asset upload; creating
that Release remains an operator step. Retain the frozen build artifacts when that prerequisite
is pending and rerun the failed jobs from the same run once it is satisfied. The protected
`publish-native-assets` job verifies the facade receipt and target manifest from the trusted
workflow revision, keeps byte-identical existing assets, uploads only missing assets, and reads
back every exact pinned target URL/digest without authentication. pub.dev publication depends on
that readback; the facade must never be published while its target downloads are unavailable.

The Flutter build job emits a package archive receipt binding the release commit, tree, version, and
archive digest. The pub.dev job runs the trusted verifier from the workflow revision, enforces
bounded regular-file-only extraction into a new directory with `--require-thin`, and configures
Dart OIDC only after that verification succeeds. It publishes the already-qualified bytes with
`dart pub publish --force --skip-validation` and reconciles the public package members afterward;
it does not rerun package hooks under publication credentials. Do not replace this boundary with
a direct `tar -x` step.

For a workflow-only recovery after a release tag already exists, Python, Android, and Apple may use
the updated workflow definition from `main`, but they still check out and verify the immutable
release-tag commit and tree. For Apple, use `--ref main` only for that workflow-only recovery and
keep `-f release_tag="$RELEASE_TAG"` unchanged; the artifact source always remains the fixed tag.
Also keep `-f source_ref="$RELEASE_TAG" -f publish=true`; Android retains
`prepare_central_bundle=true` and its verified group, and PyPI retains `publish_to_pypi=true` when
that registry is selected.

Credentialed publication workflows that expose `source_ref` require
the matching immutable tag/ref or a reviewed 40-character commit; a mutable `main` ref is valid only
for an explicitly non-publishing build or validation run.

## Follow-On Registry Work

- Roll out the exact Apple thin delivery tree to a separately selected public SwiftPM repository,
  then retain public URL resolution and installed consumer evidence.
- Submit/promote Android's signed standard bundle through Portal under an actually verified
  namespace, then resolve and compare the public coordinate; no direct Central uploader is present.
- Publish the expanded Python/Node matrices and Flutter thin package through their existing owners
  only when selected for a release; retain registry readback and target consumer evidence.
- Extend Flutter packaged-app execution beyond the current macOS host proof to selected device
  targets before claiming their runtime qualification.
