# Flowchart U4 Dugong Batch-Retirement Preregistration

Date: 2026-07-28

This receipt freezes the native-memory admission bounds for the Dugong transient-node batch
retirement candidate before any candidate allocator matrix is collected. It does not admit the
candidate and does not close the remaining Flowchart adapter work in U4.

## Outcome

The latency and native-memory admission described below was not completed and must not be cited as
a measured speedup. The mechanism remains as an algorithmic complexity repair: repeated
`remove_node` calls invalidate and rebuild graph adjacency between removals, with worst-case
`O(T * (V + E))` work for `T` transient nodes, while `remove_nodes` consumes the targets and scans
the graph once in `O(T + V + E)` time with `O(V)` temporary space. Graph-mode equivalence,
compound-parent behavior, iterator-panic atomicity, normalization, and layout semantics are covered
by the Dugong and Graphlib suites. U10 ran all 391 tests successfully.

This is structural evidence only. No ordinary-fixture latency, allocation, or peak-memory claim is
attached to commit `c3130d4dc`.

## Frozen comparison

- Adjacent base: `84790e61a035e17dbbdc07a6b59703c54a2fe52f`
- Candidate mechanism commit: `c3130d4dcbd978893095e1aa2c07e6640ce7e49d`
- Frozen pre-optimization memory source: `06616dd71df627f6146217e2af58b9dbdd251122`
- Frozen baseline manifest commit: `9fbcd23c9`
- Candidate contract: `contracts/flowchart-u4-dugong-batch-memory-v1.json`
- Workload: `flowchart-modular-generator-v1`
- Scales: `1, 2, 4, 10, 32, 100`
- Repeats: five matched operation/zero pairs per scale, each in a fresh process
- Seed: `0x4D45524D414E`

The latency diagnostic was observed before this receipt, but no candidate native-memory sample was
collected. Base and candidate must use this exact contract file and digest. A bound passes only when
its bootstrap upper bound is at or below the cap; a crossing interval is inconclusive and a lower
bound above the cap fails.

## Bounds

| Metric | Frozen baseline upper bound | Slope cap | Scale-100 cap | Rationale |
|---|---:|---:|---:|---|
| Allocation count | 4,265,827 | 1.5 | 4,500,000 | Retain about 5.5% max-scale headroom while forbidding a worse growth class. |
| Allocated bytes | 33,701,769,467 | 2.0 | 6,442,450,944 | Require the repeated-CSR mechanism to remove at least the dominant scale-100 excess and restore at-most-quadratic growth. |
| Peak growth bytes | 144,199,480 | 1.5 | 167,772,160 | Retain a 160 MiB ceiling and about 16% headroom while preserving the baseline growth class. |

The shared `flowchart-end-to-end-memory-v1.json` remains an infrastructure smoke contract and must
not be used for admission. These candidate-bound limits qualify only the shared Dugong subcandidate.
U4 still requires adjacent public latency confirmation, exact semantic signatures, family controls,
and a separate decision on the remaining Flowchart adapter scans.
