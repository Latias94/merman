---
type: Verification
title: Web and Typst consumers after reference graph reuse
timestamp: 2026-09-20T18:21:31Z
git_commit: f3a783c2944a77bc3b561b29f64b41af450ebe8b
related_plan: docs/plans/2026-09-16-1618-refactor-theme-product-boundaries-plan.md
---

# Source and scope

All five Web packages and the Typst publish profile were rebuilt from immutable source
`f3a783c2944a77bc3b561b29f64b41af450ebe8b`, including `b707f6e5b`'s reference-graph allocation
optimization. The subsequent `9de95db0f` commit changes documentation only. This refresh reuses
the previous detached checkout and workspace target cache, with sequential one-job Cargo builds.
Tracked source remains unchanged; the only untracked entry is the `target` cache symlink.
No tag, publication, budget change or qualification promotion is part of this work.

Tools are Rust 1.95.0, Node 24.21.0, npm 12.0.2, wasm-pack 0.15.0, Binaryen 131 and wasm-tools
1.253.0 on macOS ARM64. Chromium is 151.0.7922.34 through Playwright 1.62.1. Local Typst is
0.15.1; CI and the crate-release workflow pin 0.15.0. The Typst result therefore proves this
host tool lane, not execution with the pinned CI compiler.

# Web package and browser evidence

The existing `npm run build` builds all five profiles, compiles TypeScript and assembles the
package group. The Web contract suite passes 147/147. The complete owner smoke passes WASM
input freshness, prepack checks, the 35-family matrix for every package and DOM safety.

The existing `web_package_group.py pack` and `verify-artifact` owners create and verify five
npm archives, bound to the source above. Their legal digest is
`sha256:273b430589c2d5a440efe481c1ff625d1bd6b098e6bbb08c8b7818e1db99a67e`.
A fresh consumer installs all five archives offline with lifecycle scripts disabled.

A real headless Chromium page imports each installed package's declared public entry and uses
its normal `initMerman()` loader. All five WASM requests return HTTP 200 from their installed
package paths. Runtime capabilities equal the checked-in artifact recipe. Analysis, editor and
ASCII expose no structured theme catalog; full and ASCII also produce expected ASCII labels.

Each full/render package passes the 22 shared support vectors, two light/dark materialization
goldens and three structured authoring error vectors, preserving diagnostic envelopes. Their
preset catalogs match the shared golden and their complete Cyberpunk exports equal the saved
recipe from the [Python/Node refresh](2026-09-21-installed-theme-consumers.md).

Both browser packages read that complete recipe directly and render the unchanged Flowchart,
Sequence and XY fixtures with native labels and the same per-family diagram IDs. Each output
is valid SVG without `foreignObject`, matches its direct preset render, and equals the
installed Python/Node output byte-for-byte. These six browser comparisons extend the existing
three-transport witness to Web full and render. They do not measure browser typography or
establish visual preset qualification.

# Typst package evidence

`typst-package-smoke --profile publish` rebuilds the exact artifact recipe and creates a real
package consumer. It compiles 22 positive fixtures, rejects nine expected compile failures,
and passes 22 shared support vectors, two materializations and three structured errors.
The independently versioned Typst package remains `0.3.0`, with plugin ABI 3 and Rust package
`0.8.0-alpha.7`.

The packaged `merman_typst_plugin.wasm` is 11,198,698 bytes, SHA-256
`0f4ef79e45058113070e1bf8d665efcce7f0bb2e94789017ec499fa853b7a7c8`, exactly matching the
owner's artifact manifest. That manifest records Binaryen 131, source input digest
`becd2c4e79af87aa442e1d90fab21132f18dc1f4b9ba8989c3e18c904bfbf14e` and root Cargo.lock digest
`b5ffed4195632a60a41be06544d43103d6c33617dc4fc7f4642d9a3ea2cab8f0`.

# Artifact size and budget evidence

The existing `wasm-size-matrix` passes all 24 raw/stripped/gzip/brotli checks for the six profiles.
`docs/release/WASM_SIZE_BUDGETS.json` is unchanged, SHA-256
`360eef84cf282df5804808d6f14ebb40b3bae017d740394b765129bace0a2bae`.
Measurements are artifact footprints, not startup, throughput or peak memory.

| Profile | Raw bytes | Stripped/post-link bytes | Gzip bytes | Brotli bytes |
| --- | ---: | ---: | ---: | ---: |
| web-analysis | 3,691,710 | 3,691,445 | 1,421,598 | 1,080,974 |
| web-ascii | 5,219,333 | 5,219,068 | 1,926,045 | 1,454,347 |
| web-editor | 3,802,933 | 3,802,668 | 1,464,912 | 1,111,529 |
| web-full | 16,314,293 | 16,314,028 | 6,084,368 | 4,474,308 |
| web-render | 13,717,169 | 13,716,904 | 5,136,993 | 3,797,215 |
| typst-wasm | 18,347,186 | 11,198,698 | 4,256,499 | 3,148,259 |

The five installed npm archive identities are:

| Archive | Bytes | SHA-256 |
| --- | ---: | --- |
| `mermanjs-web-0.8.0-alpha.7.tgz` | 6,237,539 | `38f0457457f5e03d59f3c1abfb281fb2e8a556c2f7303a5c6ba17d0162267ebb` |
| `mermanjs-web-analysis-0.8.0-alpha.7.tgz` | 1,519,015 | `aa6b8a96b18d928a8dcb16d2972e47ba1093cee4f983dbe745cebefa380e37c5` |
| `mermanjs-web-ascii-0.8.0-alpha.7.tgz` | 2,020,968 | `dd15e41c4d68027a398fa8b53a37a65927ebf9b02773184fd6c369d01ba2aa05` |
| `mermanjs-web-editor-0.8.0-alpha.7.tgz` | 1,569,007 | `9cc4d04b8d06cb07883858627e18e72d171e841abe1b26eef449689aaa558a26` |
| `mermanjs-web-render-0.8.0-alpha.7.tgz` | 5,280,056 | `be403f51fc77e123a9cd09cf804a738521ca02f6b2e1cfd6c8673dfd6ee52655` |

These are current-source observations, not matched alpha.6 deltas or attribution to one
optimization. `artifact-identities.json` also binds the individual Web WASM files, Typst plugin,
logs, browser evidence and drivers; its SHA-256 is
`b32d8278a11aac2c31eced22bd85dfa78533ddf930e414129e071b847c2ff69f`.

The exact size and Typst consumer commands were:

```console
cargo run --locked -p xtask -- typst-package-smoke --profile publish --out <new-output> --keep-artifacts --typst /opt/homebrew/bin/typst
cargo run --locked -p xtask -- wasm-size-matrix --surface web --web-package-root platforms/web/packages --budget-file docs/release/WASM_SIZE_BUDGETS.json
cargo run --locked -p xtask -- wasm-size-matrix --surface typst --budget-file docs/release/WASM_SIZE_BUDGETS.json
```

The selected Binaryen 131 directory precedes the system Binaryen 132 on `PATH`.

# Evidence location and limits

`target/bench/experiments/web-typst-consumers-20260921/` retains build/test logs, the verified
Web package group, installed consumer, browser driver/results and generated Typst package.
`context.json` records the fixed source checkout and tool paths; `environment.json` records
actual tool versions and collection time (`2026-09-21T02:21:31+08:00`, September 20 UTC).
The date in the evidence paths follows the host's Asia/Shanghai clock. The package smoke retains its
compiled fixtures under the exact `target/typst-package-smoke/run-*` path in `typst-smoke.log`.

This is local artifact and consumer evidence. Registry installs, other operating systems,
other browsers and the CI-pinned Typst compiler remain unverified. Successful fixture renders
do not qualify the preset portfolio or turn constrained Typst resource rejections into support.
Cold start, first render, throughput, large-diagram memory and matched alpha.6 attribution are
outside this refresh; the broader audit and C7a eligibility remain open.
