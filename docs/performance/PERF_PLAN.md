# Performance Plan

This is the single current performance backlog for Merman. It records what to optimize and why.
Use [RUNBOOK.md](RUNBOOK.md) for the execution loop and [BENCHMARKING.md](BENCHMARKING.md) for
measurement semantics and tool details. Dated reports are evidence for a particular revision and
host; they are not rolling sources of truth.

As of 2026-10-12, ordinary CLI Mermaid graphical output defaults to deterministic
measurement without a native font context or system discovery for layout.
PNG/JPEG/PDF painting can still load system fonts through the existing codec path.
Native/system context support remains compiled in default/release profiles and
runs only on explicit requests. Fixed font sources avoid catalogue discovery for
faster, reproducible native output;
see the [CLI font selection rules](../../crates/merman-cli/README.md#explicit-fonts).
Dated native-font reports retain their original runtime defaults and measurements;
they do not measure the revised default policy.

## Current evidence

The [October 21 root-cause investigation](theme_regression_root_cause_macos_2026-10-21.md)
pauses small allocation repairs and prioritizes newly introduced work against v0.9.0. Independent
history audits and full-public-E2E diagnostic interventions identify the generated CSS/XML and
bounded append protocols as important candidates: combined bypass changes Packet by -1.368 us,
ZenUML by -0.565 us and Kanban by -13.914 us. The interventions deliberately omit contracts and
are not production candidates or formal recovery. They do not support a unified writer explanation
of ZenUML. Its matched typed-parse diagnostic is only +0.340 us, while a separate default CSS
producer intervention removes 60 non-ID substitutions and changes rich E2E by -0.831 us. All
points and controls are preserved; no independent-stage or intervention deltas are added.
The same audit finds repeated compiled-theme request/index ownership and five-set coverage
reconstruction; None does not execute their nonempty work. PreparedSource Arc/metadata cleanup
is parked because a pre-redraw release checkpoint already had similar Packet/ZenUML gaps.
Next redesign these larger mechanisms with complete normal contracts, then obtain adjacent
formal E2E evidence. The latest admitted release classification remains 14/2/3 on b9.

The safe literal CSS candidate in the same root-cause report is retired: 32 fixed diagnostic
calls give Kanban 53.406 to 50.44775 us (-5.539%, -2.95825 us), below its preregistered 10%
relative screen despite passing 1 us. All four SVG identities match; 5241 renderer tests and
two new seam controls pass. Full classification-bypass diagnostics did not predict this safe
subset's materiality. Preserve the rejected patch and records; do not retain its new literal
type/macro or lower the gate. Next implement the independently registered all-family ledger
ownership repair for warm memory/work, retaining all source, terminal and error semantics and
explicit bounded cold-cache state. It does not close the None release regressions.

The registered all-family ledger ownership refactor is now kept for warm allocation/work: the
compiled family program owns one immutable request domain, local plans hold outcomes only, terminal
paths merge into one checked ledger, and coverage no longer reconstructs five sets. Across 418
stable theme cells, 416 improve all four allocation metrics and 2 are unchanged; all 380 non-empty
themed cells reduce allocation count and bytes. Across 645 site controls, 482 of 505 jointly stable
sites improve and none regresses; 140 variable sites remain inconclusive. SVG, report, callback and
error contracts match, and the full release/DOM/WASM/theme acceptance checks pass. This remains a
memory/work ownership result with an explicit O(R+B) retained-key bound, not a latency or v0.9.0
convergence claim. Run a fresh balanced timing confirmation before reclassifying release regressions.

A fresh eight-fixture diagnostic after the ledger refactor shows no material latency admission: Kanban
-0.81%, Packet -1.83%, ZenUML -0.67%, Journey -1.42%, while Flowchart, C4, Block and Sequence
remain within small diagnostic noise. Keep `e76736165` for the verified warm allocation/work
reduction; do not reclassify the v0.9.0 latency regressions. The next isolated candidate is the
ZenUML production CSS call that repeats normalized-ID classification 35 times after the public
SVG boundary has already normalized the ID.

The [October 20 JSON disposal checkpoint](json_disposal_frontier_macos_2026-10-20.md) removes
scalar values from the nonrecursive disposal worklist and keeps the next child container inline.
On the clean `b8a5f59c1` base, all 418 catalog cells across 35 logical families improve allocations;
all 505 jointly stable site controls improve without a four-metric regression, while 140 variable
controls remain inconclusive. Venn/None catalog peak falls 183387 to 180461 B, with 64152 fewer
allocated bytes; Packet/None saves four allocations and 288 B. All output/report/callback/error
controls match. Release tests pass 8101 (seven skipped), with Clippy, 37 DOM entries, optimized
full WASM, 34 theme acceptance tests, compiled pipeline discovery and native-memory smoke.
The helper also exists unchanged in v0.9.0: this is shared memory/work removal, without a latency
or theme-regression attribution. The earlier metadata-owner candidate remains rejected for its
stable Venn peak regressions; its code is absent from this checkpoint. Reevaluate it independently
against this new disposal baseline only after the root-cause priorities above. Tag-shared
editor facts and fresh-source ownership are parked. All-family release convergence remains
incomplete.

The [October 20 appearance/CSS release checkpoint](theme_v090_appearance_css_macos_2026-10-20.md)
compares production `b9a58731c` with v0.9.0 using fresh 608 A/A and 304 fixed balanced A/B
observations: 14 within the joint budget, two confirmed regressions and three inconclusive scenes,
with zero consumer contract failures. ZenUML remains +17.51% / +3.499 us and Packet +28.83% /
+2.332 us. Kanban is +18.95% / +8.521 us but remains inconclusive because its absolute A/A order
certificate is unqualified; it is not recovered. Sankey and Wardley relative bounds cross 10%.
All 76 fresh input/output inventories and physical raw replays remain exact to their receipts:
19 comparable scenes, 57 changed-output scenes with source-backed reasons. The first timing
attempt is rejected for shared-target reuse of a hook executable; its records are preserved and
none of its observations is reused. Bounded rebuild and independent physical executable admission
precede this accepted run, and both actual no-hook binaries are archived before later native work.
PR #226 is merged as `a933b02c1`, whose complete source tree equals tested `b9a58731c`; the tested
revision remains explicit. These release-range observations do not attribute latency to an
individual repair. The later JSON disposal checkpoint and rejected metadata candidate retain
their own admission boundaries; neither reclassifies this release comparison. The all-family
convergence goal remains incomplete.

The [October 20 CSS identifier checkpoint](css_identifier_borrow_macos_2026-10-20.md) borrows
unchanged identifiers inside the existing serializer. On the fresh `7e0582307` base, Packet medium /
None saves one allocation and 19 bytes; all 418 catalog cells are stable, with 132 improvements,
286 unchanged and no increases. All 645 SVG/report and ordered-measurer controls and 26 complete
errors match. Full release nextest passes 8100 tests (seven skipped), with strict Clippy, 37 fresh
archived DOM reports, optimized full WASM and 34 theme acceptance tests. A test-only formatting
failure is preserved explicitly; all five final-format supplemental stages pass. Final source
verification uses two explicit test-format overrides, preserving all other measured source and
frozen inputs. This is allocation/work removal, without a latency claim. The separate accepted release
comparison above retains its own production revision and qualification boundaries.

The [October 20 scoped appearance checkpoint](scoped_appearance_path_macos_2026-10-20.md)
defers scoped path construction until the existing scoped-assignment branch consumes it. On the
fresh post-main base `88351fe6a`, Packet medium / None saves six allocations and 72 bytes; all
418 catalog cells are stable, with 396 improvements, 22 unchanged and no increases. All 645
SVG/report and ordered-measurer controls and 26 complete errors match. Current design inputs
replace retired authoring controls explicitly; historical input equivalence is not claimed.
All 8100 full release tests pass (seven skipped), together with strict Clippy, 37 archived DOM
reports, optimized full WASM, 34 release theme acceptance tests and format/path checks. All 15
serial stages finish successfully and all 15,303 frozen source hashes remain unchanged. This is
allocation/work removal, without a timing or v0.9.0 convergence claim. CSS identifier ownership
and the full release comparison now have their separate admitted checkpoints above.

The [October 20 main-resync checkpoint](theme_main_resync_macos_2026-10-20.md)
validates merge `6badd5444` of upstream `daa7c812d`, whose tested source tree is
`e1707685fb8c9f587d0139899a5c5bc8cc148bf0`. All 8100 release tests pass (seven skipped),
together with strict Clippy, 37 DOM entries, optimized full WASM including wasm-opt, 34 release
theme acceptance tests and 245 Python performance contracts. All 13 serial command stages finish
successfully and all 19,448 frozen non-documentation source hashes remain unchanged. Fresh DOM
report copies and WASM input/artifact digests are archived with the ignored experiment records.
This checkpoint integrates prepared redraw and persistent mounting while preserving theme
ownership repairs. Historical allocation and v0.9.0 latency reports retain their original
revisions; no new timing claim or all-family goal completion follows from this merge. The private
probe migrations still require source revalidation and current exact error oracles before use.

The [October 20 main-integration checkpoint](theme_main_integration_macos_2026-10-20.md)
validates clean merge `9b4ff208f` of upstream `d249bb2dc`: 7651 release tests pass (seven skipped),
strict Clippy and 37 DOM entries pass, optimized full WASM completes, 245 Python performance
contracts pass, and 34 release theme acceptance tests pass. The independent public API boundary
and complete-feature compiled pipeline discovery also pass (459 benchmarks, 459 receipts, seven
lane groups). All 14 command stages reach successful terminal states. These are integration
correctness checks; historical allocation and release-timing evidence retain their own revisions.
The early review draft does not close the all-family goal or cross-platform qualification.
Before another adjacent experiment, migrate the private harness's retired authoring API and
specific input controls to the current theme design contract, preserving a retired/replacement
mapping and obtaining new exact error oracles. These requirements were completed for the later
scoped-appearance checkpoint above.

The [October 20 empty explicit-owner query checkpoint](empty_explicit_owner_query_macos_2026-10-20.md)
skips temporary path construction when the explicit owner set is empty, while preserving outer
Tracking/Frozen compatibility checks and the complete nonempty-set query. The preselected
Flowchart / None primary saves 40 allocations and 1344 bytes per warmed render. All 418 catalog
cells are stable and exact: 371 improve and 47 retain all four operation-growth metrics, with
no increases. Of 504 jointly stable controls, 108 improve and 396 are unchanged; 141 variable
controls remain inconclusive. All 645 output/report and independent ordered-measurer controls
and 26 errors match. All 930 focused and 7651 full release tests pass (seven skipped), together
with strict Clippy, 37 DOM entries, optimized full WASM and format/path checks. The independent
terminal review verifies current DOM reports; experiment-local report copies were not archived.
This is exact allocation/work removal, without a latency or release-convergence claim. Next
integrate current upstream theme semantics and independently evaluate the remaining scoped
appearance and ownership candidates. The all-family goal remains incomplete.

The [October 20 post-initialization full confirmation](theme_v090_post_initialization_confirmation_macos_2026-10-20.md)
compares clean `a6742779c` with v0.9.0 over all 19 preregistered same-SVG scenes. Fresh 608 A/A
and 304 fixed balanced A/B observations establish 15 within the joint budget, three regressions
and one inconclusive scene, with zero evidence-contract failures. ZenUML remains +17.18% /
+3.443 us, Packet +27.18% / +2.203 us and Kanban +18.09% / +8.198 us. Class tiny now qualifies
both A/A metrics but its relative bounds cross 10%, so it remains inconclusive. Six unqualified
absolute certificates retain their precision failures and uncapped requirements; all 19 preselect
the qualified relative certificate. Both actual sampled executables were archived before later
native work. Earlier results retain their original heads and classifications. This release-range
observation does not attribute latency changes to an individual repair or cover the typed request
API. Continue independently registered ownership/query reductions; the all-family goal is incomplete.

The [October 20 prepared compiled-theme initialization checkpoint](compiled_theme_initialization_macos_2026-10-20.md)
reuses one lazily prepared initialization per compiled plan only when host overrides are exactly
pristine. Host authority, deferred errors and per-installation detector journals retain their
original behavior. The preselected Kanban / Editor Light primary saves 2695 allocations and
602,095 allocated bytes per warmed render; all 380 themed catalog cells improve and the 38 None
cells retain all four operation-growth metrics. All 645 saved SVG/report identities, independent
ordered callback controls and 26 errors match; 503 jointly stable controls have 208 improvements,
295 unchanged and no increases, with 142 variable controls retained as inconclusive. This trades
per-request work for bounded plan retention: absolute process-live counters increase 175,285 to
284,624 bytes in themed cells; the primary's absolute process peak increases 35,463 bytes despite
its smaller operation peak growth. The constant None +12 bytes follow retained probe argument
lengths and remain unadjusted in raw evidence. All 930 focused and 7651 full release tests pass
(seven skipped), together with strict Clippy, 37 DOM entries, optimized full WASM and format/path
checks. This is warmed allocation/work removal, with no cold-start, compilation, latency or
v0.9.0 convergence claim. The separate post-initialization full19 confirmation above covers
this clean revision and the preceding XHTML change. The all-family goal remains active.

The [October 20 literal XHTML paragraph checkpoint](xhtml_plain_paragraph_macos_2026-10-20.md)
recognizes a narrow, proven plain XML paragraph before constructing its wrapper and DOM. The
original parser and all callers remain unchanged for every unproven input. The no-theme Kanban
primary saves 84 allocations and 13,849 allocated bytes, with the same peak and retained growth.
All 645 saved SVG/report/ordered-measurer controls and 26 errors match. Of 505 jointly stable
controls, 33 improve and 472 retain all four metrics; 140 variable controls stay inconclusive.
All 418 catalog cells are stable and exact within the warmup-metadata and five-saved-sample
contracts: 66 improve across Flowchart, Class, ER, Kanban, Swimlane and Agentflow in all eleven
modes, with 352 unchanged and no increases. Retained growth stays unchanged everywhere. All
131 focused and 5,613 full release tests, strict Clippy, full DOM, optimized full WASM and
format/diff checks pass. The baseline directory collision is preserved with its bounded
completion amendment, without repeating successful samples. This is allocation/work removal,
not a latency or release-convergence claim. Next evaluate the bounded ZenUML CSS assembly and
Packet sink-dispatch hypotheses separately, and prepare immutable compiled-theme initialization
owners while preserving dynamic journals, host overrides and deferred parse errors. The latest
full19 scoreboard remains unchanged until a separately frozen confirmation; the all-family goal
remains active.

The [October 20 post-ownership full confirmation](theme_v090_post_ownership_confirmation_macos_2026-10-20.md)
compares clean `9f8143a06` with v0.9.0 over all 19 preregistered same-SVG scenes. Fresh 608 A/A
and 304 fixed balanced A/B observations establish 12 within the joint budget, three regressions
and four inconclusive scenes, with zero evidence-contract failures. ZenUML remains +17.95% /
+3.584 us, Packet +28.66% / +2.310 us and Kanban +26.02% / +11.737 us. Class tiny cannot establish
joint regression despite its displayed lower bounds: its absolute A/A certificate is unqualified.
Eight failed absolute certificates retain their precision failures and uncapped sample requirements;
all 19 preselect the qualified relative certificate. Earlier receipts remain unchanged, and this
release-range observation does not attribute movement to an intermediate allocation patch. The
full report, frozen plan, registration, reviews and execution identities have verified lossless
archives. Next inspect the bounded stages and CPU samples from the same archived executables,
then remove concrete redundant work without summing stage deltas or assigning the default gaps
to compiled-theme initialization. Keep all 35 theme families in the independent catalog controls;
the all-family goal remains active.

The [October 20 Kanban bound-owner checkpoint](kanban_bound_paint_owner_macos_2026-10-20.md)
removes the second task-decision array and copied typed strings, binding palette values in the
existing owner at the original phase. Ten themed catalog cells each save five allocations and
556 bytes. No-theme medium saves 16 allocated and peak bytes through the smaller boxed artifact,
with the same allocation count. All 645 SVG/report/ordered-callback controls and 26 errors match.
Of 503 jointly stable controls, two improve and 501 retain all four metrics; 142 variable rows
remain inconclusive. All 418 catalog cells are stable and exact within their warmup-metadata and
five saved sample contracts: eleven improve and 407 retain all four metrics. Retained growth is
unchanged everywhere. All 87 focused and 5,613 full release tests, Clippy, full DOM, optimized
full WASM, formatting and independent reviews pass. This is allocation/work removal without a
clock or release-convergence claim. Next freeze the latest clean revision for a fresh full
19-scene release confirmation, then use existing stages and bounded system CPU sampling to
attribute remaining gaps. Keep unimplemented receipt and finite-fusion drafts separate until
those observations support them; the all-family goal remains active.

The [October 20 Pie CSS facts checkpoint](pie_css_facts_macos_2026-10-20.md) borrows five temporary
success values from the existing immutable binding. Each of ten themed Pie catalog cells saves
five allocations and 71 bytes; no-theme Pie is unchanged. All 645 SVG/report/ordered-callback
controls and 26 errors match. All 504 jointly stable controls retain all four memory metrics;
141 variable rows remain inconclusive. All 418 catalog cells are stable: ten improve and 408
retain all four metrics, with exact warmup result metadata and all five saved sample SVG/report
identities. Peak and retained growth are unchanged everywhere. All 75 focused and 5,613 full
release tests, Clippy, full DOM, optimized full WASM, formatting and independent reviews pass.
This is allocation/work removal without a latency or release-convergence claim. Next independently
verify Kanban's duplicated bound-paint owner using its active typed catalog primary and retain
no-theme controls unchanged; keep receipt and finite-geometry candidates separate.

The [October 20 XY empty-owner checkpoint](xy_empty_paint_owner_macos_2026-10-20.md) defers its
initial sized empty Arc until the existing active capture phase. Captured empty maps remain real
owners with pointer identity and single completion. Default XY saves one allocation and 40 bytes,
including 40 bytes of peak growth. All 645 SVG/report/ordered-callback controls and 26 errors match.
Of 506 jointly stable rows, 25 improve and 481 stay unchanged; no memory metric increases and
139 variable rows remain inconclusive. All 418 catalog cells are stable and exact, with eleven
XY improvements and 407 unchanged. All 106 focused and 5,613 full release tests, Clippy, full DOM,
optimized full WASM, formatting and independent reviews pass. This is allocation/work removal
without latency or release-convergence claims. Next independently verify Pie's temporary CSS
facts against the source-selected active typed catalog cell; retain no-theme as an unchanged
control and keep receipt/palette/finite-geometry candidates separate.

The [October 20 empty GitGraph palette checkpoint](gitgraph_empty_palette_css_macos_2026-10-20.md)
skips pure CSS construction only when all eight assignments are absent. The caller's empty
write/checkpoint remains. Default GitGraph saves one allocation and 16 bytes; its no-theme catalog
cell saves one and 19 bytes due to the different fixed ID. All 645 SVG/report/ordered-callback
controls and 26 errors match. Of 503 jointly stable rows, one improves and 502 stay unchanged;
142 variable rows remain inconclusive. All 418 catalog cells are stable and exact: one improves,
417 stay unchanged, including all ten active GitGraph presets. Peak/retained growth stay unchanged.
All 121 focused and 5,612 full release tests, Clippy, full DOM, optimized full WASM, formatting and
independent reviews pass. This is a small allocation/work reduction without a clock claim.
Next independently verify the XY empty paint owner, preserving captured empty-map identity,
foreign-receipt rejection and single completion; keep other typed owners and finite-check fusion
separate and retain the unresolved release-distance goal.

The [October 20 ZenUML font checkpoint](zenuml_default_font_owner_macos_2026-10-20.md) borrows
the static CSS default while retaining owned normalization for every authored Some font.
Both independent primaries save one allocation and 23 bytes. All 645 SVG/report/ordered-callback
controls and 26 errors match. Of 504 jointly stable rows, 54 improve and 450 stay unchanged;
all four memory metrics are nonincreasing, peak/retained growth stay unchanged and 141 variable
rows remain inconclusive. All 418 catalog cells are stable and exact, with eleven ZenUML
improvements and 407 unchanged. Real empty and quoted font controls retain all four metrics.
All 49 focused and 5,612 full release tests, Clippy, full DOM, optimized full WASM, formatting and
independent reviews pass. This removes a temporary owner, without quantifying its share of the
3.561 us release difference or reclassifying latency. Next evaluate empty GitGraph palette CSS
construction independently; retain the caller's empty write/checkpoint and all active palette
behavior. XY paint, Pie CSS and the narrower ZenUML finite-check fusion remain separate candidates.

The [October 20 shared root-font checkpoint](common_root_font_owner_macos_2026-10-20.md) removes
the redundant initial font clone while retaining configured-root selection and explicit empty
values. Info, Error and Kanban each save one allocation and 39 bytes; GitGraph saves two through
construction and its existing subsequent clone. All 642 SVG/report/ordered-callback controls and
26 errors match. Of 502 jointly stable rows, 201 improve and 301 stay unchanged, with no increase
in any memory metric; 140 variable rows remain inconclusive. All 418 catalog cells are stable
and exact: 209 cells across 19 families improve and 209 stay unchanged. Packet remains unchanged.
All 708 focused and 5,612 full release tests, Clippy, full DOM, optimized full WASM, formatting and
independent reviews pass. This is allocation/work removal, not release-distance attribution or
latency admission. Next independently verify the ZenUML default CSS font owner, keeping its
Some(empty)/normalization behavior and the finite-geometry fusion candidate separate.

The [October 20 Quadrant checkpoint](quadrant_point_binding_macos_2026-10-20.md) removes the
retained point-binding array and its independent traversal/settings projection. Borrowed source,
class and prepared values flow into the necessary layout loop, with one final string copy.
All three primaries save twelve allocations: default medium saves 384 bytes, empty authoring
332 bytes and typed radius/fill 319 bytes. All 642 SVG/report/ordered-callback controls and 26
errors match. Of 501 jointly stable rows, 35 improve and 466 stay unchanged; all four memory
metrics are nonincreasing and 141 variable rows remain inconclusive. All 418 catalog cells are
stable and exact, with eleven Quadrant improvements and 407 unchanged. Forty-five focused and
5,612 full release tests pass, as do Clippy, full DOM, optimized full WASM, formatting and
independent source/raw/report reviews. This removes duplicated ownership/work without a latency
claim or historical release reclassification. Next independently measure the shared root-font
owner, preserving separately configured and explicitly empty root values; keep the default
GitGraph, XY paint, Pie CSS and ZenUML candidates separate.

The [October 19 terminal receipt checkpoint](terminal_receipt_value_state_macos_2026-10-19.md)
replaces copied Info/Error CSS observations with existing sticky comparison state and borrows
Treemap's expected title fill. All three primaries save three allocations: Info/Error each save
165 bytes and the active Treemap title saves 48 bytes. All 632 SVG/report/ordered-callback controls
and 26 errors match. Of 492 jointly stable rows, 64 improve and 428 stay unchanged; no memory
metric increases and 140 variable rows remain inconclusive. All 418 catalog cells are stable and
exact, with 22 Info/Error improvements and 396 unchanged. Seven focused and 5,612 full release
tests pass, as do Clippy, the full DOM gate, optimized full WASM and formatting checks. Independent
source, raw-evidence and report reviews find no blocker. This is allocation/work removal, not
a timing or release-convergence claim. Next independently measure Quadrant's duplicated point
binding owners, retaining source/class/config precedence, exact tokens, all checkpoints and the
full catalog; the remaining ZenUML font owner and finite-check fusion are separate candidates.

The [October 19 full same-SVG joint confirmation](theme_v090_joint_confirmation_macos_2026-10-19.md)
completes all 19 preregistered scenes against v0.9.0: 13 are within the joint budget, three
confirm regression and three remain inconclusive. Both runners complete eight fresh A/A pairs;
the frozen relative certificates select eight balanced full-cohort A/B pairs. All 912 observations
retain matching output postflights, and final runner/source identities pass with zero contract
failures. Zenuml remains +17.81%/+3.56 us, Packet +27.51%/+2.23 us and Kanban +27.58%/+12.44 us.
Class tiny, Sankey and Wardley cross the relative threshold and remain inconclusive. Seven failed
absolute certificates retain their uncapped requirements; qualified relative certificates can
still establish joint non-regression. Compressed raw observations and the pre-A/B plan are
archived with digests. This is a release observation, not individual-change attribution or
candidate admission. Next independently measure the registered Info/Error/Treemap terminal
receipt state candidate, followed by the reviewed ownership queue; keep all-family convergence
active and the 57 changed-SVG scenes plus 418 catalog cells separate from these release ratios.

The opt-in [joint release confirmation protocol](BENCHMARKING.md#statistics-and-decisions)
now implements the independently reviewed, preregistered `joint_non_regression_v1` method.
Both runners complete eight fresh A/A pairs; one qualified metric is selected before A/B, and
the entire same-SVG public cohort must complete its fixed balanced schedule. An exclusive
pre-A/B calibration plan records the full scope and identities. The protocol fixes 95% confidence
and the 10%/1000 ns budget, reports conditional decision error rather than simultaneous coverage
of all interval endpoints, and never assesses improvement or candidate admission. All 149
performance contracts, 16 baseline-manifest tests, 51 native-memory contracts and 29 driver
contracts pass; independent source and report reviews find no remaining blocker. No new timing
or historical reclassification is part of this tooling change. The separately preregistered
fresh full-cohort experiment is reported above; changed-output rows and typed catalog controls
remain separate evidence.

The comparison helper now implements the documented joint complement: either qualified regression
upper bound below its threshold proves non-regression, with the mirrored rule for non-improvement.
Positive regression/improvement still needs both strict bounds. Synthetic contracts reproduce
nine failures before the two-line fix; all 89 statistics/runner/report tests pass afterward,
including equality, crossing intervals, sample-count guards and unchanged dual-metric A/A gates.
This is a method correction, not new timing or historical receipt reclassification. Generic A/A
still imposes the stronger two-metric precision prerequisite. Next independently review and
preregister a versioned release joint-convergence protocol before collecting fresh full-scope
v0.9.0 A/A and A/B; retain the separate terminal receipt ownership candidate.

The [October 19 empty source-config checkpoint](empty_source_config_owner_macos_2026-10-19.md)
reuses the existing source handle where private consumers only read an empty JSON value. All
632 successful controls retain exact SVG/reports/ordered measurer traces, and 26 errors match.
Among 491 jointly stable rows, 459 save four allocations and 240 bytes, 32 nonempty-source rows
stay unchanged and none regress; 141 variable rows remain inconclusive. All 418 catalog cells
are stable, exact and improve by four allocations/240 bytes. Expanded core/public/renderer tests,
Clippy, minimal core features, full DOM and optimized full WASM pass. This removes fresh owner
construction and its extra destruction stack, without a clock claim. Next correct the benchmark
classifier's joint complement independently, then simplify terminal receipt value ownership;
new release calibration must preserve old observations and the complete family scope.

The [October 19 Treemap CSS checkpoint](treemap_css_receipt_macos_2026-10-19.md) borrows four
intermediate success values from the immutable prepared plan. All 632 successful controls retain
exact SVG/reports/ordered measurer traces, and 26 errors match. Among 490 jointly stable rows,
55 Treemap controls improve and none regress; the same 142 variable rows remain inconclusive.
Default medium saves four allocations and 51 bytes. All 418 catalog cells are stable and exact:
eleven Treemap cells improve and 407 stay unchanged. Renderer tests, Clippy, full DOM and optimized
full WASM build pass. This removes theme-refactor ownership without a clock claim. Next test the
shared empty source-config owner, then Info/Error and Treemap title terminal state; preserve
configuration metadata semantics, each proof state machine and successful completion uniqueness.

The [October 19 Sequence foreground checkpoint](sequence_foreground_owner_macos_2026-10-19.md)
borrows final raw/typed colors, retains the existing source decisions and removes an unconsumed
BoxTitle declaration. All 584 successful controls retain exact SVG/reports/ordered measurer traces,
and 26 errors match. Among 442 jointly stable rows, 95 Sequence controls improve and none regress;
142 variable rows remain inconclusive. Default medium saves sixteen allocations and 550 bytes;
the combined typed control saves twenty-one and 563 bytes. All 418 catalog cells are stable and
exact, with eleven Sequence improvements and 407 unchanged. Renderer tests, Clippy, full DOM and
optimized full WASM build pass. This removes ownership spanning math preparation and theme binding
without a clock claim. Next evaluate Treemap's temporary CSS success values, then independently
simplify Info/Error receipt state while retaining its completion uniqueness and proof semantics.

The [October 19 shared CSS checkpoint](common_css_borrowed_receipt_macos_2026-10-19.md) borrows
three successful font facts from their immutable prepared owner in Info/Error/Quadrant/XY. All
584 successful controls retain exact SVG/reports/ordered measurer traces, and 26 errors match.
Among 444 jointly stable rows, 102 improve by three allocations and 48–129 bytes, with no increase
in any memory metric; 140 variable rows remain inconclusive. All 418 catalog cells are stable and
exact: 44 affected cells improve and 374 remain unchanged. Renderer tests, Clippy, full DOM and
optimized full WASM build pass. This deletes theme-refactor ownership without a latency claim.
Next evaluate Treemap's temporary success values and Sequence's duplicate foreground owners as
independent allocation experiments, preserving existing source facts and actual writer fragments.

The [October 19 empty Sequence CSS checkpoint](sequence_empty_css_groups_macos_2026-10-19.md)
skips empty declarations before constructing groups that were always discarded. All 488 successful
controls retain exact SVG/reports/ordered measurer traces, and 26 inherited errors match. Among
347 jointly stable rows, 34 no-theme Sequence rows each save ten allocations and 788 bytes, with
unchanged peak and retained growth; 141 variable rows remain inconclusive. All 418 catalog cells
are stable and exact; only no-theme Sequence improves and the other 417 cells are unchanged.
Renderer tests, Clippy, full three-mode DOM and optimized full WASM build pass. This removes
theme-refactor work without a latency claim. Next borrow the shared Info/Error/Quadrant/XY CSS
success receipt from its existing prepared owner, then evaluate the independent Treemap receipt
candidate. Release convergence and the remaining source-attribution gaps stay open.

The [October 19 release SVG audit](theme_release_svg_audit_macos_2026-10-19.md) captures and
independently replays all 76 public rendering fixtures across 35 logical families. All 152 raw
files match frozen side-specific SHA/length/element identities; nineteen remain exact and
fifty-seven changed. Every changed fixture now has a scoped difference inventory. A later source
follow-up attributes five Flowchart edge/label cases to the Mermaid 12.2 label-aware terminal gate
without a runtime rejection trace. Swimlane's 66 geometry fields reconcile with the newly applied
minimum node width and existing LR formulas; eleven Mindmap solver centers remain independently
unsolved. ID/order, native CSS, XML spelling and configured-owner differences stay explicit. No changed row is
promoted to same-SVG or latency convergence. Next remove already empty Sequence CSS groups and
borrow shared/Treemap CSS success facts from their existing owners, as independent allocation
experiments. Full new typed presets still require a separate historical-function contract.

The [October 19 bounded-output experiment](bounded_active_rejected_macos_2026-10-19.md) rejects
the three-attribute active-state/cold-error candidate under its preregistered diagnostic screen.
Two fixed public AB/BA pairs observe Packet −3.15%/−0.326 us and Kanban −4.98%/−2.824 us;
neither clears both primary thresholds. No formal confirmation or integration followed. Machine
code does remove the hot call, which is insufficient for admission. Next capture and reconcile
all 76 tag/current public SVG outputs: nineteen exact and fifty-seven changed rows remain in
scope, and the existing 418 new-preset cells do not establish legacy release equivalence.

The [October 19 Sequence checkpoint](sequence_css_owner_macos_2026-10-19.md) removes duplicate
default and typed CSS scalar owners by borrowing the existing immutable prepared artifact.
All 488 successful controls retain exact SVG/reports/ordered measurer traces, and 26 inherited
public errors match. Of 347 jointly stable memory rows, 95 improve and none regress; 141 variable
rows remain inconclusive. All 418 catalog cells are stable and exact; eleven Sequence cells save
13–14 allocations and 497–571 bytes, with the other 407 unchanged. Renderer tests, Clippy, full
three-mode DOM and optimized full WASM build pass. This is allocation/work evidence. Next test
the shared writer's successful-state check and cold error conversion with calibrated public
timing; frozen machine code confirms an actual call and stack setup on the successful path.
Sequence's seven foreground/declaration copies remain a separate ownership candidate.

The [October 19 State checkpoint](state_terminal_owner_macos_2026-10-19.md) replaces the 104-cell
owned terminal CSS table with fixed borrowed selection and 22 final-winner source bits. All forty
surfaces, truthy/nullish fallbacks, gradient scoping and missing-pair behavior remain. All 402
public outputs/reports/ordered measurer traces match; 262 jointly stable memory rows never
regress, while 134 State and six Flowchart rows remain inconclusive. Twenty-six inherited public
parse errors also match. All 418 catalog cells remain stable and exact; eleven State cells save
149–173 allocations and 7,376–8,088 bytes each. Renderer tests, Clippy, full DOM and optimized full
WASM build pass. Sequence's duplicate default and typed CSS owners are the next independent
candidate; the shared writer's active-state branch still needs a calibrated latency experiment.

The [October 19 Radar checkpoint](radar_series_owner_macos_2026-10-19.md) removes a duplicated
12-color palette from the CSS binding and constructs it once in the final series plan. All 262
successful public controls retain exact SVG/reports/ordered measurer traces, and 26 public parse
errors remain identical. Across 256 jointly stable memory controls, 32 improve and none regress;
six variable Flowchart controls remain inconclusive. All 418 catalog cells retain output and
metadata; eleven Radar cells each save twelve allocations and 372–766 bytes. Renderer tests,
Clippy, full three-mode DOM and optimized full WASM build pass. This removes source-backed theme
ownership overhead without a latency claim. The next larger ownership candidate is State's
materialized terminal paint table; preserve its distinct truthy/nullish fallbacks and source bits.

The [October 19 Packet/Kanban stage attribution](packet_kanban_stages_macos_2026-10-19.md)
retains six fixed diagnostic rows and four CPU profiles from the frozen release-comparison
executables. Exact SVG emission still differs in cost; parse/layout projections differ and
Kanban moved stylesheet work across stages, so stage deltas cannot be added or treated as new
theme work. Current emission samples identify XML fragment handling and active-state error
checks as bounded next experiments. The writer already uses bounded geometric growth; changing
its exact reservation would risk the allocation bound. No new latency or convergence claim is made.

The [October 19 TreeView checkpoint](treeview_optional_proof_macos_2026-10-19.md) removes source-tree
and icon counting plus terminal theme proof work when no theme was requested. Some recipes,
including empty and unsupported recipes, retain their original proof path. All 231 public controls
preserve exact SVG/reports/measurer traces; 224 jointly stable memory rows never regress and seven
remain inconclusive for memory. All 418 catalog cells preserve output and metadata; only no-theme
TreeView improves, saving three allocations and 79 bytes. This is allocation/work evidence only.

The [October 19 calibrated release observation](theme_v090_confirmation_macos_2026-10-19.md)
confirms remaining default public E2E gaps against v0.9.0: Packet +27.33%/+2.196 us and Kanban
+26.15%/+11.712 us. Each stable row has 32 fresh AB/BA pairs after eight A/A pairs per runner;
both exceed the joint 10% and 1 us goal. C4's historical absolute calibration fails and requires
44 pairs above the registered cap of 32, so it remains inconclusive with no A/B ratio. All three
inputs and SVG identities match, and there are no evidence-contract failures. Other same-SVG and
changed-output rows remain in scope. Allocation cleanup has not yet established release convergence.

The [October 19 C4 color checkpoint](c4_consumer_colors_macos_2026-10-19.md) removes color preparation
for unused element types while retaining all stylesheet fonts and per-type reuse. All 206 public
controls preserve exact SVG/reports/ordered measurement traces; 200 jointly stable rows never
regress and six variable Flowchart rows remain inconclusive for memory. All 418 catalog cells
preserve output and metadata; eleven C4 cells each save 144 allocations and 2,775 allocated bytes.
The temporary-map candidate was rejected; the retained design reuses existing style entries.
This is allocation/work evidence, with formal latency convergence still open.

The [October 19 Packet static-style checkpoint](packet_static_style_macos_2026-10-19.md) removes
ten immutable fallback String allocations from default Packet rendering. All 197 public controls
preserve SVG/reports/ordered measurement traces; 190 jointly stable rows never regress and seven
variable Flowchart rows remain inconclusive. All 418 catalog cells preserve exact output and
metadata; 11 Packet cells each save ten allocations and 45 bytes. Renderer tests and Clippy pass.
The invalid shared-target catalog baseline is retained but excluded; the accepted catalog comparison
uses an isolated clean baseline. This is allocation evidence only, not latency convergence.

The [October 19 PR checkpoint](theme_pr_checkpoint_macos_2026-10-19.md) records integration with
`origin/main` at `252752f70`, the ER prepared-geometry/consumer-style merge and fresh checks on
product revision `08688eaf5`. Release native tests pass 8,275 cases; Clippy, private theme acceptance,
WASM, full DOM, representative feature/dependency checks, benchmark discovery, native-memory smoke,
Node and legal/security checks pass. Earlier incremental allocation and timing receipts retain
their original revisions; the upstream ER cache changes measurement callback count and phase.
The all-family convergence goal remains active while the first refactor batch enters review.

The [October 19 color serialization checkpoint](theme_color_serialization_macos_2026-10-19.md)
removes temporary numeric/hex component Strings from shared theme color formatting. All 17,091
direct color operations, 178 public-render controls and 418 catalog cells preserve exact output
and errors/reports/measurement traces as applicable. Stable memory rows never regress; default
Kanban saves 36 allocations and 222 bytes per complete render, and 67 catalog cells improve across
eight logical families. Peak and retained growth remain unchanged. Native 7,624 tests, private
theme acceptance, Clippy, optimized full WASM, smoke and all-family DOM pass. This is allocation
evidence, not a latency or theme-regression attribution: the serializer is unchanged since v0.9.0.

The [October 19 v0.9.0 refresh](theme_v090_same_svg_refresh_macos_2026-10-19.md) retains all 19
same-SVG E2E scenes in 76 fixed diagnostic calls. Seven rows exceed both 10% and 1 us: Class tiny,
C4, Sankey, Zenuml, Packet, Kanban and Wardley. Packet/Kanban/Zenuml observe
+32.78%/+28.46%/+18.47%; the complete table and separate changed-output lanes remain recorded.
These observations have no A/A calibration, formal recovery admission or theme-only attribution.
Kanban CSS work moved from emission to preparation across releases, so its private layout gap
cannot by itself identify added work. The shared color serializer's temporary component strings
are the next independent allocation candidate.

The [October 19 default font checkpoint](theme_default_font_fastpath_macos_2026-10-19.md) borrows
configured font strings and skips CSS parsing only for the two exact built-in Mermaid default
spellings; all custom and malformed values retain the parser and safety validation. All 178 public
controls preserve SVG, reports and ordered measurement traces; 84/171 jointly stable memory rows
reduce allocation count and bytes, with seven variable Flowchart rows remaining inconclusive. All
418 cells across 35 logical families and 10 presets plus no-theme preserve output and metadata; 122
reduce allocation count and bytes. Peak and retained growth are unchanged. This is allocation/work
evidence, not a calibrated latency claim.

The [October 19 family preparation checkpoint](family_theme_preparation_macos_2026-10-19.md)
removes discarded/duplicated Class, Pie, Sankey, and no-theme Quadrant preparation. All 178
public controls preserve exact SVG/report/measurement traces; 171 jointly stable rows have no
memory-metric regression and seven variable Flowchart rows remain inconclusive. All 418
family/preset/default cells preserve output and metadata; 24 reduce allocation count and bytes,
with no peak or retained-growth regressions. Candidate 1 was rejected after ER regressed by one
allocation. This is allocation/work evidence only; no calibrated latency claim was run.

The [October 18 indexed-key checkpoint](theme_indexed_keys_macos_2026-10-18.md) removes repeated
formatting of fixed Git, cScale, palette and default-restoration keys during non-exact theme
materialization. All 21 non-empty, non-palette public SVG rows reduce allocations and bytes; the
default theme saves 134 allocations/2,233 bytes and the other themes save 15–55 allocations/
137–1,024 bytes. Empty snapshot and explicit palette controls are unchanged, and all 60 SVG/report
artifacts compare byte-for-byte equal. Theme tests (139), public merman tests (396), Clippy,
formatting and release build pass. This is an allocation/work reduction with no calibrated latency
claim and no retained state.

The [October 18 staged-empty checkpoint](staged_empty_lazy_macos_2026-10-18.md) caches the exact
ordered result for empty, owner-free compatibility execution of Base, Dark, Forest and Neutral.
It preserves generated-theme insertion order by running the existing staged executor once; the
generated sorted snapshot is intentionally not used. Across 178 public SVG/report/measurer
controls and the 418-cell matrix covering 35 logical families and 10 presets, output, diagnostics,
status and admission are exact. 380 preset cells reduce 851–1,070 allocations and 56–75 KB per
operation; 38 no-theme cells are unchanged and retained growth matches everywhere. Four fixed
cache entries retain about 281 KB after first preparation; subsequent preparations have zero
retained growth. Native, Clippy, theme, WASM, smoke, DOM and format gates pass. This is an exact
allocation/work-removal admission with no calibrated latency claim; first-request timing remains
open. Nonempty, owner-bearing, invalid, bounded and cancellation paths retain the full executor.

The [October 18 root-outcome checkpoint](root_theme_outcomes_macos_2026-10-18.md) removes cloned
root-report inventories/residuals and repeated self-coverage reconstruction, retaining canonical
writer outcomes, frozen counts/capability union and output-mutation invalidation. All 178 public
SVG/report/ordered-measurer controls and 152 benchmark SVG identities match. Of 173 stable memory
inputs, 30 save 12 allocations/672 bytes and 143 stay unchanged; five variable Flowchart inputs
remain inconclusive. All 418 catalog cells preserve SVG/reports: 380 preset cells save 12–16
allocations/672–1,048 bytes, all 38 no-theme cells stay unchanged, 25 peak-growth values decrease
and retained growth always matches. Native 8,254 tests, Clippy, qualification, WASM, smoke, DOM and
format pass. Fixed adjacent E2E diagnostics range from -0.54% to +3.83%, without calibrated
latency admission. The independent root-report candidate is complete.

The [October 18 v0.9.0 refresh](theme_v090_same_svg_refresh_macos_2026-10-18.md) measures all 19
same-SVG E2E scenarios across 15 logical families in 76 fixed diagnostic calls. Eight rows exceed
both 10% and 1 us: Class tiny, Pie, Sankey, Quadrant, Zenuml, Packet, Kanban and Wardley.
Packet/Kanban/Zenuml observe +42.83%/+29.96%/+19.44%; the other rows remain in the complete table.
The 57 changed-SVG rendering fixtures and three preprocessing-only fixtures remain separate.
These uncalibrated release-range observations do not isolate theme costs or close the active goal.
Next attribute actual consumers in the flagged rows, with configured duplicate CSS normalization
kept as a separate policy/order hypothesis.

The [October 18 root style checkpoint](root_style_borrow_macos_2026-10-18.md) borrows ordinary
root styles and deferred static suffixes until actual custom-property composition needs ownership.
All 178 SVG/report/measurer controls and 152 benchmark SVG identities match. Of 172 stable memory
inputs, 168 reduce allocation and four remain unchanged; six variable Flowchart rows are inconclusive.
Default Zenuml saves one allocation/20 bytes. All 418 compiled-preset/default coverage cells across
35 logical families preserve SVG and reports: 407 save one allocation, 11 Usecase cells stay unchanged;
all peak/retained metrics match. Partial-support statuses are preserved, not treated as Verified.
Native 8,253 tests, Clippy, theme qualification, optimized full WASM, smoke, DOM and format pass.
Five-family E2E diagnostics range from -1.52% to +0.81%, with no calibrated latency claim. Separate
same-SVG v0.9.0 Zenuml context is 20.5995 → 24.0605 us (+16.80%). The subsequent
[root-outcome checkpoint](root_theme_outcomes_macos_2026-10-18.md) completes the renderer-private
report inventory/self-coverage cleanup while preserving actual writer outcomes and public evidence.

The [October 18 Engine ownership and scope checkpoint](engine_borrow_theme_scope_macos_2026-10-18.md)
removes redundant Engine clones through an internal parse-stage borrow; explicit request themes
retain an isolated engine. This is an ownership refactor: all four memory metrics stay unchanged
for 172 stable inputs; six variable Flowchart rows remain inconclusive. All 178 public SVG/report/
measurer controls, 152 benchmark SVG identities, native 8,253 tests, Clippy, theme, WASM and DOM
gates pass. Five-family E2E diagnostics range from -1.16% to +0.60%, without a calibrated speedup.
Separate same-SVG v0.9.0 Zenuml context is 20.3045 → 23.9505 us (+17.96%). The active goal below
covers all 35 logical families and 10 public presets, not only previously timed default requests.
Root style composition is completed by the [root style checkpoint](root_style_borrow_macos_2026-10-18.md);
explicit root-report rebuilding is completed by the
[root-outcome checkpoint](root_theme_outcomes_macos_2026-10-18.md).

The [October 18 root viewport checkpoint](root_viewport_state_macos_2026-10-18.md) narrows
post-write root state to `Option<ViewBox>`, avoiding a complete viewport-plan clone and retaining
only geometry consumed by canvas application. Root sizing, provenance, deferred finalization,
resource checks and cancellation remain. All 178 public SVG/report/custom-measurer pairs and 152
benchmark SVG identities match. Of 171 stable memory inputs, 156 reduce allocation and 15 remain
unchanged; seven variable Flowchart inputs remain memory-inconclusive. Default Zenuml saves four
allocations/46 bytes (471 → 467); stable peak growth never increases and retained bytes match.
Native 8,252 tests, Clippy, theme qualification, optimized full WASM, 37-diagram smoke and all-family
DOM pass. Forty fixed adjacent diagnostic calls observe Zenuml/Packet/Timeline/State/ER public E2E
changes of -0.94%/-0.28%/+0.29%/-0.01%/-0.82%, with no calibrated latency claim. Fresh same-SVG
v0.9.0 Zenuml context is 20.2635 → 23.7765 us (+17.34%); error/resource contracts differ, and this
is neither all-family evidence nor theme-only attribution. A separate consumer audit identifies
an extra Engine clone between the public facade and Operation, on the pipeline E2E path. The
subsequent [ownership refactor](engine_borrow_theme_scope_macos_2026-10-18.md) removes it. Root style
composition is subsequently addressed by the [root style checkpoint](root_style_borrow_macos_2026-10-18.md);
these ownership changes are not assigned the complete remaining release gap.

The [October 18 Zenuml source-configuration checkpoint](zenuml_source_namespace_macos_2026-10-18.md)
resolves the actual `themeVariables.zenuml` object once for both typography and source colors.
Missing/non-object namespaces return the same defaults, replacing 36 full role-path requests with
one namespace lookup; explicit empty strings, non-string fallback and raw activation-background
priorities remain. The unused stored `surfaceColor` entry is removed while retaining it as a fallback input.
All 86 public SVG/report/custom-measurer controls and 152 benchmark SVG identities match. Six
configured inputs reduce allocation, 80 are unchanged; default allocations remain 471, and peak/
retained bytes never increase. Native 8,252 tests, Clippy, theme qualification, optimized full WASM,
37-diagram smoke and all-family DOM pass. Fixed adjacent diagnostics observe Zenuml preparation
4.6143 → 3.8969 us (-15.55%) and public E2E 24.7250 → 24.1535 us (-2.31%), with Packet/Timeline
E2E -0.38%/-0.64%. These are not calibrated latency claims. Same-SVG v0.9.0 Zenuml E2E context is
20.155 → 24.220 us (+20.17%), without an all-family or theme-only cause. The subsequent
[root viewport checkpoint](root_viewport_state_macos_2026-10-18.md) removes unused post-write plan
ownership. The extra intermediate facade Engine clone is subsequently removed by the
[Engine ownership refactor](engine_borrow_theme_scope_macos_2026-10-18.md).
Previous XML A/A remains inconclusive and is not retried.

The [October 17 XML legal-whitespace checkpoint](xml_legal_whitespace_macos_2026-10-17.md)
lets exact XML-legal TAB/LF/CR use the existing bulk classifier instead of forcing complete chunks
through scalar state processing. Actual Zenuml CSS scalar bytes fall from 2,818 to zero; 507 paired
byte/append/observation controls match, as do all 74 public SVG/report/allocation/measurer cases
and 152 benchmark SVG identities. The first range-pattern implementation is rejected for
Kanban/Timeline public E2E regressions (+10.72%/+18.95%): it lost native vectorization. The retained
equivalent non-short-circuit predicate restores NEON on this host. Fresh adjacent diagnostics
observe Zenuml emission/E2E -25.09%/-9.66%, with Packet/Kanban/Timeline E2E +3.29%/+0.19%/+0.99%.
These are not calibrated latency claims. Same-SVG v0.9.0 Zenuml E2E context is 19.977 → 24.973 us
(+25.01%), without theme-only attribution. Native, Clippy, private theme, optimized full WASM,
37-diagram smoke and all-family DOM pass. Preparation samples put 19.66% under source-style
construction; the subsequent [source-configuration checkpoint](zenuml_source_namespace_macos_2026-10-18.md)
addresses that repeated path ownership. The subsequent
[root viewport checkpoint](root_viewport_state_macos_2026-10-18.md) removes unused post-write plan
ownership; the separate [Engine borrow](engine_borrow_theme_scope_macos_2026-10-18.md) removes its
redundant facade clones. Previous A/A remains inconclusive and
is not retried.

The [October 17 Zenuml CSS encoding checkpoint](zenuml_css_encoding_macos_2026-10-17.md) repairs a double XML-encoding bug in configured Zenuml stylesheet values. CSS generation now keeps logical values and the existing `XmlTextOutput` performs the sole XML encoding; font normalization, CSS structure, resource limits, cancellation and error ownership remain unchanged. Four public regressions cover quoted/ampersand/entity/Unicode/style-closing font values, raw color character data, forbidden XML scalars and the corrected exact SVG byte ceiling. The focused Zenuml/XML suite passes 49 tests. Across 74 public controls, 57 SVGs match exactly (39 reduce allocation count/bytes, 18 unchanged); 17 change CSS text only, while parsed structure, attributes, geometry, reports and ordered measurement callbacks remain equivalent. Default Zenuml falls from 564 to 471 allocations and 153,368 to 152,840 allocated bytes, with unchanged peak/retained bytes. No calibrated latency claim is made: two AB/BA diagnostics observe Zenuml end-to-end -12.06%, Packet +1.15% and Timeline +0.31%. Against the frozen v0.9.0 same-output context, Zenuml end-to-end is 20.087 → 27.537 µs (+37.09%); this does not isolate themes or claim release-gap recovery. Native, Clippy, theme, WASM, smoke, DOM and format gates pass. Raw evidence remains under `target/bench/experiments/zenuml-css-encoding-20261017/`.

The [October 17 Zenuml ownership checkpoint](zenuml_style_ownership_macos_2026-10-17.md) removes
per-message font copies and moves all three single-title owners' unused predicates behind theme
presence. All 59 public SVG/report and custom-measurer trace pairs match; 35 inputs reduce
allocation and 24 remain unchanged, with identical peak growth and retained bytes. Default
Zenuml saves six allocations/150 bytes; 128 ordinary messages save 256/6,400 bytes. Treemap/Venn
save four/two allocations on their default no-theme paths. Fixed adjacent diagnostics observe
E2E changes of +1.27%/-0.36%/-0.38% for Zenuml/Treemap/Venn, with Timeline control +0.12%;
no confirmed latency claim. Fresh same-SVG Zenuml tag context remains +52.58%, without a theme-only
attribution.
A CSS Display-wrapper candidate is rejected because repeated placeholders rerun entity decoding
and increase default/empty allocations. The subsequent [CSS encoding checkpoint](zenuml_css_encoding_macos_2026-10-17.md)
fixes the separate double-encoding defect with logical CSS values and one terminal XML owner; its
changed-output resource/byte boundaries are reported separately. The previous XML A/A failure
remains inconclusive and is not retried.

The [October 17 XML chunk-classification checkpoint](xml_chunk_classification_macos_2026-10-17.md)
reuses the existing full chunk classification to omit the ordinary-text delimiter search, without
buffering or added state. All 77 public SVG/report inputs preserve all four allocation metrics;
152 adjacent SVG identities match. Ten-row adjacent diagnostics observe Packet/Kanban public E2E
changes of -2.47%/-3.52%, with Timeline +1.06%; these are not calibrated latency improvements.
The fresh 64-invocation A/A attempt is inconclusive: Packet base's identity interval excludes zero,
blocking both rows before A/B despite Kanban calibrating. No retries or confirmed latency claim.
Fresh diagnostics across all 19 exact-SVG-matching v0.9.0 E2E rows leave Packet/Kanban/Timeline
at +37.00%/+29.55%/+7.54%; Zenuml remains +54.40%, while Class medium is +1.69%. No all-family
average, equivalent error/resource contract or theme-only cause is claimed. Changed-output rows
remain separate. The ownership checkpoint above addresses the measured Zenuml font copies and
single-title predicates; the subsequent CSS encoding and XML legal-whitespace checkpoints above
address their independent encoding questions.
Keep Packet/Kanban writer and actual preparation costs separate. Unused post-write root
viewport-plan ownership is addressed in the [root viewport checkpoint](root_viewport_state_macos_2026-10-18.md);
Kanban temporary label/font copies remain a separate allocation candidate.

The [October 17 remaining-cost diagnosis](remaining_render_cost_macos_2026-10-17.md) supplied the
preceding writer experiment. Inclusive Packet/Kanban emission samples placed 34.55%/52.32% beneath
XML text writing, including sink descendants. Fixed-byte fragment probes showed sensitivity and a
hostile-suffix counterexample to unconditional coalescing. The implemented classification reuse
above does not add buffering; append/error boundaries rule out unconditional coalescing within
this contract. Default per-element theme certification and complete terminal XML reinspection
are absent. The diagnosis itself changes no production code.

The [October 17 Packet/Kanban preparation checkpoint](packet_kanban_preparation_macos_2026-10-17.md)
removes requestless Packet compatibility snapshots and configured-style declaration roundtrips,
reuses Kanban's diagram font in CSS, and delays unused typography fallbacks. Its admission claim
is exact public-operation allocation reduction: 77 SVG/theme-report inputs match, 69 reduce
allocation count and bytes, eight remain unchanged, and none increase peak growth or alter retained
bytes. Default Packet saves only one allocation and 16 bytes; Kanban saves six and 187 bytes.
This closes the small preparation-ownership cleanup, not the remaining release-range latency gap.

The [October 17 shared XML output checkpoint](shared_xml_output_macos_2026-10-17.md) removes
duplicate plain-text cancellation observation and uses bounded bulk searching at the XML owner.
All 67 public SVG/report/allocation controls and 152 cross-family SVG identities match. Four-pair
diagnostics observe Timeline render/public E2E at 24.320/51.725 -> 14.560/42.943 us; Packet and
Kanban public E2E decrease by 5.46%/8.66%, with small Class/Flowchart changes. The separate
96-invocation A/A attempt is inconclusive: Kanban's 1.483 us absolute noise exceeds its 1 us gate,
blocking the entire three-row confirmation set. No calibrated latency improvement is claimed.
Native, private theme, optimized full WASM and all-family DOM checks pass. Strict verification
exposes and fixes an orphan Railroad gate on the generic fallback helper; the subsequent 661-build
release/transport sweep is deliberately stopped, so full strict is not certified. Separate tag
diagnostics leave Timeline/Packet/Kanban E2E gaps of 8.58%/44.41%/38.17%. That checkpoint selected
Packet's requestless compatibility copies/style parsing and Kanban's repeated font resolution for
the preparation batch above. Those small reductions do not explain the full diagnostic gaps.
Preserve CSS validation and actual measurement/budget responsibilities. Class/Flowchart remain
separate optimization work.

The [October 17 Timeline writer-outcome checkpoint](timeline_writer_outcomes_macos_2026-10-17.md)
removes per-event SVG reverse inspection/path reconstruction and expected theme inventories.
All 57 allocation/report inputs and 76 cross-family SVG inputs match. Medium Timeline removes
49 allocations; 256-event classic and fully themed Redux inputs remove 5,389 and 9,531.
All Timeline allocation/byte counts decrease, retained bytes stay identical, and no peak increases.
Four-pair diagnostics observe render/public E2E changes of -10.14%/-6.68%, without A/A admission.
Direct CSS streaming was rejected for worse latency and buffer-growth behavior; the final path
retains one short-lived CSS batch. At that checkpoint, shared XML serialization was the next
profiling priority, preserving escaping, exact byte ceilings and bounded cancellation. See that
checkpoint for the separate v0.9.0 context: its public E2E diagnostic still observes a +31.51% gap.

The [October 17 demand-driven cleanup](native_theme_demand_driven_macos_2026-10-17.md) completes the
next theme ownership batch: Packet/Architecture/Kanban self-certification, State path lists and leaf
typography copies, immutable default construction, shared writer observation, and ER/Mindmap
representations. All 76 current-base SVG identities match. Five-repeat allocation probes remove
37/43/158 allocations for Packet/Kanban/State, and 2,108 for ER at four entities with 64 attributes.
The complete 304-invocation diagnostic observes Packet/Kanban/Timeline changes of -16.4%/-10.3%/-12.1%;
these are two AB/BA pairs without A/A calibration, not confirmed speedups. The low-latency comparator
now enforces its documented noise and identity-zero gates. Native, private qualification, bindings,
real full WASM and all-family DOM checks pass. Block indexing remains deferred because its repeated
passes have distinct actual consumers. The checkpoint retains scope, controls and rejected links.

The [October 16 v0.9.0 cross-family comparison](native_v0_9_cross_family_macos_2026-10-16.md)
covers 76 SVG inputs across 38 families: 19 identical-output diagnostic rows and 57 changed-output
context rows. Kanban, Timeline and Packet retain substantial SVG-emission overhead after the first
cleanup. The Kanban helper's confirmation classification does not satisfy the stricter documented
low-latency calibration condition; the report preserves that discrepancy and the raw measurements.
The [demand-driven rendering plan](../plans/2026-10-16-refactor-demand-driven-theme-rendering-plan.md)
is completed by the October 17 checkpoint above, with fresh adjacent-revision verification.
Binaryen packaging now succeeds through all five Web package builds and runtime smoke checks with its narrow pass bypass.

The [October 16 macOS runtime cleanup](native_theme_runtime_cleanup_macos_2026-10-16.md)
implements the accepted responsibility plan: ordinary generation no longer performs terminal
XML/reference certification, retired text scans or non-requested output hashes; data interfaces
avoid public report construction. Five matched-SVG diagnostic rows observe lower latency, including
Class 1,098→706 µs and Journey 101→38 µs. These are two-pair AB/BA diagnostics after explicit
resource/API migrations, without A/A calibration or a complete release-range recovery claim.
The checkpoint records actual native, qualification, bindings and all-family DOM verification,
plus the local Binaryen packaging failure separately from successful wasm32 execution.

The [October 16 runtime responsibility cleanup plan](../plans/2026-10-16-1800-refactor-theme-runtime-responsibilities-plan.md)
supersedes earlier instructions to preserve unconditional output observers and their retired
experimental error/digest contracts. The current cleanup separates generation, opaque-input
admission, native preparation and private qualification. Dated measurements below remain historical
evidence; the linked cleanup checkpoint records the fresh integrated validation and its limits.

The [October 16 macOS causal attribution](native_theme_root_cause_macos_2026-10-16.md) directly
reproduces the theme merge on clean Mermaid-12.2-aligned commits with identical Class/Journey SVGs.
Current-head sampling identifies mandatory standalone XML/reference validation and the separate
whole-SVG reserved-token search as major owners; Class parsing is nearly unchanged. These are
diagnostics, not calibrated latency admission. Preserve the failed/mismatched rows and investigate
these output observers before another broad theme-cache change.

The [October 16 runtime-responsibility analysis](../research/2026-10-16-theme-rendering-runtime-responsibilities.md)
reopens whether ordinary generation should incur these observers at all. It proposes separating
producer correctness, opaque-input admission, consumer preparation, and explicit identities.
Implementation follows the linked cleanup plan; its changed runtime contracts require fresh
integrated verification and do not by themselves establish a latency recovery.

The [October 16 overlay-order correction](native_theme_overlay_order_2026-10-16.md) preserves
actual Map insertion order for overlapping literal roots and their replay. Its sparse index
contains both ancestor and descendant participants, K <= E; unrelated roots add no entries.
All 2,006 core tests pass, including 42 eager-oracle order cases. This corrects interpreter
equivalence without reintroducing full-table reads or making a new latency claim.

The [October 16 reference dispatch checkpoint](native_xml_dispatch_windows_2026-10-16.md)
retains exact-name routing as an internal refactor with 5,352 renderer tests and all 152 SVG
identities passing. The native benchmark shrinks 87,040 bytes. External compilation invalidates
the first diagnostic after three completed measurements; discard the entire attempt. No latency
improvement or non-regression is admitted, and the release-range regression remains open.

The [October 16 terminal phase attribution](native_svg_terminal_phases_windows_2026-10-16.md)
isolates complete XML walk plus facts as the largest of three instrumented private segments on
Class medium/tiny, Journey and Kanban: mean 487/78/84/61 us, versus fingerprint 43/9/7/12 us and
reference analysis 21/6/4/0.2 us. All 512 timed outputs retain exact identities. Temporary timers
and stderr perturb execution, so these are attribution only, without an E2E percentage or speedup.
Investigate attribute processing while preserving complete validation, limits and checkpoints;
do not merge diagnostic hooks or infer helper-level costs from this segment alone.

The [October 16 standalone attribution](native_standalone_attribution_windows_2026-10-16.md)
measures only current public finalization: Class medium/tiny, Journey and Kanban mean 750/104/107/80 us
in a fixed 16-batch probe, with exact identities and clear compiler boundaries. Class-medium batches
range 589–876 us; this is attribution, not an E2E percentage, v0.9.0 ratio or admitted speedup. Split
fingerprint, complete XML/fact collection and reference-analysis costs before the next production
refactor. The [fixed-prefix search candidate](native_reserved_token_search_windows_2026-10-16.md)
is not retained: one 128-byte allocation disappears and SVGs match, but 42 diagnostic measurements
provide no stable admitted latency gain and mixed controls remain. Preserve its patch and evidence.

The [October 16 borrowed compatibility queries](native_theme_evidence_queries_windows_2026-10-16.md)
remove complete evidence snapshots from admission equality and residual counting. Same-binary
controls fall from two default allocations (32 bytes), or 772 allocations (45,088 bytes) at 128 frozen
fields, to zero while preserving exact query results. All 152 SVG identities and 5,163 core/renderer
tests pass. The native benchmark grows 171,008 bytes (0.42%). A complete 42-observation diagnostic has
mixed small movements and establishes neither latency improvement nor non-regression; retain only
the resource-work/ownership claim and continue attributing mandatory standalone finalization.

The [October 16 Class edge ownership refactor](native_class_edge_ownership_windows_2026-10-16.md)
shares one ordered borrowed edge sequence across path/label emission. Class medium removes 29
allocations and 1,028 allocated bytes; the zero-edge tiny control is unchanged. All 152 SVG rows
and 3,321 renderer tests pass. This removes duplicate work and transient ownership; observed
external compilation rejects the diagnostic before measuring, so no latency claim is admitted.

The [October 16 Class default-binding repair](native_class_shared_defaults_windows_2026-10-16.md)
removes node-dependent copies of shared defaults: padding a raw border token by K bytes now adds
exactly 4K preparation bytes at both 16 and 64 nodes, versus 20K/68K previously. It removes four default
allocations per node; all 152 SVG identities and 3,320 renderer tests pass. The retained result is a
preparation ownership/resource-bound improvement. Both latency attempts fail before measuring
(tasklist failure, then observed external compilation), so latency non-regression and recovery
remain unconfirmed. Shared edge ordering and common token/evidence work are separate candidates.

The [October 16 three-way frontier](native_theme_threeway_frontier_windows_2026-10-16.md)
compares v0.9.0, merged themes and the latest frozen optimized candidate in one complete
36-invocation diagnostic with identical Class/Journey/Kanban SVGs. Optimized means improve
3.20%-17.22% over merged but remain 46.99%-228.36% slower than v0.9.0; no calibrated latency
or non-regression admission follows. Both separately attempted emission-stage runs detect
compilation and are rejected. Extra source-review lanes exclude canonical JSON, rustls and large
default report clones as new primary owners. A public preparation allocation probe establishes
Class's retained-token slope: adding K bytes to a shared border token adds `(C+4)*K` current
preparation bytes for C nodes, while old preparation remains unchanged and SVGs match exactly.
Prioritize binding selections that borrow artifact-owned defaults, preserving typed/source
authority and completion; investigate repeated edge ordering and borrow-until-append styles as
separate changes. This is added pre-emission ownership, not a new overall complexity-order claim.

The [October 15 scalar confirmation](native_svg_scalar_confirmation_windows_2026-10-15.md)
uses clean committed base/head builds and complete eight-pair A/A schedules. Six of seven
fixtures fail calibration; Kanban completes 32 A/B pairs and confirms non-regression, but its
improvement interval does not clear the joint 10% and 50 us gate. Overall latency admission remains
inconclusive. The committed character algorithm's [correctness and size evidence](native_svg_scalar_experiments_windows_2026-10-15.md)
remains valid, without closing the SVG regression. Do not append samples to the frozen run.
The [October 15 policy-borrow experiment](native_svg_policy_borrow_windows_2026-10-15.md)
proves that borrowing the operation-owned policy removes the actual per-element 488-byte copy;
all 152 SVG rows and 3,317 renderer tests pass. One complete three-pair diagnostic retry has mixed
changes (Class medium -1.87%, Journey -0.63%, Class tiny -16.34%, Flowchart +4.38%); no latency or
non-regression admission follows. The ownership refactor is committed as `0f368910e`; preserve the
failed compiler-observed attempt and the independent State projection reproducibility caveat.
The larger frontier is typed SVG emission with complete facts carried across root/family/final
admission; a raw-byte sink alone cannot remove parsing. Preserve Usecase final-length resource
admission and formatting projection charges in that separate refactor.
Reference-name filtering and structured writer facts remain separate unadmitted candidates.

The [October 15 terminal follow-up](native_svg_terminal_followup_windows_2026-10-15.md)
rules out lost existing XML facts on default Journey and keeps explicit-pipeline stripping
separate from ordinary finalization. A checkpoint-dispatch specialization passes semantic
checks but adds 210,944 native executable bytes without dependable latency evidence; it is
not retained. Do not reverse the deliberate WASM body-sharing decision from callback counts
alone. Continue with actual event-derived facts only if terminal prefix errors, resource
admission and mutation invalidation remain complete.

The [October 15 Windows data-flow refactor](native_theme_refactor_windows_2026-10-15.md)
removes the reachable quadratic staged-provenance scan and reduces terminal allocation.
Class medium terminal allocation drops from 1,699 to 746 operations and peak growth from
160,850 to 30,234 bytes. The public 4,096-variable Dark diagnostic falls from about 148 ms
to 30 ms. Ordinary SVG latency recovery remains open: three-pair diagnostics do not clear
the joint improvement gate, and terminal timing does not show a consistent win despite
less allocation. Preserve complete validation and advance the generated-XML ownership
boundary only with exact event-derived facts, mutation invalidation and fresh evidence.

The [October 15 Windows attribution](native_theme_attribution_windows_2026-10-15.md) reproduces
the `v0.9.0` to merged-theme `b9844db7b` regression on identical Class and Journey SVGs.
Three-pair diagnostics observe Class medium +64.30% and Journey +274.31%; these are not a new
calibrated confirmation. Independent probes isolate ordinary standalone finalization at about
639 us for Class and 105 us for Journey, with resource hashing at about 39 us and 6 us.
Prioritize the shared XML/reference data flow and bounded serialization rather than another
local report-clone cleanup. A separate public Dark/frontmatter probe confirms quadratic staged
provenance work: 4,096 extra variables take about 151 ms versus 15 ms at v0.9.0.
Keep that structural repair separate from the default SVG lane. No production speedup is admitted.

The [October 14 native repair closeout](native_theme_repair_2026-10-14.md) closes N01–N36
as correctness or owner-local structural repairs. The fourteen-row frozen confirmation cohort
passes non-regression against clean corrected control `d5ae01105`; no row clears the registered
improvement gate. Final code `ac4a40437` preserves all 38 corrected outputs. Against original
main, nineteen fixtures remain byte-identical and the thirteen-row confirmation intersection
retains ten material regressions and three inconclusive outcomes. Class remains about 85.7%
(554.3 us) slower than main. Keep the main gap in the active attribution queue; the final cost
has not been separated into required validation and avoidable work. Preserve operation,
resource, source-ownership and exact-artifact contracts while profiling the remaining owners.
The repaired-control non-regression result does not close the historical release-range queue
or establish a whole-corpus latency or allocation improvement.

The [October 14 root-cause checkpoint](native_theme_root_cause_2026-10-14.md)
classifies the original nineteen changed-output fixtures and records the initial 25 high-confidence
repair items, expanded to 36 during implementation. Public probes reproduce a shared stylesheet/XML
boundary failure in 28 of 38 legal-font variants, incorrect scoped-theme derivation,
Neo non-palette Swimlane border override and four Block geometry/configuration gaps.
The [repair plan](../plans/2026-10-14-0053-refactor-native-theme-regressions-plan.md)
fixes correctness first, then removes demonstrated configuration, family-evidence and
reference-summary waste. The root-cause investigation alone did not admit performance recovery;
the closeout above records the subsequent implementation and confirmation separately. Retain
the original timing receipts.

The [October 13 native main-versus-theme checkpoint](native_main_theme_2026-10-13.md)
compares clean main `3a4b8c393` with theme branch `f47abded0` on macOS ARM64. Nine
byte-identical default-SVG cases confirm regression after stable eight-pair A/A calibration
per executable and eight fresh balanced AB/BA pairs. Class medium increases 82.35%
(537.66 us); Journey increases 246.49% (67.85 us). All nine simultaneous 95% lower bounds
clear the unchanged 10%/50 us gate. Nineteen of 38 discovery inputs change SVG bytes and
remain outside same-output latency admission. Parse diagnostics and separate CPU captures
identify selected-theme materialization/configuration copying and exact standalone SVG
finalization as profiling priorities. Preserve output, resource, cancellation and admission
contracts. This is current main-versus-branch evidence, separate from published release ranges;
no performance implementation is included in the checkpoint.

The [reference attribute-value experiment](reference_attribute_values_2026-09-20.md) rejects
skipping repeated normalization of values the reference collector does not read. Class medium
improves only 9.90 µs (0.84%) in two diagnostic pairs, below the registered joint latency gate.
All 245 pipeline tests and exact public SVG/receipt/preset identities pass; no production patch
is integrated. The complete XML/reference finalization cost remains an attribution target.

The [XML/reference single-pass candidate](xml_reference_single_pass_2026-09-20.md) is accepted
on the clean macOS ARM64 confirmation lane. It fuses XML event and attribute traversal while
retaining full validation, resource checks, cancellation and exact output contracts. Class medium
improves 10.925% and 136.988 microseconds with simultaneous 95% bounds clearing both thresholds;
Sequence medium also clears the improvement gate. Class tiny, XY Chart medium and Flowchart medium
confirm non-regression. The candidate is integrated as `bdb209e11`. This is an adjacent-source
improvement and does not close the alpha.6 release-range regression, cross-host cost matrix or
C7a/U10.

The [reference-plan checkpoint](reference_plan_2026-09-21.md) removes a repeated dependency-graph
evaluation while retaining conservative effect amplification. Class medium drops 12 allocations,
21,136 cumulative bytes and 5,184 bytes of peak growth per complete SVG render. Five controls
preserve exact SVGs, complete public receipts and preset fingerprints. Two diagnostic timing
pairs show no material slowdown; they do not establish a latency improvement. The earlier
no-effect-only reuse candidate was rejected because default Class contains filter definitions.

The [SHA-256 upgrade](sha2_upgrade_2026-09-20.md) admits sha2 0.11.0 on the measured macOS ARM64
lane. After eight A/A pairs per executable and eight balanced confirmation pairs, Class medium
falls from 1,461.74 to 1,248.73 µs (14.58%, 213.01 µs); Sequence medium also clears the existing
joint improvement gate. All five fixtures confirm non-regression and preserve exact SVGs,
public receipts, preset fingerprints and measured heap behavior. Feature, platform, legal and
dependency checks pass. This follows the [backend attribution](sha2_arm64_attribution_2026-09-20.md);
it does not close the alpha.6 release-range regression or establish cross-host/package results.

The [SVG attribute-validation checkpoint](svg_attribute_validation_2026-09-20.md) removes repeated
second-pass duplicate-name storage after full XML validation. Class medium drops 492 allocations
and 41,088 cumulative bytes per complete SVG render; five controls preserve exact output and peak
growth. Two diagnostic timing pairs show no material slowdown, but do not establish a confirmed
speedup or close the alpha.7 regression. The earlier single-byte-search candidate was rejected
because measured allocation did not change.

The [identity-shadow-offset checkpoint](zero_offset_shadow_2026-09-18.md) reduces actual filter
work from 140 to 112 primitives for the fixed Cyberpunk XY scene, preserving PNG/PDF bytes and
existing limits. It does not claim a latency improvement. Larger default-profile charts remain
limited by conservative SVG reference-expansion accounting and conversion budgets, and native PDF
cumulative allocation remains a U10 profiling target.

The [native export checkpoint](native_export_2026-09-20.md) adds current public-renderer throughput
for three complete default/Cyberpunk scenes across SVG, PNG and PDF. All 18 rows pass output-byte
replay at default limits; six PDFs also pass independent PDFKit reader/raster smoke. Cyberpunk
PDF costs 378.6–730.6 ms per output on the measured macOS ARM64 host, versus 42.6–59.0 ms for PNG.
These are different output workloads, not a revision regression. Keep localized PDF filter-image
work and cumulative allocation as profiling targets; matched output and calibrated evidence are
still required before changing implementation, quality or budgets.

The September 16 [theme capability and impact audit](../knowledge/engineering/verification/2026-09-16-theme-capability-impact-audit.md)
adds confirmed alpha.6-to-alpha.7 default-SVG regressions and matched CPU samples. These are
active work below; the earlier headless optimization decisions remain historical and unchanged.
- [System font query feasibility](../research/2026-10-10-system-font-query-feasibility.md)
  records an isolated Windows source-query probe and font-kit/fontique source
  audit. Targeted lookup is promising, but complete render equivalence, fallback,
  resource controls and cross-platform costs are not admitted. No production
  default or dependency changed; persistent caching is not the first experiment.
  The [isolated bridge plan](../plans/2026-10-10-system-font-query-candidate.md)
  tests public rendering with selected-family paths. Fontique 0.12.0 has no
  public pre-mapping candidate/loader hook, so this is not an admitted bounded
  system-discovery replacement.

- [Default CLI system-font checkpoint](native_system_discovery_2026-10-10.md)
  records identical-output fresh-process A/B on a font-heavy Windows host:
  preallocated bounded reads reduce median time from 2091.91 to 1468.03 ms.
  Full catalogue acquisition remains a cost for explicitly requested system-font
  context operations; ordinary CLI Mermaid layout no longer selects it by default.
  Legacy codec painting can still acquire system fonts independently of measurement.
  Investigate a content-correct catalogue/snapshot design without weakening identity,
  resource accounting or fallback policy. Explicit-font timings are a separate lane.

- [Native font measurement checkpoint](native_font_measurement_2026-10-10.md)
  retains final-row metric reuse as a structural duplicate-work repair (14.85%
  less allocation traffic on the controlled uncached-wrap lane, latency
  inconclusive). Cold/batch evidence prioritizes bounded cache replacement and
  lock attribution next. Direct single-face shaping is not admitted because it
  diverges from fallback and numeric semantics.

The release-range baseline dates to 2026-07-27 and 2026-07-28. Later checkpoints below record
their own revisions, dates, and admission decisions:

- [Alpha.3 to Alpha.5 Refactoring Report](../release/ALPHA3_TO_ALPHA5_REFACTORING_REPORT.md)
  compares `v0.8.0-alpha.3` with `d2698d0a3`.
- [Three-runner checkpoint](renderer_comparison_2026-07-27.md) compares that historical candidate
  with Mermaid.js 11.16.0 and `mermaid-rs-renderer` at `7ff1196`.
- [Current mmdr checkpoint](renderer_comparison_2026-07-28_75c9fd156_vs_mmdr.md)
  compares measured source `75c9fd156` with the same mmdr revision using the long preset and
  byte-identical input gating.
- [Current Node transport checkpoint](NODE_TRANSPORT_ADMISSION.md) compares N-API and Node-WASM
  from source `5f540c08d` with one capability recipe and the independently validated 4,001-case
  schema-2 corpus. It remains a private, inconclusive macOS arm64 result rather than a selected
  transport.
- [Runtime evidence manifest](baselines/runtime-06616dd71.json) freezes source `06616dd71`, the
  native-memory executable, full six-scale report, recipes, fixtures, and host. Its
  `infrastructure-smoke` owner contract is not candidate-admission evidence.
- [Flowchart work-accounting decision](flowchart_layout_work_accounting_2026-08-04.md) accepts the
  single render-owned Dagre/ELK work meter as a structural repair. Its causal public controls
  confirmed `flowchart_medium` at -2.60% and `flowchart_nested_clusters` at +3.09%, both within the
  registered non-regression gate.
- [Sequence metric-reuse decision](sequence_operation_metric_reuse_2026-08-07.md) accepts the
  private built-in operation sidecar for the two registered high-message lanes, which improved by
  40.00% and 40.88%; its six-scale allocation curves also passed.
- The current-base Requirement and Mindmap U9 hypotheses are closed independently:
  [Requirement](requirement_metric_reuse_2026-08-07.md) is rejected as written and by upper bound,
  while [Mindmap](mindmap_metric_reuse_2026-08-07.md) is rejected as superseded by U8.
- [Browser-WASM disposition decision](wasm_host_measurement_disposition_2026-08-08.md) accepts a
  smaller owner-local replacement for the rejected custom visitor. It removes one disposition
  Serde decode per handled callback, shrinks the optimized render artifact by 388 bytes, and keeps
  latency as supporting non-regression evidence rather than a release claim.
- [Final four-lane attribution](headless_performance_final_attribution_2026-08-08.md) closes U13
  with one late accepted structural candidate, records the representative interaction guard,
  preserves the fail-closed alpha.3 coverage boundary, and reconciles the public break inventory.
- [Playground startup investigation](playground_startup_2026-10-01.md) leaves initial-render
  scheduling unchanged after two unstable A/A calibrations. Profiled Monaco and first-SVG main
  thread costs remain attribution evidence; no cold-start speedup is admitted.

- [Native layout-work policy calibration](native_layout_work_calibration_2026-10-01.md) accepts
  the 15,000,000-unit offline default for CLI and rustdoc across 73 fixtures and both themes.
  Exact admission boundaries and identical-output controls passed. The conservative CLI scheduler
  now admits one default graphical backend at a time; no throughput or memory improvement is claimed.
- [Interactive layout-work policy calibration](interactive_layout_work_calibration_2026-10-01.md) accepts
  the 14,100,000-unit general library/Web default across the same 73 fixtures and both themes.
  Exact `W/W-1`, registered cardinality, and configuration boundaries passed. The profile is an
  admission baseline; host deadlines, cancellation, concurrency, and memory remain separate.

Evidence dates use the local Asia/Shanghai calendar recorded by each receipt. Decisions through
2026-10-01 are authoritative; there is no blanket clock-drift exclusion for the U10-U13 receipts.
Every claim remains bound to named revisions, executables, fixtures, and raw reports by digest.

| Comparison lane | Shared rows | Median ratio | Geometric mean | Faster / slower |
| --- | ---: | ---: | ---: | ---: |
| `d2698d0a3` / alpha.3, complete SVG product | 34 | 1.10x | 1.09x | 13 / 21 |
| `d2698d0a3` / alpha.3, minimal same-capability SVG | 32 | 1.12x | 1.07x | 10 / 22 |
| `75c9fd156` / `mermaid-rs-renderer`, identical inputs | 30 | 0.664x | 0.297x | 18 / 12 |
| `d2698d0a3` native / warm Mermaid.js browser | 34 | 0.0237x | not reported | 34 / 0 |

The cross-runner rows are not quality-adjusted rankings. Merman and `mermaid-rs-renderer` have
different Mermaid coverage and output goals, and native Rust versus warm Chromium is not a
browser-WASM comparison. The mmdr lane excludes Treemap and XYChart because their same-named
fixtures differ; `flowchart_large` and Info have no shared measurement.

## Triage policy

A regression enters the active queue when it breaches the materiality contract registered for its
public scenario. When no scenario-specific contract exists, the comparator's same-host default is
the joint `>10% AND >50 us` budget. The 50-microsecond value is a stable reporting fallback, not a
universal engineering cutoff.

Candidates and regressions must instead be judged against the evidence class that matches the real
cost:

- complete operations below 500 microseconds may preregister the noise-derived low-latency gate in
  [BENCHMARKING.md](BENCHMARKING.md);
- interactive work may use a documented frame or response budget;
- high-frequency integrations may use cumulative CPU time, throughput, or tail-latency objectives;
- memory work may use allocation, peak, or retained-state objectives; and
- reachable repeated work or input amplification may use exact counters and old/new complexity
  bounds with representative public latency as a non-regression control.

The scenario, expected call volume, A/A noise floor, decision statistic, and thresholds must be
frozen before confirmation. A small, owner-local, semantics-preserving removal of repeated work can
land under structural or memory evidence even when the per-call absolute latency delta is below 50
microseconds. A candidate that adds caches, protocol machinery, persistent state, or broad
abstractions needs stronger measured public-operation evidence proportional to that maintenance and
correctness cost. A large ratio on a tiny private stage remains attribution evidence only.

An externally reachable complexity or resource-amplification repair uses a different evidence
class. It must name the user-controlled variables, prove the reachable old/new time and added-space
bounds, preserve semantic and resource-accounting behavior, and keep representative public work
within its non-regression budget. Such a result may be accepted as a structural repair without
clearing a latency threshold, but it is not a measured speedup unless a separate end-to-end timing
gate passes.

Before implementation:

1. Select and preregister the admission class.
2. Match capabilities between base and target revisions.
3. Attribute parse, layout, render, and end-to-end stages.
4. For timing or throughput, run at least eight balanced base and head A/A calibration pairs,
   derive the fixed even AB/BA confirmation count from the preregistered MDE, and collect fresh
   order-balanced pairs within the fixed maximum budget. For structural work, freeze the input
   variables, old/new time and space bounds, and exact counter or scale-curve method instead.
5. Record model size, SVG bytes/elements, and the relevant semantic, DOM, or raster parity result.
6. Profile only after the slow stage is known.

## 2026-10-05 native release follow-up

The active release comparison is [v0.7.0 Dagre versus current Dagre](library_dagre_v070_to_01cc4562f_2026-10-05.md), measuring complete reused-engine crate SVG calls without CLI startup. Both sides explicitly select Dagre/classic, the default theme, and matching Flowchart settings. All six selected controls have lower current estimates in both recorded pairs: about 15%-33% lower paired-median time. Five controls are measured in the scoped investigation; the nested-cluster control retains its earlier receipt. These are descriptive observations: both same-executable A/A calibrations failed the registered noise gate, so no speedup or blanket non-regression is statistically confirmed. ELK has no v0.7.0 Flowchart implementation baseline; neither ELK nor alpha.7 is included in this release comparison.

The [initial observation](library_rendering_v070_to_01cc4562f_2026-10-05.md) and [expanded native corpus observation](library_rendering_expanded_2026-10-05.md) retain their original default-product evidence and unresolved findings. Their aggregate counts are not the current release-performance headline. In particular, changed defaults prevent assigning the four earlier Flowchart weave, ports, fanout, and long-label slowdowns to a Dagre implementation regression: the matched-Dagre observations have 15%-21% lower current costs. This does not resolve every previously recorded signal or establish a cause.

Retain LIBPERF-01 (Mindmap), LIBPERF-02 (SVG-emission ownership boundaries), and LIBPERF-03 (State plus historical Requirement/Gitgraph identity instability) as deferred findings under their original contracts. LIBPERF-04 remains resolved by restoring the twelve AgentFlow/Usecase selectors. LIBPERF-05 records coverage of the earlier native corpus; memory, additional scales, cold-engine, and other transports remain outside this comparison. For LIBPERF-06, the four matched-Dagre controls do not reproduce the prior slowdown direction; parallel-merges and backedges/subgraphs have not received matched-backend confirmation. LIBPERF-07 retains the smaller/noisy historical signals. A stronger release claim requires a new preregistered stable-runner campaign with A/A calibration and fresh balanced pairs; do not retry the failed schedule until favorable. No production fix or alpha bisect is part of this checkpoint. The older priorities below retain their original revision/host scope.

## Priorities

The [current Web/Typst artifact snapshot](../knowledge/engineering/verification/2026-09-20-web-typst-consumers-single-pass.md)
at `bdb209e11` passes all 24 existing size-budget metrics across six profiles, with installed Web
and Typst owner checks. The refresh changes no size limits. Matched release-range footprint and
runtime comparisons remain separate open work.

The [historical artifact checkpoint](theme_web_artifact_gate_2026-09-13.md) at `eda8c3846` records
all five then-current Web profiles exceeding raw, stripped, Gzip and Brotli limits despite passing
production smoke and an installed Chromium consumer. The explicit Binaryen `-Oz` candidate was
rejected because compressed sizes grew. The clean revision-86 baseline at `4e4f3acc3` confirms
those twenty historical failures. Its five-sort canonical encoding/font-catalog candidate is also
rejected: render shrinks only 2552 bytes while gzip grows
683 bytes and Brotli grows 2034 bytes. Keep that rejected result closed and require a distinct
causal hypothesis for further size work; raw stable-sort symbol totals are not admission evidence.

The [family evidence index repair](theme_evidence_requirement_index_2026-09-13.md) at `d92dee3ad`
removes the quadratic ordered-key deduplication step by using the ledger's existing membership
index. Native route order and compatibility ownership are preserved, with no added retained index.
The clean private Release owner suite passed 3,273 tests, SVG structure and real Typst package
checks passed, and all four Typst size controls decreased slightly. This is a structural work-bound
repair, not a measured latency or peak-memory improvement; it did not close the size gates at
that checkpoint.

The [typography winner guard](theme_typography_recomputation_2026-09-13.md) removes repeated font-stack
copying for paint-only rules. Its bounded public SVG probe reduces cumulative allocated bytes by
90.719% with unchanged peak heap growth; the Web render artifact has a small measured size cost.
That allocation repair did not close the artifact-size gate or change family support at its
checkpoint.

| Priority | Fixture | Current latency | Current / alpha.3 | Current / mmdr | User impact |
| --- | --- | ---: | ---: | ---: | --- |
| P1 | `requirement_medium` | 196.96 us | Historical candidate was 2.12-2.25x before the focused fix | 2.77x (+125.88 us) | Operation-scoped label reuse removed 60.6% of SVG emission. Latest gaps are layout (2.81x) and SVG emission (3.85x), while parse is 0.72x mmdr. |
| P1 | `mindmap_medium` | 165.26 us | No confirmed current regression; the ordinary-label fix reduced its pre-fix latency by 75% | 2.23x (+91.20 us) | The layout ratio compares real COSE-Bilkent with mmdr's radial fallback, so only same-algorithm work is actionable. |
| P1 | `flowchart_large` | 23.20 ms | about 1.00x at the historical checkpoint | unavailable | This is not a refactor regression, but it is the only standard fixture beyond a 16.7 ms frame. |
| P1 | `class_medium` | 900.36 us | Historical candidate was 1.10-1.15x (+89-126 us) | 0.38x | A common family worth rechecking against alpha.3; Merman remains about 2.6x faster than mmdr. |

Do not prioritize the current Info, Packet, Radar, or Sankey ratios without new absolute-cost
evidence. Their measured alpha.3 increases are about 2-5 microseconds. Architecture,
`flowchart_small`, Gantt, and `flowchart_ports_heavy` are within the current noise band.

Kanban is no longer a cross-runner P0: it now measures 57.66 us versus mmdr's 28.19 us, a 29.47 us
absolute gap on a cross-runner lane with different product semantics. A new candidate requires a
current causal owner-local upper bound and a registered scenario objective rather than a fixed
absolute cutoff. Its alpha.3 lane must be rerun before declaring the historical release regression
closed. XYChart is not a valid cross-runner priority until both runners use the same fixture.

## Completed work

### 2026-09-18 bounded syntax-snapshot index reuse

The [expanded attribution](lsp_index_reuse_attribution_2026-09-18.md) led to a snapshot-owned compact
index with an independent 8 MiB per-session retention budget. Complete admitted indexes are reused
across full/delta/range requests; contested initialization and budget denial use local construction.
Accounting follows the last index reference, including older snapshots. There is no replacement
policy or token-result cache. See the [bounded-reuse receipt](lsp_bounded_index_reuse_2026-09-18.md)
for lifetime/concurrency tests, exact resource boundaries, and eight paired handler controls.

All first/hot/edit controls pass; the ordinary 4 MiB hot-range observation is 2292.53 -> 22.66 us,
while dense 4 MiB indexes are denied retention. This is a structural repeated-work/resource
admission, not a separately admitted latency speedup. First-request preparation and source updates
remain; real editor request frequency should inform any future budget or replacement policy.

### 2026-09-17 compact LSP line indexing

The [baseline attribution](lsp_line_index_attribution_2026-09-17.md) identified whole-source
line-table construction in short-range semantic-token requests. The request-local index now stores
one `usize` line-start offset instead of three offsets per line, deriving line ends as needed.
Initialized index storage is reduced by two thirds; all registered reserved-capacity controls match
that reduction, including 4 MiB dense Markdown at 96 -> 32 MiB and pure LF at 192 -> 64 MiB.
See the [compact-index receipt](lsp_compact_line_index_2026-09-17.md) for semantic boundaries and
eight paired handler non-regression controls. No whole-process memory or admitted latency speedup
is claimed. Full-source scanning remains; snapshot reuse is a separate hypothesis requiring an
explicit retained-memory policy and repeated-request evidence, especially against one request
per edit.

### 2026-09-17 layout image scanning

The separate Flowchart layout image helper now compares fixed-size prefixes and consumes each
searched tag span once. This removes the remaining quadratic suffix-lowercasing term from mixed
image labels while preserving metric values and opaque measurer requests. See the
[layout scanning receipt](layout_image_scan_2026-09-17.md) for the linear preparation bound,
original-implementation behavior checks, and validation scope. No measured speedup is claimed.

### 2026-09-17 LSP semantic-token scheduling

Full, delta, and range token computation now leaves the transport poll and shares the analysis
executor's two CPU permits and eight task slots. Cancellation retains permits until physical work
exits and cannot cancel sibling requests. See the [scheduling repair receipt](lsp_syntax_scheduling_2026-09-17.md)
for deterministic progress, resource-accounting evidence, and remaining cancellation latency limits.
No measured latency improvement is claimed.

### 2026-09-17 label work removal

Flowchart image normalization uses a fixed-size ASCII prefix comparison instead of repeatedly
lowercasing the remaining label, removing the helper's quadratic suffix-scanning term. Built-in
RaTeX measurement retains display-list dimensions but skips discarded SVG serialization. Neither
change adds a cache or changes custom callbacks. See the [structural repair receipt](label_scan_math_measurement_2026-09-17.md)
for bounds, correctness evidence, and validation limits; no latency improvement is claimed.

### 2026-08-08 headless hardening decisions

| Unit | Decision | Durable boundary |
|---|---|---|
| U1 evidence and current baselines | `completed-evidence-only` | The clean branch-start boundary, benchmark/output-identity contracts, candidate-local ignored ledgers, and cleanup rules are frozen in [the U1 receipt](headless_performance_branch_start_2026-08-02.md). |
| U2 pinned registry baselines | `rejected-inconclusive` | Two current-base confirmations failed the preregistered A/A stability contract on opposite sides. The `OnceLock + COW` candidate and its candidate-only tests were removed in an explicit cleanup commit; see [the U2 receipt](cached_baseline_registries_2026-08-03.md). |
| U3 Browser-WASM disposition | `accepted-structural` | A direct `handled` property read replaces the disposition-only Serde struct decode before the existing complete result deserialization. The rejected custom visitor remains absent; the optimized render artifact is 388 bytes smaller, full Web smoke passes, and noisy browser timing is supporting evidence only. See [the U3 follow-up receipt](wasm_host_measurement_disposition_2026-08-08.md). |
| U4 State Rough lifecycle | `accepted-memory`, `accepted-structural` | Seeded Rough reuse is operation-owned and releases every cache allocation after success, ordinary error, unwind, concurrency, and recovery; native rectangles skip unused Rough construction. The representative public lane made no latency claim. See [the U4 receipt](state_rough_operation_cache_2026-08-02.md). |
| U5 Architecture and Flowchart work accounting | `accepted-structural` | One render-owned operation meter receives neutral lower-crate work-control charges for Architecture FCoSE, Flowchart Dagre/Dugong, and Flowchart ELK without clamping configuration or substituting algorithms. See the [Flowchart accounting receipt](flowchart_layout_work_accounting_2026-08-04.md) and [interactive policy calibration](interactive_layout_work_calibration_2026-08-07.md). |
| U6 ELK hierarchy preparation | `accepted-structural` | Stable scope ownership and iterative postorder remove suffix cloning and repeated descendant discovery. A source-equivalent synthetic adjacent pair passed eight-pair public ELK A/A and AB/BA controls with exact SVG identity; see [the U6 receipt](elk_hierarchy_preparation_2026-08-03.md). No latency or memory claim is admitted. |
| U7 Dugong transient retirement | `accepted-structural` | Stable batch retirement preserves node/edge/compound order and exact writeback; public controls matched SVG identity. One self-loop row was stable, while the long-edge base A/A lane was inconclusive, so no latency claim is admitted; see [the U7 receipt](dugong_transient_retirement_2026-08-02.md). |
| U8 rich inline HTML planning | `accepted-structural` | Cumulative five-slice planner closure removes repeated run/style scans and growing transient payloads while preserving Mermaid 11.16.1 wrapping and opaque callback semantics; see [the U8 receipt](rich_inline_html_planning_2026-08-07.md). No latency or peak-memory claim is admitted. |
| Interactive layout-work policy calibration | `accepted-structural` | The 14,100,000-unit general library/Web ceiling follows the registered headroom rule for the current 73-member Mermaid 12 corpus in both themes and has exact `W/W-1`, node/edge cardinality, configuration-amplification, isolated-stage, timeout, and output evidence; see [the policy receipt](interactive_layout_work_calibration_2026-10-01.md). No latency or memory claim is admitted. |
| Native layout-work policy calibration | `accepted-structural` | The 15,000,000-unit CLI/rustdoc default follows the fixed margin rule over 73 current fixtures in both themes, with exact `W/W-1`, full-prefix cardinality, configuration-amplification, isolated-stage, timeout, and identical-output controls. See [the native receipt](native_layout_work_calibration_2026-10-01.md). One graphical backend fits the unchanged native scheduler pool; no latency or memory improvement is claimed. |
| U9 Sequence operation metrics | `accepted-latency`, `accepted-memory` | Reuse is private to the exact built-in carrier and semantic owner; host/custom callbacks remain unchanged. See [the Sequence receipt](sequence_operation_metric_reuse_2026-08-07.md). |
| U9 Requirement Markdown width | `rejected-as-written`, `rejected-upper-bound` | The old shared change reduced opaque callbacks; the compliant owner-local upper bound was only 13.249 us / 7.379% and did not justify another metric carrier without a registered high-volume, memory, or structural objective. See [the Requirement receipt](requirement_metric_reuse_2026-08-07.md). |
| U9 Mindmap inline metrics | `rejected-superseded` | U8 already removed every admissible discarded built-in request; the remaining opaque callbacks are observable behavior. See [the Mindmap receipt](mindmap_metric_reuse_2026-08-07.md). |
| U10 CLI transaction metadata | `accepted-structural` + `rejected-not-admitted` | The mandatory target-to-index map removes the stage-slot `O(N^2)` lookup term. The optional sealed-manifest/compact-frontier topology was not admitted because no adjacent candidate or three-platform durability matrix exists; the baseline journal remains unchanged. See [the U10 receipt](cli_transaction_metadata_2026-08-08.md). |
| U11 semantic-token reducer | `superseded-removed` | The parser-backed token planner was deleted on 2026-08-17 when Tree-sitter became the sole syntax-highlighting owner. The [U11 receipt](semantic_token_reducer_2026-08-08.md) remains historical evidence for the removed implementation. |
| U12 facts-contract deletion | `accepted-structural` | Facts schema 2 deletes the unused Flowchart-only rich graph, its analysis-side projection/rule, and the mirrored wire surface. The diagnostics-only retained-object split remains narrow and policy-neutral; no latency claim is made. See [the U12 receipt](diagnostics_capture_specialization_2026-08-08.md). |
| U13 final attribution and integration | `completed-one-new-candidate` | The fixed-budget native, browser-WASM, CLI, and editor/analysis scan admitted one smaller owner-local Browser-WASM structural candidate after the materiality policy was made scenario-aware. The production-equivalent representative guard found no material regression, alpha.3 remains coverage-only, and the public break inventory is reconciled in [the U13 receipt](headless_performance_final_attribution_2026-08-08.md). |

No failed U9 production path or global text cache remains. The old Requirement and Mindmap
experiment branches are historical hypotheses only.

### Mindmap ordinary-label rendering

Commit `b2129d9ec` resolved the confirmed ordinary-label regression without a family-local
classifier. Against pre-fix revision `71cb231c8`, three same-host runs measured:

| Metric | Pre-fix median | Fixed median | Change |
| --- | ---: | ---: | ---: |
| Mindmap end-to-end | 654,210 ns | 161,330 ns | -75.34% |
| Mindmap layout | 94,693 ns | 89,608 ns | -5.37% |
| Mindmap SVG | 542,590 ns | 62,383 ns | -88.50% |
| Kanban control | 63,881 ns | 56,272 ns | -7,609 ns |

The control delta remained below the registered 10,000 ns noise threshold. The retained
implementation defines an exact syntax-free ASCII projection in the Markdown interpreter and lets
the sanitizer skip DOM work only when its complete effective tag policy preserves the generated
paragraph. Unicode, entities, Markdown, HTML, icons, math, unknown security levels, and customized
sanitizer policies retain the full path. The faster punctuation-blacklist experiment was rejected
after `#quot;` exposed Mermaid placeholder leakage.

### Requirement operation-scoped label preparation

Commit `8d45b8634` carries Requirement label text, exact measurements, and stable edge identity from
layout into SVG emission. It does not introduce a global cache or syntax heuristic. Markdown
conversion and strict sanitization still happen at render time; only duplicate measurement and
label-plan reconstruction were removed. The public layout JSON remains unchanged.

Three same-host long runs alternated base/head order with 30 samples, two-second warm-up, and
three-second measurement windows:

| Stage | Before median | Fixed median | Change |
| --- | ---: | ---: | ---: |
| Parse | 5,527.6 ns | 5,612.6 ns | +1.54% |
| Layout | 132,050 ns | 131,560 ns | -0.37% |
| SVG emit | 137,430 ns | 54,165 ns | -60.59% |
| End-to-end | 274,840 ns | 198,980 ns | -27.60% |

The latest focused mmdr run at `75c9fd156` measured Merman/mmdr ratios of 0.72x parse, 2.81x
layout, 3.85x SVG emit, and 2.81x end-to-end. All 1,148 `merman-render` tests passed, with one
pre-existing skip; the
focused Requirement selection passed 21/21, and the Look SVG test passed 2/2.

This repair predates the complete evidence rule above: the A/B receipt did not record model size,
SVG byte/element counts, or a raster receipt. Its retained correctness evidence is the unchanged
public layout JSON plus the focused, Look, and full render test suites. Do not use the richer-output
hypothesis to explain the residual until those structural measurements are collected.

The latest full standard run measured 196.96 us versus mmdr's 71.08 us and retained the same
30 comparable rows, 18 Merman leads, and 12 mmdr leads. Its input gate excludes different Treemap
and XYChart sources from every ratio.

## Work queue

### Active: all-family theme performance convergence to v0.9.0

The maintainer requested a persistent goal on 2026-10-18. Cover all 35 logical family catalog
entries and all 10 compiled presets, including partial support and fallback; retain parser aliases
as coverage rather than counting them as independent logical families. Use the existing 38-scene
cross-family corpus and additional stress inputs when actual cost warrants them. A compiled theme
is prepared outside warm-render measurements; its per-request application remains inside.

For comparable public E2E operations, the near-release target is no gap exceeding both 10% and
1 us, assessed per row rather than through an average. Decision-grade latency evidence still
requires the existing A/A and fixed AB/BA contract; uncalibrated diagnostics cannot close the goal.
Different SVG/capabilities remain separate lanes with source-backed reasons and alternative
adjacent evidence. Necessary residual fixed work may remain only with an explicit consumer and
cost justification. Do not mark missing coverage, noisy timing or unexplained residuals complete.

Continue high-confidence ownership/work/allocation repairs in independent measured batches.
Preserve theme/config fallback, source semantics, actual writer evidence, error order, budgets,
cancellation and measurement callbacks. Engine borrowing, root style borrowing and canonical
root-outcome ownership are complete. The October 19 family-preparation ownership batch is recorded
in its [checkpoint](family_theme_preparation_macos_2026-10-19.md). All 418 warm
compiled-preset/default coverage cells preserve the frozen adjacent output/report contract. The
latest admitted full 19-scene same-SVG confirmation at `b9a58731c` establishes 14 within the joint
budget, two confirmed regressions (ZenUML and Packet) and three inconclusive scenes (Sankey,
Kanban and Wardley). Kanban's +18.95% / +8.521 us remains a priority despite its unqualified
absolute A/A certificate. The earlier b9 attempt is rejected for source/executable mismatch;
the accepted identity repair uses entirely fresh observations and physical no-hook admission.
Attribute actual consumers before selecting further repairs. Empty explicit-owner queries,
absent scoped appearance paths and borrowed unchanged CSS identifiers now have independent
admitted allocation checkpoints. Frozen path storage remains separate. The intermediate metadata
clone was independently measured and rejected: its Packet allocation win accompanies stable
Venn peak regressions. Its patch and failed records are preserved and its code is absent. The
separate JSON disposal repair passes all 418 catalog/645 site behavior controls and its original
stable-memory gate. The October 21 root-cause investigation now takes precedence over that
allocation queue: redesign generated CSS/XML and bounded fragment protocol, isolate ZenUML's
default producer and remaining completion/layout work, then unify immutable compiled-theme
request ownership and freeze coverage once for unchanged evidence. Its diagnostic interventions
retain full rich E2E/drop but are not safe production candidates or formal recovery. Pre-PR224
observations refute PreparedSource Arc as the original root cause. Keep metadata/fresh-owner,
440-slot reporting and unused editor-facts candidates parked; the latter two existed in the tag.
Use calibrated confirmation for subsequent latency claims; see the
[scope checkpoint](engine_borrow_theme_scope_macos_2026-10-18.md) and
[release-context refresh](theme_v090_same_svg_refresh_macos_2026-10-18.md).

The October 21 ZenUML selector-owner cleanup forwards the already normalized `SvgDiagramId`
into the stylesheet writer instead of repeating the selector eligibility scan for its 35 CSS
occurrences. Four balanced public E2E controls preserve exact SVG identities and show a 4.19% to
6.06% ZenUML reduction, about 1.0 to 1.45 us, with no control regression. It is retained as a
safe local cleanup below the 10% formal latency gate; it does not change the v0.9.0 classification.
See the [compact evidence archive](evidence/zenuml-id-normalization-2026-10-21/manifest.json).

### Active: alpha.7 default SVG regression

The September 16 release-range audit confirms regressions in unthemed complete SVG operations
from published alpha.6 source to candidate `a5e3cd2d6`: Class tiny +270% to +274%, Class medium
+125% to +127%, and XY Chart medium +162% to +166% (simultaneous confidence bounds). The same
Rust 1.95 toolchain, harness, features and exact SVG bytes were used. Dependency changes remain
part of this release-range comparison; this is not theme-only attribution.

Stage diagnostics and matched alpha.6/candidate CPU sampling identify prepared-text token
partitioning and standalone SVG finalization as shared cost centers. First review the minimal
typed-theme/SVG dependency and execution boundary separately from native-export guarantees.
Theme font embedding/shaping was retired in `54b4d484a`; that removes its dependency cost but does
not close the default-SVG regression. Investigate scanning/allocation and ownership of already
validated artifact facts. Preserve output bytes, terminal evidence, malformed-token and
XML errors, resource limits and cancellation. Do not revive the rejected July raw-string API or
skip admission because the caller did not explicitly select a theme.

The September 20 [allocation repair](svg_attribute_validation_2026-09-20.md) now reuses the XML
owner's attribute-name uniqueness result in private second passes. Full XML validation and
resource/error/cancellation semantics remain unchanged. Its measured allocation reduction does
not close this latency gate; continue profiling finalization and prepared-text work rather than
assuming source-level temporary allocations survive optimization.

Follow-up source tracing at `2db16c6b7` excludes per-tag prepared-text copying from the default
Class cause: Class owns an empty prepared-text ledger, so these fixtures only execute the reserved
spelling search. A direct-copy candidate was rejected before measurement rather than switching
its registered workload. Ordinary finalization also computes the resource fingerprint only once.
Keep the remaining XML/reference work and reserved-spelling search distinct from prepared-font
token rewriting when forming the next hypothesis.

The SHA backend candidate is now [accepted](sha2_upgrade_2026-09-20.md): the workspace upgrade
to 0.11.0 preserves both the framed resource fingerprint and raw public artifact digest while
confirming a 14.58% Class-medium latency reduction against an adjacent old-SHA baseline with
identical benchmark source. All five controls pass. Continue attributing the remaining Class
cost and renew final-candidate artifact evidence; the isolated native win does not close this
release-range regression. The [September 21 matched renewal](alpha6_current_class_2026-09-21.md)
compares published alpha.6 with post-upgrade source `aa35110ee`: Class tiny remains +209.94%
(50.65 to 156.99 µs), and Class medium +102.32% (600.34 to 1,214.61 µs). Both clear the unchanged
10%/50 µs regression gate with stable calibration, eight balanced confirmation pairs and exact
SVG identities. These current observations supersede the pre-upgrade values for this source;
they remain release-range evidence, not theme-only attribution.
Two-pair stage diagnostics show sub-microsecond parse deltas and family SVG emission increases
of 16.50/92.57 µs. Layout projection identities differ only in the default secure-key list;
geometry matches, but that changed-config stage is not sampled. Four fresh CPU captures retain
the measured binary identities: that capture's standalone finalization accounts for 2,393 of 7,493
main-thread end-to-end samples, including 2,327 in XML/reference resource-budget checking.
These overlapping samples identify the next owner to investigate, not removable time. Preserve
the new artifact/resource/admission contract and do not infer a finalizer-only cause by subtracting
independent stage measurements.

The subsequent [single-evaluation candidate](reference_plan_2026-09-21.md) measures a bounded
allocation reduction without weakening effect accounting; full strict verification remains
blocked by existing workspace Clippy errors. It does not materially resolve the measured
alpha.6 latency gap. Continue source-backed attribution before choosing another candidate;
do not infer a speedup from reduced temporary storage.

The [September 24 correctness and attribution checkpoint](../knowledge/engineering/verification/2026-09-24-modern-theme-performance-attribution.md)
refreshes sampling at `8325653c3` after font retirement. Ordinary XML/reference budget validation
accounts for 36.50%, 33.20% and 24.49% of the Flowchart-tiny, Sequence-medium and Class-medium
main-thread samples respectively (inclusive, overlapping counts). This default BestEffort path
already combines XML/reference collection in one pass and does not perform strict resvg/resource
closure certification. Attribute its remaining attribute validation and reference-plan allocations
before proposing another traversal refactor. Class also constructs node/relation/marker terminal
expectations without a selected theme; determine their source-style responsibilities before making
that work conditional. Keep reserved-token defenses, fingerprint identities and cancellation/error
ordering intact. The 8-second `profile_render` stage data are diagnostic; its render stage includes
preparation and is not Criterion's render-only body. No new latency improvement is claimed.

Exit: a measured candidate against a fixed adjacent baseline, representative Class/XY Chart and
cross-family controls, full relevant negative-contract tests, and the existing scenario-specific
confirmation gate. Sampling percentages alone cannot admit a change or justify the added cost.
See the [September 16 capability and impact audit](../knowledge/engineering/verification/2026-09-16-theme-capability-impact-audit.md)
for exact observations, excluded fixtures and remaining memory/throughput work. This active
regression is separate from the historical closed hypotheses below.

### P1.0: Profile residual Requirement layout and SVG construction

The repeated-label cause is closed. Profile Dugong layout separately from path/DOM string
construction on the same fixture. Do not move strict sanitization out of the render phase, and do
not treat mmdr's smaller SVG as semantic equivalence.

Exit: the remaining focused 2.81x layout and 3.85x SVG ratios are attributed to named operations
with checked-in absolute stage receipts, and any retained change preserves Requirement goldens,
sanitization, custom-measurer behavior, and public layout JSON.

Completed on 2026-07-29. Sampling attributed 96.6% of Requirement prepare time to text/label work
and Dugong, both excluded from the residual unit; the dispersed remainder has no qualifying
owner-local term. This 2026-07-29 residual item predates the 2026-08-02 U9 family metric-reuse unit
and closes without production changes; see
[the closure receipt](runtime_hypothesis_closures_2026-07-29.md).

### P1.1: Add fair reusable and strict cross-runner lanes

The current public-path comparison is intentionally asymmetric: Merman's one-shot helper creates a
render environment per call, while mmdr reuses its theme/config; Merman uses strict render-model
parsing, while the mmdr benchmark uses its permissive parser. Keep the public-path lane, then add:

- a reusable environment/config lane for both implementations; and
- a strict parsing lane where both products perform their documented validation.

Exit: reports name the public, reusable, and strict lanes separately and never merge their ratios.

### P1.2: Compare Mindmap with the same layout algorithm

Run a tidy-tree lane only if both implementations expose equivalent configuration. Do not replace
Merman's default COSE-Bilkent layout with mmdr's radial fallback to improve a benchmark.

Exit: the report separates algorithm cost from parser/render cost and retains default-product
geometry.

### P1.3: Prepare Kanban labels once per operation

Kanban layout converts, sanitizes, and measures section/item Markdown, while SVG rendering repeats
the conversion and sanitization. First add a stage benchmark and prove the share of the current
29.47 us cross-runner gap. If material, carry a private operation-scoped label plan as Requirement
does; do not add a global cache or syntax classifier.

Exit: Kanban semantic/layout/SVG goldens and hostile Markdown/HTML behavior remain identical, and a
decision-grade adjacent A/B confirmation shows the exact stage and absolute saving.

Completed on 2026-07-29. The accepted private artifact reuses prepared section/card-title XHTML and
geometry while leaving detail-only measurements in SVG emission. The public medium lane improved
from 51.65 us to 40.71 us (-21.19%, -10.94 us); see
[the U5 receipt](kanban_prepared_labels_candidate_2026-07-29.md).

### P1.4: Avoid editor-only bookkeeping in render-only parsing

Several typed render parsers reuse semantic constructors that also populate editor facts and lexeme
journals, then discard them. Prefer a shared constructor with a semantic-only or no-op facts sink;
do not fork a second parser.

Exit: semantic models, errors, recovery, spans, and editor output remain identical, while
`parse`, `compatibility_json_parse`, and end-to-end measurements show where the fixed cost moved.

Completed on 2026-07-29. Mindmap and Requirement were rejected by whole-parser upper bounds. A
temporary Kanban no-fact candidate saved about 1.0 us (2.49%) on the accepted 40.71 us public path,
below its 4.07 us and 10% low-latency thresholds, and was removed. See
[the closure receipt](runtime_hypothesis_closures_2026-07-29.md).

### P1.5: Measure reporting overhead on the string-only SVG path

Each render session initializes and updates measurement-provenance counters, and
the string-only API constructs a completed report before discarding it. A/B a no-report terminal
path while preserving the report-returning API unchanged. This is a hypothesis, not a proven
cause; reject it if it cannot clear the preregistered public-scenario contract or justify a
separate structural or memory claim.

Exit: report APIs retain identical evidence, string APIs retain identical SVG/error behavior, and
the stage benchmark demonstrates any saving.

Completed on 2026-07-29. A minimal raw-string terminal candidate improved the smallest public Info
path by 0.391 us (9.76%), below the low-latency gate's 1 us and 10% minima. It was removed without
expanding the API surface or tests; see
[the closure receipt](runtime_hypothesis_closures_2026-07-29.md).

### P1.6: Optimize large Flowchart scaling

Build a size/density curve rather than tuning against one 420-line fixture. Attribute ordering,
crossing minimization, routing, text measurement, and SVG emission separately. Preserve
source-backed layout semantics and reject magic-number changes made only to improve the benchmark.

The clean `06616dd71` native allocator baseline completed 30 matched operation/zero pairs on an
Apple M4 Pro with Rust 1.95.0. Every repeat was deterministic, so the one-sided bootstrap bounds
collapsed to the estimates:

| Metric | Log-log slope upper bound | `100x` upper bound | Infrastructure cap | Result |
| --- | ---: | ---: | ---: | --- |
| Allocation count | 1.411568 | 4,265,827 | 2.0 / 10,000,000 | Pass |
| Allocated bytes | 2.338849 | 33,701,769,467 B | 2.0 / 8 GiB | Failed bound |
| Peak growth | 1.434359 | 144,199,480 B | 2.0 / 2 GiB | Pass |

This makes cumulative allocation the first qualified scaling target. It does not prove which
Flowchart stage owns the bytes; U4 still requires owner-local attribution and an adjacent public
latency comparison before retaining production code.

Exit: the curve identifies the first superlinear or allocation-heavy stage and a representative
large preview fits the agreed interactive budget.

Completed on 2026-07-29 with a split decision. The indexed Flowchart adapter candidate was rejected
and removed after it failed to produce an admissible public-operation result. The separate Dugong
batch-retirement mechanism remains because it bounds repeated adjacency reconstruction from
worst-case `O(T * (V + E))` to `O(T + V + E)`; its preregistered latency and memory admission was
not completed, so no measured speedup is claimed. Candidate-only memory lanes, contracts,
generators, and tests were deleted. See
[the adapter decision](flowchart_u4_adapter_candidate_2026-07-29.md) and
[the batch preregistration outcome](flowchart_u4_dugong_batch_preregistration_2026-07-28.md).

### P1.7: Bound Resvg finalization and export work

Remove input-amplifiable work from terminal reference and attribute validation independently of
ordinary-fixture latency. Measure one-shot raster export and ResvgSafe finalization on complete
public operations before changing ownership or error-order contracts.

Completed on 2026-07-29. Duplicate parsed-ID reference edges changed from `O(D * R)` to
`O(D + R)`, and duplicate expanded-attribute membership changed from `O(A^2)` to expected `O(A)`.
The one-worker PNG candidate saved only 0.78%; the single-reader XML candidate regressed by 0.57%.
Both latency candidates and their one-fixture benchmark lanes were removed. See [the U8
receipt](resvg_pipeline_candidates_2026-07-29.md).

### P1.8: Linearize rich inline HTML planning

Keep inline HTML segmentation source-backed and owner-local. A candidate may remove repeated
fragment/style discovery and scratch-payload copies, but opaque host measurer calls, request text,
order, and failure behavior remain observable and must not be reduced without a separate contract.
Use fixed-byte `R`, `K`, and orthogonal `R x K` structural controls; do not turn a structural
result into a latency claim without an adjacent public A/B.

Completed on 2026-08-07 with an accepted structural result. One indexed logical source and one
opaque style-group side index reduce Rust planning to `O(B + R + K)` plus explicitly observed
backend work, remove the growing `B * K` scratch-copy term, and preserve host/built-in semantics.
The side index adds `O(R + K)` logical `usize` slots; no peak-memory or latency improvement is
claimed. See [the decision receipt](rich_inline_html_planning_2026-08-07.md).

### P1.9: Remove duplicated theme and terminal SVG work

Reduce ownership overhead on the frozen theme-ownership branch without altering configuration
revisions, provenance, failure atomicity or terminal SVG admission. Keep fresh-process Windows
CLI startup as a separate full-capability experiment.

The 2026-10-14 candidate removes one complete JSON copy-on-write from exclusively owned legacy
theme calculation, consumes terminal reference collections instead of cloning discarded data,
and reuses adjacent final-byte observations to reduce explicit best-effort parity/readable XML
traversals from three to two. Observations have retention limits and fall back to the existing
scanner without changing error order. Strict/resvg-safe paths retain their validation scopes.

These are structural work reductions. Combined native diagnostics support non-regression but
do not establish a calibrated clean-revision latency or peak-memory improvement. The complete
Windows delay-import candidate did not meet its joint materiality screen and was rejected;
no linker setting or ordinary CI performance gate is retained. See
[the checkpoint and validation receipt](SVG_OWNED_WORK_2026-10-14.md).

## Guardrails

- Correctness and Mermaid parity take precedence over a timing ratio.
- Keep native, Node N-API, Node-WASM, and browser-WASM measurements in separate lanes.
- Do not infer a shared cause from unrelated family ratios.
- Do not add family allow-lists, bypass deterministic runtime policy, or remove sanitization.
- Keep exploratory Markdown/JSON under `target/bench`; check in only dated, decision-relevant
  evidence.
