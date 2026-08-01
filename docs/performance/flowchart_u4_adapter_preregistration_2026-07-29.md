# Flowchart U4 Adapter Preregistration

Date: 2026-07-29

This receipt freezes the latency, work-curve, semantic, and native-memory admission criteria for
the Flowchart adapter candidate before production adapter code is changed or candidate evidence is
collected. It is intentionally separate from the Dugong batch-retirement complexity repair, whose
latency and native-memory admission was not completed.

Retirement note: the candidate was rejected before native-memory confirmation. Its two
candidate-bound memory lanes, contracts, and generators were subsequently removed from the active
benchmark surface. The values below preserve the historical preregistration only and are not a
runnable admission contract for a future candidate.

## Frozen comparison

- Adjacent base commit: `6b5f3e0ef2bc1b3162712b5a2de71fe8f887e213`.
- Adjacent base tree: `7e9f82864725122c37e7b8931c3bdaaf5f790a4f`.
- Candidate scope: Flowchart compound-hierarchy adjustment and cluster extraction only.
- Public primary lane: `flowchart_large`.
- Public controls: `flowchart_medium`, ports-heavy Flowchart, and `class_medium`.
- Diagnostic panels: fixed-node cluster/edge interaction, boundary-connected clusters,
  extractable clusters, bounded-degree growth, and true edge-density growth.
- Historical native-memory contract IDs: `flowchart-u4-adapter-low-cluster-memory-v1` and
  `flowchart-u4-adapter-high-cluster-memory-v1` (retired after rejection).
- Low-cluster contract SHA-256:
  `2c9291e89e60a06763a2e872328233176a16c557f9c8c8ed3cc8b1ab9336d056`.
- High-cluster contract SHA-256:
  `18b27ca429c9c9b52afb869893e4df7c8b1f25a2f7b9c7a00cfbd6cb62c85634`.
- Native-memory scales: `1, 2, 4, 10, 32, 100`.
- Native-memory repeats: five matched operation/zero pairs per scale, each in a fresh process.
- Seed: `0x4D45524D414E`.

The two memory workloads keep exactly four nodes and four edges per scale. The low-cluster lane
keeps two clusters while the high-cluster lane grows clusters with scale. Even clusters have
boundary edges to an unclustered hub; odd clusters remain extractable. Both contracts would have
been required for this candidate's admission.

## Frozen memory bounds

One pre-candidate boundary pair at 1x and 100x established the order of magnitude. The 100x low
lane observed 546,300 allocations, 83,389,043 allocated bytes, and 6,801,424 bytes of peak growth.
The 100x high lane observed 931,819 allocations, 122,364,609 allocated bytes, and 9,361,306 bytes
of peak growth. These exploratory values are not admission evidence.

| Lane | Metric | Slope cap | Scale-100 cap |
|---|---|---:|---:|
| Low cluster | Allocation count | 1.4 | 700,000 |
| Low cluster | Allocated bytes | 1.4 | 134,217,728 |
| Low cluster | Peak growth bytes | 1.4 | 12,582,912 |
| High cluster | Allocation count | 1.4 | 1,200,000 |
| High cluster | Allocated bytes | 1.4 | 167,772,160 |
| High cluster | Peak growth bytes | 1.4 | 16,777,216 |

A metric passes only when the bootstrap upper bounds for both slope and max-scale value are at or
below their caps. A crossing interval is inconclusive; a lower bound above a cap fails. Contract
files and their digests were frozen before the adjacent candidate was measured.

## Admission rules

The candidate is admitted only when all of the following hold:

1. A/A calibration passes for both frozen executables with at least eight balanced pairs each.
2. Fresh balanced AB/BA evidence reports `confirmed_improvement` for `flowchart_large` under the
   preregistered relative and absolute minimum effect.
3. None of the three public controls reports a confirmed regression.
4. Both native-memory contracts pass from clean source identities with exact executable digests.
5. Fixed-node cluster/edge panels remove the old cluster-by-edge and node-by-edge work curves; a
   test-only exact work counter confirms no per-cluster or per-node full-edge scan remains.
6. Differential tests preserve node order, edge order, labels, parents, extraction timing,
   collision winners, SVG hashes, and viewBox dimensions.
7. The public runtime resource-policy work-unit behavior remains unchanged. Any policy migration
   requires a separate breaking contract and is outside this candidate.

No single microbenchmark, profiler sample, or process exit code can substitute for this complete
admission set.
