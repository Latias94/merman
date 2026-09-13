---
type: Verification Evidence
title: Installed Node authoring and support revision 82
timestamp: 2026-09-13
related_plan: docs/plans/2026-08-09-001-portable-theme-architecture-convergence-addendum.md
git_branch: refactor/presentation-theme-model
tags: theme,node,wasm,authoring,verification
---

# Scope

Both Node artifacts were rebuilt from commit `840560118e8b3be6be77b3a3ee163e8a270b79ac`
and installed from npm tarballs into separate temporary consumer projects. The native target was
`darwin-arm64`; the independent WASM package used wasm-pack's Node target. The host was macOS
ARM64 with Node 26.6.0. No package was published.

Each artifact's existing build receipt validates the source/lock/dependency closure, build recipe,
artifact bytes, and runtime catalog during package assembly. Both record source digest
`sha256:fc6e6a769aba7c6b95d758a6833b405a74d9b4b5c71910dec02ae0035a5ead65`.
Their input digests differ because their build recipes and tools differ:

| Artifact | Input digest |
| --- | --- |
| N-API darwin-arm64 | `sha256:89af2e6d5a97cd1eb3f4a9438fb16a8a461938ba81adf22609391f7e7320b447` |
| Node WASM | `sha256:2b1aea0cb39a972f3fdf6b4365e22835dd1f042186736b30fa942b7ae21c7ee5` |

# Installed observations

The release workflow's assembly, packed-package validation, npm pack/install, and
`smoke-installed-package.mjs` entry points were used. Installation used `--ignore-scripts`.
The smoke resolved the public package entry point from each consumer's installed `node_modules`.
Both returned the same authoring summary:

| Check | Count per package |
| --- | --- |
| Shared light/dark materialization vectors | 2 |
| Complete preset catalog comparisons | 2 |
| Family isolation checks | 3 |
| Rule override / complete-spec cold start | 1 each |
| Preset exports | 3 |
| Support queries | 20: ten revision-82 vectors through two engines |
| Authoring diagnostics | 6 |
| Encoded-theme resource-limit rejections | 2 |
| JSON operations / SVG renders | 23 each |

The support vectors include Class `EdgeLabelBackground.fill` returning Unsupported after its
projection retirement. SVG assertions parse actual output, require native text, and check the
visible state fill for the override witness. Catalog comparisons use the independent checked-in
preset oracle, whose qualified cells remain empty.

| Packed artifact | Bytes | SHA-256 |
| --- | ---: | --- |
| `mermanjs-node-0.8.0-alpha.6.tgz` | 112726 | `4acf37960679c5965aaf329a3292ed708ef836a198741477ca8559e8bea84fc1` |
| `mermanjs-node-darwin-arm64-0.8.0-alpha.6.tgz` | 10519125 | `cea1601cc17794db20a33309b020aba4dfab3c0a60e79de21439d17867de165c` |
| `mermanjs-node-wasm-0.8.0-alpha.6.tgz` | 7172694 | `294604c223c17d18fc11d4c5987b32249f9e227f5dc79fcabf45153b9cb6bfe1` |

# Target identity repair

The installed smoke previously accepted `--target win32-x64-msvc` on this macOS ARM64 host,
loaded the native ARM64 package, and exited successfully while reporting the target as Windows.
This reproduced an evidence-labeling error; it was not a Windows execution witness.

The script now compares a native target with the existing loader's `resolveNodeTarget()` before
loading any package. It also distinguishes GNU/musl using the same runtime detector as loading.
Node WASM retains its host-independent target. The regression runs the real script as a child
process and requires rejection for a different native target. The actual installed native package
still passes with `darwin-arm64`; the false Windows target exits 1 without emitting a success
report. The installed WASM package passes after the same change. Node tests pass 105/105.

# Reproduction and limits

Use the commands in `.github/workflows/release-node.yml`: `build-candidate.mjs --candidate napi
--target darwin-arm64` or `--candidate node-wasm`, then `assemble-packages.mjs --target
darwin-arm64` or `--wasm`, `verify-packages.mjs --packed-root`, npm pack/install, and
`smoke-installed-package.mjs --project <consumer> --version 0.8.0-alpha.6 --target <target>`.
Run `npm test --prefix platforms/node` for the ordinary contract suite.

Logs are `/tmp/theme-node-installed-20260913-{build,native,wasm-build,wasm,contracts}.log` and
`/tmp/theme-node-installed-20260913-target-{negative-before,after,test}.log`. Temporary consumer
and tarball locations are recorded in `/tmp/theme-node-installed-20260913-path.txt`.

These are actual installed Node observations for the named host and WASM runtime, not evidence
for Windows/Linux execution, Web browser packages, Typst, Python wheels, native export profiles,
nonempty catalog qualification cells, or C7a contract freeze. Prior observations from other
source revisions remain historical. No size budget or qualification claim was widened.
