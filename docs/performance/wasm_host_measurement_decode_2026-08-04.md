# WASM host-measurement single-decode decision — 2026-08-04

## Decision

Status: **rejected**.

For the benchmark's `handled: true` host text measurement results, the candidate replaced two
top-level `serde_wasm_bindgen::from_value` calls with one custom struct deserialization. Null and
`undefined` still bypassed deserialization, `{ handled: false }` returned before reading payload
getters, and semantic validation remained owned by `merman-bindings-core`.

The implementation passed its semantic and control gates but did not clear the preregistered
latency gate, even on a public Flowchart input deliberately chosen to amplify callback transport.
The production implementation and candidate-only tests were therefore removed in cleanup commit
`2d9a0473f`. This receipt makes no accepted-latency or accepted-artifact claim.

## Revision and artifact boundary

| Role | Commit | Optimized `web-render` WASM | Artifact SHA-256 | Git tree |
|---|---|---:|---|---|
| baseline | `685dd3ce1f6d37c85e24a572063ccae7b2926633` | 12,026,579 bytes | `12769ce53cc24ea1816b1a404c391b591dd4a07a9229687a906bf9c65a35f91d` | `68bda9bebd1f0f207e57ea2dd60a95fcb363f919` |
| candidate | `fb73f563e44e745cdcde73c7ab981c7eb8918893` | 12,025,392 bytes | `b9461f6062881ae4ceef308619941e403bbfd10a2d34b227183a7719d7942756` | `b7c8cd14c592dc370db8598922154d585a27e1db` |
| cleanup | `2d9a0473f` | candidate removed | — | `68bda9bebd1f0f207e57ea2dd60a95fcb363f919` |

The candidate artifact was 1,187 bytes smaller (`-0.00987%`). Artifact size was a control rather
than a primary claim and does not override the failed latency decision.

## Registered protocol

- Browser: Microsoft Edge `151.0.4129.59`, reporting Headless Chromium `151.0.0.0`.
- Host: Apple Silicon macOS; Rust `1.95.0`; Node.js `26.5.0`; `wasm-pack 0.15.0`;
  `wasm-opt 131`.
- Profile: `web-render`, release `wasm-size`, build time excluded.
- Primary fixture: one valid Flowchart node with a 200-token label, producing 467 non-nullish
  `wrapped` callbacks and 33,492 callback returned-result JSON bytes per render.
- Ordinary control: `flowchart TD` with two nodes and one edge, producing four callbacks and 284
  callback returned-result JSON bytes per render.
- Primary schedule: 20 warmups per artifact, 16 renders per timed sample, eight A/A pairs, eight
  B/B pairs, and 12 alternating AB/BA pairs.
- Control schedule: 80 warmups per artifact, 96 renders per timed sample, eight A/A pairs, eight
  B/B pairs, and eight alternating AB/BA pairs.
- Acceptance gate: paired median improvement greater than both `10%` and `50 us`; A/A control
  noise at or below `3%`; exact output, callback, payload, and operation traces.

Raw pairs, artifact records, field-access traces, and bootstrap intervals remain in the ignored
experiment ledger at
`target/bench/experiments/headless-performance-hardening/u3/browser-host-measure-ab.json`.
That raw file is 326,033 bytes with SHA-256
`35a61fc28d1cfb4c2684bbb2fd8c8b3916fd5c8d396c3755180b38afc47b2569`.

## Results

| Lane | Paired median | Relative | Control noise | Result |
|---|---:|---:|---:|---|
| 467-callback primary | `43.75 us` faster | `2.012%` faster | `2.171%` | latency gate failed |
| four-callback control | `1.04 us` slower | `0.154%` slower | `1.241%` | within control budget |

The primary bootstrap interval for the paired median was `[0, 93.75] us`; the relative interval
was `[0%, 4.262%]`. Its upper relative bound remained far below the required `10%`, so this was a
disconfirming result rather than a second-run timing ambiguity.

Both artifacts produced identical SVG digests and sizes for both fixtures. The primary output was
30,436 bytes with SHA-256
`64eb220196651547d2e2677073b5274916c6619004decd14e5b78c5e0c796ac8`; the control output was
10,611 bytes with SHA-256
`096b2c59fca6b20747f58cff81f4750952df8b32f8b23f1909f68275454f5728`.
Callback counts, payload bytes, operation counts, and handled-result field access order were also
identical.

The adjacent source revisions establish the decode-count change: the baseline calls
`serde_wasm_bindgen::from_value` once for disposition and once for the complete result, while the
candidate calls it once for the complete result. Browser Proxy traces prove that the candidate
does not expand observable payload getter work, but those getter traces alone cannot distinguish
one struct deserialization from two. The decision therefore does not overstate the dynamic trace
as a direct internal decode counter.

## Semantic verification before cleanup

- Focused `merman-wasm` Nextest: one passed, eight skipped.
- Native `cargo check` and owner-local Clippy with `svg`: passed.
- Real WASM callback smoke: explicit `handled: true`, null, `undefined`, `handled: false`, Proxy
  field access, malformed types, non-finite and negative values, zero lines, WASM32 `2^32` line
  overflow, and thrown callback all preserved their corresponding measured or fallback result.
- Web script suite: 107 passed. Six unrelated legal-projection tests remained blocked by the
  pre-existing scoped Rust dependency-report input drift; they were not represented as passing.

## Conclusion

The candidate removes one top-level deserialization call for each `handled: true` result, but the
required early-return visitor added substantial protocol code and delivered only a small
end-to-end effect. Retaining it would violate the program's evidence rule for optional latency
candidates. The final tree therefore keeps the simpler established bridge and records this path
as closed unless a future transport redesign changes the cost boundary materially.
