# Publish Order

Status: maintained workspace publish order, with explicitly dated historical channel snapshots.
Last updated: 2026-10-12 (Asia/Shanghai)

Use the [versioned upgrade index](README.md#workspace-upgrades) for consumer migrations and [Releasing](RELEASING.md) for current publication and recovery procedures. Snapshot sections below preserve the versions and observations from their stated dates; they are not instructions to repeat those uploads.

## Version Decision

Selected preparation target: `0.9.0`; latest verified published Rust workspace and CLI/LSP release: `0.8.0`. Independent prerequisites and channels are tracked in the [0.9.0 preparation record](#090-preparation). The broader theme refactor remains deferred; start with the [stable upgrade guide](V070_TO_V080_UPGRADE_GUIDE.md) or [alpha.7 delta](ALPHA7_TO_0_8_0_UPGRADE_GUIDE.md).

## 0.9.0 preparation

Status on 2026-10-12 (Asia/Shanghai): workspace 0.9.0 is selected for preparation only. The last verified stable workspace release is v0.8.0, published on 2026-10-06. No 0.9.0 tag, registry acceptance, artifact publication, or successful immutable preflight is asserted by this record. Installation examples describe the target contract; publication evidence remains owner-specific.

The source delta starts at `v0.8.0` (`2029bd547077b1a6800878e125663633d81e6d87`) and includes reviewed `origin/main` commit `745d223512591bc4964cf1d4e16cf2ae8e8f44bc` plus [release-preparation PR #194](https://github.com/Latias94/merman/pull/194). Freeze and record the final 40-character source SHA only after all owner changes and packaged documentation are complete. The lightweight documentation evidence ledger is under `target/release-docs-0.9.0/`; it is not a publication receipt.

| Surface | Selected target | Preparation and delivery boundary |
| --- | --- | --- |
| Independent rough drawing crate | `roughr-merman 0.12.4` | Private host-random upgrades preserve the public legacy svgtypes 0.11 API. Owner SemVer and fresh published-consumer evidence are required; independently publish and confirm crates.io visibility before workspace dependents. |
| Independent grammar | `tree-sitter-mermaid 0.3.1` / `@mermanjs/tree-sitter-mermaid 0.3.1` | Exact CLI/Rust/Web 0.27.1 contract, compatible language adapter `^0.1.9`, and generated language ABI 15. Verify generated parser, root WASM, package/provenance, and owner smoke. Independently publish the required Cargo package before workspace dependents; npm and GitHub evidence remain separate. |
| Workspace Rust / CLI / LSP | `0.9.0` / intended `v0.9.0` | Version projection is prepared. Changelogs remain `Unreleased`; dependency/legal checks, frozen-source preflight, packaging, and publication receipts are required. |
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
| Playground dependency migration | A real npm 12 installation passed npm ls and audit with no findings. TypeScript and Vite prepared builds, final dist isolation checks, and 446 owner tests passed; the prepared CSP check also passed. Its local frontend input build does not establish a fresh release Rust-WASM receipt. |
| Grammar generation and package consumers | SDK 34 and CLI 0.27.1 produced a byte-identical regeneration with ABI 15. Corpus 259/259, generation tests 9/9, native Node, Web-WASM, C, and packed package consumers passed; the freshly packed Cargo consumer also passed on pinned Rust 1.95.0; registry availability remains separate. |
| Native CLI format migration | The selected 0.9.0 contract removes native `render -e` / `batch -e`; migrate to `-f/--format`. The pinned `mmdc -e/--outputFormat` remains supported. Alias-removal behavior is covered by the passed CLI contract selection. |
| Native, WASM, size, and package-content checks | Require final-source owner results; compilation and static declarations do not replace these artifact checks. |
| Immutable preflight and release | Not recorded. Keep selected changelogs `Unreleased` until the approved intended tag date is set; rerun evidence after the final source freeze. |

See the [0.8.0 upgrade guide](V080_TO_V090_UPGRADE_GUIDE.md), [Typst preparation guide](TYPST_050_PREPARATION.md), and [owner checklist](RELEASING.md#090-preparation-checklist). Dated publication snapshots below retain their original release context.

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

Version `0.3.0` is the first Typst package rebuilt after the text-measurement closure reduction. ICU4X collation data and generated font-metric tables are no longer linked into the production artifact; the plugin keeps deterministic measurement and the existing ABI 2 exports. The package version changes because the shipped implementation closure and size characteristics are materially different, while the wrapper protocol remains compatible.

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

The `merman-doc` implementation crate joined the coupled graph in `0.8.0-alpha.7`, bringing it to 21 packages. Both `merman-cli` and `merman-rustdoc` depend on it, so Cargo metadata places it before either consumer without a separate workflow or a hand-maintained package list. Future releases must continue to verify the derived graph and each generated `.crate` through the release preflight and per-batch publication dry runs.

`roughr-merman` is versioned separately as `0.12.3`. The workflow reads each crate's own package
version, so it can skip already-published crates while still keeping one dependency-ordered list.

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
