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

The release-mode support discovery gate also passes
`cargo nextest run --release --locked -p merman-render --lib --test theme_support_discovery_test`
with the support-manifest selection: 58 tests passed and 2,520 unrelated tests were skipped.

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
checkout test currently stops at the Modern Mermaid license record: the upstream `LICENSE` has one
additional trailing blank line, so its byte hash is
`4e7ef9d2fd6e8b957ede09ca1bffe1844740721ea273d226b79bca2a9b9f3198`, while the normalized copied
notice is `45195cf54a3816f4774dc762360b9d9a311fd66c7146d262ebe12ae55e420264`. A temporary exact
source notice copy allowed the test to advance through Modern Mermaid and Mermaid, where it then
stopped at the separately shared Excalidraw checkout revision
`e160ff7ba0641fba729c528482de5277ffb19c58` instead of the manifest's
`e4ab626739f5f163c5eca56190f615643218b61c`. We left both shared checkouts and the normalized
notice untouched; therefore the three-source aggregate gate remains open, while the snapshot,
source-file and semantic-matrix hashes remain independently verified.

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
portfolio-wide qualification. Earlier inspections still flag dark arrowheads and Cyberpunk
Mindmap label contrast for follow-up.

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

The pinned cargo-dist `0.32.0` macOS ARM64 binary was downloaded to `/tmp` and verified against
its release checksum (`aa343b2ff78ec2981f17a65140250c5ad6062c74072163f68c5c2686d94763a7`). Its
plan passed the repository's release artifact-bundle verifier for the `macos-15` native
`aarch64-apple-darwin` route. A serial local build from source `0f5125e75` produced and verified
both native archives with the real CLI and LSP smoke contracts:

| Archive | Packed bytes | SHA-256 |
| --- | ---: | --- |
| `merman-cli-aarch64-apple-darwin.tar.xz` | 13,745,380 | `a5a132d375f8d04a785898bd75c6adcc960dd9e85afd3cbfdd079f0e00eb15bd` |
| `merman-lsp-aarch64-apple-darwin.tar.xz` | 4,079,896 | `201313d1b706752bd62b2ec90197f2d2bb07d6a94e2efb97ae2d41328ef106ea` |

The archive checks used `scripts/verify_cli_release_archive.py` and
`scripts/verify_lsp_release_archive.py` with `--execute`, including the generated sidecar
checksums. These are current-source macOS ARM64 results; they do not imply Linux, Windows or
Intel archive execution.

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

The isolated native memory owner run is now complete at
`target/bench/experiments/theme-perf-current-20260920/native-memory-full-10000.json`. It used a
clean Merman clone, 30 fresh-process pairs over scales 1/2/4/10/32/100, five repeats per scale,
and 10,000 bootstrap resamples. The infrastructure-smoke contract passed all caps with slopes
of 1.259 (allocation count), 1.394 (allocated bytes) and 1.467 (peak growth bytes); at scale 100
the measured values were 1,660,450 allocations, 349,202,213 allocated bytes and 85,121,537 peak
growth bytes. The owner contract deliberately sets `candidate_admission: false`, so this is
reproducible memory evidence and a bounded risk signal, not release admission or a matched alpha.6
comparison.

Matched alpha.6 latency outside the two Class rows, cold start, themed/native workloads, PNG/PDF
throughput, compile/discovery cost and same-source archive size comparisons remain unverified. The
declared U10 rule still treats unavailable metrics as unverified; no default budget or limit was
changed.

The current Node transport first-render probe was attempted with the checked-in benchmark harness
and the existing macOS ARM64 N-API/WASM artifacts, but the harness rejected both candidates before
sampling because their build receipts still bind the old `crates/merman-node/Cargo.lock` digest
`sha256:4b7ab318...`, while the current source digest is
`sha256:b3c42d2379...`. No stale-artifact timings were admitted as evidence, and the cold-start /
first-render gate therefore remains open until candidates are rebuilt from this exact source and
lockfile.

# Classification and C7b deferral

The candidate evidence is classified as follows:

| Class | Current conclusion |
| --- | --- |
| Completed | Typed theme tokens, source-backed fixture matrix, default/Clear/Transparent semantics, light/dark exchange, CSS boundary, Web and Typst consumer contracts, macOS ARM64 CLI/LSP replay, legal projections and local Rust 1.95 floor contract. |
| Limited support | Host-dependent native Cyberpunk scenes, selected Brutalist/Spotless cells, portable-font gaps, browser/PDF/HTML resource boundaries, and diagnostic mmdr/memory measurements. These retain explicit host or evidence-class receipts. |
| Explicitly unsupported or unverified | Aurora browser `backdrop-filter` cells, Typst Cyberpunk under the constrained resource policy, 343 discovery `Unsupported` responses, 99 discovery `Unverified` responses, and every unexecuted hosted archive or consumer route. Empty catalog qualification cells remain intentional. |
| Performance and size change | Current artifact sizes and the two matched Class regressions are measured; alpha.6 attribution, cold start, first render, PNG/PDF throughput, compile/discovery cost and cross-host size deltas remain unverified. No budget was relaxed. |
| Preset conclusion | All ten preset IDs have representative literal execution, but only named Brutalist/Spotless/Cyberpunk cells have qualification evidence. This is insufficient for portfolio-wide catalog promotion. |

C7b is explicitly deferred. Its known scope includes Class's four Text routes, Block's 32 legacy
routes, additional mechanism breadth, broader preset/application review, portable and controlled
font variants, and the unresolved brand-edit/resource-failure journeys. These items require their
own family-local semantic and export evidence; successful C7a literal renders do not promote them.

# Requirement closure matrix

| Requirement | Evidence anchor | Status at this source |
| --- | --- | --- |
| Alpha.7 artifact and profile matrix | Web/Typst size matrices, installed Python/Node packages, native Cyberpunk profile and dependency-closure reports | Complete for the exercised macOS ARM64 profiles; other declared hosts remain unverified |
| Installed consumer journeys | Python, Node native/WASM, Web browser and Typst package records | Complete for those consumers; C/UniFFI and mobile journeys remain limited to owner smokes |
| Cross-transport authoring/support goldens | Shared light/dark, materialization, rule-edit, resource-error and preset-exchange vectors | Complete for Python and Node native/WASM; broader transport parity remains open |
| Catalog qualification | Native Cyberpunk/Brutalist/Spotless receipts and discovery inventory | Qualification cells intentionally remain empty; no catalog promotion |
| CLI/LSP archive replay | cargo-dist macOS ARM64 archive assembly and execute-mode verifiers | Complete on macOS ARM64; Linux, Windows and Intel execution unverified |
| Version, legal and preparation checks | Alpha.7 version projections, 13 license reports, 382 legal projections and preparation-mode release checks | Complete for preparation mode; publication-date preflight is intentionally deferred |
| Modern Mermaid source coverage | Hash-bound source snapshot, 24 themes, 18 mechanisms, 25 fixtures and 120 target cells | Semantic matrix complete; aggregate three-source checkout gate remains open |
| Theme semantics | Typed token, family, canvas, CSS, default/Clear/Transparent, light/dark and support-state fixtures | Complete at the fixture-contract level, with Aurora and resource residuals explicitly Unverified |
| Preset usability | Native selected cells, literal ten-preset execution and documented visual follow-ups | Limited; portfolio-wide default/dark/high-contrast/document/brand/export review remains open |
| WASM/Node/Python/Typst/CLI/native size | Current artifact identities, Web/Typst budgets, CLI/LSP archive sizes and legal digests | Measured current snapshots; matched alpha.6 attribution and cross-host deltas remain open |
| Cold start, first render and export throughput | Current cold-parse Criterion run, mmdr diagnostic and alpha.6 Class confirmation | Partial; cold start/first render and PNG/PDF throughput are not decision-grade |
| Large-diagram memory | 30 fresh-process pairs, six scales and 10,000 bootstrap resamples | Infrastructure smoke passed; owner contract excludes release admission |
| Theme compile and discovery cost | Discovery receipts and current fixture/support inventory | Structural discovery evidence exists; matched compile/discovery cost remains unverified |
| Architecture and delivery impact | Typed capability boundary, dependency observations, no Inter/new proof engine and C7b scope | Complete as a bounded impact audit; follow-up work is explicitly deferred |

# Open C7a gates

The following evidence is still required before C7a can be marked eligible:

- Same-source CLI/LSP archive assembly and replay for every declared host. The macOS ARM64
  archive route now passes locally; Linux, Windows and Intel archive execution remain open.
- Installed Web, Typst, Native C/UniFFI and relevant mobile/Apple/Flutter consumer journeys,
  including the six public customization/resource journeys and explicit resource failures. Web
  and Typst now pass on macOS ARM64; the remaining C/UniFFI and mobile evidence is still bounded
  to the owner checks described above.
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
