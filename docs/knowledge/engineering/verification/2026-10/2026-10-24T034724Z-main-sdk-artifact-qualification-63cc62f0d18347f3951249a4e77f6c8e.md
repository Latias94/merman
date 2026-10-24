---
type: "Verification Evidence"
title: "Main SDK artifact qualification"
description: "Verification Evidence for Main SDK artifact qualification."
timestamp: 2026-10-24T03:47:24Z
record_id: "63cc62f0d18347f3951249a4e77f6c8e"
producer_id: "codex-sdk"
run_id: "headless-sdk-20261024"
---

# Verification

The main implementation source is `63c219156571d40e4304d9fcafa81f6af3a69c84`
on `feat/headless-sdk-20261024`, based on `83e3ea74b`. DrawingList is excluded.
The remaining uncommitted changes at qualification time were font-fixture documentation
and engineering-memory records, not production sources, headers, recipes or legal inputs.

All Cargo work used two build jobs and ran serially. Native artifacts used the exact
descriptor-owned `native-sdk` profile and explicit `aarch64-apple-darwin` target;
WASI used `wasm-size` and `wasm32-wasip1`.

# Result

- All six local archives passed `sdk_archive.py package` and `verify`, using an actual
  runtime catalog and exact profile/target cargo-about report.
- Standard, math and full ran in wazero 1.12.0 and wasmtime 50.0.1 without preopened paths.
- Native C and both WASI runtimes produced identical decoded RGBA8 for the shared basic
  PNG; math and full also matched formula PNG pixels. Metadata and viewport checks passed.
- Standard returned structured absent-math errors; standard/math retained byte-equivalent
  explicit-Dagre fallback for absent ELK. Full admitted and planned ELK.
- Every macOS archive was extracted into a temporary prefix. Included CMake consumers
  linked and ran against both its dynamic and static library. `otool -L` retained
  `@rpath/libmerman_ffi.dylib`; `DYLD_PRINT_LIBRARIES` confirmed the extracted SDK loaded,
  rather than a producer build-tree library. Each archive's two CTest consumers passed.

# Evidence

The qualification loop ran the existing owner commands in this order for each flavor:

1. `scripts/artifact_profile_recipe.py PROFILE --build --locked --target-triple TARGET`.
2. Stage that library/module set immediately; normalize and ad-hoc sign the macOS dylib.
3. Run `platforms/wasi/smoke.py` and `platforms/wasi/native_smoke.py --wasi-dir ...`.
4. Run `scripts/sdk_archive.py catalog-sidecar`, generate the exact Rust license report,
   then `package` and `verify` before another feature build.
5. Extract each native archive, configure/build its included CMake consumer, inspect
   actual dylib loading, and run CTest for shared/static linking.

Local archives and checksum sidecars are under `target/sdk-output`; the six staged inputs,
raw catalogs, metadata, outputs and runtime evidence are under `target/sdk-input`.
The local command log is `/tmp/merman-sdk-main-artifacts-final.log` and ends with
`ALL_LOCAL_SDKS_PASS`. These ignored machine-local files are evidence, not published assets.

Other completed main gates: 46 Swimlane/fallback tests, 24 export-font tests, 362 bindings-core
tests, 74 FFI tests, 18 WASI native unit tests and 55 xtask contract tests; the exact complete
SVG structure gate; representative dependency closures; the 45-case representative feature
matrix and isolated consumers; generated legal chain; ABI/binding generation; Flutter
ffigen; and workflow analyzers. Final Python contract rerun: 212 tests passed. Each host
directory's Go tests and 14 Python tests passed. `cargo fmt --all -- --check` passed.

# Follow-up

This is macOS ARM64 and WASI evidence only. The 18-case SDK/preflight workflow owns the
other native target hosts and per-target legal reports. No GitHub workflow was dispatched,
no tag/version was created, no assets were published, and no remote branch was pushed.
The workspace version remains 0.9.2; local preview archives must not replace existing
v0.9.2 assets. Maintenance adaptation and its own qualification are separate evidence.

# Citations

- Plan: `docs/plans/2026-10-24-0925-feat-headless-sdk-plan.md` (U1-U5, U7; AE1-AE4).
- Owners: `.github/workflows/release-sdk.yml`, `scripts/sdk_archive.py`,
  `platforms/wasi/README.md`, `distribution/sdk/README.md`.
- Previous checkpoint: record `7ce5367f00bf44b6afc88958c6fbd514`.
