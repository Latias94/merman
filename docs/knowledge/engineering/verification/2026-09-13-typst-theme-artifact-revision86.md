---
type: Verification Evidence
title: Clean-source Typst artifact at support revision 86
timestamp: 2026-09-13
related_plan: docs/plans/2026-08-09-001-portable-theme-architecture-convergence-addendum.md
git_branch: refactor/presentation-theme-model
tags: theme,typst,wasm,authoring,verification
---

# Source and scope

Source: `f151192c9a55199b9863f2f2a11ce730d43463c8`.
The detached checkout `/tmp/merman-typst-f151192c9` was clean before and after verification.
Shared Cargo caches were reused; workspace Rust sources and manifests were refreshed before the
serial builds, and compiler logs identify the detached checkout. Cargo and Binaryen each used
at most two jobs. This slice changes the final-artifact verifier, shared diagnostic expectations,
and tests; it does not change production rendering or the theme protocol.

The actual `publish` package uses artifact profile `typst-wasm`, target `wasm32-unknown-unknown`,
Cargo profile `wasm-size`, no default features, and `analysis`, `layout-cytoscape`, `layout-elk`,
`svg`. The artifact owner applies the canonical Binaryen/strip recipe before the package receives
its WASM. Workspace/plugin version remains `0.8.0-alpha.6`, Typst package version `0.3.0`, and
plugin ABI 4. Nothing was published.

Toolchain: macOS ARM64, Rust/Cargo 1.95.0, Binaryen 131 and wasm-tools 1.253.0. The real consumer
used CI-pinned `typst 0.15.0 (3ae52774)`, downloaded from the official Typst release. Its locally
recorded binary SHA-256 is `3719b5ca1324cfc38e57d45c2379dfee14f3c6a6117a173b91a22a9e9cf90e2f`;
this is a local identity record, not an independently verified signed checksum.

# Executed checks

| Check | Clean-source result |
| --- | --- |
| Release Typst plugin and final-artifact verifier tests, production features | 41 passed; 619 unrelated tests skipped |
| Release Typst plugin, no default features | 14 passed |
| Shared vectors through the optimized WASM ABI in Wasmi | 15 complete support responses at revision 86, two materializations, three authoring/resource errors |
| Complete shared preset catalog | Ten ordered descriptors, including empty qualification cells |
| Actual installed Typst package through pinned CLI | 22 compilations succeeded; nine negative fixtures failed as expected |
| Runtime dependency closure for `typst-wasm` | 115 packages; passed |
| Workspace formatting | Passed |

The existing `typst-package` CI job builds the production package and runs package smoke. The
package builder calls the final-artifact validator, so the shared vectors now execute against the
packaged WASM through that existing gate. Native Rust tests alone are not the artifact witness.
The package smoke also retains its existing render, analysis, resource, ABI and export checks.
Clippy passed before the implementation commit; pre-existing warnings remain. No full workspace,
complete browser suite, other-host build, or pixel-equivalence claim is made by this record.

Each successful vector checks the closed transport envelope. Materializations decode through
`MaterializedThemeWireV1` and compare contract-owned canonical spec bytes, retaining the four
version checks and rejection of unknown fields. This correctly treats serialized `17.0` and
canonical `17` as the same contract value without weakening presence or structure checks.

Authoring failures pin numeric status independently: invalid definitions return 1 and the encoded
byte limit returns 10; all three currently use error category `generic`. The verifier checks the
complete diagnostic and optional resource details after requiring and removing only nonempty
human-readable messages. Mutation tests reject wrong nonzero status, malformed status, wrong
category/code/path/resource details, changed versions, extra fields and changed spec values.

# Artifact identity and remaining size gate

The clean packaged WASM is 11,564,035 bytes with SHA-256
`7d6d1b49615f862ee2b1e8d67ec4f77cfefcac6ab79ce263602421d47ba814c9`.
The artifact manifest records source-input fingerprint
`a0e98f8d2cacd2f174499058a3773e261f74c50f2dcc3f278c577b08115efa54`
(1,108 files across 15 workspace packages) and tool fingerprint
`88df78cfc6abcfb1f4c895daf872c437f467b1a0caeddab6fde6bdba6a96c8dc`.
Cargo.lock SHA-256 is `66d9946dce474e1945b52bbc090ec69d79cfed680a098b4cfe4d6422ec4c0775`.

The earlier main-worktree package had 11,564,274 bytes and SHA-256
`34971855cbfa187f5a75d88b0e0ce12fc9bfb0b4e33cb06c593cedc48cac5474`.
Both manifests have identical input and tool records, but their artifact bytes differ. This record
therefore identifies and measures the clean package separately; it does not claim byte-for-byte
reproducibility across checkout paths. The cause of this difference has not been isolated.

All four existing Typst size budgets still fail. No profile capability or budget was changed.

| Metric | Clean-source bytes | Existing budget |
| --- | ---: | ---: |
| Raw linked WASM | 18,681,418 | 15,000,000 |
| Optimized/stripped WASM | 11,564,035 | 10,200,000 |
| gzip of optimized/stripped WASM | 4,433,946 | 3,925,000 |
| Brotli of optimized/stripped WASM | 3,275,738 | 2,750,000 |

The size command reused the clean build's raw WASM through the shared `target/wasm-build`
cache and applied the same production optimizer. Its optimized byte count matches the packaged
payload above. The raw WASM SHA-256 is
`096f653a34ed01c844d860181066820aa8df607dc0a8c94c0888f371bec7cf15`.

These are final linked/optimized/compressed measurements under the checked-in size recipe. They
do not demonstrate a latency or memory improvement. This verifier/test slice does not explain the
pre-existing size excess.

# Reproduction and limits

Run from the exact source checkout with `CARGO_BUILD_JOBS=2` and `BINARYEN_CORES=2`:

```console
cargo nextest run --locked --release -p xtask -p merman-typst-plugin --features merman-typst-plugin/analysis,merman-typst-plugin/layout-cytoscape,merman-typst-plugin/layout-elk,merman-typst-plugin/svg -E 'test(typst_plugin_smoke) | package(merman-typst-plugin)'
cargo nextest run --locked --release -p merman-typst-plugin --no-default-features
cargo run --locked --release -p xtask -- typst-package-smoke --profile publish --out /tmp/typst-verified-package --keep-artifacts --typst /absolute/path/to/typst-0.15.0
python3 scripts/verify_artifact_dependency_closures.py --profile typst-wasm
cargo fmt --all -- --check
cargo run --locked --release -p xtask -- wasm-size-matrix --surface typst --budget-file /absolute/path/to/docs/release/WASM_SIZE_BUDGETS.json
```

Use an xtask built from the selected checkout. The size command exits 1 for the four budget failures.
Local logs are `/tmp/theme-typst-clean-{tests,minimal,package,closure,fmt,size}.log`.
The retained package, tool identity and serial verification state are located through
`/tmp/theme-typst-r86-path.txt`; the clean package is under `clean-package/merman/0.3.0` there.
The artifact owner's schema-3 manifest records input/tool fingerprints and the optimized payload.

This establishes one final Typst artifact's authoring/discovery behavior. Public catalog cells
remain empty; it neither issues preset qualification nor closes C7a. Web/Typst size budgets,
remaining host/profile qualification, publication and formal candidate/rollout/freeze stay open.
C7b still includes Class's four Text routes and Block's 32 legacy routes.
