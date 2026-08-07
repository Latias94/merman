# Sequence Operation Metric Reuse Decision — 2026-08-07

## Decision

`accepted-latency`, `accepted-memory`

Merman now carries the qualified built-in measurement of an ordinary Sequence message from actor
spacing into later layout and SVG consumers through a private, operation-owned sidecar. The clean
adjacent candidate improved the two registered public high-message operations by 40.00% and
40.88%. Both normal controls remained non-regressing, while the candidate reduced allocation
count and allocated bytes without adding process-retained state.

This decision does not establish a global text cache and does not permit callback elision for an
opaque host or custom measurer. Reuse requires the exact prepared semantic model, immutable message
owner, complete text style, and built-in measurement-operation carrier. Wrapped messages, notes,
math, final direct-text probes, carrier misses, and every host/custom route retain their existing
measurement behavior.

## Revisions and behavior authority

- Implementation source: `92042047ad0b3f94b21cd18832446e670c92c95c`
- Candidate-neutral harness `H`: `fb2c58cb94dfb8f4a7c178a62c77769ebfdd497e`
- Control `A`, explicit no-reuse baseline:
  `f097d812361f4fb13848f2f524f221f1398ee30b`
- Candidate `B`, qualified operation reuse:
  `a74da31da440b947b986ab8e0ff7f80658a6cb26`
- Correctness hardening:
  - `e92bb2338f8c8c7395a99122a75eea8a617c2f82`
  - `00bfebcb8d37896f3a896e26a06910a08d41bbe4`
- Pinned Mermaid behavior reference: Mermaid 11.16.1 at
  `repo-ref/mermaid` commit `7ecca0cd7f1658ef74f4e7e91f925724ef403bbf`
- Reference release range: Merman `v0.8.0-alpha.3` to this performance branch
- Host: Apple M4 Pro, `aarch64-apple-darwin`
- Toolchain: Rust 1.95.0, Cargo 1.95.0
- Profile: Cargo `bench`, `CARGO_BUILD_JOBS=1`
- `Cargo.lock` SHA-256:
  `4eb38cee29796405570fa3172fffd049429fdef5692f868fd6be02e45ee93a71`

The pinned Mermaid source performs message measurement in three semantically distinct contexts:

1. `getMaxMessageWidthPerActor()` measures messages and notes for actor spacing, with separate
   wrapping and math handling.
2. `boundMessage()` measures ordinary message dimensions while updating vertical bounds.
3. `drawMessage()` measures the message again while emitting the SVG text.

The candidate preserves the resulting layout and SVG semantics. It reuses only the exact ordinary,
unwrapped, non-math message result already produced by the first built-in operation. Note,
wrapped, math, host, fallback, and direct-text paths remain independent.

## Public latency confirmation

The public confirmation used clean source revisions and separately frozen executables. Each side
first passed eight balanced A/A calibration pairs. The decision then used eight alternating AB/BA
pairs, 30 samples per observation, a two-second warmup, a three-second measurement window, 10,000
deterministic bootstrap resamples, and simultaneous 95% Bonferroni bounds.

| Public operation | Control | Candidate | Paired change | 95% relative interval | Absolute improvement |
|---|---:|---:|---:|---:|---:|
| `end_to_end/sequence_block_repeat_high` | 2,241,150 ns | 1,344,625 ns | -40.003% | -40.245% to -39.786% | 896,525 ns |
| `end_to_end/sequence_block_unique_high` | 2,280,925 ns | 1,348,425 ns | -40.882% | -41.177% to -40.609% | 932,500 ns |

The 95% absolute-improvement intervals were 889,087.5 to 904,181.3 ns for repeated messages and
923,875 to 944,476.1 ns for unique messages. Both lanes therefore clear the preregistered 10%
relative and 50-microsecond absolute gates by a wide margin.

Output identity matched exactly on both revisions:

| Fixture | SVG SHA-256 | Bytes | Elements |
|---|---|---:|---:|
| repeated high | `bae0f84a0f50cc831e7ca652b315c37c763056dbb5aa4138da609e3398c1207e` | 130,653 | 579 |
| unique high | `9fe0ca8a5dfecb6b93868265f14cbb499cf2b4cb356831dc51aff7d489ce5448` | 130,653 | 579 |

Raw evidence:

- `target/bench/experiments/u9-sequence-operation-metrics-v1/public-discovery.json`
  - SHA-256:
    `c0a6d86087b8dfe5fe05788f361d669294ae66b2d221da7837fd6fba944db431`
- `target/bench/experiments/u9-sequence-operation-metrics-v1/public-confirmation.json`
  - SHA-256:
    `5679f61e1c6360711b1d0b051859fe50ff0ee87865460147b3e0aec102bbb929`

## Normal controls

The same confirmation run included one Sequence control without messages and one unrelated diagram
family. Neither crossed the 10% regression limit.

| Public control | Control | Candidate | Paired change | 95% relative interval | Result |
|---|---:|---:|---:|---:|---|
| `end_to_end/sequence_actor_only_control` | 75,748.4 ns | 74,871.9 ns | -1.157% | -1.395% to -0.788% | non-regression |
| `end_to_end/state_medium` | 377,668.8 ns | 375,898.8 ns | -0.468% | -1.158% to +0.178% | non-regression |

Their exact output identities were respectively
`9d80d63061b78a095a47425c3b0492be0699a15699d9f085be4053c0c7d0c69c` and
`4952aae5a3ab562c06b0ecf8b0b7a075587d5ffbcb9f291db7c34b4ff8727e35`.

## Stage attribution

Two-pair diagnostics localize the public improvement without serving as acceptance gates:

- high repeated layout: approximately -52.88%;
- high unique layout: approximately -53.33%;
- high repeated SVG emission: approximately -39.91%;
- high unique SVG emission: approximately -40.53%.

Raw diagnostics:

- layout SHA-256:
  `8fc5182ad689bebebf28c8e9e6a432dcf781eb18695c426550e7e4ab73385809`
- render SHA-256:
  `e9adf448da00102397fe7a02db803d6b3aa7b58cc9d21068226f6ec91c17d5ae`

These results support the expected owner: the candidate removes repeated built-in message-bound
work across both layout and SVG emission. Only the end-to-end confirmation determines acceptance.

## Memory evidence

The native System allocator probe ran repeated-message and unique-message workloads at six scales
(`1, 2, 4, 10, 32, 100`), with five fresh-process operation/zero pairs per scale. All four reports
passed their source-bound contracts.

- Control executable SHA-256:
  `8c2ec6b2dbf39e5c4bb621bf1f3766216149962a26e8e6718cd23ff2b607fdc5`
- Candidate executable SHA-256:
  `1be4ce893fc4ac3af0dcfd98fcad835661c9bbe98008920e59d6f0488a53a087`

At 100 equal-length messages, repeated and unique workloads have the same allocation shape:

| Metric | Control | Candidate | Delta |
|---|---:|---:|---:|
| Allocation count | 10,707 | 6,809 | -3,898 (-36.4%) |
| Allocated bytes | 2,771,197 | 2,636,708 | -134,489 (-4.85%) |
| Peak growth bytes | 270,413 | 270,413 | 0 |
| Retained growth while the prepared artifact is alive | 86,309 | 88,920 | +2,611 |

Every fitted slope upper bound remained below the registered `1.35` cap. Every sample also
returned to its pre-operation live-byte snapshot after the prepared artifact was dropped, so the
candidate adds no process-retained state.

Raw evidence:

| Revision and lane | Report SHA-256 |
|---|---|
| control repeated | `e9ff97660849e24b9e208a7fa1f37b44e344ec04bef5ec6e933492d0ca6f0933` |
| control unique | `7be5a2b7d4804908f2d36c0766b26bb02a9f4ef9d83e4072ed14d045948d331e` |
| candidate repeated | `e6099776e661fbe762c7c1de6e54d4a684ce530f47053c02de454b07697f73a3` |
| candidate unique | `e4191759979f3090a6828a84b63a6d23292b566e206b4fdebe074a3705c7050a` |

The owner contracts are:

- `docs/performance/contracts/sequence-message-repeated-memory-v1.json`
  - SHA-256:
    `7493500d998944c2a2e6de1ea8b3fc6f3d124c90e19dffdfc830195c2d1724fa`
- `docs/performance/contracts/sequence-message-unique-memory-v1.json`
  - SHA-256:
    `5b0a71cfe7729d1f8e438dd5a519293b0ff51c4612345f496c4248df00558b4d`

## Correctness and callback contracts

The final candidate passed:

- default-feature Sequence integration tests: 40 passed, 1 skipped;
- math-feature Sequence integration tests: 43 passed, 1 skipped;
- full `merman-render --features math`: 1,504 passed, 3 skipped;
- strict all-target `merman-render` Clippy with math;
- `cargo fmt --check` and `git diff --check`.

The focused contracts prove:

- exact model identity, semantic owner, complete `TextStyle`, and built-in carrier binding;
- Stateful host success preserves the full callback trace and can still alter geometry;
- host error preserves trace, fallback behavior, SVG, phase, operation, source, and profile
  provenance;
- wrapped messages, notes, math, and direct-text probes do not reuse ordinary-message metrics;
- concurrently alive prepared artifacts do not share measurement state;
- layout and SVG identities remain exact across `A` and `B`.

The tests deliberately do not treat fewer host calls as an optimization. Opaque host observability
remains part of the public behavior contract.

## Rejected evidence attempts

Two protocol failures are retained for auditability and excluded from the decision:

1. The first control repeated-memory command used a 180-second timeout and expired during the clean
   prebuild. It produced no performance conclusion. The successful matrix used 1,800 seconds.
2. A combined layout/render attribution filter was rejected before building because registered
   benchmark groups must be measured separately. The accepted diagnostics use separate layout and
   render reports.

## Claim boundary

The accepted latency claim applies only to ordinary, unwrapped, non-math Sequence messages rendered
through the exact built-in operation carrier. The accepted memory claim applies to the recorded
operation-local sidecar and the registered message-count range. No claim is made for opaque hosts,
custom measurers, notes, wrapped labels, math labels, browser measurement, process-wide caches, or
other diagram families.
