# Revision-90 installed theme consumers — 2026-09-14

Source: `c68394113c079d44f28a0aaa1d743d4f38a949dc`.

## Result

Rebuilt Flutter macOS ARM64 Native Assets, Node N-API for macOS ARM64, and Node-WASM
from the same committed source after production family-provider retirement. Both Node
candidates were assembled, packed, installed into separate consumer directories offline
with lifecycle scripts disabled, and exercised through their installed public APIs.
All three consumers passed. No production source, dependency, build recipe, or size
budget changed during this verification.

Flutter passed two materializations, all 22 shared support queries, five error cases,
and three budgeted authoring operations through each of its one-shot and reusable
consumers. The separate ABI 3 Dart contract test passed. The budget matrix covers
materialize, describe support, and export preset through the actual native library.

Each installed Node transport reported the following successful checks:

| Check | Count per transport |
| --- | ---: |
| Shared materialization vectors | 2 |
| Catalog checks | 2 |
| Family light/dark isolation checks | 3 |
| Rule override checks | 1 |
| Cold spec checks | 1 |
| Preset export checks | 3 |
| Support queries | 44 |
| Authoring diagnostics | 6 |
| Resource-limit checks | 2 |
| JSON operations | 23 |
| SVG renders | 23 |

The support count exercises the 22 revision-90 vectors through both asynchronous
and synchronous operations; it does not mean 44 distinct supported mechanisms. In particular, Block Text fill is
partial typed support and Text stroke remains Unsupported. Equal smoke counts are
bounded consumer evidence, not proof that every transport behavior is identical.

## Build and consumer commands

The existing canonical builders own capability selection, artifact validation and
Node source/dependency receipts. Cargo builds ran serially; Flutter used two jobs,
and the Node builder used its existing one-job recipe. Host: macOS ARM64,
Rust 1.95.0, Node 26.6.0, and N-API CLI 3.7.4. Flutter used the existing local Dart SDK.

```text
CARGO_BUILD_JOBS=2 python3 platforms/flutter/build-native.py host
# From platforms/flutter:
dart run tool/theme_authoring_smoke.dart
dart run tool/abi3_contract_test.dart

CARGO_BUILD_JOBS=2 node platforms/node/scripts/build-candidate.mjs --candidate napi --target darwin-arm64
CARGO_BUILD_JOBS=2 node platforms/node/scripts/build-candidate.mjs --candidate node-wasm
node platforms/node/scripts/assemble-packages.mjs --target darwin-arm64 --output-root <napi-packages>
node platforms/node/scripts/assemble-packages.mjs --wasm --output-root <wasm-packages>
node platforms/node/scripts/verify-packages.mjs --packed-root <packages>
npm pack <assembled-package> --json --pack-destination <tarballs>
# In separate private consumer directories, using only the matching local tarballs:
npm install --offline --ignore-scripts --no-audit --no-fund <tarballs>
node platforms/node/scripts/smoke-installed-package.mjs --project <consumer> --version 0.8.0-alpha.6 --target darwin-arm64
node platforms/node/scripts/smoke-installed-package.mjs --project <consumer> --version 0.8.0-alpha.6 --target node-wasm
```

The native consumer installs both the loader and matching platform package; the WASM
consumer installs only its own package. Neither publishes to a registry or uses a
browser-WASM binary as a substitute for the Node-WASM build.

## Artifact observations

Values are bytes of final packaged binaries or npm archives, not peak memory or
whole-application installed footprints. The Flutter hash is taken after the canonical
builder's install-name and signing steps. Node build receipts identify the source,
lockfile, effective dependency closure, tools and final artifacts.

| Artifact | Bytes | SHA-256 |
| --- | ---: | --- |
| Flutter macOS ARM64 dylib | 21,876,896 | `cb4c907041bca16b6b915f47e18e9247d7b94292c6ee92c28555a6089ee0fb7d` |
| Node N-API macOS ARM64 binary | 24,680,416 | `1929d5868fc826adba48fe0206db40cf1d69e33291d71391553499e6d4170048` |
| Node-WASM module | 20,117,239 | `de47219b7bc8c2d3c3d4aadb3a2f65f9279b439e1b00c87f98d1fb816b5a1fe6` |
| Node platform npm tarball | 10,677,677 | `adfd8d681b2ba5647b2598abe2ea7a1bbf327af2b85c49d68527d655e2fe54a5` |
| Node loader npm tarball | 112,726 | `4acf37960679c5965aaf329a3292ed708ef836a198741477ca8559e8bea84fc1` |
| Node-WASM npm tarball | 7,242,458 | `9c9dd7647e619fa04e76a11720fe19102447ab8c3fe069d568dbd81203fb08d9` |

These values establish this build's identities and size observations. They do not
constitute an equal-profile A/B, an optimization, or approval of new native size
limits. Flutter's size-oriented `native-distribution` recipe and Node's independent
release recipe differ. The Node loader and platform tarballs are separate download
costs. No startup-time or peak-memory measurement was made here.

The [accepted Web/Typst budget reassessment](../../../performance/theme_artifact_budget_reassessment_2026-09-14.md)
retains its own source-bound measurements and approximately 3% regression margin.
Necessary capability growth may justify a new baseline after dependency attribution
and packaged-consumer checks; passing a size limit alone does not prove acceptable
startup or memory costs. This run leaves those limits unchanged.

## Scope and remaining gates

The checkout had no tracked changes during builds. The maintainer's two unrelated
untracked knowledge directories were left untouched. This is not a fresh-checkout
or cold dependency-install claim. Retained build receipts, installed-smoke logs,
tarballs, exact commands and artifact hashes are under
`target/bench/experiments/theme-consumers-revision90/`.

This closes the revision-90 Flutter/macOS and installed Node consumer verification
slice. It does not attest new Web, Typst, Python, Windows or Linux artifacts, repeat
the full workspace/browser matrices, populate public qualified cells, or freeze C7a.
The earlier clean-checkout owner run remains separately bound to `1c37f8096`.
Continue final artifact-profile verification and public discovery/catalog rollout
using the existing owner gates; do not infer global qualification from these smokes.
