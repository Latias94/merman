---
type: Verification Evidence
title: Installed Node and Python consumers at support revision 85
timestamp: 2026-09-13
related_plan: docs/plans/2026-08-09-001-portable-theme-architecture-convergence-addendum.md
git_branch: refactor/presentation-theme-model
tags: theme,node,wasm,python,authoring,verification
---

# Source and artifact scope

All three artifacts were rebuilt from `c00de7fa16046152222a6cc993bcdbf9f17119d6` on macOS ARM64,
then installed into separate temporary consumer projects. Tracked source files were unchanged
through all builds and installed runs; the two pre-existing untracked engineering directories
were left untouched. This was the active checkout, not a new clean-checkout release candidate.
Cargo builds ran sequentially using the existing target directory. The Node builder selected
one build job; the Python builder used `CARGO_BUILD_JOBS=2`.

The N-API artifact targets `darwin-arm64`; Node WASM uses the independent `node-wasm` recipe and
wasm-pack's Node target. Both use `layout-cytoscape,layout-elk,svg`, plus their transport feature,
with default features disabled. Node was 26.6.0. Existing build receipts checked source/lockfile,
dependency closure, build recipe, artifact bytes, and the probed runtime contract during assembly.
Both receipts name the source commit above and source digest
`sha256:611de8e3c71fa0cac36492d1455af4dbe00a15d80e8319a6c2f29dae8d59e29e`.

| Node artifact | Build input digest |
| --- | --- |
| N-API darwin-arm64 | `sha256:0d68cd5562af7c2bafed4692488db9885d1c04e72014daaabf46881c995ceb03` |
| Node WASM | `sha256:ace231f642a28ec8a2c26380fa472afaf6264a3d015867a4e6a8af553123a646` |

Python used the `python-uniffi-native` artifact recipe: `aarch64-apple-darwin`,
`native-distribution`, and exactly `analysis,ascii,layout-cytoscape,layout-elk,svg` with default
features disabled. The builder regenerated bindings from the production rlib/cdylib, checked
committed support projections, and verified native wheel layout and target-specific licenses.
The installed Python version was 3.14.6. The wheel declares
`py3-none-macosx_11_0_arm64` and `Root-Is-Purelib: false`; its bundled
`merman/libmerman_uniffi.dylib` is 22115200 bytes with SHA-256
`7e4cca5990ff09f7036513d69ef2f05a1d869deb91536e1df9faa65d0f9ae859`.

# Installed observations

Node package assembly and packed-package validation passed before npm pack/install. Each fresh
consumer installed local tarballs with `--ignore-scripts --offline --no-audit --no-fund`.
`smoke-installed-package.mjs` resolved each public entry point from that consumer's installed
`node_modules`. Both artifacts returned identical authoring summaries:

| Check | Count per Node artifact |
| --- | --- |
| Shared light/dark materialization vectors | 2 |
| Complete preset catalog comparisons | 2 |
| Family isolation checks | 3 |
| Rule override / complete-spec cold start | 1 each |
| Preset exports | 3 |
| Support queries | 28: fourteen revision-85 vectors through asynchronous and synchronous calls |
| Authoring diagnostics | 6 |
| Additional encoded-theme resource-limit rejections | 2 |
| JSON operations / SVG renders | 23 each |

The Python wheel was installed with `--no-index --no-deps` into a new virtual environment.
An explicit check required `merman.__file__` to resolve under that environment's `site-packages`.
The checked-in `examples/smoke.py` ran with `python -E` from the consumer directory. The full
UniFFI smoke passed, including 28 support queries through one-shot and reusable APIs, three
catalog comparisons, and six shared authoring/resource-error observations. Light/dark canonical
materialization, three-family reuse and isolation, rule editing, complete-spec cold start, and
preset export equivalence also passed.

All consumers compared complete responses against the shared `authoring-v1/support.json` golden,
including revision 85, the retired Class background, typed Block background, and typed Class
Edge.fill and Cluster fill/stroke queries. Preset metadata matched the independent ten-entry
catalog golden. Its qualified cells remain empty. The native preset export checks still require
no embedded font assets or Mermaid compatibility payload.

| Packed artifact | Bytes | SHA-256 |
| --- | ---: | --- |
| `mermanjs-node-0.8.0-alpha.6.tgz` | 112726 | `4acf37960679c5965aaf329a3292ed708ef836a198741477ca8559e8bea84fc1` |
| `mermanjs-node-darwin-arm64-0.8.0-alpha.6.tgz` | 10565069 | `cc3a7a836a6faf69fbaa3e9aee4794108e25fc6033a60af8d8d058ca8e4df7e5` |
| `mermanjs-node-wasm-0.8.0-alpha.6.tgz` | 7188736 | `3205fc5d905d5bfa1a4e2fb1432bbe8dae4c30a758b00094521cc7646798edf7` |
| `merman-0.8.0a6-py3-none-macosx_11_0_arm64.whl` | 9186674 | `05c6813185cfb7a4b505b0dbe11e58afa746faa80766509a78eb0884338892fd` |

# Reproduction and logs

Use the existing Node release entry points:

```text
node platforms/node/scripts/build-candidate.mjs --candidate napi --target darwin-arm64
node platforms/node/scripts/build-candidate.mjs --candidate node-wasm
node platforms/node/scripts/assemble-packages.mjs --target darwin-arm64 --output-root <native-packages>
node platforms/node/scripts/assemble-packages.mjs --wasm --output-root <wasm-packages>
node platforms/node/scripts/verify-packages.mjs --packed-root <packages>
```

Pack each assembled package with `npm pack --pack-destination <tarballs>`, install the tarballs
into separate consumer projects, then run `smoke-installed-package.mjs --project <consumer>
--version 0.8.0-alpha.6 --target <darwin-arm64-or-node-wasm>`.

```text
CARGO_BUILD_JOBS=2 python3 scripts/build-python-uniffi-wheel.py --wheel-dir target/python-wheel-revision85-c00de7fa1
python3 -m venv <consumer>/venv
<consumer>/venv/bin/python -m pip install --no-index --no-deps <wheel>
<consumer>/venv/bin/python -E <repo>/platforms/python/merman/examples/smoke.py
```

Additional checks on the same source passed: ordinary Node contracts 105/105, rebuilt Web
TypeScript and theme-catalog tests 8/8, release workflow security guards 33/33, and focused Python
authoring/license/platform scripts 36/36. The Web tests preserve unknown profile/admission IDs
and reject missing conditions or duplicate qualification cells; they use a stub transport, not
a newly built browser WASM artifact.

Node logs are `/tmp/theme-node-revision85-{napi-build,wasm-build,native-installed,wasm-installed,contracts}.log`.
Consumer/tarball locations and preserved receipt copies are under the directory recorded in
`/tmp/theme-node-revision85-path.txt`. Python logs are
`/tmp/theme-python-revision85-{build,installed,contracts}.log`; the consumer path is recorded in
`/tmp/theme-python-revision85-path.txt`. Additional logs are
`/tmp/theme-revision85-{web-build-ts,web-catalog,workflow-security}.log`.

# Remaining delivery boundary

These results replace revision-82 Node/Python observations for the named source and host.
The earlier records remain historical. No packages were published and no size budgets changed.
No full workspace, full browser suite, browser WASM rebuild, Typst rebuild, or other host-platform
build was run in this slice. This does not qualify native PNG/JPEG/PDF profiles, populate public
catalog cells, finish complete C5 classification/primary-writer closure, or close the C7a
candidate, rollout, and contract-freeze gates.
