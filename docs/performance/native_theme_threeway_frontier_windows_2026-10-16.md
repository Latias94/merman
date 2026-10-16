# Latest native theme three-way frontier on Windows, 2026-10-16

The latest frozen optimized candidate recovers part of the merged-theme latency gap in this
three-round diagnostic, but remains substantially slower than v0.9.0. No calibrated latency or
non-regression admission follows. Additional source review and an exact allocation probe identify
Class preparation's duplicated default-style ownership as a separate measurable memory frontier.

## Frozen comparison boundary

The run spans October 15-16 on Windows 11, Intel Core i7-11700, Rust 1.95.0,
x86_64-pc-windows-msvc. Native bench artifacts use default features plus SVG, strict parsing,
deterministic measurement, trusted-native resources and a reused renderer. Historical locked
artifacts are reused with their original provenance, not relabeled as fresh clean-ref builds.

- v0.9.0: `f754a3c3077d35d2a372c2fec04f9e9b87db1c3f`.
- Merged themes: `b9844db7b747111690bf6369d4536885a85e35a7`.
- Optimized: `ad3d5f07bcb5dd6ee6d1cf244727835341193c68` plus the uncommitted policy-borrow
  patch `c8c68182779d12661bb4552d3496d80e5918c6e88699e60fd1899e1ac2811764`.

Seven registered fixtures are discovered using all three executables. Class medium, Journey,
Class tiny and Kanban have exactly equal SVG identities. XY Chart, Flowchart and Sequence have
historical output differences and remain documented exclusions from same-output timing. Selected
harness, corpus and fixture hashes match. The merged and optimized lockfiles match; v0.9.0 has a
different product dependency set, recorded explicitly rather than treated as an accidental mismatch.

## Public source-to-SVG diagnostic

Three side orders put each revision once in each position: v0.9.0/merged/optimized,
optimized/v0.9.0/merged, merged/optimized/v0.9.0. Each invocation uses 30 Criterion samples,
two-second warmup and three-second measurement. Values are arithmetic means of three independent
console estimates in microseconds. All 36 invocations preserve selected pre/postflight output,
frozen executable and discovery identities. All 72 compiler-boundary snapshots are clear.
These snapshots do not establish continuous observation or host isolation.

| Fixture | v0.9.0 (us) | Merged (us) | Optimized (us) | Optimized / merged | Optimized / v0.9.0 |
| --- | ---: | ---: | ---: | ---: | ---: |
| class_medium | 1893.767 | 2875.667 | 2783.733 | -3.20% | +46.99% |
| journey_medium | 70.554 | 265.917 | 231.673 | -12.88% | +228.36% |
| class_tiny | 383.110 | 668.130 | 608.923 | -8.86% | +58.94% |
| kanban_medium | 102.132 | 350.957 | 290.537 | -17.22% | +184.47% |

This is one complete frozen diagnostic, not a chained estimate from earlier experiments. There is
no A/A calibration or fixed power-sized confirmation here. The ordinary joint 10% and 50 us gate
is unchanged. Do not append samples or interpret favorable means as confirmed improvement.

## Emission-stage rejection

A separately registered `render` stage excludes preparation/layout computation, standalone XML
validation and facade sealing; it includes consuming/destructing the prepared artifact and output.
The first attempt observes multiple Cargo/rustc/link processes after the second-round merged
Journey invocation and stops with 17 complete rows. One fresh retry observes compilation after the
first-round merged Journey invocation and stops with four rows. Both entire stage attempts are
rejected, have no summary and are not spliced. No stage CPU attribution is drawn from these rows.
No further retries are scheduled; the independently completed end-to-end run remains separate.

## Exact Class preparation allocation probe

One standalone probe source links to the existing v0.9.0 and current optimized renderer libraries
and the unchanged checked-in native-memory counting allocator. No Cargo rebuild or production
source modification is involved. It measures only public `family::prepare`: source parsing,
session construction and input cloning precede counting; the prepared result remains alive at
snapshot completion; SVG rendering, SHA-256, identity assertions and result destruction follow.
Net retained growth includes any consumed-input deallocation during prepare. It is not process
RSS or the entire operation's memory. The boundary includes layout, plans and completion state,
so the complete version difference cannot be attributed solely to four style strings.

Generated default-HTML Class inputs use 16/64 nodes with no inline style, `useGradient:false` and
`nodeBorder = "rgb(" + K ASCII spaces + "1,2,3)"`, K=0/1024/4096. Every source is below 16 KiB.
Two warmups precede three allocation observations per shape. All observations are identical for
each shape/side; all six cross-version SVG byte counts and hashes match exactly.

| Nodes | Extra token bytes K | v0.9.0 net retained bytes | Current net retained bytes |
| ---: | ---: | ---: | ---: |
| 16 | 0 | 6793 | 22302 |
| 16 | 1024 | 6793 | 42782 |
| 16 | 4096 | 6793 | 104222 |
| 64 | 0 | 25081 | 73566 |
| 64 | 1024 | 25081 | 143198 |
| 64 | 4096 | 25081 | 352094 |

For each node count C, increasing K leaves the measured old preparation allocation/retention
unchanged, while current allocated and retained bytes both rise exactly `(C + 4) * K`. The
node-dependent coefficient matches the independently identified per-node owned default stroke
token; the additional coefficient of four is C-independent and is not attributed to a specific
owner by this probe. This is measurable added pre-emission
ownership. The SVG itself already grows with C times token bytes, so this is not evidence of a new
overall time-complexity order or a demonstrated resource-policy vulnerability. No timing is taken
from this instrumented probe.

## Other-owner review

Three independent read-only lanes inspect default preparation, SVG emission and facade sealing
against both historical revisions. Findings are separated by actual default-path reachability:

- **Default Class preparation:** `class/node_binding.rs:209-216,247-255` owns fill/stroke/width/dash
  per node, even when the values come from shared defaults. `family/preparation.rs:1223` builds
  this plan with either HTML or SVG labels. v0.9.0 borrows those defaults during emission. This
  is the highest-priority newly measured ownership frontier, not a proven CPU percentage.
- **Default Class edges:** paths and labels in `svg/parity/class/edge.rs:184,545` each collect and
  sort the same edges; v0.9.0 shares one ordered sequence. Center-map IDs now own Strings, and
  `terminal_style` clones even without added line-hop styling. Default Class medium has 14 labeled
  relations. The old implementation also used two temporary SVG buffers that the new code removes;
  any repair must measure net effect and preserve the newer streaming design.
- **Default HTML Class titles:** `svg/parity/class/node.rs:1194` creates an owned normalized style
  even when no inherited fill is appended. The default medium fixture has 15 nonempty title styles.
  A borrowed normalized slice can preserve the no-append case; empty member styles are not counted
  as heap allocations.
- **Explicit SVG-label Class only:** member/method fallback checking and `all_runs_normal` analyze
  the same Markdown twice. Default Class uses HTML labels, so this does not explain this cohort.
  Sharing the pure analysis is distinct from moving measurement: routed host calls carry Emit
  cancellation and receipts and must not be removed or shifted into Layout. Wrapping can change
  the analyzed text and needs its actual normalized final input.
- **Low priority:** one redundant default Engine clone only increments Arc-owned config/registry
  references. CompletionOnly remains necessary; its seen-slot/sticky/unique-seal repair must not
  be disabled to avoid the cost of plans. Existing measurement routing and default scoped-theme
  materialization are not newly introduced large-copy owners.

No additional large default Journey producer or facade metadata owner is found. Report clone
fields are fixed-slot or Arc-backed; default theme reports and coverage mechanisms are empty.
The extra artifact/document/target/admission metadata digests stream typed metadata and existing
hashes, rather than repeatedly serialize full SVG. The two complete SVG digests bind different
contracts and remain necessary.

## Dependency exclusions and next repair boundary

The historical lockfile adds theme workspace crates and `serde_json_canonicalizer` 0.3.2 and
updates rustls 0.23.45 to 0.23.46. Merged and optimized locks are identical. Exact current pipeline
compiler-artifact records contain no rustls. Canonicalizer is compiled, but its callers are explicit
authoring/spec/support APIs; default SVG requests do not call it. Renderer recipe fingerprints
use their own encoder and are frozen when compiling a theme. Lockfile presence alone does not
attribute default rendering time to either dependency.

The next Class experiment should make a prepared binding select its shared defaults or own an
actual override. Resolve those selections against the artifact-owned CSS binding at consumption;
do not create self-references into sibling artifact fields or deduplicate authority by equal CSS
values. Preserve typed/source priority, same-value distinct provenance, completion, error/resource
accounting and cancellation. Measure disappearance of node-scaled default payload and test both
HTML/SVG labels and explicit overrides. Shared edge ordering and borrow-until-append style emission
are separate causal changes. The larger generated-XML/final-validation frontier remains open.

## Local evidence and state

All sources, commands, hashes, discovery/recipe receipts, 36 end-to-end observations, rejected
render attempts, and allocation probe sources/results/build receipts remain under
`target/bench/experiments/theme-threeway-frontier-20261015/`. The date in that directory is the
experiment's start date. `class-prepare-builds.json` records optimized direct-rustc commands and
rlib/executable hashes; `class-prepare-summary.json` contains all metrics and identity comparisons.
This checkpoint adds no production change, no commit and no push. The preceding policy-borrow
candidate and earlier performance-document changes remain untouched. No new release qualification,
whole-corpus latency or whole-operation memory claim is made.

Independent review verifies probe/executable/rlib hashes, all raw observations and the allocator's
counter boundaries. `class-prepare-review.json` also captures the unchanged allocator SHA-256 at
review time; that hash supplements the build receipt and was not captured during compilation.
