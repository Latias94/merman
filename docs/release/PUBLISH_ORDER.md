# Publish Order

Status: maintained workspace publish order, with explicitly dated historical channel snapshots.
Last updated: 2026-11-11 (Asia/Shanghai)

Use the [versioned upgrade index](README.md#workspace-upgrades) for consumer migrations and [Releasing](RELEASING.md) for current publication and recovery procedures. Snapshot sections below preserve the versions and observations from their stated dates; they are not instructions to repeat those uploads.

## Version Decision

Latest verified maintenance release: `0.9.3` from `release/0.9.x`, covering the workspace Rust crates, CLI/LSP, native C/WASI SDK archives and the Apple XCFramework. After alpha publication, all selected stable Web, Node, Python, Flutter and Android backfills are delivered and independently reconciled below. The Node loader, both transports and all five native members are available at `0.9.3` under `latest`.

The main-line `0.10.0-alpha.2` is published for all selected products, with independently reconciled delivery recorded below. The previous `0.10.0-alpha.1` remains available. The `0.9.x` maintenance source and tags are unchanged.

## 0.10.0 preparation

The maintainer selected stable `0.10.0` for repository preparation on 2026-11-11 (Asia/Shanghai). The pending root and five workspace-coupled package changelogs consolidate the final user experience since the previous stable release, with published alpha entries retained. Release-facing examples target the selected version; the [host guide](PREBUILT_PRODUCTS.md#published-versions) still owns verified registry availability.

Workspace manifests and package-manager locks are projected through `release-version.py` from the root Cargo authority. Tree-sitter `0.3.4` is a separately prepared prerequisite for Class syntax and scanner fixes made after published `0.3.3`; Roughr remains `0.12.6`. Typst's wrapper retains its independent development version and must select a new wrapper release before publication of its new theme and layout behavior.

No publication channels, tag date, immutable candidate source, or reusable release preflight have been selected by this preparation request. Before publication, commit and review the complete candidate, assign the intended date to the root and governed package changelogs, record each channel as selected or deferred, and run the existing immutable `scope=release` preflight for that exact source and version. The historical delivery observations below remain unchanged.

## 0.10.0-alpha.2 publication

Immutable `v0.10.0-alpha.2` identifies `a25f4e366236696bf962fcdd2ac325833ba9cdf0`, tree `f32fa8311213f94d356a1635b35241b1720d39af`. [PR #281](https://github.com/Latias94/merman/pull/281) merged after its [CI](https://github.com/Latias94/merman/actions/runs/42198400701) passed 68 jobs with four intentional skips. The exact-source [main CI](https://github.com/Latias94/merman/actions/runs/42212269595), attempt 2, also passed; its first attempt stalled while downloading fonts before tests. The public [Release](https://github.com/Latias94/merman/releases/tag/v0.10.0-alpha.2) is a prerelease; stable Latest remains `v0.9.3`.

Complete [preflight 42198335121](https://github.com/Latias94/merman/actions/runs/42198335121), attempt 1, passed all 54 jobs from the same source and workflow commit. Immutable receipt artifact `13505348268` has ZIP SHA-256 `91c4fb4b5611d5231499c616957e09becb3664e006e8d83e38398c180ec47658`. Its 18 qualified SDK archives were reused directly. [Pages](https://github.com/Latias94/merman/actions/runs/42212269419) deployed the same source; all 15 public theme-design links opened the intended design and five samples without page errors.

| Selected surface | Independently verified delivery | Publishing owner |
| --- | --- | --- |
| Independent prerequisites | Roughr `0.12.6`, grammar Cargo/npm `0.3.3` and six grammar assets; registry checksums, original bytes and applicable signatures/provenance agree. | [Roughr](https://github.com/Latias94/merman/actions/runs/42212450940), [grammar](https://github.com/Latias94/merman/actions/runs/42212457908), attempt 1 |
| Rust workspace | All 26 non-yanked `0.10.0-alpha.2` crates; prepared receipts, API, sparse index, public archives, source/VCS and registry provenance agree. | [42226260118](https://github.com/Latias94/merman/actions/runs/42226260118), attempt 1 |
| CLI/LSP | All 27 public files match the original publication bundle byte-for-byte; bundle verification and all 27 source/tag-bound attestations pass. | [42226260207](https://github.com/Latias94/merman/actions/runs/42226260207), attempt 1 |
| Native C/WASI SDK | All 18 archives match the original producer bytes and pass manifest, ABI, payload and attestation checks. All 18 parsed sidecars match their archives; three Windows sidecars were normalized from CRLF to LF by the publisher, with no archive difference. | [42233233650](https://github.com/Latias94/merman/actions/runs/42233233650), attempt 1 |
| Web npm | Five exact `0.10.0-alpha.2` tarballs, integrities, registry signatures and attestations match the original group; `alpha` advances and stable `latest=0.9.3` remains. | [42226577020](https://github.com/Latias94/merman/actions/runs/42226577020), attempt 1 |
| Node npm | Nine original tarballs, integrities, registry signatures and attestations verified. Two accepted macOS packages were retained; recovery published only the seven remaining members. Existing stable tags remain unchanged, including ARM64's historical `latest=0.10.0-alpha.1`. | Original [42226585315](https://github.com/Latias94/merman/actions/runs/42226585315), recovery [42242480489](https://github.com/Latias94/merman/actions/runs/42242480489), attempt 1 |
| Python/PyPI | Five `0.10.0a2` wheels match retained artifacts, GitHub downloads and PyPI bytes; five signatures and source/tag/run attestations pass. Four attempt-1 wheels were retained. | [42233085126](https://github.com/Latias94/merman/actions/runs/42233085126), attempt 2 |
| Android | All four public files match the original artifact; complete `io.merman:merman-android:0.10.0-alpha.2` local Maven repository, embedded/standalone AAR equality, checksums and 16 KB alignment pass. Central remains deferred. | [42233079278](https://github.com/Latias94/merman/actions/runs/42233079278), attempt 1 |
| Apple | All seven public files match the original artifact, including three ZIP archives and their checksums. XCFramework, complete local SwiftPM and thin delivery-tree source/manifest identities agree. Extracted-package, simulator and controlled HTTPS consumer qualification passed; a public remote SwiftPM repository remains deferred. | [42247417965](https://github.com/Latias94/merman/actions/runs/42247417965), attempt 2 |
| Flutter | All 59 facade members match the qualified archive, including paths, modes, sizes and bytes; the public archive SHA-256 is identical. All eleven native archives match the source-matched preflight and manifest. The repaired facade uses `09846080864abc97224642c1c7d30122913421e1`; native source and authentication tag retain a25. | [42248563790](https://github.com/Latias94/merman/actions/runs/42248563790), attempt 2 |

Node's two Linux ARM64 trusted-publisher connections had expired before their first OIDC publication. Both were recreated with the same repository/workflow/environment and direct-publish permission, then independently read back as `Valid`. The maintainer explicitly authorized retrying the original GNU tarball after reviewing the retained E404 package-PUT rejection and its bounded historical uncertainty. Recovery workflow `edb67d605f750b54ac8c4c7c1522b0e86d91fc39` reused artifact `13510065594` from the original run and canonical manifest SHA-256 `142c476532cf71bb68c31db8072c584148bea105a75ce74602aeba4fab0a6099`; product source remained a25. The original two macOS attestations retain their original owner, and the seven recovery attestations identify the recovery workflow and run. No accepted tarball or tag was replaced.

Flutter's reviewed facade-only repair raised the `code_assets` floor to 2.2.0 and qualified the facade against the eleven retained native assets without rebuilding them. The maintainer's authenticated Dart CLI published the retained archive using `--from-archive`; pub.dev accepted it at `2026-11-06T16:35:10Z`. Its SHA-256 is `0fbb0f0f1df136fcac51e65851f609a9714326a61abd690335b49cf9f6f2a648`. This was a manual CLI publication, not an OIDC upload. The original publishing owner's failed job was then retried: its pre-upload comparison found the exact accepted package, skipped credential setup and upload, and completed successfully after the final public comparison. Earlier runnerless attempts had sent no package; a failed CLI attempt from an ignored extraction directory selected zero files, with the rejection boundary reviewed against Dart and pub.dev source before the retained archive was sent.

Post-tag recovery changes disable hidden npm upload retries, preserve explicit permission rejections separately from uncertain acceptance, make Apple simulator cleanup diagnostic, normalize future SDK sidecar newlines, migrate Flutter contracts to `code_assets 2.2` and send qualified pub.dev archives directly. These commits do not change the immutable release source. Flutter's repaired facade dependency floor is separately identified above; all native products retain a25. Remote SwiftPM, Maven Central, Typst Universe, Marketplace/Open VSX and stable package-manager updates remain deferred.

Recovery main `af773bacc9e397f5b77a029f71d32c1b822aab59` passed [CI 42251023921](https://github.com/Latias94/merman/actions/runs/42251023921), attempt 2, with 68 successful jobs and four intentional skips. Only the cancelled macOS setup-action check and its aggregate gate were rerun; the other successful results were retained. Later documentation, workflow and renderer changes are reviewed through a separate pull request and require their own source-matched CI.

## 0.10.0-alpha.2 preparation

Selected for preparation on 2026-11-06 after [#279](https://github.com/Latias94/merman/pull/279), starting from main `1fc62283668c6c909b224aa499d8b98a418c1786`. [#280](https://github.com/Latias94/merman/pull/280) merged at `e6733d6ae4daba7ff6d48782eaf0ab63a65b5041`, tree `0fcdd53438a708d79e11726059cda40a1a8c05b5`. The candidate includes the Mermaid 12.3 migration, expanded theme coverage and editable theme gallery, followed by a dependency and build-tool refresh. Its [PR CI](https://github.com/Latias94/merman/actions/runs/42186457151) passed 68 jobs and four conditional skips at tested merge `653339e24d419a588c18678466629e49fc13416b`; the merged main tree is identical. Main's own CI and the final source's immutable release preflight remain distinct evidence.

The [alpha.2 changelog](../../CHANGELOG.md#0100-alpha2---2026-11-06) and five coupled platform projections use the intended tag date `2026-11-06`. Package READMEs describe target-version use; the host guide retains independently verified available downloads until each publishing owner succeeds. At the readiness-review boundary, no alpha.2 release tag, registry upload or GitHub Release had been created. Subsequent publication is recorded above.

The reference graph advances ELK to `0.12.1` and esbuild to `0.28.2`, with compatible D3 patches and Pie path behavior updated together. The [dependency policy](../DEPENDENCY_POLICY.md) records retained renderer, TypeScript and Mermaid companion contracts. Root Rust tooling is `1.99.1` with library MSRV `1.95`; host JavaScript tooling selects Node `26.13.0` and npm `12.2.2`. Independent `roughr-merman 0.12.6` and Tree-sitter Mermaid `0.3.3` remain prerequisites whose owners must publish and verify registry visibility before coupled workspace dependents.

The fresh-resolution admission owner passes the alpha.2 SVG/ASCII typed-family consumer and the previous alpha.1 facade. Alpha.1's exact sibling requirements select published alpha.1 packages, with no alpha.2 sibling patch used in that previous lane; mixed old/new compatibility is explicitly **not verified**. The independent-version guard reports Roughr `0.12.5 → 0.12.6` and grammar `0.3.2 → 0.3.3` against `v0.10.0-alpha.1`. Cargo metadata projects 26 coupled publishable crates. Prior PR verification and these local checks do not replace source-matched `scope=release` preflight.

### Alpha.2 publication scope

On 2026-11-06, after the readiness review, the maintainer authorized merging #281 once CI passes and publishing alpha.2. The selected scope is the complete alpha.1 product set plus its two independent prerequisites. Independent external channels remain deferred. The immutable candidate is `a25f4e366236696bf962fcdd2ac325833ba9cdf0`, tree `f32fa8311213f94d356a1635b35241b1720d39af`; [release preflight 42198335121](https://github.com/Latias94/merman/actions/runs/42198335121) uses that same source and workflow HEAD. Publication still requires successful qualification and each owner result.

| Delivery surface | Selected publication scope | Public version observed on 2026-11-06 |
| --- | --- | --- |
| Rust workspace, 26 crates | selected | `0.10.0-alpha.1`; ordinary OIDC owners retain the three names bootstrapped for alpha.1 |
| CLI/LSP | selected | `v0.10.0-alpha.1` prerelease archives; stable Latest remains `v0.9.3` |
| Native C/WASI SDK | selected | alpha.1's 18 archives and 18 checksum sidecars |
| Web npm, five members | selected | exact `0.10.0-alpha.1` under `alpha`; stable `latest=0.9.3` |
| Node npm, nine members | selected | exact `0.10.0-alpha.1` under `alpha`; existing seven names retain stable `latest=0.9.3`, and both ARM64 names have `latest=alpha=0.10.0-alpha.1` |
| Python / PyPI | selected | five `0.10.0a1` wheels; stable `0.9.3` |
| Flutter / pub.dev and native assets | selected | `0.10.0-alpha.1` and its original native asset URLs; stable `0.9.3` |
| Apple local SwiftPM / XCFramework | selected | seven alpha.1 assets on the primary workspace Release |
| Android AAR / local Maven ZIP | selected, group `io.merman`, no Central bundle | `io.merman:merman-android:0.10.0-alpha.1` as GitHub-hosted local-repository assets |
| Roughr and Tree-sitter independent releases | selected before coupled workspace uploads | Roughr `0.12.5`; grammar Cargo/npm/GitHub `0.3.2` |
| VS Code Marketplace, Typst wrapper, stable package managers, remote SwiftPM and Maven Central | deferred | Not selected by this readiness request; no new availability claim is made for these independent channels |

Read-only observations at `2026-11-06T09:23–09:30Z` query all 26 Cargo names through the [API](https://crates.io/api/v1/crates/merman) and sparse index, all 14 [Web/Node npm members](https://registry.npmjs.org/@mermanjs%2fweb), [PyPI](https://pypi.org/pypi/merman/json), [pub.dev](https://pub.dev/api/packages/merman), and GitHub tags and Releases. No exact alpha.2 package, workspace/Flutter tag or Release is visible. The current alpha.1 Cargo API/index checksums agree for every member; GitHub metadata lists its 27 CLI/LSP assets, 36 SDK files, five wheels, seven Apple files and four Android files. This readiness observation does not re-download or re-attest historical asset bytes. All npm names already exist, so no new-name bootstrap is indicated. Existing GitHub release environments are present, but live Trusted Publisher exchange remains untested until each publishing owner runs.

Both independent target-version API requests return 404, while their complete official sparse-index entries return 200 without the target version: [Roughr](https://index.crates.io/ro/ug/roughr-merman) remains at non-yanked `0.12.5`, and [Tree-sitter Mermaid](https://index.crates.io/tr/ee/tree-sitter-mermaid) at non-yanked `0.3.2`. Grammar npm, tag and Release are also `0.3.2`. Their existing API/index checksums agree with the alpha.1 independent receipts below. Stable package-manager, Marketplace, Typst Universe, Maven Central and remote SwiftPM availability was not reverified by this readiness review; those independently versioned or external channels remain deferred.

### Alpha.2 owner sequence

1. Freeze the final dated/documented candidate and dispatch `release-preflight.yml` with `scope=release`, its full source SHA and a workflow ref pointing to that same SHA. A successful run is mandatory before the workspace tag; the identical run HEAD also makes its 18 SDK archives eligible for later publication.
2. After CI and merge, run the independent Roughr owner and the tagged Tree-sitter owner. Verify exact Cargo API and sparse-index checksums, grammar npm integrity/provenance and selected standalone grammar assets before coupled workspace uploads.
3. Push immutable `v0.10.0-alpha.2` only after successful preflight and independent prerequisites. Its tag triggers Cargo and CLI/LSP. Web and Node publish their complete frozen groups under `alpha`; preserve existing stable tags.
4. Wait for the public, non-draft primary Release before the selected SDK, Apple, Android and Python upload owners. The SDK publisher consumes the successful source-matched producer run without rebuilding. Android uses explicit group `io.merman` with `prepare_central_bundle=false`; Central signing and submission remain separately deferred.
5. For selected Flutter publication, push `flutter-v0.10.0-alpha.2` at the same immutable workspace source only after the primary Release exists. This tag provides pub.dev authentication; all eleven native assets attach to `v0.10.0-alpha.2`, with no separate Flutter Release. Keep the existing publisher pattern and original alpha.1 URLs. The normal tag trigger does not require enabling manual-dispatch recovery.
6. Reconcile every selected registry or public download against its retained producer bytes and provenance. Recover only through the original artifact and accepted-attempt history; do not move tags, overwrite assets, or rebuild a partially accepted npm/Flutter group.

The original main CI failure in [42166940890](https://github.com/Latias94/merman/actions/runs/42166940890) was a missing dependency prefetch before Typst offline metadata. #280 adds the locked fetch, preserves offline verification, normalizes native color channels at full precision, explicitly exports the closed Typst linker metadata and narrowly admits JNI macro deprecation on Rust 1.99. The final PR's Typst package owner passes its unchanged final budgets and closed ABI, with 23 consumer fixtures and nine expected failures. Main's push-only compatibility lane in [42194317888](https://github.com/Latias94/merman/actions/runs/42194317888) then exposed that platform-filtered fetch does not cover Cargo metadata's cross-platform manifest reads. The alpha.2 candidate uses unrestricted `cargo fetch --locked` in all three affected Typst owners. A separate-cache reproduction fails before this change and passes the unchanged offline closure verifier afterward; final-source CI and release preflight remain required.

## 0.10.0-alpha.1 publication

Published and reconciled on 2026-11-03 (Asia/Shanghai), with the prepared 2026-11-02 changelog date. Immutable `v0.10.0-alpha.1` identifies `808f4c406e3453b0fa492b98cf7075a539811f1f`, tree `1c3ede040366d16672a7678cc2f5f28148e1b024`. Main was fast-forwarded to that exact reviewed commit after [CI 41453231279](https://github.com/Latias94/merman/actions/runs/41453231279) passed 68 checks with four intentional skips. The [Release](https://github.com/Latias94/merman/releases/tag/v0.10.0-alpha.1) is a public prerelease; stable Latest remains `v0.9.3`.

Complete [preflight 41453348164](https://github.com/Latias94/merman/actions/runs/41453348164), attempt 2, passed all 54 jobs, including the 765-check strict feature matrix, every platform owner, all 18 SDK recipes and three native/WASI/CLI output comparisons. Its source, tree, version, workflow/run HEAD and immutable receipt match the tag. Receipt artifact `13163628119` has digest `sha256:a8e969fbdf719076e555cfa2c48a4b4764f99c70a16d2de7425990aeeb059008`. Attempt 1's macOS ARM64 math SDK never received a hosted runner; only that unstarted job and its dependents were rerun, retaining successful artifacts.

| Selected surface | Independently verified delivery | Publishing owner |
| --- | --- | --- |
| Rust workspace | All 26 crates at `0.10.0-alpha.1`, across eight complete dependency batches. Prepared receipts, non-yanked sparse index, API checksums, anonymous archive bytes and packaged Git source agree for every crate. The 23 ordinary uploads carry registry OIDC metadata for the same source/run. | [41491730200](https://github.com/Latias94/merman/actions/runs/41491730200), attempt 1 |
| CLI/LSP | Five targets, installers, checksums, verification manifest and source-bound native preset catalog: 27 public assets. Every public byte matches the original publication bundle; the bundle verifier and signed 27-subject provenance pass source/tag/signer checks. | [41491729641](https://github.com/Latias94/merman/actions/runs/41491729641), attempt 2; 21 successful jobs |
| Native C/WASI SDK | All 18 archives and 18 SHA-256 sidecars are public. Anonymous archive bytes match the original producer artifacts; manifests, catalogs, ABI, licenses and payload hashes pass verification. Parsed public sidecars match each archive. One verified source/tag/signer attestation covers all 18 archive digests. | [41501709585](https://github.com/Latias94/merman/actions/runs/41501709585), producer preflight above |
| Web npm | All five packages at `0.10.0-alpha.1` under `alpha`. Integrities, public tarballs and registry provenance match the original frozen group from artifact `13166603161`; artifact digest `sha256:33088b6f1431b427d2bf7782a55759436b2c05a27e57d7da814742eed1a5a565`. A fresh registry consumer verifies five registry signatures and five attestations. `latest=0.9.2` and all other tags were retained at publication. | [41491775050](https://github.com/Latias94/merman/actions/runs/41491775050), attempt 1 |

The explicit [bootstrap 41491262832](https://github.com/Latias94/merman/actions/runs/41491262832) first published three names using the configured Actions environment token. Their original `.crate` bytes match the public downloads, API and sparse index. They have no OIDC attestation; the normal publisher retained matching accepted bytes. On 2026-11-03, the maintainer confirmed that Trusted Publishing is configured for all three names using repository `Latias94/merman`, workflow `release-crates.yml` and environment `crates.io`. The next new-version upload will exercise those OIDC bindings; no accepted alpha artifact is republished to test them.

| First-published crate | Accepted SHA-256 |
| --- | --- |
| `merman-model` | `65172bab3293960ea736a4b45a09eda4b1ea9606649326397ced0210ff36e535` |
| `merman-theme-contract` | `13d2fc4432707a688f763950731fc5a6a412f6181dbb69424b41e2ea8cc8a5c4` |
| `merman-markup` | `f5f69fccc337fb824ee7bb628f6f300e59a2796a10a9dad94550640ad1eb7348` |

The CLI run's first macOS ARM64 builder also never received a hosted runner. Its single-job rerun retained the other four targets and passed all dependent native verification, attestation and upload. Neither recovery moved the tag or changed accepted package bytes. The public Release notes present the theme gallery, migration examples and installation-guide links. Delivery scope, deferred bindings and artifact receipts are recorded here. After release qualification, the previously paused Dependabot CI `41415708144` was resumed.

Node, Python, Flutter and Android retained public `0.9.2` at the alpha completion boundary; Apple retained legacy `0.9.3`. Stable package managers, VS Code Marketplace, new SwiftPM repository rollout and Maven Central were outside this alpha. The newly authorized stable binding backfill below is a separate operation from the immutable `v0.9.3` source; it does not expand the alpha's delivered channels.

## 0.10.0-alpha.1 platform backfill

On 2026-11-03, the maintainer explicitly requested completing the remaining alpha platform bindings. The original platform builds retain immutable native source `808f4c406e3453b0fa492b98cf7075a539811f1f`, tree `1c3ede040366d16672a7678cc2f5f28148e1b024`, and the successful complete preflight above. The unaccepted Flutter facade has a separate layout-repair source identity recorded below; that repair does not change its original native assets or reuse preflight as proof of the corrected package. Published crates, CLI/LSP, Web and C/WASI SDK bytes are unchanged. Stable `0.9.3` tags and registry tags remain independent.

| Selected platform | Current preparation or delivery | Owner |
| --- | --- | --- |
| Apple | Delivered: all seven XCFramework/local SwiftPM/remote-tree assets. Anonymous bytes, three checksum sidecars and all five native target slices match the original artifact. macOS execution, iOS device compile/link and Simulator runtime passed in the owner; a separate local SwiftPM consumer against public bytes also passed. Public Git-based SwiftPM installation still requires a separately selected delivery repository and exact remote consumer proof. | [41563756795](https://github.com/Latias94/merman/actions/runs/41563756795), attempt 1; immutable alpha workflow/source |
| Python | Delivered: all five wheels on GitHub Release and PyPI as `0.10.0a1`; GNU Linux uses glibc 2.28. Anonymous bytes and SHA-256 values match the retained producing artifacts. `pypi-attestations 0.0.30` verifies all five source/tag/run certificates and signatures. A fresh macOS ARM64 consumer installs from PyPI and passes formula, policy, missing-codec and theme-authoring checks, including 27 support vectors. | [41563699231](https://github.com/Latias94/merman/actions/runs/41563699231), attempt 1; immutable alpha workflow/source |
| Flutter | Delivered: all 11 unchanged target-native assets and the corrected `merman 0.10.0-alpha.1` facade on pub.dev. Public native bytes, library hashes and symbols match the original producer; facade member reconciliation returns `exact`. A fresh official-registry Dart 3.12.2 macOS ARM64 consumer passes runtime and theme-authoring checks. | Native producer [41564915086](https://github.com/Latias94/merman/actions/runs/41564915086), attempt 1; repaired facade `51a57c3796d0a97f38b5bc7d9dec01ad532adc3d`, maintainer-authenticated CLI publication; original tag unchanged |
| Node | Delivered: all nine packages match the original group bytes and integrities. Both native macOS ARM64 and explicit Node-WASM registry consumers pass the frozen-release theme, math, Iconify and cross-process recipe tests. Official npm audit verifies nine registry signatures and seven attestations; the two manual ARM64 bootstraps have no attestation. Existing seven names retain stable `latest=0.9.3`. | [41566121071](https://github.com/Latias94/merman/actions/runs/41566121071), attempt 2; workflow revision `d213bf907dd732c09f919927183423b1bb6d4028` |
| Android | Delivered: qualified AAR and complete local Maven repository ZIP with SHA-256 sidecars. Public bytes match the original workflow artifact; the external Gradle consumer and 16 KB device qualification passed. The coordinate is `io.merman:merman-android:0.10.0-alpha.1`; this is GitHub-hosted local-repository delivery, not Maven Central publication or a namespace-ownership claim. Central signing/namespace remain unconfigured. | [41566148029](https://github.com/Latias94/merman/actions/runs/41566148029), attempt 1; workflow revision `d213bf907dd732c09f919927183423b1bb6d4028` |

The three first-published Rust crate names now have maintainer-confirmed Trusted Publishing configuration. Their original bootstrap provenance boundary remains recorded above; configuration does not retroactively add an attestation or justify resending accepted versions.

Node's original group is artifact `13201415983`, ZIP digest `sha256:53c67ea5a73c9569f76073d32d08dacfdea2dacec9548812262d20e38dc6e4e6`, canonical manifest SHA-256 `98676f4e348953ef18a4e88ecab416aedb2c2ed655c32ef76dba9339ebdc686f`. Attempt 1 stopped at the new-name preflight with its upload step skipped; the existing attempt-history checker returned `never-attempted`. Maintainer-authenticated first publication used only its original two Linux ARM64 tarballs, then only the same failed publisher was rerun. The final report is `released`, with seven uploads and two matching accepted versions. All seven registry attestations identify workflow revision `d213bf907dd732c09f919927183423b1bb6d4028`, the branch above and attempt 2; product source remains `808f4c406e3453b0fa492b98cf7075a539811f1f`. Official npm readback confirms both new Trusted Publishers use repository `Latias94/merman`, workflow `release-node.yml` and environment `npm`. npm created a `0.0.0-stage` placeholder for each new name and assigned both `alpha` and `latest` to its accepted alpha; those observed tags were retained. Every other existing tag is unchanged.

The complete Apple local SwiftPM archive is 105,678,740 bytes, SHA-256 `5049471349d759a93e2d05a06eafe5d262f3b1715919deb7bbc78c742a6000e7`. The binary-only XCFramework ZIP is 105,644,332 bytes, SHA-256 `f8d3989b216f4d98e9ea4bf8cf45dc5e98253053427490726f7472dd73c5271c`. Android's AAR is 24,098,096 bytes, SHA-256 `cd3987b0525c1bacb44f368b22a21389dd09ac790a4c84ac73e05eb651716527`; the complete Maven ZIP is 25,407,293 bytes, SHA-256 `b75aa9215d03b851001bd3457248d095cf36495ca08d452e4e6f8a9d4c518850`. These identities describe GitHub downloads, not public SwiftPM Git resolution or Maven Central.

Flutter recovery preserves the original tag, native files and manifest. The original publisher's server validation rejected `hook/native_assets.dart` before accepting this version. The fix moves that helper under `lib/src` and changes only two imports, with a server-aligned filename check added before future publication. The corrected facade comes from commit `51a57c3796d0a97f38b5bc7d9dec01ad532adc3d`, tree `8d9c8652b48e6869d42c15e6febc7d34820702f0`; its deterministic safety archive SHA-256 is `aa0af57ca882ba587c7cddf7d0f5a3d6d256fa39620e6baf20c29474a2624897`. All 57 source-owned files match that commit; the original native manifest remains byte-identical, SHA-256 `f7816dc7726a0fd7b0c1a8e435944703a2862756b7f00cce9dc2ef76eb7d521d`, and retains native source `808f4c406e3453b0fa492b98cf7075a539811f1f`. Existing archive and registry comparators are unchanged. Eighty focused tests, Dart analysis, native-hook contracts, official pub.dev dry-run (zero warnings, about 133 KB) and separate candidate consumers passed before sending.

The maintainer-authenticated Dart CLI uploaded the corrected facade once. This was not an OIDC upload; the existing package's Trusted Publisher remains unchanged for normal tag-triggered releases. The public archive is 136,867 bytes, SHA-256 `46af8dd4af27f380f68103debe2ec4d7e1dba087828e4cffc705529f47cb4386`. Maintained reconciliation returns `exact` for all 58 paths, executable bits, sizes and contents (772,460 uncompressed bytes). A clean consumer installed that exact version from official pub.dev, verified all 58 installed files, and ran the unchanged frozen-release runtime and theme smoke on Dart 3.12.2/macOS ARM64: formulas, callbacks, ASCII, analysis and expected missing codecs passed, alongside two theme materializations, 27 support vectors, five invalid-theme errors and three budgeted operations per consumer. Its one target-library download matches the original manifest. A transient TLS interruption needed one read-only consumer retry, with no additional upload. The original Flutter 3.47.7 relocated-app receipt still qualifies only the original facade; the corrected package has separate native Dart registry-consumer proof. Other targets retain build, selection, byte and symbol evidence rather than new runtime claims.

The workspace alpha Release now contains 79 assets; the separate Flutter prerelease has 11 native assets. The stable Release retains 68 assets and remains Latest. All selected alpha platform backfills are complete. Public SwiftPM Git-repository installation, Maven Central, stable package-manager updates and VS Code Marketplace remain outside this operation.

## 0.10.0-alpha.1 preparation

Preparation starts from `cb3642b6becef62ca288748013b6ef6c457fa781`, the PR #267 merge. The reviewed PR head passed 68 checks with four intentional skips; the merge's own CI and final candidate qualification retain separate source identities. Freeze the final version-prepared commit and run complete `scope=release` preflight from that same workflow/source ref. Previous PR and binding-candidate results do not admit this later source, and Actions artifacts are not public downloads.

The root and package changelogs describe the net user delta, with the Python version projected as `0.10.0a1`. Cargo sibling requirements are exact `=0.10.0-alpha.1`; npm groups use the `alpha` tag, leaving stable `latest` unchanged. Roughr, Tree-sitter, Typst wrapper and VS Code versions retain their independent axes. Package READMEs target the candidate API, while the host guide continues to record actual public versions.

The initial candidate preflight exposed two independent production changes that still used published versions. Prepare `roughr-merman 0.12.5` for the SVG arc-angle rounding fix and `tree-sitter-mermaid 0.3.2` for its raised `cc` build-dependency minimum. The workspace and LSP minimum requirements follow those candidates. These patches preserve the Roughr public API and the grammar's syntax, query, language ABI and runtime contracts; their owner compatibility, generated-artifact and package checks remain required. Publish and verify both independent Cargo prerequisites before the coupled workspace graph. This preparation does not create tags or upload either package, and grammar npm/GitHub delivery retains its own owner.

### Observed channel prerequisites

Read-only observations on 2026-11-02 (Asia/Shanghai) found no exact alpha.1 versions in the coupled crates.io, npm, PyPI or pub.dev channels and no workspace/Flutter alpha tag or Release. These observations precede any publication authorization. The table records preparation and recommended sequencing; it does not select upload scopes on the maintainer's behalf.

| Channel | Existing public version | Candidate prerequisite and next step |
| --- | --- | --- |
| Rust workspace | 23 existing names at `0.9.3`; three names have no published crate | `merman-model`, `merman-markup` and `merman-theme-contract` return 404 from both crates.io API and sparse index. After final preflight and authorization, bootstrap their actual alpha packages in the publisher's dependency order with a maintainer API token, reconcile accepted checksums, and configure each Trusted Publisher. OIDC alone cannot create these names. |
| CLI/LSP and native C/WASI archives | `0.9.3` Release assets | Qualify the frozen candidate before tagging. SDK publication additionally uses its exact source-matched producer run after the primary Release exists. |
| Web npm | All five members: `latest=0.9.2`, `alpha=0.8.0-alpha.7` | Existing names need the normal publisher check and frozen five-member group publication; there is no observed new-name bootstrap requirement. |
| Node npm | Seven existing members: `latest=0.9.2`, `alpha=0.8.0-alpha.7` | The two Linux ARM64 names return 404. The `npm` environment has no approval hold. Choose a documented same-artifact bootstrap route before selecting the complete nine-member publication; do not create only two names and then rebuild the remaining group. |
| Python / PyPI | `0.9.2`; latest prerelease `0.8.0a7` | Existing project; verify its publisher at upload time and publish the five frozen wheels through the owner. No new project bootstrap is inferred from a version change. |
| Flutter / pub.dev | `0.9.2`; latest prerelease `0.8.0-alpha.7` | Existing package; use its exact `flutter-v0.10.0-alpha.1` tag/Release, verify public native assets before publishing the same frozen facade. |
| Apple local SwiftPM / XCFramework | Legacy `0.9.3` binary ZIP and checksum | The complete local/remote handoff assets can publish through the Apple owner after the primary Release exists. Public Git-based SwiftPM installation separately requires a selected and qualified delivery repository; none was established by this review. |
| Android | `0.9.2` AAR Release asset | The formal asset owner requires a verified Maven namespace and signed Central handoff. The `maven-central` environment is absent; resolve that channel setup or explicitly defer Android. Central submission/promotion remains separate. |
| Independent Rust prerequisites | Roughr `0.12.4`; Tree-sitter `0.3.1` | Prepare and verify Roughr `0.12.5` and grammar `0.3.2`. Their independent publishing owners must complete before the coupled workspace graph can resolve from crates.io. No upload is selected by preparation. |
| Other independent packages, stable package managers and VS Code Marketplace | Existing independent release records below | No other independent release, stable-registry update or Marketplace publication is selected by this preparation. |

Registry observations used the [crates.io API](https://crates.io/api/v1/crates/merman), the new names' Cargo sparse-index entries, all 14 [Web/Node npm members](https://registry.npmjs.org/@mermanjs%2fweb), [PyPI](https://pypi.org/pypi/merman/json), [pub.dev](https://pub.dev/api/packages/merman), the GitHub tag/Release APIs and authenticated read-only environment metadata. Environment existence does not prove registry permission or a successful future token exchange. The release operator retains detailed observation responses outside the source tree; no credential values were queried.

### Authorized publication scope

On 2026-11-02, the maintainer requested publication after merging [PR #269](https://github.com/Latias94/merman/pull/269). Independent read-only observations completed at `2026-11-02T08:48:16Z`; selected alpha packages, the workspace tag and Release were absent. Final qualification must use the source containing the full-bindings preflight recipe repair. The earlier `41402238880` run passed its 765 strict feature checks but failed eleven binding tests because its full-library command enabled only Flowchart. Enabling `svg,all-diagrams` passes all 295 binding-library tests; no product behavior or test assertions are relaxed.

| Surface | Scope | Verified state before publication |
| --- | --- | --- |
| Workspace Rust, all 26 crates | selected, pending | 23 names have newest public `0.9.3`; `merman-model`, `merman-markup` and `merman-theme-contract` are absent from both API and sparse index. Bootstrap the new names in dependency order through the explicit Actions token owner, preserve their accepted checksums, and use the normal owner for the complete graph. The maintainer will configure their Trusted Publishers after publication. |
| CLI/LSP | selected, pending | Latest workspace Release is `v0.9.3`; final alpha delivery belongs to the tag-triggered owner. |
| Native C/WASI SDK | selected, pending | Public archives remain `0.9.3`. Publish all 18 qualified archives and 18 checksum sidecars from the successful source-matched preflight after the primary Release exists. |
| Web npm, all five members | selected, pending | Every member has `latest=0.9.2`, `alpha=0.8.0-alpha.7`. Publish one frozen group under `alpha`, retaining stable `latest`. |
| Node npm, all nine members | deferred | Seven members have `latest=0.9.2`, `alpha=0.8.0-alpha.7`; both Linux ARM64 names return 404. No Node bootstrap or publication is selected. |
| Python / PyPI | deferred | `0.9.2`; newest non-yanked prerelease `0.8.0a7`. |
| Flutter / pub.dev and native assets | deferred | `0.9.2`; newest prerelease `0.8.0-alpha.7`. |
| Apple | deferred | Legacy `Merman.xcframework-v0.9.3.zip` and checksum on [v0.9.3](https://github.com/Latias94/merman/releases/tag/v0.9.3). Alpha SwiftPM/XCFramework and public remote repository rollout are not selected. |
| Android | deferred | `merman-android-v0.9.2.aar` on [v0.9.2](https://github.com/Latias94/merman/releases/tag/v0.9.2); this does not establish Maven Central delivery. |
| Stable package managers | deferred | Homebrew, Scoop and Conda-forge report `0.9.3`; the initial-package WinGet and Nixpkgs `0.9.3` PRs remain open and their checked official package paths are absent. |
| VS Code Marketplace | deferred | Official exact-name query for `latias94.merman-vscode` returns no public match. Source version `0.1.0` is independent and does not establish Marketplace availability. |

The independent prerequisite releases `roughr-merman 0.12.5` and `tree-sitter-mermaid 0.3.2` are selected through their owners, including the grammar's npm package and GitHub assets. Before publication, both candidates are absent from the Cargo API/index; grammar npm and GitHub remain at `0.3.1`. Their compatibility and package checks precede upload, and both Cargo releases must resolve before the coupled workspace dependents publish. No Typst wrapper, Pages deployment, container registry, setup-Action Marketplace rollout or other independent release is selected.

These observations use the official registries linked above, the [GitHub Releases API](https://api.github.com/repos/Latias94/merman/releases), [Homebrew](https://formulae.brew.sh/api/formula/merman-cli.json), [Scoop](https://github.com/ScoopInstaller/Main/blob/master/bucket/merman-cli.json), [Conda-forge](https://api.anaconda.org/package/conda-forge/merman-cli), existing [WinGet](https://github.com/microsoft/winget-pkgs/pull/447353) and [Nixpkgs](https://github.com/NixOS/nixpkgs/pull/570822) submissions, and the official Marketplace exact-name query. Environment inspection confirms `CARGO_REGISTRY_TOKEN` exists in the `crates.io` environment; ordinary owners use OIDC and do not read it. A read-only authentication check rejected the local Cargo credential, so initial new-name uploads select the explicit Actions bootstrap owner and its existing environment secret. Configure the three bindings afterwards without changing accepted versions or bytes. Preserve immutable tags and original artifacts throughout recovery.

### Independent delivery and workflow recovery

The initial frozen product source was `b5bf7a3b98c74a59a43422413a67aeb4bd2c1b99`, tree `625f9299f4b9a3a8c1be49af5708595a9aa56642`, merged by fast-forward in [PR #270](https://github.com/Latias94/merman/pull/270). Its [CI 41417162086](https://github.com/Latias94/merman/actions/runs/41417162086), attempt 2, passed 68 checks with four intentional skips. Attempt 1 could not acquire a macOS ARM64 hosted runner; its unstarted job was cancelled and the strict gate rejected the resulting `abandoned` owner. Only that platform and its dependent gate were rerun; the accepted source was unchanged.

| Independent owner | Verified publication from the frozen product source |
| --- | --- |
| [Roughr 41417319456](https://github.com/Latias94/merman/actions/runs/41417319456) | `roughr-merman 0.12.5`; Cargo API, non-yanked sparse index, public download and source re-packaging agree on SHA-256 `4d2809c584f2a9a35ad3148aae1d3bb2ece00baf289b894b32029b33fb47c13a`. Candidate and published-dependent compatibility lanes passed. |
| [Grammar 41417347759](https://github.com/Latias94/merman/actions/runs/41417347759) | `tree-sitter-mermaid 0.3.2` on crates.io and `@mermanjs/tree-sitter-mermaid 0.3.2` on npm under `latest`; six public [GitHub assets](https://github.com/Latias94/merman/releases/tag/tree-sitter-mermaid-v0.3.2) match the original producer artifact. Cargo API/index/download SHA-256 is `bd26183e90e60d086d81e3644d9c6c311e0be67a97a4007c186be366e353c4ad`. npm integrity, downloaded bytes, registry signatures and provenance agree with that same run/source; its visibility delay resolved without another upload. |

The complete [preflight 41418063813](https://github.com/Latias94/merman/actions/runs/41418063813) passed all 765 strict feature checks and the corrected binding tests, then exposed a second test-recipe omission: the native consumer suite also enabled only Flowchart although it exercises several families. The corrected `all-diagrams` command passes all 71 local tests, without relaxing assertions; explicitly installing Noto CJK matches its Chinese-label fixture and the central theme owner. The remaining preflight commands were separately reviewed for required features and acceptance configuration.

The workflow-only recovery initially selected that same product source, with [SDK qualification 41432173938](https://github.com/Latias94/merman/actions/runs/41432173938) retaining a source-matched run HEAD. Before any coupled alpha upload or tag, the [PR #271](https://github.com/Latias94/merman/pull/271) dependency-policy check then detected the newly admitted [RUSTSEC-2026-0433](https://rustsec.org/advisories/RUSTSEC-2026-0433.html) against `pulldown-cmark 0.13.5`. The candidate minimum and all four governed locks advance to `0.13.6`; explicit Merman Markdown options do not enable the affected Wikilinks path. Refresh the dependency-owned legal projections and current lock-input records without changing performance ceilings or historical measurements.

This dependency update supersedes the unshipped product candidate and its in-progress qualification runs. Freeze the final patched source and run complete `scope=release` preflight with a matching workflow/run HEAD before the workspace tag; the 18 SDK archives will then come from that successful producer. The independently accepted Roughr and grammar packages retain their original source, versions and byte receipts above; their owned production trees are unchanged and the coupled publisher treats them as registry prerequisites. Do not rerun their independent upload owners from the new product SHA. No coupled alpha package, workspace tag or Release was accepted before this preparation update.

The patched candidate `364630c51944e979847ccf9d8724d09c01b878d9`, tree
`1b6a7c5f91a05bfe5015b23984cf3f6eac4109ea`, has [CI 41440858829](https://github.com/Latias94/merman/actions/runs/41440858829).
Its complete [preflight 41441001853](https://github.com/Latias94/merman/actions/runs/41441001853)
passed the strict feature matrix and optimized theme tests, then exposed missing Linux build
dependencies in the all-feature public consumer check: `yeslogic-fontconfig-sys` requires
`libfontconfig1-dev` and `pkg-config`. Install both as the central CI owner already does, retaining
the independent consumer's complete feature set and private-import rejection. The Android job
also failed during emulator unlock with an ADB `Broken pipe` before instrumentation; the same
source passed central CI instrumentation. Every other platform owner passed. This unsuccessful
preflight cannot issue release evidence or qualify its skipped SDK matrix.

The explicit three-crate bootstrap owner and missing font build dependencies are preparation
changes in [PR #272](https://github.com/Latias94/merman/pull/272). Freeze the resulting candidate
and run complete source-matched release preflight again before bootstrap or a workspace tag.
Bootstrap retains the original crate bytes, records upload intent, keeps the environment token
out of packaging and dry runs, and requires both registry API and sparse-index checksums.

## 0.9.3 binding backfill

On 2026-11-03 (Asia/Shanghai), the maintainer requested the remaining stable bindings after alpha delivery. Fresh registry observations found all five Web and seven Node `0.9.3` versions absent, Python and pub.dev at `0.9.2`, and no `merman-android-v0.9.3.aar`. Existing Apple delivery and all 64 stable Release assets were unchanged. Every selected stable package name already exists; no new-name bootstrap is required.

Initial manual owners use workflow ref `v0.9.3` and accepted product source `3b915f69c02d55b873d252cf6c8a6048e7f3427d`, tree `fc57c459736f1857b5bdc6dd148d9bb0ad645e9c`. Flutter's actual publication is triggered by `flutter-v0.9.3` at that same source after manual validation. The original successful preflight `40999149557` remains the source qualification. Stable Node retains seven packages and Python three wheels; main's additional platforms, math capability and delivery redesign are not backported. npm publishes only under `latest`, preserving the accepted Web `alpha=0.10.0-alpha.1` and other tags.

| Selected stable surface | State | Owner |
| --- | --- | --- |
| Web, five packages | All `0.9.3` packages are public under `latest`. Integrities, downloaded tarballs and source/tag/run provenance match original artifact `13172688049` (ZIP digest `sha256:3e7d9be1b754ef5603c38fec5ec153dde800cac9881f55b65d97c635c2344951`). A fresh consumer using the official npm registry verifies all five signatures and attestations. Web `alpha=0.10.0-alpha.1` and every other tag are retained. | [41503526390](https://github.com/Latias94/merman/actions/runs/41503526390), attempt 1 |
| Node, seven packages | All seven `0.9.3` packages are public under `latest`. Anonymous tarballs, integrities, complete tag maps and registry provenance match original group `13173510468`, canonical manifest SHA-256 `3dd94dbe9444c23dc42436cbb18029f37beb2316c9e9039bf7a67e44b286afc8`. The official npm audit verifies seven registry signatures and seven attestations; clean native and Node-WASM consumer smoke passes. After the maintainer's one-send decision, [incident run 41558175355](https://github.com/Latias94/merman/actions/runs/41558175355) sent only the original Windows tarball with its original verified proof. Its provenance retains attempt 2. The original attempt 3 then skipped all five native members and published only WASM and the loader. The final report is `released`; see the [incident record](NODE_093_RECOVERY.md) for retained history and final receipts. | [41503526369](https://github.com/Latias94/merman/actions/runs/41503526369), attempt 3; Windows sender above |
| Python, three wheels | `0.9.3` is available on PyPI and as GitHub Release wheels. All public bytes and SHA-256 values match the retained Linux/Windows attempt-1 artifacts and macOS attempt-2 artifact. `pypi-attestations 0.0.30` verifies each signature; source/tag/run certificate claims match the stable publisher. A clean macOS ARM64 consumer installs from PyPI and passes the stable UniFFI smoke. | [41503525971](https://github.com/Latias94/merman/actions/runs/41503525971), attempt 2 |
| Android AAR | `merman-android-v0.9.3.aar` is public: 16,254,551 bytes, SHA-256 `f26a3205cf3590db3aec3874692988952f521c8fde5b4b78fb72cdc468f154bf`. Anonymous download matches original artifact `13171517108` (ZIP digest `sha256:3515815e71792551ec249b3c48a55f95a54ca38b800d7aa56c05381222f2aba1`). The stable verifier passes both JNI slices, symbols, Kotlin API, resources and legal material. No Maven Central rollout. | [41503527230](https://github.com/Latias94/merman/actions/runs/41503527230), attempt 1 |
| Flutter | `merman 0.9.3` is public on pub.dev from the pushed stable-source `flutter-v0.9.3`. Original artifact `13178865131` has ZIP digest `sha256:e8e6cd57e15177c6ece2231a2ceb938546a7c009a5920fa9ba27d02c74340bb8`; its safety archive SHA-256 is `84c6d82800444982fe621b19eab80c5667dc2a91342fd6c4dd83e3a490e4caa7`. The trusted extractor verifies source/tree/version, and independent pub.dev member reconciliation returns `exact` for paths, executable bits, sizes and bytes. Public archive SHA-256 is `90c1b72c22b41b57fd722cc310cd27a847d448a591489a9346399af28579d24b`. This is the original bundled-native product. | Validation [41503528578](https://github.com/Latias94/merman/actions/runs/41503528578); publication [41514124799](https://github.com/Latias94/merman/actions/runs/41514124799), attempt 1 |

The stable Release has 68 assets after appending the three Python wheels and one Android AAR.
All 64 pre-existing asset IDs, names, sizes and digests remain unchanged; stable Latest is still
`v0.9.3`. The alpha Release retains all 63 assets. Published tags and accepted packages remain
immutable. The completed Node recovery retains its original artifacts, reports, explicit incident
decision and final public reconciliation. Every selected stable backfill is complete.

Python's first macOS ARM64 job never acquired a hosted runner (`runner_id=0`, no steps); GitHub
reported capacity constraints. Only that job and its dependents were rerun. The original Linux
and Windows wheels were retained. The three accepted wheels keep Python `>=3.9` and the original
`macosx_11_0_arm64`, `manylinux_2_35_x86_64` and `win_amd64` tags:

| Stable wheel target | Public SHA-256 |
| --- | --- |
| macOS ARM64 | `bdcec53fcacb2a396ddfaaf5c7e35571576fddc222e3c1d3f314aa09bea672fb` |
| GNU Linux x64 | `f83b97ef9723a5638da0b103d2e2194ef780f578ab6c4b67fabbe971c75e78a9` |
| Windows x64 | `17070e4f48ae2626b09e8cdfd8bad8293d3f25ac4701b149cf56b93ee0f076e5` |

## 0.9.3 publication

Published on 2026-10-30 (Asia/Shanghai) after the maintainer explicitly authorized the workspace Rust crates, CLI/LSP, and native C/WASI SDK surfaces. Immutable `v0.9.3` resolves to `3b915f69c02d55b873d252cf6c8a6048e7f3427d`, tree `fc57c459736f1857b5bdc6dd148d9bb0ad645e9c`, the merge of [PR #254](https://github.com/Latias94/merman/pull/254). Source-matched [preflight 40999149557](https://github.com/Latias94/merman/actions/runs/40999149557), attempt 1, passed all 47 jobs, including 18 SDK archive qualifications and three native/WASI/CLI output comparisons. Its workflow revision, run head, artifact source, and release tag all identify that same commit.

| Surface | Verified delivery | Owner evidence |
| --- | --- | --- |
| Rust workspace | All 23 crates at `0.9.3` across seven complete topological batches. Independent post-publication queries of the crates.io API and non-yanked sparse index match every prepared `.crate` checksum and the source-bound receipts. | [41005974629](https://github.com/Latias94/merman/actions/runs/41005974629), attempt 1 |
| CLI and LSP | All five native targets, installers, checksums, and verification manifest are public: 26 assets. The downloaded bundle passes its owner verifier and source-bound attestation check; downloaded Windows CLI/LSP archives also pass execution smoke locally. | [41005974469](https://github.com/Latias94/merman/actions/runs/41005974469), all 21 jobs successful |
| Native C and WASI SDKs | All 18 standard/math/full archives and 18 checksum sidecars are public. Downloaded archive bytes match the qualified producer; manifests, catalogs, ABI/capabilities, scoped license materials, payload hashes, and publisher attestations pass verification. | [Producer 40999149557](https://github.com/Latias94/merman/actions/runs/40999149557), [publisher 41008921699](https://github.com/Latias94/merman/actions/runs/41008921699), attempt 1 |

The initial public [v0.9.3 Release](https://github.com/Latias94/merman/releases/tag/v0.9.3) contained 62 assets before the Apple backfill below. SDK checksum files use LF after the publisher creates verified snapshots; Windows producer checksum files use CRLF, with identical filenames and archive digests. SDK archive bytes themselves are unchanged. Native SDK execution qualification covers the recorded C/C++ consumers and SVG/PNG controls; the output catalog alone does not establish equal smoke coverage for every codec.

Web/npm, Node/npm, PyPI, pub.dev, Android AAR, Apple XCFramework, Pages, independent packages, and external package-manager submissions were outside this publication scope. Their non-publishing preflight checks do not establish registry availability. The earlier [candidate preflight 40990939540](https://github.com/Latias94/merman/actions/runs/40990939540) had a Flutter setup timeout and is not the publication receipt; the final source-matched preflight above passed Flutter package validation.

The post-publication changelog edit improves wording only. It removes preparation history and repeated protocol details while retaining user-visible changes, migration instructions, and attribution. Published tags, packages, and archive bytes remain immutable; the archived changelogs retain their original wording.

WinGet follow-up on 2026-10-30: the maintainer separately authorized updating the unmerged initial-package [PR #447353](https://github.com/microsoft/winget-pkgs/pull/447353) to `Latias94.MermanCLI 0.9.3`. The submission contains one three-file manifest set, uses schema `1.28.0` as requested upstream, and retains the original published Windows ZIP. Its SHA-256 and release-workflow attestation match the immutable source above. [Registry candidate run 41038101443](https://github.com/Latias94/merman/actions/runs/41038101443), using workflow revision `8c9e65c58d7b540539b8ea4a52af73800c176f9b`, passed WinGet `1.29.380` validation, unattended installation, and installed-alias version/help/SVG smoke tests on Windows 2025. The submitted YAML blobs match that run's validated artifact. Upstream acceptance and catalog availability remain pending.

Nixpkgs follow-up on 2026-10-30: after the maintainer reviewed the prepared patch and PR description, the existing initial-package [PR #570822](https://github.com/NixOS/nixpkgs/pull/570822) was updated to `0.9.3` at `98e793b844161d178a9a34ca243568388a91001d`. The expression pins the immutable release source and a Nix-prefetched NAR hash, with a byte-identical release `Cargo.lock`. It explicitly names the Bash completion to match the upstream installation contract and removes `x86_64-darwin`, which Nixpkgs 26.11 no longer supports. [Review build 41045190405](https://github.com/Latias94/merman/actions/runs/41045190405) passed evaluation, native builds, installed-CLI checks, the version test, and installed SVG/PNG/JPEG/PDF rendering on `x86_64-linux`, `aarch64-linux`, and `aarch64-darwin`, using Nixpkgs `1ef8781d97d9e204bbb213472d2995641065ec72` and the exact submitted package files. This directly builds the Nixpkgs expression; `nixpkgs-review` was not run. Upstream review and acceptance remain pending.

Apple follow-up on 2026-10-31: the maintainer selected the previously deferred Apple channel after [issue #261](https://github.com/Latias94/merman/issues/261). [Apple publisher 41208150766](https://github.com/Latias94/merman/actions/runs/41208150766), attempt 1, completed validation, the five-target XCFramework build, Swift package compilation/runtime smoke, packaging, and upload from immutable `v0.9.3` source `3b915f69c02d55b873d252cf6c8a6048e7f3427d`. The public `Merman.xcframework-v0.9.3.zip` is 300,865,967 bytes with SHA-256 `b5163a4dcfccc0686f260b7d9f5ad73b6365ba8de06e6110c9afbdf43b1c797a`. Anonymous download verification matches the SHA-256 sidecar retrieved from producer artifact `13045749119`, its archive size, the public sidecar bytes, and `swift package compute-checksum`. The archive contains the iOS ARM64, iOS ARM64/x86-64 simulator, and macOS ARM64/x86-64 slices; all generated headers/module maps and 38 legal files match the tag. The Release now has 64 assets. This delivers the existing local SwiftPM/XCFramework route, not a remote `.binaryTarget(url:checksum:)` manifest. Published tag and previously accepted assets are unchanged.

| Coupled channel | Selected scope and observed delivery on 2026-10-31 |
| --- | --- |
| Rust crates, CLI/LSP, native C/WASI SDKs | Initially selected; `0.9.3` available with the original owner evidence above |
| Apple XCFramework | Selected for this backfill; `0.9.3` archive and checksum available and independently verified |
| Web npm group | Deferred; all five public members' [`latest`](https://registry.npmjs.org/@mermanjs/web/latest) versions remain `0.9.2` |
| Node npm group | Deferred; all seven public members' [`latest`](https://registry.npmjs.org/@mermanjs/node/latest) versions remain `0.9.2` |
| Python | Deferred; [PyPI](https://pypi.org/pypi/merman/json) reports `0.9.2` |
| Flutter | Deferred; [pub.dev](https://pub.dev/api/packages/merman) reports `0.9.2` |
| Android AAR | Deferred; `merman-android-v0.9.2.aar` remains the available matching [Release asset](https://github.com/Latias94/merman/releases/tag/v0.9.2) |

## 0.9.2 preparation

Maintenance release `0.9.2` comes from `release/0.9.x`, based on immutable `v0.9.1`. The branch rename preserved merged PR [#235](https://github.com/Latias94/merman/pull/235), commit `ae3299f9a9db517e91c808e63a67cb8d30a1ce37`, whose implementation tree equals its green reviewed head `8d27e9e4f8aa35b7095b45a728f5953538cde438`. Follow-up [#237](https://github.com/Latias94/merman/pull/237) additionally selects parser/CSS repairs from #227, the remaining UTF-8 recovery and Python wheel fixes from #178, the #210 workspace/Typst publisher gate, and the independent incremental-fuzz oracle repair. The compiled-theme development on `main` remains outside this release. See the [backport review](V092_BACKPORT_READINESS.md) for outcome/commit/reference mapping and exclusions. PR #237 passed its own CI before merging; publication uses the merged source and full immutable preflight recorded below.

Maintenance dependency refresh on 2026-10-22: #237 reruns `cargo update` for the workspace, fuzz, and standalone Node locks, with no manifest/API/baseline migration. The ordinary locked workspace dependency policy and both auxiliary all-features advisory checks passed against freshly fetched RustSec revision `4b587f00996ebe5c05a73eaef105ef241e876414`. The two governed font-stack maintenance advisories remain; a supplemental workspace all-features check also reports the optional UniFFI generator's unmaintained `basic-toml` dependency, with no compatible fixed release. The [compatibility decisions](V092_BACKPORT_READINESS.md#verification-boundary) separate release scope from current dependency-policy qualification; do not describe this as an exception-free all-features audit.

## 0.9.2 Publication Snapshot

Recorded on 2026-10-22 (Asia/Shanghai). Immutable workspace tag `v0.9.2` and Flutter tag `flutter-v0.9.2` resolve to commit `8e303a43b548c9e40e73f0bc657925d66e7d4fb5`, tree `a505265dbb785211628c89c0e08d7a12e5156b95`. This maintenance release retains Presentation, Options JSON schema 2, and Mermaid 12.1.0; the compiled-theme migration and Mermaid 12.2.0 development remain on `main`.

[Immutable preflight 39648535313](https://github.com/Latias94/merman/actions/runs/39648535313) passed all 24 jobs for that exact source. Every channel workflow below also runs from the immutable release source. A successful build does not establish registry acceptance; each row records its owning workflow and registry or artifact observation.

| Surface | Observed publication or preparation | Owner evidence |
| --- | --- | --- |
| Rust crates | All 22 coupled crates are published as `0.9.2`. Independent API and sparse-index checksums match the frozen package receipts; all seven topological batches record `complete`. | [39658396320](https://github.com/Latias94/merman/actions/runs/39658396320) |
| CLI / LSP | Public stable [GitHub Release](https://github.com/Latias94/merman/releases/tag/v0.9.2), with five targets per executable, installers, adjacent checksums, `sha256.sum`, and `release-verification.json`. All 25 payload assets match the receipt hashes. | [39658396265](https://github.com/Latias94/merman/actions/runs/39658396265), 21 successful jobs |
| Web npm group | All five `@mermanjs/web*` packages are published at `0.9.2` under `latest`. Registry integrities match the original artifact, all package states are `verified`, and each npm provenance attestation resolves to the frozen source and producing run. Historical alpha/staging tags are retained. | [39704598415](https://github.com/Latias94/merman/actions/runs/39704598415) |
| Node npm group | All seven packages are published at `0.9.2` under `latest`, with exact original artifact integrities and npm provenance. Registry visibility delays required recovery from the original package group and attempt-bound reports. The final observation accepted all seven matching versions without another upload and recorded `released`; the manifest digest remains `fd73e8b637df2478310259f9b48a8de07dab8593520131df0bc58775f54274f5`. | [Original run 39704602361](https://github.com/Latias94/merman/actions/runs/39704602361), [recovery 39707930840, attempt 2](https://github.com/Latias94/merman/actions/runs/39707930840) |
| Python | All three wheels are published on PyPI and attached to the GitHub Release: Linux x64 (`manylinux_2_35`), macOS ARM64, and Windows x64. Registry SHA-256 values match the producing artifacts, and each PyPI publish attestation names the matching wheel digest and the repository's `release-python.yml` publisher. | [39704606089](https://github.com/Latias94/merman/actions/runs/39704606089), 7 successful jobs |
| Flutter | `merman 0.9.2` is published on pub.dev from `flutter-v0.9.2`. The frozen native package receipt binds source, tree, version, and archive SHA-256 `755cbfea3c36e71482207d213918b892f707448e66ea33d7d79b2cc24ab88a48`; registry archive members match the original paths, executable bits, sizes, and bytes. | [39704660780](https://github.com/Latias94/merman/actions/runs/39704660780) |
| Android | `merman-android-v0.9.2.aar` is attached to the GitHub Release and matches its producing artifact (`sha256:e345dc1d21a31720f48c51da6003b1c6edfa3a005d651be7b9fb2e8c78c908f0`). It contains ARM64/x64 native libraries and the projected legal resources inside `classes.jar`. Maven Central publication remains outside this owner. | [39704610093](https://github.com/Latias94/merman/actions/runs/39704610093), 3 successful jobs |
| Apple | `Merman.xcframework-v0.9.2.zip` and its checksum are attached to the GitHub Release. The public archive matches the producing artifact and Swift checksum (`sha256:750fd20ea2a1b6f50e9d06849951bce4f901c3d6756a16dd2de4671a41f27eb5`), including legal materials. Delivery is a GitHub Release asset, not a remote SwiftPM binary-target manifest. | [39704613599](https://github.com/Latias94/merman/actions/runs/39704613599), 3 successful jobs |

VS Code Marketplace, independent Typst/grammar/Roughr releases, Pages deployment, and external package-manager submissions are outside this channel backfill.

## 0.9.1 Publication Snapshot

Published on 2026-10-18 (Asia/Shanghai) from immutable source commit `f6202f702dbde0a75049b1a62b3124feae709b3d`, tree `dc8a157907012ba99a1bbc516070b39148c264e5`. The workspace `v0.9.1` and Flutter `flutter-v0.9.1` tags resolve to that commit. This patch carries selected fixes from the `v0.9.0` line; the theme refactor and Mermaid 12.2.0 development alignment remain outside this release.

[Immutable preflight 39105735891](https://github.com/Latias94/merman/actions/runs/39105735891) passed all 24 jobs for the exact source. Its receipt artifact is `release-preflight-f6202f702dbde0a75049b1a62b3124feae709b3d-attempt-1` (artifact `12128809512`, digest `sha256:438222076c8fa05f8fe5565b6e37450e0c9d753c504ff5c95ffc6af6e91d9fbe`). Workflow success and frozen tags do not assert registry acceptance; channel results below remain separate.

| Surface | Observed publication or preparation | Owner evidence |
| --- | --- | --- |
| Rust workspace | All 22 coupled crates are available at `0.9.1`; the release run uploaded 20 and confirmed the two previously accepted crates. Every crates.io API checksum matches its prepared package, and each receipt checksum matches the sparse index. | [Successful publish 39121469824](https://github.com/Latias94/merman/actions/runs/39121469824), [receipt artifact 12136465487](https://github.com/Latias94/merman/actions/runs/39121469824/artifacts/12136465487), [initial Typst smoke 39109052057](https://github.com/Latias94/merman/actions/runs/39109052057), [partial publish 39111148178](https://github.com/Latias94/merman/actions/runs/39111148178), [recovery 39116306833](https://github.com/Latias94/merman/actions/runs/39116306833), [tag smoke retry 39120577158](https://github.com/Latias94/merman/actions/runs/39120577158), [PR #210](https://github.com/Latias94/merman/pull/210) |
| CLI / LSP archives | [v0.9.1](https://github.com/Latias94/merman/releases/tag/v0.9.1) is the public stable Latest workspace release. All 21 jobs passed; the 26 published assets across five targets passed a separate complete-bundle size/SHA256/source verification after download. | [39109052185](https://github.com/Latias94/merman/actions/runs/39109052185) |
| Web npm group | All five packages are published at `0.9.1` under `latest`. A separate observation matched all registry integrities to the original artifact; each npm provenance attestation resolves to the frozen source. Historical alpha/staging tags remain unchanged. | [39109254830](https://github.com/Latias94/merman/actions/runs/39109254830) |
| Node npm group | All seven packages are published at `0.9.1` under `latest`, with exact manifest integrities and npm provenance. The recovery reused the original group and attempt-bound report: the accepted macOS ARM64 member was skipped and the six unattempted members were published. | [Original run 39109264837](https://github.com/Latias94/merman/actions/runs/39109264837), [recovery 39111766250](https://github.com/Latias94/merman/actions/runs/39111766250) |
| Flutter | [pub.dev `merman 0.9.1`](https://pub.dev/packages/merman/versions/0.9.1) is published. A separate read-only archive reconciliation returned `exact`, matching file paths, executable bits, sizes and bytes with the frozen owner package. | [39109126069](https://github.com/Latias94/merman/actions/runs/39109126069) |
| Python | PyPI `merman 0.9.1` is published with Linux x86_64, macOS ARM64 and Windows x86_64 wheels. The three registry SHA256 values match the corresponding GitHub Release assets. | [39111572953](https://github.com/Latias94/merman/actions/runs/39111572953) |
| Android | `merman-android-v0.9.1.aar` is available on the workspace GitHub Release with SHA256 `55a7c0270f9f4fa01ec2f698e6889097a20b43fbd5dc1f924185c5f8f42c971b`. The native build, JVM contract tests and owner release-byte verification passed; this is an AAR release asset, not a Maven Central upload. | [39111583242](https://github.com/Latias94/merman/actions/runs/39111583242) |
| Apple | `Merman.xcframework-v0.9.1.zip` and its checksum are available on the workspace GitHub Release. Exact-source Xcode 15.2 / Swift 5.9.2 validation passed, including iOS XCFramework builds and generated Swift typechecks. | [Floor check 39108265268](https://github.com/Latias94/merman/actions/runs/39108265268), [publisher 39111593825](https://github.com/Latias94/merman/actions/runs/39111593825) |
| Independent Roughr / Tree-sitter / Typst / VS Code; external package managers and Pages | No publication is part of this patch operation. | Independent release boundaries |

The first and tag-retry runs stopped at the full Typst package smoke because its independently versioned API fixture pins Merman `0.9.0`, while the release artifact correctly contains `0.9.1`. PR #210 keeps the exact artifact build and examples smoke in the workspace release gate; the version-pinned API suite remains with the Typst package owner's CI. The crates.io run's seven topological batch receipts all reached `complete`. It published `merman-core` and 19 other missing crates; `dugong-graphlib` and `manatee` were already accepted by the earlier attempt. All 22 exact `0.9.1` versions were independently visible through the crates.io API with checksums matching the prepared artifacts. The publication workflow came from PR #210 while package contents remained pinned to the immutable release tag above.

The Web package-group artifact is `merman-web-npm-package-group` (artifact `12130147285`, digest `sha256:453da1d7acef9e86729ff6fb677f8746de8ceb23196540ea956fc051d615b2b9`); its manifest SHA256 is `4060abdea77b7cdb41510971ae3ddf1f249e38de0243f65d33214bfa8d5b0cbf`. The owner report reached `released`, and a subsequent read-only reconciliation returned the same result.

The Flutter owner retained `merman-flutter-package` (artifact `12130837889`, digest `sha256:44ed45c190062d7a2e3e37eb7b80d00df685666bd3aaaac8bea0ccb6fefe3ebb`). Its 86,205,594-byte source/tree-bound archive has SHA256 `765c64897f4d57850d08c7713fc884017f4a67e7ffe986b948e4ca2c0674df52`; the registry-repacked archive has SHA256 `14cbf6408f97c8dc48dd92597ca99881adc6dced735e575dcc96ac0808726d65`. Their canonical contents match. The shipped README and changelog describe `0.9.1`, dated 2026-10-18.

The Node recovery report `merman-node-npm-reconciliation-report-attempt-1` (artifact `12130887049`, digest `sha256:da802884c27103eb6a5e1d8862086c94baa92a2043b6021749ec45a51de92d5a`) reached `released`; its manifest SHA256 is `4913c3a0185c9ac0bb105e4c3ae09a5e7e8caa4912b59736374d9fd891f4feb3`, with one already-published member and six exact recovery uploads.

The Apple release assets are `Merman.xcframework-v0.9.1.zip` (297,812,765 bytes, SHA256 `75a3c4c4431e1966685476d20d6d231235458cecee45721c2129db9761c9e8c7`) and its 96-byte checksum sidecar (SHA256 `090511f3376dbe92332c45d417a4998c8ac040394ab4d74a9008b21c91ffc99a`). This is a GitHub Release XCFramework asset; it does not establish a remote SwiftPM binary target.

## 0.9.0 Publication Snapshot

Published on 2026-10-12 from immutable source commit `f754a3c3077d35d2a372c2fec04f9e9b87db1c3f`, tree `470c8d5b2dcc0a7b0746da6c93ae44ebf14d5eb7`. The workspace `v0.9.0`, Flutter `flutter-v0.9.0`, and independent `tree-sitter-mermaid-v0.3.1` tags resolve to that commit. Full [release preflight 38242910387, attempt 2](https://github.com/Latias94/merman/actions/runs/38242910387) passed all 24 jobs for that exact source; the initial Android Maven dependency lookup failed, then passed on retry without a source change. [Main CI 38242866257](https://github.com/Latias94/merman/actions/runs/38242866257) also passed.

| Surface | Observed publication or preparation | Owner evidence |
| --- | --- | --- |
| Independent rough drawing crate | `roughr-merman 0.12.4` is published and non-yanked; crates.io API and sparse-index checksum `b4accb6afabe559650a026a0938bf6ad84c7ebe66517e12b4385865e2179a070` match. Published-dependent compatibility passed before and after upload. | [38249268420](https://github.com/Latias94/merman/actions/runs/38249268420) |
| Independent grammar | `tree-sitter-mermaid 0.3.1` is published and non-yanked; API/index checksum `41d2b67fc0b1488f9db84823ca24916d5b26d01002b268b4f489032e1e7a2218` match. `@mermanjs/tree-sitter-mermaid@0.3.1` is npm `latest` with provenance; its [GitHub Release](https://github.com/Latias94/merman/releases/tag/tree-sitter-mermaid-v0.3.1) includes six verified assets. | [38249285380](https://github.com/Latias94/merman/actions/runs/38249285380) |
| Rust workspace | All 22 coupled crates at `0.9.0` are published. The seven topological result receipts reached `complete`, with prepared package digests equal to both crates.io API and Cargo sparse-index checksums. | [38250149414](https://github.com/Latias94/merman/actions/runs/38250149414) |
| New Unicode support crate | `merman-unicode-linebreak 0.9.0` completed its first upload through the token-authenticated owner workflow, in the first batch before `merman-render`. Its 18,560-byte package has checksum `0843d97d77ab53b3ccfc9f62734783b1747063b83435bd55335e1f758b1f58c2`, matching the verified local package and API/index observations. No manual bootstrap was needed. | First batch receipt from [38250149414](https://github.com/Latias94/merman/actions/runs/38250149414) |
| CLI / LSP archives | [v0.9.0](https://github.com/Latias94/merman/releases/tag/v0.9.0) is the public stable Latest workspace release, with CLI and standalone LSP archives for five targets, installers, checksum index, and `release-verification.json` bound to the release source. All 21 release jobs passed; the CLI/LSP bundle contains 26 assets. Separate native owners add their own assets below. Release-note documentation links point to the immutable tag. | [38250149396](https://github.com/Latias94/merman/actions/runs/38250149396) |
| Web npm group | All five browser packages at `0.9.0` are npm `latest` with provenance. The owner verified registry integrities against the frozen package-group artifact; all five final states are `verified`, and the manifest-bound report is `released`. | [38261092239](https://github.com/Latias94/merman/actions/runs/38261092239) |
| Node npm group | All seven packages at `0.9.0` are npm `latest` with provenance. Attempt 1 accepted the two macOS packages and Linux GNU, then retained a pending-visibility report. Attempt 2 reused the exact package group, verified and skipped those three, published only the four unattempted members, and completed with `released`; its manifest digest remains `95362f97439d80d5f92db062431a17210be7e6d393d0471ce9915fa3c8ece216`. | [38261101071, attempt 2](https://github.com/Latias94/merman/actions/runs/38261101071) |
| Python | [PyPI `merman 0.9.0`](https://pypi.org/project/merman/0.9.0/) is published with Linux x86_64, macOS ARM64 and Windows x86_64 wheels. All three registry SHA256 values and sizes match their GitHub Release assets. Existing PyPI Trusted Publishing completed the upload. | [38261109286](https://github.com/Latias94/merman/actions/runs/38261109286) |
| Android | `merman-android-v0.9.0.aar` is available on the workspace GitHub Release, SHA256 `14ef4f2f8d00924fac4a02e80bbfde4c19ce802ce22ddfc3d8b38ef844c2d0e8`. Native build, JVM contract tests and local Maven staging verification passed; this is an AAR release asset, not a Maven Central upload. | [38261119559](https://github.com/Latias94/merman/actions/runs/38261119559) |
| Apple | `Merman.xcframework-v0.9.0.zip` and its checksum are available on the workspace GitHub Release. The 297,227,577-byte archive has SHA256 `5c2b893f6175f79285526c7c3120b90d5dc10987c2fbf7d2fe497a1ac11bcfd3`. This is an XCFramework asset upload; it does not establish a remote SwiftPM binary target. | [38261127290](https://github.com/Latias94/merman/actions/runs/38261127290) |
| Scoop | Main bucket provides `merman-cli 0.9.0`. Its Windows archive URL and SHA256 match the verified release. The automatic bucket update made [PR #8680](https://github.com/ScoopInstaller/Main/pull/8680) redundant, so that PR was closed. | [Upstream publication commit](https://github.com/ScoopInstaller/Main/commit/179b1bfa935d1305c3f3294f56addc3d2a6bd881), [registry candidate validation](https://github.com/Latias94/merman/actions/runs/38261134623) |
| WinGet | Existing first-package [PR #447353](https://github.com/microsoft/winget-pkgs/pull/447353) now submits `Latias94.MermanCLI 0.9.0`, replacing its unaccepted 0.8.0 candidate. Manifest, URL, policy, scan, metadata and final validations plus CLA passed; installation validation is neutral. The verified archive passes local version/render smoke. Upstream review, acceptance and registry propagation remain pending. | [Registry candidate validation](https://github.com/Latias94/merman/actions/runs/38261134623), [current submission](https://github.com/microsoft/winget-pkgs/pull/447353) |
| Homebrew | Official BrewTestBot [PR #317871](https://github.com/Homebrew/homebrew-core/pull/317871) merged on 2026-10-13 after platform checks passed. The public formula API serves 0.9.0 with the verified source URL/checksum and five bottle entries. Installed CLI contract 6, capabilities, completions/manual pages, linkage and formula tests passed on macOS and Linux in the scheduled health run. | [Official update PR](https://github.com/Homebrew/homebrew-core/pull/317871), [formula API](https://formulae.brew.sh/api/formula/merman-cli.json), [health run 38295804248](https://github.com/Latias94/merman/actions/runs/38295804248) |
| Nixpkgs | Existing first-package [PR #570822](https://github.com/NixOS/nixpkgs/pull/570822) was updated to 0.9.0 at `af4bbec0aa0de242fb14c11cd8cdc40c12b4fdd9`. Source/NAR identity, byte-identical release Cargo.lock, the 22-feature CLI profile, static contracts and staged installed-CLI checks passed. Nixpkgs parsing, formatting and evaluation CI passed; a full build of this Nixpkgs expression and `nixpkgs-review` remain outstanding. Upstream acceptance remains pending. | [Updated PR](https://github.com/NixOS/nixpkgs/pull/570822), [Nixpkgs CI](https://github.com/NixOS/nixpkgs/actions/runs/38284094208) |
| Conda-forge | [PR #10](https://github.com/conda-forge/merman-cli-feedstock/pull/10) follows the 0.9.0 bot proposal and fixes AWS-LC jitterentropy's optimization flag order. The initial Unix-only CMake repair passed all seven platform builds. After review, `b9e4d9cff8f064bc78ada7538c4675e64ec033ad` uses CMake/Ninja globally and retains the Windows ARM64 clang-cl requirement. All seven recipe renders and dependency solves passed; fresh build/test CI and upstream acceptance remain pending. | [Review reply](https://github.com/conda-forge/merman-cli-feedstock/pull/10#discussion_r4253759074), [current-head CI](https://github.com/conda-forge/merman-cli-feedstock/actions/runs/38330751230) |
| Typst wrapper | `@preview/merman:0.5.0` was submitted on 2026-10-13 in [Universe PR #6098](https://github.com/typst/packages/pull/6098), bundling frozen Merman 0.9.0 with ABI 2. Local package smoke passed 21 fixtures and one expected diagnostic on each of Typst 0.15.0 and 0.15.1. Universe lint, packaging and package checks passed; upstream acceptance and registry availability remain pending. | [Typst owner job](https://github.com/Latias94/merman/actions/runs/38250149414/job/114826095865), [Universe PR](https://github.com/typst/packages/pull/6098), and [preparation record](TYPST_050_PREPARATION.md) |
| Flutter | [pub.dev `merman 0.9.0`](https://pub.dev/packages/merman/versions/0.9.0) is published after the frozen five-platform archive/source/tree/digest checks and OIDC upload. Registry archive SHA256 is `d7bc78d6cafc36f27720cd3ec2d82f0b211971f0ace284079e6e5ddec21d9cd6`; the retained owner artifact is 11755905195, digest `sha256:4e8778ecbf9346be919dff611653d744eadf7bb5f58d3a9bf95e527ec6dc6f5d`. The read-only reconciliation returned `exact`, confirming identical file paths, executable bits, sizes and bytes between the frozen package and registry archive. | Publisher [38261151108](https://github.com/Latias94/merman/actions/runs/38261151108), reconciliation [38269064375](https://github.com/Latias94/merman/actions/runs/38269064375) |
| VS Code | Passed the immutable preflight's package/build gates. Marketplace publishing remains separate and was not dispatched by this operation. | [38242910387](https://github.com/Latias94/merman/actions/runs/38242910387) |

The retained crates.io receipt artifact is `crates-io-receipts-f754a3c3077d35d2a372c2fec04f9e9b87db1c3f-attempt-1` (artifact 11749663381, digest `sha256:020ba4da2ef536211edfd232e2ed94bb2e1dc55eeda63ccaacca3d30781a15cb`). The immutable preflight artifact is `release-preflight-f754a3c3077d35d2a372c2fec04f9e9b87db1c3f-attempt-2` (artifact 11747715679, digest `sha256:37654d6a08f95cde8623e3987cfe290fea798464359e52ffdda2d71e47c55fea`). GitHub returned HTTP 502 during the tagged crates workflow's optional evidence lookup, so it ran and passed the full feature and Web checks instead of reusing them.

### Authentication checkpoint

On 2026-10-13, the maintainer reported configuring crates.io Trusted Publishers. Commit
`f069818bd7efc5d617c7ce56d78780449d05731e` connected the three Rust publisher owners to OIDC;
focused protocol/recovery/security tests and full script hygiene passed. Subsequent
[main CI](https://github.com/Latias94/merman/actions/runs/38279518132) passed with that integration.
No Rust version was uploaded to test it: live exchange/upload verification remains for the next
new-version release. Existing npm, PyPI and pub.dev OIDC uploads already succeeded for 0.9.0.
The old Cargo publishing secret is retained but is not a fallback in the new owners; remove it
after the first live OIDC upload is verified. Incident yanks retain their separate credential.

## 0.9.0 preparation

Preparation checkpoint before publication on 2026-10-12 (Asia/Shanghai): the approved intended release date was 2026-10-12 for workspace 0.9.0 and Typst wrapper 0.5.0. The root, workspace-coupled package, and Typst changelogs were dated for immutable preflight. The last verified stable workspace release at this checkpoint was v0.8.0, published on 2026-10-06. The preparation evidence below does not assert publication; the completed owner results are recorded in the [0.9.0 publication snapshot](#090-publication-snapshot).

The source delta starts at `v0.8.0` (`2029bd547077b1a6800878e125663633d81e6d87`). [Release-preparation PR #194](https://github.com/Latias94/merman/pull/194) was merged into `main` as `413d53730027a57beb75d361f38d69e5e2ee920c`, with the same source tree as its reviewed head `351b8158a6bc4d6ea1d5951596175cef8a4f03cd`. The PR completed with 58 successful checks and four conditional skips. The release source must include the subsequent dated-documentation commit. Bind its final 40-character SHA to the immutable preflight run; PR checks do not replace that run. The lightweight documentation evidence ledger is under `target/release-docs-0.9.0/`; it is not a publication receipt.

| Surface | Selected target | Preparation and delivery boundary |
| --- | --- | --- |
| Independent rough drawing crate | `roughr-merman 0.12.4` | Private host-random upgrades preserve the public legacy svgtypes 0.11 API. Owner SemVer and fresh published-consumer evidence are required; independently publish and confirm crates.io visibility before workspace dependents. |
| Independent grammar | `tree-sitter-mermaid 0.3.1` / `@mermanjs/tree-sitter-mermaid 0.3.1` | Exact CLI/Rust/Web 0.27.1 contract, compatible language adapter `^0.1.9`, and generated language ABI 15. Verify generated parser, root WASM, package/provenance, and owner smoke. Independently publish the required Cargo package before workspace dependents; npm and GitHub evidence remain separate. |
| Workspace Rust / CLI / LSP | `0.9.0` / intended `v0.9.0` | Version projection is prepared and changelogs are dated 2026-10-12. Frozen-source preflight, registry-visible independent prerequisites, and publication receipts remain required. |
| Web / Node / Python / Flutter / Android / Apple | `0.9.0` | Prepared source contracts and documentation only. Each owner must build and verify its exact package group or native artifact; workspace shipping does not authorize channel shipping. |
| Independent Typst wrapper | `@preview/merman:0.5.0` | Bundles Merman 0.9.0. Owner source, WASM size, interpreter, package, examples, and legal checks precede a separately authorized manual registry submission. Published 0.4.0 continues to bundle Merman 0.8.0. |
| Independent VS Code extension | `0.1.0` | Prepared runtime binaries follow Merman 0.9.0. Extension artifacts and Marketplace availability are separate from workspace publication. |

Publish order is the two independent Cargo prerequisites first, then the topologically ordered workspace crates and CLI/LSP release, followed by separately authorized channel owners. Typst preparation runs alongside workspace preparation; a Cargo plugin publication does not publish its wrapper. Retain each exact artifact and receipt through recovery instead of rebuilding an accepted version.

The [dependency policy](../DEPENDENCY_POLICY.md) records intentional exceptions: deterministic libm; generated NAPI and Tree-sitter groups; public legacy svgtypes; the shared resvg/usvg/krilla/fontdb line; and frozen reference-host and TypeScript compiler/peer contracts. The preparation audit considers newer candidates but does not equate freshness with compatibility. A new `cargo deny` pass, complete npm audit, or green artifact preflight must be recorded only when its owner provides evidence.

These are verified local source-gate results at the 2026-10-12 preparation checkpoint. They do not identify a frozen release SHA or prove registry acceptance, complete behavior testing, or immutable preflight; later source changes require the affected gates to run again.

| Preparation gate | Current evidence boundary |
| --- | --- |
| Version projection | The `release-version.py` final source projection gate passed for workspace 0.9.0 and its coupled package versions. Revalidate after later projection or source changes. |
| Workspace compilation | `cargo check --workspace --all-targets --locked` passed in 17m24s. This checks compilation, not Rust behavior tests or platform package delivery. |
| Changelog admission | `verify_release_changelog.py` passed 0.9.0 preparation admission. Dated preflight and real artifacts remain separate gates. |
| Source/process regression tests | 37 Python tests across `test_release_process`, changelog, projection, and RustSec suites passed. |
| RustSec exception governance | The two governed exceptions passed `verify_rustsec_exceptions.py`. This is not a new `cargo deny` result or evidence that the exceptions have disappeared. |
| Security source gate | SHA-256-verified official cargo-deny 0.20.2 returned exit 0 for `cargo deny check advisories bans licenses sources`; all four summaries were `ok`. Upstream duplicate-version warnings remain, as do the two governed exceptions with review due 2027-01-11. |
| CI owner validation | 90 focused checks passed; actionlint, zizmor, and dist generation checks passed. These results do not certify frozen-source remote preflight or package publication. |
| User documentation | Root/platform changelogs, install examples, migration, Typst mapping, and dependency policy are prepared. Documentation static/link checks passed; inspect final packaged documentation bytes before freeze. |
| Cargo/npm candidate audit | Current candidates and compatibility exceptions were checked against primary registries. Six mutable npm roots reported no audit findings; the frozen Mermaid reference host retains five low findings and requires coordinated admission. Detailed audit records remain release artifacts. |
| Independent patch compatibility | Roughr 0.12.4 passed 202 public API checks against 0.12.3. Fresh candidate and registry lanes on Rust 1.95.0 compiled published merman-render 0.7.0, 0.8.0, and 0.8.0-alpha.7. Independent registry publication remains pending. |
| Generator provenance and license pins | Tree-sitter generator 0.27.1 source/component pins and the official license digest agree; provenance hashes for three component pins were refreshed and verified. This does not establish grammar registry publication or final package admission. |
| Legal material checks | Pinned cargo-about 0.9.2 generated all 13 Rust license reports; 394 release legal-material projections were refreshed, and `verify-third-party-licenses.py` passed. The 18 generation/sync unit tests and 20 third-party verification tests passed. |
| Selected Rust behavior tests | The nextest selection for Rust core/analysis, Tree-sitter grammar, roughr, and Typst/wasmi passed all 2,287 selected tests; 575 tests were skipped outside this selection. This selection does not establish a complete workspace behavior pass; the CLI contract selection is recorded separately. |
| Feature matrix | The ordinary curated owner gate passed all 42 builds and its isolated consumers, checking empty/default/subset/all diagram selections and transport combinations. The wider strict release matrix remains with immutable preflight. |
| Native and binding static owner gates | `verify-native-abi`, `binding-contract`, and `typst-profile-constants` each exited 0. These static source checks do not replace final package or size checks. |
| Representative dependency closures | All 36 artifact profiles passed representative dependency-closure checks, including `typst-wasm` and all five Web profiles. Final artifact size and package checks remain separate. |
| CLI process and distribution contracts | The `process_contract`, `distribution_assets`, and `cli_native_contract` selection passed 36/36 tests, covering native `render -e` / `batch -e` removal. |
| Profile and native-font contracts | `profile_contract` and `native_fonts_contract` passed 18/18 tests. Together with the 36 process/distribution/native contract tests, the combined CLI selection passed 54/54. |
| Typst artifact and package | The exact publish recipe passed existing size budgets: raw 13,936,547; optimized/stripped 8,822,440; gzip 3,433,230; Brotli 2,552,937 bytes. The final 0.5.0 package passed interpreter validation and all fixtures with Typst 0.15.0 and 0.15.1: 21 compiled plus one expected failure per compiler. |
| Wasmi 2 migration | The initial real package run exposed missing automatic dispatch in unoptimized builds. Explicit auto-dispatch and a long-loop regression fix it; 87 grammar/Typst/Wasmi tests passed after the change, followed by the real package checks above. |
| Node candidate contracts | The selected NAPI recipe compiled with runtime 3.14.2, derive 3.6.12, and build 2.6.0. All 16 selected Rust transport tests and 99 JavaScript owner tests passed. Native loading across the published target set remains an owner CI gate. |
| Linux Rust and SVG owner | [PR run 38227361221](https://github.com/Latias94/merman/actions/runs/38227361221) passed the complete Linux workspace tests, feature contracts, Rustdoc fragment check, SVG DOM suite, and browser root-viewport audit at preparation source `24e69ad66`. The title-029 audit was revalidated for Playwright 1.64.0 and Chromium 156 with identical SVG hashes and paired paint fields; its exact browser fingerprint was regenerated. This is source-owner evidence, not immutable release preflight. |
| Web WASM release size | [Main run 38203131903](https://github.com/Latias94/merman/actions/runs/38203131903/job/114661820476) and [PR run 38221177606](https://github.com/Latias94/merman/actions/runs/38221177606/job/114718118763) measured the same final-package recipes with Rust 1.95.0 and wasm-pack 0.15.0. Analysis Brotli grows from 1,122,850 to 1,126,753 bytes (0.35%); the remaining Web profiles grow by at most 0.30% across size metrics. The analysis Brotli budget was refreshed to 1,175,000 bytes under the existing headroom rule; the other limits remain unchanged. [Follow-up run 38227361221](https://github.com/Latias94/merman/actions/runs/38227361221/job/114738155641) passed the final Web package size check at preparation source `24e69ad66`, reproducing analysis Brotli at 1,126,753 bytes. Its subsequent Chromium editor tests exposed Monaco 0.57's omitted document semantic-token feature registration; an explicit owner import restores highlighting and passed 8 Chromium worker tests plus 62 editor unit tests locally; a complete Web owner result remains required. |
| Playground dependency migration | A real npm 12 installation passed npm ls and audit with no findings. TypeScript and Vite prepared builds, final dist isolation checks, and 446 owner tests passed; the prepared CSP check also passed. Its local frontend input build does not establish a fresh release Rust-WASM receipt. |
| Grammar generation and package consumers | SDK 34 and CLI 0.27.1 produced a byte-identical regeneration with ABI 15. Corpus 259/259, generation tests 9/9, native Node, Web-WASM, C, and packed package consumers passed; the freshly packed Cargo consumer also passed on pinned Rust 1.95.0; registry availability remains separate. |
| Final-version generated metadata | Playground catalog freshness and the combined `verify-generated` gate passed. Grammar parser/WASM metadata now matches 0.3.1; 11 generator-script, 259 corpus, four WASM, and 30 nextest checks passed. SDK 34 compiler validation binds official host banners to one LLVM commit. [CI run 38215861848](https://github.com/Latias94/merman/actions/runs/38215861848) passed native, WASM full-byte reproduction, Windows generation, and packed grammar-owner checks for preparation commit `5e10b2f93`; this is not immutable release preflight. |
| Generated CLI documentation | All 15 tracked manual pages were regenerated with Merman 0.9.0 headers. The source-asset checks across the temporal environments and Cargo source-package inclusion check passed. |
| Native CLI format migration | The selected 0.9.0 contract removes native `render -e` / `batch -e`; migrate to `-f/--format`. The pinned `mmdc -e/--outputFormat` remains supported. Alias-removal behavior is covered by the passed CLI contract selection. |
| Native, WASM, size, and package-content checks | Require final-source owner results; compilation and static declarations do not replace these artifact checks. |
| Dated source and immutable preflight | The approved intended date is 2026-10-12 for workspace 0.9.0 and Typst wrapper 0.5.0. Commit the dated documentation, freeze that 40-character source SHA, and require a successful `release-preflight.yml` run for version 0.9.0 and that exact SHA before publication. No successful immutable preflight or registry publication is asserted at this checkpoint. |

See the [0.8.0 to 0.9.0 upgrade guide](V080_TO_V090_UPGRADE_GUIDE.md), [Typst preparation guide](TYPST_050_PREPARATION.md), and [release evidence procedure](RELEASING.md#release-evidence). Dated publication snapshots below retain their original release context.
The source also carries experimental compiled-theme integration. It is not part of immutable `0.8.0` artifacts, and any `0.9.0` claim remains subject to the matching artifact, theme evidence, and owner gates above.

## 0.8.0 Publication Snapshot

Recorded on 2026-10-06 in Asia/Shanghai (registry and GitHub API timestamps use UTC). The immutable `v0.8.0` and `tree-sitter-mermaid-v0.3.0` tags resolve to `2029bd547077b1a6800878e125663633d81e6d87`. Full workspace preflight [37344366248](https://github.com/Latias94/merman/actions/runs/37344366248) passed all 24 jobs for that exact source.

| Surface | Verified publication | Owner run |
| --- | --- | --- |
| Workspace Rust crates | All 21 coupled crates at `0.8.0`; all seven batch receipts are complete and registry checksums match the prepared packages | [37353884068](https://github.com/Latias94/merman/actions/runs/37353884068) |
| CLI/LSP archives | Ten archives across five targets, four installers, checksums, and verification metadata; all 26 published assets match the attested recovery bundle | [37359550915](https://github.com/Latias94/merman/actions/runs/37359550915) |
| Tree-sitter grammar | `tree-sitter-mermaid 0.3.0` on crates.io, `@mermanjs/tree-sitter-mermaid@0.3.0` on npm with `latest` and provenance, and the independent GitHub Release | [37347725013, attempt 2](https://github.com/Latias94/merman/actions/runs/37347725013) |
| Web npm group | All five packages at `0.8.0` under `latest`; npm integrities match the original package-group artifact and npm provenance is present | [37450821128](https://github.com/Latias94/merman/actions/runs/37450821128) |
| Python | PyPI `merman 0.8.0`: Linux x86_64, macOS ARM64, and Windows x86_64 wheels; SHA-256 values match the CI artifacts | [37458740996](https://github.com/Latias94/merman/actions/runs/37458740996) |
| Node npm group | All seven packages at `0.8.0` under `latest`; npm integrities match the original package-group artifact and each package has npm provenance | [37466116228](https://github.com/Latias94/merman/actions/runs/37466116228), original artifact [37455324984](https://github.com/Latias94/merman/actions/runs/37455324984) |
| Flutter | pub.dev `merman 0.8.0` is published; registry archive contents match the frozen five-platform package artifact | [37458786843](https://github.com/Latias94/merman/actions/runs/37458786843) |
| Android, Apple, VS Code, Typst, Pages, external package managers | Not published by this language-package operation; their own registries, releases, and owner workflows remain authoritative | Separate authorization and publication |

Node recovery preserved the exact original package-group artifact and prior attempt-bound reports. The initial publish stopped after accepting macOS ARM64 while npm visibility lagged. The first recovery stopped before uploading because `gh api --slurp` cannot be combined with `--jq`; [PR #173](https://github.com/Latias94/merman/pull/173) fixes that command. Recovery runs [37460934208](https://github.com/Latias94/merman/actions/runs/37460934208) and [37466116228](https://github.com/Latias94/merman/actions/runs/37466116228) used workflow commit `741e96b45` with the unchanged release source, confirmed existing packages, and uploaded only members recorded as unattempted. All seven final registry integrities and tags match the original artifact; no member was republished.

Grammar recovery preserved the original candidate. The first Cargo job stopped before uploading because a missing CDN object returned HTTP 403; the version API correctly returned 404. Local Cargo packaging produced the original CI `.crate` byte-for-byte (`a6058ff44d72f9ba6770319e1428ca45188ec29e83b9b9fe246793034c569a0f`). That exact package was published with the maintainer's existing credential, then only failed jobs of the original run were rerun. crates.io API and sparse-index checksums, npm integrity, and all six GitHub assets match the original candidate. npm was published through Trusted Publishing; no tag moved.

The first CLI/LSP run [37353884104](https://github.com/Latias94/merman/actions/runs/37353884104) passed archive and native smoke checks but stopped because WinGet was given the parent manifest directory. Recovery used workflow commit `a0d83790673dd46de408776737b70b5c64f7c21f` with the unchanged `v0.8.0` source. Attempts 1 and 2 could not acquire a hosted runner during a GitHub Actions incident. Attempt 3 reused all five completed builds and passed archive verification, native smoke checks, and asset attestation. Its WinGet validator reported schema-header warnings and returned failure; the identical generated candidates passed local official WinGet `1.29.380` validation with exit code 0.

The operator completed the GitHub Release upload from that exact 26-file, attested attempt-3 bundle, without rebuilding binaries or moving tags. Every published asset size and SHA-256 matches the downloaded bundle and attestation. The automation run remains failed at its registry-candidate step; it must not be rerun to recreate the already-published release. Merged [PR #169](https://github.com/Latias94/merman/pull/169) separates registry validation from binary publication and pins the WinGet validator. The independent [registry validation run 37416048947](https://github.com/Latias94/merman/actions/runs/37416048947) passed for both manifests. [WinGet PR #447353](https://github.com/microsoft/winget-pkgs/pull/447353) and [Scoop PR #8639](https://github.com/ScoopInstaller/Main/pull/8639), with [package request #8640](https://github.com/ScoopInstaller/Main/issues/8640), were submitted on 2026-10-06 and remain under upstream review; submission is not registry availability.

Before the language-package operation, an additional registry check on 2026-10-06 found Homebrew at `0.8.0` with bottles, but its [current formula](https://github.com/Homebrew/homebrew-core/blob/master/Formula/m/merman-cli.rb) installs only the executable, not completion or man assets. At that checkpoint, the npm Web/Node groups, PyPI, and pub.dev did not yet contain stable `0.8.0`; the table above records subsequent publication evidence. The independent Tree-sitter npm package was already at `0.3.0`. [Conda-forge PR #4](https://github.com/conda-forge/merman-cli-feedstock/pull/4) proposes `0.8.0` but has a failing Windows ARM64 build; [Nixpkgs PR #539674](https://github.com/NixOS/nixpkgs/pull/539674) still proposes the earlier `0.7.0` package. Coordinate with those existing submissions rather than opening duplicates. These observations do not authorize another owner publication; re-query each registry before shipping.

The immutable `0.8.0` crates, Python wheel metadata, Flutter pub.dev archive, and Node loader tarball contain preparation-time wording that still calls the version unpublished or a candidate. The current repository documentation reconciles publication status; it does not change already-published packages. Use the registry version and this dated snapshot when those historical availability sentences disagree.

## Alpha.7 Publication Snapshot

Published on 2026-09-30 from immutable source commit `580e39b69cc1b0ca35c4f8272683e622b2e9b8db`. The workspace tag `v0.8.0-alpha.7`, Flutter tag `flutter-v0.8.0-alpha.7`, and independent grammar tag `tree-sitter-mermaid-v0.2.0` all resolve to that commit. The merged `main` commit `bb1df0a78719c713c854b10a4bcab1ce63fd3a90` has the same source tree. This is a dated operator snapshot, not machine release authority; re-query each owning registry or GitHub Release before recovery or another publication.

| Surface | Verified publication | Owner run |
| --- | --- | --- |
| Workspace crates | All 21 coupled crates at `0.8.0-alpha.7`; every batch receipt is complete and every registry checksum matches its prepared artifact | [36727907019, attempt 2](https://github.com/Latias94/merman/actions/runs/36727907019) |
| CLI/LSP | Ten archives across five targets, installers, checksums, and verification metadata on the alpha.7 GitHub prerelease | [36727907026](https://github.com/Latias94/merman/actions/runs/36727907026) |
| Web npm group | All five packages at `0.8.0-alpha.7` under `alpha`; original artifact integrities and npm provenance match | [36730731324, attempt 2](https://github.com/Latias94/merman/actions/runs/36730731324) |
| Node npm group | All seven packages at `0.8.0-alpha.7` under `alpha`; original artifact integrities and npm provenance match | [36730739282](https://github.com/Latias94/merman/actions/runs/36730739282) |
| Python | PyPI `merman==0.8.0a7`; all three wheel hashes match the GitHub Release assets | [36734845127](https://github.com/Latias94/merman/actions/runs/36734845127) |
| Flutter | pub.dev `merman 0.8.0-alpha.7`; published package contents match the source-bound candidate archive | [36730730700](https://github.com/Latias94/merman/actions/runs/36730730700) |
| Android | Alpha.7 AAR attached to the GitHub Release; its digest matches the original workflow artifact | [36734851815](https://github.com/Latias94/merman/actions/runs/36734851815) |
| Apple | Alpha.7 XCFramework archive and matching checksum attached to the GitHub Release | [36734858180](https://github.com/Latias94/merman/actions/runs/36734858180) |
| Tree-sitter Mermaid | Independent `0.2.0` on crates.io and npm, plus six original attested assets on its own GitHub Release; the Cargo package preceded the workspace publication | [36723376105, attempt 2](https://github.com/Latias94/merman/actions/runs/36723376105) |
| Playground | Pages deployment succeeded for the merged source tree and the deployed site responds successfully | [36727744516](https://github.com/Latias94/merman/actions/runs/36727744516) |

The successful retries reconciled registry propagation delays against the original receipts or package-group artifacts. No published tag was moved and no npm group was rebuilt for recovery. The grammar GitHub Release uses the attested artifacts from its original attempt. Alpha publication leaves unrelated npm tags and GitHub's stable Latest release unchanged.

The Typst wrapper remains independently published as `@preview/merman:0.3.0`; the new `merman-typst-plugin` Cargo crate does not update that wrapper. VS Code Marketplace, Android Maven Central, and stable package-manager submissions were not part of this alpha publication.

## Alpha.6 Backfill Snapshot

The alpha.6 workspace release was published from immutable tag `v0.8.0-alpha.6` at commit
`d529f858ea3d337a1bdc8fe12e44e1403ededf2e`. The crates.io workflow published all 20 workspace
crates, and the GitHub Release published the CLI/LSP archives and their verification assets. The
browser, Node.js, Flutter, Python, Apple, Android, and Typst package groups remain independent
publication tracks; their alpha.6 availability must be established by their owning workflow or
registry rather than inferred from the workspace tag. The browser and Node package groups were
previously published as an authorized alpha-channel test at `0.8.0-alpha.5` from reviewed commit
`d4365ca4860b6b4d51c421e775daab92a815c667`, newer than the workspace `v0.8.0-alpha.5` tag. Their
verified package-group manifests and workflow artifacts identify that commit. Because that first
publication was a manual bootstrap, those npm registry artifacts do not expose npm provenance
attestations; documentation must not imply either an attestation or cross-channel byte identity.

Initial state was observed on 2026-09-04 for immutable source tag `v0.8.0-alpha.6` at commit `d529f858ea3d337a1bdc8fe12e44e1403ededf2e`; the Node npm group was reconciled on 2026-09-06. This is a dated operator snapshot, not machine release authority; re-query each owning registry or GitHub Release immediately before any mutation.

| Surface | Observed state | Backfill action |
| --- | --- | --- |
| Workspace crates and FFI source (`merman-bindings-core`, `merman-ffi`, `merman-uniffi`, `merman-wasm`) | `0.8.0-alpha.6` visible on crates.io | No action |
| CLI/LSP archives | Present on the alpha.6 GitHub Release | No action |
| Typst `@preview/merman:0.3.0` | Published on 2026-09-01 from the alpha.6 source line | No action; reconcile docs only |
| Python `merman` | PyPI exposes `0.8.0a6` with the three expected wheels, and the same hashes are attached to the alpha.6 GitHub Release | No action; reconciled by `release-python.yml` run `33862859069` |
| Android AAR | `merman-android-v0.8.0-alpha.6.aar` is attached to the alpha.6 GitHub Release | No action; reconciled by `release-android.yml` run `33858143487` |
| Apple XCFramework | `Merman.xcframework-v0.8.0-alpha.6.zip` and its checksum are attached to the alpha.6 GitHub Release | No action; reconciled by `release-apple.yml` run `33858143156` |
| Web npm group | All five public packages expose `0.8.0-alpha.6`, the `alpha` tag, the original package-group integrities, and npm provenance attestations | No action; recovered from original run `33858142954` by `release-web.yml` run `33865637706` |
| Node npm group | All seven packages expose `0.8.0-alpha.6`, the `alpha` tag, and the exact verified artifact integrities; a fresh registry install/render smoke passed on Windows. The manual bootstrap has no npm provenance. | No action; bootstrapped from original run `33869785698` artifact `9936094987`, with Trusted Publishing configured for all seven names |
| Flutter `merman` | pub.dev exposes `0.8.0-alpha.6` from the pushed `flutter-v0.8.0-alpha.6` tag | No action; published by tag-triggered run `33862063296` |

The Python wheel, Flutter package archive, and Node alpha.6 loader tarball were built before this
post-publication documentation reconciliation, so their immutable payloads may retain
prepared-candidate wording in embedded README or changelog text. The current source guidance is
corrected on `main` and in the alpha.7 package documentation; the immutable alpha.6 payloads are not rewritten.

VS Code currently produces GitHub Actions VSIX artifacts only, Homebrew validates stable formulae, and
Android Maven Central and the Typst wrapper do not share the workspace crates.io publication path.
Those surfaces are not additional alpha.6 registry mutations in this backfill.

Rationale:

- crates.io versions are immutable and `0.8.0-alpha.1` has already started the 0.8 release line.
- The workspace has added 0.8-line Typst/package-size feature work and Mermaid parity fixes that
  should be tested behind a prerelease before the next stable cut.
- Workspace-coupled platform packages should stay aligned so downstream Web, FFI, and documentation
  integrations test one coherent version graph. The unpublished VS Code extension follows its own
  `0.1.x` version track and records the bundled workspace runtime separately.

Workspace Cargo manifests were published as `0.8.0-alpha.6`. Python package metadata uses the PEP
440 spelling `0.8.0a6`, but that version remains an independent publication decision; likewise,
separately published alpha.5 channels do not share a source snapshot with the workspace release by
implication. The independently versioned VS Code extension, Typst wrapper, and `roughr-merman`
remain on their own release axes. The `tree-sitter-mermaid` language distribution also has an
independent version axis.

For every prerelease, coupled workspace dependency requirements are exact (`=X.Y.Z-alpha.N`,
`=X.Y.Z-beta.N`, or `=X.Y.Z-rc.N`). This is a source-manifest rule, not a lockfile preference:
fresh consumers must never be allowed to select a newer sibling package for an older facade. Stable
workspace releases keep ordinary compatible requirements. The four binding source crates remain in
the crates.io graph; native platform bytes are delivered by their owner workflows and are not
implied by the presence of the source crates. Registry tarballs are immutable; if a published
prerelease contains a moving sibling requirement, the correction takes effect only in a later
release (or a new compatibility line), not by editing the already-published version in this tree.

## Typst 0.4.0 publication

Verified on 2026-10-12 (Asia/Shanghai): [Typst Universe](https://typst.app/universe/package/merman)
lists `@preview/merman:0.4.0` as published on 2026-10-09. Its version mapping identifies
Merman `0.8.0`, Typst plugin ABI `2`, and a minimum Typst version of `0.15.0`.
Normal users can import the registry package directly; `--package-path` is for testing local builds.
This registry observation does not assert a byte-for-byte comparison with a new source build.
The published package still contains preparation-time README wording; the current source README
corrects its installation guidance without changing the already-published WASM artifact.

## Typst Package Surface

The Typst wrapper is an independent publication surface. `@preview/merman:0.3.0` was published to Typst Universe on 2026-09-01 from the Merman `0.8.0-alpha.6` source line and Typst compiler `0.15.0`. It is not published by crates.io: `merman-typst-plugin@0.8.0-alpha.6` is the Cargo transport crate, while `@preview/merman:0.3.0` is the user-facing Typst package containing the frozen wrapper, size-optimized WASM artifact, and third-party legal materials. Build provenance remains in the private artifact directory and is not part of the registry package.

Version `0.3.0` is the first Typst package rebuilt after the text-measurement closure reduction. ICU4X collation data and generated font-metric tables are no longer linked into the production artifact; the plugin keeps deterministic measurement and retains the existing Typst plugin ABI `2` exports. Relative to the prior `0.2.0` development candidate, the package version changes because the shipped implementation closure and size characteristics are materially different; this rebuild itself does not change the ABI 2 transport protocol.

The unmerged theme development source uses Typst plugin ABI `3`, Options `3`, and the closed
`theme_operation_json` dispatcher. Its retained `0.5.0` package metadata is a local development
value; these additions are absent from the independently submitted ABI 2 package `0.5.0` and the
frozen `0.3.0`/`0.4.0` registry artifacts. Select the next package release during its own preparation.

For a future Typst submission, bind the package to the reviewed 40-character source SHA and run the owner gates from `docs/release/RELEASING.md`: `verify-typst-profile-constants`, the `typst-wasm` dependency-closure check, `build-typst-package --profile publish --budget-file docs/release/WASM_SIZE_BUDGETS.json` for one production/budget/package pass, and the full Typst package smoke. Inspect the private artifact manifest under `target/typst-wasm-artifacts/` together with `LICENSE`, `THIRD_PARTY_NOTICES.md`, and `THIRD_PARTY_LICENSES/` in the staged package. The provenance manifest is a preflight input, not a runtime package file. Before a future submission, finish and inspect the package README with final import and usage examples. After acceptance, query the registry for the exact package and update this release record with observed publication evidence.

## Publish Order

Cargo rewrites workspace path dependencies to registry dependencies when packaging. A dependent
crate's `cargo package` or `cargo publish --dry-run` therefore requires the matching dependency
versions to exist on crates.io. Before they are available, use the registry-independent checks in
the [release preflight](RELEASING.md#release-preflight); local dry runs cover only the
registry-satisfiable batches.

Cargo metadata is the publish-order authority. Inspect the current dependency-safe projection with:

```bash
python3 tools/publish.py --list-crates-io-packages
```

The helper selects crates.io-publishable `workspace_members`, excludes names listed in
`workspace.metadata.merman-release.independent-packages`, and follows coupled workspace path
dependencies, including optional, target-specific, renamed, build, and development dependencies.
Versioned development dependencies remain in Cargo's packaged manifests, so their coupled
packages must pass the registry visibility barrier before a consumer's batch is prepared. Private
path-only development fixtures remain outside the release graph. Normal/build dependencies on
private workspace members and coupled dependency cycles are rejected before publication. The resulting
graph is topologically sorted; these release-order edges do not change production dependencies.
Independent package dependencies are treated as external registry inputs and are verified by their
dedicated workflows. Only crates within the same coupled batch use lexical ordering. The local
publish flow, release preflight, and release workflow consume this same projection; Markdown is
not parsed as a release-order database.

The `merman-doc` implementation crate joined the coupled graph in `0.8.0-alpha.7`, bringing it to 21 packages. `merman-unicode-linebreak` joined in `0.9.0`, bringing it to 22; Cargo metadata places it in the first batch before `merman-render`. Both `merman-cli` and `merman-rustdoc` depend on `merman-doc`, so metadata also places that crate before either consumer without a separate workflow or a hand-maintained package list. Future releases must continue to verify the derived graph and each generated `.crate` through the release preflight and per-batch publication dry runs.

`roughr-merman` is versioned separately; `0.12.4` is the published prerequisite for workspace 0.9.0. The workflow reads each crate's own package version, so it can skip already-published crates while still keeping one dependency-ordered list.

`tree-sitter-mermaid` is a separately packaged language distribution; `0.3.0` is published on crates.io, npm, and its own GitHub Release. It was published before the Merman `0.8.0` LSP package that requires it. Its Cargo package is
`tree-sitter-mermaid`; its npm package is `@mermanjs/tree-sitter-mermaid`. Use
`release-tree-sitter-mermaid.yml`, not the generic independent-crate workflow. It builds native Node
prebuilds, verifies the root language WASM, installs the exact npm/Cargo/C candidate, stages a
grammar-subdirectory source archive and checksums, and publishes registry packages only when the
matching immutable `tree-sitter-mermaid-vX.Y.Z` tag passes the protected crates.io and npm
environments. GitHub Release publication is a separate explicit workflow input.
Because `merman-lsp` now consumes this crate for syntax highlighting, the exact Cargo version must
exist on crates.io before publishing a dependent workspace release. The scoped npm package is
independent of the workspace crates and supplies browser consumers with the same grammar WASM and
queries.

The historical `0.1.0` npm package was bootstrapped manually from attested run `32114670734` with the
maintainer's 2FA-protected credential. Its registry tarball SHA-256 is
`a4e54b9caee7940cfbcffbe2b97d6edf04d8979b3eafe75fc0bba7804d04b23b`; it does not carry npm
provenance. Trusted Publishing is configured for later versions through
`release-tree-sitter-mermaid.yml` and the `npm` environment. The crates.io package was accepted by
run `32117231294` and has checksum
`34921c596d2732a74eb6489f1148df732163f0c4fd20737547396f40a588b559`. That run reported a false
failure because crates.io rejected curl's default User-Agent during post-publish download; direct
registry verification confirmed the published bytes. Do not rerun it to create a GitHub Release:
the independent native prebuild outputs differ across runs, so the later run's npm candidate is not
byte-identical to the bootstrapped artifact.

## Binding Release Chain

The binding-specific chain is:

```text
merman-analysis
  -> merman-editor-core
  -> merman-lsp
tree-sitter-mermaid
  -> merman-lsp

merman-render
  -> merman-export
  -> merman
  -> merman-bindings-core
  -> merman-ffi
  -> merman-uniffi
  -> merman-wasm
```

This is why `merman-ffi` cannot fully package-verify until `merman-bindings-core` is published, and
`merman-bindings-core` cannot fully package-verify until a newer `merman-render` with `math`
is available on crates.io. `merman-wasm` comes last because it combines the browser wasm-bindgen
transport with the released binding core, renderer, ASCII, and editor-capable crates.

## Browser Package Group

The npm browser SDK is not a single Cargo publication and is intentionally outside the crates.io
topological order. After the selected source revision has passed release preflight, run
`release-web.yml` for the admitted package group: `@mermanjs/web`, `@mermanjs/web-analysis`,
`@mermanjs/web-editor`, `@mermanjs/web-ascii`, and `@mermanjs/web-render`. The workflow preflights
all existing versions and tags, then publishes missing exact versions directly under the requested
final tag in manifest order, with `@mermanjs/web` last. A retry skips matching published members.

The five existing Web packages have Trusted Publishing configured. If a future surface adds a new package name, follow the [npm bootstrap and recovery procedure](RELEASING.md#required-credentials) using its exact verified artifact. Do not rebuild a package group after a bootstrap or uncertain upload; a recovery run must consume the original artifact and retain its publication history.

## Node npm Package Group

The experimental Node packages are one lockstep npm group: the native loader and five platform
packages (`@mermanjs/node`, `@mermanjs/node-darwin-arm64`, `@mermanjs/node-darwin-x64`,
`@mermanjs/node-linux-x64-gnu`, `@mermanjs/node-linux-x64-musl`, and
`@mermanjs/node-win32-x64-msvc`) plus the explicit Node-targeted WASM package
`@mermanjs/node-wasm`. Run `release-node.yml` against a reviewed immutable source commit after
the matching preflight succeeds. It builds and installs every native target and the WASM target,
preflights existing registry integrity and tags, then publishes missing exact versions directly
under the requested final tag in platform-first order, with the WASM package before the root
loader.

The first version of each npm package cannot use npm Trusted Publishing before the package exists.
For that one bootstrap, dispatch `release-node.yml` with `publish_to_npm=false` against the reviewed
immutable source and record its workflow run id. For alpha.6, run `33869785698` produced artifact
`9936094987` with Node `24.13.1`; its five platform tarballs, WASM tarball, and loader were manually
published under `alpha` after exact artifact verification. All seven registry integrities and tags
match that artifact, a fresh registry install/render smoke passed, and Trusted Publishing is now
configured for all seven package names. The immutable manual bootstrap has no npm provenance;
Trusted Publishing cannot add an attestation to an existing tarball. Alpha.7 subsequently published through Trusted Publishing, as recorded in its snapshot above. For a future publication, dispatch `release-node.yml` with `publish_to_npm=true` against the selected immutable source. Follow [Releasing](RELEASING.md#registry-propagation-and-recovery) for same-run retries or explicit `recovery_run_id` recovery; both preserve the original package-group bytes and attempt history. Do not keep an npm token in GitHub Actions.

The immutable `@mermanjs/node@0.8.0-alpha.5` loader tarball was packed before its package-local
changelog heading was dated, so the registry copy contains an `Unreleased` heading. This is a
documentation-only bootstrap defect in that immutable version; later packages carry their own dated changelogs.

The immutable `@mermanjs/node@0.8.0-alpha.6` loader tarball was packed before post-publication
documentation reconciliation, so its embedded README and changelog retain prepared-candidate
wording. This documentation-only bootstrap defect was corrected in the alpha.7 package documentation; the accepted alpha.6 registry bytes remain unchanged.

## Pre-Publish Gates

Before publishing, run focused checks:

```bash
python3 tools/publish.py --list-crates-io-packages
cargo semver-checks check-release -p roughr-merman --color always
cargo check -p merman-ffi
cargo check -p merman-uniffi
cargo check -p merman-wasm
cargo nextest run -p merman-bindings-core -p merman-ffi -p merman-uniffi -p merman-wasm
```

The `roughr-merman` check uses the latest published compatible registry version as its baseline.
For later `0.12.x` patches, `0.12.3` is the established compatibility floor; the release workflows
pin `cargo-semver-checks` so the result does not depend on a maintainer's local tool version.

For crates.io packaging, prefer publish dry-runs once registry dependencies are available. The
release workflow packages every member of a topological batch first and records the exact `.crate`
digest. It then requires all missing members in that batch to pass this gate before the first real
publish attempt, so it also covers `merman-bindings-core`, `merman-ffi`, `merman-uniffi`, and
`merman-wasm`.

```bash
cargo publish -p merman-render --locked --dry-run --registry crates-io
cargo publish -p merman-export --locked --dry-run --registry crates-io
cargo publish -p merman-bindings-core --locked --dry-run --registry crates-io
cargo publish -p merman-ffi --locked --dry-run --registry crates-io
cargo publish -p merman-uniffi --locked --dry-run --registry crates-io
cargo publish -p merman-wasm --locked --dry-run --registry crates-io
```

Before upstream crates for the same release are visible in crates.io, keep using `cargo package
--list` only as a file-list check. It does not replace publish dry-run verification.

The credentialed workflow writes `batch-NNN-prepared.json` before each batch and
`batch-NNN-result.json` after registry reconciliation. Both conform to
`distribution/crates-io/receipt-schema-v1.json` and bind the source commit/tree, Cargo and Rust
toolchains, publish graph, manifest bytes, `.crate` bytes, and observed registry checksum. A batch
result of `pending_recovery` or `mismatch` blocks every dependent batch. Recovery re-packages the
same source and skips only exact checksum matches. A rerun consumes the prior attempt artifact; a
new manual recovery uses the same immutable release tag and supplies its prior workflow id through
`recovery_run_id`. Source, tree,
toolchain, publish plan, manifest, and `.crate` identity must all agree before recovery continues,
and the publisher never yanks automatically.

## Pre-Alpha.3 Package Evidence Snapshot

The table below is historical pre-publication evidence captured on 2026-07-03. Query crates.io or
the owning artifact release directly for current availability; do not infer it from this snapshot.

| Crate | Gate | Current result |
| --- | --- | --- |
| `dugong-graphlib` | crates.io lookup | Published |
| `manatee` | crates.io lookup | Published |
| `merman-core` | crates.io lookup | Published |
| `merman-elk-layered` | crates.io lookup | Published |
| `roughr-merman` | release workflow skip-if-published check | Versioned separately as `0.12.1`; publish only when that crate version is not already visible |
| `dugong` | crates.io lookup | Published |
| `merman-analysis` | release workflow dry-run before publish | Pending after `merman-core` is published |
| `merman-ascii` | `cargo publish -p merman-ascii --locked --dry-run --allow-dirty --registry crates-io` | Pass locally; not yet published |
| `merman-layout-elk` | crates.io lookup | Published |
| `merman-editor-core` | release workflow dry-run before publish | Pending after `merman-analysis` is published |
| `merman-render` | `cargo publish -p merman-render --locked --dry-run --allow-dirty --registry crates-io` | Pass locally after release-source fix; not yet published |
| `merman-export` | release workflow dry-run before publish | Pending after `merman-render` is published |
| `merman` | release workflow dry-run before publish | Pending after `merman-export` is published |
| `merman-lsp` | release workflow dry-run before publish | Pending after `merman-editor-core` and `merman` are published |
| `merman-bindings-core` | release workflow dry-run before publish | Pending after `merman` is published |
| `merman-cli` | release workflow dry-run before publish | Pending after `merman` is published |
| `merman-rustdoc` | release workflow dry-run before publish | Pending after `merman` is published |
| `merman-ffi` | release workflow dry-run before publish | Pending after `merman-bindings-core` is published |
| `merman-typst-plugin` | release workflow dry-run before publish | Pending after `merman-bindings-core` is published |
| `merman-uniffi` | release workflow dry-run before publish | Pending after `merman-bindings-core` is published |
| `merman-wasm` | release workflow dry-run before publish | Pending after `merman-bindings-core`, `merman-editor-core`, and `merman` are published |

## Publish Guardrail

Do not run `cargo publish` as part of an implementation lane unless the release operator explicitly
requests it. `tools/publish.py` computes the current order, and `release-crates.yml` is the normal
caller of `scripts/crates_io_release.py publish-receipted`. Do not invoke that command locally merely
to test it; focused unit tests own response-loss, checksum-mismatch, and partial-recovery behavior.
