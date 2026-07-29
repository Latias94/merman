# Binding Request Overlay U6 Candidate Receipt

Date: 2026-07-29

Status: accepted. Reusable binding operations retain the typed request-overlay implementation.

## Compared identities

- Adjacent base commit: `35133aa2dbc92944d6b664b229f017369ece3875`.
- Measured candidate commit: `0be1a409286f044f40954aec686bd316ff78cb16`.
- Oracle cleanup commit: `83d5a596faf7ca563e4c2de7aabdcda3d6976ee8`.
- Accepted candidate commit: `1b89ec7e59b442b01a411cb04654587f6aefc097`.
- Host: macOS arm64, Apple M4 Pro, Node `v26.5.0`.
- Public surface: installed, revision-owned `createNodeEngine().executeOperation()`.
- Workload: warmed `semantic-json` over `info\nshowInfo\n` with `{"version":1}`.

The first cleanup commit removes only the `#[cfg(test)]` byte-merge oracle and moves the pre-existing
semantic assertions onto the typed path. It also explicitly enables `serde_json/preserve_order`,
which was already enabled transitively by `merman-core`; this fixes ownership of the wire-order
contract without changing the measured release feature closure. There are no net new Rust tests
relative to the adjacent base.

The final follow-up compiles the effective runtime policy once instead of once per operation family,
removes a single-field analysis config wrapper, and clones the engine before replacing host
measurement state. These changes do not execute inside the warmed version-only timing boundary:
that path borrows the already-constructed base engines. A post-change owner-local diagnostic found
no version-only change and improved the real resource-overlay median by 5.78%, to 9.18 us.

## Low-latency gate

The ordinary `>10%` and `>50 us` gate is impossible for an operation whose entire baseline is
below 50 us. Contracts `binding-request-overlay-low-latency-v1` and `v2` therefore preregistered
the noise-adaptive low-latency gate before confirmation. The first NAPI calibration was retained as
inconclusive because its order-effect interval excluded zero and its absolute noise endpoint was
1,005.7 ns. Version 2 added 4,096 untimed settling calls before every observation; no result from
the failed calibration entered confirmation.

| Installed row | Base mean | Head mean | Mean change | Relative log threshold | Absolute threshold | Fresh simultaneous upper bounds | Result |
|---|---:|---:|---:|---:|---:|---:|---|
| NAPI | 23,561.51 ns | 12,437.60 ns | -47.21% | 0.13736 | 3,326.90 ns | log -0.62478; absolute -10,707.74 ns | Confirmed improvement |
| Node-WASM | 16,255.72 ns | 3,469.93 ns | -78.65% | 0.11040 | 1,753.03 ns | log -1.52870; absolute -12,675.72 ns | Confirmed improvement |

Both rows required and used eight fresh balanced AB/BA pairs. NAPI A/A noise was 0.04579 log and
563.03 ns against limits of 0.05129 and 1,000 ns. Node-WASM A/A noise was 0.03680 log and
584.34 ns against limits of 0.05129 and 805.82 ns. Identity and order intervals included zero for
both base and candidate replicas.

## Controls and semantics

All public result digests matched across base/head and NAPI/WASM. Settled control medians were:

| Control | NAPI change | Node-WASM change |
|---|---:|---:|
| Empty request options | -0.58% | -2.67% |
| Real resource-tightening overlay | -20.02% | -35.81% |
| New engine plus first request | -19.12% | -19.83% |
| SVG version-only request | -7.96% | -11.70% |

The initial 64-call Node-WASM SVG control showed a +40.31 us transient while the base JIT curve
was still falling. It did not cross the ordinary absolute regression threshold, and it was not used
for admission. The frozen 4,096-call warmup plus 1,024-call settling rerun reversed the signal to
-11.70%; seven of eight pairs improved and every SVG digest matched. Build receipts independently
validated resource tightening, unknown-operation, missing-capability, and successful SVG behavior.

## Memory and artifacts

The clean native candidate-admission matrix used five fresh matched processes at each of
`1/2/4/10/32/100x`. Its 100-call results were:

| Metric | Base | Head | Change |
|---|---:|---:|---:|
| Allocation count | 608,600 | 2,823 | -99.54% |
| Allocated bytes | 78,006,700 | 234,481 | -99.70% |
| Peak live growth | 424,592 B | 1,071 B | -99.75% |

The base and head probe executables had distinct SHA-256 values
`3ed9bf254913f8b1d49699cfb7d927f2d61f59d32e4554bb941218c6e68f99c1` and
`e3321067b2b3e82aae6ad8703d94eb51b0ec3af5ebc10a7493f25ec95fab70f1`.
An earlier head run that resolved to the base executable hash through the shared Cargo target was
rejected before interpretation and rerun after forcing the revision-owned probe to rebuild.

The Node RSS envelope used five fresh processes for every transport, revision, and scale: 120
processes total. The maximum startup historical-RSS gap was zero, all output digests matched, and
the conservative maximum observed paired head-minus-base regression was 655,360 bytes, below the
1 MiB budget.

| Artifact | Base bytes | Head bytes | Delta |
|---|---:|---:|---:|
| NAPI `merman.node` | 21,488,160 | 21,488,208 | +48 |
| Node-WASM module | 17,062,044 | 17,057,627 | -4,417 |

## Complexity and decision

The candidate adds no public API, runtime dependency, unsafe code, global or unbounded cache,
background task, or second executor. State is bounded per engine. The accepted path parses the
request document once, preserves ordered wire merge semantics, validates every operation domain in
the established order, and materializes only the selected operation family. The production
serialize/reparse and whole-engine reconstruction path is gone, as is the temporary differential
oracle. Relative to the adjacent base, the final implementation is `+838/-475` across the seven
owned binding files, including the migrated pre-existing tests.

Post-cleanup verification:

```text
CARGO_BUILD_JOBS=1 cargo +1.95.0 check -p merman-bindings-core --no-default-features
CARGO_BUILD_JOBS=1 cargo +1.95.0 check -p merman-bindings-core --all-features
CARGO_BUILD_JOBS=1 cargo +1.95.0 nextest run -p merman-bindings-core --all-features
156 passed; 0 failed
```

Raw evidence remains under `target/bench/u6-node-public/`. Key report SHA-256 values are:

- NAPI calibration/confirmation: `29e996ad55daaab63322e28146f93aec55f4c002038184bdc4c4d5c52c80abd7` /
  `a6d25f871680be234b4340058bb1608131f7dfb8345bfdb40bf8a46763a5262c`.
- Node-WASM calibration/confirmation: `6f697ab80d0fa26955d2819ed5c38c8085f0f0a64043187d326404d0a7ceb148` /
  `240c9b261595fd2095fc251dff3774becf92d7d0469afa368b60a42f6e047ce4`.
- Native base/head memory: `f335a75d1722f95e4e15f7a0b318747551b072f29ece573306ab81529782c374` /
  `70d61dc3ccf8b9e576240d049abe2afa0844b041a934af4b3c61125326e6d395`.
- Node RSS envelope: `af8ee00476c074feb311a27096fa0a8125131c3e38257ff6cee2f955b3f5335f`.

The CPU, native allocation, Node RSS, semantics, controls, artifact-size, and complexity gates all
pass. U6 is accepted under the preregistered low-latency policy; the fixed 50 us ordinary gate was
not waived or reinterpreted.
