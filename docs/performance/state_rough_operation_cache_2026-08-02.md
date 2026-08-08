# State Rough operation-owned cache decision — 2026-08-02

## Decision

Status: **accepted-memory**, **accepted-structural**.

The State SVG renderer no longer retains Rough geometry in process-global or thread-local
caches. Seeded reuse is owned by one `StateRenderCtx` operation and is released on success,
ordinary `Result::Err`, unwind, and concurrent completion. Unseeded and fallback-capable seed
paths continue to bypass deterministic reuse. The current synchronous render API has no
production cancellation control or checkpoint, so cancellation is explicitly outside this
claim; no test-only cancellation sentinel is used as evidence.

The adjacent public latency control passed as **confirmed non-regression**, but the candidate was
7.67% slower at the point estimate. This receipt therefore makes no `accepted-latency` or speedup
claim.

## Adjacent evidence boundary

| Role | Commit | Tree | Meaning |
|---|---|---|---|
| `H'` | `6b07f9a2e4eaa5796d275c7be6fc068a4a006ff6` | `72b3ab4d7f73f46963d7cbe5ab3c4c1681c2ae3a` | Clean constructed baseline: exact v2 lifecycle harness with the legacy global/TLS cache implementation. |
| `C'` | `0cc646bef5b9c0cb362630892498f5337989cc9e` | `d0a6b2bc3b86c8b6a4c18921d4ab9118cd50c871` | Direct child of `H'`: operation-owned cache plus removal of unused Rough construction for native classic/neo rectangles. |
| branch source | `8f3f8effe5a234da8d293350e197da75a2bc87cb` | `d0a6b2bc3b86c8b6a4c18921d4ab9118cd50c871` | The accepted branch tree; byte-identical to `C'`. |

`H' -> C'` is an evidence-only synthetic adjacent pair, not a historical release range. Both
sides are clean, have one parent, and use identical fixture schedules, controls, contract files,
Python driver bytes, and Rust probe bytes. `C'` names `H'` as its first parent.

Harness identities:

- `tools/bench/run_state_rough_lifecycle.py`: SHA-256
  `ba1f33462170c27833e9cffd7fd56199373bc14ac1b00a5bd070af1942a99f38`.
- `rough_lifecycle_probe.rs`: SHA-256
  `ea6d8183277cbd272554b7555f6eaa396c0c91974162e6340900200973a99c56`.
- `state-rough-lifecycle-v2` contract: SHA-256
  `73795e3bbb6755d4bbced354dda97bc01aeb2909eec61070034c86bf5afa754f`.

## Bound and lifecycle result

Let `U` be the number of distinct seeded geometry keys observed across prior render operations,
`K` the number of distinct keys within the current operation, and `B` their owned path-string
capacity.

- Before: the process-global maps retained `O(U)` entries and `O(B(U))` bytes without a bound;
  each thread also retained up to 4,096 circle keys and 4,096 path keys.
- After: post-operation retained entries and owned bytes are exactly zero. Peak cache ownership is
  `O(K)` entries and `O(B(K))` bytes for the current render only.
- Repeated geometry within one seeded operation still records operation-cache hits. Cross-operation
  global/TLS hits are exactly zero.

The same single `Engine` executed 2,057 operations, including 2,048 distinct long-lived requests
across two render threads. The v2 Weak-allocation witness is sampled after the operation cache is
dropped, independently of map accounting.

| Measure | `H'` legacy cache | `C'` operation cache |
|---|---:|---:|
| Final global retained entries / bytes | 14,371 / 232,296,045 | 0 / 0 |
| Final TLS retained entries / bytes | 2,083 / 43,447,969 | 0 / 0 |
| Maximum TLS retained entries / bytes | 7,370 / 99,571,002 | 0 / 0 |
| Maximum live allocation witnesses after operation | 12 | 0 |
| Maximum live witnessed bytes after operation | 114,013 | 0 |
| Maximum operation-local peak entries / bytes | 7 / 114,013 | 7 / 114,013 |

Both sides executed exactly 14,385 geometry witnesses and 24,660 independent allocation
witnesses covering 232,523,267 owned bytes. Every cache drop was observed. The candidate proves
zero live witnesses after success, populated-cache error return, unwind, concurrency, and recovery.

The aggregate output identity is exact on both sides:

- SVG bytes: 592,581,296.
- SVG elements: 465,122.
- SHA-256: `0cb7a235bc0ff2c15fe93298522df0c658c6265b2b8ea0adcc52bccd87368fe1`.

The controls also bind configured seed `0`, fallback-capable seeds `4294967296` and `-1`, circle
and path keys, operation-local hits, prior global/TLS hit behavior, ordinary error recovery,
unwind recovery, concurrent overlap, and serial/concurrent SVG identity.

## Public latency control

`compare-self-v2` ran eight balanced confirmation pairs for the byte-identical
`end_to_end/state_medium` fixture. Both sides emitted the same 41,399-byte, 196-element SVG with
SHA-256 `4952aae5a3ab562c06b0ecf8b0b7a075587d5ffbcb9f291db7c34b4ff8727e35` before and after sampling.

| Metric | Result |
|---|---:|
| `H'` paired median | 392.918 us |
| `C'` paired median | 423.060 us |
| Relative estimate | +7.67% |
| 95% simultaneous upper bound | +8.20% |
| Absolute estimate | +30.143 us |
| 95% simultaneous upper bound | +32.184 us |
| Registered gates | 10% relative and 1,000 us absolute |
| Outcome | `confirmed_non_regression` |

The point estimate is a small slowdown, not an improvement. It remains below both preregistered
regression limits, including the Bonferroni simultaneous one-sided upper bounds. The mandatory
memory/lifecycle fix is accepted independently of an ordinary latency win.

## Raw artifacts

Raw reports remain under the ignored `target` tree:

- `target/bench/state_rough_lifecycle_v2_baseline_6b07f9a2e.json`: 2,389,292 bytes,
  SHA-256 `07f8253e7e18f6f2bca6c98d42dd2511cd660ed5d184d0404dda03b61c9fca2f`.
- `target/bench/state_rough_lifecycle_v2_candidate_0cc646bef.json`: 2,377,556 bytes,
  SHA-256 `6bf16d79daf21e92643ca8f0dee7ee9e3dcbc5353cbc08091a297f11b370023a`.
- `target/performance/u4_state_v2_end_to_end.json`: 475,388 bytes,
  SHA-256 `16c785caee959045fcc2fac6523f9bde18511021ece9ef8b31633d02d46f5a62`.
- `target/performance/u4_state_v2_end_to_end.md`: 1,597 bytes,
  SHA-256 `79151d45b816fb3ee30e959a867a7137a8d3107e83b39266907e8d832aeeab99`.

The lifecycle probes were generated at `2026-08-02T12:27:53Z` and
`2026-08-02T12:30:57Z`. The latency report was generated at
`2026-08-02T20:34:50.419006+08:00` on macOS 26.5.1, Apple M4 Pro, arm64, Python 3.14.6, and
`rustc 1.95.0`; Cargo builds used `CARGO_BUILD_JOBS=1`.

## Verification

- Both v2 lifecycle admissions and their release/error/unwind/concurrency controls passed.
- `python3 -m unittest tools.bench.test_state_rough_lifecycle_contracts`: 26 passed.
- `python3 -m unittest test_perf_contracts` from `tools/bench`: 100 passed.
- `CARGO_BUILD_JOBS=1 cargo nextest run --locked -p merman-render` at clean `C'`: 1,228
  passed, 3 skipped.
- `CARGO_BUILD_JOBS=1 cargo clippy --locked -p merman-render --tests -- -D warnings` at
  clean `C'`: passed.
- The adjacent eight-pair public latency comparison exited zero with
  `confirmed_non_regression`.

The pre-existing untracked `rust_out` and `test-results/` paths were not read, modified, removed,
or used as evidence.
