---
type: Verification Evidence
title: C7a transport catalog contract follow-up
timestamp: 2026-09-15
related_plan: docs/plans/2026-09-15-theme-c7a-c7b-replan.md
git_branch: refactor/presentation-theme-model
git_commit: a09453da0
tags: theme,c7a,transport,catalog,verification
---

# Scope and result

The current source revision passed the focused public catalog and qualification contract suites
without changing fixtures or generated outputs. This is a same-source contract observation; it is
not a claim that every packaged platform artifact has been rebuilt, and it does not freeze C7a.

# Executed checks

- `python3 -m unittest scripts.test_qualify_theme_presets scripts.test_theme_preset_catalog_contract` — 13/13 passed.
- `node --test platforms/node/tests/api-contract.test.mjs platforms/web/scripts/theme-catalog.test.mjs` — 48/48 passed.
- `dart run tool/abi3_contract_test.dart` from `platforms/flutter` — passed with `ABI 3 Dart contract tests passed`.
- The post-ARM64 focused archive/catalog and package-group suite — 91/91 passed; its command and
  log are recorded in the ARM64 archive verification record.

The qualification tests reject unknown profile, family, output, and admission identifiers before
projecting a qualified catalog. Node and Web retain open metadata where their transport contract
allows future identifiers while preserving known profile behavior. Flutter's ABI 3 consumer
contract also preserves future profile/admission catalog cells and rejects malformed or conflicting
conditions. These transport tests cover metadata handling, not an end-user portability decision.

These checks confirm the existing transport metadata and qualification-projection contracts. Packaged Web,
Node, Flutter, Typst, UniFFI and Native C ABI artifacts still require their final same-source owner
matrix and release-preflight receipts before C7a contract freeze. The selected next workspace release is `v0.8.0-alpha.7`; the candidate remains unfrozen
while the alpha.6 compatibility decision and final artifact matrix are open.


# Execution limits

Logs: `/tmp/merman-c7a-qualification-contracts.log`,
`/tmp/merman-c7a-transport-catalog.log`, and `/tmp/merman-c7a-flutter-abi3.log`.
Initial Flutter attempts used `dart test`, which is not this repository's runner and failed
before executing tests. The documented `dart run` command above succeeded; no test dependency
was added. No Rust test execution is attributed to these Python/JavaScript/Dart commands.

The local machine has Xcode 26.5 and Swift 6.3.2, with no additional Xcode installation or Swift
toolchain under the inspected standard application/toolchain directories. It cannot establish
the required Swift 5.9/Xcode 15.2 floor. A read-only GitHub run query for
`3fe3b1b970a1d5ae211a1f05510adb7f03efa4ba` returned no runs; there is no current-source hosted
preflight result to consume. Formal release preparation still needs the maintainer's version
selection and the subsequent immutable-source platform jobs.

# Theme acceptance rerun at `5980cc9ec`

At `5980cc9ec71b0a1c5181cde0f7816c7c8d935b24`, the four Cargo invocations ran sequentially with the workspace acceptance
configuration; tests within each invocation used nextest's default concurrency. Results were:

- C6 runtime, preset qualification, Block title retirement, Class edge-label-background retirement
  and Flowchart marker retirement: 10/10 passed, zero skips. Nextest marked one passing C6 test as
  `LEAK`; its cause was not investigated in this run. Passing assertions do not resolve that
  marker, and the marker alone does not establish a production memory leak.
- KTD23 legacy projection retirement: 4/4 passed, zero skips.
- KTD17 route-cutover runtime: 1/1 passed in 334.152 seconds. This is a slow existing authorization
  test, not a timeout or a failure.
- Renderer support discovery: 48/48 passed, zero skips, including unknown catalog identifiers
  remaining visible and resolving to `Unverified`.

Log: `/tmp/merman-c7a-current-retirement.log`. These results revalidate the merged Block/provider
retirement and support-discovery behavior at the named source. They do not replace the final
same-source platform matrix, selected release line, immutable preflight, or C7a contract freeze.

# C6 leak-marker recheck

The passing C6 test that received a nextest `LEAK` marker in the ten-target run was isolated twice.
The acceptance-configured `cargo test` invocation passed the test in 4.92 seconds with `png`; the
same acceptance-configured nextest invocation with the original `png,layout-cytoscape` feature set
passed in 5.00 seconds without a `LEAK` marker. The first direct `cargo test` attempt was not used
as evidence because it omitted the workspace acceptance cfg and consequently ran zero tests.
Logs: `/tmp/merman-c7a-c6-cargo-test-acceptance.log` and
`/tmp/merman-c7a-c6-original-features-recheck.log`.


# Test-only helper boundary follow-up

On the working tree based on `a6b5d75e2`, a repository-wide Rust reference check identified
four private helpers whose callers exist only in unit-test configurations:
`FamilyThemeProgram::resolve_text_style`, `SequenceConfigView::resolve_role_typography`,
`CssFontSizeContext::with_medium_px`, and `OperationWorkMeter::preflight_parsed_render`.
Each helper now uses `#[cfg(test)]`, matching its consumers. Their implementations and test
callers remain intact; production continues to use the existing metered and typed-property
paths. This is a build-boundary cleanup, not a measured runtime or artifact-size improvement.
Qualification metadata and historical acceptance helpers were not deleted merely because
an ordinary production build reports them as unused.

Validation on this working tree:

- `cargo fmt --all -- --check` and `git diff --check` passed.
- `CARGO_BUILD_JOBS=1 cargo nextest run --locked -p merman-render --lib --no-default-features -E 'test(diagram_theme::resolved::tests::) | test(sequence::config::tests::) | test(mermaid_style::tests::) | test(resources::tests::)' --test-threads 2` passed 72/72 selected tests; 2,483 tests were filtered out.
- `CARGO_BUILD_JOBS=1 cargo check --locked -p merman-render --no-default-features` passed and no longer reports these four unused methods. Other warnings remain; this is not a complete dead-code audit.

Logs: `/tmp/merman-c7a-test-helper-scope.log` and
`/tmp/merman-c7a-test-helper-production-check.log`.

The remaining Apple compiler-floor gate has a separate owner: `.github/workflows/ci.yml`
job `apple-swift-5-9-smoke` selects Xcode 15.2 and verifies Swift 5.9 before building and
checking the generated source. The `apple-xcframework` job in `release-preflight.yml` uses
`macos-15` without that compiler-floor selection. A passing preflight Apple build alone
cannot close the Swift 5.9 obligation. This is an inspection of workflow responsibilities,
not evidence of a new hosted run.

The release-contract decision recorded in the release-line assessment remains pending.
No compatibility exception, package-line change or C7a freeze was applied by this cleanup.

# Current candidate acceptance rerun at `88b23ad77`

The release-mode theme acceptance targets were rerun sequentially with the workspace acceptance
configuration and `png,layout-cytoscape`: C6 runtime, preset qualification, legacy projection
retirement, Block title retirement, Class edge-label-background retirement, Flowchart marker
retirement and route-cutover runtime. Nextest ran 15 tests across 7 binaries; all 15 passed with
zero skips. This revalidates the completed Block/provider retirement and current manifest digest;
it does not close the alpha.6 compatibility decision or the missing hosted artifact matrix.

# Cross-transport current-source rerun at `de9c3f5f1`

The current source reran the cross-transport contract lanes without changing fixtures:

- Node API and Web catalog tests: 48/48 passed.
- Python qualification, catalog and CLI archive contract tests: 67/67 passed.
- Rust bindings-core authoring/error/resource vectors plus Typst, Native C ABI and UniFFI shared support golden: 15/15 passed, with 393 unrelated tests filtered.
- Flutter ABI 3 contract runner (`dart run tool/abi3_contract_test.dart`): passed with `ABI 3 Dart contract tests passed`.

These are current-source contract and golden checks. They do not establish installed package
provenance, Windows execution, Apple Swift 5.9 execution, or the unresolved alpha.6 facade
compatibility contract.

# Current artifact/profile contract rerun at `9e692c3c4`

The artifact and archive contract suites passed 148/148 with
`python3 -m unittest scripts.test_artifact_profile_recipe scripts.test_verify_artifact_dependency_closures scripts.test_release_artifact_bundle scripts.test_verify_cli_release_archive scripts.test_verify_lsp_release_archive`.
This covers descriptor validation, dependency-closure checks, bundle contracts, and CLI/LSP
archive verifier positive and negative cases. It validates the owner tooling; it is not a
substitute for building and executing every final alpha.7 archive on its declared host.

# Current Typst artifact rebuild at `9e692c3c4`

The existing Typst publish artifact initially failed smoke because its manifest still carried
`0.8.0-alpha.6`. It was rebuilt through the owner command
`cargo run --locked -p xtask -- build-typst-package --profile publish`, then verified with the
Typst dependency-closure checker and `typst-package-smoke --profile publish --skip-wasm-build`.
The rebuilt package passed 22 shared support vectors, 2 materialization vectors, 3 structured
error vectors, all positive Typst examples, and all expected compile-fail examples. The size
matrix passed with raw 18,784,145 bytes, stripped 11,612,278, gzip 4,454,338, and Brotli
3,287,112 bytes. The runtime dependency closure contained 115 packages and passed.

The generated artifact is local release evidence only; it does not establish the missing Windows,
Swift 5.9, or alpha.6 facade compatibility gates.

# Current Web artifact rebuild and prepack rerun at `9e692c3c4`

The five Web WASM profiles were rebuilt from the current alpha.7 inputs with the repository's
local `CARGO_PROFILE_WASM_SIZE_LTO=false` workaround. `verify-wasm-inputs` then accepted all five
fresh provenance records. Package assembly initially exposed that the Web prepack verifier did
not enable the existing npm 11 array-metadata compatibility in `npmPackRecord`; this was fixed by
passing `allowNpm11: true`, with a focused regression test. After assembly, package verification,
all five installed-package smoke cases, DOM safety smoke, and the Web size matrix passed.

The final Web matrix measured stripped artifacts of 3,674,593 (analysis), 5,201,706 (ascii),
3,785,732 (editor), 15,937,402 (full), and 14,036,313 (render) bytes; the configured budget
check passed. This is current-source local artifact evidence; hosted Windows and Swift 5.9
checks and the alpha.6 facade compatibility decision remain open.

# npm 11 package-contract follow-up at `bfd4634f0`

The Web prepack fix exposed two Node assembled-package contract tests that used the same shared
`npmPackRecord` helper without opting into npm 11 array metadata. The Node test helper now passes
`allowNpm11: true`, matching the production Node verifier and Web prepack verifier. The complete
current Web/Node script set (all Web tests, Node platform-command and package-contract tests)
then passed 153/153 under npm 11.18.0. No package contract or runtime behavior was weakened.

# Current release/workflow contract rerun at `1ad432247`

After the npm 11 metadata fixes, the current Web/Node focused suite passed 16/16 (Web prepack
and all selected Node package/platform contract tests). The broader release workflow, CI-plan,
release-surface and artifact-bundle contract suite passed 97/97. Historical alpha.6 and
“version not selected” statements in the replanning document remain confined to explicitly
labeled historical evidence sections and were not relabeled as current candidate evidence.

# Installed alpha.7 Node N-API consumer at `29ebca68f`

The owner build command `npm run build:candidate --prefix platforms/node -- --candidate napi
--target darwin-arm64` rebuilt the addon from
`29ebca68f457d0a66b194aa83fa93750c79e3c4b`. The recorded source digest is
`sha256:f673e61afe2c36cfca890987ff4fd7f9e839b229836accc81291e123a2b1ec25`.
The 24,548,080-byte `merman.node` has SHA-256
`b91c13fb5df1a190adbf58c857fe067f57801e3d319866172fe6bd4762058d78`.
Its build receipt has SHA-256
`6e17eaed6a3b1635900cf79fff7d0daaf2f24d987570296ab1ec14d6deeb08c5`.
The host used Node 26.6.0, npm 11.18.0, Rust 1.95.0 and N-API CLI 3.7.4.

The existing assembly owner validated the receipt and assembled the loader plus darwin-arm64
package into a temporary directory. The packed-root verifier passed. Both packages were packed
with npm and installed offline with lifecycle scripts disabled into an empty ESM project.
`smoke-installed-package.mjs` resolved the installed public entrypoint and confirmed both package
and runtime versions were `0.8.0-alpha.7`. Rendering and the authoring witness passed: 2 shared
vectors, 2 catalog checks, 3 family isolation checks, 1 rule override, 1 cold spec, 3 preset exports,
44 support queries, 6 authoring diagnostic checks, 2 resource limit checks, 23 JSON operations and
23 SVG renders. These counters describe this witness, not independently qualified catalog cells.

The installed project is retained at
`/var/folders/zk/87rg5ff15mlfnplph83p5ntm0000gn/T/merman-alpha7-node-installed-o5sook8z/project`.
This closes the local macOS ARM64 N-API installation tranche at the named source. Node WASM,
other native targets, CLI/LSP archives and final same-source release preflight remain separate
checks. The alpha.6 facade compatibility decision is still pending; no package was published.

# Installed alpha.7 Node WASM consumer at `7b60b00fb`

The owner command `npm run build:candidate --prefix platforms/node -- --candidate node-wasm`
rebuilt the Node WASM candidate from the current alpha.7 source. The assembled
`@mermanjs/node-wasm@0.8.0-alpha.7` package passed packed-root verification, was installed
offline into an empty ESM project, and passed `smoke-installed-package.mjs` through its public
entrypoint. The witness rendered SVG and completed the same authoring checks as the native
Node consumer: 2 shared vectors, 2 catalog checks, 3 family isolation checks, 1 rule override,
1 cold spec, 3 preset exports, 44 support queries, 6 authoring diagnostics, 2 resource limit
checks, 23 JSON operations and 23 SVG renders.

This closes the local Node WASM installation tranche. It does not close other native hosts,
CLI/LSP archives, hosted compiler floors, or the unresolved alpha.6 facade compatibility
contract. No package was published.

# Local alpha.7 CLI/LSP archives at `6beea1497`

`cargo-dist 0.32.0` built the `aarch64-apple-darwin` CLI and LSP archives for
`v0.8.0-alpha.7` from the current workspace. `verify_cli_release_archive.py --execute`
passed archive structure, checksum, version, capability, completion, SVG, PNG, JPEG, PDF and
rustdoc checks. `verify_lsp_release_archive.py --execute` passed archive structure, checksum,
version and the native stdio initialize/shutdown lifecycle. Verified copies were written under
a temporary directory with archive names preserved.

This is a local macOS ARM64 archive witness. It does not establish Linux/Windows execution or
replace the unresolved alpha.6 facade compatibility decision.

# Release preparation contract rerun at `aa397206a`

`python3 scripts/verify_release_changelog.py --version 0.8.0-alpha.7` passed in preparation
mode. The immutable `--require-date` form correctly remains blocked while the candidate
changelogs are `Unreleased`, as required by the release guide; no release date was invented.
The release surface, workflow security and prerelease compatibility contract tests passed
48/48. The compatibility checker still reports the known alpha.6 previous-facade failure and
therefore C7a remains open.

# Candidate ancestry and version authority audit at `b10576aae`

After refreshing `origin/main`, `origin/main` remains an ancestor of the candidate (`4c2ac7817`),
so no main merge is pending. `release-version.py check --version 0.8.0-alpha.7` passed across
the workspace, Cargo locks, Node/Web/Playground projections, Python, Android and Flutter
surfaces. Preparation-mode changelog validation passed. The worktree contains only the two
pre-existing untracked knowledge directories; no generated or user-owned files were staged.

# Cargo-dist artifact-plan audit at `2eb8e15a5`

The generated cargo-dist plan for `v0.8.0-alpha.7` was checked with
`release_artifact_bundle.py verify-plan` for the local `aarch64-apple-darwin` lane and the
configured native runner `macos-15`. The plan contains both CLI and LSP archive families and
routes this target to a native, non-container job. The corresponding local archives already
passed structural and executable replay checks. Other matrix rows remain hosted evidence and
were not inferred from this local run.
