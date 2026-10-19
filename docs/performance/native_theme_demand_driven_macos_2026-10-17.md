# Demand-driven theme rendering on macOS

This checkpoint continues `9a297b72f` against the frozen native implementation at
`9d525625f56116d5b5d1e662a907c5360688ab1c`. It removes representation and certification work whose
only consumer was another copy of the same plan. Actual theme precedence, visible application
reports, source ownership, XML serialization, resource ceilings and cancellation remain owned by
the executing renderer.

The primary result is measured allocation removal with identical output. Latency observations
are a separate lane and require the calibration contract before they can support a confirmed
speedup. The historical [v0.9.0 comparison](native_v0_9_cross_family_macos_2026-10-16.md) retains its
output-changing rows and does not become a causal baseline for this refactor.

## Implementation

- Packet reports successful visible text writes instead of prewalking expected roles, copying
  fonts/fills and comparing emitted strings with their own plan. The family owner constructs parse
  evidence once when an explicit request needs compatibility outcomes.
- Architecture removes typography inventories and copied group/edge/surface evidence. The actual
  writer supplies requested outcomes. Native layout typography remains distinct from SVG label
  typography, and the existing direct static-paint helpers own selector/winner interpretation.
- Kanban removes CSS reparsing and layout/label certification strings. Default group styling is
  borrowed; actual prepared measurements, inherited runs and HTML/Markdown/source ownership remain.
- State replaces selected-path provenance lists with the actual source-owned bit. Ordinary leaves
  no longer construct a second typography decision. Composite labels/headers share one decision;
  note groups retain their independently consumed class-only group typography.
- Fixed environment defaults use the established catalog fingerprint bytes, finite font-source
  slices, compact capability sets and one immutable default resource ceiling. Work meters,
  deadlines and cancellation remain operation-local.
- The shared XML writer scans valid UTF-8 bytes, preserving XML 1.0 scalar filtering and cross-fragment
  `]]>` escaping. Scanned and retained work share one cancellation cadence; byte ceilings still
  check every growing append before allocation. `finish` now observes cancellation before success.
- ER lowers one source/baseline text visual per entity at the selected HTML/SVG consumer instead of
  storing four dual-mode cell visuals per attribute. Real typed differences stay in the existing
  typed selections. Mindmap removes default edge vectors and absent-theme outcome walks while
  preserving typed Node.stroke inheritance and source-owned edges.

Block source indexing is deferred. Its layout, style and writer passes have distinct real consumers;
sharing them would change lifetimes across those owners. This checkpoint has no measurement showing
that redesign is justified, and retains no speculative cache or second rendering path.

## Allocation evidence

The probe reuses the unchanged counting allocator from
`crates/merman/benches/native_memory/allocator.rs`; the probe itself remains under the ignored
experiment directory. Base and candidate use the same probe, locked default+SVG native build,
strict deterministic rendering, trusted-native policy, reused renderer and fresh request. Request
construction intentionally keeps the original benchmark's explicit environment plus struct-default
construction. After one warmup, each row records five single-operation counts. The returned SVG is
retained at the measurement boundary. These are allocation counts, cumulative allocated bytes and
peak live growth, not RSS or heap retained after dropping the result.

All 36 successful allocation inputs have byte-identical SVGs and clean allocator counters. Values
below are the medians of five counts; KiB uses 1,024 bytes.

| Input | Base allocations | Candidate allocations | Removed allocations | Removed cumulative bytes |
| --- | ---: | ---: | ---: | ---: |
| Packet medium | 172 | 135 | 37 | 5,244 |
| Architecture medium | 769 | 743 | 26 | 4,496 |
| Kanban medium | 714 | 671 | 43 | 5,098 |
| State medium | 8,182 | 8,024 | 158 | 10,765 |
| ER medium | 7,023 | 6,877 | 146 | 16,748 |
| Mindmap medium | 3,584 | 3,561 | 23 | 5,596 |
| Timeline medium | 864 | 844 | 20 | 4,012 |
| Class medium control | 11,993 | 11,973 | 20 | 4,012 |
| Flowchart large control | 102,607 | 102,587 | 20 | 4,012 |

The shared 20-allocation reduction appears in unrelated controls too. This supports removal of
fixed request/default work rather than assigning the entire delta to a family adapter.

The ER curve independently varies 1/4 entities, 1/16/64 attributes, 4/256-character font tokens and
HTML/SVG labels. With one entity, removed allocations increase from 34 to 156 to 542 as attribute
count increases; with four entities they increase from 76 to 564 to 2,108. The largest row removes
287,660 cumulative bytes and 164,596 bytes of peak growth. Font-token and label-mode controls retain
the same allocation deltas; no token-length-dependent saving is claimed.

The Mindmap curve uses 2/16/64 nodes and removes 4,252/5,596/10,972 cumulative bytes. The 256-node
baseline reaches its existing `max_layout_work_units` ceiling (15,004,444 attempted against
15,000,000); that failure is preserved separately and is not counted as a successful or faster render.

## Environment lifecycle controls

A separate probe counts five fresh-process first constructions and five subsequent constructions
per process. The first `SvgEnvironment::deterministic()` moves from 23 allocations / 2,460 bytes to
15 / 2,208; subsequent constructions move from 23 / 2,460 to 14 / 456. The shared immutable default
policy therefore has a one-time 1,752-byte construction cost. No cold-start latency claim follows.
All five processes agree, and their public environment Debug identities match the baseline.

A preconstructed-request lane clones the existing request for each operation instead of recreating
its environment. After one warmup it records five counts for Packet, Kanban, Timeline, Class and
large Flowchart. SVGs match the corresponding fresh-request lane exactly. Packet removes 25
allocations, Kanban 31, and Timeline/Class/large Flowchart each remove 8. The gain is therefore not
limited to the original benchmark's overwritten struct-default construction.

Every process verifies an empty measured region has zero allocations and clean counters. Initial
supplementary linking detected identical base/head library hashes from the shared Cargo target and
was rejected before collecting results. The accepted controls clean only the selected packages'
release artifacts, rebuild each checkout, then immediately link/freeze/collect before switching
revisions. Both library hashes and both executable hashes differ; the probe source is identical.
The earlier 36-row allocation run froze its baseline probe immediately after its original fresh
build, and the 76-input latency run uses separately frozen executables; neither uses the rejected
supplementary links.

## Same-output latency diagnostics

The original, unchanged `pipeline` benchmark compares the frozen native base and candidate with
30 Criterion samples, two-second warmup and three-second measurement per invocation. Two AB/BA
pairs per input produce 304 invocations across all 76 inputs. Every invocation repeats its exact
preflight identity and verifies postflight output. Both immutable executable digests and the
candidate source fingerprint remain unchanged through completion.

These are adjacent-revision diagnostic point estimates. Independent A/A calibration and power-sized
confirmation were not collected, so these rows do not qualify as confirmed speedups or confirmed
non-regression. The accepted claim is the allocation/work deletion with preserved behavior.

| Input | Base (us) | Candidate (us) | Diagnostic change |
| --- | ---: | ---: | ---: |
| packet_medium | 14.784 | 12.357 | -16.42% |
| kanban_medium | 78.046 | 70.005 | -10.30% |
| timeline_medium | 62.496 | 54.958 | -12.06% |
| architecture_medium | 52.801 | 50.929 | -3.55% |
| state_medium | 454.000 | 436.690 | -3.81% |
| er_medium | 423.420 | 414.460 | -2.12% |
| mindmap_medium | 285.160 | 269.665 | -5.43% |
| class_medium | 689.340 | 679.140 | -1.48% |
| flowchart_large | 4651.800 | 4567.150 | -1.82% |

75 point estimates decrease. The single positive row, `sequence_block_repeat_high`, moves from
1,581.700 to 1,586.300 us (+4.600 us, +0.29%). No diagnostic row reaches the ordinary joint
10%-and-50-us regression screen. These observations do not establish statistical admission.
There is no minimum speedup requirement for keeping the proven allocation deletion.

## Verification

- Final native pipeline discovery: all 76 SVG inputs across 38 families match the frozen base bytes.
- Full native facade/renderer/export/bindings/theme integration: 6,232 passed, 13 existing skips.
- Private theme acceptance configuration: 34 passed.
- WASM host: 47 passed; Node native wire: 21 passed.
- Node JavaScript: 115 passed with the repository-required npm 12. The default npm's packaging
  output failed the existing npm-12 contract before that toolchain correction.
- Web JavaScript: 149 passed; rebuilt optimized full WASM runtime renders 37 diagram families and
  passes the runtime and DOM-safety smokes. Its final build input snapshot is verified again
  after the comment/test-name cleanup.
- All-family DOM comparison, all-target Clippy with warnings denied, formatting and the no-Packet
  feature closure pass. DOM's existing explicit parser/browser residuals remain diagnostic residuals.
- Eight independent code-review lenses found no retained defects.
- Performance comparator contracts: 124 passed. The opt-in low-latency mode now requires zero in
  every base/head identity/order interval and applies the documented simultaneous noise gate before
  scheduling fresh confirmation. Generic equivalence calibration retains its existing contract.

An exhaustive Unicode-scalar serializer characterization passed before the writer change and after
it, including fragment boundaries and discarded XML scalars. A final-cancellation regression test
failed on the original writer and passes after the change. No tracing or profiling dependency was
added. Frozen render-stage samples were collected with the existing native executable and macOS
sampling before selecting the writer change.

## Integration

The renderer crate removes 2,677 net lines, including superseded implementation-only tests. The
implementation and benchmark correction are split into these local commits:

- `b69029fef` perf(render): reuse immutable environment defaults
- `66c97d929` refactor(theme): derive packet and architecture outcomes from SVG writes
- `d7aa48214` refactor(kanban): remove redundant theme certification
- `c030ed486` refactor(state): share consumed typography and source ownership
- `4406de6cc` refactor(er): lower entity text visuals at the consumer
- `7ca93a4b0` refactor(mindmap): remove unused default edge state
- `a328c321c` perf(svg): consolidate XML serialization and cancellation checks
- `17e2b0e1a` fix(bench): enforce low-latency calibration noise gates

## Evidence location and limits

Raw source fingerprints, binaries, allocation samples, output identities, validation logs and
registered benchmark schedules remain at
`target/bench/experiments/demand-driven-theme-20261016/`. The host is Apple M4 Pro on macOS,
aarch64, Rust 1.95.0. Cargo builds are serial; timing excludes builds, tests, allocation counters and
profiling. Native, browser-WASM and Node evidence remain separate lanes. This checkpoint does not
claim a release-range recovery percentage, a cold-start speedup, an RSS reduction or a portable
performance guarantee.

The only production-file edit after timing is a two-line resource-budget documentation correction;
an existing Kanban test is also renamed to describe actual property application. Exact patches and
before/after fingerprints are retained in `post-measurement-edits.json`. Executable production code
matches the measured candidate.
