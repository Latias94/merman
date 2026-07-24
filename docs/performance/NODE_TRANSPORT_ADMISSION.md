# Node/SSG Transport Admission Decision

Status: inconclusive. Neither private Node candidate is an admitted or supported product.

This is an internal U14 evidence record. It does not add `@mermanjs/node` to a release surface,
capability descriptor, package manifest, user guide, or changelog.

## Decision

The Node-targeted WASM and napi-rs candidates both built, packed, and ran on the local macOS arm64
host. The current installation probe installs only the root package and resolves the matching
native package through `optionalDependencies`, using a local tarball override in place of an
unpublished registry version. That proof currently covers only macOS arm64, and the candidate
artifacts predate the final refactor state. The selected transport is therefore `null` and both
candidates remain private evaluation code.

The first local report used a decimal-rounding SVG hash and labelled 13 outcomes as semantic SVG
differences. That classification is retired: decimal rounding is not an absolute-error comparator,
and manual decomposition showed that the differences begin in target-specific layout geometry
rather than the parsed semantic model. The harness now records four independent facts: exact
semantic JSON, SVG DOM structure with geometry values removed but geometry shape retained, exact
geometry, and raw SVG bytes. Semantic JSON, typed errors, and SVG structure are transport contract
evidence. Exact geometry and raw bytes remain visible reproducibility evidence, but do not by
themselves reject an otherwise correct Node binding.

## Provenance

| Field | Value |
| --- | --- |
| Measured at | `2026-07-23T16:15:48.355Z` (`Asia/Shanghai`) |
| Git commit | `cc39fdcd0f0ea6242ddb68c3093859c456c38844` |
| Harness digest | `sha256:c336e5ddfd17ee4ceb679bfff27165a2a6d5bbedf6feb14c0663f63c575ca97d` |
| Raw report | `platforms/node/reports/node-transport-comparison-local.json` (ignored local evidence) |
| Raw report SHA-256 | `sha256:3a3ce73bcdc72f4c91cf86c8e3c9cbdc05586bd3e325fca5c0b4ef68ba2cd1b1` |
| Corpus input digest | `sha256:97df8f5661c3497acc99b771788ed178e7914731e1474d5de5152da20d2ff293` |
| Host | macOS `25.5.0`, arm64, Apple M4 Pro, 14 logical CPUs, 48 GiB memory |
| Toolchain | Node `v26.5.0`; Rust/Cargo `1.95.0`; napi `3.11.0`; napi-derive `3.6.0`; napi-build `2.3.2`; `@napi-rs/cli` `3.7.4` |

The Git commit identifies the base revision. Candidate source, build configuration, and harness
inputs are additionally bound by the recorded source, input, binding-contract, artifact, and
harness digests because the measurement used a working tree.

## Candidate Evidence

Both candidates used deterministic bindings options, `trusted-native` resources, static SVG, one
warm-up pass, three measured warm passes, ten isolated cold processes, and five concurrency batches.

| Measure | Node-targeted WASM | napi-rs |
| --- | ---: | ---: |
| Shared binding-contract digest | `sha256:6fd3db7951752ee1727c8609db6869ec4d22357a6002839d9ad7f47e57f420ed` | same |
| Corpus results | 3,892 successful, 103 typed failures | 3,892 successful, 103 typed failures |
| Retired rounded-SVG match / mismatch | 3,982 / 13 | 3,982 / 13 |
| Raw SVG byte differences | 413 | 413 |
| Cold p50 / mean | 80.10 ms / 105.23 ms | 47.07 ms / 49.65 ms |
| Warm p50 / mean | 0.781 ms / 1.483 ms | 0.722 ms / 1.354 ms |
| Warm p95 | 3.071 ms | 2.750 ms |
| Peak RSS | 655,228,928 B | 238,977,024 B |
| Packed / installed footprint | 6,032,458 B / 17,407,706 B | 8,860,772 B / 22,718,975 B |
| Package count | 1 | 2 |
| Installed runtime probe | passed | passed |
| Queue saturation, dispose, non-preemptive abort | passed | passed |
| Local runtime/install smoke | `darwin-arm64` single-package passed | `darwin-arm64` root optional dependency passed |

The former mismatch set consists of Mindmap and Venn geometry differences, plus one decimal-rounding
boundary false positive. The exact paths and per-case outcomes remain in the superseded raw report.
Both candidates also preserve the typed `unknown-operation` and missing `png` capability errors; their
103 corpus failures have the same 98 parse-error and 5 render-error classifications.

No performance winner is claimed. The values are a same-host local measurement, and neither
candidate has the complete target installation evidence required for admission.

## Reproduction

Run the private harness from a checkout with the pinned Node dependencies installed. Build the
candidate artifacts serially, then produce and validate a new raw report:

```bash
cd platforms/node
npm ci
CARGO_BUILD_JOBS=1 npm run build:candidate -- --candidate napi --target darwin-arm64
CARGO_BUILD_JOBS=1 npm run build:candidate -- --candidate node-wasm
npm run benchmark -- --native artifacts/napi/darwin-arm64/merman.node \
  --wasm artifacts/node-wasm/merman_node.js \
  --output reports/node-transport-comparison-local.json
npm test
npm run check:packages
```

The raw report must validate through `scripts/benchmark/report-contract.mjs`. A future run may
reconsider admission only with exact semantic/typed-error/SVG-structure parity and passing runtime
and optional-dependency installation evidence for macOS arm64/x64, Linux x64 glibc/musl, and
Windows x64 MSVC. Geometry drift must still be reported exactly; it is never hidden behind a
widened tolerance.
