# Interactive layout-work calibration — 2026-08-07

## Decision

Status: **accepted-structural**.

Keep the `interactive` profile at `max_layout_work_units = 800,000`. This is a resource-policy
decision, separate from U5's already accepted work-accounting implementation. It admits no latency,
memory, throughput, or host-SLO claim.

The registered rule requires both 100,000 absolute units and 10% headroom over the maximum member
of the closed corpus, then rounds the required ceiling upward to a 100,000-unit quantum. The maximum
observed fixture consumed 697,752 units, so the rule computes:

```text
round_up(max(697,752 + 100,000, 697,752 * 1.10), 100,000) = 800,000
```

The accepted ceiling leaves 102,248 units, or 14.653917% relative to the corpus maximum. That is
12.781% of the ceiling. Exact `W/W-1` behavior, a deterministic node/edge cardinality boundary,
configuration amplification, isolated stage paths, timeout behavior, peak RSS, and output hashes
were recorded before accepting the policy.

## Durable evidence

The committed evidence manifest is
[`evidence/interactive_layout_work_calibration_2026-08-07.json`](evidence/interactive_layout_work_calibration_2026-08-07.json).
It binds the ignored raw evidence by byte length and SHA-256.

| Field | Value |
|---|---|
| Authoritative evidence date | `2026-08-07` |
| Source revision | `b32dae9521af2900f14fb267498d712d20f70eba` |
| Source tree | `80e0c746b58784d9e49f882344845da8e23fb077` |
| Tracked worktree | Clean before and after every probe |
| Build profile | `release` |
| Feature closure | `svg`, `layout-cytoscape`, `layout-elk`, `math`, `complete-svg` |
| Calibration source SHA-256 | `880b3fc558e6ace59f5174e48af203893dbcd343a6e473106a7719db867d7678` |
| Runner SHA-256 | `e9c660141ddf554da0f1d64d22faf18b4882d6a3c5dd9d8bff08025995684321` |
| Executable SHA-256 | `3f772fb806cbc9908491dcce1d63e710c5fa392ff61027d368ae7de3ce95816d` |
| `Cargo.lock` SHA-256 | `4eb38cee29796405570fa3172fffd049429fdef5692f868fd6be02e45ee93a71` |
| `crates/merman/Cargo.toml` SHA-256 | `0b7687ad2a989792fcbe78eab7bf95d7d63018b460ba85c097e8235718cb2959` |
| Corpus manifest SHA-256 | `cb7cca600d3980e77180832bff599f7f79608c4a41ff1116a2d4f4fd8cf52e02` |
| Fixture-member aggregate SHA-256 | `54a5e69af77e60fd545018db53336b1dd1f9c3d6a229aa7a47e4090e6060180c` |
| Raw summary | 18,319 bytes; SHA-256 `9b9d9d988bec4c337d12aa5fe167d0c925f89467b388bceab0009178edb16478` |
| Raw stderr bundle SHA-256 | `4ca69d28a23f03f9190c1cecf2b91dc9ce9b2779848fb1434e000d471142742c` |

The probe fails closed on tracked changes, untracked owned inputs, a changed postflight source
snapshot, or a feature closure different from the exact set above. Pre-existing unrelated
untracked paths are listed in the evidence manifest and were not used. The authoritative date and
Git ancestry establish chronology; future-skewed host timestamps do not.

## Host and process envelope

| Field | Value |
|---|---|
| Host | Apple M4 Pro, `Mac16,7`, 48 GiB physical memory |
| Platform | macOS 26.5.1, Darwin 25.5.0, `arm64` |
| Toolchain | Rust/Cargo 1.95.0 |
| Outer timeout | 300 seconds per process |
| Recorded processes | 12 |
| Exit status | 0 for all processes; no timeout |
| Five full-run elapsed range | 2.114740-2.192071 seconds |
| Five full-run peak RSS range | 136,871,936-146,145,280 bytes |
| Maximum recorded peak RSS | 146,145,280 bytes (139.38 MiB) |
| Full report | 38,275 bytes; SHA-256 `05b6aabc97ad2583922f2f2b632ef579e3ce6060720335e6c91267ad2fba8bdf` |

All five fresh full processes produced byte-identical reports. External elapsed and RSS establish
the recorded calibration envelope only. They are not a portable bound or an admitted performance
claim.

## Closed corpus and headroom

The probe reads the closed schema-2 list in `tools/bench/corpus.json`; it does not scan arbitrary
fixture directories. All 68 manifest members completed under the default `interactive` policy.
This receipt deliberately says “closed 68-member corpus”, not “all benchmark fixtures”.

| Field | Value |
|---|---|
| Maximum fixture | `flowchart_large` |
| Source | `crates/merman/benches/fixtures/flowchart_large.mmd` |
| Source bytes | 6,419 |
| Source SHA-256 | `edba73dc4c0a12417e052ec3c98ada82b5d773c4346b5c735df5dfa354f5b161` |
| Layout-work units | 697,752 |
| SVG bytes / elements | 307,649 / 2,902 |
| SVG SHA-256 | `1a43732507bd36e0c26776bc7a768cb5400a70bb429acfeb47d1077f57a315fe` |
| Accepted headroom | 102,248 units; 14.653917% over the maximum fixture |

## Exact fixture threshold

The maximum fixture was rerun at adjacent explicit ceilings:

- `697,752` succeeded, reported exactly 697,752 units, and produced the same SVG hash.
- `697,751` returned the structured error
  `cause=ceiling`, `phase=layout_model`, `limit=max_layout_work_units`, `actual=697752`,
  `max=697751`, `profile=interactive`, with the exact explicit override preserved.

This proves that the reported work controls admission and guards against an off-by-one or
report-only implementation.

## Registered cardinality boundary

The `flowchart-linear-chain-v1` curve generates a deterministic `N`-node, `N-1`-edge Flowchart.
Exponential bracketing followed by binary search records the last success, first rejection, and
next rejection. The result applies only to this registered curve; it is not generalized to all
Flowchart topologies.

| Nodes | Edges | Result | Work/output evidence |
|---:|---:|---|---|
| 1,643 | 1,642 | Accepted | 799,995 units; source SHA-256 `6f99bafbcd37fec43852f59ee0bc448b8998248b257f4374458e49f09ffe60b9`; SVG SHA-256 `ee77bc6284e187bb3c8b1f410c1843bdc22fb7fc88bf75501bc3b8b5ee9a6242`. |
| 1,644 | 1,643 | Rejected | Structured ceiling error with `actual=800002`; source SHA-256 `797b423414a134ef4917e58814f0b5328ff8888bd0360917c71f6fb5a2ffed2c`. |
| 1,645 | 1,644 | Rejected | Structured ceiling error with `actual=800001`; source SHA-256 `7f52983bdfd8ccd5cbcc03e5665e1b25c171bcb0b6cd5eae721fc35fbf61d609`. |

The accepted render emitted 2,397,515 SVG bytes and 23,034 elements. Its separate process peaked
at 87,932,928 bytes RSS and completed within the 300-second timeout.

## Configuration amplification boundary

A deterministic Architecture input fixes 32 services, 31 edges, `randomize=false`, and `seed=1`,
then varies only `numIter`. Above FCoSE's `5 * nodes` floor, the admission expression is strictly
increasing for this fixed shape.

| `numIter` | Result | Evidence |
|---:|---|---|
| 3,140 | Accepted | Source SHA-256 `1166a9ce9099394bb079cdb1921748eae6379f0c8017dd1e517b728141d35324`; final report consumed 48,589 units; SVG SHA-256 `e5a3fd3a1d45bfe8bccaafadd6a222633ac15132fb90ab3590b804878cc9a23b`. |
| 3,141 | Rejected | Structured ceiling error with `actual=800131`; source SHA-256 `9e74d83c995413035337bdb34c4908a16e9a4f981d94d1daad9a6e8983eda756`. |
| 3,142 | Rejected | Structured ceiling error with `actual=800385`; source SHA-256 `65918c9348ac0c4ac6630f85e4acce2edf855d507dbc52eddc46a4739960e611`. |

The accepted final consumption is lower because the 800,000-unit calculation is a checked,
non-consuming upper-bound preflight. The typed rejection validates the configured amplification
boundary; final consumed units must not replace the admission estimate.

## Isolated stage and failure paths

Each stage probe runs in a fresh process. Semantic work is timed directly. Layout preparation
finishes before the layout timer; render preparation finishes before the SVG timer. End-to-end and
failure probes use the public complete path.

| Lane | Internal elapsed | External elapsed | Peak RSS | Result |
|---|---:|---:|---:|---|
| `semantic` | 846,000 ns | 0.754493 s | 45,498,368 bytes | `flowchart-v2` semantic model |
| `layout` | 9,314,750 ns | 0.741321 s | 56,836,096 bytes | Layout JSON SHA-256 `1080294a3c2c7169a17c62b9df55870f0eace63f130a098af05d1301614b55cc` |
| `svg` | 1,697,792 ns | 0.753719 s | 54,329,344 bytes | Corpus-maximum SVG hash preserved |
| `end-to-end` | 9,776,375 ns | 0.717802 s | 54,362,112 bytes | Corpus-maximum SVG hash preserved |
| cardinality rejection | 35,824,250 ns | 0.745800 s | 78,528,512 bytes | Typed `800002 > 800000` rejection |
| exact `W-1` rejection | 8,408,834 ns | 0.765439 s | 53,542,912 bytes | Typed `697752 > 697751` rejection |

These timers prove that every required path was exercised and bounded by the runner timeout. They
are not an A/B performance measurement.

## Reproduction

Build the exact feature closure and run the evidence wrapper:

```sh
CARGO_BUILD_JOBS=1 cargo build --locked --release -p merman \
  --example layout_work_calibration --features complete-svg

python3 tools/bench/run_layout_work_calibration.py \
  --authoritative-date 2026-08-07 \
  --out-dir target/bench/layout-work-calibration-2026-08-07-final \
  --timeout-seconds 300 \
  --full-repeats 5
```

The wrapper uses `/usr/bin/time -l` on Darwin or GNU `time -v` on Linux, enforces the timeout with
the Python subprocess API, launches every lane in a fresh process, verifies five byte-identical
full reports, derives the maximum fixture and adjacent cardinality probes from the full report, and
binds commands, exit status, RSS, report hashes, source, runner, executable, toolchain, and host.

## Scope

The pinned Mermaid implementation has no public `max_layout_work_units` constant. The value is a
Merman host-safety policy and does not alter accepted diagram semantics. The `constrained` profile
remains at 125,000 units. Parse, SVG-byte, raster, and output-cardinality limits were not changed.
Untrusted or multi-tenant hosts still need wall-clock, memory, concurrency, and process-isolation
controls outside this admission policy.
