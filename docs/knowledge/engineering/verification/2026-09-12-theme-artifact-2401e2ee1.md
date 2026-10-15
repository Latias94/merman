---
type: Verification Evidence
title: Theme artifact installation checks at 2401e2ee1
timestamp: 2026-09-12
related_plan: docs/plans/2026-08-09-001-portable-theme-architecture-convergence-addendum.md
git_branch: refactor/presentation-theme-model
git_commit: 2401e2ee180f6cf72fc52a8622043a15ffde1318
tags: theme,artifacts,verification
---

# Scope

These are local artifact observations for source commit
`2401e2ee180f6cf72fc52a8622043a15ffde1318`, version `0.8.0-alpha.6`, on
macOS ARM64. They do not close C7a, authorize publication, or certify other native targets.
The CLI was built in a clean detached checkout. Python and Node were rebuilt in the primary
worktree with no tracked changes; unrelated untracked maintainer files were left untouched.
Web runtime artifacts were also built from that revision. Packing exposed a relative-output-path
bug in `scripts/web_package_group.py`; the tarballs below were packed with the two-line path fix
recorded alongside this document. The runtime source and generated package contents were unchanged.

# Verified artifacts

| Artifact | SHA-256 | Observed result |
| --- | --- | --- |
| CLI `merman-cli-aarch64-apple-darwin.tar.xz` | `d1553310974a457b38e8f5d079347815c4d018e613536ed4c00b760e06ac3abd` | Archive contents, executable resource contract, preset collection, and replay passed. |
| Python `merman-0.8.0a6-py3-none-macosx_11_0_arm64.whl` | `e98d99f568ce5bca0a71710beba03d7688133c10c313ffd930deb00397f01de8` | Native platform and license report checks, installation in a fresh venv, and Python smoke passed. |
| Node `mermanjs-node-wasm-0.8.0-alpha.6.tgz` | `3633b7ff91c6abd657b75da0fde33d5b028237222270c665acb0f8140e7e0b11` | Candidate probes, package contract verification, npm packing and installation, and installed-package smoke passed. |

CLI qualification covers Brutalist, Spotless, and Cyberpunk, each over Flowchart, State, and
Sequence in SVG and PNG: 18 cells under
`native-flowchart-state-sequence-system-fonts-v3`, all `host_dependent`.
The archive-bound companion catalog carries those cells; this does not populate the shared SDK
catalog or establish portable output on arbitrary hosts. Replay rebuilt/reused the qualification
runner and executed the comparisons again against the extracted CLI.

Python smoke exercised the shared authoring error vectors, three-family isolation, rule override,
cold spec, preset export, discovery, and structured resource errors. Node installed-package smoke
reported 23 SVG renders, 25 JSON operations, four authoring diagnostic checks, and two resource-limit
checks. These observations do not constitute a unified profile/admission golden across transports.

# Reproduction

Use the pinned repository toolchain at the source commit above. Run Cargo builds sequentially.
The CLI commands were run in `/tmp/merman-artifact-2401e2ee1`, with `CARGO_BUILD_JOBS=2` and
`CARGO_TARGET_DIR` pointing to the primary worktree target directory. cargo-dist wrote archives to
the detached checkout's `target/distrib`.

```text
dist build --tag=v0.8.0-alpha.6 --artifacts=local --target=aarch64-apple-darwin
python3 scripts/verify_cli_release_archive.py target/distrib/merman-cli-aarch64-apple-darwin.tar.xz --checksum target/distrib/merman-cli-aarch64-apple-darwin.tar.xz.sha256 --target aarch64-apple-darwin --version 0.8.0-alpha.6 --repo-root . --execute --preset-qualification-output target/preset-qualification-2401e2ee1.json
python3 scripts/verify_cli_release_archive.py target/distrib/merman-cli-aarch64-apple-darwin.tar.xz --checksum target/distrib/merman-cli-aarch64-apple-darwin.tar.xz.sha256 --target aarch64-apple-darwin --version 0.8.0-alpha.6 --repo-root . --execute --preset-qualification-check target/preset-qualification-2401e2ee1.json
python3 scripts/build-python-uniffi-wheel.py --wheel-dir target/python-wheel-2401e2ee1 --run-smoke
node platforms/node/scripts/build-candidate.mjs --candidate node-wasm
node platforms/node/scripts/assemble-packages.mjs --wasm --output-root target/node-wasm-package-2401e2ee1
node platforms/node/scripts/verify-packages.mjs --packed-root target/node-wasm-package-2401e2ee1
```

The assembled Node package was packed with `npm pack`, installed with
`npm install --ignore-scripts --no-audit --no-fund` in
`target/node-wasm-installed-2401e2ee1`, then checked with:

```text
node platforms/node/scripts/smoke-installed-package.mjs --project target/node-wasm-installed-2401e2ee1 --version 0.8.0-alpha.6 --target node-wasm
```

# Web package group

All five descriptor-owned WASM profiles were rebuilt, assembled, and checked. Full package-group
smoke passed for full, analysis, render, editor, and ascii, including real WASM execution, resource
catalog expectations, authoring diagnostics on rendering surfaces, package isolation, and the DOM
safety helper smoke. This was the repository's Node-driven Web smoke, not browser screenshot QA
or installation testing in an external browser application.

The final tarballs passed `web_package_group.py verify-artifact`, including their packed file,
provenance, legal, and package-group checks. Their manifest records the runtime source commit above.

| Package | Tarball SHA-256 | Packed bytes |
| --- | --- | --- |
| `@mermanjs/web-analysis` | `496bdd5f78b4a5f4a1817c26a4f853cf5b09dd2d5e77c48153af6e15a4b752d1` | 1520034 |
| `@mermanjs/web-render` | `0ba305774e54eb4a963f28d9be6b0657627381ad504f7ba83f6d87c6b76e671c` | 5414919 |
| `@mermanjs/web-editor` | `3dc54f3dca6776560c5f8d3c66955affb0eb30a22449bd5e4bfbf6c88dfcb41a` | 1571617 |
| `@mermanjs/web-ascii` | `6faa8602d4a4691befe9aa8d7cac16f04d13d7a3e7d01eae58470882b19d2339` | 2021282 |
| `@mermanjs/web` | `b76e6a591b212b0ff5784a184a3c9aac4d540537ed77e4d6957f0b03250b8d76` | 6072422 |

Commands (all successful after fixing the relative output path):

```text
CARGO_BUILD_JOBS=2 npm run build --prefix platforms/web
npm run smoke --prefix platforms/web
python3 scripts/web_package_group.py pack --root . --descriptor platforms/web/web-surface-descriptor.json --artifact-dir target/web-package-group-2401e2ee1 --version 0.8.0-alpha.6 --source-sha 2401e2ee180f6cf72fc52a8622043a15ffde1318 --target-dist-tag alpha
python3 scripts/web_package_group.py verify-artifact --manifest target/web-package-group-2401e2ee1/web-package-group.json --artifact-dir target/web-package-group-2401e2ee1 --version 0.8.0-alpha.6 --source-sha 2401e2ee180f6cf72fc52a8622043a15ffde1318 --descriptor platforms/web/web-surface-descriptor.json
```

The new destination-path regression failed before the fix and passed afterward; all 26 package-group
unit tests passed. The fix resolves the output directory in the caller's working directory before
starting npm from each package directory. Existing release jobs supply absolute directories.

# Unclosed WASM size gate

The exact workflow command did not reach measurement because its Debug `xtask` build exhausted
local disk space (`No space left on device`). No other session's files were removed.

```text
CARGO_BUILD_JOBS=2 cargo run --locked -p xtask -- wasm-size-matrix --surface web --web-package-root platforms/web/packages --budget-file docs/release/WASM_SIZE_BUDGETS.json
```

As supplemental evidence, the existing local `target/release/xtask` executable was run with the same
arguments. Its source freshness was not re-established by this build; this is not a passing
substitute for the workflow command. It measured the freshly built package WASM files and rejected
all five profiles on raw, stripped, gzip, and Brotli budgets. The raw byte counts below were also
checked directly against the files. The budget file was left unchanged.

| Profile | Raw bytes | Maximum raw bytes | Excess |
| --- | --- | --- | --- |
| `web-analysis` | 3,784,417 | 3,600,000 | 5.12% |
| `web-ascii` | 5,307,546 | 5,125,000 | 3.56% |
| `web-editor` | 3,895,567 | 3,775,000 | 3.19% |
| `web-full` | 15,935,669 | 14,800,000 | 7.67% |
| `web-render` | 14,048,792 | 12,625,000 | 11.28% |

This is a current artifact-size failure requiring attribution and optimization or a separately
justified budget decision. Functional package smoke does not close that release gate.

# Review reconciliation and remaining gates

- Ordinary Node CI already assembles, packs, installs, and smoke-tests Node-WASM and N-API on
  non-push runs when the Node owner is selected (`.github/workflows/ci.yml`). Push runs skip those
  build steps. This is not exclusively a release gate.
- Flowchart qualification already rejects damaged labels, marker references/identities, empty edge
  geometry, and missing label backgrounds. Its raster tests also erase paint and background regions
  (`crates/merman-theme-acceptance/src/support/preset_flowchart_proof.rs`). This session inspected
  those negative cases; it did not rerun their test suite during artifact packaging.
- The shared authoring diagnostic vectors now cover Web, Node, Typst, UniFFI, Native C ABI, and Python.
  Unified profile/admission expectations and first-party public discovery rollout still need closure.
- Full C5 classification, the required primary writer tranche, public artifact-bound qualification
  rollout, and candidate/rollout/contract decisions remain governed by the convergence addendum.
- Remaining family bridges and retirement obligations remain C7b work. Successful artifact smoke
  does not prove bridge retirement.
