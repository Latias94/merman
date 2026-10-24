# Semantic editor demand: U3 preregistration

This checkpoint registers the acceptance bounds before production observer-demand changes.
It is not an improvement result. The approved semantic capability plan remains authoritative.

## Frozen inputs

- Baseline source: `3f0f3819a6013ed6882205cf80ea4aa1336b1a83`, after PR #225, the U2
  oracle-backed Flowchart correction and the shared-corpus harness repair.
- Probe: the unchanged `crates/merman/benches/parse_memory.rs` and native allocator owner;
  `tools/bench/corpus.json` registers the nine typed/JSON/snapshot observations.
- Native host: Windows 11, x86_64 MSVC, Rust 1.95.0, locked dependencies, bench profile,
  one Cargo job, incremental compilation and benchmark debug information disabled.
- The same reused engine and generated inputs are measured in fresh subprocesses. Results
  stay alive through peak/retained checkpoints and are dropped before the final checkpoint.
- Each observation uses scales 1, 2, 4, 10, 32 and 100, five matched operation/zero repeats,
  alternating order and 10,000 deterministic bootstrap resamples.

The clean Cargo-built State typed report and six full schedules using that frozen executable
established the values below. The latter schedules are explicitly diagnostic observations for
setting bounds: `--executable` does not create durable admission evidence. Full Cargo-built,
clean reports and verified baseline envelopes are required before U3 acceptance. The first
empty-corpus attempt and any failed freezes remain historical diagnostics.

Local evidence is retained under `target/bench/semantic-boundary/baseline-post-u2/`; the ledger
at `target/bench/experiments/semantic-capability-boundaries/experiment.yaml` records report
hashes, source identities and dispositions. The frozen probe executable SHA-256 is
`dcd3f5f7f155bf95ace5d1c8c8b6c5c0e09fe61c5007d5415d6209fe2a30c800`.

## Causal claim and fixed bounds

For lexical events E and emitted editor items F, an ordinary request currently performs an
additional collector traversal and O(F) fact materialization before discarding it. Omitted
requests must perform zero editor collections and zero actual fact appends. Requested capture
keeps its existing observer, canonical output, recovery facts and original source spans.
One lexical tape, grammar construction, DB replay, validation and required resource work remain.
This is a work-removal and allocation claim, not a speedup or whole-parser complexity claim.

At scale 100, both allocation count and allocated bytes must strictly decrease on the four
ordinary Sequence/State requests. Peak and retained growth must not exceed their baseline.
Requested snapshots and the unaffected Flowchart control must not exceed baseline on any metric.
The following absolute caps are fixed before candidate measurements:

| Lane | Allocations | Allocated bytes | Peak growth bytes | Retained growth bytes |
| --- | ---: | ---: | ---: | ---: |
| State typed | 12,489 | 1,626,028 | 687,654 | 510,328 |
| State JSON | 22,756 | 3,108,666 | 1,500,595 | 1,071,875 |
| Sequence typed | 3,515 | 728,740 | 353,713 | 192,611 |
| Sequence JSON | 4,860 | 827,842 | 355,851 | 268,161 |
| State snapshot | 22,844 | 3,200,859 | 1,500,595 | 1,143,130 |
| Sequence snapshot | 4,887 | 864,555 | 358,587 | 308,279 |
| Flowchart typed control | 7,749 | 2,076,117 | 912,176 | 263,262 |

Each metric also has a log-slope cap of 1.0 for this finite generated chain workload. This
guards scale behavior without treating a slope increase caused by lower fixed overhead as
regression by itself. It does not establish a worst-case bound for arbitrary Mermaid input.
The seven existing owner contracts hold these machine-readable caps; the two other Flowchart
contracts remain infrastructure-only. Do not adjust caps after observing a candidate.

## Required proof and controls

The test-only work counters observe real collector entry and symbol, directive, diagnostic and
expected-syntax appends. Before implementation, nextest `9c6480a7-ee74-4377-b9f5-45c7fea8748a`
failed all seven selected demand tests. The Class/ER/Pie aggregate covers all twelve detected/
known-type JSON/typed requests. Ordinary Sequence produced one collection, six symbol appends
and two expected-syntax appends; State produced one collection, nine symbol appends, two
directive appends and twelve expected-syntax appends. Their target is zero.

Acceptance also requires exact canonical output and structured errors, requested complete and
recovered facts, one syntax construction, real semantic validation failures, cancellation and
deadline precedence, deep lifecycle and unchanged security/resource admission. Capture append
counts describe materialization work; they need not equal the final retained vector lengths.
Existing full warm/cold typed, known-type JSON and end-to-end rows remain same-contract controls
under the normative `generic_v2` timing procedure. No latency improvement is claimed here.

Math removal is a separate changed-capability result. State ownership transfer follows U3 with
its own adjacent baseline and must not be counted as this observer-demand improvement.
