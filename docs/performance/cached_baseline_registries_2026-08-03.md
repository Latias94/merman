# Cached Baseline Registries Decision (2026-08-03)

Status: rejected on the current branch after two statistically inconclusive confirmations. No
candidate production code remains.

## Question

Can repeated default `Engine` construction reuse process-local immutable detector, semantic-parser,
and render-parser baselines through the registries' existing `Arc::make_mut` copy-on-write storage?

The candidate preserved the pinned registry contents, detector order, parser resolution, public
APIs, and engine-local custom overlays. Its intended claim was limited to repeated fresh engines in
one already-running process; it did not claim a first-call or process-startup improvement.

## Revisions

- Adjacent base: `8f14db04d2e06257ddabf3f5eba185cd9d743af7`
- Candidate: `bcf79f91e9b950f6b83862afba1175b63bb5b501`
- Cleanup: `2d0cdd6bc96e867c6d264415036b8b8a862ef9c7`

The cleanup is an explicit inverse commit. A path-limited comparison against the adjacent base is
empty for:

- `crates/merman-core/src/detect/mod.rs`
- `crates/merman-core/src/diagram/mod.rs`
- `crates/merman-core/src/lib.rs`

## Registered evidence contract

The public lane was `parse_cold_engine/info_medium`, whose schema-v2 lifecycle is a reused process
with a fresh `Engine` per logical operation. Both sides used Rust 1.95.0, the `merman` `pipeline`
bench with `svg`, byte-identical corpus input, and independently built frozen executables.

The low-latency contract was fixed before sampling at:

- 10% relative and 1,000 ns absolute thresholds;
- 8 balanced A/A calibration pairs per side;
- at most 64 fresh balanced AB/BA confirmation pairs;
- 30 Criterion samples, 1 second warm-up, and 2 seconds measurement;
- bootstrap seed `2026080302` and 10,000 resamples.

Both executable receipts produced the same 31-byte typed render model with SHA-256
`471b942eb8877b2ee7c38b86567b13f79fba101df1e5b4767b3421a200e0ad3f`.

## Results

Neither run advanced to candidate A/B sampling, so this decision makes no measured speedup claim.

### Confirmation 1

The candidate A/A calibration was stable. The base calibration was not: one observation measured
5,995.8 ns while the other fifteen observations were between 4,443.0 ns and 4,576.3 ns. The
simultaneous relative identity and order-effect intervals crossed the registered equivalence
margin, so the harness stopped before A/B.

### Confirmation 2

The base A/A calibration was stable. The candidate calibration was not: two observations measured
1,947.3 ns and 2,081.5 ns while the other fourteen observations were between 983.0 ns and 1,138.4
ns. The resulting power calculation required 72 pairs, exceeding the preregistered cap of 64, so
the harness again stopped before A/B.

The two failures occurred on opposite sides, but the evidence rule is intentionally symmetric: a
large source-level upper bound or a prior result on another branch cannot replace stable adjacent
A/A and fresh A/B evidence on the current candidate.

## Correctness gates

While the candidate was present:

- `cargo nextest run --locked -p merman-core`: 1,393 passed;
- `cargo fmt --all -- --check`: passed;
- storage-sharing, copy-on-write detachment, cross-engine isolation, repeated fresh-engine,
  invalid custom registration, first-initialization, and concurrent-initialization tests passed.

These results establish semantic plausibility, not latency acceptance.

## Decision

Reject and remove the candidate under R4. Two confirmation attempts were inconclusive, so the
branch retains neither the `OnceLock` baselines nor candidate-only registry tests and comments.

The earlier July 2026 result for a related implementation remains historical context only. It used
different adjacent revisions and cannot satisfy this branch's causal evidence requirement.

## Raw evidence

- First confirmation JSON: SHA-256
  `b338c2565a5a33d34c28ce65888f1dc34522b499c545216cf24621d70f013955`
- First confirmation Markdown: SHA-256
  `8efc23e855873bc68f289145c5f4b20c6acd8ea9fab7270a63bd25b450736264`
- Second confirmation JSON: SHA-256
  `e2e2d2138b34b2f794257e91753dee739e624b455c92fc81def78faac9898e8f`
- Second confirmation Markdown: SHA-256
  `8efc23e855873bc68f289145c5f4b20c6acd8ea9fab7270a63bd25b450736264`

Raw reports remain under the ignored
`target/bench/experiments/headless-performance-hardening/u2/` directory.
