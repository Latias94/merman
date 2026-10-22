# Semantic boundary: historical U3/U4 adjacent work evidence

This checkpoint supplies the existing adjacent-work evidence requested by U9 of the
[semantic capability plan](../plans/2026-10-20-1252-refactor-semantic-capability-boundaries-plan.md).
It records historical work removal, allocation observations, correctness controls and unresolved
timing controls. It does not admit U3 performance, complete U4, or close the U1-U9 plan.

The audit on 2026-10-22 used read-only Python JSON, SHA-256 and arithmetic checks, plus local Git
objects. No Cargo command, executable invocation, new measurement, threshold change or receipt
rewrite was performed. The checkout was already dirty; its observed HEAD was
`43deb2860e787d0290dc31bcfc6b1f07316ec278`. Neither that revision nor the live working tree is
the measured candidate in the tables below.

## Revision and measurement boundaries

The [U3 preregistration](semantic_editor_demand_preregistration_2026-10-20.md), committed in
`755ff3739a536204760321b2e2afac9b1d703b68`, fixes the allocation caps and required controls.
Its initial post-U2 source was `3f0f3819a6013ed6882205cf80ea4aa1336b1a83`. Later source-backed
Flowchart corrections required a corrected comparison; earlier diagnostic or superseded
baselines must not be substituted for the final pair.

| Evidence | Historical baseline | Historical candidate | Scope |
| --- | --- | --- | --- |
| U3 final allocation confirmation | `d77d876407f35e6b224d556747ed91c89e7180d8` | `892e00e4708395845cc48bc239064f7de1dff167` | Two clean Cargo-built native memory batches, seven lanes each |
| U3 post-PR226 source scope check | `a7142d3a4a03a0d2fff144de2cc4972bd76fd32d` | `4ee66d7caa428658e8cb5a70f571da2c776fc746` | Source comparison only; no new allocation measurement |
| U3 public timing confirmation | `a7142d3a4a03a0d2fff144de2cc4972bd76fd32d` | `4ee66d7caa428658e8cb5a70f571da2c776fc746` | Four separately sampled groups, six row identities per group |
| U4 adjacent allocation confirmation | `f676075658a86769fd2d55fdc1fbc42803c7707c` | `78c0625d14ebfa447d2320d543ca08744bd86ab2` | Separate clean post-PR227 baseline/candidate, seven lanes each |
| U4 State SVG comparison | Clean `f676075658a86769fd2d55fdc1fbc42803c7707c` | Then-uncommitted U4 consuming implementation | Debug correctness/output observation before the final candidate commit; not a clean committed timing measurement |

All named source commits resolve in local Git. The U3 source scope check compares 15 declared
memory inputs on each side before/after PR226: all 30 file comparisons are byte-identical.
The core patch between the original pair is also byte-identical to the core patch between the
post-PR226 pair. Both `git diff <base> <candidate> -- crates/merman-core` byte streams hash to
`b7a4c6e7d1595a36e18c65d872e291dee60b0a1f2f096cca7c6847ed95edb03a`.
This preserves causal attribution of the historical U3 change. Common appearance optimizations
can still change absolute allocations; the source check does not transfer measured absolute
values to PR226, PR227 or today's integrated implementation.

The U4 pair includes the common post-PR227 state and probe characterization
`f43d6f75db782bd4941d8fcaeb3b120b124eace8` on both sides. Its complete source delta contains
only `crates/merman-core/src/diagrams/state/db.rs` and
`crates/merman-core/src/diagrams/state/tests.rs`. All 18 preregistered input hashes match the
baseline Git blobs. The other 16 inputs, including the probe, allocator, corpus, drivers,
manifests, lockfile and seven owner contracts, are unchanged in the candidate.

The U3 allocation receipt was generated at `2026-10-20T13:43:44.063889+00:00`; its source scope
check at `2026-10-20T16:40:55.068556+00:00`. The U3 timing receipt was frozen at
`2026-10-20T17:29:58.809610+00:00`. The U4 allocation receipt was created at
`2026-10-20T19:20:14.910266+00:00`, which is October 21 in the host's UTC+08:00 time zone.
Receipt timestamps and source identities remain historical.

## U3: deletion of actual discarded editor work

Production commit `358b2f2c892c74fde6117e4a2f2b5b363789396b` passes private
`EditorFactsDemand::Omit/Capture` into Sequence, State, Class, ER and Pie construction.
The omitted route bypasses the editor collector and unused recovery-fact enrichment. Requested
capture still constructs the same semantic result, complete/recovered facts and original spans.
The required lexical tape, grammar construction, DB replay, validation, resource work and outer
cancellation/error precedence remain.

The evidence observes real collector entry and actual symbol, directive, diagnostic and
expected-syntax appends. At the historical candidate, counters live at the collector entry and
`EditorSemanticFacts` mutation sites in `crates/merman-core/src/editor.rs`; they are not a flag
asserting that an optimization branch ran. Append counts can exceed final vector lengths
because finalization can deduplicate items.

| Ordinary route in the original red proof | Collections | Symbol appends | Directive appends | Diagnostic appends | Expected-syntax appends | Omitted-route target |
| --- | ---: | ---: | ---: | ---: | ---: | --- |
| Sequence example | 1 | 6 | 0 | 0 | 2 | Zero for every counter |
| State example | 1 | 9 | 2 | 0 | 12 | Zero for every counter |
| Class/ER/Pie detected/known-type, JSON/typed aggregate | Seven demand tests include all twelve route combinations | Actual append sites | Actual append sites | Actual append sites | Actual append sites | Zero discarded editor work |

The recorded nextest run `9c6480a7-ee74-4377-b9f5-45c7fea8748a` failed all seven demand tests
against old production. The initial candidate run passed 49 focused tests, including requested
capture and one-construction checks. The later State streaming change
`0b6b4bf8e3c95a9aa4c8cb62cde3b69dfb8135e9` removed the private `StateEditorEvent` vector and
drain, appending directly at existing collection/flush points with bounded terminal checks.
Its two deadline tests first failed, then the historical core/lib, projection, deep-lifecycle
and snapshot run passed 1,996/1,996 tests.

The earlier `u3-core-contracts.log` remains a failed 1,980/1,981 run: its golden-snapshot failure
exposed a Flowchart keyword-boundary defect. That result and superseded baselines are retained,
not relabeled as passes. The final allocation pair includes the corrected source.

For lexical events E and emitted editor facts F, this removes the extra collector traversal and
O(F) discarded materialization on omitted requests. State capture also removes an intermediate
event representation. This is bounded work-removal evidence, not a whole-parser complexity
proof or a latency result. In particular, the State snapshot allocation reduction below cannot
be described as omitting requested facts.

## Allocation protocol and historical results

These observations use Windows 11, x86_64 MSVC, Rust 1.95.0, locked dependencies, bench profile
and the native System allocator. The memory recipe disables default features, selects
`all-diagrams`, uses one Cargo job and disables incremental compilation and benchmark debug
information. It measures reused-engine operations in fresh subprocesses, with results alive
through peak/retained checkpoints and dropped before the final checkpoint.

Each lane uses scales 1, 2, 4, 10, 32 and 100, five operation/zero pairs at each scale,
alternating order and 10,000 deterministic bootstrap resamples. Thus each lane has 30 pairs
and 60 subprocess observations. Numbers below are operation-minus-zero medians at 100x;
byte metrics are bytes, allocation counts are counts, and arrows mean baseline to candidate.
They are not RSS, current integrated absolute allocations, or cross-host expectations.

### U3 final corrected pair

| Lane | Allocation count | Allocated bytes | Peak growth bytes | Retained growth bytes |
| --- | ---: | ---: | ---: | ---: |
| State typed | 12,490 -> 10,953 | 1,626,029 -> 1,319,235 | 687,654 -> 615,279 | 510,328 -> 510,328 |
| State JSON | 22,757 -> 21,220 | 3,108,667 -> 2,801,873 | 1,500,595 -> 1,428,220 | 1,071,875 -> 1,071,875 |
| Sequence typed | 3,516 -> 2,593 | 728,741 -> 621,339 | 353,713 -> 294,443 | 192,611 -> 192,611 |
| Sequence JSON | 4,861 -> 3,938 | 827,843 -> 720,441 | 355,851 -> 296,581 | 268,161 -> 268,161 |
| State requested snapshot | 22,844 -> 22,835 | 3,200,859 -> 3,037,339 | 1,500,595 -> 1,500,595 | 1,143,130 -> 1,143,130 |
| Sequence requested snapshot | 4,887 -> 4,887 | 864,555 -> 864,555 | 358,587 -> 358,587 | 308,279 -> 308,279 |
| Flowchart typed control | 7,749 -> 7,749 | 2,076,117 -> 2,076,117 | 912,176 -> 912,176 | 263,262 -> 263,262 |

The four ordinary lanes strictly reduce count and bytes. Their peak/retained metrics do not
increase. Requested snapshots and the unaffected Flowchart control meet the original bounds.
All seven historical candidate reports are `candidate-bound`, `candidate_admission: true`,
`outcome: pass`; the baseline reports remain observations rather than candidate admissions.
The seven owner contracts retain the preregistered absolute and finite-workload log-slope caps
of 1.0. The observed baseline values above do not replace those caps, including the four
ordinary count/byte caps that are one below these corrected baseline observations.

The read-only audit matched all 14 report hashes and all 14 manifest hashes to the receipt,
checked the 14 source/max-scale records against the reports, and rehashed the two preserved
probe executables through their report paths:

| U3 preserved probe | SHA-256 |
| --- | --- |
| Baseline | `5abf3a3920c51cebb355718aa2154af53d5329479ae40116aa46de730627c70f` |
| Candidate | `275c629b7280d6ea208e063b39620c71b2097d0039898a56c586278100885db8` |

### U4 separate adjacent pair

U4 preregisters strict 100x allocated-byte reduction in all three State routes. At every
registered scale, all four metrics in all seven lanes must have non-increasing
operation-minus-zero medians, with the existing U3 caps retained as independent checks.
The preregistration amendment changes only baseline/candidate target directories to preserve
both executables across the driver's bench-profile resets. It changes no metric, threshold,
sample budget, capability or statistical rule.

| Lane | Allocation count | Allocated bytes | Peak growth bytes | Retained growth bytes |
| --- | ---: | ---: | ---: | ---: |
| State typed | 10,953 -> 10,249 | 1,319,235 -> 1,310,627 | 615,279 -> 615,279 | 510,328 -> 510,328 |
| State JSON | 21,220 -> 20,516 | 2,801,873 -> 2,793,265 | 1,428,220 -> 1,428,220 | 1,071,875 -> 1,071,875 |
| State requested snapshot | 22,835 -> 22,131 | 3,037,339 -> 3,028,731 | 1,500,595 -> 1,500,595 | 1,143,130 -> 1,143,130 |
| Sequence typed control | 2,589 -> 2,589 | 621,291 -> 621,291 | 294,443 -> 294,443 | 192,611 -> 192,611 |
| Sequence JSON control | 3,934 -> 3,934 | 720,393 -> 720,393 | 296,581 -> 296,581 | 268,161 -> 268,161 |
| Sequence snapshot control | 4,883 -> 4,883 | 864,507 -> 864,507 | 358,587 -> 358,587 | 308,279 -> 308,279 |
| Flowchart typed control | 7,745 -> 7,745 | 2,076,063 -> 2,076,063 | 912,176 -> 912,176 | 263,262 -> 263,262 |

Each State route saves exactly 704 allocations and 8,608 allocated bytes at 100x against its
own adjacent baseline. All 168 cells (seven lanes x four metrics x six scales) match the
original report adjustments, medians and deltas and are non-increasing. All five repetitions
within each cell are identical on each side. All 96 cross-family control cells are exactly
unchanged. This observation is limited to the preregistered generated inputs, including their
String capacities; it is not an arbitrary-input memory bound.

All 14 U4 report and 14 manifest hashes match their freeze receipts; both freeze receipt
hashes match the adjacent-allocation receipt. The clean source identities and successful
candidate-bound report outcomes match as well. Both preserved probe executables rehash correctly:

| U4 preserved probe | SHA-256 |
| --- | --- |
| Baseline | `e8e4cfa25d550cf59240bd38ff5ad0a9997987d1fa5577e02d54a1089d457c34` |
| Candidate | `88445e3a10e10bf7eb7527892d8670dfd415642671c529aa7c0f943f613572a2` |

Different U3 and U4 control values are preserved as measured. U4 savings are not calculated
by subtracting the old pre-U3 baseline, and the two experiments are not added together to
predict a current allocation total.

## U4: real State buffer transfer and output controls

The consuming DB projection destructures its owner and moves nonempty String payloads for
state records, descriptions, notes, relations, styles, links and accessibility metadata.
The root-document lookup borrows identifiers instead of cloning them. Required flat documents,
graph nodes/edges, ordering and first-root-document selection remain. Selected `Vec<String>`
fields are compacted with `into_boxed_slice().into_vec()` to avoid retaining unused capacity;
outer vector storage need not survive compaction, while the owned String buffers are moved.
Typed parse still requires zero nested compatibility-document projections.

The two proof tests in historical `state/db.rs` distinguish no-slack buffer transfer from
slack compaction without copying strings:

- `state_consuming_projection_moves_owned_payloads_without_copying`
- `state_consuming_projection_compacts_vec_slack_without_copying_strings`

The actual old owner first passed 15 semantic controls. With the ownership proofs applied to
the cloning production code, the complete red run
`8f1ad370-6882-414e-b81e-c8699e14a468` ran both tests and failed both pointer-equality
assertions, exit 100. The same proof assertions then passed with the consuming implementation
in a 65/65 core/State/deep run, exit 0. This tests actual source-owned payloads, not a synthetic
clone hook or a production branch flag.

The earlier `ownership-red.log` is a compile-only failure involving six `String::as_ptr` UFCS
calls. `ownership-red-compiled.log` then ran only one of the two tests before stopping on
failure. Neither is substituted for `ownership-red-complete.log`, whose raw SHA-256 matches
the red-green receipt. The compile repair changed method-call spelling before the complete
behavioral red run; it did not relax assertions.

Additional historical logs show 105/105 State render tests and 38/38 selected State ASCII
tests passing. They cover the affected consumer paths alongside semantic ordering, duplicate
roots, dividers, hidden notes, links/styles, cancellation and deep ownership controls.
They do not certify later API migrations.

The State SVG receipt selects and renders all 286 fixtures on each side, skips none, and
records 286 exact old/new byte identities. The audit found and rehashed all 572 SVG files
against the 286 recorded digests, and both report hashes match. Existing `structure, decimals3`
DOM policy, semantic-label residuals and root-not-checked coverage remain as recorded.
Exact local old/new SVG equality is a separate observation; it does not remove those
upstream-comparison coverage limits.

There is a historical executable preservation gap: the SVG candidate log names the shared
`target/debug/xtask.exe`, whose current bytes do not match the candidate hash in the receipt.
The baseline SVG executable is separately retained and matches. See the discrepancy table
below; the stored output equality remains verifiable, but the current shared executable
cannot replay that historical candidate identity.

## Timing remains inconclusive

The frozen timing comparison uses `generic_v2`, `long`, eight A/A calibration pairs, a
maximum of 32 AB/BA confirmation pairs, sample size 30, two-second warmup and three-second
measurement, 95% confidence, seed 0 and 10,000 bootstrap resamples. The registered joint
budget is 10% and 50 microseconds. Package `merman`, bench `pipeline`, default features plus
`svg`, Rust 1.95.0 and one logical operation are preserved. Known-type JSON excludes detection;
cold-engine parse does not mean cold-process startup.

| Group | Registered rows | Inconclusive | Confirmed non-regression | Calibration-required pairs / cap |
| --- | ---: | ---: | ---: | ---: |
| `parse` | 6 | 6 | 0 | 50 / 32 |
| `parse_cold_engine` | 6 | 6 | 0 | 122 / 32 |
| `compatibility_json_parse` | 6 | 4 | 2 | 22 / 32 |
| `end_to_end` | 6 | 6 | 0 | 496 / 32 |
| Total | 24 | 22 | 2 | Group-specific calibration, unchanged cap |

Only `compatibility_json_parse/sequence_medium` and
`compatibility_json_parse/flowchart_medium` confirm non-regression, each using 22 actual
pairs; both have `confirmed_non_improvement` as their improvement outcome. The other 22
rows have zero actual confirmation pairs after unstable A/A calibration. Their reasons
include identity/order margin failures and, for some rows, a required-pairs estimate beyond
the cap. A group's required-pairs estimate does not assert that every row failed the cap.

All four groups terminated with report exit 3. The controller's terminal exit 0 means its
schedule completed; `all_required_controls_pass` remains false. Zero confirmed regressions,
zero material improvements and zero contract failures do not prove full non-regression.
Other Cargo/rustc activity and heterogeneous CPU topology were observed historically; no
uncontended-host claim or affinity/priority/power change is inferred.

The audit matched the four reports, four Markdown reports, four logs and four metadata
hashes linked by the terminal receipt. All 26 source-manifest file hashes match their
historical Git blobs. All 24 terminal row summaries match the raw reports, and both frozen
timing executable hashes still match. No row is dropped or reclassified, and no budget or
threshold is extended. The receipt's status remains **U3 performance NOT ADMITTED**.
U4 allocation/correctness development did not turn these controls into passes.

## Ignored machine evidence and integrity findings

Paths in the following tables are repository-relative beneath
`target/bench/semantic-boundary/`. They are ignored local machine evidence, not files
distributed with this document. Hashes are raw-file SHA-256 values rechecked on 2026-10-22.
The top-level receipts bind the individual reports/manifests described above.

| Receipt path beneath the evidence root | SHA-256 |
| --- | --- |
| `u3-confirmation-memory/allocation-receipt.json` | `7f41327069530df729cd249d05f748edf1ff444ea36e2d48f4e6744becece295` |
| `u3-confirmation-memory/allocation-scope-post-main226.json` | `9dc5df3dd56212fd0bc67d38c924cf02ed4e2268be6a268e2c4a083337ff921e` |
| `u3-timing-a7142d3a4-4ee66d7ca/terminal-receipt.json` | `83214826fc99ac293f097eab7996f8a49f74b28eefff74bff38fb8cbf14b8df3` |
| `u4-adjacent-post-pr227/adjacent-allocation-receipt.json` | `4e6376a95580731fae7cd2863c1338475ca5b209d345f8c606e0502105935be2` |
| `u4-adjacent-post-pr227/red-green-receipt.json` | `98705df3f1ef3da8743790c20f3046767cb92175cebb4f999b0afeae685a3c23` |
| `u4-adjacent-post-pr227/state-svg-equivalence.json` | `f2b1954dd1c6f1ef9b1f78685a32321ab2943d5ee465e2e5d770eb9795244a8e` |
| `u4-adjacent-post-pr227/preregistration.json` | `be11f9de6159a6f0aa7ece0004984de596c37966bf94a12da9ce496eed9b448c` |
| `u4-adjacent-post-pr227/preregistration-amendment-01.json` | `6e4bccda325108f16de522d1a6455ec3d90dbdf0c41b5a382172a3e8dd13dbe2` |

| Supporting path beneath the evidence root | SHA-256 |
| --- | --- |
| `u3-red-facts.log` | `d237efef708429dc6dd65513895b8e761e69c44ff88aef9f03a1e2c7cca1a2aa` |
| `u3-green-facts.log` | `591c46ffb731bf98e3c56cdc6fc068502a6d39e7d4bf902c49c803451e6d1a02` |
| `u3-core-contracts.log` | `bb1d2559c2cd01af446d46aec15c58c1816cb12be59a3bdb7450e3adc70c600b` |
| `u3-state-capture/red.log` | `a5f38f9d1ccdd96c22b6f2134f3e06b9f1be6199d03270cee38fc68d8ac63d17` |
| `u3-state-capture/green-contracts.log` | `765a93f104321d4d7f7f713b415c5d107e1ffe1208c415de1c22d1ffd730e999` |
| `u4-adjacent-post-pr227/semantic-old-owner-green.log` | `4e4dd9f38f6f177df4cb02172521c259a493251362938397275334c8104a81e8` |
| `u4-adjacent-post-pr227/ownership-red-complete.log` | `7c733dd7193ce0c776c3ccc163a975f62112027cd0edbee7514f3e61779bb37d` |
| `u4-adjacent-post-pr227/ownership-green-core.log` | `766c7c3576f6143cf4d578cbca688c7732a6df51d94fb6d82a17c66384255e44` |
| `u4-adjacent-post-pr227/state-render.log` | `3b7f7a154af06c6358824efea88984ebd31ce3fbc28329fd988e544034e6ee0d` |
| `u4-adjacent-post-pr227/state-ascii.log` | `7c591941e850cb485c55edaf60671c52c9f3ed7b576e5dd958f16766f6eda00d` |
| `u4-adjacent-post-pr227/baseline/freeze-receipt.json` | `093f8d619df8954e6e7bb41d3eca416adf9fccbf266fdccc34d03c0acbebfea4` |
| `u4-adjacent-post-pr227/candidate/freeze-receipt.json` | `55da225e8f757828557e32346544422cf91be284f2c9cc3d03d9c64a94920c72` |
| `u4-adjacent-post-pr227/state-svg-baseline/report.md` | `21f35ca0394df13c7a2443730cc1a0b171b5fd684f1dccff180e81fd8c756ba2` |
| `u4-adjacent-post-pr227/state-svg-candidate/report.md` | `2572320b8c414037257e2603aa1319a6ee09bcfec10cf14447137d963ad92058` |

All eight requested receipts, their linked allocation reports/manifests, timing files,
red/green logs, and the recorded SVG outputs were found. The following discrepancy is
retained without modifying evidence:

| Artifact | Historical expected SHA-256 | Observed SHA-256 on 2026-10-22 | Consequence |
| --- | --- | --- | --- |
| SVG candidate shared `target/debug/xtask.exe` | `6749279a4b942617c1a50e2a6701e5f75083c3916cd2ff1765d090093841841c` | `c475d3b69870ecd918e1a6c4f33c21614e8a18f9c3f6f41d2837325ae3dbba26` | Shared-path bytes no longer identify the historical SVG candidate; no matching separately frozen candidate executable was found in its evidence directory |

The independently preserved
`u4-adjacent-post-pr227/state-svg-baseline/xtask-base.exe` still hashes to
`e4071223ac4a8aae781a503f1e7e35c168458b80bd68c8749683c25b665c0ee6`,
matching its `executable.json`. This discrepancy concerns the debug SVG candidate executable,
not the four preserved allocation probes or the two frozen timing executables.
It limits historical executable replay; it does not reclassify the verified stored SVG
equality or introduce an additional performance-admission threshold.

Intermediate proof-region and production-only-patch hashes in the red-green receipt are
historical attestations. The receipt does not name standalone files or define the exact
byte slice for each, so this audit does not present them as independently rehashed file
references. The complete behavioral red/green log hashes and committed production/proof
source are available for review. A later comment-only cleanup before the candidate commit
also means an intermediate whole-DB hash is not the final committed file identity.

## What remains open for U9

This document closes an evidence-documentation gap. It supplies no new integrated
measurement or correctness run and cannot serve as whole-plan approval.

| Gate or claim boundary | Evidence supplied here | Remaining boundary |
| --- | --- | --- |
| U3 public-operation non-regression | Historical 24-row terminal comparison | 22 rows remain inconclusive; full required controls do not pass |
| U4 public-operation timing controls | Adjacent allocation and historical correctness only | U4 full-operation controls and integrated same-contract controls are not established by these receipts |
| Current allocations or general memory bound | Historical finite generated-workload medians and exact work deletion | No current absolute allocation total, arbitrary-input capacity bound, or worst-case whole-parser complexity claim |
| Historical SVG candidate executable replay | Both sets of exact SVG bytes, reports and baseline executable | Candidate shared executable identity no longer matches; the receipt cannot be replayed using today's shared binary as though it were preserved |
| Final integrated correctness/security | Historical U3/U4 focused, lifecycle and State output checks | Applicable current family/consumer, prepared-source, cancellation/resource, editor/LSP/Web and strict parity checks need their own integrated receipts |
| Integrated final bytes and dependency closure | No size measurement in this checkpoint | Isolated same-family runtime-fed semantic embedding, equivalent exercised meaning, raw/stripped bytes and resolved normal features/packages require their separate size evidence and applicable U9 review |
| Capability, packaging and stable host contracts | No installed-product or ABI verification in this checkpoint | Math defaults/positive recipes, full SVG size controls, installed catalogs, bindings/ABI/schema, generated declarations/legal and platform-owned checks remain separate obligations |
| Final migration and production quality | Historical source deltas only | U1-U8 Done criteria, removal of transitional shims/duplicate masters, migration/docs consistency, current nextest/Clippy/format and other applicable plan checks are not certified here |

Separate size or migration reports may supply evidence for their own revisions and contracts.
Their results cannot be inferred from these receipts, and this checkpoint does not
reclassify their current status. A final U9 decision must join the applicable evidence at
the intended integration revision, preserve every failed/inconclusive/platform-deferred
result, and satisfy the approved plan's Done criteria without changing the frozen rules.

## Later local artifact retention update

At 2026-10-22 21:51 UTC+08:00, following a local `cargo clean`, the ignored
`target/bench/semantic-boundary/u3-confirmation-memory/`,
`u3-timing-a7142d3a4-4ee66d7ca/`, and `u4-adjacent-post-pr227/` directories were absent.
The earlier read-only audit and its hashes describe the artifacts that existed at audit time;
the raw reports and frozen executables can no longer be independently rehashed or replayed
from this checkout. Their named source commits and this historical audit remain in Git.

Do not reconstruct original raw samples from these tables or present replacement builds as the
original frozen executables. Any renewed allocation or timing admission needs a fresh controlled
run with explicit source, recipe, output, and executable identities. The unresolved timing
classification above remains unchanged.

## Renewed adjacent allocation evidence (2026-10-23)

Fresh clean Cargo builds repeated the original U3 and U4 revision pairs, workload scales,
five operation/zero pairs per scale, seed `0x4D45524D414E`, and 10,000 bootstrap resamples.
All 28 required lane reports and manifests were frozen and verified outside Cargo targets at
`F:/codex-builds/merman-semantic-capability-boundaries/evidence/final-d0d26db24-20261023/`.
Each side retains a copied probe executable with a matching SHA-256. These are new observations,
not reconstructed samples or the original deleted executable bytes.

The seven-lane results reproduce the historical medians in this document. U3 passes all
35 preregistered allocation checks: four ordinary State/Sequence routes strictly reduce
100x allocation counts and bytes, their peak/retained growth does not increase, and the
requested-snapshot/Flowchart controls stay within the original bounds. U4 passes all
178 checks, including 168 non-increasing metric/scale cells, the seven unchanged candidate
caps and three strictly decreasing State byte counts. All paired semantic output identities
match, and every observed result drop releases operation-owned growth.

| Allocation comparison | State typed bytes at 100x | State JSON bytes at 100x | State snapshot bytes at 100x |
| --- | ---: | ---: | ---: |
| U3 editor demand | 1,626,029 -> 1,319,235 | 3,108,667 -> 2,801,873 | 3,200,859 -> 3,037,339 |
| U4 State transfer | 1,319,235 -> 1,310,627 | 2,801,873 -> 2,793,265 | 3,037,339 -> 3,028,731 |

| Retained receipt | SHA-256 |
| --- | --- |
| `u3-base/freeze.json` | `95723f07b8dba9aef9d8b8c4a430e1e61385c10f9aa52cc85cff5390f1528067` |
| `u3-head/freeze.json` | `8de3a6ce5308643a8960b4c5595f4b66ea12c7e214d7758947b9b8c742f1928c` |
| `u4-base/freeze.json` | `5d63c96b230df3bd5a6c21f33cfd3906666300d70cab5137fdb08d09ff990eed` |
| `u4-head/freeze.json` | `8375dc8c809b559b89c10c4065d3aa613c04afdb9e2d8e3fc20409a71c2b22cf` |
| `u3-allocation-comparison.json` | `fbf6a8d51c82a142deedc1eac64bbcab8fae3ff7819180effe0bfb1cfaefcc22` |
| `u4-allocation-comparison.json` | `833de0d6ad4fb19b1bf917a9b43dee7713488d458ef087406ec981bcd2f0682a` |

Concurrent unrelated Cargo activity was observed during some probe builds and recorded in
`environment-observations.jsonl`. No uncontended-host, latency or RSS claim follows from these
System allocator observations. The renewed public-operation timing schedule has not started;
the historical U3 timing result remains NOT ADMITTED.

A final completion review found missing lean Flowchart accessibility normalization and eager
full-engine initialization in binding analysis/ASCII construction. Those corrections require
their own focused tests and new final-source artifact evidence. The queued optional integrated
allocation observation of `d0d26db24` was cancelled before sampling because that product source
was superseded; all required U3/U4 samples remain retained. Allocation passes alone do not
close R14, the same-contract timing controls, or the complete U1-U9 plan.

## Renewed U3 warm-parse confirmation (2026-10-23)

The renewed registered U3 `parse` group completed with six
`confirmed_non_regression` results and zero contract failures or inconclusive rows. This is
a new observation of the existing adjacent timing pair `a7142d3a4` to `4ee66d7ca`, not a
reclassification of the historical 22 inconclusive rows. Both clean executables, shared
benchmark/corpus bytes, output identities and the discovery trust anchor were verified.

The original `generic_v2`, `long`, eight A/A calibration pairs per runner, maximum 32 fresh
AB/BA pairs, sample size 30, two-second warmup, three-second measurement, 10% AND 50,000 ns,
95% simultaneous confidence, seed 0, 10,000 resamples and divisor one were unchanged.
Every row passed all A/A identity/order/budget checks; calibration selected eight fresh
confirmation pairs. Each row contains the complete balanced AB/BA sequence and repeated
preflight/postflight output identity. Post-sampling source, fixture and discovery verification
also passed.

| Warm full parse fixture | Base mean | Head mean | Classification | Fresh pairs |
| --- | ---: | ---: | --- | ---: |
| Sequence medium | 154.810 us | 142.955 us | Confirmed non-regression | 8 |
| State medium | 168.500 us | 157.481 us | Confirmed non-regression | 8 |
| Class medium | 225.194 us | 206.931 us | Confirmed non-regression | 8 |
| ER medium | 141.335 us | 135.269 us | Confirmed non-regression | 8 |
| Pie medium | 6.199 us | 5.455 us | Confirmed non-regression | 8 |
| Flowchart medium | 368.453 us | 359.608 us | Confirmed non-regression | 8 |

All six mirrored improvement results are `confirmed_non_improvement` under the registered
materiality contract. The measured means do not establish an admitted speedup. This group
supplies only the warm full-parser non-regression control; the other 18 U3 rows and 48
U4/integrated rows are still pending. No whole-plan or U3 performance admission follows yet.

The new raw JSON, Markdown, log, discovery and copied executables are retained under
`F:/codex-builds/merman-semantic-capability-boundaries/evidence/final-f0365e6a4-20261023/timing-u3/`.
The parent evidence root's `u3-parse-confirmation-audit.json` checks the complete six-row
schedule, unchanged contract and output/fixture/discovery identities; its SHA-256 is
`adf9790743e74da5328491af6427fe94c21437c45bf3d775f0864ce59ab0fcba`.
Concurrent unrelated project work remains possible; this is not an uncontended-host claim.

## Renewed U3 cold-engine confirmation (2026-10-23)

The registered `parse_cold_engine` group also completed with six
`confirmed_non_regression` rows, zero contract failures and zero inconclusive rows. Each row
passed stable eight-pair A/A calibration on both runners and eight fresh balanced AB/BA
confirmation pairs under the unchanged protocol above. Output identities, logical-operation
divisor one, preflight/postflight, clean source files, fixture bytes, frozen executables and
discovery retention were checked after sampling. This lane constructs a fresh full engine per
iteration within a reused process; it does not measure process cold start.

| Cold-engine full parse fixture | Base mean | Head mean | Classification | Fresh pairs |
| --- | ---: | ---: | --- | ---: |
| Sequence medium | 156.353 us | 149.594 us | Confirmed non-regression | 8 |
| State medium | 172.861 us | 162.385 us | Confirmed non-regression | 8 |
| Class medium | 230.589 us | 208.920 us | Confirmed non-regression | 8 |
| ER medium | 147.435 us | 142.469 us | Confirmed non-regression | 8 |
| Pie medium | 11.550 us | 10.613 us | Confirmed non-regression | 8 |
| Flowchart medium | 369.214 us | 364.423 us | Confirmed non-regression | 8 |

Every mirrored improvement result is `confirmed_non_improvement`. The warm and cold groups
now supply 12 of the 72 registered non-regression controls. The remaining 12 U3 JSON/E2E rows
and 48 U4/integrated rows are open; no speedup, complete U3 performance admission, or whole-plan
completion follows. Historical inconclusive results remain unchanged.

The final evidence root retains `u3-cold-parse-confirmation-audit.json`, SHA-256
`d6e0da49e758f4545899a9d3ea96ca5fff531465ffab5c36c25ff7abb0873cc4`, and the raw
`timing-u3/parse_cold_engine.{json,md,log}` files. Concurrent unrelated host activity remains
possible.

## Renewed U3 known-type JSON confirmation (2026-10-23)

The registered `compatibility_json_parse` group completed with six
`confirmed_non_regression` rows, zero contract failures and zero inconclusive rows. This is
the known-type compatibility JSON operation, which excludes detection. Both runners passed
eight stable A/A pairs; every fixture then completed eight fresh balanced AB/BA pairs. The
registered protocol, thresholds, sample budget and divisor one were unchanged. Output
identities, preflight/postflight, clean sources, fixture bytes, frozen executables and discovery
retention were checked after sampling.

| Known-type JSON fixture | Base mean | Head mean | Classification | Fresh pairs |
| --- | ---: | ---: | --- | ---: |
| Sequence medium | 170.111 us | 157.915 us | Confirmed non-regression | 8 |
| State medium | 355.649 us | 343.216 us | Confirmed non-regression | 8 |
| Class medium | 280.960 us | 268.846 us | Confirmed non-regression | 8 |
| ER medium | 149.243 us | 144.032 us | Confirmed non-regression | 8 |
| Pie medium | 7.667 us | 6.769 us | Confirmed non-regression | 8 |
| Flowchart medium | 505.812 us | 490.835 us | Confirmed non-regression | 8 |

All mirrored improvement results are `confirmed_non_improvement`. These observations bring
the renewed controls to 18 of 72; six U3 E2E and 48 U4/integrated rows remain open. No speedup,
complete U3 performance admission or whole-plan completion follows. Concurrent unrelated host
activity remains possible, and historical inconclusive observations remain unchanged.

The final root retains `u3-json-parse-confirmation-audit.json`, SHA-256
`62e222b8fbd25b948b0257b3380d1815db3aba828de7e040fd2ea467a00a70ae`, and the raw
`timing-u3/compatibility_json_parse.{json,md,log}` files.

## Renewed U3 E2E remains inconclusive (2026-10-23)

The final registered U3 group completed with five `confirmed_non_regression` rows and one
`inconclusive` State row. It exited 3 with zero contract failures. The U3 schedule is now
complete: 23 controls confirm non-regression and one does not. This does not admit U3 performance;
48 U4/integrated controls remain unexecuted.

Both runners completed all eight A/A calibration pairs on all six fixtures. State's base
calibration was stable, but head failed `identity_log_within_margin`,
`identity_absolute_within_margin` and `order_absolute_within_margin`. Its head A/A identity
interval was -93.956 to +12.038 us, with a log-ratio interval of -0.113472 to +0.014686;
the absolute order-effect interval was -78.460 to +37.477 us. These exceed the registered
50 us and log(1.1) calibration boundaries. The State row retains matched exact SVG output
identity, but has no fresh confirmation pairs, means or effect bounds. It establishes neither
a regression nor non-regression.

The unchanged calibration policy scheduled ten fresh balanced AB/BA pairs for each stable
fixture. Their required counts were eight; State's calibration-derived count was ten. The
shared schedule is fixed from calibration, not an extension after inspecting confirmation.
Every stable row completed that ten-pair schedule with identical preflight/postflight outputs,
divisor one and verified post-sampling source, fixtures, discovery and frozen executables.

| E2E fixture | Base mean | Head mean | Classification | Fresh pairs |
| --- | ---: | ---: | --- | ---: |
| Sequence medium | 477.041 us | 470.080 us | Confirmed non-regression | 10 |
| State medium | Not admitted | Not admitted | Inconclusive: head A/A unstable | 0 |
| Class medium | 1,081.990 us | 1,071.860 us | Confirmed non-regression | 10 |
| ER medium | 678.544 us | 669.829 us | Confirmed non-regression | 10 |
| Pie medium | 37.636 us | 35.883 us | Confirmed non-regression | 10 |
| Flowchart medium | 1,823.720 us | 1,821.790 us | Confirmed non-regression | 10 |

All five mirrored improvement outcomes are `confirmed_non_improvement`; State's is absent.
No retry, additional samples, dropped row, threshold change or speedup claim follows. The fixed
queue continued to U4 discovery. Concurrent unrelated host work remains possible; no causal
explanation for State's calibration noise is established here.

The final root retains `u3-e2e-confirmation-audit.json`, SHA-256
`05e80b089396f465fe68fcc0c112d6bd7a2c6c5063b97d471e334479302cdf3c`, and the raw
`timing-u3/end_to_end.{json,md,log}` files. The renewed inconclusive row and historical
inconclusive observations both remain unadmitted.

## Renewed U4 warm-parse confirmation (2026-10-23)

U4 discovery rebuilt and froze both clean adjacent sources, f676075658a86769fd2d55fdc1fbc42803c7707c
and 78c0625d14ebfa447d2320d543ca08744bd86ab2. All six fixtures have identical input bytes and
matched consumed typed output. The pipeline, JSON identity helper, corpus, preflight contract
and lockfile are identical between these sources. The retained base executable SHA-256 is
`324230b8e61ca3031a955bd7248f08ca04d3b295d32fd0d04fa445b909926d80`; head is
`8a314bf1b1ba09f08a7b21e680b69d98ccabb28ddb9471f85a4c6c7288db66f3`.

The registered U4 warm full-parse group then completed with six `confirmed_non_regression`
rows, zero contract failures and zero inconclusive rows. Each runner passed eight A/A pairs;
every fixture completed eight fresh balanced AB/BA pairs under the unchanged generic_v2
protocol, thresholds and budget. Divisor one, output/preflight/postflight identity, clean
sources, fixture bytes, frozen executables and discovery retention were checked after sampling.

| Warm full parse fixture | Base mean | Head mean | Classification | Fresh pairs |
| --- | ---: | ---: | --- | ---: |
| Sequence medium | 135.058 us | 139.911 us | Confirmed non-regression | 8 |
| State medium | 152.407 us | 151.465 us | Confirmed non-regression | 8 |
| Class medium | 201.380 us | 203.230 us | Confirmed non-regression | 8 |
| ER medium | 131.505 us | 132.868 us | Confirmed non-regression | 8 |
| Pie medium | 5.419 us | 5.496 us | Confirmed non-regression | 8 |
| Flowchart medium | 356.565 us | 357.520 us | Confirmed non-regression | 8 |

The small positive means on five controls are retained; the registered material-regression
contract still classifies them as non-regressions. All mirrored improvement results are
`confirmed_non_improvement`, so no speedup is admitted. These are adjacent full-operation
controls, not semantic-only timing or timing of the later private cfg cleanup.

Across the completed U3 and U4 warm groups, 29 controls confirm non-regression and one U3 State
E2E control remains inconclusive. Eighteen U4 and 24 integrated rows remain without terminal
classifications. The U3 result is unchanged, U4 is incomplete and whole-plan performance
acceptance remains unproven. Concurrent unrelated host work remains possible.

The final root retains `u4-discovery-audit.json`, SHA-256
`b99da2da58cea3f0ea291a982e50e593a7e9d68044a42d6bda3ee8a3273f03c5`, and
`u4-parse-confirmation-audit.json`, SHA-256
`103c0382f02e0f0d3bce3095f839d3dc93c6f50f5bb4a81af7f793f2c26d72d4`. Raw discovery,
parse reports/logs, registrations and executable copies remain under `timing-u4/`.

## Renewed U4 cold-engine confirmation (2026-10-23)

The registered U4 `parse_cold_engine` group completed with six `confirmed_non_regression`
rows, zero contract failures and zero inconclusive rows. Both frozen runners passed eight
A/A pairs; all fixtures then completed eight fresh balanced AB/BA pairs under the unchanged
protocol and budget. The parent checked every pair/order, divisor one, output/preflight/postflight
identity, clean sources, fixture bytes, frozen executables and discovery retention.
This lane constructs a fresh full engine per iteration inside a reused process, not a cold process.

| Cold-engine full parse fixture | Base mean | Head mean | Classification | Fresh pairs |
| --- | ---: | ---: | --- | ---: |
| Sequence medium | 147.960 us | 146.705 us | Confirmed non-regression | 8 |
| State medium | 161.018 us | 158.206 us | Confirmed non-regression | 8 |
| Class medium | 209.639 us | 208.056 us | Confirmed non-regression | 8 |
| ER medium | 140.139 us | 137.833 us | Confirmed non-regression | 8 |
| Pie medium | 10.635 us | 10.727 us | Confirmed non-regression | 8 |
| Flowchart medium | 361.877 us | 372.050 us | Confirmed non-regression | 8 |

All mirrored improvement outcomes are `confirmed_non_improvement`. Positive Pie and Flowchart
means are retained; the registered materiality contract still confirms non-regression. No
speedup follows. U4 now supplies 12 confirmed controls, with 12 JSON/E2E rows still without
terminal classification. Across all groups, 35 confirm non-regression, one U3 State E2E remains
inconclusive and 36 U4/integrated rows remain open. Whole-plan performance is unproven.

The final root retains `u4-cold-parse-confirmation-audit.json`, SHA-256
`2184da6ad3ec5120670a10eb4dfd1d804289ef929c5f4a34d2dbcbb35c1ff7be`, and the raw
`timing-u4/parse_cold_engine.{json,md,log}` files. Original protocols/caps and the U3
inconclusive result remain unchanged; concurrent unrelated host activity is possible.
