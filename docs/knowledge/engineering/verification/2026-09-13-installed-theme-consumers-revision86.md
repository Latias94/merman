---
type: Verification Evidence
title: Clean-source installed Node and Python consumers at support revision 86
timestamp: 2026-09-13
related_plan: docs/plans/2026-08-09-001-portable-theme-architecture-convergence-addendum.md
git_branch: refactor/presentation-theme-model
tags: theme,node,wasm,python,authoring,verification
---

# Source and build scope

Source commit: `51f6308acb4c931ad135aca255ba940288932641`.
The detached checkout `/tmp/merman-class-title-51f6308ac` was clean before and after all builds
and installed runs. This is the same source that passed 3270 private Release owner tests and
all 706 native route-profile witnesses in the [Class Title record](2026-09-13-class-namespace-title-cutover.md).
Commit `e26c415de` adds that verification documentation without changing the built code.

These local development artifacts retain workspace version `0.8.0-alpha.6` (Python `0.8.0a6`).
No package publication occurred. Builds ran serially; Node's builder selects one Cargo job and
Python used `CARGO_BUILD_JOBS=2`. Existing Cargo caches were reused, while compilation logs
identify the clean checkout's sources. Node dependencies were copied into its ignored local
`node_modules`; no tracked source or generated projection changed.

Node uses the release workflow's pinned `24.13.1`, from the official ARM64 archive checked against
its published SHA-256 list. The Node builder recorded Rust/Cargo 1.95.0, N-API CLI 3.7.4 and
wasm-pack 0.15.0. N-API targets `aarch64-apple-darwin`; Node WASM independently targets
`wasm32-unknown-unknown` with wasm-pack's Node loader. Both recipes disable defaults and select
`layout-cytoscape,layout-elk,svg` plus their own transport feature.

Both Node receipts name the source commit above and source digest
`sha256:858b79b99b758c77c5d7797b371eca661a7eba6f9deb4a7f2177b1d52fa02796`.
Assembly rechecked recipe, dependency closure, source/lockfile, runtime contract, and payload hashes.

| Node artifact | Build input digest |
| --- | --- |
| N-API darwin-arm64 | `sha256:3d23bc8a551294a797f312dee43942ec848cf25dd204a5fb020b71f39ba957b1` |
| Node WASM | `sha256:b21a5dcf6848d434955176b8e7a838f3e1fb71c06c826c931d5e8d1c79e46ecc` |

Python used the `python-uniffi-native` recipe with `native-distribution`, target
`aarch64-apple-darwin`, defaults disabled and exactly
`analysis,ascii,layout-cytoscape,layout-elk,svg`. Bindings were generated from the production
rlib/cdylib; the builder checked committed support projections and target-specific licenses.
Python 3.12.8 built the wheel, matching CI's configured minor series. The same wheel was then
installed and tested separately with Python 3.12.8 and 3.9.6; the latter exercises the declared
`>=3.9` minimum series on this host.

# Installed observations

Assembled Node packages passed `verify-packages.mjs`, were packed, then installed into separate
fresh projects with `--ignore-scripts --offline --no-audit --no-fund`. The installed smoke resolved
ESM entry points from each consumer's `node_modules`. No transport or engine was substituted.
Both N-API and Node WASM passed:

| Check | Count per Node artifact |
| --- | --- |
| Shared light/dark materialization vectors | 2 |
| Complete preset catalog comparisons | 2 |
| Family isolation checks | 3 |
| Rule override / complete-spec cold start | 1 each |
| Preset exports | 3 |
| Support queries | 30: all 15 revision-86 vectors through asynchronous and synchronous calls |
| Authoring diagnostics | 6 |
| Additional encoded-theme resource-limit rejections | 2 |
| JSON operations / SVG renders | 23 each |

Each Python consumer installed the wheel with `--no-index --no-deps`. An explicit check required
`merman.__file__` to resolve inside that virtual environment's `site-packages`. The checked-in
`examples/smoke.py` ran with `python -E` from the consumer directory. Both versions passed the
full smoke and authoring task, including 30 shared support queries through one-shot/reusable
APIs, three catalog comparisons, six shared authoring/resource-error cases, light/dark isolation,
rule editing, complete-spec cold start and preset export equivalence.

All consumers matched the complete revision-86 support golden, including Class namespace Title's
partial typed claim. Their ten-entry preset catalogs still have empty qualification cells. The
native preset exports contained neither font assets nor Mermaid compatibility payloads.

Supplementary installed Class probes rendered a namespace with `Title.fill = #13579b`, checked
the actual label text and its inline CSS `color` and `fill`, then supplied source `titleColor`
and required it to suppress the typed paint. Node ran two renders per artifact; Python ran four
per interpreter through one-shot and reusable APIs. All four consumers produced byte-identical
21,518-byte typed SVGs with SHA-256
`155ecd18628635f224773301362cb6eabe3518fe96ca0f4e2d8103bd6c4ee1e7`.
These are terminal SVG assertions; the probes did not run browser pixel comparisons.

# Packed artifact identities

| Artifact | Bytes | SHA-256 |
| --- | ---: | --- |
| `mermanjs-node-0.8.0-alpha.6.tgz` | 112490 | `79e7cb7a32401a90b0cd2c26f4baf9862aed285e6413efab68faf9555e92b5d4` |
| `mermanjs-node-darwin-arm64-0.8.0-alpha.6.tgz` | 10979668 | `436b51a983c9730c1fc4c30f9b106558d0a8f169e2fc6655dd664f5a08aa985f` |
| `mermanjs-node-wasm-0.8.0-alpha.6.tgz` | 7238401 | `5e2d0666797487eb2963d53dffc4b687a06cdd6b5e3fb9e9eeadcd163982c7c2` |
| `merman-0.8.0a6-py3-none-macosx_11_0_arm64.whl` | 9189588 | `08298e550efe1208c92fd4036799e4a0f60b4b000c49d3dca014530e6d7f56b7` |

The N-API tarball's `package/merman.node` is 24,332,704 bytes with SHA-256
`9bb9a1b98dc5f37d29d5b4b909e49016b69562e8d108911ed33e3dd3a0d08ae4`.
The WASM tarball's `package/artifact/merman_node_bg.wasm` is 19,909,205 bytes with SHA-256
`4e77558522cbd171e0e2fd35f6a2689afe136a21d85b7ae83c8141a7228a5f3d`.
Both packed payloads matched their verified build receipt bytes and hashes exactly.

The wheel declares `Root-Is-Purelib: false` and `py3-none-macosx_11_0_arm64`.
Its `merman/libmerman_uniffi.dylib` is 22,131,744 bytes, byte-identical to the production library,
with SHA-256 `ca8cb859c7776d942e92f70e8375b56b8dbe8bde242981d1f15c7ec69f6e7e62`.
The builder's native-layout and target-license checks passed.

# Reproduction and logs

Use the existing build/assembly/install commands in the
[revision-85 record](2026-09-13-installed-theme-consumers-revision85.md#reproduction-and-logs),
with the source commit above and pinned Node 24.13.1. Build the wheel with Python 3.12 and the
`--python` option; install the same wheel separately under Python 3.12 and 3.9.

The directory recorded in `/tmp/theme-consumers-r86-path.txt` retains assembled packages,
tarballs, the wheel, isolated consumers, both Node build receipts, Class probe scripts/SVGs,
and `state.json` with source/status observations, payload identities and log hashes.
Node logs: `/tmp/theme-node-revision86-{napi-build,wasm-build,native-installed,wasm-installed}.log`.
Python logs: `/tmp/theme-python-revision86-{build,installed-312,installed-39}.log`.

# Remaining delivery boundary

These observations supersede revision-85 installed results for the named source and macOS ARM64
host. Browser Web packages, a newly built Typst WASM artifact, other native hosts, the full
artifact-profile/qualification matrix and formal public rollout still need current-source evidence.
The prior compiled C ABI/UniFFI/Typst goldens remain distinct from these installed package runs.
Web size gates remain open. Public catalog qualification cells and C7a candidate/contract freeze
are unchanged. Class Text and Block retain 36 executable legacy routes under C7b.
