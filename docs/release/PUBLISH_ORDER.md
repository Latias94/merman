# Publish Order

Status: maintained workspace publish order, with explicitly dated historical channel snapshots.
Last updated: 2026-10-22 (Asia/Shanghai)

Use the [versioned upgrade index](README.md#workspace-upgrades) for consumer migrations and [Releasing](RELEASING.md) for current publication and recovery procedures. Snapshot sections below preserve the versions and observations from their stated dates; they are not instructions to repeat those uploads.
## Version Decision

Latest verified workspace release across CLI/LSP, Rust crates, Web, Node, Python, Flutter, Android, and Apple: `0.9.2`. Typst wrapper 0.5.0 and the other independent surfaces retain separate publication boundaries.

The `main` development checkout synchronizes the `0.9.1` package version baseline and published
notes. Its `[Unreleased]` section retains subsequent development, including compiled themes and
Mermaid `12.2.0`; package metadata alone does not make that source the published patch. Published
artifacts continue to bind the immutable source recorded below. Choose the next workspace version
before preparing another development-line release.

Maintenance release `0.9.2` comes from `release/0.9.x`, based on immutable `v0.9.1`. The branch rename preserved merged PR [#235](https://github.com/Latias94/merman/pull/235), commit `ae3299f9a9db517e91c808e63a67cb8d30a1ce37`, whose implementation tree equals its green reviewed head `8d27e9e4f8aa35b7095b45a728f5953538cde438`. Follow-up [#237](https://github.com/Latias94/merman/pull/237) additionally selects parser/CSS repairs from #227, the remaining UTF-8 recovery and Python wheel fixes from #178, the #210 workspace/Typst publisher gate, and the independent incremental-fuzz oracle repair. The compiled-theme development on `main` remains outside this release. See the [backport review](V092_BACKPORT_READINESS.md) for outcome/commit/reference mapping and exclusions. PR #237 passed its own CI before merging; publication uses the merged source and full immutable preflight recorded below.

Maintenance dependency refresh on 2026-10-22: #237 reruns `cargo update` for the workspace, fuzz, and standalone Node locks, with no manifest/API/baseline migration. The ordinary locked workspace dependency policy and both auxiliary all-features advisory checks passed against freshly fetched RustSec revision `4b587f00996ebe5c05a73eaef105ef241e876414`. The two governed font-stack maintenance advisories remain; a supplemental workspace all-features check also reports the optional UniFFI generator's unmaintained `basic-toml` dependency, with no compatible fixed release. The [backport review](V092_BACKPORT_READINESS.md#dependency-review-on-2026-10-22) records exact versions and supplemental policy limitations; do not describe this as an exception-free all-features audit.

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
| Release declaration gates | `verify_release_changelog.py` passed 0.9.0 preparation admission; `release_surface_contract.py` passed the five static FFI/native surface declarations. Dated preflight and real artifacts remain separate gates. |
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

See the [0.8.0 upgrade guide](V080_TO_V090_UPGRADE_GUIDE.md), [Typst preparation guide](TYPST_050_PREPARATION.md), and [owner checklist](RELEASING.md#090-preparation-checklist). Dated publication snapshots below retain their original release context.
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

For a future Typst submission, bind the package to the reviewed 40-character source SHA and run the owner gates from `docs/release/RELEASING.md`: `verify-typst-profile-constants`, the `typst-wasm` dependency-closure check, the Typst size matrix, `build-typst-package --profile publish`, and the full Typst package smoke. Inspect the private artifact manifest under `target/typst-wasm-artifacts/` together with `LICENSE`, `THIRD_PARTY_NOTICES.md`, and `THIRD_PARTY_LICENSES/` in the staged package. The provenance manifest is a preflight input, not a runtime package file. Before a future submission, finish and inspect the package README with final import and usage examples. After acceptance, query the registry for the exact package and update this release record with observed publication evidence.

## Publish Order

Cargo metadata is the publish-order authority. Inspect the current dependency-safe projection with:

```bash
python3 tools/publish.py --list-crates-io-packages
```

The helper selects crates.io-publishable `workspace_members`, excludes names listed in
`workspace.metadata.merman-release.independent-packages`, follows every non-dev workspace path
dependency (including optional, target-specific, renamed, and build dependencies), rejects a
publishable crate that depends on a private workspace member, and topologically sorts the graph.
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
