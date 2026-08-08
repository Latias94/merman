# Flowchart SVG Label Sidecar Decision — 2026-08-06

## Decision

`accepted-latency`, `accepted-memory`

Merman now carries one private, operation-owned preparation of eligible non-Markdown SVG
Flowchart labels from layout into SVG emission. The candidate is accepted because the clean
adjacent public operation improved by 12.43%, both normal controls stayed below a 1.30% upper
regression bound, and the added operation-local memory remained linear and below every
preregistered cap.

This receipt does not claim a global text cache or fewer opaque host callbacks. Prepared
measurements are reused only for the built-in operation carrier with an exact owner, style,
width, wrapping, and width-mode binding. HTML, Markdown, custom host, fallback, and carrier-miss
paths preserve their measurement behavior.

## Revisions and behavior authority

- Control `A`: `c1e5c4a522481c85d910d34af8a7ee3f38563c64`
- Candidate `B`: `0445e99677483a6624c614e80611d0c1693d827f`
- Pinned Mermaid behavior reference: Mermaid 11.16.0 at
  `repo-ref/mermaid` commit `7c0cafcf42e76bfaf79d0cbbd12edb986612f014`
- Host: Apple M4 Pro, `aarch64-apple-darwin`
- Toolchain: Rust 1.95.0, Cargo 1.95.0
- Profile: Cargo `bench`, `CARGO_BUILD_JOBS=1`
- Fixture: `crates/merman/benches/fixtures/flowchart_svg_label_reuse.mmd`
- Fixture SHA-256:
  `b1629d27f98d070d75fdd251b5f8ddca62e2e51b128b48fb622e16a16e747313`

The official source establishes two distinct non-Markdown SVG measurements: wrapping probes use
`getComputedTextLength()`, while final label dimensions use `getBBox()`. Empty subgraphs follow
the ordinary node path and use the configured Flowchart wrapping width. The candidate preserves
that split and does not add a post-wrap computed-length pass.

## Public latency confirmation

The public lane used clean frozen executables, eight balanced A/A pairs on each revision, and
eight alternating AB/BA pairs. Each Criterion observation used 20 samples, a one-second warmup,
and a two-second measurement window. Bounds use 10,000 deterministic bootstrap resamples with
seed `20260806` and a 95% simultaneous Bonferroni contract.

| Metric | Control | Candidate | Paired result |
|---|---:|---:|---:|
| Median latency | 3,657,475 ns | 3,202,650 ns | -12.434% |
| Absolute improvement | — | — | 454,825 ns |
| 95% relative interval | — | — | -12.884% to -12.015% |
| 95% absolute improvement interval | — | — | 437,650 to 472,487.8 ns |

Both A/A calibrations were stable. The output identity matched exactly on both revisions:

- SVG SHA-256: `79e63decea21f98f6f56a7864c9c101dcc94c22fb1a63c5fffbb9960e255ac5e`
- SVG bytes: 165,503
- SVG elements: 1,653

Raw evidence:

- `target/bench/experiments/flowchart-svg-label-sidecar-v1/confirmation-seed-20260806-retry.json`
  in the clean head evidence worktree
- SHA-256: `5dfd9895d2f2dc878cfc32a8643dbb38463c4011a1229fb3a1fa88c3a7b4c2f8`

The first seed-`20260806` attempt was inconclusive because one head A/A order-effect bound was
51.0 microseconds against a 50-microsecond margin. The one permitted identical-protocol retry is
the accepted result above. An earlier seed-0 diagnostic is not used for the decision.

## Normal controls

The first quick control attempt was inconclusive, so the controls received a separately frozen,
longer protocol: 30 samples, two-second warmup, five-second measurement, seed `20260806`, and the
same balanced calibration and AB/BA rules. Flowchart completed in the joint run. Class required
the one permitted identical-protocol retry after base A/A noise.

| Public control | Control | Candidate | Paired change | 95% relative interval | Result |
|---|---:|---:|---:|---:|---|
| `end_to_end/flowchart_medium` | 2,003,120 ns | 2,003,290 ns | +0.0072% | -0.894% to +1.297% | confirmed non-regression |
| `end_to_end/class_medium` | 643,072.5 ns | 643,337.5 ns | +0.0398% | -0.951% to +1.071% | confirmed non-regression |

Raw evidence:

- Flowchart control SHA-256:
  `14279eb56add08eb1602971deafc18d7709c1b182627f85d72ee25fd87a9b089`
- Class retry SHA-256:
  `8d80b249912814e8b628e8ec9363228d3c00dc750ec27803a442a189cf7eb088`

## Stage attribution

Criterion diagnostics explain the public result without serving as the acceptance gate:

- unique-label preparation at 256 labels changed by approximately +2.45%; this is the cost of
  retaining the bound sidecar artifact;
- SVG emission at 256 labels improved by approximately 50.89%; the render phase no longer
  repeats eligible tokenization and built-in wrapping;
- the public end-to-end confirmation above is the causal latency decision.

## Memory evidence

The native System allocator lane ran six scales (`1, 2, 4, 10, 32, 100`) with five fresh-process
operation/zero pairs per scale. The driver reset the shared Cargo bench profile before each clean
revision build. The resulting executables were different and source-bound:

- Control executable SHA-256:
  `1725f2f634657c430e095a95a79e05360df38b8110e9ea61c4039aa559414f78`
- Candidate executable SHA-256:
  `d536b850223278388dc736347af6b5343050845ef91e27c5b0f3951261ffb030`

At scale 100:

| Metric | Control | Candidate | Delta | Gate |
|---|---:|---:|---:|---:|
| Allocation count | 31,085 | 31,509 | +424 | 40,000 candidate maximum |
| Allocated bytes | 4,478,453 | 4,594,327 | +115,874 | +1,048,576 regression cap |
| Peak growth bytes | 1,017,785 | 1,223,531 | +205,746 | +1,048,576 regression cap |
| Retained growth bytes | 237,852 | 445,782 | +207,930 | +524,288 increment cap |

Every metric passed its owner contract, all fitted slopes remained below `1.25`, and
`live_bytes_after_drop - snapshot_live_bytes` was exactly zero for every control and candidate
sample. The sidecar therefore adds bounded operation-local memory and no process-retained state.

Raw evidence:

- Control memory SHA-256:
  `eb5683f60926aec3108b310f043f779bf98ec445659e42bd23495339caa6ed18`
- Candidate memory SHA-256:
  `19a6f02936e6a9925c3ecad15f47a752276ce46d2739f87ff636af610e103c19`

Two earlier reports that produced byte-identical executables for different Git trees were
rejected. They exposed unsafe shared-target reuse in the driver and did not contribute to this
decision.

## Correctness and contract gates

The focused owner matrix passed 131 tests across:

- `flowchart_layout_test`
- `flowchart_svg_label_measurement_contract_test`
- `flowchart_svg_test`
- `swimlane_layout_test`

Coverage includes nodes, edges, subgraph titles, empty subgraphs, self-loops, ELK edge mapping,
Swimlane owners, host and fallback traces, carrier misses, HTML/Markdown exclusions, explicit
breaks, entities, NBSP, and deep hierarchy behavior. Strict `merman-render` check and Clippy had
also passed for the candidate implementation before measurement.

## Evidence-tool repairs

The measurement exposed and fixed three evidence-integrity problems without changing the
candidate runtime:

1. Frozen-runner rediscovery now compares selected output receipts instead of rejecting a
   Flowchart lane because unrelated unseeded State/Rough receipts changed.
2. One frozen runner pair can be reused across independently registered benchmark selections;
   current fixture identity and selected output receipts are still revalidated.
3. Native-memory builds now reset the shared bench profile and accept an explicit repository root,
   preventing Cargo artifacts from a different worktree from being attributed to the requested
   commit.

The Python performance contract suites passed after these changes.

## Claim boundary

The accepted latency claim applies to eligible non-Markdown SVG Flowchart labels on the built-in
operation carrier. The accepted memory claim applies to linear operation-local sidecar growth
within the recorded caps. No claim is made for HTML/Markdown labels, opaque host callbacks,
browser text measurement, global caches, or unrelated diagram families.
