# WASM host-measurement disposition decision — 2026-08-08

## Decision

Status: **accepted-structural**. No release latency claim is admitted.

Merman now reads the optional JavaScript `handled` disposition directly before deserializing a
handled host text-measurement result exactly once. The change preserves nullish fallback,
`handled: false` early return, payload validation, callback cardinality, result-field access order,
and SVG/layout output. It adds no cache, retained state, protocol field, custom Serde visitor, or
field-order table.

This is not a restoration of the rejected 2026-08-04 implementation. The earlier candidate used a
large custom visitor to combine disposition and payload parsing and remains rejected in
[its historical receipt](wasm_host_measurement_decode_2026-08-04.md). The accepted candidate uses a
small owner-local boundary: one direct property read followed by the existing complete result
deserialization.

## Revisions and claim boundary

| Role | Commit | Meaning |
|---|---|---|
| `A` | `b60b4b1202fd2403d06b7c2f65d3953a1ba3e06f` | Clean control with disposition-only Serde struct decoding. |
| `B` | `8d626b6578ba25c53407f2e377c175ff9e3ff3e3` | Direct `handled` property read and one complete result deserialization. |
| Prior rejected candidate | `fb73f563e44e745cdcde73c7ab981c7eb8918893` | Custom visitor; not restored. |
| Prior cleanup | `2d9a0473f603063f5b5cc2c843513c262e03b666` | Removed the custom visitor after its registered latency gate failed. |

The primary claim is structural repeated-work removal. For `H` handled callback results, `A`
performs `H` disposition struct deserializations plus `H` complete result deserializations. `B`
performs `H` direct property reads plus `H` complete result deserializations. The candidate does
not claim a different callback count, payload schema, asymptotic renderer bound, or general browser
throughput result.

## Why the candidate is worth retaining

The fixed 50-microsecond comparator default is not used as a universal admission cutoff. This
candidate is a small, semantics-preserving, owner-local removal of repeated transport work on a
documented high-callback path:

- production Rust changed by 13 insertions and 9 deletions;
- the rejected custom visitor and its protocol machinery remain absent;
- no persistent state, cache, allocation sidecar, or compatibility path was added;
- the optimized `web-render` WASM artifact became 388 bytes smaller; and
- the current browser A/B showed no regression and moved consistently in the expected direction.

That evidence supports `accepted-structural`. The browser timing remains supporting context rather
than an `accepted-latency` claim because independent A/A noise exceeded the timing-grade cutoff.

## Semantic and transport evidence

The real WASM smoke locks the observable boundary:

- `null` and `undefined` select fallback without result decoding;
- `handled: false` reads `handled` and no measurement payload getter;
- explicit `handled: true` and omitted `handled` preserve measured SVG and layout output;
- handled results read `handled` once and each complete payload field once in the existing order;
- malformed handled values, malformed payloads, non-finite and negative values, zero line counts,
  WASM32 line-count overflow, and thrown callbacks preserve fallback behavior; and
- callback count, request/result shape, package capabilities, SVG output, and DOM-safety admission
  remain unchanged.

Verification completed on 2026-08-08:

- `CARGO_BUILD_JOBS=1 cargo nextest run --locked -p merman-wasm --all-features`: 33 passed;
- `CARGO_BUILD_JOBS=1 cargo clippy --locked -p merman-wasm --target wasm32-unknown-unknown --no-default-features --features svg -- -D warnings`: passed;
- all five Web WASM packages rebuilt successfully;
- the full five-package Web smoke and DOM-safety smoke passed;
- `cargo fmt --all -- --check` and `git diff --check` passed.

## Artifact evidence

Both artifacts were built serially from clean detached worktrees with the same Rust 1.95.0
`web-render` / `wasm-size` recipe and `wasm-opt` toolchain.

| Artifact | Bytes | SHA-256 |
|---|---:|---|
| `A` | 12,255,769 | `f1d2fd11f093071f3a301c9ed67ca3607d77cc47dcbe3315f6176b43995b7f7b` |
| `B` | 12,255,381 | `3a1b1bbcf4ef92bdce0dfa09228ab6d06afc30131228c6ed8d9f00f3ef9ee18d` |
| Delta | -388 (-0.00317%) | — |

The first shared-target head build incorrectly reused the control crate fingerprint and produced an
identical artifact. That result was discarded. A package-scoped `merman-wasm` clean forced a real B
rebuild before the recorded size and hash comparison.

## Browser support lane

The current support lane ran in Microsoft Edge 151.0.4129.72 with eight A/A pairs on each revision
and eight alternating AB/BA pairs. Each sample rendered the same 200-token Flowchart 16 times after
20 warmups per side.

The current branch produced 468 callbacks and 33,567 returned-result bytes per render. Both sides
matched exactly at 30,932 SVG bytes and FNV-1a digest `5934a4f4`. The historical 2026-08-04 fixture
reported 467 callbacks and 33,492 bytes; that exact identity did not reproduce on the current branch,
so the new lane is explicitly current-fixture support rather than a replay of the old contract.

| Metric | Result |
|---|---:|
| A median | 2,600.000 us |
| B median | 2,371.875 us |
| Paired median movement | -5.303%, -137.500 us |
| 95% paired-bootstrap upper bound | -2.882%, -68.750 us |
| Direction | All 8 pairs favored B |
| Maximum robust A/A noise | 4.415% |

Because the A/A noise exceeded 3%, this lane is not timing-decision-grade and does not create a
release latency claim. It is sufficient as a supportive public non-regression observation because
the primary structural invariant is exact, semantic evidence is complete, the implementation is
smaller than both the control and the rejected visitor, and no pair indicated a regression.

## Raw evidence and invalid attempts

Raw evidence is intentionally ignored under
`target/bench/experiments/u3-wasm-host-disposition-reflect-v2/`:

| Artifact | Bytes | SHA-256 |
|---|---:|---|
| `experiment.yaml` | 7,517 | `504542d7d5330e1251eb08245a5601725f206a422eb80ca5fb93602cfffb4487` |
| `browser-ab.json` | 11,047 | `00b727b6b24468a290796087efaa0d4406e9b3956eb7393a4046a0f6e8486797` |
| `browser-ab-analysis.json` | 1,728 | `eaf694dba0db0c6f60511c795203da584b3e3a4d661940f85559bf9492b9a11e` |

Two temporary fixture attempts were rejected before interpretation: one produced 484 callbacks and
27,428 bytes without explicit `handled: true`; another produced 468 callbacks and 33,568 bytes.
Neither contributes to the recorded timing result.

## Public behavior and migration

No public migration is required. The browser host-measurement protocol version, callback shape,
fallback semantics, error class, result fields, and first-party Web APIs remain unchanged. This
decision changes only the internal WebAssembly transport work performed for each non-nullish
callback result.
