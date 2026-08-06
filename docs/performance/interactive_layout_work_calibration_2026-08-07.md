# Interactive layout-work calibration — 2026-08-07 (provisional)

## Decision

Status: **provisional-calibration**.

The `interactive` profile currently keeps `max_layout_work_units = 800,000`. The closed 68-member
native corpus reaches 697,752 owner-accounted layout-work units, leaving 102,248 units (14.6539%)
above the maximum fixture. This receipt records the observed value and typed rejection boundary,
but does not yet accept the policy as final host-safety evidence.

U5 itself remains `accepted-structural` in the plan and its existing receipt. This policy calibration
is a separate resource-profile evidence item. A final calibration receipt must add isolated
semantic/layout/SVG/failure lanes, bind the external timeout/RSS summary, and include the
node/edge cardinality boundary from the release-v2 probe.

## Revision and executable provenance

| Field | Value |
|---|---|
| Authoritative evidence date | `2026-08-07` |
| Git revision | `f5d3e51722fa78c02a5e316c05fa3fe382eeaef8` |
| Git tree | `9a7fa706ebef233da4c5aacf4d7539c036a66d6a` |
| Direct parent | `255c072ffe1d95c21d192cb0677b35dab011a14c` |
| Tracked worktree | Clean before and after every probe |
| Build profile | `release` |
| Feature closure | `svg`, `layout-cytoscape`, `layout-elk`, `math`, `complete-svg` |
| Executable | `target/release/examples/layout_work_calibration` |
| Executable SHA-256 | `5f991109b1190238f23241353a77aacb4f0f25a4ce036c7b3a57e51c0c93be51` |
| Calibration source SHA-256 | `df4e2145a4b13d87456599ca86b77561fd87cf7e89bc9245d489a46fc44b13d5` |
| `crates/merman/Cargo.toml` SHA-256 | `0b7687ad2a989792fcbe78eab7bf95d7d63018b460ba85c097e8235718cb2959` |
| `Cargo.lock` SHA-256 | `4eb38cee29796405570fa3172fffd049429fdef5692f868fd6be02e45ee93a71` |
| Corpus manifest SHA-256 | `cb7cca600d3980e77180832bff599f7f79608c4a41ff1116a2d4f4fd8cf52e02` |
| Rust | `rustc 1.95.0 (59807616e 2026-04-14)` |
| Cargo | `cargo 1.95.0 (f2d3ce0bd 2026-03-21)` |

The probe fails closed when tracked files are dirty, when any owned input is untracked, when the
post-run source snapshot changes, or when the enabled feature set differs from the exact closure
above. Pre-existing unrelated untracked paths were excluded from provenance and evidence. Host
filesystem timestamps that appeared later than the authoritative date were not used to establish
revision order or evidence chronology.

## Host and process envelope

| Field | Value |
|---|---|
| Host | Apple M4 Pro, `Mac16,7`, 48 GiB physical memory |
| Target | `aarch64-apple-darwin` (`macos`, `aarch64`) |
| Outer timeout | 300 seconds per fresh process |
| Fresh processes | 5 |
| Exit status | 0 for all 5 processes |
| Elapsed range | 1.4811-1.5558 seconds |
| Elapsed median | 1.5040 seconds |
| Peak RSS range | 60,866,560-62,603,264 bytes |
| Maximum peak RSS | 62,603,264 bytes (59.70 MiB) |
| Report size | 36,425 bytes |
| Report SHA-256 | `1dc0e68e1b01c15b2ee72029e0b7669e6b59973a3f4c6cdb4916c873acedf84c` |

All five fresh processes produced byte-identical reports. The elapsed and RSS values establish the
calibration envelope only; they are not an admitted speedup, regression, memory bound, or host SLO.

## Public fixture corpus

The probe reads the closed schema-2 fixture list from `tools/bench/corpus.json`; it does not scan
arbitrary `.mmd` files. The 68 selected fixture members have aggregate provenance digest
`54a5e69af77e60fd545018db53336b1dd1f9c3d6a229aa7a47e4090e6060180c`.

The maximum observed fixture was:

| Field | Value |
|---|---|
| Fixture | `flowchart_large` |
| Source | `crates/merman/benches/fixtures/flowchart_large.mmd` |
| Source bytes | 6,419 |
| Source SHA-256 | `edba73dc4c0a12417e052ec3c98ada82b5d773c4346b5c735df5dfa354f5b161` |
| Layout-work units | 697,752 |
| SVG bytes | 307,649 |
| SVG elements | 2,902 |
| SVG SHA-256 | `1a43732507bd36e0c26776bc7a768cb5400a70bb429acfeb47d1077f57a315fe` |
| Default-profile headroom | 102,248 units (14.6539%) |

Every selected fixture completed under the default `interactive` policy with no explicit resource
override.

## Exact fixture threshold

The maximum fixture was rerun with explicit adjacent ceilings:

- `max_layout_work_units = 697,752` succeeded, reported exactly 697,752 units, and produced the
  same SVG hash as the default-policy render.
- `max_layout_work_units = 697,751` rejected with structured
  `cause=ceiling`, `phase=layout_model`, `limit=max_layout_work_units`, `actual=697752`,
  `max=697751`, `profile=interactive`, and the exact explicit override in the error payload.

This proves the accounting threshold for the corpus maximum and guards against a report-only or
off-by-one calibration.

## First rejection on a monotonic Architecture curve

The boundary lane uses a deterministic 32-service, 31-edge Architecture graph with
`randomize=false` and `seed=1`. It varies only integer `numIter`. Above FCoSE's `5 * nodes = 160`
minimum, admission work for this fixed graph shape is a strictly increasing positive linear
function of configured iterations. Binary search is accepted only after checking the adjacent
successful value and two consecutive structured rejections.

| `numIter` | Result | Evidence |
|---:|---|---|
| 3,140 | Accepted | Source SHA-256 `1166a9ce9099394bb079cdb1921748eae6379f0c8017dd1e517b728141d35324`; SVG 98,851 bytes / 1,252 elements; SVG SHA-256 `e5a3fd3a1d45bfe8bccaafadd6a222633ac15132fb90ab3590b804878cc9a23b`. |
| 3,141 | Rejected | Source SHA-256 `9e74d83c995413035337bdb34c4908a16e9a4f981d94d1daad9a6e8983eda756`; `cause=ceiling`, `phase=layout_model`, `actual=800131`, `max=800000`, no explicit overrides. |
| 3,142 | Rejected | Source SHA-256 `65918c9348ac0c4ac6630f85e4acce2edf855d507dbc52eddc46a4739960e611`; `cause=ceiling`, `phase=layout_model`, `actual=800385`, `max=800000`, no explicit overrides. |

The accepted render's final session report contains 48,589 consumed work units because the
800,000-unit admission calculation is a checked, non-consuming upper-bound preflight. The typed
rejections therefore validate the admission boundary; the lower successful consumption value is
not a contradiction and must not be used as a replacement ceiling.

## Reproduction

Build the exact feature closure:

```sh
CARGO_BUILD_JOBS=1 cargo build --locked --release -p merman \
  --example layout_work_calibration --features complete-svg
```

Run each sample in a fresh process under a 300-second outer timeout and capture `/usr/bin/time -l`:

```sh
/usr/bin/time -l target/release/examples/layout_work_calibration \
  --authoritative-date 2026-08-07 \
  --corpus tools/bench/corpus.json \
  --json-out target/bench/layout-work-calibration-2026-08-07.json \
  --expected-max-fixture flowchart_large \
  --boundary-max-iterations 65536
```

The command exited 0 in every recorded process. A prior development-profile exploratory run was
used only to debug the probe and was excluded from this decision.

## Scope

The pinned Mermaid implementation has no corresponding public `max_layout_work_units` value; this
is a Merman host-safety policy, not a Mermaid semantic constant. Valid accepted renders continue to
follow the pinned Mermaid behavior graph. The `constrained` profile remains at 125,000 units. Parse,
SVG-byte, raster, and output-cardinality limits were not changed by this calibration.
