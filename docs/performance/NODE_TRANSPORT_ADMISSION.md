# Node/SSG Transport Admission Decision

Status: N-API selected for the experimental public alpha package group. Stable-product admission
remains separate from this alpha release boundary.

This is an internal U14 comparison record. The alpha package release uses the selected N-API
transport only after `release-node.yml` has built, packed, installed, and rendered the root loader
on every declared platform target. The Node-targeted WASM implementation remains comparison-only.

The measurements below predate the current size-first candidate recipe, which omits the
unadvertised math capability. They remain valid only for their recorded source revision and must
not be used as current artifact-size or transport-admission evidence. The next comparison must
rebuild both transports from the current SVG-plus-layout recipe.

## Current size-only recipe decision

A matched same-source Node-WASM experiment compared the old static-SVG recipe with the same
transport and layout backends after removing only `math`:

| Recipe | Raw WASM | Gzip | Resolved normal packages |
| --- | ---: | ---: | ---: |
| SVG + Cytoscape + ELK + math | 17,764,180 B | 6,446,727 B | 186 |
| SVG + Cytoscape + ELK | 14,699,455 B | 5,326,637 B | 127 |

The current recipe is 17.25% smaller raw, 17.37% smaller under gzip, and removes 59 resolved
packages while retaining the same callable operation set. Rust transport tests and all Node
package-contract tests pass. This defines the smaller alpha package recipe; the historical report
below does not by itself prove a stable-product admission.

## Decision

The Node-targeted WASM and napi-rs candidates both built, packed, installed, and ran on the local
macOS arm64 host. The napi-rs candidate is faster for warm SVG, substantially faster for cold
startup, and uses less peak memory on this host. Node-WASM remains smaller. N-API is selected for
the public alpha package; stable-product admission still requires current-source, all-target
release evidence. Separately, 426 successful SVG outcomes retain same-host cross-transport exact
geometry and raw-byte drift whose cause is not attributed.

Report schema 2 closes three evidence gaps from the earlier local run:

- the validator independently reloads and hashes the trusted 4,001-case corpus instead of trusting
  a report-provided digest or path list;
- cold workers receive only the declared cold SVG workload, and timing stops before SVG evidence
  projection; and
- cold and concurrent representatives retain or recompute raw SVG so byte, structure, and geometry
  hashes are bound to the actual timed output.

The two candidates match all 4,001 semantic or typed-error outcomes and every SVG structure
signature. Of those cases, 3,897 produce SVG and 104 produce matching typed failures. Exact
geometry and raw bytes differ for 426 successful SVGs. Those differences remain visible evidence;
they are not mislabeled as semantic failures or hidden behind a tolerance. The schema-2 validator
does not promote that report-only residual into a semantic/structure failure or an admission gate.

## Provenance

| Field | Value |
| --- | --- |
| Measured at | `2026-07-27T17:52:35.583Z` (UTC; `2026-07-28T01:52:35.583+08:00` in Asia/Shanghai) |
| Measured source and harness | `5f540c08db635d4f0ccc8e62429c0e385e95a485` |
| Harness digest | `sha256:f37425ba4ca0fa89970734162a8ff62c68a7dd8a7a13f72c598c533f8c7d77b0` |
| Raw report | `platforms/node/reports/node-transport-comparison-2026-07-28-5f540c08d-v2.json` (ignored local evidence) |
| Raw report SHA-256 | `sha256:a5ef6ad899033010209de2ae9cb25ffadad13c3f57fc56cbd351f777e8119226` |
| Input digest | `sha256:e81eb92204354229da1b9735f8225b96062fe8c6fd3429df4286f45005f09626` |
| Corpus digest | `sha256:6fedc4c40fad3f332a325d902441e0f41cc74236ce91a709a1a0f226b32fee3a` |
| Host | macOS `25.5.0`, arm64, Apple M4 Pro, 14 logical CPUs, 48 GiB memory |
| Toolchain | Node `v26.5.0`; Rust/Cargo `1.95.0`; napi `3.11.0`; napi-derive `3.6.0`; napi-build `2.3.2`; `@napi-rs/cli` `3.7.4` |

Both build receipts bind the same measured commit, source digest
`sha256:57fa114b4915d7c9496f5f468cf7581993efaeec5ca8e94b8bea9336907ca0e9`,
and Cargo lock digest
`sha256:379e41cda833f4a71799cfa728eaa3eeac7c98bbab109dc17d830ec9916b0bb7`.
The values below belong to that exact source and must not be carried forward without another
source-bound build and run.

The raw 21.8 MB sample report is intentionally ignored and is not part of this commit. This
checked-in record retains its content hash, trusted-input digests, aggregate values, and validation
result. Reproducing or auditing individual samples requires regenerating the raw report with the
command below.

The clean standalone candidate build caught two integration omissions in the runtime-capability
catalog changes before measurement: the native adapter set lacked its owner-defined public policy
contract, and the checked binding transport projection was not re-exported from the binding facade.
Commits `ee1304626` and `5f540c08d` fix those boundaries. This is why the locked standalone Node
workspace build remains part of the evidence protocol rather than relying only on a root-workspace
check.

## Candidate Evidence

Both candidates used deterministic bindings options, `trusted-native` resources, static SVG, one
warm-up pass, three measured warm passes, ten isolated cold processes, and five four-request
concurrency batches. Warm SVG figures below include only the 11,691 successful SVG samples, never
typed failures.

The warm timer includes the public facade call plus SHA-256 and byte-length evidence projection.
Cold and concurrent timers stop before evidence projection. The warm rows are therefore
harness-level operation latency, not isolated renderer CPU time.

| Measure | Node-targeted WASM | napi-rs |
| --- | ---: | ---: |
| Runtime artifact | 16,881,944 B WASM, plus 8,026 B JS and 44 B manifest | 21,223,312 B `.node` |
| Corpus results | 3,897 successful SVGs, 104 typed failures | same |
| Semantic/error and SVG-structure matches | 4,001 / 4,001 | 4,001 / 4,001 |
| Exact geometry / raw-byte differences | 426 / 426 | same paths |
| Warm successful-SVG p50 / mean | 0.3189 ms / 0.9266 ms | 0.2903 ms / 0.8117 ms |
| Warm successful-SVG p95 | 1.6305 ms | 1.3467 ms |
| Cold parent-to-result p50 | 137.74 ms | 47.39 ms |
| Engine-init-through-first-SVG p50 | 96.05 ms | 7.39 ms |
| Four-request concurrent batch p50 | 1.4387 ms | 0.2418 ms |
| Peak RSS | 638,189,568 B | 240,648,192 B |
| Packed / installed footprint | 6,157,111 B / 17,784,897 B | 8,966,029 B / 22,906,407 B |
| Package count | 1 | 2 |
| Installed runtime probe | passed | passed |
| Queue saturation, dispose, non-preemptive abort | passed | passed |
| Local runtime/install smoke | `darwin-arm64` single-package passed | `darwin-arm64` root optional dependency passed |

On this workload, napi-rs lowers warm p50 by 9.0%, warm p95 by 17.4%, and warm mean by 12.4%.
Its cold parent boundary is 2.91x faster, its engine-init-through-first-SVG boundary is 12.99x
faster, and its recorded peak RSS is 62.3% lower. The five concurrency batches point in the same
direction, but that sample count is evidence for follow-up rather than an admission-grade
throughput claim. The cost is a 45.6% larger packed footprint and 28.8% larger installed footprint.

This historical run establishes a local N-API latency and RSS advantage, but it does not by itself
prove stable-product admission. N-API is now selected for the experimental public alpha, whose
release workflow separately requires exact optional-platform-package installation and render
smoke evidence across the complete declared target matrix. The geometry drift remains an
unattributed report residual.

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

The raw report must validate through `scripts/benchmark/report-contract.mjs` against the checked-in
trusted corpus. A future run may reconsider admission only with exact
semantic/typed-error/SVG-structure parity and passing runtime evidence for every declared target;
the N-API candidate must also pass exact optional-platform-package installation checks. Geometry
drift must still be reported exactly, even though the current validator does not make it an
admission gate; it is never hidden behind a widened tolerance.
