# Theme regression root-cause investigation (macOS, 2026-10-21)

This investigation compares v0.9.0 (`f754a3c3`) with production `475373fc5` and separates
new theme/render work from costs that already existed in the tag. It pauses the allocation-only
cleanup queue until an owner has a measured contribution to the remaining end-to-end difference.
The current production PR is unchanged by all diagnostic interventions below.

## What historical evidence establishes

The latest admitted release comparison is still production `b9a58731c`: 19 exact-SVG scenes,
14 within the joint budget, two confirmed regressions and three inconclusive scenes. Packet's
confirmed difference is +2.332 us and ZenUML's is +3.499 us. Kanban's +8.521 us point difference
is inconclusive because its absolute A/A certificate is unqualified. These scenes cover 15 of
35 logical families; another 57 changed-output scenes cover the remaining 20 logical families
(23 aliases). Allocation coverage across 418 cells does not fill that historical timing gap.

The earlier admitted `0ce2aa0bd` checkpoint already reports approximately +2.234 us for Packet and
+3.561 us for ZenUML. It precedes prepared redraw commit `f92945b32`. That new Arc and metadata
clone cannot be the original explanation. These observations are not paired measurements of
`f929`, and their values must not be subtracted to estimate that commit's isolated cost.

| Reachable change | Introduction | Difference from the tag |
| --- | --- | --- |
| Bounded Packet output | `bff167831` | Plain String becomes bounded append/state/work/capacity protocol |
| Bounded Kanban output | `cead10639` | Same per-fragment protocol, with family checkpoints |
| Bounded ZenUML output | `41711248c` | Same per-fragment protocol, with fallible helpers |
| XML stylesheet text output | `369c4bd3b` | Every generated CSS byte passes XML classification/encoding; fragments pass another writer layer |
| Theme reports and terminal admission | `1907caa2e`, `c42b1dde2`, `e1f93a837` | Additional theme/summary/admission projections and terminal checks |
| Prepared redraw ownership | `f92945b32` | One additional prepared Arc and successful SVG metadata clone; not the original residual |

Saved default output contains approximately 1.7 KB of Packet CSS, 2.9 KB of ZenUML CSS and
16.1 KB of Kanban CSS. The XML and bounded writer paths are visible in the admitted b9 Packet/
ZenUML CPU samples. Sampling establishes reachability, not call counts or a causal microsecond
fraction. XML work is proportional to stylesheet bytes and formatter fragments; bounded appends
add per-fragment protocol and occasional reservation work. The two overlap.

## Excluded explanations

Core operation/runtime implementation and the normal no-deadline attention check are unchanged
from v0.9.0. Both versions scan the same 440 text-measurement counters; that report cannot be
counted as newly added wholesale. The old nonrecursive JSON disposal and ZenUML editor-facts
construction also existed in the tag. The latest JSON improvement establishes memory/work
removal, not theme-regression causality.

Current ordinary None + BestEffort + parity output already skips the full XML terminal observer.
The old redundant XML/fingerprint/certification passes have been removed; historical profiles of
those passes are not evidence that they still dominate. Default theme initialization is inherited,
not recomputed through compiled-theme schema validation on every request. Current None fallback
also uses cached/static root state rather than rebuilding nonempty theme request sets.

## Independent compiled-theme problem

With a compiled theme, several local plans each create a complete required-request owner/index
using `FamilyThemeEvidence::from_theme`. Some finish by cloning it. `merge_accounted_from`
consumes only outcomes, so those duplicate required/index structures are discarded. Journey and
C4 provide concrete examples. Separately, `theme_coverage_complete` reconstructs five BTreeSets;
diagnostics, rich summary and strict admission can invoke coverage more than once.

This is a source-backed ownership/validation problem across the theme family architecture. It is
not an explanation of Packet/ZenUML None timing: empty sets do not perform that nonempty work.
The natural boundary is one immutable compiled request index, local mutable terminal outcomes,
and a coverage result frozen once for the unchanged evidence state. Flowchart/Sequence/State
ledger replacement and postprocessor mutation invalidation must migrate together. A pre-mutation
Verified result must never survive an untrusted output transformation.

## Registered causal experiment

All variants use current `475373fc5`, the byte-identical existing common public Criterion harness,
locked Rust 1.95.0, optimized bench profile, default features plus all-diagrams, svg,
layout-cytoscape, layout-elk and math. The operation includes fresh control/request/environment,
raw-source parse, layout, SVG, terminal/report/admission construction and complete result drop.
It does not substitute a reused prepared source or an SVG-bytes API.

| Diagnostic variant | Deliberate intervention |
| --- | --- |
| Current | Unchanged writer implementation |
| Bounded append bypass | Replace record_write by String append, removing state/work/byte-credit/exact-reserve protocol together |
| XML wrapper bypass | Forward logical CSS bytes, removing classification/encoding/chunk routing/trailing-bracket state and wrapper checkpoint |
| Combined bypass | Apply both replacements, exposing interaction |

These are implementation-bundle interventions. The bounded arm also changes allocation policy;
the XML arm also changes fragment/work routing. Neither represents a pure guard or escaping
arithmetic cost. Neither is a valid production candidate: arbitrary authored text, resource
limits, cancellation and first-error behavior deliberately differ. All three fixed successful
fixtures must preserve exact SVG bytes/hash/element count, before timing and in every same-side
pre/postflight. The rich operation still executes, but the common receipt directly checks SVG;
this experiment does not claim cross-variant report/error equivalence.

The fixed schedule uses current fresh A/A eight pairs, then four alternating AB/BA pairs for each
intervention and the archived no-hook v0.9.0 reference, across Packet, ZenUML and Kanban. Criterion
uses 20 internal samples, one-second warmup and one-second measurement. Every process/log and
pair is retained; no retry-to-pass or sample deletion is allowed. All three changed source
variants have independent copied executable identities before any later build. The diagnostic
worktree returns to registered current writer source after those binaries are frozen.

This is bounded diagnostic evidence, not the calibrated release admission protocol. Short
four-pair data cannot reclassify the existing 14/2/3 report or prove an all-family improvement.
Independent parse/layout/render stages are not subtracted to invent a finalize duration.

## Results

The complete serial build and measurement drivers both terminate with real exit status zero.
All fifteen initial identity discoveries and all 144 scheduled observations preserve exact SVG
identity. There are 48 A/A and 96 A/B invocations, without resampling. Four independently built
current-source executables and the archived v0.9.0 reference have immutable SHA/0555 identities.

The table reports the median of four paired **variant minus current** differences, in us:

| Fixture | Bounded append bypass | XML wrapper bypass | Combined | Old reference minus current |
| --- | ---: | ---: | ---: | ---: |
| Packet | -0.506 | -1.017 | -1.368 | -2.377 |
| ZenUML | -0.411 | -0.420 | -0.565 | -3.469 |
| Kanban | -1.808 | -12.233 | -13.914 | -8.601 |

Current A/A point ranges are Packet 10.227-10.550 us, ZenUML 23.158-23.685 us and Kanban
52.616-54.679 us. Every raw pair remains available. In particular, ZenUML's four combined
intervention deltas are -0.056/-0.615/-0.566/-0.564 us; do not discard the first near-zero pair
or claim a stable formal improvement. Current partners in the fixed comparisons remain explicit,
so the table does not hide drift by dividing unrelated run medians.

These interventions strongly prioritize the XML/writer architecture for Kanban and a material
part of Packet, but do **not** support a unified writer explanation of ZenUML's principal gap.
Kanban's combined intervention exceeds its historical-reference difference because this is a
counterfactual current implementation, with different growth/work routing; it is not a valid
"percentage of the regression" decomposition. Similarly, individual arms must not be added.
The result does not justify deleting resource admission or XML encoding in production.

ZenUML therefore needs a separate causal investigation of parse, preparation, inline rich-report
carriers and finalization. A matched typed-parse diagnostic was registered using those same
existing independent executables. Its discovery stopped on Packet: current compatibility projection is 23838 B,
v0.9.0 is 23819 B, with different SHA. ZenUML's full 1512 B projection matches exactly. The failed
three-fixture run preserves all five discovery logs and real terminal status 1; it reached zero
registered A/A or A/B observations. A new ZenUML-only diagnostic is independently registered,
with fresh discovery and fixed 16 A/A + 8 A/B observations. It does not reuse discovery timings,
normalize the Packet difference, subtract parse time from E2E, or perform a release admission.

The ZenUML-only run then completes with real terminal status 0, two fresh discoveries and all 24
registered observations. Current typed-parse A/A spans 8.984-9.256 us. Its four current-minus-old
paired differences are -0.0626, +0.3673, +0.3723, +0.3136 us, median +0.34045 us. One pair reverses
sign and the short diagnostic is not a formal regression certificate. It does not support a
multi-microsecond parse explanation of the rich E2E residual; it also does not certify parse
as equivalent or permit subtracting independently timed workloads. Next investigate default
layout preparation and the shared rich-completion/report carrier separately.

## ZenUML producer and geometry evidence

Source comparison identifies a different mandatory producer expansion before the downstream
writer: the tag CSS template interpolates 35 diagram-ID occurrences; current still interpolates
those 35 IDs and adds 61 font/color/size/title occurrences across 28 distinct non-ID arguments. The
producer first assembles an ordinary String, so writer/XML intervention cannot remove that
formatting. Current defaults expand to the same inspected CSS bytes. Typography integration
`ef9595dab` introduces this dynamic family handling; `44fccd368` later corrects its XML boundary.
A separate diagnostic holds writer/layout/report behavior fixed while replacing only the
default producer's extra substitutions, retaining dynamic title fill and all 35 projected-ID
Display calls and their order. The single-function diagnostic ignores authored source CSS and
is not deployable. Two successful serial drivers freeze its independent executable, restore the
isolated source, verify six exact output discoveries, and complete 48 A/A + 24 A/B observations.

| Fixture | Current minus simplified producer, four pairs (us) | Median (us) |
| --- | --- | ---: |
| ZenUML | +0.825, +0.837, +1.404, +0.668 | +0.831 |
| Packet control | +0.245, -0.011, +0.064, -0.118 | +0.0265 |
| Kanban control | +0.988, +0.439, +0.905, -0.544 | +0.672 |

ZenUML's current A/A spans 23.116-23.856 us. The four positive ZenUML contrasts nominate the
new producer work for a semantics-preserving redesign. Control noise and code-layout effects
remain visible, especially Kanban's sign reversal. This is not a calibrated adjacent speedup or
proof of a precise fraction of the release regression. Do not add this result to earlier writer
interventions, which use different binaries and schedules. Full rich construction/drop remains
in scope, but these common benchmark receipts directly verify SVG rather than report equality.
A safe implementation must retain the dynamic configured path and prove its eligibility.

The same typography change adds a final `finite_layout_geometry` pass over all output geometry,
even for default scale 1. This is real new O(layout-size) work, but simply deleting it is incorrect:
a host can provide individually finite enormous widths whose later sums overflow. No-theme
status does not prove derived coordinates finite. A future fusion must preserve final-value
coverage and the error position after existing host callbacks.

## Safe literal serialization candidate: rejected

A separate deployable candidate replaces repeated XML classification of compile-time ASCII CSS
literals with a const-validated literal type. Dynamic values continue through the XML writer;
original fragment boundaries, byte admission, scan observations, cancellation and first-failure
semantics remain. It changes common CSS plus Packet and Kanban; ZenUML is a control. All 5241
renderer release tests pass (five skipped), followed by two new writer-seam differential tests.
The first test compile fails on a non-const `unwrap`; that failed attempt and its exact source
reconstruction remain archived. The candidate runtime is unchanged by that test-only correction.

A preregistered diagnostic completes 32 calls: four fixed balanced pairs for each of four scenes.
The unchanged public full-rich E2E harness and both frozen executable identities are verified;
all four SVG byte/hash receipts match. These are diagnostic estimates without fresh A/A.

| Scene | Base mean (us) | Candidate mean (us) | Paired relative change | Mean delta (us) |
| --- | ---: | ---: | ---: | ---: |
| Kanban | 53.406 | 50.44775 | -5.539% | -2.95825 |
| Packet | 10.256 | 10.0242 | -2.265% | -0.2318 |
| ZenUML control | 23.37375 | 23.48175 | +0.464% | +0.108 |
| Flowchart control | 1093.425 | 1100.250 | +0.619% | +6.825 |

Kanban exceeds the registered 1 us absolute screen but fails its 10% relative screen, so the
candidate is retired without formal continuation or a lowered threshold. The four owned source
files were hash-checked, archived and returned to their exact pre-candidate bytes. The measured
safe subset is much smaller than the earlier diagnostic intervention that bypassed the whole
XML adapter; those experiments must not be treated as equivalent or their deltas added.

The [candidate archive](evidence/generated-css-serialization-2026-10-21/manifest.json) retains
registrations, complete candidate patch/source, build/test/driver logs, normalized observations,
errors and independent review. The stock helper records per-process Criterion estimates and
stdout hashes, rather than preserving 32 physical stdout files; independent reparsing of all
Criterion text is unavailable. No formal v0.9.0 classification changes.

The next all-family ownership candidate targets compiled-theme request sharing, local outcome-only
plans and owner-maintained exact coverage. It is registered for public warm allocation/memory and
behavior controls, with bounded cold-cache retention explicit. It is not an explanation of the
remaining None Packet/ZenUML gap or an admitted latency improvement.

## All-family ledger ownership refactor: kept for memory and work

The registered ownership refactor is complete across the renderer theme families. The immutable
compiled family program now owns the ordered unique request keys and membership index. Each render
operation owns one checked ledger; local family plans record outcomes only. Prepared plans merge
borrowed outcomes into that ledger, and State, Sequence, Flowchart and Swimlane replacement paths
clear actual outcomes while retaining the request owner. Frozen reports retain only an independent
ordered key slice. Coverage uses the ledger's unique accounted domain instead of rebuilding five
sets at every observer. Unknown keys remain rejected, first classification and capabilities win,
postprocessing invalidation remains fail-closed, and the report Debug shape stays compatible.

All 78 changed renderer source files pass the final source review. The full release graph passes
8107 tests with 7 skipped, strict production Clippy, three DOM modes, optimized full WASM, 34
internal theme acceptance tests, 459 pipeline benchmarks across 7 groups, formatting, path and
diff checks, and the native memory protocol smoke. Exact SVG/report identities, ordered callback
traces and 26 public error controls match. The first two compile attempts failed only in migration
adapters; their logs are preserved, and the third attempt passed after using existing accessors
and checked-ledger test sinks.

The full catalog control has 418 stable cells: 416 improve and 2 are unchanged across allocation
count, allocated bytes, peak growth and retained growth; none regresses. All 380 non-empty themed
cells strictly reduce allocation count and allocated bytes. The 645-site control has 505 jointly
stable sites: 482 improve, 23 are unchanged and none regresses; 140 variable sites remain outside
admission. Representative editor-light deltas are Kanban -64 allocations/-15,064 bytes, Journey
-43/-7,861, C4 -34/-6,906 and Block -76/-18,632. Retained growth stays unchanged. The cached
request space has a transparent O(R+B) byte bound where R is unique keys and B is the UTF-8 bytes
of unique effect IDs; the ordered list and membership index temporarily own two copies, while a
frozen report keeps only one ordered slice and never keeps the program or index alive.

This is an accepted warm allocation/work and ownership improvement. It has not been admitted as a
latency improvement, a None-path fix, or a v0.9.0 convergence result. The absolute process-live
probe changes (None +9 bytes; themed +529 to +2,401 bytes) include argv, paths and other owners and
must not be described as cache-only memory. A fresh balanced public timing confirmation is the next
step before changing any v0.9.0 latency classification.

The [compact ledger evidence archive](evidence/theme-ledger-ownership-2026-10-21/manifest.json)
contains the source identity, design and correctness reviews, memory/control summaries, terminal
receipts and failed-attempt records. Full per-site and catalog raw rows remain in the ignored
experiment directory.

## Ledger latency diagnostic: no admission

After the memory/work result was committed as `e76736165`, a fresh diagnostic compared it with
clean `f46f751f` using eight end-to-end SVG fixtures and 4 balanced AB/BA pairs per fixture
(64 timed calls). SVG output identities matched and both physical executables were frozen.

| Fixture | Base | Candidate | Paired change |
| --- | ---: | ---: | ---: |
| Kanban | 53.12 us | 52.69 us | -0.81% |
| Packet | 10.15 us | 9.96 us | -1.83% |
| ZenUML | 23.09 us | 22.93 us | -0.67% |
| Flowchart | 1.090 ms | 1.097 ms | +0.66% |
| Journey | 28.23 us | 27.83 us | -1.42% |
| C4 | 109.81 us | 110.55 us | +0.67% |
| Block | 32.48 us | 32.64 us | +0.49% |
| Sequence | 278.76 us | 280.84 us | +0.75% |

No row reaches the registered materiality screen. The ownership refactor remains admitted for
warm allocation/work only; this diagnostic does not change the v0.9.0 latency classification.

The [ledger latency archive](evidence/theme-ledger-latency-2026-10-21/manifest.json) retains
the registration, full stock report and raw normalized rows.

## ZenUML repeated selector normalization: keep the safe removal

The ZenUML renderer already receives the normalized `SvgDiagramId` owned by the public SVG
entrypoint, but its stylesheet call wrapped that value in `css_selector_diagram_id` again. The
stylesheet template emits 35 mandatory diagram-ID selector occurrences, so the old call repeated
the same eligibility scan for every occurrence. Passing the existing typed ID directly removes
those repeated predicates while retaining the typed projection, XML boundary, byte accounting,
resource limits and cancellation behavior. Raw test identifiers continue to use the adapter in
the private writer tests.

A fresh diagnostic compared `4657e6017` with `b18c436a2` using four end-to-end SVG
fixtures and four balanced AB/BA pairs per fixture (32 timed calls). ZenUML moved from 24.02 us to
22.79 us, with paired bounds of -4.19% to -6.06% and -1.008 to -1.452 us. Packet, Kanban and
Flowchart control diagnostic bounds span zero; this is not calibrated non-regression admission.
All four SVG byte hashes, element counts and input hashes matched exactly; both physical benchmark
executables were frozen and the run had no contract errors. The reduction is below the 10% relative
materiality screen, so it is retained as a
low-risk source cleanup without changing the v0.9.0 classification or claiming full-gap recovery.

The [ZenUML normalization archive](evidence/zenuml-id-normalization-2026-10-21/manifest.json)
retains the compact stock report, not a separate registration. The
[correction notice](evidence/zenuml-diagnostic-corrections-2026-10-21.md) records the corrected
commit and mean fields, unknown registration chronology, and actual 50 us tool threshold versus
the 1 us advisory prose. This diagnostic does not establish formal latency admission.
The next investigation should target the remaining ZenUML completion/layout cost rather than
adding another selector cache or generalized normalization layer.

## ZenUML default CSS: retain the static fast path

The exact default source style has no configured colors, font family or font size, yet it previously
went through the full dynamic stylesheet template. The renderer now reuses the source-backed
v0.9.0 default CSS template when the source style and title fill are both default. Configured
`themeVariables.zenuml` values and non-default title fills continue through the dynamic path, so
custom CSS encoding, byte admission, cancellation and terminal evidence remain unchanged.

A fresh diagnostic compared `ff4c78c3` with `ef0b8a85` over the same four fixtures and 32 timed
calls. ZenUML moved from 23.13 us to 22.33 us, with paired bounds of -2.11% to -4.75% and
-0.486 to -1.111 us. Packet, Kanban and Flowchart control diagnostic bounds span zero; all SVG
hashes, element counts and input hashes matched. The frozen benchmark executable grew by 128 bytes.
This is a low-risk default-path cleanup below the 10% relative materiality screen, retained without
changing the v0.9.0 classification.

The [default CSS fast-path archive](evidence/zenuml-default-css-fast-path-2026-10-21/manifest.json)
retains the stock report with SVG identity checks and executable sizes, not a separate registration
or rich-report/error comparison. The
[correction notice](evidence/zenuml-diagnostic-corrections-2026-10-21.md) also applies here.
Neither diagnostic certifies formal improvement or control non-regression.
The runs use different baselines and measurement epochs; their gains must not be added.

## Revised implementation priorities

1. Diagnose and redesign generated CSS serialization as an output operation, beginning with the
   large Kanban stylesheet and Packet formatter fragment protocol. Preserve dynamic XML encoding,
   allocation-before-write byte admission, bounded cancellation and sticky first failure. No
   punctuation blacklist, all-input "safe" claim, permanent bypass or unbounded sink is admitted.
2. Redesign ZenUML default CSS production without redundant dynamic substitutions, with exact
   source-style eligibility and the complete configured fallback. Then localize its remaining
   residual with an isolated layout/preparation versus rich-completion intervention and
   generated-code evidence. The old 440-slot report scan and editor facts are not
   newly introduced. Treat inline report/result size as a testable hypothesis, not proven memcpy.
3. Separately replace duplicate compiled-theme request owners with one immutable program index
   plus local outcomes, then reuse final coverage only while its evidence state is unchanged.
   This is the genuine all-themed-family architecture frontier; it does not explain None timings.
4. Keep fresh ownership/metadata Cow and tag-shared micro-optimizations parked until the larger
   mechanisms are attributed. The previous rejected candidate's memory regression remains a
   rejection, not something to relabel after unrelated work.

The expected next delivery is a source-preserving architectural change with an adjacent normal
public E2E confirmation. Diagnostic bundle savings are a prioritization signal, not the amount
that a safe implementation is guaranteed to recover. The all-family goal remains incomplete.

## Evidence

The [checked-in archive](evidence/theme-root-cause-2026-10-21/manifest.json) preserves the
source audits, exact diagnostic patches and drivers, raw logs, observations, identity receipts,
independent reviews, and failed discovery. Physical executables remain in the ignored directory;
no diagnostic bypass is present in production. It is evidence for this host and source revision,
not an all-input equivalence or performance guarantee.

Ignored source audits, preregistration, exact variants, drivers and raw runs live under
`target/bench/experiments/theme-regression-root-cause-20261020/`. The ownership draft under
`fresh-render-owned-metadata-20261020/` remains source-only and is parked pending root-cause
priority; it has not been applied, compiled or admitted. Production source remains `475373fc5`.
