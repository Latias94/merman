---
type: Verification Evidence
title: Clean-source Web packages at support revision 86
timestamp: 2026-09-13
related_plan: docs/plans/2026-08-09-001-portable-theme-architecture-convergence-addendum.md
git_branch: refactor/presentation-theme-model
tags: theme,web,wasm,authoring,verification
---

# Source and scope

Source: `4e4f3acc336ac1beaad046cb335b9bfccb752f85`.
The detached checkout `/tmp/merman-theme-sort-4e4f3acc3` was clean before and after the five-profile
build and package smoke. Its production code is identical to the installed Node/Python source
`51f6308ac`; the intervening commits contain verification documentation. All builds used the
unchanged artifact profiles, locked dependencies, `wasm-size` Cargo profile and wasm-pack
post-link recipe. Cargo and Binaryen each used at most two jobs. Shared Cargo caches were reused;
workspace sources were rebuilt from the isolated checkout.

Toolchain: macOS ARM64, Rust/Cargo 1.95.0, Node 24.13.1, wasm-pack 0.15.0, Binaryen 131,
and wasm-tools 1.253.0. Packages retain development version `0.8.0-alpha.6`. Nothing was published.

The baseline passed 324 renderer theme tests, all five package smoke lanes and the built DOM
safety smoke. Each assembled package was packed with npm, then installed offline into one fresh
consumer with install scripts disabled. Chromium `151.0.7922.34` loaded the installed package
public entries and their actual WASM payloads.

# Installed observations

Full and render each passed all 15 revision-86 support vectors, the complete ten-preset catalog,
three shared authoring/resource diagnostic vectors, two materialization vectors, three-family
light/dark/light isolation, a State rule edit and three preset exports. Each produced 15 SVGs
and mounted three diagrams with positive bounds. SHA-256 identities for all 30 returned SVGs
were retained for differential comparison. Slim profiles initialized, returned their structured
empty preset catalogs, and exposed no materialization API.

This refreshes the earlier revision-85 browser observation. It does not qualify arbitrary themes,
prove pixel parity, promote public catalog cells, or cover other hosts and artifact profiles.

# Final artifact size

Every one of the twenty existing size checks still fails. Budgets and capabilities were unchanged.
Compression uses the stripped artifact and the same checked-in size-matrix implementation for all
five rows. These are fresh measurements, not estimated changes from historical symbol sizes.

| Profile | Raw bytes | Stripped bytes | gzip bytes | Brotli bytes |
| --- | ---: | ---: | ---: | ---: |
| web-analysis | 3,674,836 | 3,674,571 | 1,412,172 | 1,074,158 |
| web-ascii | 5,197,963 | 5,197,698 | 1,915,009 | 1,446,367 |
| web-editor | 3,785,967 | 3,785,702 | 1,456,634 | 1,105,973 |
| web-full | 15,885,180 | 15,884,915 | 5,929,838 | 4,366,107 |
| web-render | 14,000,078 | 13,999,813 | 5,284,364 | 3,896,269 |

| WASM payload | SHA-256 |
| --- | --- |
| analysis | `201937f5fa08214b4f0ba46402373673ee9a9bb52c4a157a2d85b64825218a1e` |
| ascii | `677ee22b5b6b9f1e1aa4bf2b59bf8904e0a2febdf5d1f8bd35bb07cdd55479f1` |
| editor | `63e6812cc797aa2ddc31e73c61b2f98155914deeb68cb3f4fc1916d66180e9f2` |
| full | `de6c8c54bed2934150730a0a50bf4d19af2ea1cd510039e64b8e529e15e44a40` |
| render | `e394063e69a1b12d0bb076fbd9c4273ee3203dc7d91112421dc33134da6669f9` |

# Reproduction and limits

The baseline commands were:

```console
CARGO_BUILD_JOBS=2 cargo nextest run --locked --release -p merman-render --lib -E 'test(diagram_theme::)'
CARGO_BUILD_JOBS=2 BINARYEN_CORES=2 npm run build --prefix platforms/web
npm run smoke --prefix platforms/web
xtask wasm-size-matrix --surface web --web-package-root /tmp/merman-theme-sort-4e4f3acc3/platforms/web/packages --budget-file /Users/frankorz/Documents/projects/rust/merman/.worktrees/presentation-theme-model/docs/release/WASM_SIZE_BUDGETS.json
```

Use an xtask built from the desired source. Absolute artifact and budget paths prevent an existing
xtask binary from silently resolving inputs against its own build checkout. The size command exits
1 because the recorded budgets fail. Raw logs, tool/lockfile/corpus digests, package copies and
browser output live under `target/bench/experiments/theme-canonical-sort-4e4f3acc3/` and
`/tmp/theme-sort-baseline-*.log`.

The same isolated checkout subsequently hosts an unaccepted sorting experiment. Its candidate
changes and observations are excluded from this baseline record. C7a artifact size, qualification
publication, the remaining profile/host matrix and formal rollout/freeze remain open.
