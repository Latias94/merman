# Theme artifact budget reassessment — 2026-09-14

## Decision rule

Size budgets are regression guards, not immutable product requirements. The maintainer accepts
raising them when the larger product is justified. Preserve required semantics, diagnostics,
resource limits and font safety; remove accidental dependency reachability and material duplication
before treating an increase as a new baseline. Exact attribution of every byte is not required.

A replacement baseline needs one committed source, the canonical recipes for all five Web profiles
and Typst, functional/package checks, tool versions and artifact hashes, and all four measured size
metrics. Keep the existing approximately three-percent headroom policy unless another margin has
a stated workload reason. Do not raise budgets from mixed historical artifacts. This record accepts
the one-source product baseline below and updates the size limits. It does not close the full C7a
artifact/rollout gate.

## Existing Web packages measured

The existing package files were measured with the built `xtask wasm-size-matrix --surface web
--web-package-root <absolute package root> --budget-file <absolute budget file>` command.
This mode reads package artifacts and does not rebuild them. It checks their profile metadata.
Their provenance JSON does not contain a source commit; these are **not current-HEAD build claims**.
The source checkout was `2e06b0101` plus in-flight Block Cluster changes, which the size command
neither compiled nor included in those package bytes. Raw inputs were unchanged by measurement.

| Profile | Raw | Stripped | Gzip | Brotli | Brotli above pre-rebaseline limit |
| --- | ---: | ---: | ---: | ---: | ---: |
| web-analysis | 3,674,774 | 3,674,509 | 1,412,163 | 1,074,487 | 2.33% |
| web-ascii | 5,195,193 | 5,194,928 | 1,913,913 | 1,445,450 | 1.44% |
| web-editor | 3,785,907 | 3,785,642 | 1,456,727 | 1,105,081 | 0.46% |
| web-full | 15,900,334 | 15,900,069 | 5,933,543 | 4,367,710 | 9.74% |
| web-render | 14,003,464 | 14,003,199 | 5,283,006 | 3,893,562 | 13.68% |

All twenty checks failed the pre-rebaseline budgets. The small-profile overages and complete-renderer
overages require separate explanations; font/theme rendering costs do not explain an analysis-only
package. The result is a size observation, not a benchmark speedup or attribution experiment.

| Package | Input SHA-256 |
| --- | --- |
| analysis | `dcebf8f920791f3204e79193bea1253b5f055a01a1898100e34af2cdc079cf23` |
| ascii | `fb9493da708faee13b5893e6c41165b781f1e9cb2bb95ee9359e943a9ddad59c` |
| editor | `cc470999edb824d4aafd3fb4f219955f84fc91ea3a94001bfa4a6f2caa098fca` |
| full | `410cd59ef64aaf71716bc6844d96e64efc24dfc31f0af914493783c096b6f2c0` |
| render | `ce1b470617a845f093b0ac8526e8bd15f783fce17fffc38564def905184754c1` |

Measurement executable SHA-256: `e7b76b8d10beccb34d0bc8db6ccce4493d88420414a391df54337ede3180fd89`.
The ignored experiment ledger, full retained provenance, budget hash and raw output are under
`target/bench/experiments/web-existing-artifacts-20260914-2e06b0101/`.

## What the historical anchors establish

The six artifact feature lists retain the same names, but their implementation capabilities have
changed. The budget notes at `f2adde25e:docs/release/WASM_SIZE_BUDGETS.json` identify the historical
anchors: `0f264982b` (slim), `8bf09dcd9` (complete renderer and Typst), and `1ca41032c`
(ASCII/full semantic-depth). They are not an equal-capability requirement for the new theme product.

- The ASCII semantic-depth increase was already admitted in the August 23 ASCII/full budget
  change. Do not count that work again to justify a second increase.
- The old renderer anchors predate the complete typed theme authoring/schema, materialization,
  preset export and resource/font-validation surface. Their fixed size cannot be imposed without
  accounting for those new capabilities. Font shaping, font parsing, WOFF2/Brotli decoding and
  resource digests have specific production consumers; mere presence in Cargo.lock is not proof
  that a dependency reaches every artifact.
- Earlier ICU collation and vendored font-table runtime removal offset some additions. Net size
  cannot be attributed by summing new dependencies.
- The retained `4e4f3acc3` Web package set is a useful recent control. Slim-profile core/ASCII
  sources, manifests, lockfile and profile registry are unchanged between that source and
  `2e06b0101`; complete render profiles also include subsequent typed-support changes.
- The `1ca41032c` CI measurement cited by the budget is not a locally available Git object.
  Its later merge commit must not be represented as the identical binary baseline.

The [assignment extraction](theme_assignment_wasm_size_2026-09-12.md) already removed about
109–111 KB raw from each Web artifact. The [evidence index repair](theme_evidence_requirement_index_2026-09-13.md)
removed quadratic work with a small size decrease. The
[typography winner repair](theme_typography_recomputation_2026-09-13.md) deliberately accepted
small size growth for reduced repeated work. Failed `-Oz`, unstable-sort and evidence-clone
experiments remain documented; repeating those rejected micro-optimizations is not the next gate.

## Initial evidence gap

The initial checkpoint required finishing the current consumer/renderer slice and building and
smoking all six canonical artifacts at one source. It also required effective dependency closures
to rule out unintended capability inclusion, plus a recent same-product control to detect
unexplained growth. The older accepted anchors explain product evolution. The measurements and
package checks below now satisfy this size-rebaseline scope; the earlier Web-only observation
did not provide Typst evidence.

## Capability attribution at `f2adde25e`

The five Web recipes and Typst publish recipe retain their explicit feature sets. Static feature
and production-call inspection establishes the following boundaries; this is not per-symbol byte
attribution.

- Analysis and editor include Web cooperative operation deadlines (`60bab61db`) and corrected
  source ceilings (`b69e9f8e1`, `6a84ea082`) after the August 3 slim anchor. The public operation
  dispatcher checks option size, parses timeout and creates operation control before analysis.
  Analysis configuration also moved to its shared validated contract (`da4f9d9c7`, `080dc8a5c`).
  These are reachable responsibilities, but ownership consolidation alone does not prove a net
  increase is necessary: analysis also removed its chrono dependency.
- Editor still consumes analysis and editor-core. Its syntax-highlighting migration (`a81e2d962`)
  reduced the WASM wrapper and did not add tree-sitter to this artifact. Do not attribute its
  size increase to an embedded tree-sitter runtime.
- ASCII added the viewport output contract (`bd1b849ec`), compact Flowchart/Sequence layout
  (`a7c83933c`, `9a1de2010`) and capability/report alignment (`37a7dfc1c`) after the August 23
  semantic-depth budget approval. These additions provide measurable viewport reports, compact
  output and complete fallback. They are distinct from the already admitted semantic-depth work.
- Only Web render/full enable SVG. Bindings SVG activates the theme compiler and facade SVG;
  facade SVG activates the renderer. Rustybuzz, TTF parsing, WOFF2 and Brotli font decoding belong
  to that renderer closure. Slim WASM retains small catalog/operation/error wrappers, but its
  authoring dispatch rejects unavailable operations and has an empty structured preset catalog.
  It does not carry the complete theme compiler or font-processing implementation.
- Typst publish enables `analysis`, `svg`, `layout-cytoscape`, and `layout-elk`. Its recipe
  excludes math and binary export, so neither may be used to justify its size. Web render/full
  explicitly retain math and its RaTeX font resources (`ratex-svg/embed-fonts`). These resources
  were already part of the historical artifact recipes; their presence alone does not explain
  the new size increase or justify bundling an unrelated Inter font into the core library.

A current product baseline can be accepted without claiming every net byte is indispensable.
Acceptance means that required capabilities remain intact, no material unintended dependency
closure has been found, recent same-product growth has been checked, canonical packaged-artifact smoke checks have
passed, and future growth is again bounded. It does not establish minimum possible size, latency, peak memory, or a completed C7a
release matrix.

## One-source package verification

Source: `f2adde25ea39cb646ae51a9d14714b4d5cb0ccb0`, independently checked out at
`/tmp/merman-block-cluster-clean-f2adde25e`. The later `4535a29bb` changes only verification
records; runtime source and recipes match. This run uses macOS ARM64, Rust 1.95.0,
wasm-pack 0.15.0, Binaryen 131, wasm-tools 1.253.0, Node 24.13.1 and Typst 0.15.0.
Cargo and Binaryen each use two jobs/cores. No capability or optimizer recipe was changed.

The first shared-target build failed with `E0463` because `ratex_svg` could not find
`rust_embed_impl`. The precise cache failure was not isolated. A fresh, experiment-owned target
successfully rebuilt the five Web packages without source changes. No shared caches were deleted.
Existing TypeScript dependencies were reused via a temporary checkout-local symlink; this is
not a cold dependency-install measurement.

All five canonical Web builds, input freshness checks, package contracts, production WASM smoke
and DOM safety smoke passed. The existing package-group tool packed and verified all five
packages, including artifact profiles, resource paths, legal files, hashes and source SHA.
A separate consumer installed only these tarballs offline with lifecycle scripts disabled.
Chromium `151.0.7922.34` requested all five installed WASM files through the public package
entries. For each of render/full it passed:

- the complete ten-preset catalog golden and nineteen revision-88 support queries;
- three authoring error envelopes and two canonical light/dark materializations;
- Flowchart, State and Sequence light/dark/light isolation;
- an actual State shape fill edit without changing Sequence output;
- three preset exports, fifteen SVG renders, and three mounted SVGs with positive dimensions.

Slim entries initialized with unavailable structured authoring and empty preset catalogs. This
is an installed-consumer witness, not the complete browser suite or per-text visual qualification.
No package was published and no public qualification cell was promoted.


The exact-profile dependency verifier passed all six artifact closures. Normal Cargo dependency
observations confirm that the slim profiles contain no renderer, theme-contract, Rustybuzz, WOFF2,
Brotli font decoder, tree-sitter or binary export closure. Web render/full include renderer font
processing and their existing RaTeX math resources. Typst includes renderer font processing but
neither RaTeX nor binary export. Package counts are observation metadata, not size attribution.

Typst's publish package passed nineteen shared support vectors, two materializations and three
error envelopes. Typst 0.15.0 then compiled all twenty-two positive examples/tests and rejected
all nine expected compile-fail fixtures. The first smoke attempt used an external target directory;
Typst rejected the fixture because it lay outside the helper's workspace project root. The same
built artifact and manifest were copied to the checkout's own target and revalidated by the
unchanged `--skip-wasm-build` path before the successful smoke. No source or fixture was changed.
This harness currently requires its smoke target to reside within the checkout; supporting an
external target root remains a tooling limitation.

The packaged Typst WASM is byte-identical to the canonical post-link artifact: 11,601,543 bytes,
SHA-256 `47b9e3578ef9d968f487403f1b908fc72da0168155232325383da0736711caa1`.
Its retained artifact manifest contains both the source-input and tool fingerprints. Compared
with the separately verified `f151192c9` package (11,564,035 bytes), it adds 37,508 bytes (0.32%).
That is a recent-product observation, not proof of byte reproducibility across checkout paths.

## Accepted measurements and limits

Accept the current product baseline with approximately 3% headroom, rounded upward to 1,000 bytes
per metric. All twenty-four old limits failed. The new limits preserve required functionality
and the canonical build recipes; no fonts, diagnostics or resource checks were removed for size.
This is a budget adjustment, not a measured optimization. All values below are bytes.

| Artifact | Raw | Stripped/post-link | Gzip | Brotli |
| --- | ---: | ---: | ---: | ---: |
| web-analysis | 3,674,884 | 3,674,619 | 1,412,183 | 1,074,599 |
| web-ascii | 5,198,005 | 5,197,740 | 1,914,938 | 1,445,792 |
| web-editor | 3,786,008 | 3,785,743 | 1,456,735 | 1,104,869 |
| web-full | 15,924,627 | 15,924,362 | 5,938,573 | 4,367,598 |
| web-render | 14,038,712 | 14,038,447 | 5,294,417 | 3,905,473 |
| typst-wasm | 18,758,942 | 11,601,543 | 4,447,970 | 3,282,264 |

Replacement limits in `docs/release/WASM_SIZE_BUDGETS.json`:

| Artifact | Raw limit | Stripped limit | Gzip limit | Brotli limit |
| --- | ---: | ---: | ---: | ---: |
| typst-wasm | 19,322,000 | 11,950,000 | 4,582,000 | 3,381,000 |
| web-analysis | 3,786,000 | 3,785,000 | 1,455,000 | 1,107,000 |
| web-ascii | 5,354,000 | 5,354,000 | 1,973,000 | 1,490,000 |
| web-editor | 3,900,000 | 3,900,000 | 1,501,000 | 1,139,000 |
| web-full | 16,403,000 | 16,403,000 | 6,117,000 | 4,499,000 |
| web-render | 14,460,000 | 14,460,000 | 5,454,000 | 4,023,000 |

The recent `4e4f3acc3` Web control was remeasured with this run's measurement executable and
compression tools. Current-minus-control differences are small relative to the historical budget
gap. Source paths may affect exact bytes; this is not a bit-reproducibility claim.

| Artifact | Raw/stripped delta | Gzip delta | Brotli delta |
| --- | ---: | ---: | ---: |
| web-analysis | +48 | +11 | +441 |
| web-ascii | +42 | -71 | -575 |
| web-editor | +41 | +101 | -1,104 |
| web-full | +39,447 | +8,735 | +1,491 |
| web-render | +38,634 | +10,053 | +9,204 |

Final package WASM identities (Typst uses its post-link package bytes):

| Artifact | SHA-256 |
| --- | --- |
| web-analysis | `6df5391e24b3c1a004117f596a1de186e5871775cbd95710d7855ebbefc5c593` |
| web-ascii | `34d9853db9f801e08c7a60ec90bd7cca880f1664bf5366b89225ae0f8644c2d6` |
| web-editor | `491801dbda454afd2428f41bd5673044c2b06d918b66c2d5bf5722c1b61ab098` |
| web-full | `880326f97ff9d6399f90b706ee7ca19fad6a9bd87c016ab7acd1a50f1aa102f2` |
| web-render | `7ac9a524ea5c05a897800dbac5773cb0af6c40932e6831a81bf68dd95197afe2` |
| typst-wasm | `47b9e3578ef9d968f487403f1b908fc72da0168155232325383da0736711caa1` |

The ignored evidence directory is
`target/bench/experiments/theme-artifact-rebaseline-20260914-f2adde25e/`: experiment registration,
exact commands and exit statuses, before/after limits, all metrics, retained tarballs and their
source-bound group manifest, browser results, dependency observations and Typst artifact manifest.
The measurement executable is built from the same pinned checkout. The target was isolated only
after the initial shared-cache failure; TypeScript dependencies remained reused.

This accepts the six-artifact size baseline on this host. Linux publication, the complete browser
suite, the wider native/CLI/Python profile matrix, public catalog promotion and C7a contract freeze
remain separate gates. The 3% margin is a regression guard, not permission for repeated unexplained
rebaselines. Any later material growth needs its own capability and dependency explanation.

## Final verification

All **24/24** replacement-budget checks passed on a second measurement. Every raw, stripped,
gzip and Brotli value exactly matched the first measurement. The independent review checked the
rounding rule, artifact identities, package/closure results and scope statements. No production
Rust, TypeScript or build recipe changed; a full workspace regression was not rerun for this
budget/documentation-only change. The checkout-local TypeScript symlink was removed after use.

Core reproduction commands, run from the pinned source with the recorded tools and two build jobs:

```text
npm run build --prefix platforms/web
npm run smoke --prefix platforms/web
cargo run --locked --release -p xtask -- build-typst-package --profile publish --out <package-output>
cargo run --locked --release -p xtask -- typst-package-smoke --profile publish --out <package-output> --skip-wasm-build --keep-artifacts --typst <typst-0.15.0>
cargo run --locked --release -p xtask -- wasm-size-matrix --surface web --web-package-root <absolute-assembled-packages> --budget-file <absolute-replacement-budget>
cargo run --locked --release -p xtask -- wasm-size-matrix --surface typst --budget-file <absolute-replacement-budget>
```

Use a checkout-local target for Typst smoke. The full command ledger additionally records the
standard package-group packing/verification, offline install, installed-browser witness and
six-profile dependency checks. The budget paths in final checks intentionally point at the
replacement JSON while all measured runtime source remains pinned to `f2adde25e`.
