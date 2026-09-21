---
type: Verification
title: Web and Typst consumers after XML reference single-pass integration
timestamp: 2026-09-20
git_commit: bdb209e1166af960121ce0e2b3231d9929d3bca1
related_plan: docs/plans/2026-09-16-1618-refactor-theme-product-boundaries-plan.md
---

# Source and scope

This refresh targets integrated source `bdb209e1166af960121ce0e2b3231d9929d3bca1`, which
contains the accepted XML/reference single-pass SVG optimization. The Web consumer lane was
rebuilt from that source on macOS ARM64 with Rust 1.95.0, Node 24.21.0, npm 12.0.2, wasm-pack
0.15.0 and wasm-tools 1.253.0. Cargo work used one build job. No tag, publication, budget change
or qualification promotion was performed.

Both Web and Typst lanes are current-source evidence. The Typst run uses the repository-local
Binaryen 131 tool at `/private/tmp/merman-binaryen-131/binaryen-version_131/bin/wasm-opt`; the
normal host PATH still exposes Binaryen 132, so the tool path is recorded explicitly for
reproduction.

# Web package and browser evidence

`npm run build` completed all five profiles, TypeScript generation and package assembly. The
contract check accepted 40 WASM exports, 50 runtime bindings and five package entries. The owner
smoke passed WASM input freshness, the 35-diagram matrix for every package, package prepack
checks and DOM safety. The package verifier accepted all five generated artifacts.

The owner `web_package_group.py pack` helper is incompatible with npm 12's JSON-array output
(the helper expects the older single-object shape). This is a tooling interoperability failure,
not a package-content failure. Each package was therefore packed with `npm pack --ignore-scripts`,
then bound by the owner's `create-manifest` and verified by `verify-artifact`; both manifest
operations passed against source `bdb209e11`, version `0.8.0-alpha.7` and the checked-in legal
digest.

| Archive | Packed bytes | SHA-256 |
| --- | ---: | --- |
| `mermanjs-web-0.8.0-alpha.7.tgz` | 6,239,780 | `8089c81e08e3eee44864ba47df57d7fb403cee4175957ef25c187aa68ef9d320` |
| `mermanjs-web-analysis-0.8.0-alpha.7.tgz` | 1,518,943 | `36c20f39393a9b4c056aca6e0788e64605c99097c870acd76d5acd946ddc9b6c` |
| `mermanjs-web-ascii-0.8.0-alpha.7.tgz` | 2,020,891 | `77811bbfdeddbced1c4150041a06732713b9f0833b4d12ebada12d98742ecad4` |
| `mermanjs-web-editor-0.8.0-alpha.7.tgz` | 1,569,037 | `a847879ad64e35d163d1b3e105ff4a2f428be9e84c24391f349f4b148027a463` |
| `mermanjs-web-render-0.8.0-alpha.7.tgz` | 5,281,104 | `a3d24720784674f5a2cd1e2b386665918c06b20e6a7681f6bd3736a01fd1fa79` |

The generated group manifest records source `bdb209e1166af960121ce0e2b3231d9929d3bca1`,
version `0.8.0-alpha.7`, target tag `alpha` and legal digest
`sha256:273b430589c2d5a440efe481c1ff625d1bd6b098e6bbb08c8b7818e1db99a67e`.

# Web artifact sizes

The Web size matrix passed all five checked-in budget rows without changing
`docs/release/WASM_SIZE_BUDGETS.json`.

| Profile | Raw bytes | Stripped bytes | Gzip bytes | Brotli bytes |
| --- | ---: | ---: | ---: | ---: |
| web-analysis | 3,691,430 | 3,691,165 | 1,421,467 | 1,080,046 |
| web-ascii | 5,219,051 | 5,218,786 | 1,925,986 | 1,454,210 |
| web-editor | 3,802,651 | 3,802,386 | 1,464,921 | 1,110,249 |
| web-full | 16,323,884 | 16,323,619 | 6,086,055 | 4,474,404 |
| web-render | 13,726,102 | 13,725,837 | 5,139,521 | 3,795,588 |

These are artifact footprints, not startup, throughput, browser text measurement or peak-memory
measurements. The size matrix is current-source evidence; it does not claim a matched alpha.6
delta or attribute a package-size change to the XML optimization alone.

# Typst package and artifact evidence

With Binaryen 131 selected explicitly, `typst-package-smoke --profile publish` passed the real
package consumer: 22 positive fixtures, nine expected compile failures, 22 support vectors, two
materializations and three structured errors. The package is Typst `0.3.0`; the local compiler is
Typst 0.15.1. The installed plugin is 11,207,404 bytes with SHA-256
`d7b953e3d03b2b285a8fa396e527e3843448225568f5bda7f41e821a4a5c4674`.

The current-source Typst size row also passes without changing the checked-in budget:

| Profile | Raw bytes | Stripped bytes | Gzip bytes | Brotli bytes |
| --- | ---: | ---: | ---: | ---: |
| typst-wasm | 18,355,688 | 11,207,404 | 4,258,306 | 3,148,461 |

The exact reproduction adds the Binaryen path to `PATH` before both owner commands:

```console
PATH=/private/tmp/merman-binaryen-131/binaryen-version_131/bin:$PATH cargo run --locked -p xtask -- typst-package-smoke --profile publish --out <output> --keep-artifacts --typst /opt/homebrew/bin/typst
PATH=/private/tmp/merman-binaryen-131/binaryen-version_131/bin:$PATH cargo run --locked -p xtask -- wasm-size-matrix --surface typst --budget-file docs/release/WASM_SIZE_BUDGETS.json
```

This proves the integrated source on the local Binaryen-131/Typst-0.15.1 lane; CI's pinned
Typst compiler, other hosts and registry installation remain separate boundaries.

# Evidence location and limits

The retained artifacts are under
`target/bench/experiments/installed-theme-consumers-bdb-20260920/`, including `web-smoke.log`,
`web-verify-packages.log`, `web-wasm-size-matrix.log`, the five Web archives,
`web-package-group.json`, `typst-binaryen131-smoke.log`, `typst-binaryen131-size.log` and the
installed Typst package under `typst-binaryen131/`. The earlier failed PATH run remains in
`typst-smoke.log` and `typst-wasm-size-matrix.log` as provenance for the toolchain diagnosis.

This is local macOS ARM64 evidence. Registry installation, other operating systems, other browsers, CI's pinned Typst compiler,
visual preset qualification, cold start, first render, throughput, large-diagram memory and
matched alpha.6 attribution remain outside this refresh.
The broader audit and C7a eligibility remain open.
