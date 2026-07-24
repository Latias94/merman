# Node/SSG transport comparison template

This is the reusable comparison-report schema. The current U14 measurement is recorded in
[`NODE_TRANSPORT_ADMISSION.md`](NODE_TRANSPORT_ADMISSION.md); it rejected both candidates and names
no winner.

The executable owner is `platforms/node/scripts/benchmark/run.mjs`. A completed JSON report must
pass `platforms/node/scripts/benchmark/report-contract.mjs` and record all of the following:

- full Git commit, machine OS/release/architecture/CPU/memory, Node, Rust, Cargo, napi,
  napi-derive, napi-build, and `@napi-rs/cli` versions;
- the shared corpus, deterministic bindings options, explicit resource profile, format options,
  case count, and SHA-256 input digest;
- Node-targeted WASM and napi-rs build-receipt digests;
- one fresh process per cold sample, repeated warm latency samples and distributions, RSS method,
  packed/unpacked/installed footprint, concurrent batches, queue saturation, disposal, and
  non-preemptive AbortSignal behavior;
- per-target runtime and installation results. Node-API compatibility alone is not target evidence.

The first target matrix is macOS arm64/x64, Linux x64 glibc/musl, and Windows x64. A target becomes
supported only after CI executes its native package on that exact OS/CPU/libc target. A complete
semantic corpus pass with missing target evidence yields `inconclusive`; any semantic corpus
mismatch yields `rejected`. Neither outcome selects a transport.
