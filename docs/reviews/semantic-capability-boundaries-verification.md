# Semantic capability boundaries: verification status

This audit follows the complete
[approved U1-U9 plan](../plans/2026-10-20-1252-refactor-semantic-capability-boundaries-plan.md).
The installed-product and timing source is `f0365e6a4948c4baff2f16adb735e0f143ee38c6` on
`refactor/semantic-capability-boundaries`. Final private-definition cfg cleanup and its Treemap
snapshot regression are committed at `599f714d344e94fd15c7f3a3abeb8be8ec51eeee`. Seven exact
production-feature Clippy selections, 428 owning tests and formatting pass on that patch; its
committed three-family artifact refresh also passes. The corrected 27-step integration queue,
including actual CLI archive execution, retains its original f036 source identity.

The objective remains open. The original 72-row timing cohort is terminal, with 65 confirmed
non-regressions and seven inconclusive A/A controls. Its separately authorized 24-row
follow-up is also terminal, with 21 confirmed and three inconclusive; it does not meet its
all-controls admission criterion or replace the historical result. A later quiet-window
24-row cohort and its independent audit are recorded at the end of this report; it too leaves
the registered all-controls performance criterion unmet.
The cfg cleanup changes private definitions and one intermediate field,
not runtime function bodies, public API, ABI, feature manifests or capability descriptors.
Its exact source applicability and final lean refresh are recorded in the
[continuity record](../knowledge/engineering/verification/2026-10/2026-10-23T030258Z-final-cfg-and-timing-e56e36605058434b9a90d501834db474.md).
Full artifact receipts are not relabeled as rebuilds on the cleanup revision. The original local
implementation scope excluded push, PR creation, merge, publication and release.

Following maintainer authorization to push the feature branch and create draft PR #241, the
feature branch integrated main's `83e3ea74b` version-baseline synchronization on 2026-10-24.
The conflict resolution keeps the `0.9.2` package metadata, current semantic/presentation APIs and
explicit-math recipes. Rust license reports and release projections were regenerated from the
merged manifests and locks. The three native-memory contract lockfile hashes were refreshed;
their thresholds and semantic output identities are unchanged. Historical size, allocation,
timing and installed-product receipts retain their original measured sources: this integration
does not relabel them as `0.9.2` measurements or close performance acceptance.
The PR remains a draft; no PR merge, publication or release is authorized by this integration.

## Main integration on 2026-10-26

The maintainer authorized merging `origin/main` at `93034e207` into the feature branch at
`3d2ee4c75`. This brings in main's native C/WASI SDKs, bounded memory-font constructors,
explicit system-font policy, Swimlane label fix and reviewed Mermaid security/baseline refresh.
The semantic/presentation API split and the math-free `complete-svg` conveniences remain intact.
The reference manifests, npm locks, selection receipt and bundle retain main's admitted graph;
this integration does not invent a new selection receipt or claim unchanged upstream SVG bytes.
Rust license reports and release projections are regenerated for the combined dependency graph.
Only the current native-memory contracts' lockfile fingerprints change; their workloads,
thresholds and semantic identities do not. Historical measurements retain their original sources.

Integration fixes align the bindings dependency contract with its explicit `compat-json` edge
without allowing font-feature leakage, and accept both LF and CRLF when reading generated SDK
header versions. A real workspace dependency-contract test and an LF/CRLF regression cover these
seams. The latter reproduced main's Windows fixture failure before the narrow parser correction.
Main's new Swimlane fallback regression now calls the existing `MermaidEngine` constructor;
the retired `Engine` alias is not restored.

Local evidence is retained under `target/ci-triage/merge-main-20261026/`. Generated source,
native ABI, binding contract and Mermaid reference checks pass. Feature-matrix unit tests pass
33/33, and the representative owner matrix passes all 49 builds and 16 isolated consumers.
The initial workflow/closure/legal Python suites pass 160 tests with three platform skips;
the SDK/archive/symbol/profile/legal suites pass 109 tests with one platform skip after the CRLF
fix. Performance-harness contract suites pass 20, 51, 33 and 157 tests respectively. These are
functional and contract checks, not new latency, allocation or artifact-size observations.
All 42 representative runtime dependency closures pass. Under the exact standard SDK recipes,
the native C library/header/consumer tests pass 67/67 and WASI host unit tests pass 17/17;
the standard reactor also passes `cargo check` for `wasm32-wasip1`. Native font regressions
pass 40/40 and the CI theme-consumer recipe, retaining both `system-fonts` and `compat-json`,
passes 84/84. `cargo deny check advisories bans licenses sources` passes. These local checks
do not qualify published archives, other native targets or real WASI hosts; those retain their
existing platform and release owners.
The sanitizer selection passes 55/55 tests. The two changed Swimlane/SVG-fallback integration
targets pass 27/27 after the constructor-name correction. The full 37-family release comparison
passes `structure,parity,parity-root` at three decimals with root diagnostics and the existing
exact browser-text-layout/parser-diagnostic receipts. No comparator policy or residual receipt
is loosened or added. Formatting, workflow syntax and diff/path checks also pass.
The open timing acceptance, platform-runtime evidence boundaries and independent npm audit
policy are unchanged; no risk exception, PR merge, publication or release is implied.

## Evidence ownership

The maintainer amended the SVG aggregate decision on 2026-10-25: `merman` and
`merman-rustdoc` retain `complete-svg` and `complete-svg-elk` as output/layout conveniences
without math or diagram selection. Mathematical-label consumers add `math` explicitly.
Source defaults, exact artifact recipes, capability IDs and transport contracts are unchanged.
The changelog, migration guide, ADR-0097 and implementation plan record this behavior change.
Earlier feature-matrix and artifact receipts below still describe their original sources,
including aggregate retirement; they are not relabeled as checks of this amendment. This
does not resume timing experiments or close the open performance acceptance criterion.

Focused amendment verification passes: 26 feature-matrix unit tests, the owner matrix's 49
build cases (including all four aggregate selections) and 16 isolated consumers, and both
existing Rustdoc math/family regressions under four feature selections (eight test executions).
Eight normal dependency graphs confirm that each aggregate omits RaTeX and diagram selectors
unless explicitly selected, with positive math graphs retaining RaTeX. All 36 exact artifact
profiles and 36 representative dependency-closure cases pass. The 13 Rust license reports and
424 release legal projections remain current; third-party licenses, 61 Python contracts,
formatting, and diff checks pass. Local logs are under
`target/ci-triage/pr241-svg-aggregates/`. These are scoped functional and dependency checks,
not full platform-runtime coverage or new artifact-size, allocation or timing measurements.

Final corrected artifacts are retained outside Cargo targets at
`F:/codex-builds/merman-semantic-capability-boundaries/evidence/final-f0365e6a4-20261023/`
(the **final root** below). Fresh historical U3/U4 allocation batches and the original strict
log are at the sibling `final-d0d26db24-20261023/` (**adjacent root**). Independent parser and
CLI process checks at `881b2be08` are at `renewed-20261022/` (**oracle root**).

These roots distinguish measured sources, not interchangeable final passes. The final root's
`corrected-source-applicability.json` records the d0-to-f036 source bridge. Full parser, grammar,
capability descriptors and ABI owners remain unchanged across that bridge; lean Flowchart
normalization and lazy binding engines have their own focused checks and rebuilt artifacts.
The preceding pure-alias retirement has a separate 1,494-test receipt and independent mechanical
review. No entire historical command is relabeled as a new command on the final revision.

A read-only retention check on 2026-10-23 verified the 27 corrected integration logs, 13 installed
Node step logs, Web smoke logs and all 447 retained package files, all 110 Typst files, five
Dart/native files, and five parser-oracle files against their recorded hashes. The retained size
probe executables and consumed outputs also match. The final root records this check in
`retained-product-evidence-audit-20261023T0129Z.json`; it is not another execution.
`final-cfg-source-applicability.json` binds the committed cleanup to its seven passing production
lints, 428 tests and final candidate size/output/closure receipt. Full recipes retain the changed
definition predicates; no full matrix or host rebuild is inferred from this source comparison.

## Implementation units

| Unit | Implementation and authoritative evidence | Remaining acceptance |
| --- | --- | --- |
| U1 | Harness-only `8628c554a`; consumed semantic/SVG probes and frozen recipes; current size receipts preserve comparable toolchain, corpus and outputs. | None for the harness. Deleted original raw samples remain historical; fresh observations have distinct receipts. |
| U2 | Percent/comment preprocessing and lexer correction; original UTF-8 spans; selected pinned Mermaid differential: 728 cases, 512 accepted, 216 rejected, zero mismatches. | No complete-language oracle claim. |
| U3 | Private requested facts demand and actual collector/append counters; renewed seven-lane allocation comparison passes 35 checks and all 168 metric cells do not grow. Separate timing cohorts: original 23/1, independent 5/1, quiet-window 0/6 confirmed/inconclusive. | Latest U3 E2E is wholly inconclusive from unstable base A/A; U3 performance acceptance remains unproven. |
| U4 | State consuming payload transfer, shared/hidden documents and required graph data; pointer-transfer, JSON/SVG and deep lifecycle checks; renewed allocation comparison passes 178 checks. Separate timing cohorts: original 23/1, independent 5/1, quiet-window 4/2 confirmed/inconclusive. | Latest State and Class controls are inconclusive from A/A identity/order intervals; complete U4 performance acceptance remains unproven. |
| U5 | Distinct semantic/presentation APIs and gated definitions; one family construction and paired full projection; isolated lean imports/doctests and canonical/full request checks. Integrated timing cohorts: original 19/5, independent 11/1, latest quiet-window 12/0 confirmed/inconclusive. | Keep each cohort separate; the original integrated all-controls criterion remains unmet, so overall performance acceptance is unproven. |
| U6 | Facade, ASCII, analysis, editor/LSP and binding routes use requested work; full custom adapters remain explicit; prepared outputs retain fresh operation control. | Current lean/full facade receipts pass: lean core 314, lean facade/ASCII 24, full projection 68; all-diagrams analysis 314, editor-core 132 and LSP 294. Corrected host checks pass within their stated platforms; unavailable platform owners remain CI-scoped. |
| U7 | Explicit math leaf, retired complete aggregates, direct non-math defaults and explicit positive recipes; 661 feature cases, 27 recipes, isolated closure checks, 24 CLI process profiles and actual Windows archive with positive math rendering. | None for the local recipe/installed-capability scope. |
| U8 | Capability/binding/legal projections, migration guide, ADRs and installed host packages agree; final documentation audit corrected five additional prose/recipe defects. | Unavailable platform runtime checks retain their existing CI owners; no publication. |
| U9 | Old engine names and 80 pure family aliases retired; committed cfg cleanup, seven exact production lints, 428 owner tests and final lean size refresh pass. Scoped correctness and installed products retain their source identities. Original 72-row timing cohort: 65 confirmed, seven inconclusive; independent 24-row follow-up: 21 confirmed, three inconclusive; later quiet-window cohort: 16 confirmed, eight inconclusive. | Every preregistered all-controls timing criterion remains unmet; no improvement is admitted. |

## Requirements and acceptance examples

| Requirement | Proof inspected | Result and limits |
| --- | --- | --- |
| R1 / AE1, AE10 | `merman-core/src/lib.rs`, family gates and isolated consumers; final `semantic-size-cfg-final-comparison.json` and committed 599f candidate receipt. | Three-family semantic program excludes presentation and is 25.2251% smaller, with identical consumed meaning. Exact closure/output/bytes retain the f036 result. Scope is the frozen Windows/MSVC recipe. |
| R2 / AE2 | Family construction and demand owners; `parse_projection_contract`, shared `support/parse_equivalence.rs`, Sequence/State construction counters. | One grammar/construction supplies requested results. Full/full comparison still checks complete metadata and compatibility JSON; lean/full comparisons retain their distinct phase contract. |
| R3 / AE4, AE11 | Config/security/family suites; `semantic_request_contract`; `732438a8a`; oracle and original-source span cases. | Lean Flowchart/Swimlane accessibility normalization restored at the family owner, including rich/recovered results. Full processing is unchanged. |
| R4 / AE2, AE6 | Real editor work counters, typed analysis capture, U3 allocations; binding lazy-engine counters in `engine.rs`. | Ordinary analysis/ASCII requests do not materialize full engines; no discarded compatibility JSON on the default analysis path. Public timing is terminal with the stated unmet controls. |
| R5 / AE5 | State consuming DB tests, semantic/full graph comparison, nested/shared JSON and State SVG tests; fresh U4 allocation receipt. | Three 100x State paths each remove 704 allocations and 8,608 allocated bytes; all 168 metric cells do not grow. Required graph and repeated JSON work remain. |
| R6 / AE3, AE5 | Full projection/config/theme suites and prepared pairs; final `svg-size-comparison.json`. | Full phase/error behavior and paired output retained. Same-capability SVG control is 0.3374% smaller; three exact SVG outputs match. This is not all-family timing. |
| R7 / AE3, AE4 | `semantic_presentation_boundary` and `semantic_request_contract`; admitted source config/security owners. | Valid semantics can omit visual theme evaluation; full operations retain theme errors. Malformed config and semantic/security failures retain their boundaries. |
| R8 / AE6 | ASCII authored-style and terminal tests; analysis diagnostics/rich facts; editor/LSP isolated recipe and consumer suites. | Actual consumers migrated. Presentation-only theme diagnostics are documented as full-only; explicit full custom analysis remains available. |
| R9 / AE5, AE12 | Deep lifecycle, grammar carriers, managed JSON, prepared/redraw/resource/typography/geometry suites; late-cancel regression and final Web mount 10/10. | Frozen parse facts and fresh output control/accounting retained. Binding prepared lifetime and post-failure recovery have focused coverage. |
| R10 / AE7, AE10, AE12 | Explicit custom backend routing; binding/generated ABI contracts; final Node, Web, Typst and Dart probes. | Rust migration is breaking; Web API 8, options schema 3, geometry schema 1 and native ABI 3 remain unchanged. No other-platform runtime claim. |
| R11 / AE8, AE10 | Actual Cargo defaults/edges, retired names, feature matrix, exact profile/closure evidence and CLI profile tests. | Math is absent from defaults and generic aggregates; positive selection remains available. Workspace union does not widen transport-owned catalogs. |
| R12 / AE8 | Required-math negative probes in final Web/Node/Typst/Dart artifacts; explicit positive math tests/profile; binding owner-feature test; exact Windows archive formula render. | Typed missing capability retained, with family-specific literal behavior; installed positive CLI math passes. |
| R13 / AE8, AE9 | CLI cargo-dist recipe and source defaults; CLI installation guidance, downstream recipe inspection, exact process matrix and actual archive verifier. | Official recipe explicitly selects math; source defaults omit it. The actual Windows ZIP reports math and renders a required formula. |
| R14 / AE1, AE2, AE5 | Frozen size comparisons including committed cfg candidate refresh, fresh U3/U4 allocation receipts and the separate 72-, 24- and 24-row timing cohorts. | Size and allocation evidence passes within its recorded scope. Timing cohorts retain respectively 65/7, 21/3 and 16/8 confirmed/inconclusive rows. The latest U3 E2E is wholly inconclusive, latest U4 has four confirmed/two inconclusive, and both integrated cohorts confirm all six rows. All-controls admission remains unmet; zero confirmed regressions or admitted speedups. |
| R15 / AE11 | Pinned oracle source/runtime identities, 728-case differential and original UTF-8 span tests. | Selected percent/comment observations match. This is not complete grammar equivalence. |
| R16 / AE7, AE9, AE10 | Alias inventory, current migration guide, generated/legal gates, final docs review and committed cfg cleanup. | Obsolete source APIs removed. Five remaining document defects corrected; scoped evidence and local commits retained. No general Rust semantic analyzer was introduced. |

## Verification gates

| Gate | Observed evidence | Applicability |
| --- | --- | --- |
| Workspace tests, all-target/all-feature Clippy, full doctests, generated contracts, legal projections and SVG DOM | Adjacent root `strict-historical/semantic-integrated-strict-external.log`; 13,307/13,307 tests, 661 feature cases and 27 recipes passed before the browser failure. | Historical integrated source; affected subsequent aliases and behavioral corrections have separate scoped receipts. All-feature lint does not prove each exact production-feature selection. |
| Production-feature Clippy | Final root `production-clippy-corrected-steps.json`, source inputs/patch and `cfg-cleanup-quality-verification.json`. | Seven exact selections pass: all-family core, three-family core, analysis/editor/LSP with stdio binary, ASCII, rust-static-svg, flutter-desktop-native and cli-release. Initial three unused-import failures remain in their original logs; the corrected Treemap import follows presentation. |
| Final cfg cleanup owners | Final root `gate-cleanup-corrected-verification.json`; source patch committed as 599f714d3. | Format and 79 semantic, 91 editor, 258 full tests pass: 428 total, including complete/recovered Treemap original-source snapshot facts. Original commands rejected duplicate nextest thread flags before tests ran; corrected commands use build-jobs 2 and test-threads 2. |
| Final lean artifact | Final root `semantic-size-cfg-final/receipt.json` and `semantic-size-cfg-final-comparison.json`. | Committed 599f candidate remains 2,126,336 raw/stripped bytes, 25.2251% smaller than frozen U1. Three exact consumed outputs and normal/full metadata package-feature edges match the prior candidate; no presentation/editor/compat/renderer closure. |
| Historical strict terminal status | Exit 1: Chromium 187 passed, one stale-WASM provenance failure, nine not run; non-Chromium 79 passed. | Remains a failed command. The complete mounted spec was subsequently rebuilt and rerun. |
| Alias retirement | Final root `scoped-retention-addendum/semantic-family-retirement-tests-corrected.log`, Clippy and isolated lean doctest logs. | 1,494/1,494 plus eight negative-import doctests passed. Failed initial selection log is retained. |
| Correctness corrections | Adjacent root `audit-fixes-steps.json`; current lean/full facade and language-tool receipts from `cargo nextest`. | Historical lean core 9, full core/projection 18, bindings 370, no-SVG binding 2, isolated Tree-sitter 30: 429 passed; owning Clippy and fmt passed. Current three-family lean core: 314/314; lean facade/ASCII: 24/24; full projection/ASCII/boundary/pipeline targets: 68/68; all-diagrams analysis: 314/314; editor-core: 132/132; LSP: 294/294. Additional focused receipts: all-features facade lib/tests 1,397/1,397 (`cfc66fe6-fbc9-402f-97aa-f1a6397dec55`) before presentation-only test target gating, and lean semantic/ASCII facade targets 16/16 (`a30a129a-953d-4f32-8d89-9de938e909f2`). After the gate change, the current all-features facade lib/tests pass 435/435 (two skipped), nextest run `c482fef3-4e36-4704-a633-bb366bda3f2a`. |
| Parser oracle | Final root `parser-differential-retention.json` and copied report/corpus/request/decoder/runtime. | Existing 881b2be08 execution retained; current harness hashes and compact corpus digest verified. No rerun claim. |
| Performance tooling | Final root `prior-scoped-logs/`; current `python tools/bench/test_perf_contracts.py` receipt after the `ceb318b53` target-gating change. | 157/157 performance contract tests and 33 native-memory contracts pass. Three memory-lane Cargo manifest input hashes were refreshed to the current manifest (`a4fbbf34f3ec1fa54fb77d151e1f2a1b4d63686a982054655afd8064d2dc980b`); this validates evidence identity only and is not a production latency result. |
| Node | Final root `node-installed-verification.json`; native and WASM tarballs/installations retained. | Each backend: 30 SVG renders, 26 JSON operations, 44 support queries, three-process recipe exchange. |
| Typst | Final root `typst-package.log`, `typst-retained-files.json`. | Typst 0.15.1: 23 compiled, nine expected failures; 110 retained files. No package publication. |
| Windows/Dart | Final root `dart-host-verification.json` and frozen tested DLL. | Windows GNU exports, Dart 3.11.6 ABI/capability/service/theme smoke pass, including required-math rejection. |
| Native/binding contracts | Current `cargo run --locked -p xtask -- verify-native-abi` and `verify-binding-contract` on `cabe01f8b`. | Both contract checks pass. The follow-up `scripts/verify-platform-bindings.py` reaches its existing Flutter owner but cannot resolve packages with installed Flutter/Dart 3.9.2 against the package minimum Dart 3.10.0; no ABI failure is inferred. |
| Web | Final root `web-host-verification.json` and five retained packages. | Build/source-input checks, 37-family smoke, math rejection, DOM safety and Chromium mount 10/10 pass; 447 retained files. |
| VS Code | Final root `prior-scoped-logs/semantic-vscode-final.log`. | Separate 255/255 pass after the historical strict command did not reach this gate. |
| Platform aggregate | `scripts/verify-platform-bindings.py`; `.github/workflows/ci.yml` platform and instrumentation owners. | Not passed locally: installed Flutter bundles Dart 3.9.2, below package >=3.10. Standalone Dart 3.11.6 and native smoke do not prove Flutter UI, Android emulator or Apple runtimes. Existing CI owners remain authoritative for those platforms. |
| CLI archive | Final root `cli-dist-local.log`, `cli-archive-execute.log`, `cli-archive-math-receipt.json`. | cargo-dist 0.32.0 built the local Windows ZIP; existing archive verifier executed it; exact ZIP math capability and formula render pass. No release was published. |
| Timing | Final root `run_timing.py`, `timing-source-amendment.json`, `final-runner-freeze.json`; original and independent U3/U4/integrated reports plus the quiet-window cohort reports and recomputation receipt in `semantic_parser_embedding_2026-10-22.md`. | Original 72-row cohort: 65 confirmed non-regression, seven inconclusive. First independent 24-row cohort: 21 confirmed, three inconclusive. Newly authorized quiet-window cohort: 16 confirmed, eight inconclusive. All reports have zero contract failures or confirmed regressions; no cohort meets its all-controls criterion. The new U3 E2E is wholly inconclusive under unstable base A/A; U4 has four confirmed and two inconclusive rows; integrated cold parse and integrated E2E each confirm all six rows. Every retained A/A and admitted AB/BA classification was recomputed with the registered harness; no outputs mismatch. Improvement is not admitted, including integrated E2E State whose mirrored improvement result remains inconclusive. No samples are pooled, no historical outcome is replaced, and the whole-plan timing acceptance remains unmet. |
| Feature and artifact contracts | Current `cargo run --locked -p xtask -- verify-feature-matrix` and `verify-artifact-profiles` on `ceb318b53`. | 45/45 feature-matrix builds and 36/36 exact artifact profiles pass. The test/example target gating added in this run does not widen feature catalogs or artifact profiles. |

The historical retention loss from `cargo clean` is documented in the two performance reports.
New samples do not reconstruct old data. The final corrected Web, Node, Typst and Dart receipts
are retained outside Cargo targets; their hashes are recorded in the associated performance report
and JSON manifests. No latency improvement or uncontended-host claim is made.

## Unmet acceptance

The first timing cohort retains seven inconclusive rows, its independent follow-up retains three,
and the newly authorized quiet-window cohort retains eight. In the latest cohort, all six U3 E2E
fixtures are inconclusive from unstable base A/A (State and Flowchart also exceed the registered
pair cap); U4 State and Class are inconclusive from A/A intervals outside the 50 us absolute
margin. Integrated cold-engine parse and integrated E2E each confirm all six non-regression rows.
These are separate fixed experiments and confidence scopes, not pooled evidence. The latest
cohort's 24 reports and logs are terminal and its full A/A and admitted A/B results were
deterministically recomputed from the registered harness. There are no contract failures,
confirmed regressions, or admitted improvements, but the registered all-controls timing
acceptance remains unmet. No retry or additional sample is authorized by these outcomes.

On 2026-10-23 the maintainer authorized an independent follow-up to start immediately. It is
preregistered at `F:/codex-builds/merman-semantic-capability-boundaries/evidence/followup-independent-20261023T042304Z/`
and completed four original six-fixture cohorts: U3 E2E, U4 E2E, integrated cold-engine parse
and integrated E2E. All 24 rows, including previously passing controls, remain required.
Original source pairs, frozen binaries, generic_v2 thresholds and the 32-pair cap are unchanged.
Fresh calibration and confirmation are separate from historical observations; no old row is
reclassified and no samples are pooled. Each cohort retains its original confidence scope;
this is not a joint 95% assertion across all four cohorts. Independent U3 E2E has since finished
with five confirmed non-regression rows and one inconclusive State row after unstable head A/A.
State remains unadmitted, so the new experiment cannot satisfy its all-24-controls criterion.
Independent U4 E2E also has five confirmed controls and one inconclusive State row; Class passes
that new observation. Independent integrated cold-engine parse adds six confirmed controls,
including Flowchart. The final integrated E2E cohort completed with five confirmed controls
and one inconclusive Class row: head A/A absolute identity and order intervals exceed the
50 us margins. Class has zero fresh confirmation pairs and no admitted means or bounds.
The parent deterministically recomputed all twelve final-cohort A/A summaries and the five
fresh confirmation intervals and classifications, and verified source, input, executable,
fixture, command and output identities.

| Independent cohort | Confirmed non-regression | Inconclusive | Fresh pairs per stable row | Unmet row |
| --- | ---: | ---: | ---: | --- |
| U3 E2E | 5 | 1 | 10 | State: head A/A |
| U4 E2E | 5 | 1 | 10 | State: both runners' A/A |
| Integrated cold-engine parse | 6 | 0 | 8 | None |
| Integrated E2E | 5 | 1 | 8 | Class: head A/A |
| Total | 21 | 3 | Separate cohort schedules | Three controls remain unadmitted |

The fixed queue completed at 2026-10-23T06:20:43Z. All 24 rows are terminal, with zero contract
failures or confirmed regressions. All 21 mirrored improvement classifications are
`confirmed_non_improvement`; no speedup is admitted. The independent root retains
`integrated-end_to_end-audit.json` (SHA-256
`3facd78941330bd4534fcefdd99af1ccfaf50ea1948656dc87bfec20f62f3a51`)
and `terminal-admission-audit.json` (SHA-256
`f957d2884a74fc404d75b6d656a766f37137a2745fdc8ca3e0e4c8649e831005`).
The three inconclusive rows leave the registered all-24 criterion unmet.
No historical result is replaced, no samples are pooled, and no combined
old/new pass count is an acceptance result.

At the time of this audit, the authorized local implementation, source cleanup, exact production
lint, owning tests, candidate artifact refresh and installed-product evidence work was complete
within the scopes above. Platform aggregate checks remain deferred to their existing CI owners,
as allowed by the plan. The failed historical strict command remains failed, with scoped Web and
VS Code recovery recorded separately. Subsequent timing evidence is recorded in the final
quiet-window section; it does not supersede this audit's source and product evidence. The overall
goal remains incomplete because the preregistered timing controls remain unadmitted. The fixed
queue has ended and no performance process remains. Any further measurement requires separate
authorization; no acceptance-contract change or blanket rebuild is implied here.

## Newly authorized measurement window

On 2026-10-23 the maintainer stated that the current window had no interference and explicitly
authorized starting now. A new 24-row experiment was sealed before sampling at
`F:/codex-builds/merman-semantic-capability-boundaries/evidence/controlled-host-20261023T083654Z/`
with registration SHA-256
`77e6f3fc243ba33a8632d27648be2cca7fbf09febcfdcb148ad7b7d0b0895687`.
It retains all four original six-fixture cohorts, clean source pairs, frozen binaries,
`generic_v2` thresholds, confidence scopes, fixed pair cap and all-controls admission rule.
This is an explicitly arranged new window, not an automatic repetition or historical
reclassification. Pre-sampling CPU observations were 4%, 4% and 2%; these do not independently
prove an isolated host. No other processes or system settings were changed.

The serial queue completed all four cohorts by 2026-10-23T10:22:55Z. The sealed registration
hash is unchanged, all step logs match their recorded hashes, and every report has zero
contract errors. Deterministic recomputation with the exact registered harness verified all
24 A/A rows and every admitted AB/BA row, including output identity and order. The resulting
machine-readable terminal audit is retained at
`F:/codex-builds/merman-semantic-capability-boundaries/evidence/controlled-host-20261023T083654Z/terminal-admission-audit.json`
with SHA-256
`e067810530c1ba87ca728a84158fa2b4d7e1aa6646c01dae0ef526065bf4079`; it records 48 A/A
recomputations, 128 admitted AB/BA pairs (256 runner samples) and zero matching benchmark
processes at audit time. The result is 16 confirmed non-regressions and eight inconclusive
rows; the all-24-controls admission criterion is unmet. The original 72-row and first
independent 24-row classifications remain unchanged and unpooled. No blanket rebuild,
production-code change, publication or acceptance-contract change was performed. The
objective remains incomplete pending another separately agreed experiment or an explicit
acceptance-contract decision; no further measurement is currently running or authorized.
