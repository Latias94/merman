---
type: Audit Report
title: Alpha.7 presentation theme audit and C7a boundary
timestamp: 2026-09-20
git_branch: refactor/presentation-theme-model
source_commits: b2c1d805c, 7f35c9080, 0f5125e75, 0c1b1047a7453b5668e371da38019b20b9c66815, ff9b9991bb91327b1630f326bc389ccc3bdb89fa, d529f858e, 4e03b93e7
related_plan: docs/plans/2026-09-16-1618-refactor-theme-product-boundaries-plan.md
tags: theme,audit,c7a,alpha7
---

# Decision

This record is a current-source audit, not a release approval. The completed evidence is enough
to retain the complete Cyberpunk recipe and its bounded native qualification profile, but C7a
remains open. No tag, publication, push or public catalog promotion was performed.

The strongest current result is a clean-source native qualification replay plus installed Python,
Node Darwin ARM64, Node WASM and Web package consumer exchange. The evidence is execution-local and host-bound:
macOS ARM64, system fonts, selected SVG/PNG scenes and the installed consumer profiles described
below. It does not establish the release matrix or the U10 cost gate.

# Completed evidence

The [complete-scene record](2026-09-19-cyberpunk-controlled-scenes.md) replaces the obsolete
palette-only Cyberpunk qualifier with `native-cyberpunk-full-scenes-system-fonts-v1`. The unchanged
Flowchart, Sequence and XY Chart fixtures pass the sealed SVG checks and 110 native PNG/PDF pixel
contribution probes. The profile produces six execution-local HostDependent cells. The production
CLI matches all 18 scoped SVG/PNG observations, and a detached clean checkout records and replays
the same qualification object byte-for-byte. The record binds source `a4a04ef1b`, the lockfile,
qualification executable, CLI and scene record digests.

The [installed-consumer record](2026-09-19-installed-theme-recipe-consumers.md) rebuilds the Python
wheel, Node Darwin ARM64 package and Node WASM package from clean source `0c1b1047a`. The direct
versioned recipe is accepted after JSON save/load, complete Cyberpunk files move between separate
Python and Node processes, and all three transports render identical SVG bytes for Flowchart,
Sequence and XY Chart. Node contracts pass 109/109 under Node 24.21.0/npm 12.0.2; focused Python
owner tests pass 27/27. The same tranche repairs the standalone Node lock and regenerates legal
projections without changing the intended capability recipe.

The current source acceptance replay also passes
`cargo nextest run --locked -p merman-theme-authoring-fixtures -p merman-theme-acceptance`:
three public Cyberpunk Flowchart, Sequence and XY Chart recipe round-trip tests passed.

The current HEAD refresh reran the broader semantic and acceptance lanes with
`cargo nextest run --locked -p merman-theme-fixtures -p merman-theme-acceptance`: 70 tests passed,
two intentional skips remained, and the three public Cyberpunk round-trip cases passed again.
Nextest also marked `enforcement_and_portable_expectation_invariants_fail_closed` as leaky;
that process-cleanup observation has not been diagnosed and is not counted as a clean shutdown.

The release-mode support discovery gate also passes
`cargo nextest run --release --locked -p merman-render --lib --test theme_support_discovery_test`
with the support-manifest selection: 58 tests passed and 2,520 unrelated tests were skipped.

The current HEAD refresh reran that release gate without the historical selection summary:
2,577 tests across the renderer library and support-discovery integration binary passed and two
intentional skips remained. The gate continues to
keep target, family, capability, `Unsupported`, and `Unverified` decisions distinct.

The current literal Modern Mermaid matrix remains useful input coverage: 510 rows across SVG,
PNG and PDF, with 480 successful outputs and 30 input-specific parse failures. The failures are
classified as upstream-invalid or parser-specific before theme evaluation. The matrix measures
execution against literal inputs; it does not qualify every theme facet or every output family.

# Stage 2: modern Mermaid coverage and semantic impact

The authoritative Stage 2 input is the hash-bound `gotoailab/modern_mermaid` snapshot at
`a021cbce37fc0b07a9f4791c28e983101ea06f2d`. The shared checkout is outside this worktree at
`../../repo-ref/modern_mermaid`; its `src/utils/themes.ts`, `src/components/Preview.tsx`,
`src/fonts.css` and `MERMAID_EXAMPLES.md` hashes match the corpus records. The checkout has one
untracked `pnpm-workspace.yaml`, which was not read or modified. The repository's aggregate pinned
checkout test now passes for all three sources. The Modern Mermaid license mirror preserves the
upstream `LICENSE` bytes, including its final blank line, with SHA-256
`4e7ef9d2fd6e8b957ede09ca1bffe1844740721ea273d226b79bca2a9b9f3198`. Its manifest hash was corrected
to match; checkout validation still requires exact byte equality. A path-specific Git whitespace
attribute retains the verbatim license without treating its final blank line as an editing error.
Excalidraw was verified in a separate detached worktree at the manifest's
`e4ab626739f5f163c5eca56190f615643218b61c`; the shared checkout remains at
`e160ff7ba0641fba729c528482de5277ffb19c58`.

The explicit `pinned_source_checkouts_match_every_manifest_hash` nextest passed 1/1, checking
revisions, license bytes, evidence-file hashes, citation bounds and source font bytes. The ordinary
`merman-theme-fixtures` suite passed 67/67 with two intentional skips, and
`verify-third-party-licenses.py` passed. Before/after logs, the exact checkout root and a source/hash
receipt are retained under `target/bench/experiments/theme-source-check-20260920/`. The shared
checkouts were not modified. This closes the aggregate source provenance gate; it does not promote
fixture mechanisms into qualified public presets.

The closed source matrix contains 24 themes, 18 mechanisms and 21 value facets. Seventeen
mechanisms translate to typed capabilities; `backdrop-filter` remains one explicit residual. The
manifest contains 25 fixtures, 20 of them directly backed by the Modern Mermaid source, with 19
typed-capability witnesses and six source-compatibility witnesses (the overlap is intentional).
The fixture families are Flowchart 12, State 5, Class 3, Sequence 2 and ER 2. Every theme is
projected to five output contracts—Browser SVG, standalone SVG, PNG, JPEG and PDF—so the sparse
policy expands to 120 target cells: 115 intended Portable cells for 23 themes and five explicit
Unverified cells for Aurora's browser `backdrop-filter` residual. This is a semantic expectation
matrix, not a claim that all 115 cells have been rendered by every backend.

The typed inputs cover the required mechanism groups directly:

| Concern | Evidence and boundary |
| --- | --- |
| Tokens and typography | `fixture-token-baseline` reads background, surface, primary and text tokens; font stack, letter spacing, text transform, border, dash, radius and shadow are typed getters. The mixed-script fixture proves the two licensed font slices against parsed visible text. |
| Families and semantic rules | Flowchart, State, Class, Sequence and ER fixtures retain parsed family models; `has-descendant`, `not-class`, ordinal palette and family-specific precedence are source-backed. Sequence is represented as a fixture consumer, while the corpus does not pretend that arbitrary CSS selectors are public input. |
| Canvas and blending | Solid, tiled/repeating and non-repeating linear gradients, radial gradients, layer count and `screen` blending are separate typed fields. Pattern and layering are derived independently; Aurora retains its named backdrop residual. |
| Default, Clear and Transparent | The fixture corpus records typed values and target policy. Runtime owner tests separately prove default/source/config precedence, Clear restoring the effective baseline or suppressing a binding, and Transparent emitting explicit zero-alpha paint. Clear is not silently reinterpreted as Transparent. |
| Light/dark and preset exchange | Installed Python, Node native and Node WASM consumers pass the shared light/dark isolation, materialization, rule-edit, resource-error and preset-export vectors. These are consumer contract checks; they do not promote the ten public preset catalog cells. |
| CSS and exports | Six source-compatibility fixtures retain Mermaid CSS/preference evidence, while typed fixtures reject source-owned CSS as capability proof. The five output contracts are closed in Rust; the separate literal matrix supplies 480 successful SVG/PNG/PDF executions. |
| `Unsupported` and `Unverified` | `ThemeSupportStateV1` distinguishes Unconditional, Conditional, NotApplicable, Unsupported and Unverified. Static discovery is an upper bound; terminal observation owns actual application and admission. Aurora's five target cells are intentionally Unverified, and the current discovery inventory remains 188 Conditional, 343 Unsupported, 99 Unverified and 4,023 NotApplicable SVG responses. |

The fixture contract is currently green: `cargo nextest run --locked -p merman-theme-fixtures`
passed 67/67 tests with two intentional skips. The semantic-matrix mutation, source-line, canvas
layering, typed-vs-CSS, five-output and sparse-policy tests all pass. This closes the Stage 2
corpus integrity question. It does not close preset usability: the current native evidence still
qualifies only the named Brutalist/Spotless/Cyberpunk cells, while broader family visual review,
portable-font variants and the matched U10 performance/footprint comparison remain open.

The architecture consequence is bounded: modern Mermaid remains a design reference decomposed
into Merman-owned typed tokens, family rules, canvas layers and resource metadata. No TypeScript
evaluator, partial CSS parser, bundled Inter font or second export pipeline is justified. Delivery
must keep the existing public materialization/export seam, target-local admission, explicit
Unsupported/Unverified receipts and host/resource residuals. The current Web/Typst size and
consumer results below are reproducible candidate baselines; they are not evidence that the
historical alpha.6 deltas are theme-only costs.

# Preset and capability boundaries

Brutalist and Spotless retain their earlier six native Flowchart/State/Sequence cells. Cyberpunk
now has six complete-scene Flowchart/Sequence/XY cells under the named native profile. The other
seven public preset IDs have no current qualified cells. All ten presets have execution evidence
on representative literal inputs, but that is a usability and identity observation rather than a
portfolio-wide qualification. Earlier browser or host-specific inspections still retain an
arrowhead residual for follow-up; the bounded native Mindmap sample below did not reproduce the
earlier suspected label-contrast issue.

The [dense export review](2026-09-20-preset-usability-exports.md) now supplies a bounded current
visual sample for all ten presets: a pinned Flowchart, Mindmap, dense Class, and four-series XY
scene across SVG/PNG/PDF. 116/120 cells succeeded; four Cyberpunk PNG/PDF cells hit the unchanged
default 128 SVG-conversion-filter budget at 132 primitives. PDFKit opened all 38 successful PDFs.
The inspected sheets show readable dense Class and Mindmap labels and distinguishable XY bar/line
series in this host/font sample. This improves usability guidance but remains unqualified visual
evidence; controlled fonts, browser labels, accessibility, memory/throughput, and wider families
remain open.

The public catalog declares only `light` and `dark` appearances; it has no high-contrast preset or
high-contrast target contract. High-contrast use therefore remains outside the qualified product
surface rather than inheriting a claim from the dark palettes. Documentation and presentation use
are design recommendations in the portfolio record, while brand-color edits remain scoped rule
experiments with target admission observed separately.

The public catalog keeps qualification cells empty. Discovery responses remain a coarse capability
surface: the earlier inventory recorded 188 Conditional, 343 Unsupported, 99 Unverified and 4,023
NotApplicable SVG responses, with binary-output visual queries generally Unverified. These counts
must not be converted into a support percentage or a rendered-terminal claim.

# Artifact and legal observations

The final installed artifacts are macOS ARM64 observations: Python wheel 9,042,331 bytes, Node
root package 113,927 bytes, Node native package 10,541,620 bytes and Node WASM package 7,032,865
bytes. Their embedded native/WASM payloads are recorded with SHA-256 values in the installed
consumer record. These sizes are current artifact identities, not matched alpha.6 deltas or a
budget change.

The 13 Rust license reports pass generator `--check`; release legal projections pass for 382
files; third-party and package legal checks pass for 24 governed packages; representative
dependency-closure verification passes for 34 profiles. The generated Android legal copy now
matches its source report. These checks cover manifests and generated legal material, not builds
on every declared target.

The current source rerun passed `generate-rust-license-report.py --check` (13 reports),
`sync-release-legal-materials.py --check` (382 files), `verify-third-party-licenses.py`, and
`verify_crate_package_legal_materials.py` (24 governed Cargo packages).

The current source also passes `release_surface_contract.py --version 0.8.0-alpha.7`,
`release-version.py check --version 0.8.0-alpha.7`, `cli_installation_contract.py`, and
`verify_release_changelog.py --version 0.8.0-alpha.7`. These are preparation checks; the
date-required immutable preflight remains intentionally unexecuted.

The Android module's local Kotlin/JVM transport suite also passed with
`platforms/android/gradlew test --no-daemon --max-workers=1` (`BUILD SUCCESSFUL`, 15 actionable
tasks). No `adb` device is available in this host, so Android instrumentation and runtime JNI
execution remain unverified.

The current Web package group was rebuilt from source `7f35c9080` after the profile-aware portable
font smoke repair. Node 24.21.0 with npm 12.0.2 passed the owner smoke matrix, WASM input and
package verification, and DOM safety smoke for all five packages. A fresh offline npm consumer
installed all five tarballs and a real headless Chromium consumer loaded each package. Full and
render produced themed SVGs (19,266 and 19,472 bytes respectively); full and ascii produced the
expected ASCII output. The installed capabilities matched each package recipe, including
`embedded-fonts` only on the complete package. The package-group legal digest is
`sha256:027ad254790e94b51b4d9a3322647baff65c86cfb08936ca24ad418bc503ea36`.

The Web WASM size matrix passed the checked-in budgets for all five profiles. Current stripped
bytes are 3,691,349 (analysis), 5,218,980 (ascii), 3,802,570 (editor), 16,291,188 (full) and
13,694,329 (render). The complete Web contract suite now passes 147/147 after the closure
expectation was aligned with the current `web-full` ownership of `merman-export`.

The Web owner smoke was rerun from the current checkout after refreshing all five WASM profiles
and package artifacts. Build transactions were `2124c76371ac` (full), `182394750950` (analysis),
`562ca2bf9112` (render), `bc04461cb812` (editor) and `4491a5510907` (ascii). The full smoke then
passed WASM input verification, package prepack checks, the 35-diagram package matrix and DOM
safety for all five profiles. The captured log is
`target/bench/experiments/web-consumer-current-20260920/smoke.log`, SHA-256
`ab8b44bb8f41a4db62f9eee82ef757e859221a5ce1b984b909204f6d9464a6c2`.

The current Typst WASM artifact was built with the required Binaryen 131 tool and passed the size
budget: 18,321,200 raw bytes, 11,177,136 stripped bytes, 4,253,078 gzip bytes and 3,144,348
brotli bytes. Binaryen 131 `typst-package-smoke` now passes the real package consumer: 22 positive
fixtures, 9 expected compile failures, 22 support vectors, 2 materializations and 3 structured
errors. The shared catalog remains the metadata source, while the constrained Typst policy projects
Cyberpunk as unavailable with `theme-preset.resource-policy-rejected`; this is an explicit resource
result rather than a qualification cell.

The current platform binding owner rebuilt the macOS ARM64 Flutter native asset and passed the
Android ARM64 Rust clippy checks, Flutter analysis, the theme-authoring consumer (2 materializations,
22 support queries, 5 expected errors and 3 budgeted operations) and the ABI 3 Dart contract
tests. The aggregate owner command still exits nonzero because the installed Dart formatter
(Flutter 3.12.2) rewrites one checked-in contract test; the working tree was restored after
confirming that diff was formatter-only, so no formatting change is claimed. The current Apple
XCFramework builder then completed all three slices (macOS universal, iOS device and iOS simulator)
and regenerated the Swift UniFFI bindings successfully. The resulting local XCFramework is
diagnostic and remains outside the release publication set.

The current worktree also reran the real Dart Native Assets entry point with
`dart run tool/theme_authoring_smoke.dart` from `platforms/flutter`. It passed the same two
materializations, 22 support queries, five expected errors and three budgeted operations per
consumer; the retained log is
`target/bench/experiments/flutter-theme-consumer-20260920.log` (SHA-256
`e0037683bd51c2777cf23e5669cf36db6d08f88e39aa5957b4ad6670e24cd9a1`). No generated binding,
lockfile or source file changed. This is a local macOS consumer run, not an installed pub archive,
Android device execution or hosted release matrix.

`dart pub publish --dry-run` also validated the current Flutter package without uploading: the
archive was reported at 8 MB compressed with 0 package warnings, and its legal/native entries were
listed. The dry run is package-shape evidence only; it does not prove pub.dev publication or
consumer execution on every supported target.

The current-source C ABI consumer smoke passed with both empty defaults and an explicit SVG
feature selection. `cargo nextest run --locked -p merman-ffi --no-default-features --features svg
--test c_consumer_smoke` passed the alpha5 compatibility consumer and current C consumer (2/2),
including the feature-gated shared theme-authoring error vectors. The test loads a compiled C
consumer and passes it the Rust API entrypoint; it is not an installed SDK archive replay and
does not cover all six customization journeys. The captured SVG-enabled output is retained at
`target/bench/experiments/c7a-c-abi-current-20260920/svg-nextest.log`; `nextest.log` records the
default-empty ABI check separately. A current rerun also passed both tests (2/2) with nextest run
`0a5ea627-05b1-49e7-a345-389c3ef44d42`; its log is
`target/bench/experiments/c7a-c-abi-current-20260920/svg-nextest-current.log` with SHA-256
`55e0d12d0ee1cb243d5446ca83c3bcffb98cd67cea553180aaf0beea9c4b3ae7`.

The Apple Swift consumer now executes the shared theme goldens through both the one-shot API and
a fresh reusable engine: 2 light/dark materializations, 22 support queries, and 5 expected error
calls per consumer (58 calls total). Materialized specs and support responses are compared as
complete JSON values; errors preserve the shared authoring and resource envelopes, with only
nonempty diagnostic messages excluded from exact comparison. The existing SVG, ASCII, callback,
icon, resource-limit, missing-capability and cancellation smoke also passes. The command is
`swift run --jobs 1 --package-path platforms/apple/examples/smoke MermanAppleSmoke`, using local
Swift 6.3.2 on macOS ARM64 and the earlier built XCFramework. The generated Swift files remain
unchanged. The log is `target/bench/experiments/apple-theme-goldens-20260920.log`; fixture, consumer,
executable and static-library hashes are recorded in
`target/bench/experiments/c7a-c-abi-current-20260920/native-consumer-receipt.json`. This verifies the
local package's generated UniFFI path, not registry installation, iOS execution, Swift 5.9, or the
full preset-edit/export journey.

The [Apple recipe follow-up](2026-09-20-apple-preset-recipe-journeys.md) now rebuilds the macOS
XCFramework from native source `3433717b9` and executes its ARM64 SwiftPM consumer. Both entry
points match the shared complete catalog, export all ten presets and compare their imported
State SVGs. Cyberpunk additionally matches Class/Flowchart/Sequence/XY, and ten malformed recipe
requests reject. Three fresh processes import actual original, Class-color-edited and
unsupported-width recipe files; twelve SVGs and twelve complete operation metadata objects match
the parent reusable engine. The Class color edit reaches actual shape paths without changing the
other families. Node width remains explicitly Unsupported with a source-addressed residual and
rejected target admission; ordinary cells retain Unverified target status. The universal static
library and Swift link input hashes match, and generated bindings remain unchanged. This closes
the local Apple export/scoped-edit gap, while installed archives, the remaining six-journey cases,
Swift 5.9, Intel execution and iOS remain open.

## Candidate consumer matrix

The following matrix is the candidate delivery boundary. A passing local consumer proves the
listed contract only; it does not inherit a different host, installed archive, device or export
capability.

| Transport/profile | Current evidence | Scope proved | Boundary retained |
| --- | --- | --- | --- |
| Python | Installed-consumer record from clean `0c1b1047a` | Shared authoring/support/catalog vectors and direct Cyberpunk recipe exchange | Historical clean-source wheel; no current registry installation or other host evidence |
| Node Darwin ARM64, Node WASM | Refreshed pack/install smoke, content-capability correction and customized recipe exchange below | Shared vectors, eight content-capability rejections, and three saved customized recipes imported by fresh processes per target | Local tarball installation; no registry installation, other Node hosts or mobile runtime |
| Web browser packages | Current Web owner smoke and Chromium package record; refreshed five-profile smoke log above | Five package capability projections, SVG/ASCII and DOM safety | Browser visual qualification and hosted archive provenance remain separate |
| Typst WASM | Binaryen 131 package smoke | 22 positive fixtures, nine expected failures, support/materialization/error vectors | Cyberpunk constrained policy is an explicit resource rejection; no qualification promotion |
| C ABI | [Current native C journeys](2026-09-20-c-abi-theme-journeys.md), 120 fresh processes plus checked-in examples/smokes | Ten preset recipes, scoped Class edits, shared support/errors, native exports and caller-supplied fonts | Source-built ARM64 dylib, not installed SDK archive; broader journeys and hosts remain open |
| Flutter/Dart | Current Native Assets smoke and `pub publish --dry-run` | Two materializations, 22 support queries, five errors, three budgeted operations; package shape | Local macOS arm64 package; no pub.dev publication or Android device run |
| Apple Swift | Current ARM64 SwiftPM smoke, complete preset/fresh-process and boundary records | Catalog, ten preset exports, Class paint/Clear/transparent/font boundaries and dense SVG | No iOS/Intel/Swift 5.9 runtime, installed release archive or visual qualification |
| Android Kotlin/JVM | `platforms/android/gradlew test --no-daemon --max-workers=1` | JVM transport contract tests | No `adb` device, instrumentation or runtime JNI evidence |

This matrix is intentionally conservative: it records real local execution without turning package
shape or source-level tests into release publication evidence.

The [C ABI consumer increment](2026-09-20-c-abi-theme-journeys.md) uses the exact
`c-abi-native` profile at `25f811599`. A separately linked C executable passes 120 fresh-process
cases, including 13 expected errors, with repeated-call stability. The caller-supplied Latin/CJK
font cases produce PNG/PDF with embedded-font Portable receipts; removing the CJK slice yields
explicit missing-glyph errors. PDFKit opens all six retained PDFs, and the mixed-script PNG/PDF
pair visibly preserves both scripts. This adds scoped controlled-font and native-consumer evidence
without claiming an installed archive, browser font parity or other host execution.

The pinned cargo-dist `0.32.0` macOS ARM64 binary was downloaded to `/tmp` and verified against
its release checksum (`aa343b2ff78ec2981f17a65140250c5ad6062c74072163f68c5c2686d94763a7`). Its
plan passed the repository's release artifact-bundle verifier for the `macos-15` native
`aarch64-apple-darwin` route. A serial local build from source `0f5125e75` produced the historical
archive rows below:

| Archive | Packed bytes | SHA-256 |
| --- | ---: | --- |
| `merman-cli-aarch64-apple-darwin.tar.xz` | 13,745,380 | `a5a132d375f8d04a785898bd75c6adcc960dd9e85afd3cbfdd079f0e00eb15bd` |
| `merman-lsp-aarch64-apple-darwin.tar.xz` | 4,079,896 | `201313d1b706752bd62b2ec90197f2d2bb07d6a94e2efb97ae2d41328ef106ea` |

The [current-source replay](2026-09-20-cli-lsp-current-archive-replay.md) subsequently rebuilt
both archives from HEAD `58f3a1531` and passed the same CLI/LSP verifiers with `--execute`. The
older archives fail against the current tree because their dependency-license projection is
stale; they are not used as current evidence. The refreshed result remains macOS ARM64 only and
does not imply Linux, Windows or Intel archive execution.

Static release preparation also passes `release_surface_contract.py --version 0.8.0-alpha.7`,
`release-version.py check`, `cli_installation_contract.py` and the preparation-mode changelog
check. The immutable date-required preflight was intentionally not run because this branch still
has an Unreleased projection and no publication was authorized.

The current source release/preset/archive contract selection also passes
`python3 -m unittest scripts.test_qualify_theme_presets scripts.test_verify_cli_release_archive
scripts.test_release_surface_contract scripts.test_theme_preset_catalog_contract`: 74 tests passed.
The current source `python3 scripts/verify_artifact_dependency_closures.py --representative-targets`
also passed all 34 declared representative profiles, including C ABI, Apple, Android, Flutter,
Python, Typst, Web, CLI, LSP and Rust export closures. These are dependency-boundary checks and
do not turn the Linux-reference profiles into hosted Linux execution evidence.

The current preparation pass additionally succeeded for `generate-rust-license-report.py --check`
(13 reports), `sync-release-legal-materials.py --check` (382 projections),
`verify-third-party-licenses.py`, and `verify_crate_package_legal_materials.py` (24 governed Cargo
packages). These are preparation checks; the immutable date-required workflow and publication
steps remain intentionally deferred.

The declared local compiler floor is explicit and reproducible: `cargo +1.95.0 check --locked
-p merman-theme-fixtures` passed, while the same package under `cargo +1.92.0` was rejected by
Cargo because `merman-core` and `merman-theme-fixtures` require Rust 1.95. This proves the local
floor contract only; hosted Linux/Windows compiler and native archive execution remain open.

# Performance and footprint status

The published alpha.6 versus old local candidate table in the impact audit remains a non-matched
toolchain comparison. It shows package growth, including the CLI, Python and Node artifacts, but
does not assign that growth to the theme work or justify a budget increase. The current-source
Criterion cold-parse run is retained at
`target/bench/experiments/theme-perf-current-ff9b9991b/cold-parse-summary.json`:

| Fixture | Criterion interval | Output bytes |
| --- | --- | ---: |
| architecture_medium | 8.1591–8.2323 µs | 23,877 |
| mindmap_medium | 16.950–17.094 µs | 29,734 |
| class_medium | 78.824–79.940 µs | 11,083 |
| flowchart_medium | 183.57–185.09 µs | 24,373 |

The locked `mermaid-rs-renderer` reference is now isolated at `859253415e69dce28bd65cd5a7c1d1ae8b39f4a1`
and was measured against a clean Merman clone at `4e03b93e7` on eight byte-identical native
fixtures. The pinned renderer predates the benchmark feature expected by the comparison harness,
so its isolated `/tmp` checkout received a temporary empty `benchmark` feature; the shared source
checkout was not changed. The diagnostic output is retained at
`target/bench/experiments/theme-perf-current-20260920/mmdr-quick-clean.json` and `.md`:

| Fixture | Merman | mermaid-rs-renderer | Merman / renderer |
| --- | ---: | ---: | ---: |
| flowchart_tiny | 161.78 µs | 21.44 µs | 7.55x |
| flowchart_medium | 2,976.10 µs | 3,720.70 µs | 0.80x |
| sequence_tiny | 262.52 µs | 11.33 µs | 23.17x |
| sequence_medium | 494.92 µs | 123.45 µs | 4.01x |
| state_tiny | 114.94 µs | 17.18 µs | 6.69x |
| state_medium | 789.61 µs | 1,367.60 µs | 0.58x |
| class_tiny | 203.49 µs | 14.59 µs | 13.95x |
| class_medium | 1,446.90 µs | 1,712.30 µs | 0.85x |

This runner reports `valid_diagnostic`, not a U10 baseline: it retains Criterion console
midpoints rather than raw samples, compares different native transports and does not execute DOM
or raster parity gates. It also used Merman Rust 1.95 and the renderer's declared Rust 1.98
toolchain, so the partial renderer is a context reference, not a release-equivalent semantic
oracle.

An adjacent published-alpha comparison was also run from the exact `v0.8.0-alpha.6` tag
(`d529f858e`) to a clean current-source clone at `4e03b93e7`, with the same Rust 1.95 toolchain,
feature set (`svg,layout-elk,layout-cytoscape`) and eight selected fixtures. The full discovery
receipt is retained at `target/bench/experiments/theme-perf-current-20260920/alpha6-to-alpha7-quick-all.json`.
Six fixtures were correctly excluded because their SVG identities changed between revisions:
Flowchart tiny/medium grew from 10,678/76,558 to 11,199/81,318 bytes, Sequence tiny/medium from
21,276/32,120 to 21,692/32,568, and State tiny/medium from 7,879/42,220 to 7,888/42,285.
Their timings are therefore not treated as matched regressions.

Class tiny and Class medium retained byte-identical SVG identities and completed the decision-grade
confirmation at `target/bench/experiments/theme-perf-current-20260920/alpha6-to-alpha7-class-confirmation.json`:

| Fixture | alpha.6 | current | Relative change (95% simultaneous bound) | Absolute change (95% bound) |
| --- | ---: | ---: | ---: | ---: |
| class_tiny | 50.42 µs | 202.98 µs | +302.53% (+299.76% to +305.94%) | +152.56 µs (+151.38 to +153.93 µs) |
| class_medium | 602.61 µs | 1,444.84 µs | +139.77% (+138.61% to +140.74%) | +842.23 µs (+838.38 to +845.30 µs) |

Both rows are `confirmed_regression` under eight calibration/confirmation pairs, 10,000 bootstrap
resamples, 10% relative and 50 µs absolute thresholds. This is a release-range signal rather than
theme-only attribution: the current report does not assign the cost to the presentation-theme
change, and the U10 recovery gate remains open.

The subsequent [SVG attribute-validation repair](../../../performance/svg_attribute_validation_2026-09-20.md)
removes repeated duplicate-name bookkeeping in private passes after complete XML validation.
Against a production-equivalent current baseline, Class medium decreases from 14,182 to 13,690
allocations and from 2,357,112 to 2,316,024 cumulative bytes. Five fixtures preserve exact SVG bytes,
peak growth and retained heap, with three allocation observations each. The 243 pipeline tests
pass, and two diagnostic timing pairs show no material control slowdown. This admits a bounded
allocation reduction, not a confirmed latency recovery, lower peak memory or C7a closure. A prior
single-byte-search candidate was rejected because runtime allocation did not change.

The [ARM64 SHA-256 attribution](../../../performance/sha2_arm64_attribution_2026-09-20.md) then
identifies a dependency-backend candidate at `2db16c6b7`: the selected sha2 0.10.9 features use
software compression. A 74,203-byte isolated hash averages 119.0 µs there versus 26.1 µs with
default 0.11.0; forcing the latter to software gives 110.3 µs. Digest checks pass, but no workspace
dependency changed and no end-to-end speedup is admitted. Distinct resource and output digest
contracts remain required. A separate per-tag-copy proposal was rejected because default Class
has no prepared-text ledger and never enters that branch.

The subsequent [SHA-256 upgrade](../../../performance/sha2_upgrade_2026-09-20.md) passes its
registered native admission gate. Clean measurement snapshots with identical benchmark source
use eight A/A pairs per executable and eight balanced confirmation pairs. Class medium falls
from 1,461.74 to 1,248.73 µs (14.58%, 213.01 µs); Sequence medium falls from 495.30 to 402.54 µs
and also clears both thresholds. All five rows confirm non-regression. SVG bytes, full public
receipts, all ten preset fingerprints and fifteen paired heap observations remain identical.
The same-harness benchmark executable decreases by 68,368 bytes; this is not a packaged-size
result. The candidate passes 4,740 tests (three skipped), full SVG structure, compile/feature,
dependency/legal and platform-binding checks. Android target checks and local Flutter ABI,
Native Assets and theme-authoring smokes pass; Android device execution remains unverified.
Earlier installed-package and archive receipts still belong to their recorded revisions and
must be regenerated for the final candidate. This adjacent optimization does not establish
recovery against alpha.6 or close C7a/U10.

The isolated native memory owner run is now complete at
`target/bench/experiments/theme-perf-current-20260920/native-memory-full-10000.json`. It used a
clean Merman clone, 30 fresh-process pairs over scales 1/2/4/10/32/100, five repeats per scale,
and 10,000 bootstrap resamples. The infrastructure-smoke contract passed all caps with slopes
of 1.259 (allocation count), 1.394 (allocated bytes) and 1.467 (peak growth bytes); at scale 100
the measured values were 1,660,450 allocations, 349,202,213 allocated bytes and 85,121,537 peak
growth bytes. The owner contract deliberately sets `candidate_admission: false`, so this is
reproducible memory evidence and a bounded risk signal, not release admission or a matched alpha.6
comparison.

The [theme-operation checkpoint](../../../performance/theme_operations_2026-09-20.md) adds a
current native baseline at `bb411e505`: 13 operations in each of SVG-only and advanced Cargo feature
lanes, 30 Criterion samples per row, and 780 retained samples. Both lanes pass the shared authoring,
support and preset-catalog contracts plus export/recompile fingerprint checks (13/13 test-mode
cases each). Default definition compilation averages 45.005/45.218 µs, Cyberpunk preset compilation
222.539/226.832 µs, and reused-engine catalog construction 1,494.846/1,514.842 µs (SVG/advanced).
The 22-query support batch averages 28.198/29.112 µs. Catalog requests compile all ten preset
descriptors for policy availability; reusing an engine does not cache that work. No production
cache or resource budget changed. These warm-process samples do not establish cold-start,
allocation, cross-transport or packaged-artifact costs. Alpha.6's presentation catalog is a
different contract, so the checkpoint makes no historical delta claim.

The [native export checkpoint](../../../performance/native_export_2026-09-20.md) adds 18 current
source-to-SVG/PNG/PDF rows and 540 raw samples at `671ad8972`, using the unchanged complete
Flowchart/Sequence/XY scenes under default and Cyberpunk themes. All outputs succeed at default
limits, all pre/postflight bytes match, and the 18 retained files match their hash receipts.
PDFKit independently opens and rasterizes all six PDFs; the three Cyberpunk previews retain
visible labels, glow and complete scenes. Cyberpunk PNG costs 42.6–59.0 ms (17.0–23.5 outputs/s),
PDF 378.6–730.6 ms (1.37–2.64 outputs/s), and SVG 1.94–2.40 ms. The advanced native Cargo lane uses
system fonts and warm processes; it does not qualify PDF pixels or close an installed-package,
allocation, concurrent-throughput or historical regression gate. Default and Cyberpunk have
different geometry and effects, so their timing difference is not an equal-output regression.

Matched alpha.6 latency outside the two Class rows, cold start, matched themed/native export and
compile/discovery deltas, and same-source archive size comparisons remain unverified. The declared
U10 rule still treats unavailable metrics as unverified; no default budget or limit was changed.

The current Node transport first-render probe was attempted with the checked-in benchmark harness
and the existing macOS ARM64 N-API/WASM artifacts, but the harness rejected both candidates before
sampling because their build receipts still bind the old `crates/merman-node/Cargo.lock` digest
`sha256:4b7ab318...`, while the current source digest is
`sha256:b3c42d2379...`. No stale-artifact timings were admitted as evidence, and the cold-start /
first-render gate therefore remains open until candidates are rebuilt from this exact source and
lockfile.

The candidates were then rebuilt from this source and lockfile: both receipts carry the
`sha256:b3c42d2379...` lock digest. The benchmark harness's public-API error probe was corrected,
its contract tests pass 15/15 under npm 12, and commit `8477e7a0b` now scopes the cross-candidate
catalog equality check to same-target comparisons. This preserves the intentional target contract
difference: the WebAssembly catalog reports `64`, while native reports `256` for
`svg_backend_tree_depth` in all four resource profiles. The source authority is
`crates/merman-render/src/resources.rs`, where the values are selected by
`target_arch = "wasm32"`, and the binding projection preserves that value.

The resulting representative report is retained at
`target/bench/experiments/node-current-theme-20260920/representative-target-catalog-8477e7a0b.json`.
All eight selected fixtures completed on both candidates with matching SVG structure, geometry and
raw bytes. Diagnostic p50 values were cold `162.079 ms` (WASM) versus `47.688 ms` (N-API), warm
`1.019 ms` versus `0.641 ms`, and four-request concurrency `3.268 ms` versus `0.801 ms`. Packed
footprints were `7,036,598` versus `10,659,970` bytes; peak RSS was `463,355,904` versus
`92,667,904` bytes. The report's `evidence_excluded: true` means SVG evidence projection is
excluded from the measured interval, not that the timing samples are invalid. Its transport
decision remains inconclusive because this host contributes only one target result. These are
current-source cross-transport observations with eight cold samples and three warm passes, not a
matched alpha.6/alpha.7 regression comparison or balanced confirmation run; they do not close U10.
At that checkpoint, six KaTeX/math fixtures returned `MERMAN_INVALID_TRANSPORT`. The later
content-capability correction below resolves their error classification; the representative
timing samples remain diagnostic evidence rather than release admission.

The current Node package contracts were rerun with the repository-required npm 12 CLI
(`12.0.2`): `npm test` passed 109/109 and `npm run check:packages` passed the candidate package
contracts. The host's default npm 11 was intentionally not admitted because the footprint test
requires npm 12's named `npm pack --json` result shape. The test log is
`target/bench/experiments/node-consumer-current-20260920/tests.log`, SHA-256
`b605de3d2bc04e9a769a72c1875c361d027a73e63890000f80f8af0b162cd6f3`. These checks validate the
current JavaScript contract and package shape; they do not refresh the installed-consumer record
or establish registry and mobile runtime evidence.

The current-source candidate refresh then rebuilt both `node-wasm` and macOS ARM64 N-API from
commit `7e66c9b13`, assembled and verified both package groups, packed them with npm 12, and
installed each into a fresh local consumer. The installed smoke passed for both targets with the
same 29 SVG/theme-authoring operations, including complete-scene recipes and five expected
rejections. The WASM and native smoke logs are respectively
`ecc052db7cc1ebaaad48acc7c519c2dce77ebf5e0ff199b5bcb3cdee3d55dd8d` and
`fb589f4c3663a646ae54d604b258179b97f70d0a74df22acf7395c199d9115b2`. This refresh supersedes
the historical local installed artifacts for these two macOS-hosted targets; registry installs,
other Node hosts and mobile runtimes remain unverified.

### Node content-capability correction

An installed font-resource probe exposed a Node error-decoder defect: it treated every
`missing-capability` response from an advertised operation as a transport contradiction. The
Rust engines correctly returned `embedded-fonts` or `math` as the missing capability, but the
JavaScript adapter discarded that diagnostic. SVG operation availability does not promise that
these optional content capabilities exist.

Commit `3c19a74d6` changes the adapter to compare the missing capability against the operation
prerequisite and the runtime capability catalog. Unknown operations and denial of an advertised
capability still fail closed; unadvertised content capabilities retain `MermanOperationError`,
`MERMAN_UNSUPPORTED_OPERATION`, and the original capability ID. Regression tests cover async
and sync calls, future capability IDs, and both primary and supplemental catalog contradictions.
The Node suite passes 111/111 with npm 12.0.2.

Both refreshed local package groups passed assembly, verification, offline tarball installation
and the expanded installed smoke: 30 SVG renders, 23 JSON operations, 44 support queries, six
authoring diagnostics, two resource-limit checks and eight expected content-capability failures
per target. Font assets reject in request and constructor paths, using both the spec selection
and the complete recipe. Resource-free font-family names remain in the SVG without embedded font
data. This does not prove actual host-font absence, offline browser glyph appearance or portable
font qualification.

All six math fixtures in the benchmark corpus were also replayed through the installed WASM and
Darwin ARM64 packages, both synchronously and asynchronously: 24 calls returned the expected
`missing-capability: math`. Math remains explicitly unavailable in these package recipes; no
feature was enabled and no benchmark timing or budget changed.

The final two build receipts bind commit `3c19a74d6` and source digest
`sha256:411d5537f7ebc4d906600423c739b16b6a9ab5ab2a3454ff59077dad96a5bdc1`.
Receipts, pack records, installation logs, smoke output and the six-fixture replay script/logs are
retained under `target/bench/experiments/node-content-capabilities-3c19a74d6/`. The smoke JSON
SHA-256 values are `8b1f7325b4fbeaa6bcae931fcde5c559a98faa7d2be91304d73bb1503c24b523`
(WASM) and `08ee388674e6b39dd817967d905474e3267d0de305df4f680038b350476c5244`
(Darwin ARM64). Packed sizes are 113,988 bytes (loader), 10,562,660 bytes (native target) and
7,025,279 bytes (WASM). These are local artifact observations, not matched footprint regressions.
Node 26.8.2/npm 12.0.2 executed this replay; it does not establish the Node 24 CI runtime lane.
Three read-only simplification reviewers found no reuse, quality or efficiency issue; a targeted
manual correctness review and `git diff --check` also passed.

Reproduce this boundary through the existing Node owners: `npm test` under npm 12,
`build-candidate.mjs --candidate node-wasm`, `build-candidate.mjs --candidate napi --target darwin-arm64`,
and the corresponding `assemble-packages.mjs` / `verify-packages.mjs --packed-root` paths.
Pack with `npm pack`, install each tarball group with `npm install --offline --ignore-scripts`,
then run `smoke-installed-package.mjs --project <consumer> --version 0.8.0-alpha.7 --target <target>`.

### Node customized recipe exchange

Commit `05065aa1a` extends the existing installed-package smoke with public workflow journeys
2 and 3. It reuses the Web `customizeNodeColors` example to modify two explicit roles (canvas
base and Node border) plus a Class-only fill rule in exported Cyberpunk. Three variants use an
explicit fill, Clear and transparent paint. The test preserves every other recipe field,
including layered canvas and effect graphs, and confirms the original export was not mutated.

For each installed transport, three fresh Node processes read actual saved recipe files directly,
without envelope extraction or preset regeneration. Their Class/Flowchart/Sequence/XY outputs
and complete operation metadata match the parent engine in 12 comparisons per transport. The
canvas rectangle and Class outer paths carry the requested colors. A source-styled Class node
retains its own fill, stroke and width. Changing only the Class fill leaves the other three
families' SVG bytes identical. The Class writer separates fill and stroke paths; the oracle
checks both actual outer-path terminals rather than assuming one path owns both paints.

The Clear variant also retains `unsupported-paint`, the authored `/styles/94` source location,
and rejected target admission. Ordinary explicit/transparent variants retain Unverified target
status. Restoring the default paint does not imply a qualified Clear route or a Portable SVG.
These assertions keep the rendered effect and its support explanation distinct.

Both local installed package groups from `3c19a74d6` pass this expanded smoke, and the Node suite
remains 111/111 under Node 26.8.2/npm 12.0.2. This tranche changes consumer verification and
documentation only; it reuses the unchanged installed runtime payloads and does not claim a new
runtime build. Logs are in `target/bench/experiments/node-customized-recipes-20260920/`:

| Log | SHA-256 |
| --- | --- |
| `node-wasm-smoke.log` | `3e34eaf8d44a13d03030eb4a295ce9460612c1a45a21c3135e37c04afb89fc36` |
| `darwin-arm64-smoke.log` | `528cb11dd61e96b9a2a94d45921c8d987e7704c0a82d336d3c7556cb546992f1` |

Reproduce with `smoke-installed-package.mjs --project <consumer> --version 0.8.0-alpha.7
--target <node-wasm|darwin-arm64>` against the corresponding installed tarballs. This closes the
bounded explicit-role modification and fresh-file import cases for these two installed Node
transports. It does not define global brand recoloring, certify the preset portfolio, prove
missing-font visual behavior, exercise PNG/PDF in Node, or complete the six-journey host matrix.

# Classification and C7b deferral

The candidate evidence is classified as follows:

| Class | Current conclusion |
| --- | --- |
| Completed | Typed theme tokens, source-backed fixture matrix, default/Clear/Transparent semantics, light/dark exchange, CSS boundary, Web and Typst consumer contracts, macOS ARM64 CLI/LSP replay, legal projections and local Rust 1.95 floor contract. |
| Limited support | Host-dependent native Cyberpunk scenes, selected Brutalist/Spotless cells, portable-font gaps, browser/PDF/HTML resource boundaries, and diagnostic mmdr/memory measurements. These retain explicit host or evidence-class receipts. |
| Explicitly unsupported or unverified | Aurora browser `backdrop-filter` cells, Typst Cyberpunk under the constrained resource policy, 343 discovery `Unsupported` responses, 99 discovery `Unverified` responses, and every unexecuted hosted archive or consumer route. Empty catalog qualification cells remain intentional. |
| Performance and size change | Current artifact sizes, two matched Class regressions, the adjacent ARM64 SHA upgrade and native compile/discovery plus SVG/PNG/PDF diagnostic baselines are measured; alpha.6 recovery, cold start, first render, matched export/compile/discovery deltas and cross-host size deltas remain unverified. No budget was relaxed. |
| Preset conclusion | All ten preset IDs have representative literal execution, but only named Brutalist/Spotless/Cyberpunk cells have qualification evidence. This is insufficient for portfolio-wide catalog promotion. |

C7b is explicitly deferred. Its known scope includes Class's four Text routes, Block's 32 legacy
routes, additional mechanism breadth, broader preset/application review, portable and controlled
font variants, and the unresolved brand-edit/resource-failure journeys. These items require their
own family-local semantic and export evidence; successful C7a literal renders do not promote them.

# Requirement closure matrix

| Requirement | Evidence anchor | Status at this source |
| --- | --- | --- |
| Alpha.7 artifact and profile matrix | Web/Typst size matrices, installed Python/Node packages, native Cyberpunk profile and dependency-closure reports | Complete for the exercised macOS ARM64 profiles; other declared hosts remain unverified |
| Installed consumer journeys | Python, Node native/WASM, Web browser and Typst package records; C ABI source consumer smoke | Installed records cover the named packages; Flutter's current Dart Native Assets consumer and Apple ARM64 now cover their local authoring/preset/error boundaries; C ABI archive installation, mobile and remaining six-journey routes remain limited |
| Cross-transport authoring/support goldens | Shared light/dark, materialization, rule-edit, resource-error and preset-exchange vectors | Python and Node native/WASM records retained; rebuilt local Apple consumer passes authoring/support/errors, complete preset exchange, scoped Class paint edits and fresh-process output/metadata comparisons; remaining journeys and final installed transport parity stay open |
| Catalog qualification | Native Cyberpunk/Brutalist/Spotless receipts and discovery inventory | Qualification cells intentionally remain empty; no catalog promotion |
| CLI/LSP archive replay | cargo-dist macOS ARM64 archive assembly and execute-mode verifiers | Complete on macOS ARM64; Linux, Windows and Intel execution unverified |
| Version, legal and preparation checks | Alpha.7 version projections, 13 license reports, 382 legal projections and preparation-mode release checks | Complete for preparation mode; publication-date preflight is intentionally deferred |
| Modern Mermaid source coverage | Hash-bound source snapshot, 24 themes, 18 mechanisms, 25 fixtures and 120 target cells; pinned three-source checkout test | Semantic matrix and aggregate source provenance gate complete; public preset qualification remains separate |
| Theme semantics | Typed token, family, canvas, CSS, default/Clear/Transparent, light/dark and support-state fixtures | Complete at the fixture-contract level; ARM64 Apple and external C consumers confirm Class Clear/transparent and source-owned facet precedence; C native exports also exercise supplied fonts and missing glyphs, while Aurora and unexecuted targets remain explicitly Unverified |
| Preset usability | Native selected cells, literal ten-preset execution and documented visual follow-ups | Limited but refreshed: the ten-preset dense Class/Mindmap/XY/Flowchart review covers current SVG/PNG/PDF behavior; high-contrast certification, controlled fonts, browser labels and wider-family qualification remain open |
| WASM/Node/Python/Typst/CLI/native size | Current artifact identities, Web/Typst budgets, CLI/LSP archive sizes and legal digests | Measured current snapshots; matched alpha.6 attribution and cross-host deltas remain open |
| Cold start, first render and export throughput | Cold-parse Criterion, Node transport diagnostics, alpha.6 Class confirmation and 18 native SVG/PNG/PDF rows with 540 samples | Current native warm throughput measured with output identity and PDF reader checks; Apple dense SVG admission is verified, while historical export deltas, PNG/PDF on Apple and broader cold/first-render admission remain open |
| Large-diagram memory | 30 fresh-process pairs, six scales and 10,000 bootstrap resamples | Infrastructure smoke passed; owner contract excludes release admission |
| Theme compile and discovery cost | Current two-feature-lane Criterion checkpoint: 26 rows, 780 samples and shared golden/fingerprint gates | Native warm-process baseline measured; matched historical deltas, allocation costs and transport overhead remain unverified |
| Architecture and delivery impact | Typed capability boundary, dependency observations, no Inter/new proof engine and C7b scope | Complete as a bounded impact audit; follow-up work is explicitly deferred |

# Open C7a gates

The following evidence is still required before C7a can be marked eligible:

- Same-source CLI/LSP archive assembly and replay for every declared host. The macOS ARM64
  archive route now passes locally; Linux, Windows and Intel archive execution remain open.
- Installed Web, Typst, Native C/UniFFI and relevant mobile/Apple/Flutter consumer journeys,
  including the six public customization/resource journeys and explicit resource failures. Web
  and Typst now pass on macOS ARM64; the SVG-enabled C consumer and Apple shared theme goldens pass,
  while complete native customization journeys and mobile execution remain open.
- Linux/Windows native and hosted compiler-floor results; the local Rust 1.95 floor check passes,
  but the current package evidence is macOS ARM64 only.
- Portable-font, missing-font and controlled-font coverage beyond the named host-dependent cells;
  browser, PDF and HTML labels retain their documented host/resource boundaries.
- Matched U10 performance and footprint evidence, including the large-memory workload and binary
  export throughput. Existing alpha.6 comparison numbers remain historical context.
- Broader preset/application review and non-Cyberpunk consumer cases before any catalog-cell
  promotion. A successful render on a literal input does not imply a complete reference design.

# Reproduction anchors

Use the two linked verification records for the exact qualification and installed-consumer
commands. Current source-only cold parsing is reproduced with the four `cargo bench --locked`
commands recorded in the experiment logs. Run the legal checks with:

```console
python3 scripts/generate-rust-license-report.py --check
python3 scripts/sync-release-legal-materials.py --check
python3 scripts/verify-third-party-licenses.py
python3 scripts/verify_crate_package_legal_materials.py
python3 scripts/verify_artifact_dependency_closures.py --representative-targets
```

The ignored experiment directories contain package receipts, smoke JSON, exchanged recipe files,
SVG hashes and logs. They are diagnostic evidence and are not release assets. This audit does not
change the public schema, qualification thresholds, package budgets or catalog qualification.
