---
type: Verification Evidence
title: Post-merge C7a artifact and consumer validation
timestamp: 2026-09-15
related_plan: docs/plans/2026-09-15-theme-c7a-c7b-replan.md
git_branch: refactor/presentation-theme-model
git_commit: 835d1a8c4d0651364d710f9c01a992e0a214410a
tags: theme,c7a,artifacts,bindings,verification
---

# Scope

The candidate checkout at `/tmp/merman-c7a-fde874d51` now points to `835d1a8c4`.
Its directory name identifies the initial checkout, not its current revision. This is an
unpublished development build using the workspace's existing `0.8.0-alpha.6` version;
it is not the published alpha.6 artifact and does not declare C7a contract freeze.
Cargo builds run sequentially and reuse the primary worktree's target directory. Node
uses the release toolchain, Node 24.21.0 and npm 12.0.2, installed under `/tmp/merman-c7a-tools`.

# Repaired gate failures

Ordinary Node CI still installed npm 11.17.0 after the merge, while package ownership
checks consume npm 12's named JSON records. The actual Node suite reproduced three
package failures with npm 11.18.0 (104/107 passed). Running the unchanged suite with
npm 12.0.2 passed 107/107. Commit `d3ff6a558` aligns CI with the release toolchain and
adds a regression against the package-manager and release pins. The new regression
failed against the old CI pin; the complete fixed Node suite passed 108/108.
Workflow contracts passed 87 tests; actionlint 1.7.12 and zizmor 1.29.0 passed on the
changed workflow. Zizmor used its default offline mode and existing suppressions.

The first-release qualification schema change also left schema 1 in an invalid-version
fixture. The optimized qualification unit gate reproduced 7 passes and one failure.
Commit `c2ad43c8d` replaces that invalid set with zero and the next unsupported revision.
All eight optimized tests then passed, including stale recipe/resource rejection,
wrong-preset artifacts, and the Flowchart/State/Sequence terminal rejection cases.
Rust formatting and diff checks passed.

# Installed Node artifact

The clean `c2ad43c8d` candidate built the `napi` / `darwin-arm64` recipe using the existing candidate
builder and receipt probe. The owner assembly and package verification scripts passed.
The loader and native packages were packed, installed into a new consumer directory,
and exercised through `smoke-installed-package.mjs`; no source-workspace loader was used.

| Tarball | Bytes | SHA-256 |
| --- | ---: | --- |
| `mermanjs-node-0.8.0-alpha.6.tgz` | 112781 | `3932ac23ffe280fc1d3730f654165d01cd02fc15c8a37a2cca195c9e81244b95` |
| `mermanjs-node-darwin-arm64-0.8.0-alpha.6.tgz` | 11067572 | `1313280d25c2d2f887df4ca53a6c1235d4908544141d6dcff259772b0f6e50ef` |

The installed consumer passed two shared definition vectors, two catalog checks,
three family-isolation checks, one rule override, one cold complete spec, three preset
exports, 44 support queries, six authoring diagnostics and two resource-limit checks.
It executed 23 JSON operations and 23 SVG renders. Support/error vectors ran through
both asynchronous and synchronous entry points. The package remains an SVG-only native
Node profile; this does not claim PNG support or qualify the shared preset catalog.

The artifacts and independent installation are under `/tmp/merman-c7a-c2ad43c8d-node-*`.
Logs use `/tmp/merman-c7a-c2ad43c8d-node-{build,assemble,verify,pack-loader,pack-platform,install,installed-smoke}.log`.

# Web package validation

At the same `c2ad43c8d` source, all five Web WASM artifacts and npm packages built
successfully. The generated TypeScript contract check covered 38 WASM exports,
48 runtime bindings and five package entries. The Web script suite passed 142 tests.
Package-input freshness and five-package ownership checks passed, and each package's
actual WASM smoke passed.

The aggregate mixed-package smoke then exposed another obsolete schema-3 assertion for
an analysis-only package's empty theme catalog. Its actual schema-1 output was correct.
Commit `238ed5f84` updates that expectation. Running the full smoke with this exact
one-line change passed all five packages, same-process loading, and DOM safety. No
runtime or WASM source changed. The candidate then advanced to `238ed5f84` with a clean status. Input freshness
and all twenty Web size checks passed again against the unchanged package bytes.

# Typst package and artifact budgets

Typst's real package validator still expected theme catalog schema 3. After correcting
that expectation, package compilation exposed stale schema-2 authoring examples, the
removed flat support query, and an ABI-4 test expectation. The contract checks, example and
README now use subject-based query schema 1, catalog schema 1 and published-baseline
successor ABI 3. The success/error envelope behavior is unchanged.

The package smoke also used the workspace as Typst's project root while staging its
inputs under the Cargo target directory. An external shared target made those inputs
fall outside the project root. The project root now points to the self-contained smoke
run directory (`835d1a8c4`); the consumer migration is `94aab6f90`. Real compilation with the pinned Typst 0.15.0 passed 22 documents and
nine expected failures; the embedded WASM also passed 22 support vectors, two
materialization vectors and three shared errors.

A shared-target cache initially reused an `xtask` built for the primary worktree.
Those runs are not clean-checkout evidence. Touching the candidate's unchanged `xtask`
entry source forced the tool to compile for `/private/tmp/merman-c7a-fde874d51`; its log
then identified that checkout's package, with smoke files in the external shared target.
That run passed all 22 positive and nine negative fixtures. The focused optimized
Typst tool tests also passed 22/22. No source bytes were changed
to invalidate the tool cache, and no cache-management framework was added.

Web and Typst retain the existing budgets: all 24 checks passed. Measurements in bytes:

| Profile | Raw | Stripped/post-link | Gzip | Brotli |
| --- | ---: | ---: | ---: | ---: |
| web-analysis | 3674891 | 3674626 | 1412128 | 1073799 |
| web-ascii | 5204673 | 5204408 | 1918026 | 1448699 |
| web-editor | 3786023 | 3785758 | 1456716 | 1107495 |
| web-full | 15934976 | 15934711 | 5941880 | 4373202 |
| web-render | 14044886 | 14044621 | 5295323 | 3903397 |
| typst-wasm | 18764770 | 11607520 | 4448213 | 3285165 |

Budget logs are `/tmp/merman-c7a-238ed5f84-{web-budget-clean,typst-budget}.log`.
The corrected external-target run is `/tmp/merman-c7a-typst-rebuilt-tool-smoke.log`.
The staged Typst plugin is 11607520 bytes, SHA-256
`0943c799a7762806df13d32a85ff81289ec3199716faf61443063adde707e2b4`.

# Archive and qualification progress

The clean predecessor `d3ff6a558` first built both macOS ARM64 cargo-dist archives. CLI runtime
verification, 18 preset target observations, exact CLI output matching and full record
replay passed; LSP archive execution passed. Three recipes each received six scoped cells
under `native-flowchart-state-sequence-system-fonts-v1`, all `host_dependent`. These
records remain bound to `d3ff6a558`; they were not relabeled as final-candidate evidence.
The private record is `target/preset-qualification-d3ff6a558.json` in the candidate checkout.

After all fixes were committed, the clean `835d1a8c4` checkout reran cargo-dist and
issued a new record over the actual extracted CLI. All 18 output digests matched again,
and complete replay passed. The companion gives six cells each to Brutalist, Spotless
and Cyberpunk; the other seven presets keep empty qualification scope. Git status was
empty after collection and replay. Logs are `/tmp/merman-c7a-835d1a8c4-{dist,qualification,replay}.log`.

| Final artifact | Bytes | SHA-256 |
| --- | ---: | --- |
| `merman-cli-aarch64-apple-darwin.tar.xz` | 13495852 | `9fa7e07302e69390648adc10f965d26c4b5719da8d969032187eb2b67ecb7f97` |
| `preset-qualification-835d1a8c4.catalog.json` | 14640 | `629bf66cfa9703ec576c89015c73cdb081ebf81bd65fda33c46aa0929686daa2` |

At `c2ad43c8d`, the qualification, CLI archive, release bundle and preset catalog Python
contract suites passed all 87 tests. These cover rejected stale records and wrong artifact
bindings; their fixture observations do not replace native archive execution.

# Remaining evidence

The remaining platform owner matrix is not yet claimed by this record. Installed
Node-WASM, the final Python wheel, full workspace regression, optimized retirement
revalidation and the complete browser/host execution matrix remain open. Linux/Windows
execution and C7a public rollout/contract closure still need their owner evidence. Preserve the completed Block retirement and its independent
historical authority; no new bridge implementation is part of this verification work.
